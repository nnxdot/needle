# 1.8.0 [windows, linux]
Navidrome built in, moving covers, and calmer Settings.
! **Navidrome and Subsonic, built in.** Sign in to your music server in Settings › Library › Music servers, with no plugin to add. Its songs show with your own, with a small server icon beside the artist (turn it off in Settings › Appearance › Mark server songs). Long lists load faster, and Refresh everything gets every song and cover again and removes songs the server no longer has.
- [play] **Moving covers.** An animated WebP or GIF beside an album's songs (cover.webp, animated.gif), or the album's moving cover on your music server, moves in the big player and in full screen. Turn it off in Settings › Appearance › Moving covers.
- [palette] **Calmer Settings.** Every setting is a card with one short line. The look and the background are chosen by picture, and where things sit by small sketches. Sign-in forms open only when you press Connect or Sign in.
- [signal] **Music that keeps playing.** When the network sleeps, a song from your server waits and goes on from where it stopped. If the server does not answer for 5 minutes, the song pauses at its place, and Play goes on from there.
- [lyrics] **Lyrics under the player.** The line being sung shows under the player. Turn it on in Settings › Appearance.
- [fullscreen] **One bar over the big player.** The big player shares the window's title bar, and its immersive cover fills the window at any size.
- A cover changed on your server comes in with Refresh everything.
- Stars you give the playing song show at once.
- Small covers are smooth, not jagged.
- On Windows 10, the window's glass no longer spills past its edge.
- The buttons on the right of the player are larger, and the lyrics button is easier to tell apart.
- Rows on Home scroll sideways with arrows.
- Artist pictures from fanart.tv are the newest by when the photo was taken.
- Last.fm signs in with one click on Connect.
# 1.7.2 [macos]
A fix for a crash.
- Needle no longer quits when you leave an album's or artist's page while its colours are still coming in, or just as a new song's colours come in.
- An album's or artist's colours now show at once; only the window's background fades into them.
- Each crash report is sent once, not again at every start.

# 1.7.1 [windows, linux]
Artist pages made new, an immersive big player, and Last.fm in one click.
! **Artist pages, made new.** An artist's page opens on a wide photo of them that fades into the page, with their logo where fanart.tv has one, and a large Play between Shuffle, Radio, and Blend. Their latest release sits beside their top songs, over their albums. Turn on Look things up online in Settings › Artwork and lyrics for the photos and logos.
- [fullscreen] **An immersive big player.** In the big player and in full screen, the cover fills the left side of the window from top to bottom and fades into its own colour. Turn it off in Settings › Appearance › Immersive cover.
- [account] **Last.fm in one click.** Press Sign in with Last.fm in Settings › Listening services. You no longer need to make a Last.fm API account first.
- [lyrics] **Lyrics from your music server.** Songs from Navidrome and other music servers show the lyrics the server has, also with online lookups off.
- On Windows 10, the window no longer lags behind the mouse when you drag or resize it with Mica or Acrylic on.
- Artist photos are found for more artists, also by their other names (a group's name in Hangul, say).
- An A-B loop ends when another song plays, or when you start a song again.
- In Fix my library, a copy that cannot go to the Recycle Bin is kept, and the other copies are still merged.
- A library backup leaves out the key of your phone remote.
- Radio from a song that is no longer in your library says so.

# 1.7.1 [macos]
Fixes for a crash, covers, and crash reports.
- Needle no longer quits at times when its colours change while the sidebar shows.
- Covers that are not square (a tall Blu-ray cover, a wide one) fill their tile, the same size as the rest.
- With crash reports on, Needle also sends the crashes macOS writes down, not only the ones Needle catches itself. They tell what went wrong and where in the app, with no names and no files.

# 1.7.0 [macos]
Needle for Mac, made new as a Mac app, with album and artist pages after Apple Music.
! **A Mac app, made new.** Needle is now built with the Mac's own tools: Liquid Glass throughout, the system's menus, sheets, and keyboard, and VoiceOver for every button and switch.
- [albums] **Album and artist pages, after Apple Music.** The whole window takes an album's or artist's colours. Artist pages open on a wide photo of them, with their logo where fanart.tv has one, a large Play between round buttons, and Latest Release beside Top Songs.
- [mini] **The player in the title bar.** A small screen in the bar shows the song, where it is, and the times, beside the controls.
- [panel] **A side panel that floats.** Lyrics, the queue, and details open in a glass panel over the page, and close again.
- [fullscreen] **An immersive big player.** The cover fills the side of the big player, and moving covers (a cover.mp4 beside the songs) play there.
- Closing the window keeps the music playing; Quit (⌘Q) ends Needle.
- Your music folders are read when Needle starts and when you plug in a drive.
- Merging duplicates moves the extra copies to the Trash, where Put Back works.
- Space and the arrow keys no longer act inside file pickers or dialogs.
- Removing a music folder asks first.
- A theme's film grain and font go back to your own when you choose another theme.
- Many smaller fixes for lyrics timing, history, the volume, mute, and updates.

# 1.6.4 [macos]
Needle on the Mac, in Liquid Glass.
! **Liquid Glass.** On macOS 26 and newer, the sidebar, the player, and the side panel float on Liquid Glass over your music's colours, and your songs scroll beneath the player's glass. Turn it off in Settings › Appearance.
- [menu] **The Mac's own menus.** Right-click a song, an album, an artist, or a playlist for the Mac's own menu, with its icons and submenus.
- [palette] **Glass everywhere.** The command palette (⌘K), menus, search suggestions, tooltips, messages, dialogs, and the big player's controls sit on glass too.
- Reduce transparency (System Settings › Accessibility) makes the window solid.
- On macOS before 26, the same panes use the system's frosted glass.

# 1.6.3
Albums and artists get their own menu, and rating takes one click.
- [albums] **Right-click an album or artist.** On Home, Albums, Artists, an artist's page, and your listening history: play, shuffle, play next, add to the queue or a playlist, or open the album or artist.
- [star] **Rate with one click.** The Rating column has five stars to click. The empty ones show when you point at a song, and always while the list is sorted by rating.
- [plus] **More collections.** Press + beside Collections in the sidebar to add Rated, Most played, Recently played, or Never played. Press it again to take one away.
- Pressing Next or Previous while paused shows the new song at once, and the music stays paused. Next also works on the song waiting from last time.
- A file that a tag reader cannot handle is skipped, instead of stopping the whole scan.

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
