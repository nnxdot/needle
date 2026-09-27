package fyi.nnx.needle.ui

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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import fyi.nnx.needle.core.PcSong
import fyi.nnx.needle.core.PcState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Runs a call to the computer off the main thread, and says what went wrong. */
private fun pc(work: () -> Unit) {
    fyi.nnx.needle.NeedleApp.instance.scope.launch {
        runCatching { work() }.onFailure { showMessage(it.message ?: "Your computer did not answer") }
    }
}

@Composable
fun ConnectScreen() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var connected by remember { mutableStateOf(core.pcConnected()) }
    var link by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var state by remember { mutableStateOf<PcState?>(null) }
    var query by remember { mutableStateOf("") }
    var found by remember { mutableStateOf<List<PcSong>>(emptyList()) }

    fun connect(address: String) {
        busy = true
        scope.launch {
            val result = withContext(Dispatchers.IO) { runCatching { core.connectPc(address) } }
            busy = false
            result.onSuccess { connected = true; showMessage("Connected to Needle on your computer") }
                .onFailure { showMessage(it.message ?: "Could not connect") }
        }
    }
    LaunchedEffect(Unit) {
        if (!connected) connected = withContext(Dispatchers.IO) { core.reconnectPc() }
    }
    LaunchedEffect(connected) {
        while (connected) {
            state = withContext(Dispatchers.IO) { runCatching { core.pcState() }.getOrNull() }
            delay(1000)
        }
    }
    LaunchedEffect(query, connected) {
        delay(300)
        found = if (!connected || query.isBlank()) emptyList()
        else withContext(Dispatchers.IO) { runCatching { core.pcSearch(query) }.getOrDefault(emptyList()) }
    }

    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item { LargeTitle("Your computer") }
        if (!connected) {
            item {
                Column(Modifier.padding(horizontal = Edge), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Icon(Icons.Rounded.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(48.dp))
                    Text("Control Needle on your computer, play its music here, and move what plays from one to the other.", style = MaterialTheme.typography.bodyLarge)
                    Text("On the computer, turn on the phone remote in Settings › Playback. Then scan its QR code, or type its address. Both need to be on the same network.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Button(enabled = !busy, onClick = {
                        val scanner = GmsBarcodeScanning.getClient(
                            context,
                            GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build(),
                        )
                        scanner.startScan()
                            .addOnSuccessListener { code -> code.rawValue?.let(::connect) }
                            .addOnFailureListener { showMessage("The scanner could not start: ${it.message}") }
                    }) {
                        Icon(Icons.Rounded.QrCodeScanner, contentDescription = null)
                        Text("  Scan the QR code")
                    }
                    OutlinedTextField(link, { link = it }, label = { Text("Or its address") }, placeholder = { Text("http://192.168.1.5:47380/r/…") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    FilledTonalButton(enabled = !busy && link.isNotBlank(), onClick = { connect(link) }) { Text("Connect") }
                }
            }
            return@LazyColumn
        }
        val s = state
        item {
            Column(
                Modifier.padding(horizontal = Edge).fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                val current = s?.current
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    AsyncImage(
                        model = coverRequest(current?.cover),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.size(64.dp).clip(SmallCoverShape).background(MaterialTheme.colorScheme.surfaceContainerHigh),
                    )
                    Column(Modifier.weight(1f)) {
                        Text(current?.title ?: "Nothing playing", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(current?.artist ?: "on your computer", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                    IconButton(onClick = { pc { core.pcDo("previous") } }) { Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous") }
                    IconButton(onClick = { pc { core.pcDo("toggle") } }) {
                        Icon(if (s?.playing == true) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, contentDescription = "Play or pause on the computer")
                    }
                    IconButton(onClick = { pc { core.pcDo("next") } }) { Icon(Icons.Rounded.SkipNext, contentDescription = "Next") }
                }
                var volume by remember(s?.volume) { mutableStateOf(s?.volume ?: 1f) }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.VolumeUp, contentDescription = "Volume on the computer", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    Slider(value = volume, onValueChange = { volume = it }, onValueChangeFinished = { pc { core.pcVolume(volume) } }, modifier = Modifier.weight(1f).padding(start = 8.dp))
                }
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Button(enabled = current != null, onClick = {
                        pc { if (core.moveFromPc()) showMessage("Playing here now") }
                        fyi.nnx.needle.NeedleApp.instance.openPlayer.tryEmit(Unit)
                    }) { Text("Play here") }
                    FilledTonalButton(onClick = {
                        pc {
                            val moved = core.moveToPc()
                            if (moved > 0u) showMessage("Playing on your computer now")
                        }
                    }) { Text("Send to the computer") }
                }
            }
        }
        item { SectionHeader("Search your computer") }
        item {
            OutlinedTextField(query, { query = it }, placeholder = { Text("Songs, albums, artists") }, singleLine = true, shape = RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = Edge))
        }
        items(found, key = { it.id }) { song ->
            var menu by remember { mutableStateOf(false) }
            Row(
                Modifier.fillMaxWidth().clickable { menu = !menu }.padding(horizontal = Edge, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                AsyncImage(coverRequest(song.cover), contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.size(48.dp).clip(SmallCoverShape).background(MaterialTheme.colorScheme.surfaceContainerHigh))
                Column(Modifier.weight(1f)) {
                    Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            if (menu) {
                Row(Modifier.padding(start = Edge + 62.dp, bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilledTonalButton(onClick = {
                        core.playFromPc(listOf(song), 0u, 0.0)
                        fyi.nnx.needle.NeedleApp.instance.openPlayer.tryEmit(Unit)
                        menu = false
                    }) { Text("Play here") }
                    TextButton(onClick = { pc { core.pcPlay(listOf(song.id)) }; menu = false }) { Text("Play on the computer") }
                }
            }
        }
        item {
            TextButton(onClick = { core.disconnectPc(); connected = false; state = null }, modifier = Modifier.padding(horizontal = 12.dp, vertical = 16.dp)) {
                Text("Disconnect")
            }
        }
    }
}
