# 1.6.2
Fixes for your files, saved settings, and everyday listening.
- [lyrics] **Safer file tidying.** Existing lyrics are left intact, and Needle tells you which files could not move. If Undo cannot put a file back, it stays available so you can clear the obstruction and try again.
- [eq] **Settings that stay saved.** Equalizer changes no longer erase new presets, bring back deleted ones, or revert other preferences. Tag edits also keep your measured loudness; replacing the audio clears measurements that no longer apply.
- [playlist] **More reliable CUE albums.** Different recordings no longer share an identity just because their file sizes and track timings match. Rescanning marks removed tracks as missing, and exported M3U playlists can be brought back with their CUE tracks and repeated entries in order.
- [cast] **Moving music between phone and computer.** Song matching uses the title and artist separately, and streamed songs keep their format. If the computer does not have the current song, Needle tells you before moving playback.
- On Linux, clicking the player artwork opens the big player instead of showing a window-resize cursor.
- Covers refresh when their picture changes at the same file path, without restarting Needle.
- Searches return small result sets faster in large libraries. Album and artist summaries now respect limits such as one song per artist or album.
- File tidying stays inside your selected music folders, leaving similarly named sibling folders alone.
- Unicode credit lines no longer panic during import, and unsupported DSF headers return an error before seeking.
- Incomplete speaker-discovery replies no longer crash the parser or discard other valid speakers.
- Queued listens for a disabled or signed-out service no longer hold up another listening service.

# 1.6.1
Needle gets faster and calmer, and you can lay it out your way.
- [globe] **Needle for Mac.** Needle now runs on Macs with Apple silicon (M1 or newer), with the menu bar icon, media keys, Now Playing, and your passwords in the Keychain. Get it from needle.nnx.fyi.
! **Faster everywhere.** Needle redraws only what changes, so it uses a fraction of the processor it did: pages at rest redraw 7 to 15 times a second instead of 40 to 70, and a start uses about a sixth of the work. Large covers are drawn from small copies, which saves memory and smooths scrolling.
- [panel] **Player in the top bar.** Turn it on in Settings › Appearance: the controls, the song playing, and the volume sit in the title bar, as in Apple Music, and the search field moves to the bottom.
- [menu] **Compact sidebar.** Also in Settings › Appearance: folding the sidebar (Ctrl+B) leaves a strip of page icons.
- [chevron-left] **Mouse back and forward buttons.** The side buttons of your mouse go back and forward between pages, and Alt+Right goes forward.
- [globe] **Search your server as you type (experimental).** For octo-fiesta: the Navidrome / Subsonic plugin can ask your server while you search, and songs it can fetch show above your results. Turn it on under your server in Settings › Plugins.
- Playlists in the sidebar show their picture, or a mosaic of their covers.
- The search field can be hidden in Settings › Appearance; Ctrl+F then opens the command palette.
- A music server that does not answer no longer freezes Play, Pause, and Next.
- Your music server's song list is refreshed on start only when it is over 6 hours old, a little after Needle opens.
- [mini] **Cover view in the mini player.** Point at the cover and press its button: the cover fills the mini player, with the song and the controls over it. The song is centred, as in Apple Music.
- With the player in the top bar, dragging the seek bar or the volume no longer keeps them stuck to the pointer.
- A short window keeps the sidebar's Settings, Import, and Fix my library in reach: the rest of the sidebar scrolls.
- The mini player's lyrics and queue buttons are as large as its other buttons.

# 1.5.2
A quick fix for 1.5.0.
- Needle no longer closes when a song's title or lyrics have curly quotes or Japanese or Chinese letters in a place too narrow for them (Windows).
- The seek bar is silent while you hold it, and the song goes on from where you let go: no more stutter or pops, and the bar no longer slides on by itself.
- On an album sorted by another column, the # column counts the songs in order instead of showing jumbled track numbers.
- Needle closes at once, even when your music server does not answer, so updates no longer stop at "Closing applications".

# 1.5.0
Needle comes to Linux, and your music can fill the whole screen.
! **Needle for Linux.** Needle now runs on Linux as well as Windows: Ubuntu, Fedora, Mint, and more. Everything you know is there, from lyrics and themes to plugins and your music server, with media keys, the tray icon, Discord status, and your keyring. It draws its own window and needs no GTK.
- [fullscreen] **Full screen player.** Press F11, or the full screen button in the big player. The cover drifts behind large synced lyrics, and the controls step aside while you listen. Esc brings you back.
- [lyrics] **Lyrics beside your library.** The new lyrics button in the player bar (or Ctrl+L) opens the lyrics next to your songs. They follow the song, and a click on a line jumps there.
- [playlist] **Playlists, made properly.** A new playlist window gives each one a name, a description, and a cover: a picture of your own, or its songs' covers. Smart playlists are built from menus, such as Genre is K-pop and Played not in the last 30 days, and count the songs that match as you go. Drag songs into the order you want, and add more with Add songs.
- [plugin] **More places to find lyrics.** Plugins can now find lyrics too. The new NetEase lyrics plugin finds timed lyrics for a great many songs, K-pop and J-pop above all. Turn it on in Settings › Plugins, or press Also look on NetEase where a song has none.
- Songs in Dolby Digital Plus (Dolby Atmos) now show their sample rate and channels.
- Names in other scripts, such as Korean, no longer turn into "…" in Songs.
- Error messages no longer say the same thing twice.
