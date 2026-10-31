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
import android.media.AudioManager
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * The order to give a device's audio interfaces back to Android: the audio control interface
 * last. Android's `releaseInterface` brings the kernel's driver back to each one at once. That
 * driver binds only to the audio control interface, and there it takes every streaming
 * interface that is free and skips the others for good. Given back first, the control
 * interface would bring back a sound card with nothing to play on (sound/usb/card.c in Linux).
 */
internal fun <T> controlLast(interfaces: List<T>, isControl: (T) -> Boolean): List<T> = interfaces.sortedBy(isControl)

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

    /** The name of the USB audio device plugged in, if any. */
    private val _plugged = MutableStateFlow<String?>(null)
    val plugged: StateFlow<String?> = _plugged

    private const val PERMISSION = "fyi.nnx.needle.USB_PERMISSION"
    @Volatile private var connection: UsbDeviceConnection? = null
    @Volatile private var held: UsbDevice? = null
    private var started = false

    /** Taking the DAC and giving it back, one at a time, in order. */
    private val changing = Mutex()

    /** Brings the phone's volume to the driver while it has the DAC. */
    private var following: Job? = null

    /** "Copy DAC details" waiting for the listener to allow Needle to look at the DAC. */
    @Volatile private var details: ((String?) -> Unit)? = null

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
                        details?.let { done ->
                            details = null
                            if (granted && device != null) copyDetails(app, done) else done(null)
                        }
                        if (!NeedleApp.instance.app.value.usbBitPerfect) return
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
                    release(app, State.Fallback(name, why)) { fyi.nnx.needle.ui.useAndroidMixer(app, true) }
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
        _plugged.value = dac?.let(::nameOf)
        when {
            dac == null -> { release(context, State.None) }
            !on -> {
                release(context, State.None)
                giveBackIfLost(context, dac)
            }
            held?.deviceName == dac.deviceName && _state.value is State.Driving -> {}
            _state.value is State.Fallback && held == null && (_state.value as State.Fallback).name == (dac.productName ?: "") -> {}
            usb.hasPermission(dac) -> open(context, dac)
            else -> {
                _state.value = State.Asking(dac.productName ?: "The DAC")
                ask(context, usb, dac)
            }
        }
    }

    private fun nameOf(device: UsbDevice) = device.productName ?: device.manufacturerName ?: "USB DAC"

    private fun ask(context: Context, usb: UsbManager, dac: UsbDevice) {
        val intent = PendingIntent.getBroadcast(
            context, 0, Intent(PERMISSION).setPackage(context.packageName),
            PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        usb.requestPermission(dac, intent)
    }

    private fun audioInterfaces(device: UsbDevice) =
        (0 until device.interfaceCount).map(device::getInterface).filter { it.interfaceClass == UsbConstants.USB_CLASS_AUDIO }

    /** The phone's music volume, 0 to 1. */
    private fun phoneVolume(context: Context): Float {
        val audio = context.getSystemService(AudioManager::class.java) ?: return 1f
        val top = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        return if (top > 0) audio.getStreamVolume(AudioManager.STREAM_MUSIC).toFloat() / top else 1f
    }

    /**
     * While the driver has the DAC, Android's volume reaches nothing: the phone's buttons and the
     * player's slider move a number only. So that number is brought to the driver, which sets
     * the DAC's own volume, or lowers the samples for a DAC with none.
     */
    private fun followVolume(context: Context) {
        following?.cancel()
        following = NeedleApp.instance.scope.launch(Dispatchers.IO) {
            var last = -1f
            while (true) {
                val now = phoneVolume(context)
                if (now != last) {
                    last = now
                    runCatching { NeedleApp.instance.core.usbDacVolume(now) }
                }
                delay(if (NeedleApp.instance.playback.value?.playing == true) 200 else 1000)
            }
        }
    }

    private fun open(context: Context, device: UsbDevice) {
        val usb = context.getSystemService(UsbManager::class.java) ?: return
        val name = nameOf(device)
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            changing.withLock {
                if (this@UsbDriver.connection != null || !NeedleApp.instance.app.value.usbBitPerfect) return@withLock
                val connection = runCatching { usb.openDevice(device) }.getOrNull()
                if (connection == null) {
                    _state.value = State.Fallback(name, "Android did not open it")
                    return@withLock
                }
                // The audio interfaces, from Android's own driver.
                for (intf in audioInterfaces(device)) connection.claimInterface(intf, true)
                val core = NeedleApp.instance.core
                runCatching { core.usbDacVolume(phoneVolume(context)) }
                runCatching { core.usbDacOpen(connection.fileDescriptor, connection.rawDescriptors, name) }.onSuccess { info ->
                    this@UsbDriver.connection = connection
                    held = device
                    _state.value = State.Driving(info)
                    // Android's mixer is not used meanwhile.
                    fyi.nnx.needle.ui.useAndroidMixer(context, false)
                    followVolume(context)
                    NeedleLog.info(
                        "Needle's USB driver has $name: UAC ${info.uac}, up to ${info.bits}-bit, rates ${info.rates.joinToString(",")}, " +
                            "DoP ${info.dop}, volume ${info.volume}",
                    )
                }.onFailure { e ->
                    _state.value = State.Fallback(name, e.message ?: "It does not play with Needle's driver")
                    NeedleLog.info("Needle's USB driver could not use $name: ${e.message}")
                    giveBack(connection, device)
                    fyi.nnx.needle.ui.useAndroidMixer(context, true)
                }
            }
        }
    }

    /** Hands the interfaces back to Android's own driver, and closes the connection. */
    private fun closeConnection(connection: UsbDeviceConnection, device: UsbDevice?) {
        if (device != null) {
            for (intf in controlLast(audioInterfaces(device)) { it.interfaceSubclass == 1 }) {
                runCatching { connection.releaseInterface(intf) }
            }
        }
        runCatching { connection.close() }
    }

    /**
     * The DAC goes back to Android, and Needle's output opens again a moment later, once the sound
     * card's files are back: one opened while the card was away may play to nothing.
     */
    private suspend fun giveBack(connection: UsbDeviceConnection, device: UsbDevice?) {
        closeConnection(connection, device)
        delay(500)
        runCatching { NeedleApp.instance.core.usbOutputAgain() }
    }

    /**
     * Lets the DAC go back to Android, off the main thread; `then` runs when that is done.
     * With no DAC held it runs at once.
     */
    fun release(context: Context, next: State, then: () -> Unit = {}) {
        val c = connection
        val device = held
        connection = null
        held = null
        following?.cancel()
        following = null
        _state.value = next
        if (c == null) {
            then()
            return
        }
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            changing.withLock {
                runCatching { NeedleApp.instance.core.usbDacClose() }
                giveBack(c, device)
            }
            then()
        }
    }

    /**
     * Needle ended while it held the DAC (a crash, or Android stopped it): the kernel let the
     * interfaces go, but gave them to no driver, so nothing plays on the DAC until it is plugged
     * in again. Android's own driver is brought back here. A claim without force works only on
     * an interface no driver has, so a DAC in order is not touched. Needs Android's leave from
     * before; no dialog is shown for this.
     */
    private fun giveBackIfLost(context: Context, device: UsbDevice) {
        val usb = context.getSystemService(UsbManager::class.java) ?: return
        if (!usb.hasPermission(device)) return
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            changing.withLock {
                if (connection != null) return@withLock
                val look = runCatching { usb.openDevice(device) }.getOrNull() ?: return@withLock
                val control = audioInterfaces(device).firstOrNull { it.interfaceSubclass == 1 }
                if (control != null && look.claimInterface(control, false)) {
                    NeedleLog.info("${nameOf(device)} had no driver: it goes back to Android")
                    giveBack(look, device)
                } else {
                    runCatching { look.close() }
                }
            }
        }
    }

    /**
     * What Needle read from the DAC, for a report (`done` gets null when there is none, or when
     * the listener said no). Reading it takes nothing from Android's own driver.
     */
    fun copyDetails(context: Context, done: (String?) -> Unit) {
        val usb = context.getSystemService(UsbManager::class.java) ?: return done(null)
        val dac = held ?: usb.deviceList.values.firstOrNull(::isDac) ?: return done(null)
        if (!usb.hasPermission(dac)) {
            details = done
            ask(context, usb, dac)
            return
        }
        NeedleApp.instance.scope.launch(Dispatchers.IO) {
            val text = changing.withLock {
                val own = connection
                val c = own ?: runCatching { usb.openDevice(dac) }.getOrNull() ?: return@withLock null
                val state = "Needle's driver: " + when (val s = _state.value) {
                    is State.Driving -> "it has the DAC"
                    is State.Asking -> "waiting to be allowed"
                    is State.Fallback -> "refused: ${s.why}"
                    State.None -> "off"
                }
                val dacText = runCatching {
                    NeedleApp.instance.core.usbDacDetails(c.fileDescriptor, c.rawDescriptors ?: ByteArray(0), nameOf(dac), dac.vendorId.toUShort(), dac.productId.toUShort())
                }.getOrElse { "Needle could not read it: ${it.message}\n" }
                if (own == null) runCatching { c.close() }
                fyi.nnx.needle.ui.usbPhoneDetails(context) + state + "\n" + dacText
            }
            done(text)
        }
    }
}
