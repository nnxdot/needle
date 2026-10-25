package fyi.nnx.needle

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbManager
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Needle's own USB DAC driver, on the Android side (Settings › Sound › Bit-perfect USB): finds
 * a USB audio device, asks Android for it, takes its audio interfaces from Android's own driver
 * (`claimInterface(force)`), and hands the connection to the core (`usbDacOpen`), which plays
 * the music to it as it is. While Needle holds the DAC, other apps play from the phone. When the
 * driver cannot play to it, the DAC goes back to Android and Android's bit-perfect mixer (if
 * there is one) is used instead (`ui/Usb.kt`).
 */
object UsbDriver {
    /** What the switch's line says. */
    sealed interface State {
        /** No USB audio device is plugged in. */
        data object None : State
        /** Waiting for the listener to allow Needle to use it. */
        data class Asking(val name: String) : State
        /** Needle's driver has it. */
        data class Driving(val info: fyi.nnx.needle.core.UsbDacInfo) : State
        /** Needle's driver could not play to it; Android's output has it. */
        data class Fallback(val name: String, val why: String) : State
    }

    private val _state = MutableStateFlow<State>(State.None)
    val state: StateFlow<State> = _state

    private const val PERMISSION = "fyi.nnx.needle.USB_PERMISSION"
    private var connection: UsbDeviceConnection? = null
    private var held: UsbDevice? = null
    private var started = false

    /** Follows USB devices from the app's start: called once, by [NeedleApp]. */
    fun start(context: Context) {
        if (started) return
        started = true
        val app = context.applicationContext
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context, intent: Intent) {
                when (intent.action) {
                    UsbManager.ACTION_USB_DEVICE_ATTACHED -> check(app)
                    UsbManager.ACTION_USB_DEVICE_DETACHED -> {
                        val gone = intent.device()
                        if (gone == null || gone.deviceName == held?.deviceName) release(app, State.None)
                        check(app)
                    }
                    PERMISSION -> {
                        val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                        val device = intent.device()
                        if (granted && device != null) open(app, device)
                        else if (device != null) _state.value = State.Fallback(device.productName ?: "The DAC", "Needle was not allowed to use it")
                    }
                }
            }
        }
        val filter = IntentFilter().apply {
            addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
            addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            addAction(PERMISSION)
        }
        if (Build.VERSION.SDK_INT >= 33) app.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        else @Suppress("UnspecifiedRegisterReceiverFlag") app.registerReceiver(receiver, filter)
        // The driver's own word: what it plays, or why it stopped (then Android's output plays).
        NeedleApp.instance.scope.launch {
            while (true) {
                delay(1500)
                if (_state.value !is State.Driving) continue
                val status = withContext(Dispatchers.IO) { runCatching { NeedleApp.instance.core.usbDacStatus() }.getOrNull() } ?: continue
                status.problem?.let { why ->
                    val name = (held?.productName ?: "The DAC")
                    NeedleLog.info("Needle's USB driver stopped for $name: $why")
                    release(app, State.Fallback(name, why))
                    fyi.nnx.needle.ui.useAndroidMixer(app, true)
                }
            }
        }
        check(app)
    }

    private fun Intent.device(): UsbDevice? =
        if (Build.VERSION.SDK_INT >= 33) getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
        else @Suppress("DEPRECATION") getParcelableExtra(UsbManager.EXTRA_DEVICE)

    /** A USB device with an audio streaming interface that plays (an OUT endpoint). */
    private fun isDac(device: UsbDevice): Boolean = (0 until device.interfaceCount).any { i ->
        val intf = device.getInterface(i)
        intf.interfaceClass == UsbConstants.USB_CLASS_AUDIO && intf.interfaceSubclass == 2 &&
            (0 until intf.endpointCount).any { intf.getEndpoint(it).direction == UsbConstants.USB_DIR_OUT }
    }

    /** Takes the DAC when the switch is on and one is plugged in; lets it go when it is off. */
    fun check(context: Context) {
        val on = NeedleApp.instance.app.value.usbBitPerfect
        val usb = context.getSystemService(UsbManager::class.java) ?: return
        val dac = usb.deviceList.values.firstOrNull(::isDac)
        when {
            dac == null -> { release(context, State.None) }
            !on -> release(context, State.None)
            held?.deviceName == dac.deviceName && _state.value is State.Driving -> {}
            _state.value is State.Fallback && held == null && (_state.value as State.Fallback).name == (dac.productName ?: "") -> {}
            usb.hasPermission(dac) -> open(context, dac)
            else -> {
                _state.value = State.Asking(dac.productName ?: "The DAC")
                val intent = PendingIntent.getBroadcast(
                    context, 0, Intent(PERMISSION).setPackage(context.packageName),
                    PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
                usb.requestPermission(dac, intent)
            }
        }
    }

    private fun open(context: Context, device: UsbDevice) {
        val usb = context.getSystemService(UsbManager::class.java) ?: return
        val name = device.productName ?: device.manufacturerName ?: "USB DAC"
        val connection = runCatching { usb.openDevice(device) }.getOrNull()
        if (connection == null) {
            _state.value = State.Fallback(name, "Android did not open it")
            return
        }
        // The audio interfaces, from Android's own driver.
        for (i in 0 until device.interfaceCount) {
            val intf = device.getInterface(i)
            if (intf.interfaceClass == UsbConstants.USB_CLASS_AUDIO) connection.claimInterface(intf, true)
        }
        NeedleApp.instance.scope.launch {
            val opened = withContext(Dispatchers.IO) {
                runCatching { NeedleApp.instance.core.usbDacOpen(connection.fileDescriptor, connection.rawDescriptors, name) }
            }
            opened.onSuccess { info ->
                this@UsbDriver.connection = connection
                held = device
                _state.value = State.Driving(info)
                // Android's mixer is not used meanwhile.
                fyi.nnx.needle.ui.useAndroidMixer(context, false)
                NeedleLog.info(
                    "Needle's USB driver has $name: UAC ${info.uac}, up to ${info.bits}-bit, rates ${info.rates.joinToString(",")}, " +
                        "DoP ${info.dop}, volume ${info.volume}",
                )
            }.onFailure { e ->
                closeConnection(connection, device)
                _state.value = State.Fallback(name, e.message ?: "It does not play with Needle's driver")
                NeedleLog.info("Needle's USB driver could not use $name: ${e.message}")
                fyi.nnx.needle.ui.useAndroidMixer(context, true)
            }
        }
    }


    private fun closeConnection(connection: UsbDeviceConnection, device: UsbDevice?) {
        // Released interfaces go back to Android's own driver.
        if (device != null) for (i in 0 until device.interfaceCount) runCatching { connection.releaseInterface(device.getInterface(i)) }
        runCatching { connection.close() }
    }

    /** Lets the DAC go back to Android. */
    fun release(context: Context, next: State) {
        val c = connection
        if (c != null) {
            runCatching { NeedleApp.instance.core.usbDacClose() }
            closeConnection(c, held)
        }
        connection = null
        held = null
        _state.value = next
    }
}
