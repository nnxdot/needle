package fyi.nnx.needle.ui

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.media.AudioMixerAttributes
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.collectAsState
import kotlinx.coroutines.launch
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Usb

/** The one line of a setting that cannot apply while Needle's own USB driver has the DAC. */
const val OffWithUsb = "Off while Bit-perfect USB is on"

/**
 * Whether the music goes to Needle's own USB driver now: such settings show as off, and keep what
 * is saved for later.
 */
@Composable
fun soundToolsOff(): Boolean {
    val driver by fyi.nnx.needle.UsbDriver.state.collectAsState()
    val playback = fyi.nnx.needle.NeedleApp.instance.playback.collectAsState()
    // Only this, not the whole playback, which changes at every tick.
    val network by remember { androidx.compose.runtime.derivedStateOf { playback.value?.networkOutput == true } }
    return driver.takesTheMusic(network)
}

private val MUSIC: AudioAttributes = AudioAttributes.Builder()
    .setUsage(AudioAttributes.USAGE_MEDIA)
    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
    .build()

@RequiresApi(34)
private fun usbOutput(audio: AudioManager): AudioDeviceInfo? = runCatching {
    audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).firstOrNull {
        it.type == AudioDeviceInfo.TYPE_USB_DEVICE || it.type == AudioDeviceInfo.TYPE_USB_HEADSET
    }
}.getOrNull()

/**
 * The DAC's bit-perfect mixer in a format Needle's output can send exactly: stereo, 32-bit
 * whole numbers (what DACs such as the FiiO BTR17 take, and which carry 16- and 24-bit music
 * as it is), else 32-bit float, else 16-bit, at 44.1 kHz or else 48 kHz. A mixer in another
 * format (24-bit packed in 3 bytes) would take nothing from Needle.
 */
@RequiresApi(34)
private fun bitPerfect(audio: AudioManager, device: AudioDeviceInfo): AudioMixerAttributes? = runCatching {
    audio.getSupportedMixerAttributes(device)
        .filter {
            it.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT &&
                it.format.channelMask == android.media.AudioFormat.CHANNEL_OUT_STEREO &&
                it.format.encoding in SENDABLE &&
                it.format.sampleRate in listOf(44100, 48000)
        }
        .minByOrNull { SENDABLE.indexOf(it.format.encoding) * 2 + (if (it.format.sampleRate == 44100) 0 else 1) }
}.getOrNull()

/** The encodings Needle's output can send a mixer exactly, the DAC's own (whole numbers) first. */
private val SENDABLE = listOf(
    android.media.AudioFormat.ENCODING_PCM_32BIT,
    android.media.AudioFormat.ENCODING_PCM_FLOAT,
    android.media.AudioFormat.ENCODING_PCM_16BIT,
)

/** A format's encoding in words: "16-bit", "24-bit", "32-bit float". */
private fun encodingName(encoding: Int): String = when (encoding) {
    android.media.AudioFormat.ENCODING_PCM_8BIT -> "8-bit"
    android.media.AudioFormat.ENCODING_PCM_16BIT -> "16-bit"
    android.media.AudioFormat.ENCODING_PCM_24BIT_PACKED -> "24-bit"
    android.media.AudioFormat.ENCODING_PCM_32BIT -> "32-bit"
    android.media.AudioFormat.ENCODING_PCM_FLOAT -> "32-bit float"
    else -> "encoding $encoding"
}

private fun kHz(rate: Int) = if (rate % 1000 == 0) "${rate / 1000}" else "${rate / 1000.0}"

/**
 * Why a DAC that is plugged in has no bit-perfect mode Needle can use, in words that say what
 * is missing: Android offers none for it on this phone, or it offers only formats Needle cannot
 * send yet. Needle plays to the DAC as usual either way, through Android's mixer.
 */
@RequiresApi(34)
private fun whyNot(audio: AudioManager, device: AudioDeviceInfo): String {
    val name = device.productName
    val all = runCatching { audio.getSupportedMixerAttributes(device) }.getOrNull()
        ?: return "Android did not say whether $name can play bit-perfect. Needle plays to it as usual."
    val perfect = all.filter { it.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT }
    if (perfect.isEmpty()) {
        return "This phone's Android offers no bit-perfect mode for $name. Needle plays to it as usual, through Android's mixer."
    }
    val bits = perfect.map { encodingName(it.format.encoding) }.distinct().joinToString(", ")
    val rates = perfect.map { it.format.sampleRate }.distinct().sorted().joinToString(", ") { kHz(it) }
    return "In bit-perfect mode $name takes $bits at $rates kHz. Needle can send it 16-bit, 32-bit, or 32-bit float at 44.1 or 48 kHz, but not 24-bit packed in 3 bytes yet, so it plays to it as usual."
}

/**
 * What Android says about a USB DAC, in Needle's log (once for each DAC plugged in): its
 * formats and the mixers Android offers for it. A report of a DAC that will not play
 * bit-perfect can then be answered from the log.
 */
@RequiresApi(34)
private fun logDac(audio: AudioManager, device: AudioDeviceInfo) {
    val key = "${device.id}:${device.productName}"
    if (key == loggedDac) return
    loggedDac = key
    val mixers = runCatching { audio.getSupportedMixerAttributes(device) }
    val listed = mixers.getOrNull()?.joinToString("; ") {
        val kind = if (it.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT) "bit-perfect" else "default"
        "$kind ${encodingName(it.format.encoding)} ${it.format.sampleRate} Hz ${it.format.channelCount} ch (mask ${it.format.channelMask})"
    }?.ifEmpty { "none" } ?: "failed: ${mixers.exceptionOrNull()?.javaClass?.simpleName}"
    val preferred = runCatching { audio.getPreferredMixerAttributes(MUSIC, device) }.getOrNull()
        ?.let { "${it.mixerBehavior} ${encodingName(it.format.encoding)} ${it.format.sampleRate} Hz" } ?: "none"
    val usable = bitPerfect(audio, device)?.let { "${encodingName(it.format.encoding)} ${it.format.sampleRate} Hz" } ?: "none"
    fyi.nnx.needle.NeedleLog.info(
        "USB DAC ${device.productName} (type ${device.type}) on Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}), " +
            "${Build.MANUFACTURER} ${Build.MODEL}: rates ${device.sampleRates.joinToString(",").ifEmpty { "any" }}, " +
            "encodings ${device.encodings.joinToString(",") { encodingName(it) }.ifEmpty { "any" }}, " +
            "channels ${device.channelCounts.joinToString(",").ifEmpty { "any" }}; mixers: $listed; preferred: $preferred; Needle can use: $usable",
    )
}

/**
 * What Android says of its outputs, for "Copy DAC details". By type only, but for USB: another
 * output's name can be a person's (their earbuds).
 */
fun usbPhoneDetails(context: Context): String {
    val out = StringBuilder()
    val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
    out.append("Needle $version on Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}), ${Build.MANUFACTURER} ${Build.MODEL}\n")
    out.append("Bit-perfect USB: ${if (fyi.nnx.needle.NeedleApp.instance.app.value.usbBitPerfect) "on" else "off"}\n")
    val audio = context.getSystemService(AudioManager::class.java) ?: return out.toString()
    val stream = AudioManager.STREAM_MUSIC
    out.append("Music volume: ${audio.getStreamVolume(stream)} of ${audio.getStreamMaxVolume(stream)}, fixed ${audio.isVolumeFixed}\n")
    val outputs = runCatching { audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).toList() }.getOrDefault(emptyList())
    out.append("Outputs by type: ${outputs.joinToString(", ") { it.type.toString() }}\n")
    for (device in outputs.filter { it.type == AudioDeviceInfo.TYPE_USB_DEVICE || it.type == AudioDeviceInfo.TYPE_USB_HEADSET }) {
        out.append(
            "USB output (type ${device.type}): ${device.productName}, rates ${device.sampleRates.joinToString(",").ifEmpty { "any" }}, " +
                "encodings ${device.encodings.joinToString(",") { encodingName(it) }.ifEmpty { "any" }}, " +
                "channels ${device.channelCounts.joinToString(",").ifEmpty { "any" }}\n",
        )
    }
    if (Build.VERSION.SDK_INT >= 33) {
        val route = runCatching { audio.getAudioDevicesForAttributes(MUSIC).map { it.type } }.getOrNull()
        out.append("Music goes to type: ${route?.joinToString(",") ?: "not known"}\n")
    }
    if (Build.VERSION.SDK_INT >= 34) {
        val usb = usbOutput(audio)
        val mixer = usb?.let { runCatching { audio.getPreferredMixerAttributes(MUSIC, it) }.getOrNull() }
        out.append("Preferred mixer: ${mixer?.let { "${it.mixerBehavior} ${encodingName(it.format.encoding)} ${it.format.sampleRate} Hz" } ?: "none"}\n")
    }
    val playback = runCatching { core.playback() }.getOrNull()
    out.append("Player: ${if (playback?.playing == true) "playing" else "not playing"}\n")
    return out.toString()
}

/** The DAC last written to the log, so one plugged in is written once. */
private var loggedDac: String? = null

/** The format last written to the log, so each change is written once (and "off" at a start, never). */
private var loggedOutput: String? = "off"

/**
 * Needle's output opens in the mixer's format (its rate, and 16-bit, 32-bit, or float), or as
 * Android likes when `mixer` is null. Which one goes in Needle's log.
 */
@RequiresApi(34)
private fun matchOutput(mixer: AudioMixerAttributes?) {
    val sample = when (mixer?.format?.encoding) {
        android.media.AudioFormat.ENCODING_PCM_32BIT -> fyi.nnx.needle.core.UsbSample.INT32
        android.media.AudioFormat.ENCODING_PCM_FLOAT -> fyi.nnx.needle.core.UsbSample.FLOAT32
        else -> fyi.nnx.needle.core.UsbSample.INT16
    }
    val rate = mixer?.format?.sampleRate ?: 0
    runCatching { core.setUsbOutput(rate.toUInt(), sample) }
        .onFailure { fyi.nnx.needle.NeedleLog.info("Bit-perfect USB: the output could not be set (${it.message})") }
    val shown = if (mixer == null) "off" else "${encodingName(mixer.format.encoding)} at $rate Hz"
    if (shown != loggedOutput) {
        loggedOutput = shown
        fyi.nnx.needle.NeedleLog.info(
            if (mixer == null) "Bit-perfect USB off: Needle's output opens as Android likes"
            else "Bit-perfect USB on: Needle's output opens in $shown",
        )
    }
}

/**
 * Needle's output follows the USB DAC's mixer from the start and wherever the app is, not only
 * while Settings shows the switch: a DAC unplugged (or plugged back in) with bit-perfect on
 * changes the format Needle opens in at once, and so does a start with it already on. Needle's
 * own driver (`UsbDriver`) starts here too; it has the DAC first when Bit-perfect USB is on.
 */
fun followUsbMixer(context: Context) {
    // Bit-perfect USB on from before 2.0 (Android's mixer chosen): the switch stays on.
    if (Build.VERSION.SDK_INT >= 34 && !fyi.nnx.needle.NeedleApp.instance.app.value.usbBitPerfect) {
        val audio = context.getSystemService(AudioManager::class.java)
        val dac = audio?.let { usbOutput(it) }
        val mixer = dac?.let { runCatching { audio.getPreferredMixerAttributes(MUSIC, it) }.getOrNull() }
        if (mixer?.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT) {
            fyi.nnx.needle.NeedleApp.instance.updateApp { it.copy(usbBitPerfect = true) }
        }
    }
    fyi.nnx.needle.UsbDriver.start(context)
    if (Build.VERSION.SDK_INT < 34) return
    val audio = context.getSystemService(AudioManager::class.java) ?: return
    fun check() {
        val dac = usbOutput(audio)
        dac?.let { logDac(audio, it) }
        val mixer = dac?.let { device -> runCatching { audio.getPreferredMixerAttributes(MUSIC, device) }.getOrNull() }
        matchOutput(mixer?.takeIf { it.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT })
    }
    check()
    audio.registerAudioDeviceCallback(
        object : android.media.AudioDeviceCallback() {
            override fun onAudioDevicesAdded(devices: Array<out AudioDeviceInfo>) = check()
            override fun onAudioDevicesRemoved(devices: Array<out AudioDeviceInfo>) = check()
        },
        android.os.Handler(android.os.Looper.getMainLooper()),
    )
}

/**
 * Android's own bit-perfect mixer for the DAC on or off (Android 14 and newer), with Needle's
 * output opened to match: the fallback when Needle's driver cannot play to a DAC.
 */
fun useAndroidMixer(context: Context, on: Boolean) {
    if (Build.VERSION.SDK_INT < 34) return
    val audio = context.getSystemService(AudioManager::class.java) ?: return
    val device = usbOutput(audio)
    val offer = device?.let { bitPerfect(audio, it) }
    val wanted = on && offer != null
    runCatching { toggle(audio, device, offer, wanted) }
    matchOutput(if (wanted) offer else null)
}

/**
 * Bit-perfect playback to a USB DAC: Needle's own driver plays the music to it as it is, at each
 * song's rate and depth (Android 8 and newer); where it cannot, Android's bit-perfect mixer
 * (Android 14 and newer). Other sounds play from the phone meanwhile.
 */
@Composable
fun UsbBitPerfect() {
    val context = LocalContext.current
    val app by fyi.nnx.needle.NeedleApp.instance.app.collectAsState()
    val driver by fyi.nnx.needle.UsbDriver.state.collectAsState()
    val audio = remember { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }
    // Android's mixer, for the fallback's words (Android 14 and newer).
    var device by remember { mutableStateOf(if (Build.VERSION.SDK_INT >= 34) usbOutput(audio) else null) }
    DisposableEffect(audio) {
        val callback = object : android.media.AudioDeviceCallback() {
            override fun onAudioDevicesAdded(devices: Array<out AudioDeviceInfo>) { if (Build.VERSION.SDK_INT >= 34) device = usbOutput(audio) }
            override fun onAudioDevicesRemoved(devices: Array<out AudioDeviceInfo>) { if (Build.VERSION.SDK_INT >= 34) device = usbOutput(audio) }
        }
        audio.registerAudioDeviceCallback(callback, android.os.Handler(android.os.Looper.getMainLooper()))
        onDispose { audio.unregisterAudioDeviceCallback(callback) }
    }
    val connected = device
    val offer = remember(connected) { if (Build.VERSION.SDK_INT >= 34) connected?.let { bitPerfect(audio, it) } else null }
    // The mixer's format in words (Android 14 and newer only).
    val offerText = remember(offer) { if (Build.VERSION.SDK_INT >= 34 && offer != null) "${encodingName(offer.format.encoding)} at ${kHz(offer.format.sampleRate)} kHz" else null }
    androidx.compose.runtime.LaunchedEffect(connected) { if (Build.VERSION.SDK_INT >= 34) connected?.let { logDac(audio, it) } }
    // What the driver plays now ("96 kHz, 24-bit"), read while the row shows.
    var playing by remember { mutableStateOf<String?>(null) }
    // Below 100 %, a DAC with no volume of its own hears lowered samples.
    var exact by remember { mutableStateOf(true) }
    androidx.compose.runtime.LaunchedEffect(driver) {
        while (driver is fyi.nnx.needle.UsbDriver.State.Driving) {
            val status = runCatching { core.usbDacStatus() }.getOrNull()
            playing = status?.playing
            exact = status?.bitPerfect ?: true
            kotlinx.coroutines.delay(1500)
        }
        playing = null
        exact = true
    }
    val replug by fyi.nnx.needle.UsbDriver.replug.collectAsState()
    val line = when (val d = driver) {
        is fyi.nnx.needle.UsbDriver.State.Driving -> {
            val now = playing ?: "Needle's own driver, up to ${d.info.bits}-bit"
            // The driver's words say it when it converts; the volume's are added here.
            d.info.name + ": " + now + (if (exact || "not bit-perfect" in now) "" else ", not bit-perfect below 100%")
        }
        is fyi.nnx.needle.UsbDriver.State.Fallback if replug != null -> "Unplug $replug and plug it in again to use Android's sound"
        is fyi.nnx.needle.UsbDriver.State.Asking -> "Allow Needle to use ${d.name}"
        is fyi.nnx.needle.UsbDriver.State.Fallback -> if (app.usbBitPerfect && offer != null) "${d.name}: Android's bit-perfect mixer, $offerText" else "${d.name} plays through Android"
        else -> if (replug != null) "Unplug $replug and plug it in again to use Android's sound"
        else if (connected != null && Build.VERSION.SDK_INT >= 23) "${connected.productName}: the exact sound; other sounds wait"
        else "Plug in a USB DAC for the exact sound"
    }
    // Why Needle's driver could not, in full, under the row.
    val why = (driver as? fyi.nnx.needle.UsbDriver.State.Fallback)?.why?.takeIf { app.usbBitPerfect }
    Column {
        SettingRow(
            androidx.compose.material.icons.Icons.Rounded.Usb,
            "Bit-perfect USB",
            line,
            // The whole row is the one switch, named by its words.
            Modifier.toggleable(app.usbBitPerfect, role = androidx.compose.ui.semantics.Role.Switch) { on ->
                fyi.nnx.needle.NeedleApp.instance.updateApp { it.copy(usbBitPerfect = on) }
                if (on) {
                    // Needle's driver first; the moment after, it has the DAC or Android's mixer does.
                    fyi.nnx.needle.NeedleApp.instance.scope.launch(kotlinx.coroutines.Dispatchers.Main) {
                        kotlinx.coroutines.delay(100)
                        fyi.nnx.needle.UsbDriver.check(context)
                    }
                } else {
                    fyi.nnx.needle.UsbDriver.release(context, fyi.nnx.needle.UsbDriver.State.None) {
                        useAndroidMixer(context, false)
                        fyi.nnx.needle.UsbDriver.check(context)
                    }
                }
            },
        ) { Switch(checked = app.usbBitPerfect, onCheckedChange = null) }
        if (why != null) {
            Text(
                if (replug != null) "Needle's own driver could not play to it ($why)."
                else "Needle's own driver could not play to it ($why), so ${if (offer != null) "Android's bit-perfect mixer" else "Android"} plays to it instead.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 68.dp, end = 16.dp, bottom = 14.dp),
            )
        }
        val plugged by fyi.nnx.needle.UsbDriver.plugged.collectAsState()
        plugged?.let { name ->
            Divider()
            SettingRow(
                androidx.compose.material.icons.Icons.Rounded.ContentCopy,
                "Copy DAC details",
                "What Needle read from $name, for a report",
                Modifier.clickable {
                    fyi.nnx.needle.UsbDriver.copyDetails(context) { text ->
                        if (text == null) {
                            showMessage("Needle could not read the DAC")
                        } else {
                            val clipboard = context.getSystemService(android.content.ClipboardManager::class.java)
                            clipboard?.setPrimaryClip(android.content.ClipData.newPlainText("DAC details", text))
                            showMessage("DAC details copied")
                        }
                    }
                },
            )
        }
    }
    Divider()
}

private fun toggle(audio: AudioManager, device: AudioDeviceInfo?, offer: AudioMixerAttributes?, on: Boolean): Boolean {
    if (Build.VERSION.SDK_INT < 34 || device == null) return false
    return if (on && offer != null) {
        audio.setPreferredMixerAttributes(MUSIC, device, offer)
    } else {
        audio.clearPreferredMixerAttributes(MUSIC, device)
    }
}
