import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

@main
struct NeedleWidgets: WidgetBundle {
    var body: some Widget {
        NowPlayingWidget()
        RecentWidget()
        PlaylistsWidget()
        RadioWidget()
        StatsWidget()
        NeedleLiveActivity()
        PlayPauseControl()
        RadioControl()
    }
}

private let pink = Color(red: 1, green: 0.361, blue: 0.541)

// MARK: - What the widgets read

struct Entry: TimelineEntry {
    let date: Date
    let snapshot: Snapshot
}

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> Entry {
        Entry(date: .now, snapshot: Snapshot(title: "Song", artist: "Artist", album: "Album"))
    }

    func getSnapshot(in context: Context, completion: @escaping (Entry) -> Void) {
        completion(Entry(date: .now, snapshot: context.isPreview ? sample : Snapshot.load()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
        // The app asks for new timelines when the song or the library changes.
        completion(Timeline(entries: [Entry(date: .now, snapshot: Snapshot.load())], policy: .never))
    }

    private var sample: Snapshot {
        Snapshot(title: "Refraction", artist: "Mira Vale", album: "Glass Hours", playing: true,
                 recent: (1...4).map { .init(id: "\($0)", title: "Album \($0)", detail: "Artist") },
                 playlists: (1...4).map { .init(id: "\($0)", title: "Playlist \($0)", detail: "20 songs") },
                 weekSeconds: 15_000, weekSongs: 210, topArtist: "Mira Vale")
    }
}

/// A cover from the shared folder, or a soft gradient when there is none.
struct Cover: View {
    let name: String?
    var corner: CGFloat = 12

    var body: some View {
        Group {
            if let url = Snapshot.coverURL(name), let image = UIImage(contentsOfFile: url.path) {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                LinearGradient(colors: [pink, .purple.opacity(0.8)], startPoint: .topLeading, endPoint: .bottomTrailing)
                    .overlay(Image(systemName: "music.note").font(.title2).foregroundStyle(.white.opacity(0.8)))
            }
        }
        .clipShape(.rect(cornerRadius: corner))
    }
}

// MARK: - Now playing

struct NowPlayingWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "NowPlaying", provider: Provider()) { entry in
            NowPlayingView(snapshot: entry.snapshot)
        }
        .configurationDisplayName("Now Playing")
        .description("The song playing, with play, pause, and skip.")
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryRectangular, .accessoryCircular, .accessoryInline])
    }
}

struct NowPlayingView: View {
    @Environment(\.widgetFamily) private var family
    let snapshot: Snapshot

    var body: some View {
        let title = snapshot.title ?? "Not Playing"
        let artist = snapshot.artist ?? "Needle"
        switch family {
        case .accessoryInline:
            Label(snapshot.title.map { "\($0) · \(artist)" } ?? "Needle", systemImage: snapshot.playing ? "waveform" : "music.note")
                .widgetURL(NeedleLink.player)
        case .accessoryCircular:
            Button(intent: TogglePlaybackIntent()) {
                Image(systemName: snapshot.playing ? "pause.fill" : "play.fill").font(.title2)
            }
            .buttonStyle(.plain)
            .containerBackground(for: .widget) { AccessoryWidgetBackground() }
        case .accessoryRectangular:
            HStack {
                VStack(alignment: .leading) {
                    Text(title).font(.headline).lineLimit(1)
                    Text(artist).font(.caption).lineLimit(1)
                }
                Spacer()
                Button(intent: TogglePlaybackIntent()) { Image(systemName: snapshot.playing ? "pause.fill" : "play.fill") }
                    .buttonStyle(.plain)
            }
            .widgetURL(NeedleLink.player)
            .containerBackground(for: .widget) {}
        case .systemMedium:
            HStack(spacing: 14) {
                Cover(name: snapshot.cover, corner: 14).frame(width: 110, height: 110)
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(.headline).lineLimit(2)
                    Text(artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                    Spacer()
                    controls
                }
            }
            .widgetURL(NeedleLink.player)
            .containerBackground(for: .widget) { backdrop }
        default:
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .top) {
                    Cover(name: snapshot.cover).frame(width: 56, height: 56)
                    Spacer()
                    Button(intent: TogglePlaybackIntent()) {
                        Image(systemName: snapshot.playing ? "pause.fill" : "play.fill").font(.title3)
                            .frame(width: 40, height: 40)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.interactive(), in: .circle)
                }
                Spacer()
                Text(title).font(.subheadline.bold()).lineLimit(1)
                Text(artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            .widgetURL(NeedleLink.player)
            .containerBackground(for: .widget) { backdrop }
        }
    }

    private var controls: some View {
        HStack(spacing: 18) {
            Button(intent: PreviousSongIntent()) { Image(systemName: "backward.fill") }
            Button(intent: TogglePlaybackIntent()) { Image(systemName: snapshot.playing ? "pause.fill" : "play.fill").font(.title2) }
            Button(intent: NextSongIntent()) { Image(systemName: "forward.fill") }
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 14).padding(.vertical, 8)
        .glassEffect(.regular.interactive(), in: .capsule)
    }

    private var backdrop: some View {
        ZStack {
            Color.black
            Cover(name: snapshot.cover, corner: 0).blur(radius: 30).opacity(0.7)
            LinearGradient(colors: [.clear, .black.opacity(0.4)], startPoint: .top, endPoint: .bottom)
        }
    }
}

// MARK: - Recently played and playlists

struct RecentWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "Recent", provider: Provider()) { entry in
            Shelf(items: entry.snapshot.recent, empty: "Albums you play show here") { PlayAlbumIntent(key: $0.id) }
        }
        .configurationDisplayName("Recently Played")
        .description("Tap an album to play it.")
        .supportedFamilies([.systemMedium, .systemLarge])
    }
}

struct PlaylistsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "Playlists", provider: Provider()) { entry in
            Shelf(items: entry.snapshot.playlists, empty: "Your playlists show here") { PlayPlaylistIntent(id: $0.id) }
        }
        .configurationDisplayName("Playlists")
        .description("Your playlists, a tap away.")
        .supportedFamilies([.systemMedium, .systemLarge])
    }
}

/// Up to four covers that play their album or playlist when tapped.
struct Shelf<I: AudioPlaybackIntent>: View {
    @Environment(\.widgetFamily) private var family
    let items: [Snapshot.Item]
    let empty: String
    let intent: (Snapshot.Item) -> I

    var body: some View {
        Group {
            if items.isEmpty {
                Text(empty).font(.subheadline).foregroundStyle(.secondary)
            } else if family == .systemLarge {
                VStack(spacing: 10) {
                    ForEach(items.prefix(4)) { item in
                        Button(intent: intent(item)) {
                            HStack(spacing: 12) {
                                Cover(name: item.cover, corner: 10).frame(width: 56, height: 56)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.title).font(.subheadline.bold()).lineLimit(1)
                                    Text(item.detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                }
                                Spacer()
                                Image(systemName: "play.fill").foregroundStyle(pink)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            } else {
                HStack(spacing: 10) {
                    ForEach(items.prefix(4)) { item in
                        Button(intent: intent(item)) {
                            VStack(alignment: .leading, spacing: 4) {
                                Cover(name: item.cover, corner: 10).aspectRatio(1, contentMode: .fit)
                                Text(item.title).font(.caption2.bold()).lineLimit(1)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .containerBackground(for: .widget) { Color(white: 0.08) }
    }
}

// MARK: - Radio and stats

struct RadioWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "Radio", provider: Provider()) { _ in
            RadioView()
        }
        .configurationDisplayName("My Radio")
        .description("One tap starts radio from what you play most.")
        .supportedFamilies([.systemSmall, .accessoryCircular])
    }
}

struct RadioView: View {
    @Environment(\.widgetFamily) private var family

    var body: some View {
        Button(intent: StartRadioIntent()) {
            if family == .accessoryCircular {
                Image(systemName: "dot.radiowaves.left.and.right").font(.title2)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Image(systemName: "dot.radiowaves.left.and.right").font(.largeTitle).foregroundStyle(.white)
                    Spacer()
                    Text("My Radio").font(.headline).foregroundStyle(.white)
                    Text("Songs like the ones you love").font(.caption).foregroundStyle(.white.opacity(0.8))
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            }
        }
        .buttonStyle(.plain)
        .containerBackground(for: .widget) {
            if family == .accessoryCircular { AccessoryWidgetBackground() }
            else { LinearGradient(colors: [pink, .purple], startPoint: .topLeading, endPoint: .bottomTrailing) }
        }
    }
}

struct StatsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "Stats", provider: Provider()) { entry in
            let s = entry.snapshot
            VStack(alignment: .leading, spacing: 4) {
                Text("THIS WEEK").font(.caption2.bold()).foregroundStyle(pink)
                Text(hours(s.weekSeconds)).font(.title.bold()).minimumScaleFactor(0.6)
                Text("\(s.weekSongs) songs").font(.caption).foregroundStyle(.secondary)
                Spacer()
                if let artist = s.topArtist {
                    Text("Most: \(artist)").font(.caption.bold()).lineLimit(2)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .widgetURL(NeedleLink.stats)
            .containerBackground(for: .widget) { Color(white: 0.08) }
        }
        .configurationDisplayName("Listening")
        .description("Your week of music.")
        .supportedFamilies([.systemSmall])
    }

    private func hours(_ seconds: Double) -> String {
        let minutes = Int(seconds / 60)
        return minutes >= 60 ? "\(minutes / 60) h \(minutes % 60) m" : "\(minutes) min"
    }
}

// MARK: - The Live Activity: the sleep timer, and a server's songs coming in

struct NeedleLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: NeedleActivity.self) { context in
            HStack(spacing: 14) {
                Image(systemName: context.attributes.kind == "sleep" ? "moon.zzz.fill" : "arrow.down.circle.fill")
                    .font(.title).foregroundStyle(pink)
                VStack(alignment: .leading, spacing: 2) {
                    Text(context.state.title).font(.headline)
                    if !context.state.detail.isEmpty { Text(context.state.detail).font(.subheadline).foregroundStyle(.secondary) }
                }
                Spacer()
                if let ends = context.state.ends {
                    Text(timerInterval: Date.now...max(ends, .now), countsDown: true)
                        .font(.title2.monospacedDigit().bold()).multilineTextAlignment(.trailing).frame(width: 90)
                } else if context.attributes.kind == "sync" {
                    ProgressView().tint(pink)
                }
            }
            .padding()
            .activityBackgroundTint(.black.opacity(0.6))
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Image(systemName: context.attributes.kind == "sleep" ? "moon.zzz.fill" : "arrow.down.circle.fill").foregroundStyle(pink).font(.title2)
                }
                DynamicIslandExpandedRegion(.center) {
                    Text(context.state.title).font(.headline).lineLimit(1)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    if let ends = context.state.ends {
                        Text(timerInterval: Date.now...max(ends, .now), countsDown: true).monospacedDigit().frame(width: 60)
                    }
                }
                DynamicIslandExpandedRegion(.bottom) {
                    if !context.state.detail.isEmpty { Text(context.state.detail).font(.caption).foregroundStyle(.secondary) }
                }
            } compactLeading: {
                Image(systemName: context.attributes.kind == "sleep" ? "moon.zzz.fill" : "arrow.down.circle.fill").foregroundStyle(pink)
            } compactTrailing: {
                if let ends = context.state.ends {
                    Text(timerInterval: Date.now...max(ends, .now), countsDown: true).monospacedDigit().frame(width: 44)
                } else {
                    ProgressView().tint(pink)
                }
            } minimal: {
                Image(systemName: context.attributes.kind == "sleep" ? "moon.zzz.fill" : "arrow.down.circle.fill").foregroundStyle(pink)
            }
        }
    }
}

// MARK: - Control Center buttons

struct PlayPauseControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "fyi.nnx.needle.playpause") {
            ControlWidgetButton(action: TogglePlaybackIntent()) {
                Label("Play or Pause", systemImage: "playpause.fill")
            }
        }
        .displayName("Needle: Play or Pause")
    }
}

struct RadioControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "fyi.nnx.needle.radio") {
            ControlWidgetButton(action: StartRadioIntent()) {
                Label("Play My Radio", systemImage: "dot.radiowaves.left.and.right")
            }
        }
        .displayName("Needle: Play My Radio")
    }
}
