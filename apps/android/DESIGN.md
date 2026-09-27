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

- Album, playlist, and artist pages: the cover's colour fills the top and fades into the page,
  as Apple Music does.
- The player: the blurred cover behind it.
- Everything else: the theme's own surfaces.

## Menus

- A song's menu is a sheet from the bottom: the song at the top with its stars, then its actions
  in rounded groups with gaps between them (Material 3 Expressive menus).
- Albums and playlists have the same kind of sheet: a long press on a cover, or More at the top
  right of their page.
- Small choices (sort, view) are Expressive dropdown menus.

## Motion

- Pages slide a little and fade; covers move between the grid, the page, and the player.
- Presses shrink a little and spring back. Nothing bounces for show.
- "Reduce motion" (or Android's own setting) turns it into cross-fades.
