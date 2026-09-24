# Validation record

Environment: Windows x64, Rust 1.98.1, MSVC build tools. Verification used isolated databases and generated audio, not the user’s personal music library.

## Automated core checks

`cargo test -p needle-core` passes 37 tests (3 more are ignored because they need a real audio device or FFmpeg) for playback queue operations under each repeat mode, play-next/jump/play-from-index, A–B loop bounds, ReplayGain and album-gain peak protection, listen qualification, session save and restore, 50,000-item queues published without per-tick copies, output recovery (device error, stalled stream, seek on a dead stream, missing configured device, changed system default, no device at all) against a simulated device thread; album loudness over mixed sample rates and album grouping; Ogg Opus length, pre-skip alignment, seeking, 5.1 channel order, import, and tag writes;  typed/bounded queries and escaping; ordinary titles and leading zeroes; import/rescan/moved-file identity; watched-folder import notifications; database persistence and history; tag backups with unchanged decoded samples and recording IDs; Unicode search; known-signal R128 loudness and fingerprinting; authenticated encrypted transfer with ID remapping and deduplication; rejection of invalid imports without partial changes; exact 16/24-bit integer conversion; and AIFF sound-data boundaries.

Library additions add further tests; with both sets merged, `cargo test -p needle-core` passes 68 tests (3 ignored). They cover history paging, statistics (totals, rankings, zero-filled local days, hours, ranges), and index use after migration; album/artist/genre summaries, album keys and rules, and escaped field-value prefixes; autocomplete contexts, cursor replacement, plain-text detection matching the parser, and quoted library values that round-trip through search; batch tag writes with a missing track, duplicates, cancellation, and invalid edits; album artist/track number writes and removal; shared-value detection; backup restore, restore-undo, and rejection of foreign or audio-mismatched backups; and playlists, M3U round trips, ratings, and missing-file recovery.

`cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are required by the packaging script. A third-party `proc-macro-error2` future-compatibility notice remains in Cargo output; it is not an application warning or current build failure.

## Format exercise

`cargo run -p needle-core --example format_check -- artifacts/format-fixtures-verified`

Three-second stereo 48 kHz fixtures were generated with the locally installed FFmpeg. Each format was imported, decoded, retagged, decoded again, and compared sample for sample. WAV, FLAC, MP3, AAC/M4A, ALAC/M4A, Ogg Vorbis, and AIFF passed with 144,000 decoded frames each. Artist and MusicBrainz recording IDs survived a fresh metadata read in all seven. ID3 recording IDs use MusicBrainz’s UFID owner mapping. This is a focused fixture set, not an exhaustive conformance claim.

The exercise found and fixed two decoder-boundary issues:

1. AIFF metadata after the SSND chunk could be interpreted as audio. The file reader now ends at the sound-data boundary, with a regression test.
2. AAC/M4A priming and padding were not trimmed by the underlying container decoder. A bounded parser honors the single normal-rate edit used in the tested M4A file. Complex edit lists and alternative priming metadata are not covered.

Every tag write now compares decoded audio digests before replacing the original file, in addition to keeping a backup. A difference rejects the write.

## Opus

The fixtures in `crates/needle-core/testdata` were encoded by FFmpeg's libopus: a 1 s stereo tone at 96 kb/s, a 1.5 s mono tone from 44.1 kHz at 48 kb/s, and a 0.5 s 5.1 tone at 192 kb/s. Each decodes to the exact length FFmpeg produces. `cargo test -p needle-core --lib opus -- --ignored` compares against FFmpeg's libopus decode: CELT channels agreed at 101–106 dB SNR; the mono file's first 400 ms and the 5.1 file's centre and LFE streams are hybrid (SILK+CELT) packets and agreed at 43.5 dB and 53.7 dB. The same run decoded 60 s of stereo pink noise at 160 kb/s at roughly 150× real time. The published crate's direct DFT managed about 3×, which is why Needle uses a patched copy.

## Live audio

`cargo run -p needle-core --example audio_smoke` and the same command with `-- --exclusive` passed on the default Focusrite output:

- Stream started at 48 kHz stereo.
- Pause, seek while paused, resume, and graceful shutdown completed.
- Repeat-one preserved the future queue.
- Restart restored the track, position, and future queue paused.

The WASAPI probe found packed 24-bit and 16-bit support at 48 kHz, while 44.1 kHz and 96 kHz requests were rejected by this endpoint configuration. Needle chose packed 24-bit PCM and rejected unsupported native rates without resampling. This is a device/driver observation, not a universal Focusrite specification. No external DAC capture or all-platform bit-perfect certification was performed.

## 500,000-track library benchmark

`cargo run -p needle-core --example benchmark -- artifacts/benchmark-library`

The synthetic database contained 500,000 unique indexed track records and occupied approximately 555 MB. The debug-build exercise measured the first 100 results with the corresponding bounded count, seven warm iterations per query. Assertions checked actual matches, including leading-zero search terms.

| Query | Median | Maximum |
|---|---:|---:|
| `Artist 00420` | 22.7 ms | 26.0 ms |
| `Track 0123456` | 21.2 ms | 26.9 ms |
| `year >= 2020 and rating >= 4 order by rating desc limit 100` | 5.4 ms | 5.8 ms |
| `bpm > 170 limit 100` | 4.4 ms | 5.0 ms |
| Empty query | 16.9 ms | 17.9 ms |

These are local, warm-cache synthetic timings, not a promise that every query, disk, cold start, or real-world metadata distribution will remain under 100 ms.

### Browsing, history, and completion

`cargo run --release -p needle-core --example browse_benchmark -- DIR`

A release build over 500,000 synthetic tracks (41,667 albums, 10,000 artists) and 1,000,000 listens spread over three years and 50,000 tracks. Five warm iterations each; other builds were running on the machine, so timings varied by roughly 2× between runs.

| Call | Median | Maximum |
|---|---:|---:|
| `history_stats(None)` | 370 ms | 414 ms |
| `history_stats` (last 30 days, 51,375 listens) | 115 ms | 119 ms |
| `history_page(10000, 200)` | 7.9 ms | 9.0 ms |
| `albums("")` | 466 ms | 491 ms |
| `albums("year >= 2020")` | 226 ms | 250 ms |
| `artists("")` | 374 ms | 465 ms |
| `artist_albums` | 6.5 ms | 7.1 ms |
| `genres()` | 52 ms | 58 ms |
| `field_values("album", "", 10)` | 65 ms | 78 ms |
| `suggest_with_library` (`genre = "Ge`) | 67 ms | 78 ms |

Opening an existing large library for the first time after this change builds the new indexes once.

## 0.3.0 checks

- `cargo test -p needle-core` passes 97 tests (4 ignored: a real audio device, FFmpeg, and the 166 MB stem model). New tests cover the equalizer (transparency when flat, +6 dB at a band's own frequency, no clipping, live changes reaching a playing source, mono/balance/crossfeed), the rule language additions against a real library, iTunes XML and Spotify import (idempotent re-import, no scrobbling of imported listens), LRC parsing and LRCLIB response shapes, folder covers, plugins (loading disabled, hooks, commands, confinement to the plugin folder, the operation limit, permission refusals), stem windows, resampling, and the stem mixer, and Previous after the queue ends.
- The ignored stem test was run with the real model: a 10-second mix split in 6.7 s (release build, CPU), putting a 55 Hz tone in the bass stem and clicks in drums with a near-silent vocal stem.
- Live requests were made once to LRCLIB (59 timed lines for a known song), MusicBrainz/Wikidata/Commons (an artist photo), and ListenBrainz (listen format). Last.fm import was not run without an API key.
- In the running app: the title bar drags, the window resizes from its edges including the top, and maximize/restore and double-click work (checked with Windows hit-testing and scripted drags after a fix for a focusable element that swallowed title-bar clicks). The big player showed timed lyrics following playback; the mini player opened, expanded (staying on screen), showed lyrics and the queue, and returned to the main window. The Sound page applied presets that persisted across restarts. The Import page detected an iTunes XML, and importing set ratings, play counts, and a playlist. An example plugin added a track-menu command that rated a song. A demo song was split into stems in the app and played back from its stems with the Karaoke mix.

## 0.9.1 checks

- The window glass rows in Settings › Appearance span the full width again (they sat in a wrapper that sized itself to its content). With Ambient background on, the glass choices are greyed out with a note, since Ambient paints over the whole window. Night is a lighter charcoal grey so it reads clearly apart from Midnight's true black in the previews; the contrast test still passes for every look. Checked from screenshots.

## 0.9.0 checks

- Soft edges (text and rounded corners) on the see-through glass layer showed fringes, because GPUI's Windows renderer added coverage to the window's alpha instead of blending it. Needle now builds with a patched gpui 0.2.2 that blends alpha "over"; zoomed screenshots of the sidebar text, the search field, and the signal chip on Acrylic show clean edges.
- Ambient is now a setting that works with Night, Midnight, and Day (an old "ambient" look setting becomes Night with Ambient on). Midnight is neutral true black, so the three look previews differ clearly; each has a short description. The contrast test covers every look with and without Ambient.
- The stems mixer was redesigned: one compact colored row per stem with icon, level, slider, live meter, Solo and Mute; mixes are chips that light up when they match. Checked in the big player.

## 0.8.1 checks

- The command palette again sizes to its list (it had stretched to the window's height), and Listening history shows Ctrl+7. Switching Settings sections no longer replays the page fade, so it no longer looks like a reload. Checked from screenshots.

## 0.8.0 checks

- `cargo test --workspace` passes (101 core tests, 6 ignored, and the UI tests). The contrast test now includes the Ambient look. New tests cover the Discord card layouts (title and lines, paused state, no logo, no picture) and reading covers saved with an ".img" ending (which kept colors from the music from working on covers found in files).
- Checked from screenshots: the Discord settings section and its preview; compact rows on one line; the title font choice; custom colors with colors from the music off; a nearly black cover no longer laying a dark band over the light look; the bold, glowing sung lyric line in the Day look and the mini player's own lyrics scrolling; stem mixes in two rows inside the side panel; the playlist right-click menu and Play from it; crisper sidebar text on glass; solid previous/next icons; the Ambient look with a cover and with a chosen color, and the big player filled by the cover. Clicking the page you are on no longer reloads it. The mini player's seek and volume sliders are its own, so they no longer drift when both windows are open.
- A test module that imported everything from a file using GPUI picked up GPUI's own `test` attribute and made the compiler recurse until it ran out of stack; it now imports what it needs by name.

## 0.7.3 checks

- At narrow window widths the title bar pushed the maximize and close buttons off the window (the title bar component does not shrink its contents). The search field is now sized from the window width, and the palette button shows only its icon below 1,100 px. Checked at the minimum size (916 px), about 1,100 px, and 1,136 px: all three window buttons stay visible; Home, Songs, Artists, Listening history, Settings, and the big player also fit.

## 0.7.0 checks

- Fixed and checked in the running app: the mini player can no longer be resized by dragging (it sizes itself); its Playing next / History tab labels no longer jump to the left on hover (a hover style reset text alignment, so the labels are now centered by layout); long messages wrap inside their bubble, and in the big player the bubble sits at the top instead of over the controls; *Turn on online lookups* in the lyrics panel now turns lookups on in place and searches LRCLIB at once (before, it switched to Settings behind the big player, so nothing seemed to happen); and with nothing playing, the times read 0:00 and the seek bar rests at the start.
- The mini player's History tab and the Listening history page show covers, group back-to-back plays of a song (checked with four plays in a row, shown as ×4), and use short dates.
- Film grain is off by default and has a strength slider.
- Discord Rich Presence uses Needle's own Discord application. A live test with Discord open showed a "Listening" presence named Needle with the uploaded logo and a time bar for eight seconds, confirmed by Discord's reply, and then cleared it. A second live test showed "Listening to Armageddon" with the artist, the album, and the album cover found in the iTunes catalog (Discord proxied the image), with the Needle logo as the small badge. A test checks the cover match: a different artist never counts, and the same song on the same album wins over remixes. It talks only to the local Discord pipe and first checks that the program serving the pipe is a Discord client (Discord, Canary, PTB, Development, Vesktop, Equibop, Legcord, or WebCord); a live check against this PC's Discord passed. Tests cover the activity message (listening type, 2–128 character fields, time bar in milliseconds, paused state without a time bar) and the client name check.

## 1.4.2 checks

- Player bar: the quality chip is on its own line above the controls and the volume slider is 150 px (wide windows) or 110 px; a screenshot of a separate copy (its own data folder, playing a generated test tone) shows the chip above a longer slider. The scroll wheel over the volume changes it in 5% steps while the window is active; this was not tried with a real mouse. `cargo test --workspace` passes.

## 1.4.1 checks

- Discord: status changes settle for 0.8 s, the cover is looked up before the update, a refused change is sent again after 5 s, and nothing is sent while a server song is connecting. A diagnostic build logging every status and reply showed exactly one status per song, each accepted by Discord; the brief second card is Discord's own switch between activities with different names. `cargo test --workspace` passes; there is no automated test against Discord.

## 1.4.0 checks: music sources and Navidrome / Subsonic

- `needle-core` tests: source paths and ids round-trip (ids with `/`, `#`, `%`); song maps are read leniently (numbers as text, a title required); syncing keeps ratings, updates changed songs, and marks songs the server lost as missing without touching another source's songs; signing out marks them missing; the stream cache drops the oldest files first and keeps downloads under way; duplicates, album tag issues, file moves, covers to embed, and measuring for radio all skip server songs; a plugin without the network permission cannot be a source.
- End to end against a pretend Subsonic server on 127.0.0.1 (the bundled plugin, unchanged): a wrong password shows the server's message; the right one signs in and syncs two songs with their tags; both songs' cover is fetched once; no request contains the password; a WAV streams to 44,100 stereo frames with sound in them, lands in the cache, and plays again with no second download; a qualified listen sends `scrobble` with the time in milliseconds, a rating sends `setRating`; signing out marks the songs missing.
- Streaming tests against a slow local file server (4 MB at about 2 MB/s): a read at 3.6 MB returns the right bytes well before the whole file could arrive, through a range request, and the finished cache file is identical to the original; with range requests turned off the same test takes 1.9 s and fails, so it measures the jump. A server that ignores ranges still gives the right bytes and a whole file. Kept songs live outside the cache, survive pruning, and are not downloaded again; prefetching fills the cache.
- Not tried: a real Navidrome, Airsonic, or Gonic server (the user's own Navidrome synced 6,728 songs and played them in a test build), very large libraries, and the Settings form, sidebar entry, and song menu, which were not looked at (the app was not run).

## 1.4.0 checks: custom themes

- New tests in the `needle` crate: theme files are read and written back unchanged; mistakes are explained (an unknown color key lists the right ones, a bad color, base, or font says what is wrong); names become file names without overwriting; user and plugin themes load, plugin themes only while the plugin is on and never saved over, and edits to a file are noticed.
- Readability: for every base look, 10 page colors (black, greys, white, and saturated red, yellow, green, blue, pink) against 10 text and accent colors and a clashing sidebar, every text color meets 4.5:1 on the sidebar, page, and cards, and accent labels meet 4.5:1 on the accent. Chosen colors that already read are kept exactly.
- `needle-core`: a plugin with only a `themes` folder and no script loads and turns on without an error.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all --check` pass. The app was not run and no screenshots were taken, so the Appearance page, the editor, the color pickers, dropping a file on the window, and the Import and Export dialogs were not looked at or tried by hand.

## 1.3.11 checks

- Scrobbles and "now playing" use Apple's spelling of the artist when the tags write it another way, only when Apple has the same song on the same album. A new test covers it: names that already agree stay as tagged, and without the same album a different artist is never taken. The live iTunes search for "키키 Hey Hi" returns KiiiKiii on "WhyKiiiKiii - EP". It was not tried against a real Last.fm account before release.

## 1.3.1 checks

- The Discord cover search now accepts Apple's result when the artist is written another way (for example 키키 in the tags and KiiiKiii at Apple), if the song and the album both match. A new test covers it; the song name alone is still refused.
- A live iTunes search for "키키 Hey Hi" returned "Hey Hi" on "WhyKiiiKiii - EP" by KiiiKiii, which the new rule accepts.
- Last.fm (track.updateNowPlaying) and ListenBrainz (playing_now) are told when a song starts or plays again after a pause, so they show it live. A test checks that nothing is sent when the service is off or the song has no artist. It was not tried against a real Last.fm account before release.

## 1.3.0 checks

- `cargo test --workspace` passes (163 core tests, 10 ignored, plus 8 UI tests); `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean. The app was not run and no screenshots were taken, so the layout fixes below were not looked at.
- **Duplicates:** a test puts "Aozora Jumping Heart" with "(Off Vocal)", "(Instrumental)", "(Japanese ver.)", "[Live]", "（Off Vocal）" (full-width brackets), and "- Instrumental" on one album: only the two Off Vocal copies are grouped. "(2011 Remaster)", "[Explicit]", "(Remastered 2009)", and "(Album Version)" still count as the same recording; "Remix" and "Taylor's Version" do not.
- **Dolby Atmos (E-AC-3) files:** Symphonia cannot decode them, and Windows' own Dolby decoder on this PC offers only S/PDIF and HDMI pass-through outputs (IEC 61937), no PCM. So Needle ships `needle-ffmpeg.exe`: FFmpeg 7.1.1, LGPL, 1.5 MB, built by `scripts/build-ffmpeg.sh` in MSYS2 UCRT64 with only the MP4 reader, the AC-3/E-AC-3 decoders, resampling, and float output. Tests with a 1-second 5.1 E-AC-3 file made by FFmpeg: it plays as 48 kHz stereo for about a second at a clearly heard level, seeks to 0.5 s with about half left, the player's normal decode path uses it, and a broken file fails at once with a clear message. The reported song ("SWEET SOUR", Dolby Digital Plus with Atmos, 5.1, 768 kb/s) decodes in full (177.4 s) and plays through the player's decode path. Where the decoder is missing, Needle tries Windows (which works only where Windows has a PCM Dolby decoder) and otherwise explains that the song is Dolby. The installer build now fails if `needle-ffmpeg.exe` is missing.
- **Hide to tray:** a switch in Settings › Playback adds an icon in the notification area (the app icon, from resource 1); closing the window then hides it and the music plays on; the icon shows Needle again, its menu plays, pauses, skips, or quits, "Hide to tray" is in the command palette, and opening Needle or a file again brings the window back. Built, not tried: the app was not run.
- **Phone remote:** jsdom checks (no window, no screenshots) cover tapping rows to play, + to play next, repeat cycling off → all → one with its "1" badge, shuffle, and the viewport that turns zoom off, plus all earlier checks. A worker test shuffles 30 songs: the playing song stays and Up next holds the same songs in another order. The server accepts repeat off/all/one and shuffle, and refuses other repeat values.
- **Discord:** the status hides while paused when *Show while paused* is off, and clears after the chosen time of nothing playing; built, not tried with Discord.
- **Layout:** song titles and artists shrink with an ellipsis instead of running over the columns; covers of any shape are cut to their square (and artist photos to their circle); the welcome guide's privacy text wraps so its switches stay in the card; the sidebar can be folded away (button or Ctrl+B), and the side panel shows whenever 360 px remain for the page. Built, not looked at.

## 1.2.0 checks

- `cargo test --workspace` passes (157 core tests, 8 ignored, plus 8 UI tests); `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean. The app was not run and no screenshots were taken, so none of the new screens were looked at or used by hand. The release build and installer were not built for this record.
- **Log file and crash reports:** a test crashes a thread on purpose and finds a report naming the thread, the message, and the source file, and the crash line in the log. Scrubbing takes out file paths (drive letters and network shares) and keeps source file names; reports are cut at 24 KB; with reports off they wait and are deleted after 30 days. The site's `/api/crash` was run locally with a stand-in bucket: it stores version, system, report, and time (no IP address), and refuses GET, bad JSON, missing fields, and bodies over 32 KB. It is not deployed yet, and the `needle-crashes` bucket and its 12-month lifecycle rule do not exist yet.
- **Better equalizer:** a parametric band gives +6 dB at its frequency; a switched-off band and the graphic bands in parametric mode do nothing; the suggested preamp follows the loudest boost; Equalizer APO/AutoEq files are read (PK, LSC, HSC, HPQ, OFF lines, a Q left out) and written back to the same bands; a saved preset gives the same sound back. The graphic equalizer tests still pass after it moved to the shared filter code.
- **Several speakers at once:** a test group lines up a 2 s and a 0.5 s member (1.5 s of silence for the quicker one), follows a +100 ms and a −50 ms timing change while playing, keeps playing when one member fails, and fails when all do; groups round-trip through the settings. End to end, the player played one song on two pretend DLNA speakers and both received more than a second of audio. "This computer" as a member uses the default sound device and was not tested, and no real speakers were tried.
- **Phone remote:** a test starts the server, loads the page, refuses a wrong key and unknown paths, reads the state and a search, changes the volume through the player, queues a song, and refuses a bad volume and an unknown action. The page's script passed `node --check`, and in a headless jsdom browser (no window, no screenshots) it showed the title, artist, output, progress, times, pause state, cover URL, and queue from a sample state, switched to search, showed results, and sent next, jump, and play to the right addresses. It was not opened on a real phone, and the Windows firewall prompt was not seen.
- **Winget:** the build script's manifests now carry the publisher, privacy and help links, a license, and a description. Nothing was sent to winget yet.
- **Privacy policy and help pages:** `privacy.html` and `help.html` list every online service found in the code (LRCLIB, iTunes Search, MusicBrainz, Cover Art Archive, Wikidata, Wikimedia Commons, Last.fm, ListenBrainz, AcoustID, Hugging Face, needle.nnx.fyi) and where each setting is. HTML tags balance on all four pages; they were not looked at in a browser.
- **Welcome guide:** builds and opens on a first start with no music; people with music are marked as welcomed without seeing it. It was not tried by hand.

## 1.1.0 checks

- `cargo test --workspace` passes (149 core tests, 8 ignored, plus 8 UI tests); `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean. The release build and installer were not built for this record, and the app was not run.
- **Faster adding of music:** a song's identity is now a hash of its size and its first and last megabyte (`q1:` hashes), not of the whole file. Files are read on 2–8 threads, and songs are written to the database in batches of up to 500 in one transaction. Tests: 60 files imported at once with progress reports, a second scan finds all 60 unchanged, and when a missing song reappears as two copies only one copy keeps its rating and plays. The time for a large real library was not measured.
- **Songs from older versions:** existing songs are read again once (they are skipped only at metadata version 2), which replaces their whole-file hashes. A moved song that still has a whole-file hash is found by its size and then its whole-file hash (tested).
- **Duplicates:** "Fix my library" and the duplicate finder confirm quick-hash matches with a whole-file hash before calling files identical. A test with two equal files and a third that differs only in its middle groups only the equal two.
- **Updates:** Needle asks `https://needle.nnx.fyi/latest.json` (same shape as a GitHub release) instead of GitHub, because the repository is private. The build script writes `dist\latest.json`. Needle 1.0.0 still asks GitHub, so people on 1.0.0 must install 1.0.1 by hand once.
- **Effect and DSP plugins:** plugins can add effects to Sound › Effects, built from 11 native blocks or written as WebAssembly (run by wasmi with no imports, a 32 MB memory cap, and a fuel budget per 512-frame block). Tests: a low-pass follows its slider (5 kHz removed at 500 Hz, kept at 20 kHz); all blocks together stay finite and the limiter holds the peak; an echo rings on and clears after a seek; a WebAssembly gain takes slider values; an endless loop, imports, a missing `init`, and a buffer outside memory are refused or stopped while the music plays on unchanged; the example Bitcrusher (WebAssembly text) crushes a ramp to a few steps and processes 10 s of stereo sound far faster than real time in debug and release builds; effects play inside the real playback source, follow slider changes, and vanish when their plugin is turned off; the WebAssembly example on the plugins page works as printed; a script with the `audio` permission turns an effect on. wasmi uses its portable dispatch, because its optimized tail-call dispatch overflowed the stack on Windows. The Sound page's Effects card was built and compiles, but was not looked at or used, since the app was not run.
- **Plugins page:** `website/public/plugins.html` documents plugin.toml, permissions, events, commands, every host function, song and listen fields, limits, the four examples, and common problems, checked against `plugins.rs`. Its HTML tags balance; it was not looked at in a browser.

## 1.0.0 checks

- `cargo test --workspace` passes (138 core tests, 8 ignored, plus 8 UI tests); `cargo clippy --workspace --all-targets -- -D warnings` is clean. The release build and installer were not built for this record.
- **Windows media controls:** title, artist, album, and cover showed in the Windows media flyout; play/pause and next worked from it (checked with a script reading the system media session).
- **Crossfade:** a test compares when the next song starts with and without crossfade.
- **Formats:** WavPack, Monkey's Audio, DSF (as PCM), and CUE sheets decode in tests; the CUE album and the three formats were imported and played in the app.
- **Folders and columns:** checked from screenshots: folder pages with subfolders, resizing columns from their left edge, reordering, sorting, and the column menu.
- **Karaoke lyrics and timing editor:** word timing reads and writes back unchanged (enhanced LRC); in the app, words of a real song were tapped in, saved as an .lrc beside the file (then removed), and lit up word by word in the big player.
- **Your year in music:** tested on a library with two synthetic years of listening (totals, top artists and songs, streaks, genres, new artists).
- **Fix my library:** duplicates were merged in a copy of the test music (one extra copy went to the Recycle Bin; plays, history, and playlists moved); files were moved by pattern with their covers and lyrics and then undone; a real MusicBrainz lookup matched "Abbey Road" and wrote tags to test copies, with backups.
- **Offline radio:** tempo and key are found on generated test signals; a real song measured as 112 BPM, A minor. Radio from a song, from an artist, and a blend of two artists were played in the app.
- **Speakers:** DLNA control, Chromecast messages, and AirPlay's RTSP and audio packets are tested against pretend devices; a test plays a real song through the player to a pretend DLNA speaker that downloads the live stream; Chromium played Needle's live WAV stream in real time (muted). No real Chromecast, DLNA, or AirPlay device was on the test network, so none was tried. AirPlay uses the older RAOP protocol; AirPlay 2 pairing is not supported.
- **Opening files and updates:** a second launch handed a song to the running Needle, which played it; only one Needle stayed open. The update check compares versions and verifies downloads against SHA256SUMS.txt in tests; no real update was installed. The Inno Setup installer script and winget manifests were written but not compiled (Inno Setup is not installed on the build machine).

## 0.6.0 checks

- `cargo test --workspace` passes, including a new test that glass only thins the back layer (and the page when asked) and never changes text colors.
- On Windows 11, checked from screenshots: Mica behind the sidebar, title bar, and player in the Night and Day looks (the sidebar took the wallpaper's tint); Acrylic showing a frosted blur of the windows behind; Clear showing the desktop through the back layer only, with the page solid; the mini player with glass; and the Appearance controls. Extending the window frame into the client area made Windows draw its own caption buttons behind Needle's when maximized, so Needle no longer does that; Mica still shows, and a maximized window has only Needle's buttons.
- The page no longer floats as a separate card with a border, shadow, and gaps; it now sits flush with the window edge and the player, with one hairline and a rounded corner where it meets the sidebar, and the details panel shares its surface. Checked in the Night and Day looks.
- Needle.exe and needle-cli.exe now carry the app icon (resource 1, which GPUI uses for the window). The running app showed it in the taskbar; the icon was checked at 16–256 px on light and dark backgrounds.
- Not checked: Windows 10 (where Mica falls back to Acrylic), the Transparency effects setting turned off, and smoothness while dragging with Acrylic on slower PCs.

## 0.5.0 checks

- `cargo test --workspace` passes (98 core tests, 4 ignored, plus the UI contrast test). The contrast test now builds the palette for all three looks and 27 cover colors (24 hues, a dark muted blue, a pale yellow, and grey) and checks every text color on every surface at 4.5:1. A new core test covers the Home shelves, including the order of recently played albums after recorded listens.
- In the running app, checked from screenshots with a generated library of eight albums with distinct covers: Home with its shelves and Resume card; the palette taking a teal cover's color while it played and a green album's color on that album's page; generated covers and a red tint for music without artwork; the album grid with shadows; the big player with the blurred cover behind it; the mini player's glow; the Appearance section with live previews; and the Night, Midnight, and Day looks. Film grain was checked at 4× zoom.
- On the 500,000-track library, Home showed loading shelves at once and its covers within about two seconds (the first start also builds a new play-count index).
- Not checked: very large or unusual cover images, and text over very bright covers in the Day look beyond the tested palette colors (the glow behind page titles fades into the page surface).

## 0.4.0 checks

- `cargo test -p needle-core` still passes 97 tests; `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- In the running app, checked from screenshots: the track menu with shortcut hints; ↓ and → moving into *Add to playlist ›* from the keyboard; settings sections; the palette finding a song by a word in its title, and "plugins" + Enter opening Settings › Plugins; a song dragged onto a playlist in the sidebar (the playlist went from 2 to 3 songs in the database); the hover play button on an album cover; *Albums › Needle Studio* breadcrumbs on an album page; *Clear search* and *Search everything* on a search with no results; the big player growing from the cover and shrinking back into it (checked at a slowed-down duration), with the page behind it intact afterwards; and timed lyrics keeping the sung line at the same height while the text moved up.
- Not checked: how the motion looks on a slow GPU, and the Windows animation setting turned off (it is read with `SystemParametersInfo(SPI_GETCLIENTAREAANIMATION)`).

## Native UI exercise

The redesigned interface (0.2.0) was driven on Windows with scripted clicks and keys and checked from screenshots: first-run screen and demo import; song table, selection, right-click menu, album grid and album page, artist grid; settings in dark and light themes; rule suggestions with Tab completion; the history page with ranges, charts, and top lists; Play on a 500,000-track library queuing its first 50,000 matches without the interface stalling; and album and artist pages over the same library. A three-track selection had its genre changed through the multi-track editor; the database then showed the new genre on all three, and restoring the earlier version of one file brought its original genre back. `cargo test -p needle` checks WCAG AA contrast for every text colour on every surface in both themes.

The earlier interface was checked as follows:

The Windows app was opened and inspected directly. The first-run screen created and imported its original demos; library rows and details rendered; Ctrl+F focused search; `bpm > 90` returned the expected two tracks; a smart playlist was saved and reopened with its live rule and management controls. Shared playback advanced through both tracks. The persisted history contained two qualified 24-second listens and each corresponding track had a play count of one. SQLite integrity checking returned `ok`.

## Not verified

macOS/Linux builds and playback; exhaustive codec/container metadata; unusual MP4 timelines; physical device hot-plug (recovery is tested only with a simulated device); listening to stem separation quality on real music; GPU-accelerated separation; Last.fm history import; screen-reader operation (GPUI 0.2.2 exposes no accessibility tree on Windows); Last.fm and ListenBrainz sign-in screens against the live services; external DAC sample capture; real Last.fm/ListenBrainz submissions; an authenticated AcoustID response; production signing; a compiled installer and a real auto-update; Chromecast, DLNA, and AirPlay hardware; and commercial service setup.

## Packaged release checks

The optimized `dist/Needle/Needle.exe` opened successfully as a desktop application, showing the persisted library, smart playlist, imported M3U playlist, paused current track, and updated play count. The separate `needle-cli.exe` passed checks for help, typed search, M3U export/import, SQLite backup integrity, layout export/import, encrypted bundle export/import without duplicate play counts, tag preview without writing, and nonzero exit status for an invalid query. The portable ZIP includes both executables, documentation, layout presets, dependency license texts, original MPL dependency source archives, and executable SHA-256 checksums.
