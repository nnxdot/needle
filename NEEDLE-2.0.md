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

### Eleven new things

1. **Visualizer.** Light that moves with the music in the cover's colours: in the big player,
   and as a full-screen mode (parties, a TV). All systems. The core gives the spectrum of what
   plays.
2. **Waveform seek bar.** The song's real loud and quiet shape in the seek bar. All systems.
   The core measures and keeps each song's peaks.
3. **Word-by-word lyrics and translation.** Words light up as they are sung, from word-timed
   lyrics (enhanced LRC `<mm:ss.xx>` word stamps; LRCLIB has only lines); every app already
   does this. A translated line
   can show under each line: on the device on Apple (Translation) and Android (ML Kit);
   Windows and Linux need another way. The core parses word timing.
4. **CarPlay.** iPhone. Needs Apple's CarPlay audio entitlement (the paid developer account).
   Android Auto exists already.
5. **Watch app.** Apple Watch and Wear OS: control Needle; later, songs kept on the watch.
6. **Mood map.** Every song a dot, by calm ↔ lively and dark ↔ bright (`radio.rs` already
   measures this); circle an area to play it. All systems.
7. **Lyric cards to share.** A picture of a chosen lyric line in the cover's colours. All
   systems.
8. **Your library in numbers.** Formats, hi-res share, decades, genres, top artists, and
   albums with missing covers or low quality. All systems; the core counts.
9. **On this day.** A Home shelf of what you played one year and five years ago. All
    systems; the core finds them.
10. **Smart mixes.** The crossfade ends where a song's music ends (not in its silent tail)
    and lasts whole bars at its tempo (core `smart_mix`, `crossfade::smart_ending`). It does
    not line up the beat phase. All systems where Needle mixes the sound.
11. **Record crate.** Flip through albums like records in a box. All systems.

Listen together was dropped by the user on 2026-10-25: not in 2.0, on any system.

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
  used too: `artist.jpg` or `folder.jpg` for the photo, before any found online. For the wide
  picture and the logo the order is: the listener's choice, then fanart.tv's (while online
  lookups are on and a fanart.tv key is set), then `fanart.jpg`/`backdrop.jpg`/`banner.jpg`
  and `logo.png`/`clearlogo.png` in the artist's folder (the user's call, 2026-10-25).

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

### Four more, from the design mockups (the user's picks, 2026-10-26)

The user chose these from the "pushing the design further" mockups: Search (4), the TV's Home
(6), the big player's three looks (7), and motion (8). Not chosen: Home's stage and mixes,
the album page's extra parts, and the artist poster.

**Search page.** All systems.
- Typing in search on a page that is not a list (Home, an artist's page, Settings, the
  Library in numbers…) opens Search, where it opened Songs before. On a list page (Songs,
  Albums, Artists, a playlist, an album, a folder) it still filters that list.
- Under the search field: chips **All, Songs, Albums, Artists, Lyrics**. Songs, Albums, and
  Artists open those pages filtered by the search (as today). Lyrics lists every song whose
  lyrics hold the words (up to 50).
- All shows, in order: the **best match** large (its cover or photo, "Song", "Album", or
  "Artist", the name, and Play), **Songs** (up to 5, with Play on hover or tap), **Albums**
  (a row of covers), **Artists** (circles), **In the lyrics** (up to 3: the line, the words
  lit in amber, and the song under it; it plays from the start), and **Try a rule** (the
  chips in `find::RULE_EXAMPLES`: tapping one puts its rule in the search field).
- A section with nothing is left out. With nothing at all: "Nothing matches". A rule that
  does not compile shows its error, as the Songs page does.
- Core: `Library::find(query, limit)`; phones and Mac: `Needle::find(query, limit)` →
  `SearchFound` and `Needle::rule_examples()` (name, rule, name, rule…). The best match is
  the album, artist, or song whose name is the search, starts with it, or holds it, in that
  order; albums before artists before songs when two are as close; the most played wins.
- Lyrics are searched in an index of each song's own lyrics (the file beside it and its
  tags), read slowly in the background (`find::start_reading_lyrics`, started by
  `Needle::new` and the desktop app), and of any lyrics shown since (LRCLIB, a server,
  plugins). Nothing is fetched to fill it. Three letters at least.

**TV Home.** Android TV only.
- The album in focus fills the screen behind (its cover, wide, darkened at the left and
  foot); moving the focus changes it, with a short fade.
- Its kind ("Jump back in"…), name, artist, year, and songs sit large above the row, with
  Play and Shuffle.
- The tile in focus grows, with a 3 dp ring in the cover's colour (amber when it has none).
- The visualizer is one button away (on the player).

**The big player's looks.** All systems.
- A **Look** button in the big player steps through four looks, kept in the setting
  `big_look` (`model::BIG_LOOKS`: "cover", "record", "lyrics", "visualizer"; phones:
  `LibraryChoices.big_look`):
  - **Cover**: as today, the cover and the lyrics beside it.
  - **Record**: the cover as a turning record (grooves, the cover as its label, the arm
    resting on it), over the cover's colours. Android's Record cover shape is the model.
  - **Lyrics**: only the words, very large, centred, lit as sung (word by word where timed),
    with the translation under each line when Translate is on. No lyrics: the Cover look.
  - **Visualizer**: the visualizer's light fills the space.
- Every look keeps the player's buttons, the seek bar, and the cover's colours.
- Reduce motion: the record keeps still, and the visualizer rests.

**Motion.** All systems.
- **The cover goes to its page.** Choosing an album: its cover grows into the album page's
  cover (the Mac: `matchedGeometryEffect`; the iPhone: zoom transition; Android: shared
  element). Windows and Linux: every page fades in and rises 10 px (GPUI has no shared
  elements, and cannot scale a picture as it draws).
- **The header folds on scroll.** On album, artist, and playlist pages, scrolling down shrinks
  the big title into the title bar (or the top bar on phones). Windows and Linux: on album and
  playlist pages the cover shrinks to 72 px and the title to 26 px over the first 200 px of
  scrolling, and the path and details lines go; the artist page's picture already scrolls
  away with the page.
- **Covers glow under the pointer.** Desktop: a cover lifts by 2 px, with a soft light in its
  own colour behind it.
- **Shelves fill in** with soft placeholder tiles while they load (done on the desktop).
- **The colour follows the music.** When the song changes, the sidebar's tint and the
  player's colour change to the new cover's over about half a second, not at once. Windows and Linux already did (the palette fades between covers).
- **Reduce motion** (Needle's or the system's): all of these become plain fades or nothing.

### From the new-ideas mockups (artifacts/ideas, approved 2026-10-26)

The mockups' names and numbers are made up. Each feature's logic is in the core once; the
phones reach it through `crates/needle-mobile/src/core2.rs`.

**Sleep timer.** All systems. 15, 30, 45 minutes, 1 hour, End of song, End of album; **Fade
out** (over the last minute; kept in `sleep_fade`); a moon button in the player, lit while it
runs, with the time left on hover. When it ends the music pauses; for End of song or album the
next song waits paused at its start (it is held back, so none of it plays; with crossfade on,
the overlapping seconds of the last song are cut). Repeat and Autoplay do not start after it.
Fading needs the shared output: exclusive output keeps its volume. **Then sleep the PC** is
desktop only (`sleep_computer`); **Shake to add 10 minutes** is Android only. Core:
`audio::Command::Sleep`, `SleepLonger`, `SleepFade`; `PlaybackState::sleep`, `sleep_ended`.

**Speed and pitch.** All systems where Needle makes the sound. Speed 0.5× to 2× (chips 0.75,
1, 1.25, 1.5, 2), pitch −12 to +12 semitones, **Keep the pitch** (time-stretch: Needle's own
WSOLA in `tempo.rs`, no new library), **Only for this song**. Off on exclusive output and
Android's bit-perfect USB (`PlaybackState::tempo_available`). Kept in `speed`, `pitch`,
`keep_pitch` unless for one song. The song's time (seek bar, lyrics, listens) runs in the
song's own seconds.

**Year in review** ("Your 2026 in music"). All systems. Hours, plays, new songs found that year,
plays by month, the top artist with hours (and the months in the top five), top songs with
plays (up to 50). Opens from Listening history; Home shows it in December. **Save as a picture**
(Pictures › Needle on computers, as lyric cards), **Make a playlist** ("Top songs of 2026",
saved again it is refreshed), **Play top 50**. Core: `Library::wrapped`,
`save_year_playlist`, `wrapped::default_year`, `home_year`.

**Time the lyrics.** All systems. Tap as each line starts (or each word, in word mode),
−0.2 s / +0.2 s, Redo line, Undo; saved as an `.lrc` beside the song (phones: in Needle's own
storage, as before). Core: `timing::LyricTiming`; phones: the `LyricTimer` object.

**Taskbar buttons, global shortcuts, the small floating player.** Windows and Linux only, in
`crates/needle`.

