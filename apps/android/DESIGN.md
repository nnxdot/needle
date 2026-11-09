# Needle for Android: design rules

The Needle look, after Needle for the Mac (NEEDLE-2.0.md). One system on every screen. When a
screen needs something new, it uses these first.

## Idea

The music is the colour. The page is one quiet background (Night, Midnight, or Day); covers
carry the colour, and album, artist, and player pages take their cover's colour. Amber (the
theme's accent, `#e2b46c` on dark, `#8c5a12` on light) or the cover's colour marks only what
plays and the one main button.

## The six rules

1. **What plays floats.** On a phone, the mini player and the tabs are one small dock of
   frosted glass over the foot of the page (`Dock.kt`); the page runs on under it. On a tablet
   or TV, the player alone floats over the page's foot, at most 820 dp wide.
2. **Panels float.** On a tablet or TV, the queue, the lyrics, and the details sit in a rounded
   pane (22 dp corners, 320 dp wide) over the page's right side, not in a column with a line.
3. **One quiet background.** Behind everything, tinted by the cover where the page is a
   cover's. No lines between the parts of the screen (no frame, no band behind a scrolled
   title); only soft lines between rows. On a tablet or TV the side bar sits on the window's
   darker colour and the page is a sheet beside it (see below).
4. **The music is the colour.** Album, artist, and playlist pages and the player take their
   cover's colours (`CoverTheme`) and the blurred cover at their top (`CoverBackdrop`).
5. **Big title, small text.** One bold title per page (28 on a phone, `headlineMedium`), with
   the rows' text small and their second lines grey.
6. **Soft covers.** 6–8 dp corners and a soft shadow; 12 in the player. Artists are circles
   everywhere.

## Glass

- One material floats: glass (`Glass.kt`). All glass is drawn there, so every part matches.
  - The fill: the page's raised colour. On Android 12 and newer the page under it is blurred
    (Haze, RenderEffect) and the fill is see-through; before that, the fill is solid. A phone
    with little memory is solid too, and so are the big panes on a TV.
  - The edge light: a 1 dp line, brighter along the top and fading down the sides, and a faint
    outline. No flat line round it. On light glass (Day) a white line would not show: there it
    is a soft dark line round the edge with the white light just inside it.
  - A sheet's edge runs its whole real height, top and sides; a sheet has no line along its
    bottom, which is the screen's.
  - A soft shadow under it.
  - The dock and the floating player take the playing cover's colour from the left, with
    "Mini player in the song's color" on. A song's, an album's, and a playlist's sheet take
    that cover's colour faintly from the left, always. Dialogs and menus stay neutral.
- Where it is used:
  - The dock and the floating player: blurred glass over the page.
  - The side pane (lyrics, Up next): dark glass in the cover's colour over the page, so its
    white words read in any theme. Where it cannot blur, the player's solid backdrop.
  - The side bar: a pane of glass on the window, inset 6 dp, 20 dp corners. Nothing but the
    window is under it, so its fill is solid.
  - Sheets and dialogs (`GlassSheet`, `GlassAlertDialog`) are their own windows, which the
    page's blur does not reach. The system blurs what is behind their window (Android 12 and
    newer, where it is turned on; not on a TV), set once as the window opens, at its full
    radius; only the fill animates: it turns a little see-through as the window opens, and solid
    again as a sheet leaves, so the page never shows sharp through it. Elsewhere they keep the
    solid fill. Both have the edge light.
  - Menus (`GlassMenu`): a solid fill with the edge light. The system's blur behind a small
    menu would blur the whole screen.
  - The top of a page: once the page has scrolled, what scrolls up blurs and fades into the
    page under the status bar (`TopEdge`), and fades under a page's pinned title bar. Before it
    has scrolled there is no fade over the page's top, and nothing to blur.
  - Messages (the snackbar), a page that could not load (one message, above the dock, inside the
    page, never a window over the dock), and the bar of chosen songs: glass bars in the dock's
    shape.
  - The alarm's time: Material's time picker in a glass dialog (with a remote, the boxes to type
    the time in).
- Nothing else is filled: no tiles, no filled groups.

## Structure

- **Page:** a bold title at the top left (`Page` in Design.kt shrinks it into a bar as the
  page scrolls, on the page's own colour; `LargeTitle` scrolls with the list), actions at its
  right. A page opened from another has a back button above or beside its title, besides the
  back gesture.
- **Rows:** plain, on the page. A soft line under each row but the last, from where the row's
  words start (`segment`, `softLine`). The playing song's row: its title in the accent and a
  faint tint; chosen rows: a soft accent tint. A row lit has 10 dp corners (`RowShape`).
- **Lists:** 60 dp rows; cover 48 dp; title and one grey line; the row's menu at the right.
  Long press opens the same menu. Settings › Appearance › Density makes them 48 or 76 dp.
- **Home** is the Mac's Home: a greeting, the song playing now (a card with its cover, "Playing
  now" in the accent, and a round Play/Pause), then shelves of covers ("Jump back in", "New in
  your library", "On repeat", "Favorites") and "Artists you play most" in circles, each shelf
  with a grey line under its title and See all where there is a page of it.
- **Library** is one page with filter pills at the top (Playlists, Albums, Artists, Songs, and
  More). With no pill chosen: Favorites, History, and Radio as plain rows, then what came in lately.
- **Radio** (`Radio.kt`) is the iPhone's Radio tab, under Library on a phone (the dock keeps its
  three tabs) and in the side bar under Home: your station (a large cover, darkened at its foot,
  with a round Play), quick mixes as plain tiles in the page's raised colour (not glass, not
  Material cards), each mix's sign in the colour of a cover from it, and a station for each song
  played most.
- **Grids:** two covers across on phones, a title and one grey line under each; on tablets as
  many as fit (`gridColumns` in Adaptive.kt).
- **Search** (`Search.kt`): the field, then chips (All, Songs, Albums, Artists, Lyrics). All
  shows the best match in a card (cover, kind, name, a round Play), then Songs (5), Albums and
  Artists as rows of tiles, In the lyrics (the words lit in amber), and Try a rule.
- **The big player's looks:** the Look button at its top steps through Cover, Record (the round
  cover turning, grooves, the arm resting), Lyrics (only the words, large and centred), and
  Visualizer; kept in `LibraryChoices.big_look`.
- **Settings:** rows under short headings on the page itself, a soft line between rows
  (SETTINGS-STYLE.md).

## Tablets and TVs: the desktop layout

- Pages 560 dp wide or more use the room, never a phone's layout stretched: album and playlist
  pages put the cover at the left with the name and buttons beside it; an artist's picture is a
  wide band.
- The side bar (`SideBar.kt`) in place of the dock's tabs, as on the Mac and the computer:
  - 240 dp wide, a pane of glass on the window's colour (`surfaceContainerLowest`). No logo.
  - At its top, a rounded search field ("Search"); it opens the Search page.
  - Home, Radio, Songs, Albums, Artists, Folders. Then a grey "Collections" label: Favorites,
    Recently added, Listening history. Then a grey "Playlists" label with a +, and the
    playlists, each with an 18 dp cover.
  - At its foot: Add music folder, Fix my library, Import, Settings.
  - Small rows (34 dp): an 18 dp grey icon and 14 sp text. The row open has a soft rounded
    highlight (the text colour at 8 %, 8 dp corners) and an amber icon.
  - Where the list is longer than the screen, its cut edges fade (`fadeCut`).
  - Below 840 dp it starts folded to its icons (64 dp); the button beside the search field
    folds and opens it.
- The page is a sheet beside the side bar: the page's background, with one round top-left
  corner (18 dp; `Modifier.pane` in Adaptive.kt).
- The player floats over the page's foot (`FloatingPlayer`), and the side pane over its right.

## Shapes (each means one thing)

- Covers of albums, songs, and playlists: soft squares, 8 dp (6 dp when small), so the picture
  stays the picture; 12 dp in the player.
- Artists: circles.
- Round buttons and icons in a shape: circles. No scalloped "cookie" shapes and no arch.
- Buttons: pills. A page's buttons are a filled **Play** pill in the page's colour and a glass
  **Shuffle** pill beside it; the page's own button (Add to a playlist, Edit, Radio) is a round
  glass button.
- Choosing songs: a floating glass bar in the dock's shape with the chosen songs' actions, Play
  its one filled button (a circle).

## Colour

- Album, playlist, and artist pages, the player, and the mini player (Settings › Appearance)
  take the cover's whole colour scheme (`CoverTheme`: Material's colour maths on the cover's
  colour). A change of cover blends over, never snaps.
- The top of those pages and the player: the cover, huge, blurred, and slowly drifting
  (`CoverBackdrop`). The pages run up under the status bar and down behind the dock.
- Everything else: the theme's own background; with "Colors from the music" on, the
  whole app takes the playing song's scheme.

## Stays Android

- The back gesture, the touch ripple, Roboto (Roboto Flex for titles), the system's share sheet,
  and the system's media controls (notification, lock screen, Android Auto, widgets).

## Foldables

- Flat, a foldable is laid out by its width like any screen: the phone layout on the cover
  screen, the side bar and floating player when open. Folding and unfolding do not restart
  Needle (MainActivity handles the size change), so the song, the page, and its place stay.
- Half closed (Jetpack WindowManager's FoldingFeature, `Fold.kt`), nothing sits on the crease:
  - Tabletop (crease across): the pages and the side bar above it, the bar of controls below
    it; the full player puts the cover (or lyrics, or the light) above and the song and its
    controls below.
  - Book (crease down): the library at the left, laid out by its half's width, and the player
    always at the right.

## TV and keys

- On Android TV the app is the tablet's: the side bar, the floating player, the wide player.
  A remote moves the focus (`TvFocus.kt`).
- Home on a TV (`TvHome.kt`): the album in focus fills the screen behind (darkened at the left
  and foot, a short fade as the focus moves); its shelf, name, artist, year, and songs large
  with Play and Shuffle; one row of tiles under it, the one in focus a little larger with a
  3 dp ring in its cover's colour.
- The side bar is folded to its icons and opens while the focus is in it (move left from the
  page). Its focus ring follows the highlight's shape: a 2 dp ring inside the row's 8 dp
  corners (`softRing`).
- What has the focus shows plainly, from across a room: cards grow a little and take a ring in
  the text colour (`pressScale`, `focusRing`); other buttons take a ring or a deeper shade.
- The player and the welcome guide keep the focus while they are open (`focusLayer`). A page
  opened from the page takes the focus; its Play first, where it has one (`firstFocus`).
- Text boxes let the remote leave them up and down (`remoteEscape`); on a TV the search box
  opens the keyboard only on OK (`RemoteTyping`).

## Type

- Roboto for text; Roboto Flex for titles (Settings › Appearance › Titles chooses another).
- Titles bold; section titles semibold; rows' text small with a grey second line.

## Menus

- A song's menu is a sheet from the bottom: the song at the top with its stars, then its actions
  as plain rows, a soft line between groups.
- Albums and playlists have the same kind of sheet: a long press on a cover, or More at the top
  right of their page.
- Small choices (sort, view) are dropdown menus.

## Motion

- Pages slide a little and fade; covers move between the grid, the page, and the player.
- Rows and covers come in one after another as a page opens (`entrance`), never when scrolled to.
- On album and playlist pages, the cover shrinks toward its top and fades as the page scrolls;
  the artist picture moves slower than the page. A bar with the name fades in once the top has
  gone.
- Progress is a wave while music plays and a flat line while it rests.
- Presses shrink a little and spring back. Nothing bounces for show.
- Back: the back swipe shows the page behind as the page slides off to the right, and goes there
  on release (predictive back); let go early, the page slides back.
- Rows in the queue and in playlists slide to their new places when a song is taken out or moved
  (not after a drag, whose rows are in place already).
- The dock's open tab sits on a soft light that slides from tab to tab.
- "Reduce motion" (or Android's own setting) turns it into cross-fades: pages, the player, the
  side pane, the floating player, the dock's mini player, and the bar of chosen songs fade; sheets
  come and go without a slide; the loading shimmer and the tab light stand still.
