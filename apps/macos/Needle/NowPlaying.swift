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
        commands.playCommand.addTarget { [weak store] _ in
            MainActor.assumeIsolated { if store?.playback?.playing != true { core.toggle() } }
            return .success
        }
        commands.pauseCommand.addTarget { [weak store] _ in
            MainActor.assumeIsolated { if store?.playback?.playing == true { core.toggle() } }
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

    func update(_ now: MacPlayback) {
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
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: now.title,
            MPMediaItemPropertyArtist: now.artist,
            MPMediaItemPropertyAlbumTitle: now.album,
            MPMediaItemPropertyPlaybackDuration: now.duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: now.position,
            MPNowPlayingInfoPropertyPlaybackRate: now.playing ? 1.0 : 0.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
        ]
        if let artwork { info[MPMediaItemPropertyArtwork] = artwork }
        // Only when something shown changed, or the time drifted.
        let old = center.nowPlayingInfo
        // Where the card's clock is now: it runs on by itself at its rate since it was set.
        let expected = (old?[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double ?? -10)
            + (old?[MPNowPlayingInfoPropertyPlaybackRate] as? Double ?? 0) * Date().timeIntervalSince(infoSet)
        let drift = abs(expected - now.position)
        let rate = old?[MPNowPlayingInfoPropertyPlaybackRate] as? Double
        if old?[MPMediaItemPropertyTitle] as? String != now.title || rate != (now.playing ? 1.0 : 0.0) || drift > 1.5
            || (old?[MPMediaItemPropertyArtwork] == nil && artwork != nil) {
            center.nowPlayingInfo = info
            infoSet = Date()
            center.playbackState = now.playing ? .playing : .paused
        }
    }
}
