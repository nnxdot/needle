# Pictures for the website

Real screenshots of Needle for needle.nnx.fyi, from a made-up library: the site's own made-up
albums (website/site/src/lib/albums.js; the covers are free Unsplash photos), as silent 24-bit
FLAC files with tags, made-up plays, and drawn made-up artist pictures. No one's own music,
names, or folders.

1. `python library.py <music> <repo>/website/site/public/img/covers`: 12 albums, 38 songs.
2. Give a new data folder an empty `secrets.json` (`{}`), so the test copy never touches the
   system's password store. Then scan the music folder into it (`needle --data-dir <data> scan
   <music>`, or the app's Add folder).
3. `python history.py <data>/library.db`: 795 plays over the past year, mostly late at night,
   favourites, and play counts.
4. `python artists.py <data>`: a drawn picture for each made-up artist (needs `blake3`).
5. For the Sakura look, copy `sakura.toml` into `<data>/themes/`.
6. Optional: playlists `Late drives` (Oda Lind, Pixel Choir, Sol Arden) and `Sunday coffee`
   (Hana Mori, Juniper Bay, Fern & Field).

Windows and Linux test copies take these switches (crates/needle/src/ui/bench.rs):
`NEEDLE_SHOW_PAGE` (home, albums, artists, history, year, server, songs, settings:N),
`NEEDLE_SHOW_PANEL` (details, queue, lyrics), `NEEDLE_SHOW_PLAY` (a rule, such as
`album = "Harbor Lights"`) with `NEEDLE_SHOW_SEEK` (seconds), and `NEEDLE_SHOW_BIG`.
