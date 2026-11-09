import AppKit
import MediaPlayer

/// The media keys, the Now Playing card in Control Center, and AirPods taps: macOS's own
/// controls for whatever plays.
@MainActor
final class NowPlaying {
    private var shownSong: String?
    /// When the card was last written.
    private var infoSet = Date()
    private var artwork: MPMediaItemArtwork?

    func start(_ store: Store) {
        let commands = MPRemoteCommandCenter.shared()
        let core = store.core
        // macOS may call these on another thread: the store is read on the main one.
        commands.playCommand.addTarget { [weak store] _ in
            Task { @MainActor in if store?.playback?.playing != true { core.toggle() } }
            return .success
        }
        commands.pauseCommand.addTarget { [weak store] _ in
            Task { @MainActor in if store?.playback?.playing == true { core.toggle() } }
            return .success
        }
        commands.togglePlayPauseCommand.addTarget { _ in core.toggle(); return .success }
        commands.nextTrackCommand.addTarget { _ in core.next(); return .success }
        commands.previousTrackCommand.addTarget { _ in core.previous(); return .success }
        commands.changePlaybackPositionCommand.addTarget { event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            core.seek(seconds: event.positionTime)
            return .success
        }
        for command in [commands.playCommand, commands.pauseCommand, commands.togglePlayPauseCommand,
                        commands.nextTrackCommand, commands.previousTrackCommand, commands.changePlaybackPositionCommand] {
            command.isEnabled = true
        }
        commands.skipForwardCommand.isEnabled = false
        commands.skipBackwardCommand.isEnabled = false
    }

    /// `speed`: how fast the music plays (Speed and pitch); the card's clock runs at it.
    func update(_ now: MacPlayback, speed: Double = 1) {
        let center = MPNowPlayingInfoCenter.default()
        guard let id = now.current else {
            if shownSong != nil {
                center.nowPlayingInfo = nil
                center.playbackState = .stopped
                shownSong = nil
            }
            return
        }
        let shown = id + "\u{1}" + (now.artwork ?? "")
        if shown != shownSong {
            shownSong = shown
            artwork = nil
            if let path = now.artwork, let image = NSImage(contentsOfFile: path) {
                artwork = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
            }
        }
        // The song's own seconds go by at the speed it plays (1.5× plays 1.5 of them a second).
        let rate = now.playing ? max(0.25, speed) : 0
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: now.title,
            MPMediaItemPropertyArtist: now.artist,
            MPMediaItemPropertyAlbumTitle: now.album,
            MPMediaItemPropertyPlaybackDuration: now.duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: now.position,
            MPNowPlayingInfoPropertyPlaybackRate: rate,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
        ]
        if let artwork { info[MPMediaItemPropertyArtwork] = artwork }
        // Only when something shown changed, or the time drifted.
        let old = center.nowPlayingInfo
        // Where the card's clock is now: it runs on by itself at its rate since it was set.
        let expected = (old?[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double ?? -10)
            + (old?[MPNowPlayingInfoPropertyPlaybackRate] as? Double ?? 0) * Date().timeIntervalSince(infoSet)
        let drift = abs(expected - now.position)
        let shownRate = old?[MPNowPlayingInfoPropertyPlaybackRate] as? Double
        if old?[MPMediaItemPropertyTitle] as? String != now.title || shownRate.map({ abs($0 - rate) > 0.001 }) ?? true || drift > 1.5
            || (old?[MPMediaItemPropertyArtwork] == nil && artwork != nil) {
            center.nowPlayingInfo = info
            infoSet = Date()
            center.playbackState = now.playing ? .playing : .paused
        }
    }
}
