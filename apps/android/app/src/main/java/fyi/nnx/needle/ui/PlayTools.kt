package fyi.nnx.needle.ui

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bedtime
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.SleepTimerState
import fyi.nnx.needle.core.SpeedPitch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ---------- The sleep timer

/** The sleep timer while it runs, read a few times a second (null while it does not). */
@Composable
fun rememberSleepTimer(): SleepTimerState? {
    val state by produceState<SleepTimerState?>(null) {
        while (isActive) {
            value = withContext(Dispatchers.IO) { runCatching { core.sleepTimer() }.getOrNull() }
            delay(if (value != null) 500 else 1500)
        }
    }
    return state
}

/** "stops at 22:18", in the phone's own clock format. */
private fun stopsAt(context: Context, unixSeconds: Long): String =
    android.text.format.DateFormat.getTimeFormat(context).format(java.util.Date(unixSeconds * 1000))

/** Minutes left, as the sheet says it: "12 min" or "1 h 05 min". */
private fun left(seconds: Double): String {
    val m = kotlin.math.ceil(seconds / 60).toInt().coerceAtLeast(0)
    return if (m >= 60) "%d h %02d min".format(m / 60, m % 60) else "$m min"
}

/**
 * The sleep timer (NEEDLE-2.0.md, "Sleep timer"): 15, 30, 45 minutes, 1 hour, End of song, End
 * of album; Fade out over the last minute; Shake to add 10 minutes (Android's own). While it
 * runs: when it stops, Add 10 minutes, and Turn off.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SleepTimerSheet(onDismiss: () -> Unit) {
    val context = LocalContext.current
    val running = rememberSleepTimer()
    val minutes = remember { runCatching { core.sleepMinutes() }.getOrDefault(listOf(15u, 30u, 45u, 60u)) }
    // The choice: "15".."60" for minutes, or "end_of_song", "end_of_album".
    var choice by remember { mutableStateOf("30") }
    var fade by remember { mutableStateOf(runCatching { core.sleepFade() }.getOrDefault(true)) }
    var shake by remember { mutableStateOf(SleepShake.on(context)) }
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = state, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 16.dp).navigationBarsPadding()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.Bedtime, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(10.dp))
                Text("Sleep timer", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
            }
            Text(
                if (running == null) "Music stops softly when the time is up."
                else "Stops at ${stopsAt(context, running.endsAt)}, in ${left(running.remaining)}" + when (running.kind) {
                    "end_of_song" -> ", at the end of this song."
                    "end_of_album" -> ", at the end of this album."
                    else -> "."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 14.dp),
            )
            if (running == null) {
                val pill = FilterChipDefaults.filterChipColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    selectedContainerColor = MaterialTheme.colorScheme.primary,
                    selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
                )
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    minutes.forEach { m ->
                        FilterChip(choice == "$m", { choice = "$m" }, label = { Text(if (m.toInt() == 60) "1 hour" else "$m min") }, shape = RoundedCornerShape(50), colors = pill, border = null)
                    }
                    FilterChip(choice == "end_of_song", { choice = "end_of_song" }, label = { Text("End of song") }, shape = RoundedCornerShape(50), colors = pill, border = null)
                    FilterChip(choice == "end_of_album", { choice = "end_of_album" }, label = { Text("End of album") }, shape = RoundedCornerShape(50), colors = pill, border = null)
                }
            }
            SheetSwitch("Fade out", "Over the last minute", fade) { on ->
                fade = on
                runCatching { core.setSleepFade(on) }
            }
            SheetSwitch("Shake to add 10 minutes", null, shake) { on ->
                shake = on
                SleepShake.set(context, on)
            }
            Spacer(Modifier.height(16.dp))
            if (running == null) {
                val ends = remember(choice) {
                    choice.toIntOrNull()?.let { stopsAt(context, System.currentTimeMillis() / 1000 + it * 60L) }
                }
                Button(
                    onClick = {
                        val m = choice.toIntOrNull()
                        runCatching { core.startSleepTimer(if (m != null) "minutes" else choice, (m ?: 0).toUInt(), fade) }
                            .onFailure { showMessage(it.message ?: "The sleep timer did not start") }
                        onDismiss()
                    },
                    shape = CircleShape,
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                    modifier = Modifier.fillMaxWidth().height(52.dp),
                ) { Text(if (ends != null) "Start · stops at $ends" else "Start", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold) }
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    FilledTonalButton(onClick = { runCatching { core.addSleepMinutes(10u) } }, modifier = Modifier.weight(1f).height(52.dp)) { Text("Add 10 minutes") }
                    Button(
                        onClick = { runCatching { core.cancelSleepTimer() }; onDismiss() },
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                        modifier = Modifier.weight(1f).height(52.dp),
                    ) { Text("Turn off") }
                }
            }
        }
    }
}

/** A switch row in a sheet: the words at the left, the switch at the right. */
@Composable
private fun SheetSwitch(title: String, sub: String?, on: Boolean, change: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (sub != null) Text(sub, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(on, change)
    }
}

/**
 * Shake to add 10 minutes (Android only): while a sleep timer runs and this is on, a firm shake
 * of the phone gives it 10 more minutes, with a short buzz. Kept on the phone, not with the
 * library's settings (the computers have no such thing).
 */
object SleepShake {
    private const val Key = "sleep_shake"
    private var listening: SensorEventListener? = null

    fun on(context: Context): Boolean = context.getSharedPreferences("needle", Context.MODE_PRIVATE).getBoolean(Key, false)

    fun set(context: Context, on: Boolean) {
        context.getSharedPreferences("needle", Context.MODE_PRIVATE).edit().putBoolean(Key, on).apply()
    }

    /** Watches the sleep timer for as long as Needle runs, listening for shakes only while it is needed. */
    fun start(context: Context) {
        val app = context.applicationContext
        NeedleApp.instance.scope.launch(Dispatchers.Main) {
            while (isActive) {
                val running = withContext(Dispatchers.IO) { runCatching { NeedleApp.instance.core.sleepTimer() }.getOrNull() } != null
                if (running && on(app)) listen(app) else stop(app)
                delay(2000)
            }
        }
    }

    private fun listen(context: Context) {
        if (listening != null) return
        val sensors = context.getSystemService(SensorManager::class.java) ?: return
        val sensor = sensors.getDefaultSensor(Sensor.TYPE_ACCELEROMETER) ?: return
        var peaks = 0
        var firstPeak = 0L
        var lastAdded = 0L
        val listener = object : SensorEventListener {
            override fun onSensorChanged(event: SensorEvent) {
                val (x, y, z) = Triple(event.values[0], event.values[1], event.values[2])
                val g = kotlin.math.sqrt(x * x + y * y + z * z) / SensorManager.GRAVITY_EARTH
                if (g < 2.3f) return
                val now = android.os.SystemClock.uptimeMillis()
                if (now - firstPeak > 700) { firstPeak = now; peaks = 0 }
                peaks++
                // Two hard jolts within a moment, and not again for three seconds.
                if (peaks >= 2 && now - lastAdded > 3000) {
                    lastAdded = now
                    peaks = 0
                    runCatching { NeedleApp.instance.core.addSleepMinutes(10u) }
                    buzz(context)
                    showMessage("10 more minutes")
                }
            }

            override fun onAccuracyChanged(sensor: Sensor, accuracy: Int) {}
        }
        sensors.registerListener(listener, sensor, SensorManager.SENSOR_DELAY_UI)
        listening = listener
    }

    private fun stop(context: Context) {
        val l = listening ?: return
        context.getSystemService(SensorManager::class.java)?.unregisterListener(l)
        listening = null
    }

    private fun buzz(context: Context) {
        runCatching {
            val vibrator = if (android.os.Build.VERSION.SDK_INT >= 31) {
                context.getSystemService(android.os.VibratorManager::class.java)?.defaultVibrator
            } else {
                @Suppress("DEPRECATION")
                context.getSystemService(android.os.Vibrator::class.java)
            }
            vibrator?.vibrate(android.os.VibrationEffect.createOneShot(60, android.os.VibrationEffect.DEFAULT_AMPLITUDE))
        }
    }
}

// ---------- Speed and pitch

/**
 * Speed and pitch (NEEDLE-2.0.md): speed 0.5× to 2× (chips 0.75, 1, 1.25, 1.5, 2), pitch −12 to
 * +12 semitones, Keep the pitch, Only for this song. Off with Bit-perfect USB, where nothing may
 * change the sound.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SpeedPitchSheet(onDismiss: () -> Unit) {
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    var now by remember { mutableStateOf<SpeedPitch?>(null) }
    LaunchedEffect(Unit) { now = withContext(Dispatchers.IO) { runCatching { core.speedAndPitch() }.getOrNull() } }
    val presets = remember { runCatching { core.speedPresets() }.getOrDefault(listOf(0.75f, 1f, 1.25f, 1.5f, 2f)) }
    var speed by remember { mutableFloatStateOf(1f) }
    var pitch by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(now) { now?.let { speed = it.speed; pitch = it.semitones } }
    fun apply(change: (SpeedPitch) -> SpeedPitch) {
        val v = now ?: return
        val next = change(v)
        now = next
        scope.launch(Dispatchers.IO) { runCatching { core.setSpeedAndPitch(next) } }
    }
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = state, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 16.dp).navigationBarsPadding()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Rounded.Speed, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(10.dp))
                Text("Speed and pitch", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
            }
            val v = now
            if (v != null && !v.available) {
                Text(
                    "Off while Bit-perfect USB plays: nothing may change the sound then.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 8.dp, bottom = 8.dp),
                )
                return@Column
            }
            val enabled = v != null
            Row(Modifier.fillMaxWidth().padding(top = 16.dp)) {
                Text("Speed", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                Text("%.2f×".format(speed).replace(".00×", "×").replace("0×", "×"), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.primary)
            }
            Slider(
                value = speed,
                onValueChange = { speed = (kotlin.math.round(it * 20) / 20f) },
                onValueChangeFinished = { apply { it.copy(speed = speed) } },
                valueRange = 0.5f..2f,
                enabled = enabled,
            )
            val pill = FilterChipDefaults.filterChipColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                selectedContainerColor = MaterialTheme.colorScheme.primary,
                selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                presets.forEach { p ->
                    FilterChip(
                        selected = kotlin.math.abs(speed - p) < 0.01f,
                        onClick = { speed = p; apply { it.copy(speed = p) } },
                        label = { Text("%.2f×".format(p).replace(".00×", "×").replace("0×", "×")) },
                        enabled = enabled,
                        shape = RoundedCornerShape(50),
                        colors = pill,
                        border = null,
                    )
                }
            }
            Row(Modifier.fillMaxWidth().padding(top = 16.dp)) {
                Text("Pitch", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                val semis = kotlin.math.round(pitch).toInt()
                Text(
                    (if (semis >= 0) "+" else "") + "$semis semitones",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Slider(
                value = pitch,
                onValueChange = { pitch = kotlin.math.round(it) },
                onValueChangeFinished = { apply { it.copy(semitones = pitch) } },
                valueRange = -12f..12f,
                steps = 23,
                enabled = enabled,
            )
            SheetSwitch("Keep the pitch", "Faster without the chipmunk voice", v?.keepPitch ?: true) { on -> apply { it.copy(keepPitch = on) } }
            SheetSwitch("Only for this song", null, v?.onlyThisSong ?: false) { on -> apply { it.copy(onlyThisSong = on) } }
            if (speed != 1f || pitch != 0f) {
                TextButton(onClick = {
                    speed = 1f
                    pitch = 0f
                    apply { it.copy(speed = 1f, semitones = 0f) }
                }) { Text("Back to normal") }
            }
        }
    }
}
