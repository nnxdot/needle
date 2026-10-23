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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

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
 * float (which carries 16- and 24-bit music as it is) or else 16-bit, at 44.1 kHz or else
 * 48 kHz. A mixer in another format would take nothing from Needle.
 */
@RequiresApi(34)
private fun bitPerfect(audio: AudioManager, device: AudioDeviceInfo): AudioMixerAttributes? = runCatching {
    audio.getSupportedMixerAttributes(device)
        .filter {
            it.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT &&
                it.format.channelMask == android.media.AudioFormat.CHANNEL_OUT_STEREO &&
                it.format.encoding in listOf(android.media.AudioFormat.ENCODING_PCM_FLOAT, android.media.AudioFormat.ENCODING_PCM_16BIT) &&
                it.format.sampleRate in listOf(44100, 48000)
        }
        .minByOrNull {
            (if (it.format.encoding == android.media.AudioFormat.ENCODING_PCM_FLOAT) 0 else 2) +
                (if (it.format.sampleRate == 44100) 0 else 1)
        }
}.getOrNull()

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
    return "In bit-perfect mode $name takes $bits at $rates kHz. Needle can send it only 16-bit or 32-bit float at 44.1 or 48 kHz so far, so it plays to it as usual."
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

/** The DAC last written to the log, so one plugged in is written once. */
private var loggedDac: String? = null

/** Needle's output opens in the mixer's format, or as Android likes when `mixer` is null. */
@RequiresApi(34)
private fun matchOutput(mixer: AudioMixerAttributes?) {
    val float = mixer?.format?.encoding == android.media.AudioFormat.ENCODING_PCM_FLOAT
    runCatching { core.setUsbFormat((mixer?.format?.sampleRate ?: 0).toUInt(), float) }
}

/**
 * Needle's output follows the USB DAC's mixer from the start and wherever the app is, not only
 * while Settings shows the switch: a DAC unplugged (or plugged back in) with bit-perfect on
 * changes the format Needle opens in at once, and so does a start with it already on.
 */
fun followUsbMixer(context: Context) {
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
 * Bit-perfect playback to a USB DAC (Android 14 and newer): Android stops mixing and hands
 * the DAC the music as it is, at its own rate. Other sounds are silent meanwhile.
 */
@Composable
fun UsbBitPerfect() {
    if (Build.VERSION.SDK_INT < 34) return
    val context = LocalContext.current
    val audio = remember { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }
    var device by remember { mutableStateOf(usbOutput(audio)) }
    DisposableEffect(audio) {
        val callback = object : android.media.AudioDeviceCallback() {
            override fun onAudioDevicesAdded(devices: Array<out AudioDeviceInfo>) { device = usbOutput(audio) }
            override fun onAudioDevicesRemoved(devices: Array<out AudioDeviceInfo>) {
                device = usbOutput(audio)
                if (device == null) matchOutput(null)
            }
        }
        audio.registerAudioDeviceCallback(callback, android.os.Handler(android.os.Looper.getMainLooper()))
        onDispose { audio.unregisterAudioDeviceCallback(callback) }
    }
    val connected = device
    val offer = remember(connected) { connected?.let { bitPerfect(audio, it) } }
    val why = remember(connected, offer) { if (connected != null && offer == null) whyNot(audio, connected) else null }
    androidx.compose.runtime.LaunchedEffect(connected) { connected?.let { logDac(audio, it) } }
    var on by remember(connected) {
        mutableStateOf(
            connected != null && runCatching {
                audio.getPreferredMixerAttributes(MUSIC, connected)?.mixerBehavior == AudioMixerAttributes.MIXER_BEHAVIOR_BIT_PERFECT
            }.getOrDefault(false),
        )
    }
    val usable = connected != null && offer != null
    fun setMixer(wanted: Boolean) {
        runCatching { toggle(audio, connected, offer, wanted) }
            .onSuccess { success ->
                if (success) {
                    on = wanted
                    matchOutput(if (wanted) offer else null)
                } else {
                    showMessage("Android could not change the USB mixer. Try reconnecting the DAC.")
                }
            }
            .onFailure { showMessage(it.message ?: "Android could not change the USB mixer") }
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            // The whole row is the one switch, named by its words.
            .toggleable(on, enabled = usable, role = androidx.compose.ui.semantics.Role.Switch) { setMixer(it) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text("Bit-perfect USB", style = MaterialTheme.typography.bodyLarge, color = if (usable) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant)
            Text(
                when {
                    connected == null -> "Plug in a USB DAC to send it the music exactly as it is"
                    offer == null -> why.orEmpty()
                    else -> "${connected.productName} gets the music exactly as it is, at ${kHz(offer.format.sampleRate)} kHz; other sounds wait"
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = on, enabled = usable, onCheckedChange = null)
    }
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(horizontal = 16.dp))
}

private fun toggle(audio: AudioManager, device: AudioDeviceInfo?, offer: AudioMixerAttributes?, on: Boolean): Boolean {
    if (Build.VERSION.SDK_INT < 34 || device == null) return false
    return if (on && offer != null) {
        audio.setPreferredMixerAttributes(MUSIC, device, offer)
    } else {
        audio.clearPreferredMixerAttributes(MUSIC, device)
    }
}
