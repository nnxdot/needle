import SwiftUI
import VisionKit

/// Needle on a computer: connect with the phone remote's address or its QR code, work its
/// player, search its music and play it there or here, and move what plays from one to the
/// other.
struct ComputerView: View {
    @Environment(Store.self) private var store
    @State private var connected = false
    @State private var link = ""
    @State private var busy = false
    @State private var scanning = false
    @State private var query = ""
    @State private var found: [PcSong] = []

    var body: some View {
        Form {
            if !connected {
                connectForm
            } else {
                player
                Section {
                    Button("Play here", systemImage: "iphone") {
                        Task {
                            if let moved = await store.ask({ try $0.moveFromPc() }) {
                                store.message = moved ? "Playing here now" : "Nothing plays on the computer"
                            }
                        }
                    }
                    Button("Move to the computer", systemImage: "laptopcomputer") {
                        Task {
                            if let missing = await store.ask({ try $0.moveToPc() }) {
                                store.message = missing > 0 ? "\(count(Int(missing), "song")) are not on the computer" : "Playing on the computer now"
                            }
                        }
                    }
                } footer: {
                    Text("Moving takes the song, its place, and what is up next.")
                }
                search
                if let pc = store.pc, !pc.upNext.isEmpty {
                    Section("Up next on the computer") {
                        ForEach(Array(pc.upNext.prefix(30).enumerated()), id: \.offset) { i, song in
                            Button {
                                let (index, id) = (UInt32(i), song.id)
                                store.run { try $0.pcJump(index: index, id: id) }
                            } label: { pcRow(song) }
                        }
                    }
                }
                Section {
                    Button("Disconnect", role: .destructive) { store.core.disconnectPc(); connected = false }
                }
            }
        }
        .navigationTitle("Your computer")
        .themed()
        .sheet(isPresented: $scanning) {
            QRScanner { code in
                scanning = false
                link = code
                connect()
            }
            .ignoresSafeArea()
        }
        .task {
            connected = await store.load { $0.pcConnected() || $0.reconnectPc() } ?? false
        }
        .task(id: query) {
            guard connected, !query.trimmingCharacters(in: .whitespaces).isEmpty else { found = []; return }
            try? await Task.sleep(for: .milliseconds(300))
            let q = query
            found = await store.load { try $0.pcSearch(query: q) } ?? []
        }
    }

    private var connectForm: some View {
        Section {
            if DataScannerViewController.isSupported {
                Button("Scan the QR code", systemImage: "qrcode.viewfinder") { scanning = true }
            }
            TextField("http://192.168.1.5:47380/r/…", text: $link)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.URL)
            Button("Paste", systemImage: "doc.on.clipboard") { link = UIPasteboard.general.string ?? link }
            Button("Connect") { connect() }.disabled(busy || link.isEmpty)
        } footer: {
            Text("On the computer, turn on the phone remote in Settings › Playback. Scan its QR code, or type the address under it. Both need to be on the same network.")
        }
    }

    @ViewBuilder
    private var player: some View {
        if let pc = store.pc, let song = pc.current {
            Section(pc.playing ? "Playing on the computer" : "Paused on the computer") {
                HStack(spacing: 14) {
                    Artwork(path: song.cover, seed: song.album + song.artist, size: 200)
                        .frame(width: 64, height: 64).clipShape(.rect(cornerRadius: 10))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(song.title).font(.headline).lineLimit(1)
                        Text(song.artist).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                HStack(spacing: 28) {
                    Spacer()
                    Button { pcDo("previous") } label: { Image(systemName: "backward.fill") }
                    Button { pcDo("toggle") } label: { Image(systemName: pc.playing ? "pause.fill" : "play.fill") }
                    Button { pcDo("next") } label: { Image(systemName: "forward.fill") }
                    Spacer()
                }
                .font(.title2)
                .buttonStyle(.borderless)
                HStack {
                    Button { pcDo("shuffle") } label: { Image(systemName: "shuffle") }
                    Spacer()
                    Picker("Repeat", selection: Binding(get: { pc.repeat }, set: { mode in store.run { try $0.pcRepeat(mode: mode) } })) {
                        Text("No repeat").tag("off")
                        Text("Repeat all").tag("all")
                        Text("Repeat one").tag("one")
                    }
                    .fixedSize()
                }
                .buttonStyle(.borderless)
                ComputerVolume()
            }
            Section {
                Button("Show it in the player", systemImage: "play.rectangle") { store.showPlayer = true }
                    .disabled(!store.onComputer)
            } footer: {
                Text("While nothing plays on this iPhone, its player works the computer's.")
            }
        } else {
            Section { Text("Nothing plays on the computer.").foregroundStyle(.secondary) }
        }
    }

    private var search: some View {
        Section("The computer's music") {
            TextField("Search the computer", text: $query)
                .textInputAutocapitalization(.never)
            ForEach(Array(found.prefix(50).enumerated()), id: \.offset) { i, song in
                Menu {
                    Button("Play on the computer", systemImage: "laptopcomputer") { let id = song.id; store.run { try $0.pcPlay(ids: [id]) } }
                    Button("Play next there", systemImage: "text.line.first.and.arrowtriangle.forward") { let id = song.id; store.run { try $0.pcPlayNext(ids: [id]) } }
                    Button("Add to the queue there", systemImage: "text.line.last.and.arrowtriangle.forward") { let id = song.id; store.run { try $0.pcEnqueue(ids: [id]) } }
                    Divider()
                    Button("Play here", systemImage: "iphone") {
                        let songs = Array(found.prefix(50))
                        store.core.playFromPc(songs: songs, start: UInt32(i), at: 0)
                    }
                } label: { pcRow(song) }
            }
        }
    }

    private func pcRow(_ song: PcSong) -> some View {
        HStack(spacing: 12) {
            Artwork(path: song.cover, seed: song.album + song.artist, size: 120)
                .frame(width: 40, height: 40).clipShape(.rect(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 1) {
                Text(song.title).foregroundStyle(.primary).lineLimit(1)
                Text(song.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }

    private func connect() {
        busy = true
        let address = link.trimmingCharacters(in: .whitespacesAndNewlines)
        Task {
            let result = await store.ask { try $0.connectPc(link: address) }
            busy = false
            if result != nil {
                connected = true
                store.message = "Connected to Needle on your computer"
            }
        }
    }

    private func pcDo(_ action: String) {
        store.run { try $0.pcDo(action: action) }
    }
}

/// The camera, reading the phone remote's QR code.
struct QRScanner: UIViewControllerRepresentable {
    let found: (String) -> Void

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let scanner = DataScannerViewController(recognizedDataTypes: [.barcode(symbologies: [.qr])],
                                                qualityLevel: .balanced, isHighlightingEnabled: true)
        scanner.delegate = context.coordinator
        try? scanner.startScanning()
        return scanner
    }

    func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(found: found) }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let found: (String) -> Void
        private var done = false

        init(found: @escaping (String) -> Void) { self.found = found }

        func dataScanner(_ scanner: DataScannerViewController, didAdd items: [RecognizedItem], allItems: [RecognizedItem]) {
            guard !done else { return }
            for case .barcode(let code) in items {
                if let text = code.payloadStringValue, text.contains("/r/") {
                    done = true
                    scanner.stopScanning()
                    found(text)
                    return
                }
            }
        }
    }
}
