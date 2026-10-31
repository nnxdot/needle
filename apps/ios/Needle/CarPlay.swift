import CarPlay
import UIKit

/// Needle in the car (CarPlay): Home's shelves, your playlists, and your albums, and the car's
/// own Now Playing screen, which follows the lock screen's controls. The car asks for this only
/// in builds signed with Apple's CarPlay audio entitlement.
@MainActor
final class CarPlaySceneDelegate: UIResponder, CPTemplateApplicationSceneDelegate {
    private var interface: CPInterfaceController?
    private var store: Store { Store.main }

    func templateApplicationScene(_ scene: CPTemplateApplicationScene, didConnect interface: CPInterfaceController) {
        self.interface = interface
        let home = CPListTemplate(title: "Home", sections: [])
        home.tabImage = UIImage(systemName: "house.fill")
        let playlists = CPListTemplate(title: "Playlists", sections: [])
        playlists.tabImage = UIImage(systemName: "music.note.list")
        let albums = CPListTemplate(title: "Albums", sections: [])
        albums.tabImage = UIImage(systemName: "square.stack.fill")
        interface.setRootTemplate(CPTabBarTemplate(templates: [home, playlists, albums]), animated: false, completion: nil)
        Task {
            await fillHome(home)
            await fillPlaylists(playlists)
            await fillAlbums(albums)
        }
    }

    func templateApplicationScene(_ scene: CPTemplateApplicationScene, didDisconnectInterfaceController interface: CPInterfaceController) {
        self.interface = nil
    }

    // MARK: The tabs

    /// The Mac's Home shelves, as lists.
    private func fillHome(_ template: CPListTemplate) async {
        guard let home = await store.load({ try $0.home() }) else { return }
        let favorites = await store.load { try $0.homeShelves(limit: 20).favorites } ?? []
        let room = CPListTemplate.maximumItemCount
        let shelves: [(String, [Album])] = [
            ("Jump back in", home.recent), ("New in your library", home.added),
            ("On repeat", home.mostPlayed), ("Favorites", favorites),
        ]
        let sections = shelves.filter { !$0.1.isEmpty }.map { title, albums in
            CPListSection(items: albums.prefix(min(12, room)).map(albumItem), header: title, sectionIndexTitle: nil)
        }
        template.updateSections(sections)
    }

    private func fillPlaylists(_ template: CPListTemplate) async {
        let playlists = await store.load { try $0.playlists() } ?? []
        let items = playlists.prefix(CPListTemplate.maximumItemCount).map { playlist in
            let item = CPListItem(text: playlist.name, detailText: nil, image: UIImage(systemName: "music.note.list"))
            item.accessoryType = .disclosureIndicator
            item.handler = { [weak self] _, done in
                Task { @MainActor in
                    let id = playlist.id
                    let songs = await self?.store.load { try $0.playlistSongs(id: id) } ?? []
                    self?.showSongs(songs, title: playlist.name)
                    done()
                }
            }
            return item
        }
        template.updateSections([CPListSection(items: Array(items))])
    }

    private func fillAlbums(_ template: CPListTemplate) async {
        let albums = await store.load { try $0.albums() } ?? []
        template.updateSections([CPListSection(items: albums.prefix(CPListTemplate.maximumItemCount).map(albumItem))])
    }

    // MARK: Items

    /// An album: its cover, name, and artist; chosen, its songs.
    private func albumItem(_ album: Album) -> CPListItem {
        let item = CPListItem(text: album.title, detailText: album.artist, image: nil)
        item.accessoryType = .disclosureIndicator
        if let path = album.artwork {
            Task { @MainActor in
                if let image = await Covers.load(path, size: 180) { item.setImage(image) }
            }
        }
        item.handler = { [weak self] _, done in
            Task { @MainActor in
                let key = album.key
                let songs = await self?.store.load { try $0.albumSongs(key: key) } ?? []
                self?.showSongs(songs, title: album.title)
                done()
            }
        }
        return item
    }

    /// A list of songs; chosen, one plays (with the rest after it) and Now Playing opens.
    private func showSongs(_ songs: [Song], title: String) {
        let items = songs.prefix(CPListTemplate.maximumItemCount).enumerated().map { index, song in
            let item = CPListItem(text: song.title, detailText: song.artist, image: nil)
            item.handler = { [weak self] _, done in
                self?.store.play(songs, from: index)
                self?.showPlayer()
                done()
            }
            return item
        }
        let list = CPListTemplate(title: title, sections: [CPListSection(items: Array(items))])
        interface?.pushTemplate(list, animated: true, completion: nil)
    }

    private func showPlayer() {
        guard let interface, interface.topTemplate !== CPNowPlayingTemplate.shared else { return }
        interface.pushTemplate(CPNowPlayingTemplate.shared, animated: true, completion: nil)
    }
}
