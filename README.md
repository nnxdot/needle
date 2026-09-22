# Needle

A native Rust/GPUI music player with a local SQLite library. **0.1.0 is a working Windows preview.** [PROPOSAL.md](PROPOSAL.md) is the original product vision; [IMPLEMENTATION.md](IMPLEMENTATION.md) records the implemented scope and remaining work.

## Run

Open **`dist/Needle/Needle.exe`**. Choose **Add a music folder**, or try the three original demo recordings on the first-run screen. Files stay in their existing folders. No account is needed for playback, search, playlists, ratings, or history.

The portable folder also contains `needle-cli.exe`. The Windows build needs a working GPU driver and audio output. The package does not need Rust, Node, Python, or FFmpeg installed.

## Listen and organize

- Double-click a track, or select it and press Enter. Space plays/pauses, Ctrl+F focuses search, Ctrl+O adds a folder, and Ctrl+Left/Right changes tracks. Escape leaves a text field or closes an editing panel.
- Use the plus beside a track to queue it. The queue supports removal, reordering, repeat, and A–B looping. Closing Needle saves the queue and position; reopening restores it paused.
- Search ordinary artist/title/album/genre text, or write a rule. **Save playlist** creates a live rule or a snapshot of shown tracks. Regular and smart playlists can be renamed, exported as M3U8, and deleted without deleting music.
- The heart marks a favorite; Track details provides a 0–5 rating. History records local listening even when network services are disabled.
- Large collections use pages of 1,000 tracks. Play, shuffle, and “Save shown tracks” act on the current page. Search and smart rules operate over the library, up to the current 500,000-result limit.
- Tag edits show editable values before writing. Writes keep an original-file backup and verify that decoded audio is unchanged before replacing a file. Batch editing is available through the CLI.

Supported and exercised with generated fixtures: WAV PCM, AIFF PCM, FLAC, MP3, AAC and ALAC in M4A, and Ogg Vorbis. Opus, WavPack, APE, DSD, DRM, and streaming services are not implemented. Raw AAC is not part of the validated format set.

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

This is a bounded query language, not the proposal’s complete expression pipeline. Artist-constrained shuffle, similarity, grouped history analytics, and expression autocomplete remain unimplemented. The core also exposes simple `{artist} — {title}` display templates.

## Audio output

Shared output follows the system/device sample rate; the footer displays the negotiated path. Volume and optional track ReplayGain apply there. **Measure loudness** computes EBU R128 integrated loudness and true peak, storing normalization data in the library. The normalization target is −18 LUFS with peak protection.

Windows exclusive output opens WASAPI at the file’s native rate and channel count, in packed 24-bit or 24-valid-bit integer PCM. It bypasses volume and ReplayGain; adjust volume on the audio device. Unsupported rates produce an error instead of resampling. This machine’s Focusrite endpoint accepted 48 kHz packed 24-bit PCM and rejected 44.1/96 kHz in its current driver configuration. Device behavior will vary.

Integer 16/24-bit sample conversion has exact round-trip tests. End-to-end DAC bit-perfect certification, 32-bit integer preservation, ASIO, macOS hog mode, Linux exclusive output, and exhaustive gapless conformance are not claimed. AAC trimming supports a single normal-rate MP4 edit; unusual movie timelines or other priming metadata need further work.

## Optional online services

Nothing is submitted until you request a lookup or enable a listening service. MusicBrainz sends artist/title text; AcoustID sends a fingerprint and duration; cover requests send a release ID. No audio file is uploaded.

MusicBrainz results are cached for seven days and limited to one request per second. Review a match, then explicitly save its tags. Cover art is cached locally and does not rewrite embedded artwork. Set `NEEDLE_HTTP_USER_AGENT` to identify your application and contact address before distributing a service-connected build.

AcoustID requires an API key for a registered application. Save it with `needle-cli login acoustid` (or set `NEEDLE_ACOUSTID_API_KEY`); the key is checked on the first lookup, not when saved. Do not put credentials in source control. Its free service is for noncommercial use and permits at most three requests per second. Needle spaces requests by at least 350 ms. [AcoustID API](https://acoustid.org/webservice), [MusicBrainz API requirements](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting), [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

For scrobbling, sign in, then enable the service in Settings:

| Service | Sign in | What is stored | Environment override |
|---|---|---|---|
| ListenBrainz | `needle-cli login listenbrainz`, paste your user token; Needle validates it with `/1/validate-token` | Token and user name | `NEEDLE_LISTENBRAINZ_TOKEN` |
| Last.fm | `needle-cli login lastfm`; approve Needle in the browser page it opens | Session key and user name; the application API key and shared secret if you entered them | `NEEDLE_LASTFM_API_KEY`, `NEEDLE_LASTFM_SECRET`, `NEEDLE_LASTFM_SESSION` |
| AcoustID | `needle-cli login acoustid` | API key | `NEEDLE_ACOUSTID_API_KEY` |

Secrets are stored in Windows Credential Manager under targets ending in `.nnx.Needle` (for example `lastfm-session.nnx.Needle`), never in the library database, sync bundles, or logs; error messages and stored queue errors are redacted. A nonempty environment variable takes precedence over the stored value, so signing out does not remove a credential supplied by the environment. `needle-cli services` shows what is configured and where it came from, without printing secrets; `needle-cli logout <service>` removes stored credentials (`lastfm` removes the session; `lastfm-app` removes the application key and secret). On other platforms there is no secure store yet; use environment variables.

Last.fm requires an application API key and shared secret from [a registered API account](https://www.last.fm/api/account/create). A distributor can compile defaults in by setting `NEEDLE_LASTFM_API_KEY` and `NEEDLE_LASTFM_SECRET` at build time; note that anything compiled in can be extracted from the executable, and that a developer's own variables will be baked in if present during the build. Otherwise `login lastfm` asks for them (`--new-app` replaces them). The sign-in token expires after 60 minutes.

Qualified plays are queued locally and retried in chronological order every minute, with backoff after failures. Local qualification requires half the track or four minutes; Last.fm also requires a track longer than 30 seconds. Responses that can never succeed (missing artist/title, invalid parameters, a scrobble Last.fm ignores as too old) mark that listen as failed instead of retrying it forever. A rejected session or token pauses that service's queue, with a "sign in again" message, until the credential changes; nothing is dropped. Network errors, rate limits, and service outages are retried. `needle-cli scrobble` submits due listens and prints queue counts, the latest error, and the next retry time. [Last.fm scrobbling rules](https://www.last.fm/api/scrobbling), [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html).

## Data, backups, and layouts

The default location is `%LOCALAPPDATA%\nnx\Needle\data` on Windows; Settings and `needle-cli doctor` show the resolved path. Use `--data-dir PATH` to isolate another library. The folder holds `library.db`, artwork, tag backups, and optional demos. Back up through Settings or the CLI so SQLite’s WAL is included correctly.

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
.\needle-cli.exe duplicates 'artist = "Example"'
.\needle-cli.exe tag 'album = "Example"' '.\changes.json'
```

`changes.json` contains only fields to change, e.g. `{"album":"Correct album","year":2026}`. Preview first, then add `--apply` to write each matching file with backups. Batch failures are reported per file; already-successful writes remain saved. Available tag fields are title, artist, album, genre, year, and musicbrainz_id.

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
