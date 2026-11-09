//! Playlists that live on a music server (made in Navidrome's web page, or any Subsonic
//! server), kept in step with Needle's playlists both ways.
//!
//! Each server playlist is a Needle playlist with a [`ServerPlaylist`] link: the source that
//! brings it, its id on the server, and its name and songs as they were at the last sync. A
//! sync compares both sides with that last state:
//!
//! - changed only on the server: Needle takes the server's version;
//! - changed only in Needle (renamed, songs added, removed, or moved): the change goes to the
//!   server;
//! - changed on both sides in different ways, or the server refuses the change: the server's
//!   version stays as the server playlist, and Needle's version is kept as a playlist of the
//!   person's own ("… (Needle copy)"). Nothing is lost on either side;
//! - deleted on the server: it leaves Needle, unless it was changed in Needle and not sent
//!   yet: then it stays as a playlist of the person's own;
//! - deleted in Needle: it is deleted on the server, unless the server's copy changed since
//!   the last sync: then it comes back.
//!
//! Signing out of the server takes its playlists out of Needle (not off the server).
//! Needle's own playlists are never touched.
use crate::{
    database::Library,
    model::{Playlist, ServerPlaylist},
    sources::{self, Song},
};
use anyhow::Result;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

/// A playlist as the server has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Remote {
    pub id: String,
    pub name: String,
    pub comment: String,
    /// Its songs, in order.
    pub songs: Vec<Song>,
}

/// What a server that keeps playlists can do.
pub trait Server {
    /// The ids of the playlists the person can see on the server.
    fn playlist_ids(&self) -> Result<Vec<String>, String>;
    /// One playlist with its songs.
    fn playlist(&self, id: &str) -> Result<Remote, String>;
    /// Give a playlist this name and exactly these songs (server ids, in order).
    fn save_playlist(&self, id: &str, name: &str, songs: &[String]) -> Result<(), String>;
    fn delete_playlist(&self, id: &str) -> Result<(), String>;
}

/// What a sync changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// New in Needle.
    pub added: usize,
    /// Taken from the server.
    pub updated: usize,
    /// Changes made in Needle that went to the server (deletions too).
    pub sent: usize,
    /// Gone from Needle, as gone from the server.
    pub removed: usize,
    /// What the person should know: copies kept, changes the server refused.
    pub notes: Vec<String>,
    /// Songs added to the library for a playlist, still without a cover (see
    /// `sources::Synced::covers`).
    pub covers: Vec<(String, String, String)>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.added + self.updated + self.sent + self.removed > 0 || !self.notes.is_empty()
    }
}

/// One sync at a time: a sync after an edit and an "Update now" must not both write.
static SYNCING: Mutex<()> = Mutex::new(());

/// Playlist writes that read first (an app's save or delete, a sync's results) happen one at a
/// time, so none writes over another with what it read before. Never held while the server is
/// asked anything.
static WRITING: Mutex<()> = Mutex::new(());

pub(crate) fn writing() -> MutexGuard<'static, ()> {
    WRITING.lock().unwrap_or_else(|e| e.into_inner())
}

type Hook = Arc<dyn Fn(&str) + Send + Sync>;
static EDITED: RwLock<Option<Hook>> = RwLock::new(None);

/// Called with a source's id when one of its playlists is changed or deleted in Needle, so the
/// change can go to the server (the plugin host sets this).
pub fn set_edit_hook(hook: impl Fn(&str) + Send + Sync + 'static) {
    *EDITED.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(hook));
}

pub(crate) fn edited(plugin: &str) {
    let hook = EDITED.read().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(hook) = hook {
        hook(plugin);
    }
}

/// The Needle id of a server playlist: the same at every sync, and different for each source.
pub fn playlist_id(plugin: &str, id: &str) -> String {
    let hash = blake3::hash(format!("{plugin}\0{id}").as_bytes()).to_hex();
    format!("srv-{}", &hash[..32])
}

/// Where the server playlists deleted in Needle wait to be deleted on the server.
fn deleted_key(plugin: &str) -> String {
    format!("server-playlists-deleted:{plugin}")
}

/// Remember that a server playlist was deleted in Needle (see [`Library::delete_playlist`]).
/// The caller holds [`writing`].
pub(crate) fn remember_deleted(library: &Library, link: &ServerPlaylist) -> Result<()> {
    let key = deleted_key(&link.plugin);
    let mut deleted: Vec<ServerPlaylist> = library.get_json(&key)?.unwrap_or_default();
    deleted.retain(|d| d.id != link.id);
    deleted.push(link.clone());
    library.set_json(&key, &deleted)
}

/// Bring a source's playlists in step with the server's. With `only_edited`, only the
/// changes made in Needle are sent (and playlists gone from the server leave); the rest waits
/// for the next full sync. When the server cannot list its playlists, nothing changes.
pub fn sync(
    library: &Library,
    plugin: &str,
    server: &dyn Server,
    only_edited: bool,
) -> Result<Report> {
    let _one = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let mut report = Report::default();
    let listed = server.playlist_ids().map_err(anyhow::Error::msg)?;
    let on_server: HashSet<&String> = listed.iter().collect();

    // Deleted in Needle: deleted on the server too, when the server's copy is as it was (its
    // name, songs, and comment). One that changed there, or that the server would not delete,
    // comes back at once.
    let key = deleted_key(plugin);
    let deleted: Vec<ServerPlaylist> = {
        let _write = writing();
        library.get_json(&key)?.unwrap_or_default()
    };
    let (mut settled, mut gone, mut back) = (HashSet::new(), HashSet::new(), HashSet::new());
    for link in &deleted {
        if !on_server.contains(&link.id) {
            settled.insert(link.id.clone());
            continue;
        }
        // Not reached: tried again at the next sync.
        let Ok(remote) = server.playlist(&link.id) else {
            continue;
        };
        settled.insert(link.id.clone());
        let unchanged = shown_name(&remote.name) == link.name
            && song_ids(plugin, &remote) == link.songs
            && remote.comment == link.comment;
        if !unchanged {
            report.notes.push(format!(
                "“{}” changed on the server after you deleted it, so it is back",
                link.name
            ));
            back.insert(link.id.clone());
            continue;
        }
        match server.delete_playlist(&link.id) {
            Ok(()) => {
                report.sent += 1;
                gone.insert(link.id.clone());
            }
            Err(error) => {
                report.notes.push(format!(
                    "“{}” could not be deleted on the server ({error}), so it is back",
                    link.name
                ));
                back.insert(link.id.clone());
            }
        }
    }
    // Only the ones handled leave the list: a playlist deleted meanwhile waits for the next.
    let waiting: HashSet<String> = {
        let _write = writing();
        let mut now: Vec<ServerPlaylist> = library.get_json(&key)?.unwrap_or_default();
        now.retain(|link| !(settled.contains(&link.id) && deleted.contains(link)));
        library.set_json(&key, &now)?;
        now.into_iter().map(|link| link.id).collect()
    };

    let mut ours: HashMap<String, Playlist> = HashMap::new();
    for playlist in library.playlists()? {
        if let Some(link) = &playlist.server
            && link.plugin == plugin
        {
            ours.insert(link.id.clone(), playlist);
        }
    }

    for id in &listed {
        if gone.contains(id) || waiting.contains(id) {
            continue;
        }
        let local = ours.get(id);
        if only_edited && !back.contains(id) && !local.is_some_and(Playlist::edited_here) {
            continue;
        }
        let remote = match server.playlist(id) {
            Ok(remote) => remote,
            Err(error) => {
                if local.is_some_and(Playlist::edited_here) {
                    report.notes.push(format!(
                        "Your changes to “{}” wait for the server: {error}",
                        local.map(|l| l.name.as_str()).unwrap_or_default()
                    ));
                }
                continue;
            }
        };
        let theirs = add_songs(library, plugin, &remote.songs, &mut report)?;
        match local {
            None => {
                let playlist = Playlist {
                    id: playlist_id(plugin, id),
                    name: shown_name(&remote.name),
                    description: remote.comment.clone(),
                    track_ids: theirs.clone(),
                    updated_at: chrono::Utc::now().timestamp(),
                    server: Some(link(plugin, &remote, theirs)),
                    ..Default::default()
                };
                library.store_playlist(&playlist)?;
                report.added += 1;
            }
            Some(local) => sync_one(library, plugin, server, local, &remote, theirs, &mut report)?,
        }
    }

    // Gone from the server.
    for (id, playlist) in ours {
        if on_server.contains(&id) || gone.contains(&id) {
            continue;
        }
        let _write = writing();
        // As it is now: the person may have changed it while the server was asked.
        let Some(now) = library.playlist_by_id(&playlist.id)? else {
            continue;
        };
        if now.edited_here() {
            report.notes.push(format!(
                "“{}” was deleted on the server; your changes are kept as your own playlist",
                now.name
            ));
            detach(library, now)?;
        } else {
            library.drop_playlist(&now.id)?;
            report.removed += 1;
        }
    }
    Ok(report)
}

/// Make a server playlist the person's own: no link, and an id of its own. The server
/// playlist's id is kept for the server's own copy, which a later sync (after signing in
/// again) would write into, over the person's changes.
fn detach(library: &Library, mut playlist: Playlist) -> Result<()> {
    let old = std::mem::replace(&mut playlist.id, uuid::Uuid::new_v4().to_string());
    playlist.server = None;
    library.store_playlist(&playlist)?;
    library.drop_playlist(&old)
}

/// One playlist that is both in Needle and on the server.
fn sync_one(
    library: &Library,
    plugin: &str,
    server: &dyn Server,
    local: &Playlist,
    remote: &Remote,
    theirs: Vec<String>,
    report: &mut Report,
) -> Result<()> {
    let base = local.server.clone().unwrap_or_default();
    let here = local.edited_here();
    let there = shown_name(&remote.name) != base.name || theirs != base.songs;
    if here && !there {
        // Songs from elsewhere (files on this computer, another server) cannot be on the
        // server's playlist: the whole list is kept as the person's own as well. Songs no
        // longer in the library at all just go.
        let (mut mine, mut ids, mut others) = (vec![], vec![], false);
        for id in &local.track_ids {
            if let Some(track) = library.track(id)? {
                match track.source() {
                    Some((owner, song)) if owner == plugin => {
                        mine.push(id.clone());
                        ids.push(song);
                    }
                    _ => others = true,
                }
            }
        }
        match server.save_playlist(&remote.id, &local.name, &ids) {
            Ok(()) => {
                if others {
                    keep_copy(library, local, report, "songs that are not on the server")?;
                }
                let mut sent = local.clone();
                sent.track_ids = mine;
                sent.server = Some(ServerPlaylist {
                    name: sent.name.clone(),
                    songs: sent.track_ids.clone(),
                    ..base
                });
                commit(library, local, sent, Pushed, report)?;
                report.sent += 1;
            }
            Err(error) => {
                let why = format!("the server did not take the changes: {error}");
                let version = server_version(plugin, local, remote, theirs);
                commit(library, local, version, Pulled(Some(&why)), report)?;
                report.updated += 1;
            }
        }
    } else if here {
        let same = local.name == shown_name(&remote.name) && local.track_ids == theirs;
        let why = (!same).then_some("it changed on the server too");
        let version = server_version(plugin, local, remote, theirs);
        commit(library, local, version, Pulled(why), report)?;
        report.updated += 1;
    } else if there || remote.comment != base.comment {
        let version = server_version(plugin, local, remote, theirs);
        commit(library, local, version, Pulled(None), report)?;
        report.updated += 1;
    }
    Ok(())
}

/// The server's version of a playlist in Needle. Its own description (when changed in
/// Needle) and picture stay.
fn server_version(
    plugin: &str,
    local: &Playlist,
    remote: &Remote,
    theirs: Vec<String>,
) -> Playlist {
    let base = local.server.clone().unwrap_or_default();
    let mut playlist = local.clone();
    if playlist.description == base.comment {
        playlist.description = remote.comment.clone();
    }
    playlist.name = shown_name(&remote.name);
    playlist.track_ids = theirs.clone();
    playlist.updated_at = chrono::Utc::now().timestamp();
    playlist.server = Some(link(plugin, remote, theirs));
    playlist
}

/// How a sync's result for one playlist came about.
#[derive(Clone, Copy)]
enum Outcome<'a> {
    /// Needle's version went to the server.
    Pushed,
    /// The server's version was taken; with why, when Needle's version is to be kept as a
    /// copy (it changed on both sides, or the server refused it).
    Pulled(Option<&'a str>),
}
use Outcome::{Pulled, Pushed};

/// Write what a sync made of `local` (the playlist as read before the server was asked).
///
/// The person may have changed it meanwhile; their newer version is never written over:
/// - after a push, it stays, on the new last-synced state, and goes to the server next;
/// - after taking the server's version, it is kept as a copy of their own (one copy, of the
///   newest version, also when a copy was to be kept anyway).
///
/// Deleted meanwhile: after a push, the deletion waits with the new last-synced state (the
/// server's copy is now as Needle sent it). After taking the server's version it keeps the
/// state it was deleted from, so a server playlist changed since comes back, not deleted.
fn commit(
    library: &Library,
    local: &Playlist,
    mut result: Playlist,
    outcome: Outcome,
    report: &mut Report,
) -> Result<()> {
    let link = result.server.clone().unwrap_or_default();
    let again = {
        let _write = writing();
        match library.playlist_by_id(&local.id)? {
            None => {
                if matches!(outcome, Pushed) {
                    let key = deleted_key(&link.plugin);
                    let mut deleted: Vec<ServerPlaylist> =
                        library.get_json(&key)?.unwrap_or_default();
                    for waiting in deleted.iter_mut().filter(|d| d.id == link.id) {
                        *waiting = link.clone();
                    }
                    library.set_json(&key, &deleted)?;
                }
                false
            }
            Some(now) => {
                let changed = now.name != local.name || now.track_ids != local.track_ids;
                match outcome {
                    Pushed if changed => {
                        let mut newer = now;
                        newer.server = Some(link.clone());
                        library.store_playlist(&newer)?;
                        true
                    }
                    _ => {
                        let why = match outcome {
                            Pulled(Some(why)) => Some(why),
                            Pulled(None) if changed => {
                                Some("it changed on the server while you edited it")
                            }
                            _ => None,
                        };
                        if let Some(why) = why {
                            keep_copy(library, &now, report, why)?;
                        }
                        // Its description and picture may have changed meanwhile: those stay.
                        if now.description != local.description {
                            result.description = now.description;
                        }
                        result.cover = now.cover;
                        library.store_playlist(&result)?;
                        false
                    }
                }
            }
        }
    };
    if again {
        edited(&link.plugin);
    }
    Ok(())
}

/// A save from an app that read the playlist before a sync moved it on (`seen` is the
/// last-synced state the app read, `stored` the playlist as it is now). Name and songs are
/// taken each on its own: what only the sync changed stays as the sync left it, what only the
/// person changed is theirs, and when both changed the same one, the sync's stays and the
/// person's whole version is kept as a copy (returned). The server part is the stored one.
pub(crate) fn merge_save(
    saved: &Playlist,
    seen: &ServerPlaylist,
    stored: &Playlist,
) -> (Playlist, Option<Playlist>) {
    let now = stored.server.clone().unwrap_or_default();
    let mut merged = saved.clone();
    merged.server = stored.server.clone();
    let mut clash = false;
    if saved.name != stored.name && now.name != seen.name {
        if saved.name != seen.name {
            clash = true;
        }
        merged.name = stored.name.clone();
    }
    if saved.track_ids != stored.track_ids && now.songs != seen.songs {
        if saved.track_ids != seen.songs {
            clash = true;
        }
        merged.track_ids = stored.track_ids.clone();
    }
    (merged, clash.then(|| copy_of(saved)))
}

/// Needle's version of a server playlist, as a playlist of the person's own.
fn copy_of(local: &Playlist) -> Playlist {
    Playlist {
        id: uuid::Uuid::new_v4().to_string(),
        name: format!("{} (Needle copy)", local.name),
        query: None,
        track_ids: local.track_ids.clone(),
        updated_at: chrono::Utc::now().timestamp(),
        description: local.description.clone(),
        // The picture is a file a playlist owns: deleting one would take it from the other.
        cover: None,
        rules: None,
        server: None,
    }
}

/// Keep Needle's version of a server playlist as a playlist of the person's own.
fn keep_copy(library: &Library, local: &Playlist, report: &mut Report, why: &str) -> Result<()> {
    let copy = copy_of(local);
    library.store_playlist(&copy)?;
    report.notes.push(format!(
        "Your version of “{}” is kept as “{}”: {why}",
        local.name, copy.name
    ));
    Ok(())
}

fn link(plugin: &str, remote: &Remote, songs: Vec<String>) -> ServerPlaylist {
    ServerPlaylist {
        plugin: plugin.to_string(),
        id: remote.id.clone(),
        name: shown_name(&remote.name),
        songs,
        comment: remote.comment.clone(),
    }
}

/// A playlist needs a name in Needle; a server can have one without.
fn shown_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "Untitled playlist".into()
    } else {
        name.into()
    }
}

/// The library ids of a server playlist's songs, in order.
fn song_ids(plugin: &str, remote: &Remote) -> Vec<String> {
    remote
        .songs
        .iter()
        .map(|s| sources::track_id(plugin, &s.id))
        .collect()
}

/// The library ids of a server playlist's songs; songs not in the library yet (the server
/// did not list them in a sync) are added from what the playlist says about them, and ones
/// marked missing are not missing.
fn add_songs(
    library: &Library,
    plugin: &str,
    songs: &[Song],
    report: &mut Report,
) -> Result<Vec<String>> {
    let now = chrono::Utc::now().timestamp();
    let db = library.connection()?;
    let mut ids = Vec::with_capacity(songs.len());
    for song in songs {
        let track = sources::to_track(plugin, song, now);
        let known = library.track(&track.id)?;
        if known.as_ref().is_some_and(|t| t.missing) {
            // The server's list of songs left it out, but its playlist has it: it is there.
            db.execute("UPDATE tracks SET missing=0 WHERE id=?", [&track.id])?;
        }
        if known.is_none() {
            Library::upsert_on(&db, &track)?;
            if !song.cover.is_empty() {
                let key = if song.cover_id.is_empty() {
                    song.cover.clone()
                } else {
                    song.cover_id.clone()
                };
                report
                    .covers
                    .push((track.id.clone(), song.cover.clone(), key));
            }
        }
        ids.push(track.id);
    }
    Ok(ids)
}

/// After signing out: the source's playlists leave Needle (they stay on the server). One
/// changed in Needle and not sent yet stays, as the person's own. Returns how many left.
pub fn forget(library: &Library, plugin: &str) -> Result<usize> {
    let _one = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let _write = writing();
    let mut count = 0;
    for playlist in library.playlists()? {
        if playlist.server_plugin() != Some(plugin) {
            continue;
        }
        if playlist.edited_here() {
            detach(library, playlist)?;
        } else {
            library.drop_playlist(&playlist.id)?;
            count += 1;
        }
    }
    library.set_json(&deleted_key(plugin), &Vec::<ServerPlaylist>::new())?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// What happens in Needle while the server is asked (`"playlist"` or `"save"`, and the id).
    type During = Box<dyn FnMut(&str, &str)>;

    /// A server in memory.
    #[derive(Default)]
    struct Fake {
        lists: RefCell<Vec<Remote>>,
        refuse: bool,
        down: bool,
        saved: RefCell<usize>,
        during: RefCell<Option<During>>,
    }

    impl Fake {
        fn meanwhile(&self, what: &str, id: &str) {
            if let Some(during) = self.during.borrow_mut().as_mut() {
                during(what, id);
            }
        }
    }

    fn song(id: &str) -> Song {
        Song {
            id: id.into(),
            title: format!("Song {id}"),
            artist: "Artist".into(),
            ..Default::default()
        }
    }

    fn remote(id: &str, name: &str, songs: &[&str]) -> Remote {
        Remote {
            id: id.into(),
            name: name.into(),
            comment: String::new(),
            songs: songs.iter().map(|s| song(s)).collect(),
        }
    }

    impl Server for Fake {
        fn playlist_ids(&self) -> Result<Vec<String>, String> {
            if self.down {
                return Err("down".into());
            }
            Ok(self.lists.borrow().iter().map(|r| r.id.clone()).collect())
        }
        fn playlist(&self, id: &str) -> Result<Remote, String> {
            self.meanwhile("playlist", id);
            self.lists
                .borrow()
                .iter()
                .find(|r| r.id == id)
                .cloned()
                .ok_or_else(|| "not found".into())
        }
        fn save_playlist(&self, id: &str, name: &str, songs: &[String]) -> Result<(), String> {
            self.meanwhile("save", id);
            if self.refuse {
                return Err("not yours".into());
            }
            *self.saved.borrow_mut() += 1;
            let mut lists = self.lists.borrow_mut();
            let list = lists.iter_mut().find(|r| r.id == id).ok_or("not found")?;
            list.name = name.into();
            list.songs = songs.iter().map(|s| song(s)).collect();
            Ok(())
        }
        fn delete_playlist(&self, id: &str) -> Result<(), String> {
            self.lists.borrow_mut().retain(|r| r.id != id);
            Ok(())
        }
    }

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        (dir, library)
    }

    fn ids(plugin: &str, songs: &[&str]) -> Vec<String> {
        songs.iter().map(|s| sources::track_id(plugin, s)).collect()
    }

    fn server_list(library: &Library, plugin: &str, id: &str) -> Option<Playlist> {
        library.playlist_by_id(&playlist_id(plugin, id)).unwrap()
    }

    fn names(library: &Library) -> Vec<String> {
        library
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect()
    }

    #[test]
    fn new_renamed_changed_and_deleted_playlists_follow_the_server() {
        let (_dir, library) = library();
        // Needle's own playlist, never touched.
        let own = Playlist {
            id: "own".into(),
            name: "Mine".into(),
            track_ids: vec!["x".into()],
            ..Default::default()
        };
        library.save_playlist(&own).unwrap();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Road trip", &["a", "b"]));
        fake.lists.borrow_mut().push(remote("2", "Gym", &["c"]));
        let report = sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!(report.added, 2);
        let trip = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(trip.name, "Road trip");
        assert_eq!(trip.track_ids, ids("subsonic", &["a", "b"]));
        assert!(!trip.edited_here());
        // The songs were not in the library: they are now, so the playlist plays.
        assert_eq!(library.playlist_tracks(&trip).unwrap().len(), 2);

        // Renamed and changed on the server; "Gym" deleted there.
        {
            let mut lists = fake.lists.borrow_mut();
            lists[0].name = "Road trip 2026".into();
            lists[0].songs = vec![song("b"), song("a"), song("d")];
            lists.remove(1);
        }
        let report = sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!((report.updated, report.removed), (1, 1));
        let trip = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(trip.name, "Road trip 2026");
        assert_eq!(trip.track_ids, ids("subsonic", &["b", "a", "d"]));
        assert!(server_list(&library, "subsonic", "2").is_none());
        assert_eq!(names(&library), ["Mine", "Road trip 2026"]);
        assert_eq!(
            library.playlist_by_id("own").unwrap().unwrap().track_ids,
            ["x"]
        );

        // Nothing changed: nothing to do.
        assert!(!sync(&library, "subsonic", &fake, false).unwrap().changed());
    }

    #[test]
    fn edits_in_needle_go_to_the_server() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        // Renamed, one taken out, the rest moved.
        mix.name = "Better mix".into();
        mix.track_ids = ids("subsonic", &["c", "a"]);
        library.save_playlist(&mix).unwrap();
        assert!(
            library
                .playlist_by_id(&mix.id)
                .unwrap()
                .unwrap()
                .edited_here()
        );
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.sent, 1);
        let on_server = fake.playlist("1").unwrap();
        assert_eq!(on_server.name, "Better mix");
        assert_eq!(
            song_ids("subsonic", &on_server),
            ids("subsonic", &["c", "a"])
        );
        assert!(
            !server_list(&library, "subsonic", "1")
                .unwrap()
                .edited_here()
        );
        // Sending only what changed in Needle asks nothing more of the server.
        assert_eq!(
            sync(&library, "subsonic", &fake, true).unwrap(),
            Report::default()
        );
        assert_eq!(*fake.saved.borrow(), 1);
    }

    #[test]
    fn a_save_without_the_link_keeps_it() {
        // An app that sends back a playlist without the server part (an older Mac model).
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.server = None;
        mix.name = "Renamed".into();
        library.save_playlist(&mix).unwrap();
        let saved = server_list(&library, "subsonic", "1").unwrap();
        assert!(saved.server.is_some() && saved.edited_here());
    }

    #[test]
    fn changes_on_both_sides_keep_both() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids.push(sources::track_id("subsonic", "z"));
        library.save_playlist(&mix).unwrap();
        fake.lists.borrow_mut()[0].songs.push(song("y"));
        let report = sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!(report.notes.len(), 1);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().track_ids,
            ids("subsonic", &["a", "b", "y"])
        );
        let copy = library
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.name == "Mix (Needle copy)")
            .unwrap();
        assert!(copy.server.is_none());
        assert_eq!(copy.track_ids, ids("subsonic", &["a", "b", "z"]));
        // The server was not changed.
        assert_eq!(*fake.saved.borrow(), 0);
    }

    #[test]
    fn a_refused_change_is_kept_as_a_copy() {
        let (_dir, library) = library();
        let fake = Fake {
            refuse: true,
            ..Default::default()
        };
        fake.lists.borrow_mut().push(remote("1", "Theirs", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut list = server_list(&library, "subsonic", "1").unwrap();
        list.name = "Mine now".into();
        library.save_playlist(&list).unwrap();
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.notes.len(), 1);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().name,
            "Theirs"
        );
        assert!(names(&library).contains(&"Mine now (Needle copy)".to_string()));
    }

    #[test]
    fn songs_from_elsewhere_stay_in_a_copy() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let file = crate::model::Track {
            id: "file".into(),
            path: "/music/file.flac".into(),
            title: "A file".into(),
            ..Default::default()
        };
        Library::upsert_on(&library.connection().unwrap(), &file).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids.push("file".into());
        library.save_playlist(&mix).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(
            song_ids("subsonic", &fake.playlist("1").unwrap()),
            ids("subsonic", &["a"])
        );
        let copy = library
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.server.is_none())
            .unwrap();
        assert_eq!(copy.track_ids.last().unwrap(), "file");
    }

    #[test]
    fn deleting_in_needle_deletes_on_the_server_unless_it_changed_there() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Old", &["a"]));
        fake.lists.borrow_mut().push(remote("2", "Busy", &["b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "1"))
            .unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "2"))
            .unwrap();
        fake.lists.borrow_mut()[1].songs.push(song("c"));
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.sent, 1);
        assert!(fake.playlist("1").is_err());
        assert!(fake.playlist("2").is_ok());
        // "Busy" changed on the server: it is back at once, as the server has it.
        assert_eq!(names(&library), ["Busy"]);
        assert_eq!(
            server_list(&library, "subsonic", "2").unwrap().track_ids,
            ids("subsonic", &["b", "c"])
        );
        assert!(
            library
                .get_json::<Vec<ServerPlaylist>>(&deleted_key("subsonic"))
                .unwrap()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_comment_changed_on_the_server_keeps_a_deleted_playlist() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Notes", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "1"))
            .unwrap();
        fake.lists.borrow_mut()[0].comment = "Written on the server".into();
        sync(&library, "subsonic", &fake, true).unwrap();
        assert!(fake.playlist("1").is_ok());
        let back = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(back.description, "Written on the server");
    }

    #[test]
    fn a_deletion_made_during_a_sync_waits_for_the_next() {
        let (dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "First", &["a"]));
        fake.lists.borrow_mut().push(remote("2", "Second", &["b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "1"))
            .unwrap();
        // While the server is asked about the first deletion, the second playlist is deleted.
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, id| {
            if what == "playlist" && id == "1" {
                Library::open(&path)
                    .unwrap()
                    .delete_playlist(&playlist_id("subsonic", "2"))
                    .unwrap();
            }
        }));
        sync(&library, "subsonic", &fake, true).unwrap();
        *fake.during.borrow_mut() = None;
        assert!(fake.playlist("1").is_err());
        // The second one waited, and goes at the next sync instead of coming back.
        sync(&library, "subsonic", &fake, false).unwrap();
        assert!(fake.playlist("2").is_err());
        assert!(names(&library).is_empty());
    }

    #[test]
    fn an_edit_made_during_a_push_is_not_lost() {
        let (dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        // Brings songs b and c into the library.
        fake.lists
            .borrow_mut()
            .push(remote("2", "All", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids = ids("subsonic", &["a", "b"]);
        library.save_playlist(&mix).unwrap();
        // While the first change goes to the server, another song is added.
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, _| {
            if what == "save" {
                let other = Library::open(&path).unwrap();
                let mut now = other
                    .playlist_by_id(&playlist_id("subsonic", "1"))
                    .unwrap()
                    .unwrap();
                if now.track_ids.len() == 2 {
                    now.track_ids.push(sources::track_id("subsonic", "c"));
                    other.save_playlist(&now).unwrap();
                }
            }
        }));
        sync(&library, "subsonic", &fake, true).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.track_ids, ids("subsonic", &["a", "b", "c"]));
        // The newer change waits, on top of what the server has now, and goes next.
        assert!(now.edited_here());
        assert_eq!(
            now.server.as_ref().unwrap().songs,
            ids("subsonic", &["a", "b"])
        );
        sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(
            song_ids("subsonic", &fake.playlist("1").unwrap()),
            ids("subsonic", &["a", "b", "c"])
        );
        assert_eq!(names(&library), ["All", "Mix"]);
    }

    #[test]
    fn a_save_read_before_a_sync_keeps_what_the_sync_brought() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        // An editor opens the playlist; meanwhile a song is added on the server and synced.
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        fake.lists.borrow_mut()[0].songs.push(song("d"));
        sync(&library, "subsonic", &fake, false).unwrap();
        // The editor saves only a new name.
        open.name = "Renamed".into();
        library.save_playlist(&open).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.name, "Renamed");
        assert_eq!(now.track_ids, ids("subsonic", &["a", "b", "c", "d"]));
        sync(&library, "subsonic", &fake, true).unwrap();
        let on_server = fake.playlist("1").unwrap();
        assert_eq!(on_server.name, "Renamed");
        assert_eq!(on_server.songs.len(), 4);
        assert_eq!(names(&library), ["Renamed"]);
    }

    #[test]
    fn a_save_read_before_a_sync_that_changed_the_same_songs_is_kept_as_a_copy() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        fake.lists.borrow_mut()[0].songs.push(song("d"));
        sync(&library, "subsonic", &fake, false).unwrap();
        // The editor took a song out.
        open.track_ids.pop();
        library.save_playlist(&open).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.track_ids, ids("subsonic", &["a", "b", "c", "d"]));
        assert!(!now.edited_here());
        let copy = library
            .playlists()
            .unwrap()
            .into_iter()
            .find(|p| p.name == "Mix (Needle copy)")
            .unwrap();
        assert_eq!(copy.track_ids, ids("subsonic", &["a", "b"]));
    }

    #[test]
    fn deleted_while_a_server_change_comes_in_it_comes_back() {
        let (dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        fake.lists.borrow_mut()[0].name = "Renamed there".into();
        // Deleted in Needle while the sync asks the server for it.
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, id| {
            if what == "playlist" && id == "1" {
                let other = Library::open(&path).unwrap();
                if other
                    .playlist_by_id(&playlist_id("subsonic", "1"))
                    .unwrap()
                    .is_some()
                {
                    other
                        .delete_playlist(&playlist_id("subsonic", "1"))
                        .unwrap();
                }
            }
        }));
        sync(&library, "subsonic", &fake, false).unwrap();
        *fake.during.borrow_mut() = None;
        // It changed on the server since it was last synced: not deleted there, but back.
        sync(&library, "subsonic", &fake, true).unwrap();
        assert!(fake.playlist("1").is_ok());
        assert_eq!(names(&library), ["Renamed there"]);
    }

    #[test]
    fn a_refused_change_edited_meanwhile_makes_one_copy() {
        let (dir, library) = library();
        let fake = Fake {
            refuse: true,
            ..Default::default()
        };
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        fake.lists
            .borrow_mut()
            .push(remote("2", "All", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids = ids("subsonic", &["a", "b"]);
        library.save_playlist(&mix).unwrap();
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, _| {
            if what == "save" {
                let other = Library::open(&path).unwrap();
                let mut now = other
                    .playlist_by_id(&playlist_id("subsonic", "1"))
                    .unwrap()
                    .unwrap();
                now.track_ids.push(sources::track_id("subsonic", "c"));
                other.save_playlist(&now).unwrap();
            }
        }));
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        let copies: Vec<Playlist> = library
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.name == "Mix (Needle copy)")
            .collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].track_ids, ids("subsonic", &["a", "b", "c"]));
        assert_eq!(report.notes.len(), 1);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().track_ids,
            ids("subsonic", &["a"])
        );
    }

    #[test]
    fn signing_in_again_does_not_write_over_a_kept_playlist() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.name = "My mix".into();
        library.save_playlist(&mix).unwrap();
        // Signed out before the change reached the server: it is kept as the person's own.
        forget(&library, "subsonic").unwrap();
        sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!(server_list(&library, "subsonic", "1").unwrap().name, "Mix");
        let mut all = names(&library);
        all.sort();
        assert_eq!(all, ["Mix", "My mix"]);
    }

    #[test]
    fn deleted_on_the_server_after_an_edit_stays_as_the_persons_own() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.name = "Edited".into();
        library.save_playlist(&mix).unwrap();
        fake.lists.borrow_mut().clear();
        sync(&library, "subsonic", &fake, false).unwrap();
        let all = library.playlists().unwrap();
        assert_eq!(all.len(), 1);
        let kept = &all[0];
        assert!(kept.server.is_none());
        assert_eq!(kept.name, "Edited");
        // An id of its own, not the server playlist's.
        assert_ne!(kept.id, mix.id);
    }

    #[test]
    fn a_server_that_cannot_list_changes_nothing() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let down = Fake {
            down: true,
            ..Default::default()
        };
        assert!(sync(&library, "subsonic", &down, false).is_err());
        assert!(server_list(&library, "subsonic", "1").is_some());
    }

    #[test]
    fn sources_do_not_mix_and_signing_out_takes_only_its_own() {
        let (_dir, library) = library();
        let one = Fake::default();
        one.lists.borrow_mut().push(remote("1", "From one", &["a"]));
        let two = Fake::default();
        two.lists.borrow_mut().push(remote("1", "From two", &["a"]));
        sync(&library, "subsonic", &one, false).unwrap();
        sync(&library, "jellyfin", &two, false).unwrap();
        assert_ne!(playlist_id("subsonic", "1"), playlist_id("jellyfin", "1"));
        assert_eq!(names(&library), ["From one", "From two"]);
        let mine = Playlist {
            id: "own".into(),
            name: "Mine".into(),
            ..Default::default()
        };
        library.save_playlist(&mine).unwrap();
        // A change not sent yet stays, as the person's own.
        let mut edited = server_list(&library, "subsonic", "1").unwrap();
        edited.name = "Edited".into();
        library.save_playlist(&edited).unwrap();
        assert_eq!(forget(&library, "jellyfin").unwrap(), 1);
        assert_eq!(names(&library), ["Edited", "Mine"]);
        assert_eq!(forget(&library, "subsonic").unwrap(), 0);
        let left = library.playlists().unwrap();
        assert!(left.iter().all(|p| p.server.is_none()));
        assert_eq!(left.len(), 2);
    }
}
