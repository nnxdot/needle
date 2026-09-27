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

### Hard on a phone

- Stems.
- Tag editing and Fix my library (desktop jobs).
- Plugins that need a desktop.

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

## Publishing

- **Android:** free as an APK installed directly; Play Store is $25 once.
- **iPhone:** needs the $99 a year Apple Developer account in practice (a free self-signed app
  stops working after 7 days).

## Easier alternative

Grow the existing phone remote (browse the library, queue, lyrics) instead of a full app: small
work, no store needed.
