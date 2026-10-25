# Needle 2.0

What 2.0 holds, for every system. The user approved all of it on 2026-10-25. Each item says
which systems it is for and who builds it: **core** (the shared Rust in `crates/needle-core`,
built by the coordinator thread), **Windows** (also Linux: `crates/needle`), **Apple** (macOS
`apps/macos`, iPhone `apps/ios`), **Android** (`apps/android` with `crates/needle-mobile`).
Settings follow [SETTINGS-STYLE.md](SETTINGS-STYLE.md).

Branches: `needle-2.0` holds the core and Windows work; the platform threads work on their
own branches from it and merge it again when the core changes.

## One look on every screen

The Mac app is the model. Six rules:

1. **The player floats.** One rounded bar of glass over the foot of the page; the page runs
   on under it. No flat band across the window.
2. **Panels float.** Queue, lyrics, and details sit in a rounded pane (22 px corners, 320 px
   wide) over the page's right side, not in a column with a line.
3. **One quiet background.** Night, Midnight, or Day behind everything, tinted by the cover.
   No lines between the parts of the window; only soft lines between rows.
4. **The music is the colour.** Album and artist pages take their cover's colour. Amber
   (`#e2b46c` on dark, `#8c5a12` on light), or the cover's colour, marks only what plays and
   the one main button.
5. **Big title, small text.** One bold page title (30–34 px desktop, 28 px phone); 13 px text
   with grey second lines.
6. **Soft covers.** 6–8 px corners and a soft shadow; 12 px in the player. Artists are
   circles everywhere.

### Windows and Linux

- The player is the Mac's floating bar (56 px high, at most 820 px wide): transport, a small
  "screen" with the cover, the song, "artist — album", and a thin progress line along its
  foot, then the panels' buttons and the volume. The format chip (`FLAC 24/48`) sits in the
  screen.
- The side panel (Details, Queue, Lyrics) is a floating pane, 22 px corners.
- The page is a sheet on the window's Mica with one round top-left corner, as in Windows 11's
  own apps. The sidebar sits on the Mica.
- Search is at the top of the sidebar; Ctrl+K still opens the palette with rules.
- Back, Forward, the sidebar button, and the palette share one small rounded group.
- Stays Windows: the caption buttons, Segoe UI Variable, Mica.
- GPUI cannot blur behind a pane, so the floating parts use a strong see-through fill
  (about 95 %) with a soft shadow and a thin light line.
- Linux has no Mica: the look's own colours.

### Android: the Needle look

The user chose the Needle look over Material 3 Expressive.

- Home is the Mac's Home: a greeting, the song playing now (a card with its cover and a round
  amber Play/Pause), then shelves of covers ("Jump back in", "New in your library",
  "On repeat", …), each with a grey line under its title.
- The mini player and the tabs float together as one small dock of frosted glass (real blur
  on Android 12 and newer, a solid fill before).
- Plain rows with soft lines, no filled groups. No scalloped "cookie" shapes and no arch:
  soft squares for covers, circles for artists and round buttons.
- Album, artist, and player pages keep the cover's colour and the blurred cover at the top,
  with calmer buttons: a filled Play pill in the page's colour and a glass Shuffle pill.
- Stays Android: the back gesture, the touch ripple, Roboto, the system's share sheet and
  media controls.
- Tablets and TVs get the desktop layout: the sidebar, the floating player, the floating pane.
- `apps/android/DESIGN.md` is rewritten to match.

### iPhone

Already close (Liquid Glass, the mini player in the tab bar, album pages in the cover's
colour). Home becomes the Mac's Home (no giant cards with glass labels); amber instead of pink;
the same words as the Mac for shelves, titles, and menus.

## Features

### Player buttons you choose

All systems. A **Player buttons** row in Appearance › Layout opens under itself: a small copy
of the real player bar to drag buttons in, the unused buttons in one line below, and a button
dragged out of the bar leaves it. Buttons: Shuffle, Previous, Play/Pause, Next, Repeat,
Favorite, Lyrics, Queue, Speakers. Android's notification takes the same buttons as far as it
allows; the iPhone lock screen and the Windows media overlay stay as they are. The choice is a
setting in the core (`player_buttons`, a list of names in order).

### Needle's own USB DAC driver

Android. Today Android uses its own bit-perfect mixer (`ui/Usb.kt`): Android 14 and newer,
44.1 or 48 kHz, stereo 32-bit or 16-bit only. Needle's driver talks to the DAC directly
(UsbManager permission, `claimInterface(force = true)`, libusb on the device's file
descriptor) with USB Audio Class 1 and 2: every rate the DAC offers, 16/24/32-bit in 2, 3,
or 4-byte slots, the async feedback endpoint, the DAC's own volume, DSD over DoP (and native
DSD for known chips), on Android 8 and newer. It runs behind the existing **Bit-perfect USB**
switch, with Android's mixer as the fallback when a DAC does not work with it. While Needle
holds the DAC, other apps play from the phone.

### Twelve new things

1. **Visualizer.** Light that moves with the music in the cover's colours: in the big player,
   and as a full-screen mode (parties, a TV). All systems. The core gives the spectrum of what
   plays.
2. **Waveform seek bar.** The song's real loud and quiet shape in the seek bar. All systems.
   The core measures and keeps each song's peaks.
3. **Word-by-word lyrics and translation.** Words light up as they are sung, from word-timed
   lyrics (enhanced LRC `<mm:ss.xx>` word stamps; LRCLIB has only lines). A translated line
   can show under each line: on the device on Apple (Translation) and Android (ML Kit);
   Windows and Linux need another way. The core parses word timing.
4. **Listen together.** Friends hear the same song at the same moment, each from their own
   library or server. A Discord activity invite carries the room (presence scopes, no lobby
   approval); a small room service of Needle's own on Cloudflare passes play, pause, and seek.
   (Discord lobbies were checked: limited access, 100 joins and 100 messages per 2 hours for
   all users together.)
5. **CarPlay.** iPhone. Needs Apple's CarPlay audio entitlement (the paid developer account).
   Android Auto exists already.
6. **Watch app.** Apple Watch and Wear OS: control Needle; later, songs kept on the watch.
7. **Mood map.** Every song a dot, by calm ↔ lively and dark ↔ bright (`radio.rs` already
   measures this); circle an area to play it. All systems.
8. **Lyric cards to share.** A picture of a chosen lyric line in the cover's colours. All
   systems.
9. **Your library in numbers.** Formats, hi-res share, decades, genres, top artists, and
   albums with missing covers or low quality. All systems; the core counts.
10. **On this day.** A Home shelf of what you played one year and five years ago. All
    systems; the core finds them.
11. **Smart mixes.** Crossfade that lines up the beats of the two songs (each song's BPM is
    known) and fades at the right moment. All systems where Needle mixes the sound.
12. **Record crate.** Flip through albums like records in a box. All systems.

### Artists

- **Better sorting.** ARTISTSORT and ALBUMARTISTSORT tags; accents and other scripts sort
  properly ("Ólafur" with the O's); "A feat. B" and "A & B" listed under each artist; an
  **Only album artists** choice; sort by name, most played, most songs, or recently added; an
  A–Z bar. Today (`browse.rs` `Library::artists`) artists group by the track artist and sort
  by name without a leading "The ", and nothing else.
- **Pictures for every artist.** A slow fetch in the background (one artist at a time, only
  while idle, Wi-Fi only on phones, with progress such as "212 of 640 artists") and a
  **Check all now** button. MusicBrainz artist ids are read from the tags
  (MUSICBRAINZ_ARTISTID) so most artists skip the name search; MusicBrainz stays at one
  request a second.
- **Your own artist pictures.** On an artist's page, the photo, the banner, and the logo each
  have **Choose picture…**, **Pick from fanart.tv** (all of fanart.tv's choices), and
  **Reset**. A chosen picture is kept and never replaced. Pictures in the artist's folder are
  used too: `artist.jpg` or `folder.jpg`, `fanart.jpg`, `banner.jpg`, `logo.png`.

### Fonts

Two rows in Appearance: **Titles** and **Text**, with the same fonts on every system, carried
inside Needle (all SIL Open Font License), each name shown in its own font, and **a font on
this device** (an installed font on computers; a .ttf or .otf file on phones). Titles: Plain,
Poster (Roboto Flex, narrow and heavy), Fraunces, Instrument Serif, DM Serif Display, Space
Grotesk, Bricolage Grotesque, Outfit. Text: Plain, Inter, Atkinson Hyperlegible, Lexend, IBM
Plex Mono. Bahnschrift (Windows today) gives way to Poster. Today only titles change, and the
iPhone has no choice at all. Theme files' `font` sets the title font; a new `text_font` sets
the text.

### The queue

- **A queue in immersive mode.** A Queue button beside the Lyrics button switches the side
  between the lyrics and what plays next. Today the Mac's immersive mode shows only lyrics
  (`BigPlayer.swift` `side`) and Windows' has no queue (`immersive.rs`).
- **A highlight under the pointer** on queue rows, as Windows has (`panel.rs`): the Mac's
  `QueueRow` gets the same soft background, in the side panel, the big player, and the mini
  player.
