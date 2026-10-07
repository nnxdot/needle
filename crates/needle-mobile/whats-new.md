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
