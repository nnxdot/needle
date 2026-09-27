# Needle for Android: design rules

One system on every screen. When a screen needs something new, it uses these first.

## Idea

The music is the colour. The page is quiet charcoal (or black, or light, per theme); covers carry
the colour, and the playing or opened cover tints the page around it. Amber (the theme's accent)
marks only what plays, the main action, and links.

## Structure

- **Page:** a large title that shrinks as the page scrolls (Material's flexible top bar), actions
  at its right. Every top-level page and every page opened from one has it.
- **Content sits on the page**, not in cards. Cards (rounded 28 dp surfaces) are for groups of
  settings and for the few tiles on Home.
- **Lists:** 60 dp rows; cover 52 dp (12 dp corners); title and one line under it; the row's
  menu at the right. Long press opens the same menu.
- **Grids:** two covers across on phones, a title and one line under each.
- **Library** is one page with filter pills at the top (Playlists, Albums, Artists, Songs, and
  More), as Spotify and Apple Music do, not a list of places to go.

## Shapes (each means one thing)

- Covers of albums, songs, and playlists: rounded squares, 12 dp (8 dp when small).
- Artists: circles.
- Icons in a shape: the scalloped "cookie", always the same one.
- Buttons: pills; the main Play is the one large filled button.

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

## Type

- Roboto Flex, one variable font. Text at its usual width; display and headline sizes narrow
  and heavy (`titleFamily`), as posters are. Big titles grow narrower as their page scrolls.

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
