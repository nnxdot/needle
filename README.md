# Needle

A native Rust/GPUI music player with a local SQLite library. **0.9.1 is a working Windows preview.** [PROPOSAL.md](PROPOSAL.md) is the original product vision; [IMPLEMENTATION.md](IMPLEMENTATION.md) records the implemented scope and remaining work.

## Run

Open **`dist/Needle/Needle.exe`**. Choose **Add a music folder**, or try the three original demo recordings on the first-run screen. Files stay in their existing folders. No account is needed for playback, search, playlists, ratings, or history.

The portable folder also contains `needle-cli.exe`. The Windows build needs a working GPU driver and audio output. The package does not need Rust, Node, Python, or FFmpeg installed.

## Listen and organize

- **Home.** Needle opens on Home: a greeting, the song you were playing (with Resume), and shelves of covers — albums you played last, your most played albums, albums with your favorite songs, new additions, and the artists you play most. Each shelf has *See all*.
- **Find music.** The sidebar holds Home, Songs, Albums, Artists, Favorites, Recently added, Listening history, and your playlists. The search field in the title bar takes plain words or a rule; while you type a rule it suggests fields, comparisons, and values from your library (Tab completes, ↑/↓ choose, Esc closes).
- **Play.** Double-click a track, press Enter, or use **Play**/**Shuffle** in a page header. Play and Shuffle queue everything the page matches, across its 1,000-track pages, up to 50,000 tracks. Hover an album or artist cover for a play button that plays it without leaving the page.
- **Menus.** Right-click a playlist in the sidebar to play, shuffle, queue, rename, export, or delete it. Right-click a track (or use its ⋯ button) for Play, Play next, Add to queue, favorites, *Add to playlist ›*, *Plugins ›*, Go to album/artist, Edit tags, Split into stems, Show in File Explorer, and Copy file path. Every menu uses the same look and shows its shortcut. ↑/↓ move, → opens a submenu, ← goes back, Enter chooses, Esc closes.
- **Command palette.** Ctrl+K (or the *Ctrl K* button in the title bar) opens one box that goes to any page, playlist, or settings section, runs any action or plugin command, and finds songs, albums, and artists as you type. ↑/↓ choose and Enter runs.
- **Drag and drop.** Drag songs (one or a selection) onto a playlist in the sidebar to add them, onto *Favorites* to like them, or onto the queue button to queue them.
- **Moving around.** Album and artist pages show where you are (*Albums › Artist*) with clickable steps. Back returns to the same scroll position. Empty pages always offer a next step, such as *Clear search*, *Browse songs*, or *Add music folder*.
- **Select.** Click selects, Shift-click or Shift+↑/↓ extends, Ctrl-click adds or removes one track, and Ctrl+A selects the page. Actions in the menu and side panel apply to the whole selection.
- **Side panel.** *Details* shows the focused track: artwork, 0–5 stars, format and file facts, loudness measurement, MusicBrainz/AcoustID lookups, and why it is playing. *Queue* shows what is next; double-click an entry to jump to it, or reorder and remove entries. Ctrl+J opens the queue. When it is on and the library has music, the side panel shows on song pages (and the Queue on any page) whenever the page keeps at least 360 px beside it. The button beside the Needle name (or Ctrl+B) folds the sidebar away, for narrow windows. Closing Needle saves the queue and position; reopening restores it paused.
- **Player bar.** Title and artist link to the album and artist pages. The chip next to the volume shows the signal path (for example `FLAC 24/96 → 48 kHz (resampled)` or `→ Exclusive 96 kHz`); hover it for the full path. The loop button sets A, then B, then clears an A–B loop.
- **Playlists.** The plus in a page header or beside *Playlists* saves the current search as a smart playlist that updates itself, or saves the shown (or selected) tracks as a regular playlist. Playlists can be renamed, exported as M3U8, and deleted without deleting music.
- **Tags.** Ctrl+E or *Edit tags* opens the editor in the side panel. With several tracks selected it shows shared values, marks differing ones as mixed, and writes only the fields you change, with per-file progress and results. Every write keeps an original-file backup and verifies that decoded audio is unchanged; *Earlier versions of this file* restores a backup, and a restore can itself be undone.
- **History.** Every listen is recorded locally, even offline. The history page lists every listen with its cover, grouping back-to-back plays of the same song into one row (×4) and showing when it was ("Today · 18:02"), loading more as you scroll and shows listening per day and per hour of day, plus top artists, albums, and tracks for 7 days, 30 days, 12 months, or all time.
- **Big player.** Click the artwork in the player bar (or Ctrl+P) for a full-window player tinted by the album art, with synced lyrics, what's next, and the stem mixer beside it. Esc closes it.
- **Mini player.** The mini-player button (or Ctrl+M) swaps the main window for a small one with the essentials. It expands to show what's playing next, your recent history, or lyrics, can stay on top of other windows, and its art or expand button returns to the full window.
- **Lyrics.** Needle shows lyrics from an `.lrc` or `.txt` file beside the song, from the song's own tags, or (with online lookups on) from LRCLIB. Timed lyrics follow the song: the sung line is bold with a soft glow in the music's color, and the others fade back. Click a line to jump there.
- **Artwork and artist photos.** Covers come from the file, then from `cover`/`folder`/`front`/`albumart` images (JPEG, PNG, or WebP) or a lone image in the folder. With online lookups on, missing covers come from the Cover Art Archive and artist photos from Wikimedia Commons via MusicBrainz and Wikidata; Settings can look up every missing cover at once.
- **Sound.** The equalizer button in the player bar opens *Sound* (also in Settings › Sound): a 10-band graphic equalizer (31 Hz–16 kHz, ±12 dB) with a preamp and ten presets, or a parametric equalizer of up to 20 bands (peak, low and high shelf, low- and high-pass, notch; 20 Hz–20 kHz, ±24 dB, Q 0.1–20). *Load EQ file…* reads an AutoEq `ParametricEQ.txt` or an Equalizer APO configuration, *Save EQ file…* writes one, and *Save as preset* keeps your own. Also balance, mono, headphone crossfeed, and effects from plugins. Changes apply while you listen. These tools apply to shared output only; exclusive output stays bit-for-bit.
- **Dolby Atmos music.** Dolby Digital and Dolby Digital Plus tracks in MP4/M4A files, such as Atmos downloads from Apple Music, play as 48 kHz stereo through `needle-ffmpeg.exe`, a small LGPL FFmpeg built only for them (`scripts/build-ffmpeg.sh`). Atmos height sound is not decoded.
- **Several speakers at once.** *Play on several at once…* in the speaker menu plays on any mix of Chromecast, DLNA, and AirPlay speakers and this computer. Needle delays the quicker ones so all play in step, and Settings › Playback › *Speaker timing* moves each by up to a second, while it plays, to line them up by ear. A speaker that stops is dropped and the rest play on.
- **Phone remote.** Settings › Playback › *Phone remote* serves a web page for phones on the same network: what is playing with its cover, play/pause/next/previous, seeking, repeat, shuffling Up next, volume, Up next (tap a song to play it), and search (tap to play, + to play next). It behaves like an app: no zooming or text selection while you use it. The address (shown as a QR code) holds a random key; *New address* locks out old ones. It answers only private-network addresses (home and office networks, and also VPNs that use them), so anyone who has the address and can reach this computer that way can use it. It is off by default.
- **Hide to tray.** Settings › Playback › *Hide to tray* adds an icon by the clock. Closing the window then hides Needle there while the music plays on. Click the icon to bring it back; right-click it to play, pause, skip, or quit. *Hide to tray* is also in the command palette.
- **Welcome guide.** The first start shows a short guide: music folders (or three demo songs), a look, what may go online (lookups, update checks, crash reports), and tips. Settings › Your data shows it again.
- **Logs and crash reports.** Needle writes a log to `logs\needle.log` in its data folder (about 3 MB kept). A crash writes a report to `crashes\`; the next start sends it to needle.nnx.fyi with file paths and names taken out, unless *Send crash reports* in Settings › Your data is off (then reports are deleted after 30 days). *Copy error report* copies the recent log, likewise cleaned, for an email.
- **Stems.** In the big player's *Stems* tab or the details panel, *Split into stems* separates a song into drums, bass, vocals, and everything else on this computer (about half the song's length on a modern CPU; the 166 MB model downloads the first time). Turn on *Play from stems* to mix them live: each stem is one compact colored row with a level slider, a live meter, and Solo (S) and Mute (M), and the Full mix, Karaoke, Vocals only, No drums, and Bass only mixes light up when they match. Stems are cached until you delete them.
- **Import.** *Import* in the sidebar brings over ratings, play counts, date added, and playlists from an iTunes, Apple Music, or MusicBee library XML (found automatically when it exists); listening history and playlists from a Spotify data download (folder or ZIP); scrobbles from Last.fm and listens from ListenBrainz by user name (later imports fetch only new ones); and every M3U playlist in a folder. Songs match by path, then artist and title. Listens for songs you don't have are kept for your statistics; imported listens are never scrobbled.
- **Discord.** *Show while paused* and *Clear when nothing plays* (5 minutes to an hour, or never; 10 minutes by default) decide how long the status stays when the music stops. While Discord is open on this PC, your Discord profile shows "Listening to" the song, with the artist, the album, the cover, and a time bar ("Paused" without the bar when paused). Discord can only show pictures from the web, so Needle finds the cover in Apple's iTunes catalog by artist and song name (only a matching artist counts; otherwise Discord shows the Needle logo). *Show covers on Discord* turns that lookup off. There is nothing to set up. Settings › Discord turns it off, chooses what the card's title and three lines show (song, artist, album, or Needle), hides the Needle logo, or stops cover lookups, with a live preview. Needle only talks to the real Discord app on this PC (it checks which program serves Discord's local pipe) and sends nothing else.
- **Plugins.** See below.
- **Keyboard.** Space play/pause · Ctrl+←/→ previous/next · ←/→ seek 10 s · Ctrl+↑/↓ volume · Ctrl+K command palette · Ctrl+F search · Shift+Enter play next · Ctrl+Enter add to queue · Ctrl+1–7 sidebar pages · Ctrl+, settings · Ctrl+P big player · Ctrl+M mini player · Alt+← or Backspace back · Tab/Shift+Tab move between controls · Esc closes menus, clears search, then the selection.
- **Motion.** Pages fade in, the playing song shows moving bars, a heart pops when you like a song, the big player grows out of the cover and shrinks back into it, its background fades to each new cover's colour, lyrics glide to the sung line, and loading lists shimmer. *Reduce motion* in Settings › Appearance turns all of this off, and Needle also keeps still when Windows' *Animation effects* setting is off.
- **Settings.** Settings are grouped into sections with a list on the left: Playback, Sound, Library, Appearance, Online services, Discord, Stems, Plugins, Your data, and Keyboard. The palette can open any section directly.
- **Look.** Needle takes its colors from the music: the accent, the surfaces, and a soft glow behind each page come from the playing song's cover, fading over about a second when the song changes. Album and artist pages take that album's or artist's own colors, and the big player and mini player glow with a blurred copy of the cover. Music without a cover gets a made-up one — a two-color gradient with the album's first letter — that also colors the app. Settings › Appearance › Fonts chooses the title font: Segoe UI (default), Bahnschrift, or Fraunces (Nick's font). With *Colors from the music* off, a row of colors tints the interface instead. *Film grain* in Settings › Appearance adds a fine texture over the window; it is off by default. Settings › Appearance offers three looks with live previews: Night (charcoal grey), Midnight (true black), and Day (light). *Ambient background* works with any of them: the cover (or the chosen color) fills the whole background, blurred, and fills the whole screen in the big player, and *Colors from the music* turns the tinting off.
- **Window glass.** The sidebar, title bar, and player bar are glass. By default they use Windows 11's Mica, a soft color from your wallpaper. Settings › Appearance › *Window glass* switches to Acrylic (frosted glass), Clear (see-through, no blur), or Solid. *See-through* sets how much shows through, and *Glass behind the page* lets a little show through the page too. The mini player uses the same glass. When Windows' own *Transparency effects* setting is off, Needle stays solid; on Windows 10, Mica falls back to Acrylic.
- **Appearance.** Compact rows fit one line per song with a small cover; comfortable rows are taller with a larger cover. Text colors are checked by a test to meet WCAG AA contrast (4.5:1) on every surface in all three looks, for 27 different cover colors.

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

favorite and not played(30d) shuffle by artist limit 50
genre in ("Jazz", "Soul") and duration < 5:00
title matches "^(intro|interlude)" or duration < 1m
year between 1990 and 1999 order by artist, year desc
recent(90d) limit 1 per album
skipped(30d) order by play_count desc
```

- **Combine:** `and`, `or`, `not`, parentheses.
- **Compare numbers:** `=`, `!=`, `<`, `<=`, `>`, `>=`, `between A and B`, `in (…)`. Lengths may be written `3:30`, `4m`, `90s`, or `1h`; `added_at` and `last_played` accept dates such as `2024-05-01`.
- **Compare text:** `=`, `!=`, `contains`, `starts with`, `ends with`, `matches "regular expression"` (ignoring case), and `in ("a", "b")`. Put `not` after the field to negate: `artist not in (…)`, `genre not contains "live"`.
- **Functions and words:** `recent(30d)`, `played(7d)` or `played(2025)`, `skipped(30d)`, `exists(field)`, `favorite` (4 stars or more), `unplayed`, and `missing`.
- **Shape the results:** `order by field [asc|desc]` (several fields separated by commas) or `shuffle`; `shuffle by artist` (or album, or genre) keeps neighbours apart; `limit N`, or `limit N per artist|album|album_artist|genre|year|format`, optionally followed by an overall `limit N`.
- **Fields:** title, artist, album, album_artist, genre, year, format, path, bpm, rating, duration (also `length`), sample_rate, bit_depth, bitrate, channels, track_number, disc, replay_gain, musicbrainz_id, play_count (also `plays`), added_at (also `added`), and last_played. Missing values do not satisfy numeric comparisons.

Quoted text supports backslash escapes. Case-insensitive matching folds only A–Z (an SQLite limit), so `"BJÖRK"` does not match `Björk`. This is a bounded query language, not the proposal’s complete expression pipeline; similarity search remains unimplemented. The search field's suggestions come from `query::suggest`, which offers context-aware completions (fields, type-appropriate operators, functions, connectives, `order by` fields and directions, and quoted library values); it stays silent for ordinary words that are not rule vocabulary. `not missing` on its own is read as plain text; write `not missing and …` or use parentheses. The core also exposes simple `{artist} — {title}` display templates.

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

## Themes

A custom theme is a small TOML file in the library's `themes` folder. It starts from one of the looks (Night, Midnight, or Day) and changes any of 14 colors; the colors it leaves out follow the base look and the colors it sets.

```toml
name = "Sakura"
base = "light"          # dark (Night), midnight, or light (Day)
music_colors = true     # may colors from the music tint what the theme leaves out
[colors]
page = "#fff5f7"
accent = "#c2185b"
[extras]
grain = 0.15            # film grain, 0 to 1
font = "fraunces"       # title font: system, bahnschrift, or fraunces
```

Colors are `sidebar`, `page`, `card`, `card_hover`, `border`, `border_soft`, `text`, `text_muted`, `text_faint`, `accent`, `accent_text`, `danger`, `success`, and `glow`. Text colors are moved just enough to meet WCAG AA contrast (4.5:1) on the sidebar, page, and cards; when nothing can read on both, the sidebar moves toward the page. Themes show next to the built-in looks in Settings › Appearance. *Make a copy* starts a theme from the look in use and opens an editor with a color picker per color that saves as you go. *Export…* saves the theme to share, and *Add a theme…* or dropping a `.toml` file on the window adds one without overwriting. Needle notices edits to theme files within two seconds. Plugins can bring themes in a `themes` folder (a plugin with only themes needs no script); those show while the plugin is on and are copied to be changed. The full guide is at https://needle.nnx.fyi/themes.

## Plugins

Plugins are small [Rhai](https://rhai.rs) scripts. Each lives in its own folder inside the library's `plugins` folder (Settings › Plugins › *Open plugins folder*) with a `plugin.toml`:

```toml
id = "this-week"
name = "This week's favourites"
version = "1.0.0"
author = "you"
description = "Builds a playlist of your most played songs this week."
entry = "main.rhai"
permissions = ["library.read", "library.write"]
```

Permissions are `library.read`, `library.write`, `playback`, `network`, `files` (the plugin's own folder only), and `audio` (its own effects' sliders and on/off). New plugins start off; turning one on in Settings grants what it lists. A script may define:

- `on_load()`, `on_track_start(track)`, `on_listen(listen)`, `on_pause()`, `on_resume()`
- `commands()` returning `[ #{ id, title, scope } ]`, where scope `"track"` adds the command to the track menu (it receives the selected track IDs) and `"global"` shows a button in Settings
- `run(command, track_ids)`
- `effects()` returning sound effects for Sound › Effects: chains of built-in blocks (gain, filter, compressor, limiter, delay, reverb, chorus, tremolo, saturate, width, pan) whose settings can follow sliders (`"$slider"`), or DSP code as a WebAssembly module (`.wasm`, or `.wat` text) exporting `memory`, `init(rate, channels, max_frames) -> buffer`, `process(frames)`, and optionally `param(index, value)` and `reset()`. Modules get no imports, 32 MB of memory, and a work budget per 512-frame block; one that fails or cannot keep up is stopped and the music plays on. Effects play after the equalizer on shared output and on other speakers, never on exclusive output. The full guide is at https://needle.nnx.fyi/plugins.

The host API: `notify(text)`, `log(text)`, `now()`, `now_playing()`, `library_search(rule)`, `recent_listens(n)`, `set_rating(id, stars)`, `save_playlist(name, ids)`, `play(ids)`, `enqueue(ids)`, `play_next(ids)`, `toggle_playback()`, `next_track()`, `previous_track()`, `http_get(url)`, `http_post_json(url, map)`, `parse_json(text)`, `read_file(name)`, `write_file(name, text)`, `setting(key)`, `set_setting(key, value)`, `set_effect(effect, slider, value)`, and `effect_on(effect, on)`. Calls outside a plugin's permissions fail with a message. Plugins run on their own thread with limits on operations, nesting, and sizes, so an endless loop stops with an error instead of freezing Needle. *Add example plugins* installs seven: now playing to a text file, a weekly favourites playlist, skipping very short tracks, a five-star menu command, Studio effects (Room, Echo, Night mode, Old radio), a Bitcrusher written in WebAssembly text, and Navidrome / Subsonic (a music source). Each bundled plugin has a version and a `.bundled.json` record of the files Needle wrote; when a newer Needle ships a newer version, unchanged copies update themselves on start, and changed ones get *Update* (the changed files are kept as `.mine`) or *Keep mine* in Settings › Plugins.

### Music sources

A plugin with the `network` permission can be a music source: it defines `source()` (a name and sign-in fields), `signed_in()`, `sign_in(fields)`, `sign_out()`, `songs(page)` (pages of song maps until an empty one), `stream(id)` (an http(s) link), and optionally `playing(id)`, `played(id, started_at)`, and `rate(id, stars)`. Its songs become library tracks with the path `source://<plugin>/<id>` and a stable id, so ratings, plays, and playlists survive syncing again; songs the server stops listing are marked missing, never deleted. The whole list is fetched on sign-in, on start (at most every 30 minutes), and with *Update now*. Songs stream into `<data>/stream-cache` while the decoder reads what has arrived (FLAC, MP3, WAV, AAC/M4A, Ogg Vorbis; other types download first); the 2 GB played longest ago are removed first. Covers are downloaded once per link. Tag editing, stems, Fix my library, AcoustID, and measuring for radio skip streamed songs, and *Save to my music* downloads one into the first music folder as a normal file. Helpers: `secret`/`set_secret`/`delete_secret` (Windows Credential Manager, per plugin), `md5`, `random_text`, and `url_encode`.

The bundled **Navidrome / Subsonic** plugin (Settings › Plugins › *Set up*) signs in with a salted token (the password never leaves the computer and is kept in Credential Manager), lists songs with `search3` 250 at a time, streams the original file (`format=raw`), and sends plays (`scrobble`) and ratings (`setRating`) to the server. Its songs appear under *Servers* in the sidebar.

## Data, backups, and layouts

The default location is `%LOCALAPPDATA%\nnx\Needle\data` on Windows; Settings and `needle-cli doctor` show the resolved path. Use `--data-dir PATH` to isolate another library. The folder holds `library.db`, artwork and artist photos, tag backups, plugins, the stem model and split stems when used, and optional demos. Each tag write or restore first copies the file it replaces into `backups`; backups are not pruned automatically. A backup can be restored only when its decoded audio matches the current file. Back up through Settings or the CLI so SQLite’s WAL is included correctly.

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
