package fyi.nnx.needle.ui

import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fyi.nnx.needle.WakeAlarm
import kotlinx.coroutines.delay

/** "In 9 hours 25 minutes". */
private fun until(at: Long): String {
    val minutes = ((at - System.currentTimeMillis()) / 60_000L).coerceAtLeast(0)
    val h = minutes / 60
    val m = minutes % 60
    return "In " + listOfNotNull(if (h > 0) count(h.toInt(), "hour") else null, if (m > 0 || h == 0L) count(m.toInt(), "minute") else null).joinToString(" ")
}

/**
 * Wake up to music (Android only, NEEDLE-2.0.md's mockups): the time, the days, what plays (a
 * playlist, a smart mix, or everything shuffled), Fade in, Snooze, and On. Android must allow
 * Needle exact alarms; when it does not, the page says so and opens that setting.
 */
@Composable
fun AlarmScreen() {
    val context = LocalContext.current
    var s by remember { mutableStateOf(WakeAlarm.read(context)) }
    var allowed by remember { mutableStateOf(WakeAlarm.allowed(context)) }
    var picking by remember { mutableStateOf(false) }
    var timing by remember { mutableStateOf(false) }
    // The countdown, and the permission, looked at again now and then.
    var tick by remember { mutableStateOf(0) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(20_000)
            tick++
            allowed = WakeAlarm.allowed(context)
        }
    }
    fun change(next: WakeAlarm.Setting) {
        s = next
        WakeAlarm.save(context, next)
    }
    val playlists by rememberLoaded { playlists() }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp + LocalBottomInset.current)) {
        item { LargeTitle("Wake up to music") }
        if (!allowed) item {
            Column(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 8.dp)) {
                Text("Android must let Needle ring on time.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                TextButton(onClick = {
                    tryOpen("alarm settings") {
                        context.startActivity(
                            Intent(android.provider.Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, android.net.Uri.parse("package:${context.packageName}"))
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                        )
                    }
                }) { Text("Allow alarms") }
            }
        }
        item {
            val next = remember(s, tick) { WakeAlarm.next(s) }
            Column(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    "%d:%02d".format(s.hour, s.minute),
                    fontSize = 88.sp,
                    fontWeight = FontWeight.Light,
                    color = if (s.on) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(16.dp)).clickable(onClickLabel = "Change the time") { timing = true }.padding(horizontal = 12.dp),
                )
                Text(
                    if (next != null) until(next) else "Off",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item {
            val letters = java.text.DateFormatSymbols.getInstance().shortWeekdays
            // Monday first: Calendar's 2..7, then 1 (Sunday).
            val order = listOf(2, 3, 4, 5, 6, 7, 1)
            Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
                order.forEachIndexed { i, calendarDay ->
                    val on = i in s.days
                    Text(
                        letters[calendarDay].take(1),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                        textAlign = TextAlign.Center,
                        color = if (on) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier
                            .size(40.dp)
                            .clip(CircleShape)
                            .background(if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh)
                            .clickable { change(s.copy(days = if (on) s.days - i else s.days + i)) }
                            .semantics { selected = on; contentDescription = letters[calendarDay] }
                            .padding(top = 10.dp),
                    )
                }
            }
        }
        item {
            Row(
                Modifier.fillMaxWidth().clickable { picking = true }.padding(horizontal = Edge, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        when (s.what) {
                            "playlist" -> s.playlistName.ifBlank { "A playlist" }
                            "shuffle" -> "Everything, shuffled"
                            else -> "A smart mix"
                        },
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    Text(
                        when (s.what) {
                            "playlist" -> "Playlist · shuffled"
                            "shuffle" -> "Your whole library"
                            else -> "From the song you played last"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        item { AlarmSwitch("Fade in", "From quiet to your volume in 2 minutes", s.fadeIn) { change(s.copy(fadeIn = it)) } }
        item { AlarmSwitch("Snooze", "10 minutes", s.snooze) { change(s.copy(snooze = it)) } }
        item { AlarmSwitch("On", null, s.on) { change(s.copy(on = it)) } }
    }
    if (timing) {
        LaunchedEffect(Unit) {
            android.app.TimePickerDialog(context, { _, h, m -> change(s.copy(hour = h, minute = m, on = true)) }, s.hour, s.minute, android.text.format.DateFormat.is24HourFormat(context))
                .apply { setOnDismissListener { timing = false } }
                .show()
        }
    }
    if (picking) {
        AlertDialog(
            onDismissRequest = { picking = false },
            confirmButton = { TextButton(onClick = { picking = false }) { Text("Done") } },
            title = { Text("What wakes you") },
            text = {
                LazyColumn {
                    item { PickRow("A smart mix", s.what == "mix") { change(s.copy(what = "mix")); picking = false } }
                    item { PickRow("Everything, shuffled", s.what == "shuffle") { change(s.copy(what = "shuffle")); picking = false } }
                    playlists.orEmpty().forEach { p ->
                        item { PickRow(p.name, s.what == "playlist" && s.playlistId == p.id) { change(s.copy(what = "playlist", playlistId = p.id, playlistName = p.name)); picking = false } }
                    }
                }
            },
        )
    }
}

@Composable
private fun PickRow(name: String, chosen: Boolean, onClick: () -> Unit) {
    Text(
        name,
        style = MaterialTheme.typography.bodyLarge,
        fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Normal,
        color = if (chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 12.dp),
    )
}

@Composable
private fun AlarmSwitch(title: String, sub: String?, on: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = Edge, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (sub != null) Text(sub, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(on, change)
    }
}
