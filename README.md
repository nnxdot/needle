# Needle

A native Rust/GPUI music player with a local SQLite library. **0.2.0 is a working Windows preview.** [PROPOSAL.md](PROPOSAL.md) is the original product vision; [IMPLEMENTATION.md](IMPLEMENTATION.md) records the implemented scope and remaining work.

## Run

Open **`dist/Needle/Needle.exe`**. Choose **Add a music folder**, or try the three original demo recordings on the first-run screen. Files stay in their existing folders. No account is needed for playback, search, playlists, ratings, or history.

The portable folder also contains `needle-cli.exe`. The Windows build needs a working GPU driver and audio output. The package does not need Rust, Node, Python, or FFmpeg installed.

## Listen and organize

- **Find music.** The sidebar holds Songs, Albums, Artists, Favorites, Recently added, Listening history, and your playlists. The search field in the title bar takes plain words or a rule; while you type a rule it suggests fields, comparisons, and values from your library (Tab completes, ↑/↓ choose, Esc closes).
- **Play.** Double-click a track, press Enter, or use **Play**/**Shuffle** in a page header. Play and Shuffle queue everything the page matches, across its 1,000-track pages, up to 50,000 tracks. Right-click a track (or use its ⋯ button) for Play next, Add to queue, favorites, Add to playlist, Go to album/artist, Edit tags, Show in File Explorer, and Copy file path.
- **Select.** Click selects, Shift-click or Shift+↑/↓ extends, Ctrl-click adds or removes one track, and Ctrl+A selects the page. Actions in the menu and side panel apply to the whole selection.
- **Side panel.** *Details* shows the focused track: artwork, 0–5 stars, format and file facts, loudness measurement, MusicBrainz/AcoustID lookups, and why it is playing. *Queue* shows what is next; double-click an entry to jump to it, or reorder and remove entries. Ctrl+J opens the queue. Closing Needle saves the queue and position; reopening restores it paused.
- **Player bar.** Title and artist link to the album and artist pages. The chip next to the volume shows the signal path (for example `FLAC 24/96 → 48 kHz (resampled)` or `→ Exclusive 96 kHz`); hover it for the full path. The loop button sets A, then B, then clears an A–B loop.
- **Playlists.** The plus in a page header or beside *Playlists* saves the current search as a smart playlist that updates itself, or saves the shown (or selected) tracks as a regular playlist. Playlists can be renamed, exported as M3U8, and deleted without deleting music.
- **Tags.** Ctrl+E or *Edit tags* opens the editor in the side panel. With several tracks selected it shows shared values, marks differing ones as mixed, and writes only the fields you change, with per-file progress and results. Every write keeps an original-file backup and verifies that decoded audio is unchanged; *Earlier versions of this file* restores a backup, and a restore can itself be undone.
- **History.** Every listen is recorded locally, even offline. The history page lists every listen (loading more as you scroll) and shows listening per day and per hour of day, plus top artists, albums, and tracks for 7 days, 30 days, 12 months, or all time.
- **Keyboard.** Space play/pause · Ctrl+←/→ previous/next · ←/→ seek 10 s · Ctrl+↑/↓ volume · Ctrl+K or Ctrl+F search · Ctrl+1–6 sidebar pages · Ctrl+, settings · Alt+← or Backspace back · Tab/Shift+Tab move between controls · Esc closes menus, clears search, then the selection.
- **Appearance.** Dark and light themes and compact/comfortable rows are in Settings. Text colours are checked by a test to meet WCAG AA contrast (4.5:1) on every surface in both themes.

Supported and exercised with generated fixtures: WAV PCM, AIFF PCM, FLAC, MP3, AAC and ALAC in M4A, Ogg Vorbis, and Ogg Opus (`.opus`, or Opus inside `.ogg`). WavPack, APE, DSD, DRM, and streaming services are not implemented. Raw AAC is not part of the validated format set.

Opus uses a pure-Rust decoder (a patched `opus-decoder` crate), so no system libraries are needed. Pre-skip, end trimming, the header's output gain, seeking, and mono/stereo/5.1 channel mappings are handled and tested. Its CELT output matched FFmpeg's libopus decode at over 100 dB SNR on the fixtures; hybrid (SILK+CELT) packets, typical of low-bitrate speech-like encodes, matched at about 44–54 dB, which is close but not identical. Opus R128 gain tags are not read, and only the first stream of a chained Ogg file plays.

## Rules

```text
rating >= 4 and not played(7d) shuffle limit 20
recent(30d) and bpm > 120
artist contains "Björk" order by year desc
format = "FLAC" and sample_rate >= 96000
played(2025)
missing
```

Use `and`, `or`, `not`, parentheses, `=`, `!=`, `<`, `<=`, `>`, `>=`, `contains`, `exists(field)`, `recent(30d)`, and `played(7d)`/`played(2025)`. Optional suffixes are `order by field asc|desc` or `shuffle`, followed by `limit N`. Quoted text supports backslash escapes. Fields include title, artist, album, album_artist, genre, year, format, path, bpm, rating, duration, sample_rate, bit_depth, play_count, added_at, and last_played. Missing BPM values do not satisfy numeric comparisons.

This is a bounded query language, not the proposal’s complete expression pipeline. Artist-constrained shuffle and similarity remain unimplemented. The search field's suggestions come from `query::suggest`, which offers context-aware completions (fields, type-appropriate operators, functions, connectives, `order by` fields and directions, and quoted library values); it stays silent for ordinary words that are not rule vocabulary. `not missing` on its own is read as plain text; write `not missing and …` or use parentheses. The core also exposes simple `{artist} — {title}` display templates.

## Audio output

Shared output follows the system/device sample rate; the signal-path chip in the player bar shows the negotiated path. Volume and optional ReplayGain apply there. **Measure loudness** computes EBU R128 integrated loudness and true peak, storing normalization data in the library. The normalization target is −18 LUFS with peak protection.

Album gain measures every track of an album (same album artist, or artist when that is blank, and album title) as one programme, and stores the album gain and highest true peak on each track. When **Keep album dynamics** is on in Settings (shown once ReplayGain is on), playback uses it and falls back to track gain for tracks without an album measurement. The same peak protection applies. Album and track ReplayGain tags already in files are read when scanning. `needle-cli loudness EXPR --album` measures the albums of matching tracks.

If the output device disappears or its stream stops responding, Needle reopens the selected device, or the system default when that device is gone, and continues from the same position. If nothing can be opened, playback pauses with an error and the queue is kept; play retries. When no device is selected, Needle follows changes of the system default output. A selected device that is missing at startup falls back to the default with a notice. Settings lists output devices and can refresh the list. These paths are tested with a simulated device; physically unplugging headphones or a USB DAC has not been exercised.

Windows exclusive output opens WASAPI at the file’s native rate and channel count, in packed 24-bit or 24-valid-bit integer PCM. It bypasses volume and ReplayGain; adjust volume on the audio device. Unsupported rates produce an error instead of resampling. This machine’s Focusrite endpoint accepted 48 kHz packed 24-bit PCM and rejected 44.1/96 kHz in its current driver configuration. Device behavior will vary.

Integer 16/24-bit sample conversion has exact round-trip tests. End-to-end DAC bit-perfect certification, 32-bit integer preservation, ASIO, macOS hog mode, Linux exclusive output, and exhaustive gapless conformance are not claimed. AAC trimming supports a single normal-rate MP4 edit; unusual movie timelines or other priming metadata need further work.

## Optional online services

Nothing is submitted until you request a lookup or enable a listening service. MusicBrainz sends artist/title text; AcoustID sends a fingerprint and duration; cover requests send a release ID. No audio file is uploaded.

MusicBrainz results are cached for seven days and limited to one request per second. Review a match, then explicitly save its tags. Cover art is cached locally and does not rewrite embedded artwork. Set `NEEDLE_HTTP_USER_AGENT` to identify your application and contact address before distributing a service-connected build.

AcoustID requires an API key for a registered application. Save it with `needle-cli login acoustid` (or set `NEEDLE_ACOUSTID_API_KEY`); the key is checked on the first lookup, not when saved. Do not put credentials in source control. Its free service is for noncommercial use and permits at most three requests per second. Needle spaces requests by at least 350 ms. [AcoustID API](https://acoustid.org/webservice), [MusicBrainz API requirements](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting), [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

For scrobbling, sign in under **Settings › Listening services** (or with the CLI), then turn the service on there. Settings also shows queued and failed listens, with a retry button and a "sign in again" message when a session is rejected.

| Service | Sign in | What is stored | Environment override |
|---|---|---|---|
| ListenBrainz | `needle-cli login listenbrainz`, paste your user token; Needle validates it with `/1/validate-token` | Token and user name | `NEEDLE_LISTENBRAINZ_TOKEN` |
| Last.fm | `needle-cli login lastfm`; approve Needle in the browser page it opens | Session key and user name; the application API key and shared secret if you entered them | `NEEDLE_LASTFM_API_KEY`, `NEEDLE_LASTFM_SECRET`, `NEEDLE_LASTFM_SESSION` |
| AcoustID | `needle-cli login acoustid` | API key | `NEEDLE_ACOUSTID_API_KEY` |

Secrets are stored in Windows Credential Manager under targets ending in `.nnx.Needle` (for example `lastfm-session.nnx.Needle`), never in the library database, sync bundles, or logs; error messages and stored queue errors are redacted. A nonempty environment variable takes precedence over the stored value, so signing out does not remove a credential supplied by the environment. `needle-cli services` shows what is configured and where it came from, without printing secrets; `needle-cli logout <service>` removes stored credentials (`lastfm` removes the session; `lastfm-app` removes the application key and secret). On other platforms there is no secure store yet; use environment variables.

Last.fm requires an application API key and shared secret from [a registered API account](https://www.last.fm/api/account/create). A distributor can compile defaults in by setting `NEEDLE_LASTFM_API_KEY` and `NEEDLE_LASTFM_SECRET` at build time; note that anything compiled in can be extracted from the executable, and that a developer's own variables will be baked in if present during the build. Otherwise `login lastfm` asks for them (`--new-app` replaces them). The sign-in token expires after 60 minutes.

Qualified plays are queued locally and retried in chronological order every minute, with backoff after failures. Local qualification requires half the track or four minutes; Last.fm also requires a track longer than 30 seconds. Responses that can never succeed (missing artist/title, invalid parameters, a scrobble Last.fm ignores as too old) mark that listen as failed instead of retrying it forever. A rejected session or token pauses that service's queue, with a "sign in again" message, until the credential changes; nothing is dropped. Network errors, rate limits, and service outages are retried. `needle-cli scrobble` submits due listens and prints queue counts, the latest error, and the next retry time. [Last.fm scrobbling rules](https://www.last.fm/api/scrobbling), [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html).

## Data, backups, and layouts

The default location is `%LOCALAPPDATA%\nnx\Needle\data` on Windows; Settings and `needle-cli doctor` show the resolved path. Use `--data-dir PATH` to isolate another library. The folder holds `library.db`, artwork, tag backups, and optional demos. Each tag write or restore first copies the file it replaces into `backups`; backups are not pruned automatically. A backup can be restored only when its decoded audio matches the current file. Back up through Settings or the CLI so SQLite’s WAL is included correctly.

Encrypted bundles transfer history, ratings, and playlists between libraries containing the same music files. Use a passphrase of at least 12 characters. XChaCha20-Poly1305 authenticates the contents; Argon2 derives the key. Matching uses file hashes, existing nonzero local ratings win, listen IDs deduplicate, and newer playlist timestamps win. Unmatched files are reported. Imported history is never re-scrobbled. Audio, credentials, and playback position are excluded. This is **manual transfer**, not automatic peer-to-peer/CRDT sync.

Layout import/export uses one JSON file for sidebar width, inspector width, and row height. See [layouts/compact.json](layouts/compact.json). Layout version and numeric ranges are validated; arbitrary UI components/columns are not supported.

## Command line

```powershell
.\needle-cli.exe --help
.\needle-cli.exe scan 'D:\Music'
.\needle-cli.exe search 'rating >= 4 order by artist' --json
.\needle-cli.exe play 'D:\Music\track.flac'
.\needle-cli.exe playlist 'Favorites' --query 'rating >= 4'
.\needle-cli.exe export 'Favorites' '.\favorites.m3u8'
.\needle-cli.exe backup '.\library-backup.db'
.\needle-cli.exe loudness 'album = "Example"'
.\needle-cli.exe loudness 'artist = "Example"' --album
.\needle-cli.exe duplicates 'artist = "Example"'
.\needle-cli.exe tag 'album = "Example"' '.\changes.json'
```

`changes.json` contains only fields to change, e.g. `{"album":"Correct album","year":2026}`. Preview first, then add `--apply` to write each matching file with backups; progress goes to standard error. Batch failures are reported per file; already-successful writes remain saved. Available tag fields are title, artist, album, album_artist, genre, year, track_number, and musicbrainz_id. An empty album_artist removes that tag (Needle then shows the track artist); a track_number of 0 removes the number.

Other commands include `devices`, `exclusive`, `history`, `demo`, `import-playlist`, `lookup`, `identify`, `fingerprint`, `cover`, `scrobble`, `login`, `logout`, `services`, `layout-import`, `layout-export`, `sync-import`, and `sync-export`. Sync CLI commands read `NEEDLE_SYNC_PASSPHRASE` from the environment. Duplicate analysis is capped at 5,000 candidate tracks and reports suggestions without deleting files.

## Build and verify

Install Rust and the Visual Studio C++ build tools/Windows SDK. GPUI and its component library are pinned; dependencies are in Cargo.lock.

```powershell
cargo run -p needle --bin needle-desktop
cargo test -p needle-core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
powershell -File scripts/build-windows.ps1
```

[VALIDATION.md](VALIDATION.md) records the checks run here, including the 500,000-track benchmark. Audio smoke examples use a quiet generated tone and the actual output device. FFmpeg is used only to generate disposable verification fixtures, never by the shipped app.
