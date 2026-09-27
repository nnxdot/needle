# Needle on phones (plan, not started)

Each app uses its own platform's design: **Material 3 Expressive**
on Android, **Liquid Glass** (iOS 26) on iPhone.

## Why it is a big job

- GPUI (Needle's desktop UI) runs only on Windows, macOS, and Linux. Every screen must be built
  again with a phone toolkit.
- The Rust core (library, playback, tags, lyrics, plugins, server sync) can be reused through
  UniFFI bindings.
- New phone work: background playback, lock screen and notification controls, and file access
  (iOS sees only files imported through Files, or streamed).
- Stems (ONNX) are too heavy for a phone: left out. FFmpeg for Dolby needs its own phone builds.
- Estimate: months, not days. The screens are the work.

## Android

### Built with

- **UI:** Jetpack Compose with Material 3 Expressive: bold shapes, spring motion, colours from
  the wallpaper (dynamic colour).
- **Core:** the Needle Rust core, called from Kotlin through UniFFI.
- **Playback:** Android's media service (Media3 `MediaSessionService`): lock screen and
  notification player, Bluetooth buttons, Android Auto, background play.

### Where the music comes from

1. Files on the phone or SD card.
2. The user's home Needle, streamed over the home network (builds on the phone remote).
3. Navidrome / Subsonic servers, through the same plugin as desktop.

### Screens

- Bottom bar: Home, Library, Search, Playlists.
- Mini player above the bar; swipe up for the full player.
- Full player: big cover with colours from it, seek bar, lyrics and queue one swipe away.
- Library: albums, artists, songs, as on desktop.
- Settings: the desktop sections, shorter.

## Needle's features on a phone

### Carry over well

- Playback: lossless formats (FLAC, ALAC, WAV, …), Opus, gapless, crossfade, ReplayGain, and
  the loudness scan that measures it.
- Sound: the equalizer, crossfeed, and plugin sound effects.
- Lyrics: timed lyrics, karaoke, `.lrc` files, NetEase, and the lyrics timing editor (tap along).
- Library: albums, artists, genres, songs, folders, favorites, recently added, history, search,
  covers, star ratings, CUE sheets.
- Smart search ("rating is at least 4", a folder path) and smart playlists; playlist pictures.
- Radio from the library.
- A-B loop (repeat part of a song).
- Colour from the playing cover, and the immersive full screen (drifting cover, large lyrics).
- Listening stats (the History page) and Wrapped.
- Scrobbling: Last.fm and ListenBrainz.
- Servers: Navidrome / Subsonic, with the experimental octo-fiesta search.
- Online lookups (off until turned on): LRCLIB lyrics, MusicBrainz and Wikidata artist photos.
- Themes, and plugins (the plugin engine is Rust, so it runs on a phone).
- Casting: Chromecast and DLNA. AirPlay is harder on Android.
- The sync file: history, ratings, and playlists exported and imported as one encrypted file with
  a passphrase (`crates/needle-core/src/sync.rs`); no audio or passwords. A base for phone and PC
  sync.
- The demo library, What's new, and crash reports.

### Work, but changed for a phone

- The big player / full screen becomes the phone's full player.
- The mini player becomes the bottom bar, the lock screen, and the notification player.
- Media keys and the tray become Android's media controls, Bluetooth buttons, and Android Auto.
- Sound outputs: Android picks the speaker or headphones; Needle offers only a few choices.
- Exclusive mode (Windows) becomes bit-perfect USB DAC output (Android 14 and newer).
- Dolby (Atmos) music needs FFmpeg built for Android.
- The phone remote turns around: the app controls the desktop Needle (see Connect below).
- The command palette becomes a search that also runs actions.
- Drag and drop becomes long-press menus.
- Updates: the Play Store; or, for a directly installed APK, Needle's own updater (Android asks
  to allow it).

### Hard on a phone

- Stems (too heavy).
- Tag editing and Fix my library (desktop jobs).
- Importing from other players.
- Discord status (Discord's phone app does not allow it).
- Plugins that need a desktop.

### Desktop only

- Window glass (Mica, Acrylic), window sizes, the tray, and the command line tool.

## iPhone

- **UI:** SwiftUI with Liquid Glass: the tab bar, mini player, and buttons get the glass look
  from the system.
- **Core:** the same Rust core, called from Swift through UniFFI.
- Same layout as Android, in Apple's style: tab bar with the mini player on it, lock screen and
  Dynamic Island controls, CarPlay.
- Limit: only music imported through the Files app, or streamed.

## Live Activities and widgets

### Live Activities (iPhone) and Live Updates (Android)

- The playing song needs no Live Activity: iPhone shows it on the lock screen and in the Dynamic
  Island through Now Playing (`MPNowPlayingInfoCenter`), as Apple Music does. On Android it is
  the media notification, the lock screen player, and the quick settings player.
- A Live Activity (ActivityKit) fits other things: a sleep timer counting down, music syncing or
  downloading from the home Needle or a server, and radio showing what is up next.
- Android's equivalent is Live Updates (Android 16 and newer), for progress such as a sync.

### Widgets

- Android: Jetpack Glance, in Material style. iPhone: WidgetKit, with the Liquid Glass look.
- Now playing: the cover, the song, and play, pause, and skip.
- Recently played: tap an album to play it.
- Playlists: shortcuts to favourite playlists.
- Radio: one tap starts radio from the library.
- Stats: listening time this week, or a Wrapped-style fact.
- iPhone also: lock screen widgets and Control Center buttons (for example "Play my radio").

## Connect (like Spotify Connect)

### What Needle has now: the phone remote

- Turned on in Settings; a QR code opens a control page in the phone's browser (nothing to
  install). Code: `crates/needle-core/src/remote.rs`, `remote.html`.
- Play, pause, next, previous, seek, volume, shuffle, repeat; search the library and play, play
  next, or add to the queue; jump to a song in the queue.
- Only private-network addresses (home network, or a VPN using them); every address holds a long
  random key.

### What Spotify Connect has that Needle does not

1. Devices found by themselves (Needle needs the QR code).
2. Moving playback between devices at the same spot.
3. The phone as the speaker (the remote only controls the PC).
4. Working away from home (Spotify goes through its own servers).

### What a phone app could add

- Discovery of PCs running Needle on the network with Bonjour / mDNS: no QR code.
- Moving playback both ways between PC and phone: the song, the queue, and the position.
- Playing the PC's music on the phone, streamed over the home network.
- One PC controlling another PC the same way.
- Away from home: through a VPN such as Tailscale, with no NNX server; or through a relay server,
  which needs accounts and servers (see the account discussion: optional, end-to-end encrypted).

## Publishing

- **Android:** free as an APK installed directly; Play Store is $25 once.
- **iPhone:** needs the $99 a year Apple Developer account in practice (a free self-signed app
  stops working after 7 days).

## Easier alternative

Grow the existing phone remote (browse the library, queue, lyrics) instead of a full app: small
work, no store needed.
