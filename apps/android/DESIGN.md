# Needle for Android: design rules

One system on every screen. When a screen needs something new, it uses these first.

## Idea

The music is the colour. The page is quiet charcoal (or black, or light, per theme); covers carry
the colour, and the playing or opened cover tints the page around it. Amber (the theme's accent)
marks only what plays, the main action, and links.

## Structure

- **Page:** a large title that shrinks as the page scrolls (Material's flexible top bar), actions
  at its right, and under the title a line of what the page holds (Home: songs and albums;
  Library: albums, artists, playlists). Every top-level page and every page opened from one has it.
- **Covers and tiles sit on the page.** Rows sit in groups (`segment` in Design.kt), as Android
  16's own lists: one tone a step above the page (`groupColor`), the rows 2 dp apart, the group's
  ends rounded 24 dp and the corners between rows 6 dp. No hairlines. The playing song's row
  takes the secondary container, chosen songs the primary container. Settings' groups are the
  same, their rows a gap of the page apart.
- **Tiles** (Home, Library): each on its own tone, Material's secondary or tertiary container,
  its icon's cookie in the tone's strong colour.
- **Lists:** 60 dp rows; cover 52 dp (12 dp corners); title and one line under it; the row's
  menu at the right. Long press opens the same menu.
- **Home:** the first shelf is Material's carousel (multi-browse from three covers; with fewer,
  each keeps one card's width); then the tiles, recently added, your playlists, most played, and
  the artists with the most songs.
- **Grids:** two covers across on phones, a title and one line under each; on tablets as many as
  fit (`gridColumns` in Adaptive.kt).
- **Tablets** (pages 560 dp wide or more) use the room, never a phone's layout stretched: album
  and playlist pages put the cover at the left with the name and buttons beside it; an artist's
  photo is a wide band; the player on its side puts the cover (or the lyrics, or the queue) at the
  left and the controls at the right; Settings shows its list and the chosen section side by side
  from 840 dp.
- **Side bar** on tablets, in place of the tabs at the foot (`SideBar.kt`): the tabs, your
  playlists, the albums played last, and Settings at its foot, as Spotify's. It folds to icons and
  covers; it starts open on screens 840 dp wide or more. It sits on a frame (`surfaceContainer`), and
  the pages in a pane with 28 dp corners beside it (`Modifier.pane()`), as Google's apps on a
  tablet. The open tab's icon is filled, the others outlined.
- **Library** is one page with filter pills at the top (Playlists, Albums, Artists, Songs, and
  More), as Spotify and Apple Music do, not a list of places to go.

## Shapes (each means one thing)

- Covers of albums, songs, and playlists: rounded squares, 12 dp (8 dp when small).
- Artists: Material's arch (`artistShape`), as a portrait's frame.
- Icons in a shape: the scalloped "cookie", always the same one (Settings' sections too).
- Buttons: pills; the main Play is the one large filled button. On a page, Shuffle, Play, and
  the page's own button are one connected group (Material's button group): round at its ends,
  small corners where they meet.
- Waiting: Material's shape-changing loading indicator (pull to refresh, loading dialogs).
- Choosing songs: Material's floating toolbar, in its vibrant colours, Play its one filled button.

## Colour

- Album, playlist, and artist pages, the player, and the mini player take the cover's whole
  Material colour scheme (`CoverTheme`: Material's colour maths on the cover's colour). The page
  is tinted with the cover's hue; buttons, menus, and the playing song use its colours; a
  change of cover blends over, never snaps.
- The top of those pages and the player: the cover, huge, blurred, and slowly drifting
  (`CoverBackdrop`). The pages run up under the status bar and down behind the mini player.
- Everything else: the theme's own surfaces; with "Colours from what's playing" on, the whole
  app takes the playing song's scheme.
- Settings › Appearance chooses: cover colours on pages, colours from what is playing, the mini
  player's colour, the moving backdrop and its softness, the seek bar (wave, line, thick), and
  the player's cover (square, a turning record, a Material shape).

## Navigation

- Every page opened from another has a back button at its top left, besides the back gesture.
- Covers fly between pages of one tab only; switching tabs cross-fades.

## TV and keys

- On Android TV the app is the tablet's: the side bar, the pane, the wide player. A remote
  moves the focus (`TvFocus.kt`).
- What has the focus shows plainly, from across a room: cards grow a little and take a ring in
  the text colour (`pressScale`, `focusRing`); other buttons take a ring or a deeper shade.
- The player and the welcome guide keep the focus while they are open (`focusLayer`). A page
  opened from the page takes the focus; its Play first, where it has one (`firstFocus`).
- Text boxes let the remote leave them up and down (`remoteEscape`); on a TV the search box
  opens the keyboard only on OK (`RemoteTyping`).

## Type

- Roboto Flex, one variable font. Text at its usual width; display and headline sizes narrow
  and heavy (`titleFamily`), as posters are. Big titles grow narrower as their page scrolls.
- Section titles use the emphasized styles (`titleLargeEmphasized`): the title font, a step
  heavier.

## Menus

- A song's menu is a sheet from the bottom: the song at the top with its stars, then its actions
  in rounded groups with gaps between them (Material 3 Expressive menus).
- Albums and playlists have the same kind of sheet: a long press on a cover, or More at the top
  right of their page.
- Small choices (sort, view) are Expressive dropdown menus.

## Motion

- Pages slide a little and fade; covers move between the grid, the page, and the player.
- Rows and tiles come in one after another as a page opens (`entrance`), never when scrolled to.
- On album and playlist pages, the cover shrinks toward its top and fades as the page
  scrolls; the artist photo moves slower than the page. A bar with the name fades in once
  the top has gone.
- Buttons change shape (Material 3 Expressive): pressed ones square their corners a little;
  play is round while paused and a softer square while playing.
- Progress is a wave while music plays and a flat line while it rests (Android 16).
- Presses shrink a little and spring back. Nothing bounces for show.
- "Reduce motion" (or Android's own setting) turns it into cross-fades.
