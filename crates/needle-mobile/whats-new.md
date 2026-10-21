# 1.7.1
Artist pages with their picture and logo, an immersive player, and fixes.
! **New artist pages.** An artist's page opens with a wide picture of them and their logo (with online lookups on, from fanart.tv), a large Play between Shuffle and Radio, their latest release in a box of its own, and their top songs as one group.
- [player] **Immersive cover.** Turn on Settings › Appearance › Immersive cover and the player's cover fills its top (on a tablet or a TV, its left side) and fades into the cover's colour.
- [video] **Moving covers.** Put a short video named cover.mp4 beside an album's songs, and it plays, silent and looping, in the immersive player. Turn it off with Moving covers.
- [album] **Covers beside your songs.** Album covers kept as a picture beside the songs (such as cover.jpg) now show. On Android 13 and newer, Needle asks once to see pictures and videos: tap Allow in Settings › Library, or on an album with no cover.
- Messages in the player now show at the top, not over its buttons, go after 2.5 seconds, and go at once when you tap or swipe them.
- No wide gap above an artist's top songs.
- Album and playlist titles fit on narrow phones.
- You can add a fanart.tv key of your own in Settings › Library. Needle has its own key, so this is not needed.
- On a TV, an artist's picture and logo stay in view while the remote is on the page's buttons.

# 1.7.0
A new look, tablets and TVs, louder music, and a smoother app.
! **A fresh Material look.** Needle now uses Material 3 Expressive throughout: songs, playlists, and settings sit in rounded groups, Shuffle and Play are one joined button group, Home shows your playlists and artists, and the filter buttons, tabs, and colours in Night and Day are clearer.
- [sound] **Full volume.** Needle played about 4 dB quieter than other players, because it kept its own volume at 65% under the phone's. It now plays at full level, and your phone's volume buttons set how loud it is.
- [panel] **Made for tablets.** On a tablet or an unfolded phone, a side bar holds your tabs, playlists, and recent albums, with pages in a rounded pane beside it. Phones turned sideways and split screens get layouts that fit.
- [fullscreen] **Android TV.** Needle is in your TV's apps and works with a remote: a ring shows where you are, the arrows move the seek bar ten seconds, and the search box opens the keyboard. A 32-bit version is there for older TV boxes.
- [player] **Smoother playing.** The player, the moving cover behind it, lyrics, and song lists do much less work while music plays, so scrolling and animations stutter less.
- [car] **Better in the car.** A song picked in an album, a playlist, or Favorites plays on into the rest of it, and album covers kept beside your songs show in Android Auto.
- Needle pauses for calls and other apps and plays again after, does not play over a call, and pauses when headphones come out.
- Presses from a headset, the notification, a car, or a widget show at once.
- Back brings you to the page as you left it, with its scroll, filter, and search.
- Changing a setting no longer makes a short break in the music.
- Next works from the notification and Bluetooth on the last song and with Repeat One.
- Bit-perfect USB follows your DAC as you plug it in or out, wherever you are in the app.
- Many buttons now do their job once when tapped twice: saving a playlist, adding to the queue, saving Wrapped, and more.
- TalkBack reads headings, the song playing, the seek bar's time, and the names of more buttons; text fits better at large sizes.
- Send crash reports now stays off when you turn it off.
- Fixed crashes on devices with no browser or file picker, in the widgets, from a car's queue buttons, and while reading folders on Android 16.

# 1.6.5
Your music on your Discord profile.
- [discord] **Discord status.** With the Discord app on your phone, your Discord profile shows what you're playing: the song, the artist, the album, the cover, and a time bar, as on the computer. There is nothing to sign in to. Turn it off, or choose what the card shows, in Settings › Discord.
- Later updates for the phone reach it by themselves, even when the computer versions stay the same.

# 1.6.3
Swipe through songs, and fixes from your reports.
- [player] **Swipe the cover.** In the player, swipe the cover left for the next song or right for the one before. Turn it off in Settings › Appearance.
- [computer] **Your computer on the lock screen.** While your phone shows Needle on your computer, the lock screen and the notification show its song, and their buttons control it.
- [sound] **Bit-perfect USB that plays.** Needle now sends the DAC music in the format its bit-perfect mode asks for, and an unusual answer from Android no longer closes the app.
- Reading your music can no longer get stuck on "Reading your music" for hours. The notice shows the folder being read and goes away if Needle closes.
- Pressing Next or Previous while paused shows the new song at once, and the music stays paused.
- The heart sits beside the song's name in the player.
- Messages show above the bottom bars, not over them.
- The welcome guide's text is readable on dark themes.
- Hold a cover at the top of Home to open the album's menu.

# 1.6.2
Safer imports, saved settings that stick, and fixes across your library.
- [playlist] **Keep every song.** Opening two files with the same name now keeps separate, complete copies. Playlist edits affect the song you selected, even with missing songs or repeated entries in the list.
- [sound] **Settings that stay saved.** Equalizer changes no longer erase new presets, restore deleted ones, or revert other preferences. Tag edits keep measured loudness; replacing the audio clears measurements that no longer apply.
- [palette] **Reliable custom themes.** Creating or importing a theme keeps existing themes intact. Themes from different plugins stay separate, plugin themes are edited as copies, and saving the theme in use refreshes its colours.
- [computer] **Moving music between phone and computer.** Song matching uses title and artist separately, streamed songs keep their format, and a missing current song is reported before playback moves.
- [car] **Faster library searches.** Search and Android Auto browsing load the requested results instead of building the whole song list first. Media searches can also start playback.
- Failed page loads show an error and Retry instead of staying on a loading screen. Failed or cancelled operations release their busy state so you can try again.
- File tidying protects existing lyrics, reports every file that could not move, and keeps failed Undo steps available for retry. Similarly named folders outside your music roots are left alone.
- CUE recordings have stronger identities for sync and duplicate checks. Rescans mark removed tracks as missing, and M3U playlists preserve CUE tracks and repeated entries.
- Editing app-owned copies does not need shared-storage access. Android 8 and 9 request write permission before changing shared music files; Android 10 explains its storage restrictions.
- USB mixer controls only change their displayed state after Android accepts the request, report failures, and follow connected-device changes.
- Picking files copies them in the background, keeps incomplete copies out of the library, and cleans up cancelled copies.
- Playing-song indicators now follow playback in favorites, recent songs, and folders.
- Album and artist summaries respect per-group query limits. A disabled or signed-out listening service no longer blocks another service's queued listens.
- Unicode credit lines, unsupported DSF headers, and incomplete speaker-discovery replies are handled without the reported panics.

# 1.6.1
Needle comes to your phone.
! **Needle for Android.** Your music, on your phone, with the same core as Needle on your computer: your folders, lyrics that follow the song, stars and favorites, playlists and smart playlists, radio, your listening history, and Your year. No account, no ads, no tracking.
- [palette] **In your music's colours.** Album, playlist, and artist pages take the colours of their cover, with the cover blurred and drifting behind them. Turn on Colours from what's playing in Settings › Appearance and the whole app follows the song.
- [player] **A player to make your own.** Choose the seek bar (a wave, a line, or a thick bar) and the cover (a rounded square, a turning record, or a Material shape). The play button changes shape as it plays.
- [computer] **Your computer, from your phone.** Scan the QR code in Needle on your computer to play, pause, skip, and search it; while it plays, your phone shows it in its own player.
- [widget] **Widgets.** Now playing, on the song's own blurred cover, and your recently added albums, one tap from playing.
- [car] **Android Auto.** Your albums, playlists, and favorites in the car.
- [plugin] **Plugins.** Your Navidrome or Subsonic server, NetEase lyrics, and effects, as on your computer.
- [sound] **Sound.** A ten-band equalizer, crossfeed, mono, balance, crossfade, even loudness, Dolby Atmos music as stereo, and bit-perfect play to USB DACs.
- [sync] **Sync.** Move your history, ratings, and playlists between your phone and your computer as one locked file.
