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
   glide waited up to 30 frames for lyrics that were not on screen. It now waits only for
   lyrics that are shown (`crates/needle/src/ui/lyrics.rs`).

   Result of 1 to 3, benchmark with a song playing: pages at rest went from 40 to 50 frames
   a second to 13 to 17 (the rest is the moving seek bar and the search box's blinking
   cursor); processor time for the whole benchmark went from 17.0 s to about 12 to 18 s
   (it varies with start-up work, see 5).
4. **Animations** (kept). The lyrics view (line fades and the glide), the big player's
   opening and colour fade, the page change, and the colour fade after a new cover all ask
   for every frame while they run, as they should; each runs under a second, and a frame
   costs 2 to 4 ms. On a fast screen they take many frames (up to 180 a second here).
5. **Each start uses one processor core for about 30 seconds** (open). Needle rescans every
   music folder at start, even when nothing changed.
6. **The audio worker waits on the network** (open). With a music server that does not
   answer, opening a song blocks the worker for about 21 seconds (the Windows connect
   timeout). Play, Pause, and Next all wait behind it. 1.5.2 only stopped closing from
   waiting.
7. **Slow frames** (open). Settings had one 19 ms frame, start-up one of 45 ms. To look at.
8. **Work repeated on every redraw** (open). For example, the page header adds up the length
   of every listed song on each render.
9. **Blur and glass** (open). Not measured yet (the GPU's time is not in the frame log).

## Also asked for during this work

- **Mouse side buttons** (done): Back and Forward go between pages (Back leaves full screen or
  the big player first, as Esc does). Alt+Right goes forward, as Alt+Left goes back. The
  benchmark sends both buttons and checks the pages.
- **A collapsed sidebar like Apple Music's** (open): a narrow strip of page icons.

## Patches to list in THIRD-PARTY-NOTICES.md before release

- gpui-component 0.5.1 copied to `third-party/patched` (text input notify, item 1).
- gpui: `NEEDLE_FRAME_LOG` in `src/window.rs`, reasons in `src/app/context.rs` and
  `src/elements/animation.rs`, `DispatchEventResult` public.
