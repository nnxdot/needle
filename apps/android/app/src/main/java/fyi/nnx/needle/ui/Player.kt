package fyi.nnx.needle.ui

import android.os.Build
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.spring
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.text.withStyle
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Repeat
import androidx.compose.material.icons.rounded.RepeatOne
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.material3.LocalContentColor
import androidx.palette.graphics.Palette
import coil3.compose.AsyncImage
import coil3.imageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import fyi.nnx.needle.NeedleApp
import fyi.nnx.needle.core.RepeatMode
import fyi.nnx.needle.core.Song
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The song playing, floating above the tabs as in Apple Music: cover, title, play or pause, and
 * next. Tap it for the full player.
 */
@Composable
fun MiniPlayer(onOpen: () -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    val haptics = rememberHaptics()
    // Swiped sideways, it skips: left for the next song, right for the one before.
    var drag by remember { mutableFloatStateOf(0f) }
    val shift by animateFloatAsState(drag, spring(dampingRatio = 0.7f, stiffness = 500f), label = "mini swipe")
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        shape = RoundedCornerShape(16.dp),
        shadowElevation = 8.dp,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 10.dp, vertical = 8.dp)
            .graphicsLayer { translationX = shift; alpha = 1f - (kotlin.math.abs(shift) / 900f).coerceIn(0f, 0.5f) }
            .pointerInput(song.id) {
                detectHorizontalDragGestures(
                    onDragEnd = {
                        when {
                            drag < -160f -> { haptics(false); core.next() }
                            drag > 160f -> { haptics(false); core.previous() }
                        }
                        drag = 0f
                    },
                    onDragCancel = { drag = 0f },
                ) { _, dx -> drag += dx }
            }
            .clip(RoundedCornerShape(16.dp))
            .clickable(onClick = onOpen),
    ) {
        Box {
            Row(
                Modifier.padding(start = 8.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Cover(song.artwork, Modifier.size(44.dp).sharedCover("now-${song.id}"), SmallCoverShape)
                Column(Modifier.weight(1f)) {
                    Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        song.artist,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                IconButton(onClick = { haptics(false); core.toggle() }) {
                    PlayPauseIcon(p.playing, 30.dp)
                }
                IconButton(onClick = { core.next() }) {
                    Icon(Icons.Rounded.SkipNext, contentDescription = "Next", modifier = Modifier.size(30.dp))
                }
            }
            // A hairline of progress along the bottom edge.
            val progress = if (song.duration > 0) (p.position / song.duration).toFloat().coerceIn(0f, 1f) else 0f
            Box(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth(progress)
                    .height(2.dp)
                    .background(MaterialTheme.colorScheme.primary),
            )
        }
    }
}

/** The cover's strongest colour, where the background cannot be a blurred cover. */
@Composable
private fun coverColor(path: String?): Color {
    val context = LocalContext.current
    var color by remember { mutableStateOf(Color(0xFF3A2E20)) }
    LaunchedEffect(path) {
        if (path == null) return@LaunchedEffect
        val found = withContext(Dispatchers.IO) {
            val request = ImageRequest.Builder(context).data(fileUri(path)).size(96).allowHardware(false).build()
            val bitmap = (context.imageLoader.execute(request) as? SuccessResult)?.image?.toBitmap()
                ?: return@withContext null
            val palette = Palette.from(bitmap).generate()
            (palette.darkVibrantSwatch ?: palette.dominantSwatch)?.rgb
        }
        if (found != null) color = Color(found)
    }
    return animateColorAsState(color, tween(400), label = "cover colour").value
}

/** Behind the player: the cover itself, blurred and darkened, as in Apple Music. */
@Composable
private fun PlayerBackdrop(song: Song) {
    val glow = coverColor(song.artwork)
    Box(Modifier.fillMaxSize().background(Color(0xFF0F0E0D))) {
        val app by NeedleApp.instance.app.collectAsState()
        if (app.coverBackdrop && Build.VERSION.SDK_INT >= 31 && song.artwork != null) {
            AnimatedContent(song.artwork, transitionSpec = { fadeIn(tween(500)) togetherWith fadeOut(tween(500)) }, label = "backdrop") { art ->
                AsyncImage(
                    model = fileUri(art),
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxSize()
                        .graphicsLayer { scaleX = 1.4f; scaleY = 1.4f }
                        .blur(90.dp),
                )
            }
            Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.35f)))
        } else {
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(glow, Color(0xFF0F0E0D)))))
        }
        // Darker at the foot, so the controls always read.
        Box(
            Modifier.fillMaxSize().background(
                Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f)),
            ),
        )
    }
}

private enum class Panel { Cover, Lyrics, UpNext }

@Composable
fun FullPlayer(onClose: () -> Unit, open: (Route) -> Unit) {
    val playback by NeedleApp.instance.playback.collectAsState()
    val p = playback ?: return
    val song = p.current ?: return
    var panel by rememberSaveable { mutableStateOf(Panel.Cover) }
    val haptics = rememberHaptics()
    var favorite by remember(song.id) { mutableStateOf(song.rating >= 4) }
    // The A-B loop: the first tap marks A, the second B, the third lets go.
    var loopStart by remember(song.id) { mutableStateOf<Double?>(null) }
    val looping = remember(p.position, song.id) { core.loopRange() }
    // Pulled down far enough, the player closes.
    var pull by remember { mutableFloatStateOf(0f) }

    Box(
        Modifier
            .fillMaxSize()
            .graphicsLayer { translationY = pull.coerceAtLeast(0f) }
            .clickable(enabled = false) {}
            .pointerInput(Unit) {
                detectVerticalDragGestures(
                    onDragEnd = { if (pull > 220f) onClose() else pull = 0f },
                    onDragCancel = { pull = 0f },
                ) { _, dy -> pull += dy }
            },
    ) {
        PlayerBackdrop(song)
        CompositionLocalProvider(LocalContentColor provides Color.White) {
            Column(
                Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = 28.dp),
            ) {
                // The handle to pull the player down by.
                Box(
                    Modifier
                        .padding(top = 8.dp, bottom = 8.dp)
                        .align(Alignment.CenterHorizontally)
                        .size(width = 40.dp, height = 5.dp)
                        .clip(CircleShape)
                        .background(Color.White.copy(alpha = 0.45f))
                        .clickable(onClick = onClose),
                )
                if (panel != Panel.Cover) SmallNowPlaying(song)
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    when (panel) {
                        Panel.Cover -> {
                            val scale by animateFloatAsState(if (p.playing) 1f else 0.82f, tween(350), label = "cover size")
                            Cover(
                                song.artwork,
                                Modifier
                                    .fillMaxWidth()
                                    .aspectRatio(1f)
                                    .sharedCover("now-${song.id}")
                                    .graphicsLayer { scaleX = scale; scaleY = scale }
                                    .shadow(if (p.playing) 32.dp else 12.dp, RoundedCornerShape(12.dp)),
                                RoundedCornerShape(12.dp),
                            )
                        }
                        Panel.Lyrics -> LyricsPanel(song, p.position) { open(Route.Timing(song.id)) }
                        Panel.UpNext -> UpNextPanel(p.queueVersion)
                    }
                }
                if (panel == Panel.Cover) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                song.title,
                                style = MaterialTheme.typography.titleLarge,
                                fontWeight = FontWeight.Bold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                song.artist,
                                style = MaterialTheme.typography.titleMedium,
                                color = Color.White.copy(alpha = 0.7f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        IconButton(onClick = {
                            haptics(false)
                            favorite = runCatching { core.toggleFavorite(song.id) }.getOrDefault(favorite)
                            NeedleApp.instance.libraryVersion.value++
                        }) {
                            Icon(
                                if (favorite) Icons.Rounded.Favorite else Icons.Rounded.FavoriteBorder,
                                contentDescription = if (favorite) "Take out of favorites" else "Add to favorites",
                                tint = if (favorite) MaterialTheme.colorScheme.primary else Color.White.copy(alpha = 0.7f),
                            )
                        }
                        SongMenu(song, open)
                    }
                }
                SeekBar(position = p.position, duration = song.duration)
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 16.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(onClick = { core.previous() }, modifier = Modifier.size(72.dp)) {
                        Icon(Icons.Rounded.SkipPrevious, contentDescription = "Previous", modifier = Modifier.size(48.dp))
                    }
                    IconButton(onClick = { haptics(false); core.toggle() }, modifier = Modifier.size(88.dp)) {
                        PlayPauseIcon(p.playing, 64.dp)
                    }
                    IconButton(onClick = { core.next() }, modifier = Modifier.size(72.dp)) {
                        Icon(Icons.Rounded.SkipNext, contentDescription = "Next", modifier = Modifier.size(48.dp))
                    }
                }
                Row(
                    Modifier.fillMaxWidth().padding(bottom = 12.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RoundToggle(panel == Panel.Lyrics, { panel = if (panel == Panel.Lyrics) Panel.Cover else Panel.Lyrics }) {
                        Icon(Icons.Rounded.Lyrics, contentDescription = "Lyrics")
                    }
                    Row {
                        RoundToggle(looping.isNotEmpty() || loopStart != null, {
                            when {
                                looping.isNotEmpty() -> { core.setLoop(null, null); loopStart = null; showMessage("Loop off") }
                                loopStart == null -> { loopStart = p.position; showMessage("Loop from ${time(p.position)}: tap again where it ends") }
                                else -> {
                                    core.setLoop(loopStart, p.position)
                                    showMessage("Looping ${time(loopStart!!)} to ${time(p.position)}")
                                    loopStart = null
                                }
                            }
                        }) {
                            Text(if (looping.isNotEmpty()) "A-B" else if (loopStart != null) "A…" else "A-B", style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.Bold)
                        }
                        IconButton(onClick = { Ui.sheet.value = Sheet.PlayOn }) {
                            Icon(Icons.Rounded.Cast, contentDescription = "Play on", tint = Color.White.copy(alpha = 0.7f))
                        }
                        IconButton(onClick = { core.shuffle() }) {
                            Icon(Icons.Rounded.Shuffle, contentDescription = "Shuffle what is up next", tint = Color.White.copy(alpha = 0.7f))
                        }
                        RoundToggle(p.repeat != RepeatMode.OFF, {
                            core.setRepeat(
                                when (p.repeat) {
                                    RepeatMode.OFF -> RepeatMode.ALL
                                    RepeatMode.ALL -> RepeatMode.ONE
                                    RepeatMode.ONE -> RepeatMode.OFF
                                },
                            )
                        }) {
                            Icon(
                                if (p.repeat == RepeatMode.ONE) Icons.Rounded.RepeatOne else Icons.Rounded.Repeat,
                                contentDescription = "Repeat",
                            )
                        }
                    }
                    RoundToggle(panel == Panel.UpNext, { panel = if (panel == Panel.UpNext) Panel.Cover else Panel.UpNext }) {
                        Icon(Icons.AutoMirrored.Rounded.QueueMusic, contentDescription = "Up next")
                    }
                }
            }
        }
    }
}

/** An icon button that shows a soft circle when on, as Apple Music's lyrics and queue buttons. */
@Composable
private fun RoundToggle(on: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit) {
    val fill by animateColorAsState(if (on) Color.White.copy(alpha = 0.2f) else Color.Transparent, label = "toggle")
    IconButton(onClick = onClick, modifier = Modifier.clip(CircleShape).background(fill)) {
        CompositionLocalProvider(LocalContentColor provides if (on) Color.White else Color.White.copy(alpha = 0.7f)) {
            icon()
        }
    }
}

/** The song, small, above the lyrics or the queue. */
@Composable
private fun SmallNowPlaying(song: Song) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(song.artwork, Modifier.size(56.dp), SmallCoverShape)
        Column(Modifier.weight(1f)) {
            Text(song.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.7f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/**
 * A thin bar that thickens while held, as in Apple Music. The time shown follows the finger;
 * the song moves only when it lets go.
 */
@Composable
private fun SeekBar(position: Double, duration: Double) {
    var held by remember { mutableStateOf<Float?>(null) }
    val length = duration.coerceAtLeast(1.0).toFloat()
    val shown = held ?: (position.toFloat() / length).coerceIn(0f, 1f)
    val thickness by animateDpAsState(if (held != null) 10.dp else 5.dp, label = "seek thickness")
    Column(Modifier.padding(top = 16.dp)) {
        BoxWithConstraints(
            Modifier
                .fillMaxWidth()
                .height(28.dp)
                .pointerInput(length) {
                    detectTapGestures { core.seek((it.x / size.width).coerceIn(0f, 1f) * length.toDouble()) }
                }
                .pointerInput(length) {
                    detectHorizontalDragGestures(
                        onDragStart = { held = (it.x / size.width).coerceIn(0f, 1f) },
                        onDragEnd = { held?.let { core.seek(it * length.toDouble()) }; held = null },
                        onDragCancel = { held = null },
                    ) { change, _ -> held = (change.position.x / size.width).coerceIn(0f, 1f) }
                },
            contentAlignment = Alignment.CenterStart,
        ) {
            Box(Modifier.fillMaxWidth().height(thickness).clip(CircleShape).background(Color.White.copy(alpha = 0.25f)))
            Box(Modifier.fillMaxWidth(shown).height(thickness).clip(CircleShape).background(Color.White.copy(alpha = if (held != null) 1f else 0.85f)))
        }
        Row(Modifier.fillMaxWidth()) {
            Text(time((shown * length).toDouble()), style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f))
            Spacer(Modifier.weight(1f))
            Text("-" + time(length.toDouble() - shown * length), style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f))
        }
    }
}

@Composable
private fun LyricsPanel(song: Song, position: Double, time: () -> Unit) {
    val lyrics by rememberLoaded(song.id) { lyrics(song.id) }
    val lines = lyrics?.lines.orEmpty()
    val state = rememberLazyListState()
    val now = lines.indexOfLast { it.time <= position + 0.2 }
    val app by NeedleApp.instance.app.collectAsState()
    LaunchedEffect(now, app.liveLyrics) {
        if (app.liveLyrics && now >= 0) state.animateScrollToItem((now - 1).coerceAtLeast(0))
    }
    // Keeps the screen on while the lyrics show, when that is on.
    val view = LocalView.current
    DisposableEffect(app.keepScreenOnLyrics) {
        view.keepScreenOn = app.keepScreenOnLyrics
        onDispose { view.keepScreenOn = false }
    }
    when {
        lyrics == null || (lines.isEmpty() && lyrics?.plain.isNullOrBlank()) -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                if (lyrics?.instrumental == true) "Instrumental" else "No lyrics for this song",
                style = MaterialTheme.typography.titleMedium,
                color = Color.White.copy(alpha = 0.6f),
            )
            if (lyrics?.instrumental != true) {
                androidx.compose.material3.TextButton(onClick = time) { Text("Add and time them", color = Color.White) }
            }
        }
        lines.isEmpty() -> LazyColumn(Modifier.fillMaxSize()) {
            item { Text(lyrics!!.plain, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold) }
            item { androidx.compose.material3.TextButton(onClick = time) { Text("Time these lyrics", color = Color.White) } }
        }
        else -> LazyColumn(Modifier.fillMaxSize(), state = state) {
            itemsIndexed(lines) { i, line ->
                val lit = !app.liveLyrics || i == now
                val alpha by animateFloatAsState(if (lit) 1f else if (i < now) 0.25f else 0.4f, tween(300), label = "line")
                val grow by animateFloatAsState(if (i == now && app.liveLyrics) 1f else 0.94f, spring(dampingRatio = 0.7f, stiffness = 300f), label = "line size")
                // Karaoke: the words sung so far are lit, the rest wait.
                val sung = if (i == now && line.words.isNotEmpty() && app.liveLyrics) {
                    androidx.compose.ui.text.buildAnnotatedString {
                        line.words.forEach { word ->
                            withStyle(androidx.compose.ui.text.SpanStyle(color = if (word.time <= position) Color.White else Color.White.copy(alpha = 0.4f))) {
                                append(word.text)
                            }
                        }
                    }
                } else null
                Text(
                    sung ?: androidx.compose.ui.text.AnnotatedString(line.text.ifBlank { "♪" }),
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White.copy(alpha = alpha),
                    modifier = Modifier
                        .fillMaxWidth()
                        .graphicsLayer { scaleX = grow; scaleY = grow; transformOrigin = TransformOrigin(0f, 0.5f) }
                        .clip(RoundedCornerShape(8.dp))
                        .clickable { core.seek(line.time) }
                        .padding(vertical = 12.dp),
                )
            }
        }
    }
}

@Composable
private fun UpNextPanel(version: ULong) {
    val songs by rememberLoaded(version) { upNext() }
    val list = songs.orEmpty()
    Column(Modifier.fillMaxHeight()) {
        Text("Up next", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(vertical = 8.dp))
        if (songs != null && list.isEmpty()) {
            Text("Nothing up next", color = Color.White.copy(alpha = 0.6f))
            return@Column
        }
        LazyColumn(Modifier.fillMaxSize()) {
            itemsIndexed(list, key = { i, s -> "$i-${s.id}" }) { i, song ->
                // Swiped away, a song leaves the queue.
                val dismiss = rememberSwipeToDismissBoxState()
                LaunchedEffect(dismiss.currentValue) {
                    if (dismiss.currentValue != SwipeToDismissBoxValue.Settled) core.removeUpNext(i.toUInt())
                }
                SwipeToDismissBox(
                    state = dismiss,
                    backgroundContent = {
                        // Only while a row is being swiped; the rows themselves are see-through.
                        if (dismiss.dismissDirection != SwipeToDismissBoxValue.Settled) Box(
                            Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)).background(Color.White.copy(alpha = 0.12f)).padding(horizontal = 16.dp),
                            contentAlignment = Alignment.CenterEnd,
                        ) { Text("Remove", color = Color.White.copy(alpha = 0.8f)) }
                    },
                ) {
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).clickable { core.jump(i.toUInt()) }.padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Cover(song.artwork, Modifier.size(44.dp), SmallCoverShape)
                        Column(Modifier.weight(1f)) {
                            Text(song.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(song.artist, style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.6f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }
        }
    }
}

/** Play and pause, turning into each other. */
@Composable
private fun PlayPauseIcon(playing: Boolean, size: Dp) {
    AnimatedContent(
        playing,
        transitionSpec = { (scaleIn(tween(180), 0.6f) + fadeIn(tween(180))) togetherWith (scaleOut(tween(140), 0.6f) + fadeOut(tween(140))) },
        label = "play pause",
    ) { on ->
        Icon(
            if (on) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
            contentDescription = if (on) "Pause" else "Play",
            modifier = Modifier.size(size),
        )
    }
}
