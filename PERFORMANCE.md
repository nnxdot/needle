# Performance work

Stutters, slow starts, and waits in Needle, found from 2026-09-25 on (library of 6,908 songs,
mostly on a Navidrome server, plus local `Downloads` and `Music` folders). Each item says how
it was found, what was done, and the numbers before and after.

Status: **open**, **fixed**, or **kept** (looked at, and left as it is, with the reason).

## How it is measured

- `NEEDLE_FRAME_LOG=<file>` (a GPUI patch in `third-party/patched/gpui-0.2.2/src/window.rs`)
  writes, per step or every 5 seconds: frames drawn per second, the time to build a frame
  (render, layout, paint) and to present it, the longest wait between two frames, and the
  top reasons a frame was asked for (the code line of each `cx.notify()` and
  `request_animation_frame()`, and the name of each running animation).
- `NEEDLE_BENCH=1` (`crates/needle/src/ui/bench.rs`) walks Needle through Home, Songs,
  Albums, Artists (each at rest and scrolled), History, Settings, search typing, the queue and
  lyrics panels, the big player, and the mouse's back and forward buttons, then closes it.
- `scripts/perf-run.ps1 -DataDir <copy> -Files <song> -Bench` runs both on a copy of a data
  folder (volume set to 0) with a local song playing, and prints the processor time too.
  Results are kept in `artifacts/perf-*.txt`.

## Found

1. **The search box redrew the whole window on every frame, forever** (fixed). The text input
   of gpui-component notified itself on every paint, which asked for the next frame. Any
   window with an input redrew at full speed (40 to 90 frames a second) while nothing moved.
   Fixed in a patched copy (`third-party/patched/gpui-component-0.5.1`,
   `src/input/element.rs` and `src/input/state.rs`): it notifies only when what it keeps
   from painting changed.
2. **Needle's timer redrew the whole window 25 times a second while playing** (fixed). `poll`
   in `crates/needle/src/ui/mod.rs` ended with `cx.notify()` every 40 ms. It now redraws
   only when something shown changed (the playing state, the whole seconds of the time, an
   event, the lyric line, a notice), and at least once a second. The seek bar moves only by
   a visible step (a thousandth of its length).
3. **Hidden lyrics kept asking for frames** (fixed). After every new lyric line, the lyrics
   glide waited up to 30 frames for lyrics that were not on screen, and eased hidden lyrics
   toward their last layout. It now moves only lyrics on screen, and lyrics that come into
   view start from the sung line (`glide_lyrics` in `crates/needle/src/ui/lyrics.rs`).

   Result of 1 to 3, benchmark with a song playing: pages at rest went from 40 to 50 frames
   a second to 13 to 17 (the rest is the moving seek bar and the search box's blinking
   cursor); processor time for the whole benchmark went from 17.0 s to about 12 to 18 s
   (it varies with start-up work, see 5).
4. **Animations** (kept). The lyrics view (line fades and the glide), the big player's
   opening and colour fade, the page change, and the colour fade after a new cover all ask
   for every frame while they run, as they should; each runs under a second, and a frame
   costs 2 to 4 ms. On a fast screen they take many frames (up to 180 a second here).
5. **A start used half a processor core for about 40 seconds** (fixed). Not the folder scan:
   the music server plugin synced the whole song list on every start more than 30 minutes
   after the last sync. Timed on the Navidrome server here (6,908 songs, 28 pages of 250):
   each page took 1.1 s for the server to answer, 0.3 s for the plugin's script to turn the
   songs into Needle's form (8 s of processor time in all), 2 ms to read the JSON, and the
   database 70 ms in all. Now a start syncs only when the last sync is over 6 hours old, and
   20 seconds after start (`RESYNC_AFTER`, `SYNC_AFTER_START` in
   `crates/needle-core/src/plugins.rs`); signing in and Sync in Settings still sync at once.
   A start without a sync due: 3.2 s of processor time in the first 40 s (was about 18 s).
6. **The audio worker waited on the network** (fixed). With a music server that does not
   answer, opening a song blocked the worker for about 21 seconds (the Windows connect
   timeout), and Play, Pause, Next, and the volume all waited behind it. A song from a server
   that is not kept here now opens on a helper thread; it stays first in the queue until it
   is in, a change to the queue drops it, and a jump asked for meanwhile is done once it is
   in (`fill`, `take_opened` in `crates/needle-core/src/audio.rs`). (A save of the session before every
   command, added for a worker stuck opening a song, was taken out again once songs opened
   on a helper thread: it made each command slower, and three audio tests failed now and
   then under load on Linux, 3 times in about 25 runs; without it, 25 of 25 passed.) Test: with a server that
   takes 1.5 s and fails, Play, the volume, and Pause answer within 0.5 s, and the next song
   plays after it.
7. **Large covers drawn at full size** (fixed). GPUI decodes an image and sends it to the
   graphics card at its full size however small it is drawn, and some covers here are 3000
   to 4134 pixels wide (a 68 MB texture each). Rows, tiles, and artist photos now draw a
   copy of at most 256 or 640 pixels (whichever covers twice the size on screen), made once
   on one background thread and kept in `artwork/thumbs`; covers under 400 KB and covers
   shown larger than 320 points draw the original (`crates/needle/src/ui/thumbs.rs`, with a
   test). Peak memory in the benchmark: about 750 MB before, about 560 MB after.
8. **Stalls on the UI thread** (looked at, none left but one). The benchmark notes each time
   the UI thread is over 30 ms late (a timer every 50 ms; a step that blocks 80 ms on
   purpose proves it is caught) and each `open`, `navigate`, `refresh`, `poll`, or `render`
   over 8 ms. None show up now except the first draw of Settings, 16 to 18 ms once (text
   laid out for the first time on a page full of it; later draws take 3 to 4 ms). Kept.
9. **Work repeated on every redraw** (kept). A frame takes 2 to 4 ms to build on every page
   (5 to 9 ms at most), so the sums and lists built during a render are not worth changing.
10. **Blur and glass** (kept). While a song plays, Needle uses 0.16% of the graphics card
    (Windows' GPU counters).
12. **Endless animations redrew at the screen's rate** (fixed). A GPUI animation redraws the
    whole window on every screen refresh (up to 180 times a second here) for as long as it
    is on screen. The bouncing bars beside the playing song and the full screen's drifting
    cover never stop, so they now move on Needle's own timer instead (`slow_repeat` in
    `crates/needle/src/ui/motion.rs`; 25 times a second while playing). The playing song's
    album went from 34 to 26 frames a second, Songs after the big player from 33 to 21.
11. **157 threads** (kept). An empty profile runs 126: GPUI and the other libraries start
    worker pools sized to the processor, and idle threads cost almost nothing.

13. **Listing sound outputs and reading the keyring stalled the window** (fixed). Both ran on
    the UI thread when Needle started and when Settings opened; on Linux (WSL) opening
    Settings took 53 ms. Both now run on another thread (`list_output_devices`,
    `refresh_services`), and the stall is gone.

## Linux (WSL, Ubuntu, WSLg Wayland)

The `.deb` built by `scripts/build-linux.sh` was installed in WSL and the whole benchmark
run with `scripts/linux-bench.sh` (a copy of a real profile, a song playing): every step,
including the player in the top bar, the folded and hidden sidebar, the mini player, the
playlist editor, full screen, and the mouse's back and forward buttons, ran without a crash
or an error (only the expected "no keyring" warning in WSL). WSL draws in software, so its
speed says nothing about real Linux. Clippy is clean and the tests pass in Ubuntu 24.04, and
the packages install and start on Ubuntu 24.04 and Fedora 44. Note: to install a rebuilt
package of the same version, use `dpkg -i`; `apt-get install --reinstall` put back its cached
older copy.

## Also asked for during this work

- **Mouse side buttons** (done): Back and Forward go between pages (Back leaves full screen or
  the big player first, as Esc does). Alt+Right goes forward, as Alt+Left goes back. The
  benchmark sends both buttons and checks the pages.
- **A collapsed sidebar like Apple Music's** (done): Ctrl+B or the sidebar button folds the
  sidebar to a 60-point strip of page icons (names as tips, the page shown marked by an
  accent bar at the edge, groups split by thin lines, playlists and the bottom buttons as
  icons); its first icon unfolds it again. The benchmark scrolls Songs with it folded.
- **Compact sidebar** (done, on by default): Settings › Appearance. On, folding the sidebar
  leaves the strip of icons; off, folding hides it altogether, as before.
- **octo-fiesta** (done, experimental, off by default): the Navidrome / Subsonic plugin
  (1.3.0) has a switch, "Search the server as you type (experimental)", in Settings › Plugins. On, typing a search on Songs asks the server (after a 0.45 s pause); songs it
  finds that are not in the library show above the list under "On your server", to play or
  queue. Through octo-fiesta these include songs from its streaming services, fetched when
  played. Tested with a pretend server: off, nothing is asked; on, only the song not in the
  library comes back. Not tried against a real octo-fiesta.
- **Player in the top bar** (done, off by default): Settings › Appearance. The title bar
  holds the play controls, the song playing with its seek bar, and the volume, lyrics, and
  queue; the search field moves to the bottom, and its suggestions open upward.
- **Hide the search field** (done, off by default): Settings › Appearance. Ctrl+F then opens
  the command palette, which searches too. With the player on top as well, the bottom is
  empty. The benchmark turns both on and checks that Ctrl+F opens the palette.
- **A graphics card choice for stems** (tried, not added). DirectML (ONNX Runtime's route to
  any DirectX 12 card) was wired in and timed with the stem model on this PC: one 7.8-second
  step took 1.1 s on the processor and 461 s on an RTX 4070 Ti SUPER (the session took 8 s to
  make). The model's operations do not suit DirectML, so the choice would only slow splitting
  down; the processor splits 10 s of music in 5.8 s. A CUDA route would need NVIDIA's large
  runtime libraries alongside Needle.

## Patches to list in THIRD-PARTY-NOTICES.md before release

- gpui-component 0.5.1 copied to `third-party/patched` (text input notify, item 1).
- gpui: `NEEDLE_FRAME_LOG` in `src/window.rs`, reasons in `src/app/context.rs` and
  `src/elements/animation.rs`, `DispatchEventResult` public.
