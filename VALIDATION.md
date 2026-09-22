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

## Native UI exercise

The redesigned interface (0.2.0) was driven on Windows with scripted clicks and keys and checked from screenshots: first-run screen and demo import; song table, selection, right-click menu, album grid and album page, artist grid; settings in dark and light themes; rule suggestions with Tab completion; the history page with ranges, charts, and top lists; Play on a 500,000-track library queuing its first 50,000 matches without the interface stalling; and album and artist pages over the same library. A three-track selection had its genre changed through the multi-track editor; the database then showed the new genre on all three, and restoring the earlier version of one file brought its original genre back. `cargo test -p needle` checks WCAG AA contrast for every text colour on every surface in both themes.

The earlier interface was checked as follows:

The Windows app was opened and inspected directly. The first-run screen created and imported its original demos; library rows and details rendered; Ctrl+F focused search; `bpm > 90` returned the expected two tracks; a smart playlist was saved and reopened with its live rule and management controls. Shared playback advanced through both tracks. The persisted history contained two qualified 24-second listens and each corresponding track had a play count of one. SQLite integrity checking returned `ok`.

## Not verified

macOS/Linux builds and playback; exhaustive codec/container metadata; unusual MP4 timelines; physical device hot-plug (recovery is tested only with a simulated device); screen-reader operation (GPUI 0.2.2 exposes no accessibility tree on Windows); Last.fm and ListenBrainz sign-in screens against the live services; external DAC sample capture; real Last.fm/ListenBrainz submissions; an authenticated AcoustID response; production signing, installer, auto-update, and commercial service setup.

## Packaged release checks

The optimized `dist/Needle/Needle.exe` opened successfully as a desktop application, showing the persisted library, smart playlist, imported M3U playlist, paused current track, and updated play count. The separate `needle-cli.exe` passed checks for help, typed search, M3U export/import, SQLite backup integrity, layout export/import, encrypted bundle export/import without duplicate play counts, tag preview without writing, and nonzero exit status for an invalid query. The portable ZIP includes both executables, documentation, layout presets, dependency license texts, original MPL dependency source archives, and executable SHA-256 checksums.
