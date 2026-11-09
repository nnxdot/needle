//! Playlists that live on a music server (made in Navidrome's web page, or any Subsonic
//! server), kept in step with Needle's playlists both ways.
//!
//! Each server playlist is a Needle playlist with a [`ServerPlaylist`] link: the source that
//! brings it, its id on the server, and its name and songs as they were at the last sync. A
//! sync compares both sides with that last state:
//!
//! - changed only on the server: Needle takes the server's version;
//! - changed only in Needle (renamed, songs added, removed, or moved): the change goes to the
//!   server, and is read back. When the server did not keep it, it is a change on both sides;
//! - changed on both sides in different ways, or the server refuses the change: the server's
//!   version stays as the server playlist, and Needle's version is kept as a playlist of the
//!   person's own ("… (Needle copy)"). Nothing is lost on either side;
//! - the server cannot be reached (no connection, a timeout, a server error): the change waits,
//!   and is sent again later. Only a real "no" from the server counts as a refusal;
//! - deleted on the server: it leaves Needle, unless it was changed in Needle and not sent
//!   yet: then it stays as a playlist of the person's own;
//! - deleted in Needle: it is deleted on the server, unless the server's copy changed since
//!   the last sync: then it comes back;
//! - read-only on the server (another person's, or a smart playlist): never changed or
//!   deleted there.
//!
//! Signing out of the server takes its playlists out of Needle (not off the server); ones
//! changed in Needle and not sent stay as the person's own. Their own words and pictures wait
//! for the server's playlist to come back. Needle's own playlists are never touched.
use crate::{
    database::Library,
    model::{Playlist, ServerPlaylist},
    sources::{self, Song},
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
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
    /// Entries Needle cannot read (no title): their places among all the entries, from 0 and
    /// in order, and their ids.
    pub unread: Vec<(usize, String)>,
    /// The server lets this person only read it.
    pub read_only: bool,
    /// When the server last changed it, as the server writes it ("" when it does not say).
    pub changed: String,
}

impl Remote {
    /// The ids of all its entries on the server, in order: its songs and the ones Needle cannot
    /// read.
    pub fn entries(&self) -> Vec<String> {
        let mut all: Vec<String> = self.songs.iter().map(|s| s.id.clone()).collect();
        put_back(&mut all, &self.unread);
        all
    }
}

/// Entries Needle cannot read, back at their places (or at the end, when the list is shorter
/// now).
fn put_back(ids: &mut Vec<String>, unread: &[(usize, String)]) {
    for (at, id) in unread {
        let at = (*at).min(ids.len());
        ids.insert(at, id.clone());
    }
}

/// A playlist in the server's list of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listed {
    pub id: String,
    /// The server lets this person only read it (another person's, or a smart playlist).
    pub read_only: bool,
    /// When the server last changed it ("" when it does not say).
    pub changed: String,
    /// How many entries it has, when the server says.
    pub song_count: Option<usize>,
}

/// Why the server did not do what was asked.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerError {
    /// The server answered, and said no: asking again gives the same answer.
    Refused(String),
    /// No answer that could be read (no connection, a timeout, a server error, not signed in):
    /// asking later may work.
    Unreachable(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(why) | Self::Unreachable(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ServerError {}

/// What a server that keeps playlists can do.
pub trait Server {
    /// The playlists the person can see on the server.
    fn playlists(&self) -> Result<Vec<Listed>, ServerError>;
    /// One playlist with its songs.
    fn playlist(&self, id: &str) -> Result<Remote, ServerError>;
    /// Give a playlist exactly these songs (server ids, in order), and this name when one is
    /// given (else its name stays as it is).
    fn save_playlist(
        &self,
        id: &str,
        name: Option<&str>,
        songs: &[String],
    ) -> Result<(), ServerError>;
    fn delete_playlist(&self, id: &str) -> Result<(), ServerError>;
    /// The server and person signed in ("" when not known): what waits for a server is
    /// dropped when the next sign-in is to another.
    fn account(&self) -> String;
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
    /// Changes made in Needle (edits or deletions) still wait for the server: try again later.
    pub waiting: bool,
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

/// Sources signing out (a library's folder and the source): no full sync brings their
/// playlists back meanwhile.
static SIGNING_OUT: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

/// How many times each source (a library's folder and the source) began signing out: a sync
/// that began before stops between playlists.
static SIGN_OUTS: Mutex<Vec<(PathBuf, String, u64)>> = Mutex::new(Vec::new());

/// Signing out of a source starts (`true`) or is done (`false`). Starting stops a sync under
/// way at its next playlist.
pub(crate) fn signing_out(library: &Library, plugin: &str, on: bool) {
    let mut all = SIGNING_OUT.lock().unwrap_or_else(|e| e.into_inner());
    all.retain(|(dir, p)| !(p == plugin && *dir == library.directory));
    if on {
        all.push((library.directory.clone(), plugin.to_string()));
        let mut counts = SIGN_OUTS.lock().unwrap_or_else(|e| e.into_inner());
        match counts
            .iter_mut()
            .find(|(dir, p, _)| p == plugin && *dir == library.directory)
        {
            Some(count) => count.2 += 1,
            None => counts.push((library.directory.clone(), plugin.to_string(), 1)),
        }
    }
}

/// Whether the source is signing out now.
pub fn is_signing_out(library: &Library, plugin: &str) -> bool {
    SIGNING_OUT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|(dir, p)| p == plugin && *dir == library.directory)
}

fn sign_outs(library: &Library, plugin: &str) -> u64 {
    SIGN_OUTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(dir, p, _)| p == plugin && *dir == library.directory)
        .map_or(0, |c| c.2)
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

/// The server and person the waiting deletions and kept words belong to.
fn account_key(plugin: &str) -> String {
    format!("server-playlists-account:{plugin}")
}

/// Remember that a server playlist was deleted in Needle (see [`Library::delete_playlist`]).
/// The caller holds [`writing`].
pub(crate) fn remember_deleted(db: &Connection, link: &ServerPlaylist) -> Result<()> {
    let key = deleted_key(&link.plugin);
    let mut deleted: Vec<ServerPlaylist> = Library::get_json_on(db, &key)?.unwrap_or_default();
    deleted.retain(|d| d.id != link.id);
    deleted.push(link.clone());
    Library::set_json_on(db, &key, &deleted)
}

/// Whether changes made in Needle to a source's playlists (edits, deletions) wait to go to the
/// server.
pub fn pending(library: &Library, plugin: &str) -> Result<bool> {
    pending_on(&library.connection()?, plugin)
}

fn pending_on(db: &Connection, plugin: &str) -> Result<bool> {
    let deleted: Vec<ServerPlaylist> =
        Library::get_json_on(db, &deleted_key(plugin))?.unwrap_or_default();
    if !deleted.is_empty() {
        return Ok(true);
    }
    Ok(Library::playlists_on(db)?.iter().any(|p| {
        p.edited_here()
            && p.server
                .as_ref()
                .is_some_and(|s| s.plugin == plugin && !s.read_only)
    }))
}

fn immediate(db: &mut Connection) -> Result<Transaction<'_>> {
    Ok(db.transaction_with_behavior(TransactionBehavior::Immediate)?)
}

/// A sync stops after this many requests in a row reached no server: the rest waits for the
/// next, instead of each playlist waiting for its own timeout.
const STOP_AFTER: usize = 3;

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
    if !only_edited && is_signing_out(library, plugin) {
        return Ok(Report::default());
    }
    let mut run = Run {
        library,
        plugin,
        server,
        db: library.connection()?,
        report: Report::default(),
        sign_outs: sign_outs(library, plugin),
        misses: 0,
    };
    run.check_account()?;
    let listed = server.playlists().map_err(anyhow::Error::new)?;
    run.all(&listed, only_edited)?;
    run.report.waiting = pending_on(&run.db, plugin)?;
    if run.misses >= STOP_AFTER && run.report.waiting {
        run.report
            .notes
            .push("The server stopped answering; your playlist changes wait for it".into());
    }
    Ok(run.report)
}

/// One sync under way.
struct Run<'a> {
    library: &'a Library,
    plugin: &'a str,
    server: &'a dyn Server,
    /// One connection for the whole sync.
    db: Connection,
    report: Report,
    /// [`sign_outs`] when the sync began.
    sign_outs: u64,
    /// Requests in a row that reached no server.
    misses: usize,
}

impl Run<'_> {
    /// Stop here: the source began signing out, or the server stopped answering.
    fn stop(&self) -> bool {
        self.misses >= STOP_AFTER || self.signed_out()
    }

    fn signed_out(&self) -> bool {
        sign_outs(self.library, self.plugin) != self.sign_outs
    }

    /// Count a request that reached no server; any answer starts the count again.
    fn answered<T>(&mut self, result: &Result<T, ServerError>) {
        if matches!(result, Err(ServerError::Unreachable(_))) {
            self.misses += 1;
        } else {
            self.misses = 0;
        }
    }

    /// Signed in to another server or as another person since the last sync: the deletions
    /// and the kept words and pictures waiting were for the other one, and are dropped.
    fn check_account(&mut self) -> Result<()> {
        let now = self.server.account();
        if now.is_empty() {
            return Ok(());
        }
        let key = account_key(self.plugin);
        let _write = writing();
        let tx = immediate(&mut self.db)?;
        let before: Option<String> = Library::get_json_on(&tx, &key)?;
        let mut covers: Vec<String> = vec![];
        if before.as_ref().is_some_and(|b| *b != now) {
            Library::set_json_on(
                &tx,
                &deleted_key(self.plugin),
                &Vec::<ServerPlaylist>::new(),
            )?;
            covers = tx
                .prepare(
                    "SELECT cover FROM server_playlist_own WHERE plugin=? AND cover IS NOT NULL",
                )?
                .query_map([self.plugin], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            tx.execute(
                "DELETE FROM server_playlist_own WHERE plugin=?",
                [self.plugin],
            )?;
        }
        if before.as_deref() != Some(now.as_str()) {
            Library::set_json_on(&tx, &key, &now)?;
        }
        tx.commit()?;
        // The pictures kept for the other one's playlists are no one's now.
        for cover in covers {
            self.library.forget_playlist_picture(&cover);
        }
        Ok(())
    }

    fn all(&mut self, listed: &[Listed], only_edited: bool) -> Result<()> {
        let plugin = self.plugin;
        let on_server: HashMap<&str, &Listed> = listed.iter().map(|l| (l.id.as_str(), l)).collect();

        // Deleted in Needle: deleted on the server too, when the server's copy is as it was (its
        // name, songs, and comment). One that changed there, or that the server would not
        // delete, comes back at once. One the server could not be asked about waits.
        let key = deleted_key(plugin);
        let deleted: Vec<ServerPlaylist> = {
            let _write = writing();
            Library::get_json_on(&self.db, &key)?.unwrap_or_default()
        };
        let (mut settled, mut gone, mut back) = (HashSet::new(), HashSet::new(), HashSet::new());
        for link in &deleted {
            if self.stop() {
                break;
            }
            let Some(entry) = on_server.get(link.id.as_str()) else {
                settled.insert(link.id.clone());
                continue;
            };
            if entry.read_only || link.read_only {
                self.report.notes.push(format!(
                    "“{}” is read-only on the server, so it is back",
                    link.name
                ));
                settled.insert(link.id.clone());
                back.insert(link.id.clone());
                continue;
            }
            let remote = self.server.playlist(&link.id);
            self.answered(&remote);
            // Not reached: tried again at the next sync.
            let Ok(remote) = remote else {
                continue;
            };
            let unchanged = shown_name(&remote.name) == link.name
                && song_ids(plugin, &remote) == link.songs
                && remote.comment == link.comment;
            if !unchanged {
                self.report.notes.push(format!(
                    "“{}” changed on the server after you deleted it, so it is back",
                    link.name
                ));
                settled.insert(link.id.clone());
                back.insert(link.id.clone());
                continue;
            }
            let result = self.server.delete_playlist(&link.id);
            self.answered(&result);
            match result {
                Ok(()) => {
                    self.report.sent += 1;
                    settled.insert(link.id.clone());
                    gone.insert(link.id.clone());
                }
                Err(ServerError::Refused(error)) => {
                    self.report.notes.push(format!(
                        "“{}” could not be deleted on the server ({error}), so it is back",
                        link.name
                    ));
                    settled.insert(link.id.clone());
                    back.insert(link.id.clone());
                }
                // Waits: tried again later (a deletion that went after all is settled then).
                Err(ServerError::Unreachable(_)) => {}
            }
        }
        // Only the ones handled leave the list: a playlist deleted meanwhile waits for the next.
        // The deletions waiting and the playlists are read together, so a playlist deleted in
        // between is in one or the other.
        let (waiting, ours, orphans) = {
            let _write = writing();
            let tx = immediate(&mut self.db)?;
            let mut now: Vec<ServerPlaylist> = Library::get_json_on(&tx, &key)?.unwrap_or_default();
            now.retain(|link| !(settled.contains(&link.id) && deleted.contains(link)));
            Library::set_json_on(&tx, &key, &now)?;
            let waiting: HashSet<String> = now.into_iter().map(|link| link.id).collect();
            let (mut ours, mut orphans) = (HashMap::new(), HashMap::new());
            for playlist in Library::playlists_on(&tx)? {
                match &playlist.server {
                    Some(link) if link.plugin == plugin => {
                        ours.insert(link.id.clone(), playlist);
                    }
                    // A server playlist an older Needle saved without its link.
                    None if playlist.id.starts_with("srv-") && playlist.query.is_none() => {
                        orphans.insert(playlist.id.clone(), playlist);
                    }
                    _ => {}
                }
            }
            tx.commit()?;
            (waiting, ours, orphans)
        };

        for entry in listed {
            if self.stop() {
                break;
            }
            let id = &entry.id;
            if gone.contains(id) || waiting.contains(id) {
                continue;
            }
            // One saved by an older Needle without its link is linked again, as changed in
            // Needle since a last sync of nothing: the rules for changes on both sides decide.
            let local = ours.get(id).cloned().or_else(|| {
                orphans
                    .get(&playlist_id(plugin, id))
                    .map(|orphan| Playlist {
                        server: Some(ServerPlaylist {
                            plugin: plugin.to_string(),
                            id: id.clone(),
                            ..Default::default()
                        }),
                        ..orphan.clone()
                    })
            });
            let edited = local.as_ref().is_some_and(Playlist::edited_here);
            if only_edited && !back.contains(id) && !edited {
                continue;
            }
            if !only_edited
                && !back.contains(id)
                && !edited
                && local.as_ref().is_some_and(|l| same_as_listed(l, entry))
            {
                continue;
            }
            let remote = self.server.playlist(id);
            self.answered(&remote);
            let mut remote = match remote {
                Ok(remote) => remote,
                Err(error) => {
                    if let Some(local) = local.as_ref().filter(|l| l.edited_here()) {
                        self.report.notes.push(format!(
                            "Your changes to “{}” wait for the server: {error}",
                            local.name
                        ));
                    }
                    continue;
                }
            };
            remote.read_only |= entry.read_only;
            if remote.changed.is_empty() {
                remote.changed = entry.changed.clone();
            }
            let theirs = self.add_songs(&remote.songs)?;
            match local {
                None => self.add_new(id, &remote, theirs)?,
                Some(local) => self.sync_one(&local, &remote, theirs)?,
            }
        }

        // Gone from the server.
        for (id, playlist) in ours {
            if on_server.contains_key(id.as_str()) || gone.contains(&id) {
                continue;
            }
            if self.signed_out() {
                break;
            }
            let _write = writing();
            let tx = immediate(&mut self.db)?;
            // As it is now: the person may have changed it while the server was asked.
            let Some(now) = Library::playlist_on(&tx, &playlist.id)? else {
                continue;
            };
            if made_own(&now) {
                self.report.notes.push(format!(
                    "“{}” was deleted on the server; your changes are kept as your own playlist",
                    now.name
                ));
                detach(&tx, now)?;
            } else {
                Library::drop_playlist_on(&tx, &now.id)?;
                self.report.removed += 1;
            }
            tx.commit()?;
        }
        Ok(())
    }

    /// A playlist new in Needle. Words or a picture of the person's own, kept when they signed
    /// out, come back with it.
    fn add_new(&mut self, id: &str, remote: &Remote, theirs: Vec<String>) -> Result<()> {
        let plugin = self.plugin;
        let mut playlist = Playlist {
            id: playlist_id(plugin, id),
            name: shown_name(&remote.name),
            description: remote.comment.clone(),
            track_ids: theirs.clone(),
            updated_at: chrono::Utc::now().timestamp(),
            server: Some(link(plugin, remote, theirs)),
            ..Default::default()
        };
        let _write = writing();
        let tx = immediate(&mut self.db)?;
        // Deleted in Needle while the server was asked: it is not brought back.
        let deleting: Vec<ServerPlaylist> =
            Library::get_json_on(&tx, &deleted_key(plugin))?.unwrap_or_default();
        if deleting.iter().any(|d| d.id == id) || Library::playlist_on(&tx, &playlist.id)?.is_some()
        {
            return Ok(());
        }
        let own: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT description, cover FROM server_playlist_own WHERE plugin=? AND id=?",
                [plugin, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((description, cover)) = own {
            if !description.is_empty() {
                playlist.description = description;
            }
            playlist.cover = cover;
            tx.execute(
                "DELETE FROM server_playlist_own WHERE plugin=? AND id=?",
                [plugin, id],
            )?;
        }
        Library::store_playlist_on(&tx, &playlist)?;
        tx.commit()?;
        self.report.added += 1;
        Ok(())
    }

    /// One playlist that is both in Needle and on the server.
    fn sync_one(&mut self, local: &Playlist, remote: &Remote, theirs: Vec<String>) -> Result<()> {
        let base = local.server.clone().unwrap_or_default();
        let here = local.edited_here();
        let there = shown_name(&remote.name) != base.name || theirs != base.songs;
        if here && remote.read_only {
            // Never sent: the server's version stays, the person's is kept as their own.
            let version = server_version(self.plugin, local, remote, theirs);
            self.commit(
                local,
                version,
                Pulled(Some("it is read-only on the server")),
            )?;
            self.report.updated += 1;
        } else if here && !there {
            self.push(local, remote)?;
        } else if here {
            let same = local.name == shown_name(&remote.name) && local.track_ids == theirs;
            let why = (!same).then_some("it changed on the server too");
            let version = server_version(self.plugin, local, remote, theirs);
            self.commit(local, version, Pulled(why))?;
            self.report.updated += 1;
        } else {
            let shown =
                there || remote.comment != base.comment || remote.read_only != base.read_only;
            if shown || remote.changed != base.changed || remote.unread != base.unread {
                let version = server_version(self.plugin, local, remote, theirs);
                self.commit(local, version, Pulled(None))?;
                if shown {
                    self.report.updated += 1;
                }
            }
        }
        Ok(())
    }

    /// Send Needle's version of a playlist changed only in Needle, then read it back: what the
    /// server has now decides. Songs from elsewhere (files on this computer, another server)
    /// cannot be on the server's playlist: they are left out of what is sent (`commit` keeps
    /// the whole list as the person's own as well). Songs no longer in the library just go.
    fn push(&mut self, local: &Playlist, remote: &Remote) -> Result<()> {
        let plugin = self.plugin;
        let base = local.server.clone().unwrap_or_default();
        let (_, mut ids) = server_songs(&self.db, plugin, &local.track_ids)?;
        // Entries Needle cannot read stay on the server, in place.
        put_back(&mut ids, &remote.unread);
        // Renamed on the server only when renamed in Needle: an unnamed one stays unnamed.
        let name = (local.name != base.name).then_some(local.name.as_str());
        let sent = self.server.save_playlist(&remote.id, name, &ids);
        self.answered(&sent);
        // Read back after any push, also one that failed: it can have half gone (the songs
        // and the name are two requests).
        let back = self.server.playlist(&remote.id);
        self.answered(&back);
        let before = (shown_name(&remote.name), remote.entries());
        let waits = |run: &mut Self, error: String| {
            run.report.notes.push(format!(
                "Your changes to “{}” wait for the server: {error}",
                local.name
            ));
        };
        let back = match back {
            Ok(back) => back,
            Err(error) => {
                match sent {
                    // Gone or not: the next sync sees (a server version as Needle's is no
                    // change on both sides).
                    Ok(()) => waits(self, format!("it could not be read back ({error})")),
                    Err(ServerError::Unreachable(error)) => waits(self, error),
                    Err(ServerError::Refused(error)) => {
                        let why = format!("the server did not take the changes: {error}");
                        let theirs = song_ids(plugin, remote);
                        let version = server_version(plugin, local, remote, theirs);
                        self.commit(local, version, Pulled(Some(&why)))?;
                        self.report.updated += 1;
                    }
                }
                return Ok(());
            }
        };
        let entries = back.entries();
        let now = (shown_name(&back.name), entries.clone());
        // The server may leave out songs it no longer has: then the rest is as sent.
        let songs_went = entries == ids || (entries != before.1 && in_order_within(&entries, &ids));
        let name_went = name.is_none_or(|n| now.0 == shown_name(n));
        let theirs = self.add_songs(&back.songs)?;
        let mut moved = link(plugin, &back, theirs.clone());
        moved.pushed_over = Some((base.name.clone(), base.songs.clone()));
        match sent {
            _ if songs_went && name_went => {
                let mut pushed = local.clone();
                pushed.track_ids = theirs;
                pushed.server = Some(moved);
                self.commit(local, pushed, Pushed { again: true })?;
                self.report.sent += 1;
            }
            Err(ServerError::Unreachable(error)) if now == before => waits(self, error),
            Err(ServerError::Unreachable(error)) if songs_went => {
                // The songs went, the name did not: the server's state is the new last-synced
                // one, and the new name waits.
                let mut half = local.clone();
                half.track_ids = theirs;
                half.server = Some(moved);
                self.commit(local, half, Pushed { again: false })?;
                waits(self, error);
            }
            result => {
                let why = match result {
                    Err(ServerError::Refused(error)) => {
                        format!("the server did not take the changes: {error}")
                    }
                    Ok(()) => "the server did not keep the changes".into(),
                    Err(ServerError::Unreachable(_)) => "it changed on the server too".into(),
                };
                let version = server_version(plugin, local, &back, theirs);
                self.commit(local, version, Pulled(Some(&why)))?;
                self.report.updated += 1;
            }
        }
        Ok(())
    }

    /// The library ids of a server playlist's songs; songs not in the library yet (the server
    /// did not list them in a sync) are added from what the playlist says about them, and ones
    /// marked missing are not missing. One transaction.
    fn add_songs(&mut self, songs: &[Song]) -> Result<Vec<String>> {
        let plugin = self.plugin;
        let now = chrono::Utc::now().timestamp();
        let tx = immediate(&mut self.db)?;
        let mut ids = Vec::with_capacity(songs.len());
        {
            let mut known = tx.prepare_cached("SELECT missing FROM tracks WHERE id=?")?;
            for song in songs {
                let track = sources::to_track(plugin, song, now);
                let missing: Option<bool> =
                    known.query_row([&track.id], |r| r.get(0)).optional()?;
                match missing {
                    // The server's list of songs left it out, but its playlist has it: it is
                    // there.
                    Some(true) => {
                        tx.execute("UPDATE tracks SET missing=0 WHERE id=?", [&track.id])?;
                    }
                    Some(false) => {}
                    None => {
                        Library::upsert_on(&tx, &track)?;
                        if !song.cover.is_empty() {
                            let key = if song.cover_id.is_empty() {
                                song.cover.clone()
                            } else {
                                song.cover_id.clone()
                            };
                            self.report
                                .covers
                                .push((track.id.clone(), song.cover.clone(), key));
                        }
                    }
                }
                ids.push(track.id);
            }
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Write what a sync made of `local` (the playlist as read before the server was asked),
    /// in one transaction.
    ///
    /// The person may have changed it meanwhile; their newer version is never written over:
    /// - after a push, it stays, on the new last-synced state, and goes to the server next
    ///   (at once with `again`);
    /// - after taking the server's version, it is kept as a copy of their own (one copy, of
    ///   the newest version, also when a copy was to be kept anyway). Songs that only left
    ///   the library meanwhile (a refresh) are no change of theirs.
    ///
    /// Deleted meanwhile: after a push, the deletion waits with the new last-synced state (the
    /// server's copy is now as Needle sent it). After taking the server's version it keeps the
    /// state it was deleted from, so a server playlist changed since comes back, not deleted.
    fn commit(&mut self, local: &Playlist, mut result: Playlist, outcome: Outcome) -> Result<()> {
        let link = result.server.clone().unwrap_or_default();
        let plugin = link.plugin.clone();
        // Which songs can go to the server, worked out before the lock: again under it only
        // when the songs changed in between.
        let split = match (&outcome, Library::playlist_on(&self.db, &local.id)?) {
            (Pushed { .. }, Some(now)) => Some(Split::of(&self.db, &plugin, now.track_ids)?),
            _ => None,
        };
        let again = {
            let _write = writing();
            let tx = immediate(&mut self.db)?;
            let report = &mut self.report;
            let again = match Library::playlist_on(&tx, &local.id)? {
                None => {
                    if matches!(outcome, Pushed { .. }) {
                        let key = deleted_key(&plugin);
                        let mut deleted: Vec<ServerPlaylist> =
                            Library::get_json_on(&tx, &key)?.unwrap_or_default();
                        for waiting in deleted.iter_mut().filter(|d| d.id == link.id) {
                            *waiting = link.clone();
                        }
                        Library::set_json_on(&tx, &key, &deleted)?;
                    }
                    false
                }
                Some(now) => match outcome {
                    Pushed { again } => {
                        let split = match split {
                            Some(split) if split.songs == now.track_ids => split,
                            _ => Split::of(&tx, &plugin, now.track_ids.clone())?,
                        };
                        // The newest version: songs not on the server keep the whole list as the
                        // person's own (once), and leave the server playlist. As stored now:
                        // its description and picture may have changed meanwhile.
                        let mut newest = now;
                        if split.foreign {
                            keep_copy(&tx, &newest, report, "songs that are not on the server")?;
                        }
                        newest.track_ids = if newest.track_ids == local.track_ids {
                            // Not changed meanwhile: as the server has it now.
                            result.track_ids.clone()
                        } else {
                            split.mine
                        };
                        // Words left as the old comment follow the server's new one.
                        let old = local.server.as_ref().map(|s| s.comment.as_str());
                        if Some(newest.description.as_str()) == old {
                            newest.description = link.comment.clone();
                        }
                        newest.server = Some(link.clone());
                        Library::store_playlist_on(&tx, &newest)?;
                        // Changed meanwhile: that change goes next.
                        again && newest.edited_here()
                    }
                    Pulled(why) => {
                        let changed = now.name != local.name
                            || (now.track_ids != local.track_ids
                                && now.track_ids != in_library(&tx, &local.track_ids)?);
                        let why = match why {
                            Some(why) => Some(why),
                            None if changed => Some("it changed on the server while you edited it"),
                            None => None,
                        };
                        if let Some(why) = why {
                            keep_copy(&tx, &now, report, why)?;
                        }
                        // Its description and picture may have changed meanwhile: those stay.
                        if now.description != local.description {
                            result.description = now.description;
                        }
                        result.cover = now.cover;
                        Library::store_playlist_on(&tx, &result)?;
                        false
                    }
                },
            };
            tx.commit()?;
            again
        };
        if again {
            edited(&plugin);
        }
        Ok(())
    }
}

/// A playlist's songs as they can go to a source's server.
struct Split {
    /// The songs it was worked out for.
    songs: Vec<String>,
    /// Some cannot be on the server (files, another server's songs).
    foreign: bool,
    /// The ones on the server.
    mine: Vec<String>,
}

impl Split {
    fn of(db: &Connection, plugin: &str, songs: Vec<String>) -> Result<Self> {
        let foreign = has_foreign(db, plugin, &songs)?;
        let mine = server_songs(db, plugin, &songs)?.0;
        Ok(Self {
            songs,
            foreign,
            mine,
        })
    }
}

/// Whether the server's list says a playlist is as at its last sync: same time of change, same
/// count of entries, and as readable. Playlists that are read-only are read every time (a
/// smart playlist's songs change without the playlist being changed).
fn same_as_listed(local: &Playlist, entry: &Listed) -> bool {
    local.server.as_ref().is_some_and(|link| {
        !entry.read_only
            && !link.read_only
            && !entry.changed.is_empty()
            && link.changed == entry.changed
            && entry
                .song_count
                .is_none_or(|count| count == link.songs.len() + link.unread.len())
    })
}

/// Whether `part` is `whole` with some left out, in the same order.
fn in_order_within(part: &[String], whole: &[String]) -> bool {
    let mut rest = whole.iter();
    part.iter().all(|id| rest.any(|w| w == id))
}

/// Whether the person made something of a server playlist that only Needle has: changes not
/// sent yet, its own words (not the server's comment), or its own picture. Such a playlist is
/// kept, as theirs, when it leaves the server.
fn made_own(playlist: &Playlist) -> bool {
    playlist.edited_here()
        || playlist.cover.is_some()
        || playlist
            .server
            .as_ref()
            .is_some_and(|link| playlist.description != link.comment)
}

/// Make a server playlist the person's own: no link, and an id of its own. The server
/// playlist's id is kept for the server's own copy, which a later sync (after signing in
/// again) would write into, over the person's changes. The caller's transaction holds both
/// writes.
fn detach(db: &Connection, mut playlist: Playlist) -> Result<()> {
    let old = std::mem::replace(&mut playlist.id, uuid::Uuid::new_v4().to_string());
    playlist.server = None;
    Library::store_playlist_on(db, &playlist)?;
    Library::drop_playlist_on(db, &old)
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

/// Of a playlist's songs, the ones on this source's server: their library ids and their ids
/// on the server, in order. The rest (files, another server's songs, songs gone from the
/// library) are left out.
fn server_songs(
    db: &Connection,
    plugin: &str,
    track_ids: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let (mut mine, mut ids) = (vec![], vec![]);
    let mut path = db.prepare_cached("SELECT path FROM tracks WHERE id=?")?;
    for id in track_ids {
        let found: Option<String> = path.query_row([id], |r| r.get(0)).optional()?;
        if let Some(found) = found
            && let Some((owner, song)) = sources::parse_path(&found)
            && owner == plugin
        {
            mine.push(id.clone());
            ids.push(song);
        }
    }
    Ok((mine, ids))
}

/// Whether a playlist holds songs that cannot be on this source's server (files, another
/// server's songs), not just songs gone from the library.
fn has_foreign(db: &Connection, plugin: &str, track_ids: &[String]) -> Result<bool> {
    let mut path = db.prepare_cached("SELECT path FROM tracks WHERE id=?")?;
    for id in track_ids {
        let found: Option<String> = path.query_row([id], |r| r.get(0)).optional()?;
        if found.is_some_and(|p| sources::parse_path(&p).is_none_or(|(owner, _)| owner != plugin)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The songs still in the library, in order.
fn in_library(db: &Connection, track_ids: &[String]) -> Result<Vec<String>> {
    let mut known = db.prepare_cached("SELECT 1 FROM tracks WHERE id=?")?;
    let mut kept = vec![];
    for id in track_ids {
        if known.exists([id])? {
            kept.push(id.clone());
        }
    }
    Ok(kept)
}

/// How a sync's result for one playlist came about.
#[derive(Clone, Copy)]
enum Outcome<'a> {
    /// Needle's version went to the server; with `again`, a change made meanwhile goes at
    /// once.
    Pushed { again: bool },
    /// The server's version was taken; with why, when Needle's version is to be kept as a
    /// copy (it changed on both sides, or the server refused it).
    Pulled(Option<&'a str>),
}
use Outcome::{Pulled, Pushed};

/// A save from an app that read the playlist before a sync moved it on (`seen` is the
/// last-synced state the app read, `stored` the playlist as it is now). Name and songs are
/// taken each on its own: what only the sync changed stays as the sync left it, what only the
/// person changed is theirs, and when both changed the same one, the sync's stays and the
/// person's whole version is kept as a copy (returned). The server part is the stored one.
///
/// Needle's own push moves the last-synced state too, but is no change made meanwhile: the
/// songs count as changed only when the stored songs are not what the app read.
pub(crate) fn merge_save(
    saved: &Playlist,
    seen: &ServerPlaylist,
    stored: &Playlist,
) -> (Playlist, Option<Playlist>) {
    let now = stored.server.clone().unwrap_or_default();
    let pushed = now
        .pushed_over
        .as_ref()
        .is_some_and(|(name, songs)| *name == seen.name && *songs == seen.songs);
    let mut merged = saved.clone();
    merged.server = stored.server.clone();
    let mut clash = false;
    if saved.name != stored.name && now.name != seen.name && !pushed {
        if saved.name != seen.name {
            clash = true;
        }
        merged.name = stored.name.clone();
    }
    if saved.track_ids != stored.track_ids
        && now.songs != seen.songs
        && stored.track_ids != seen.songs
        && !pushed
    {
        if saved.track_ids != seen.songs {
            clash = true;
        }
        merged.track_ids = stored.track_ids.clone();
    }
    // The description follows the server's comment while it was left alone: one the editor
    // still shows as the old comment is the sync's to change.
    if saved.description != stored.description
        && now.comment != seen.comment
        && saved.description == seen.comment
    {
        merged.description = stored.description.clone();
    }
    (merged, clash.then(|| copy_of(saved)))
}

/// A save of a playlist the server lets the person only read, that changed its name or songs:
/// the person's version as a playlist of their own, and the playlist with the server's name
/// and songs kept (its words and picture as saved). `None` for any other save.
pub(crate) fn keep_read_only(stored: &Playlist, saved: &Playlist) -> Option<(Playlist, Playlist)> {
    let link = stored.server.as_ref().filter(|s| s.read_only)?;
    if saved.name == stored.name && saved.track_ids == stored.track_ids {
        return None;
    }
    let mut kept = saved.clone();
    kept.name = stored.name.clone();
    kept.track_ids = stored.track_ids.clone();
    kept.server = Some(link.clone());
    Some((copy_of(saved), kept))
}

/// Needle's version of a server playlist, as a playlist of the person's own.
pub(crate) fn copy_of(local: &Playlist) -> Playlist {
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
fn keep_copy(db: &Connection, local: &Playlist, report: &mut Report, why: &str) -> Result<()> {
    let copy = copy_of(local);
    Library::store_playlist_on(db, &copy)?;
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
        read_only: remote.read_only,
        changed: remote.changed.clone(),
        unread: remote.unread.clone(),
        pushed_over: None,
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

/// After signing out: the source's playlists leave Needle (they stay on the server). One
/// changed in Needle and not sent yet stays, as the person's own. Words or a picture of the
/// person's own wait in a table of their own for the server's playlist to come back at the
/// next sign-in. Deletions waiting stay too, unless that sign-in is to another server or
/// person. Returns how many left. A sync under way stops at its next playlist (see
/// [`signing_out`]).
pub fn forget(library: &Library, plugin: &str) -> Result<usize> {
    let _one = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let _write = writing();
    let mut db = library.connection()?;
    let tx = immediate(&mut db)?;
    let mut count = 0;
    for playlist in Library::playlists_on(&tx)? {
        let Some(link) = playlist.server.clone().filter(|l| l.plugin == plugin) else {
            continue;
        };
        if playlist.edited_here() {
            detach(&tx, playlist)?;
            continue;
        }
        let words = if playlist.description != link.comment {
            playlist.description.clone()
        } else {
            String::new()
        };
        if !words.is_empty() || playlist.cover.is_some() {
            tx.execute(
                "INSERT INTO server_playlist_own VALUES (?,?,?,?) ON CONFLICT(plugin,id)
                 DO UPDATE SET description=excluded.description, cover=excluded.cover",
                params![plugin, link.id, words, playlist.cover],
            )?;
        }
        Library::drop_playlist_on(&tx, &playlist.id)?;
        count += 1;
    }
    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// What happens in Needle while the server is asked (`"playlist"`, `"save"`, or
    /// `"delete"`, and the id).
    type During = Box<dyn FnMut(&str, &str)>;

    /// A server in memory.
    #[derive(Default)]
    struct Fake {
        lists: RefCell<Vec<Remote>>,
        /// Saves are refused (a real "no").
        refuse: bool,
        /// The list of playlists cannot be read.
        down: bool,
        /// Every request but the list reaches no server (the connection dropped).
        cut: Cell<bool>,
        /// A push's songs go, then the connection drops before its new name.
        half: Cell<bool>,
        /// A push is answered "ok", but nothing changes.
        forgetful: Cell<bool>,
        /// Ids the person may only read.
        read_only: RefCell<HashSet<String>>,
        account: RefCell<String>,
        saved: RefCell<usize>,
        /// Requests for one playlist, its songs, a push, or a deletion.
        asked: Cell<usize>,
        during: RefCell<Option<During>>,
    }

    impl Fake {
        fn meanwhile(&self, what: &str, id: &str) {
            self.asked.set(self.asked.get() + 1);
            if let Some(during) = self.during.borrow_mut().as_mut() {
                during(what, id);
            }
        }
        fn reached(&self) -> Result<(), ServerError> {
            if self.cut.get() {
                return Err(ServerError::Unreachable("timed out".into()));
            }
            Ok(())
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
            songs: songs.iter().map(|s| song(s)).collect(),
            ..Default::default()
        }
    }

    impl Server for Fake {
        fn playlists(&self) -> Result<Vec<Listed>, ServerError> {
            if self.down {
                return Err(ServerError::Unreachable("down".into()));
            }
            Ok(self
                .lists
                .borrow()
                .iter()
                .map(|r| Listed {
                    id: r.id.clone(),
                    read_only: self.read_only.borrow().contains(&r.id),
                    changed: r.changed.clone(),
                    song_count: Some(r.songs.len() + r.unread.len()),
                })
                .collect())
        }
        fn playlist(&self, id: &str) -> Result<Remote, ServerError> {
            self.meanwhile("playlist", id);
            self.reached()?;
            let mut found = self
                .lists
                .borrow()
                .iter()
                .find(|r| r.id == id)
                .cloned()
                .ok_or_else(|| ServerError::Refused("not found".into()))?;
            found.read_only = self.read_only.borrow().contains(id);
            Ok(found)
        }
        fn save_playlist(
            &self,
            id: &str,
            name: Option<&str>,
            songs: &[String],
        ) -> Result<(), ServerError> {
            self.meanwhile("save", id);
            self.reached()?;
            if self.refuse {
                return Err(ServerError::Refused("not yours".into()));
            }
            *self.saved.borrow_mut() += 1;
            if self.forgetful.get() {
                return Ok(());
            }
            let mut lists = self.lists.borrow_mut();
            let list = lists
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| ServerError::Refused("not found".into()))?;
            let unread: Vec<String> = list.unread.iter().map(|u| u.1.clone()).collect();
            list.songs = songs
                .iter()
                .filter(|s| !unread.contains(s))
                .map(|s| song(s))
                .collect();
            list.unread = songs
                .iter()
                .enumerate()
                .filter(|(_, s)| unread.contains(s))
                .map(|(i, s)| (i, s.clone()))
                .collect();
            if !list.changed.is_empty() {
                list.changed += "+";
            }
            if self.half.get() {
                return Err(ServerError::Unreachable("connection reset".into()));
            }
            if let Some(name) = name {
                list.name = name.into();
            }
            Ok(())
        }
        fn delete_playlist(&self, id: &str) -> Result<(), ServerError> {
            self.meanwhile("delete", id);
            self.reached()?;
            self.lists.borrow_mut().retain(|r| r.id != id);
            Ok(())
        }
        fn account(&self) -> String {
            self.account.borrow().clone()
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
    fn an_editor_saving_after_signing_out_keeps_its_own_playlist() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        forget(&library, "subsonic").unwrap();
        open.name = "My draft".into();
        let id = library.save_playlist_as(&open).unwrap();
        assert_ne!(id, playlist_id("subsonic", "1"));
        assert!(
            library
                .playlist_by_id(&id)
                .unwrap()
                .unwrap()
                .server
                .is_none()
        );
        // Signing in again brings the server's back beside it, over nothing.
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut all = names(&library);
        all.sort();
        assert_eq!(all, ["Mix", "My draft"]);
    }

    #[test]
    fn songs_from_elsewhere_added_during_a_push_make_one_copy() {
        let (dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        fake.lists
            .borrow_mut()
            .push(remote("2", "All", &["a", "b"]));
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
        // Another song is added while the push runs.
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, _| {
            if what == "save" {
                let other = Library::open(&path).unwrap();
                let mut now = other
                    .playlist_by_id(&playlist_id("subsonic", "1"))
                    .unwrap()
                    .unwrap();
                if !now.track_ids.contains(&sources::track_id("subsonic", "b")) {
                    now.track_ids.push(sources::track_id("subsonic", "b"));
                    other.save_playlist(&now).unwrap();
                }
            }
        }));
        sync(&library, "subsonic", &fake, true).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        let copies: Vec<Playlist> = library
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.name == "Mix (Needle copy)")
            .collect();
        assert_eq!(copies.len(), 1);
        assert!(copies[0].track_ids.contains(&"file".to_string()));
        assert_eq!(
            song_ids("subsonic", &fake.playlist("1").unwrap()),
            ids("subsonic", &["a", "b"])
        );
    }

    #[test]
    fn a_save_read_before_a_sync_keeps_the_servers_new_comment() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        fake.lists.borrow_mut()[0].comment = "New notes".into();
        sync(&library, "subsonic", &fake, false).unwrap();
        open.name = "Renamed".into();
        library.save_playlist(&open).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.name, "Renamed");
        assert_eq!(now.description, "New notes");
    }

    #[test]
    fn an_editor_saving_only_new_words_keeps_the_synced_songs() {
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
        // An editor opens it with the file; the push takes the file out (and copies it once).
        let opened = server_list(&library, "subsonic", "1").unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        let mut saved = opened.clone();
        saved.description = "New words".into();
        library.save_playlist_changes(&opened, &saved).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.track_ids, ids("subsonic", &["a"]));
        assert_eq!(now.description, "New words");
        assert!(!now.edited_here());
        sync(&library, "subsonic", &fake, true).unwrap();
        let copies = library
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.name == "Mix (Needle copy)")
            .count();
        assert_eq!(copies, 1);
    }

    #[test]
    fn a_push_takes_the_servers_comment_as_seen() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        fake.lists.borrow_mut()[0].comment = "Notes from the server".into();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.name = "Renamed".into();
        library.save_playlist(&mix).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.description, "Notes from the server");
        // Deleting it now deletes it on the server: nothing changed there since.
        library.delete_playlist(&now.id).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        assert!(fake.playlist("1").is_err());
        assert!(names(&library).is_empty());
    }

    #[test]
    fn no_full_sync_while_signing_out() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        signing_out(&library, "test-signing-out", true);
        assert_eq!(
            sync(&library, "test-signing-out", &fake, false).unwrap(),
            Report::default()
        );
        signing_out(&library, "test-signing-out", false);
        assert_eq!(
            sync(&library, "test-signing-out", &fake, false)
                .unwrap()
                .added,
            1
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

    fn copies(library: &Library) -> Vec<Playlist> {
        library
            .playlists()
            .unwrap()
            .into_iter()
            .filter(|p| p.name.ends_with("(Needle copy)"))
            .collect()
    }

    fn waiting_deletions(library: &Library) -> Vec<ServerPlaylist> {
        library
            .get_json(&deleted_key("subsonic"))
            .unwrap()
            .unwrap_or_default()
    }

    #[test]
    fn an_edit_that_reaches_no_server_waits_and_goes_later() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids.pop();
        library.save_playlist(&mix).unwrap();
        fake.cut.set(true);
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert!(report.waiting);
        assert!(
            report.notes.iter().all(|n| n.contains("wait")),
            "{report:?}"
        );
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert!(now.edited_here(), "the edit waits");
        assert_eq!(now.track_ids, ids("subsonic", &["a"]));
        assert!(
            copies(&library).is_empty(),
            "no copy for a dropped connection"
        );
        assert!(pending(&library, "subsonic").unwrap());
        // The server is back: the edit goes.
        fake.cut.set(false);
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.sent, 1);
        assert!(!report.waiting);
        assert_eq!(
            song_ids("subsonic", &fake.playlist("1").unwrap()),
            ids("subsonic", &["a"])
        );
        assert!(!pending(&library, "subsonic").unwrap());
    }

    #[test]
    fn a_deletion_that_reaches_no_server_waits_and_does_not_come_back() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Old", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "1"))
            .unwrap();
        fake.cut.set(true);
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert!(report.waiting);
        assert!(names(&library).is_empty(), "the deletion is not undone");
        assert_eq!(waiting_deletions(&library).len(), 1);
        fake.cut.set(false);
        sync(&library, "subsonic", &fake, false).unwrap();
        assert!(fake.playlist("1").is_err());
        assert!(names(&library).is_empty());
        assert!(waiting_deletions(&library).is_empty());
    }

    #[test]
    fn a_push_cut_off_between_songs_and_name_sends_the_name_next() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.name = "Better".into();
        mix.track_ids.pop();
        library.save_playlist(&mix).unwrap();
        fake.half.set(true);
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert!(report.waiting);
        assert!(copies(&library).is_empty());
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.name, "Better");
        let link = now.server.clone().unwrap();
        assert_eq!(link.name, "Mix", "the server still has the old name");
        assert_eq!(link.songs, ids("subsonic", &["a", "b"]), "the songs went");
        fake.half.set(false);
        sync(&library, "subsonic", &fake, true).unwrap();
        let there = fake.playlist("1").unwrap();
        assert_eq!(there.name, "Better");
        assert_eq!(song_ids("subsonic", &there), ids("subsonic", &["a", "b"]));
        assert!(copies(&library).is_empty());
        assert!(
            !server_list(&library, "subsonic", "1")
                .unwrap()
                .edited_here()
        );
    }

    #[test]
    fn a_push_the_server_did_not_keep_is_kept_as_a_copy() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids.pop();
        library.save_playlist(&mix).unwrap();
        fake.forgetful.set(true);
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.sent, 0);
        assert_eq!(report.notes.len(), 1, "{report:?}");
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.track_ids, ids("subsonic", &["a", "b"]));
        assert!(!now.edited_here());
        let kept = copies(&library);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].track_ids, ids("subsonic", &["a"]));
    }

    #[test]
    fn signing_out_and_in_again_brings_own_words_and_picture_back_without_copies() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Words", &["a"]));
        fake.lists.borrow_mut().push(remote("2", "Picture", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut words = server_list(&library, "subsonic", "1").unwrap();
        words.description = "My notes".into();
        library.save_playlist(&words).unwrap();
        let mut picture = server_list(&library, "subsonic", "2").unwrap();
        picture.cover = Some("/art/p.img".into());
        library.save_playlist(&picture).unwrap();
        for _ in 0..3 {
            assert_eq!(forget(&library, "subsonic").unwrap(), 2);
            assert!(names(&library).is_empty());
            sync(&library, "subsonic", &fake, false).unwrap();
        }
        assert_eq!(names(&library), ["Picture", "Words"]);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().description,
            "My notes"
        );
        assert_eq!(
            server_list(&library, "subsonic", "2")
                .unwrap()
                .cover
                .as_deref(),
            Some("/art/p.img")
        );
    }

    #[test]
    fn a_dead_server_stops_the_sync_after_three_tries() {
        let (_dir, library) = library();
        let fake = Fake::default();
        for i in 0..20 {
            fake.lists
                .borrow_mut()
                .push(remote(&i.to_string(), &format!("List {i}"), &["a"]));
        }
        sync(&library, "subsonic", &fake, false).unwrap();
        // Every one changed on the server, and then the connection drops.
        for list in fake.lists.borrow_mut().iter_mut() {
            list.name += " 2";
        }
        fake.cut.set(true);
        fake.asked.set(0);
        sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!(fake.asked.get(), STOP_AFTER);
    }

    #[test]
    fn signing_out_stops_a_sync_under_way() {
        let (dir, library) = library();
        let fake = Fake::default();
        for i in 0..10 {
            fake.lists
                .borrow_mut()
                .push(remote(&i.to_string(), &format!("List {i}"), &["a"]));
        }
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |_, id| {
            if id == "2" {
                signing_out(&Library::open(&path).unwrap(), "subsonic", true);
            }
        }));
        sync(&library, "subsonic", &fake, false).unwrap();
        signing_out(&library, "subsonic", false);
        assert_eq!(
            fake.asked.get(),
            3,
            "no playlist asked for after the sign-out began"
        );
    }

    #[test]
    fn a_playlist_that_did_not_change_is_not_read_again() {
        let (_dir, library) = library();
        let fake = Fake::default();
        let mut one = remote("1", "Mix", &["a", "b"]);
        one.changed = "2026-11-01T10:00:00Z".into();
        fake.lists.borrow_mut().push(one);
        sync(&library, "subsonic", &fake, false).unwrap();
        fake.asked.set(0);
        assert!(!sync(&library, "subsonic", &fake, false).unwrap().changed());
        assert_eq!(fake.asked.get(), 0);
        // Changed there: read again.
        {
            let mut lists = fake.lists.borrow_mut();
            lists[0].name = "Mix 2".into();
            lists[0].changed = "2026-11-02T10:00:00Z".into();
        }
        sync(&library, "subsonic", &fake, false).unwrap();
        assert_eq!(fake.asked.get(), 1);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().name,
            "Mix 2"
        );
    }

    #[test]
    fn songs_a_refresh_took_out_meanwhile_are_no_edit() {
        let (dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        // Song c left the server, and its playlist.
        {
            let mut lists = fake.lists.borrow_mut();
            lists[0].name = "Renamed there".into();
            lists[0].songs.pop();
        }
        // While the sync asks for it, a refresh takes song c out of the library and its
        // playlists (as `sources::apply_with` does).
        let path = dir.path().to_path_buf();
        *fake.during.borrow_mut() = Some(Box::new(move |what, _| {
            if what == "playlist" {
                let other = Library::open(&path).unwrap();
                let c = sources::track_id("subsonic", "c");
                other
                    .connection()
                    .unwrap()
                    .execute("DELETE FROM tracks WHERE id=?", [&c])
                    .unwrap();
                let mut list = other
                    .playlist_by_id(&playlist_id("subsonic", "1"))
                    .unwrap()
                    .unwrap();
                list.track_ids.retain(|id| *id != c);
                if let Some(link) = &mut list.server {
                    link.songs.retain(|id| *id != c);
                }
                other.store_playlist(&list).unwrap();
            }
        }));
        let report = sync(&library, "subsonic", &fake, false).unwrap();
        assert!(report.notes.is_empty(), "{report:?}");
        assert!(copies(&library).is_empty());
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().name,
            "Renamed there"
        );
    }

    #[test]
    fn a_server_playlist_an_older_needle_unlinked_is_linked_again() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        fake.lists.borrow_mut().push(remote("2", "Same", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        // An older Needle saved them without the link, one with a change.
        for (id, name) in [("1", "Mine now"), ("2", "Same")] {
            let mut list = server_list(&library, "subsonic", id).unwrap();
            list.server = None;
            list.name = name.into();
            library.store_playlist(&list).unwrap();
        }
        sync(&library, "subsonic", &fake, false).unwrap();
        for id in ["1", "2"] {
            let list = server_list(&library, "subsonic", id).unwrap();
            assert!(list.server.is_some() && !list.edited_here());
        }
        assert_eq!(server_list(&library, "subsonic", "1").unwrap().name, "Mix");
        // The changed one is kept, once; the same one makes no copy.
        let kept = copies(&library);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "Mine now (Needle copy)");
    }

    #[test]
    fn entries_needle_cannot_read_stay_on_the_server() {
        let (_dir, library) = library();
        let fake = Fake::default();
        let mut list = remote("1", "Mix", &["a", "b"]);
        list.unread = vec![(1, "untitled".into())];
        fake.lists.borrow_mut().push(list);
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        mix.track_ids.push(sources::track_id("subsonic", "a"));
        library.save_playlist(&mix).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(
            fake.playlist("1").unwrap().entries(),
            ["a", "untitled", "b", "a"]
        );
        assert!(
            !server_list(&library, "subsonic", "1")
                .unwrap()
                .edited_here()
        );
    }

    #[test]
    fn an_unnamed_server_playlist_is_not_renamed_by_a_push() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", " ", &["a", "b"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut mix = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(mix.name, "Untitled playlist");
        mix.track_ids.pop();
        library.save_playlist(&mix).unwrap();
        let report = sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(report.sent, 1);
        assert_eq!(fake.playlist("1").unwrap().name, " ");
    }

    #[test]
    fn deletions_waiting_survive_signing_out_but_not_another_account() {
        let (_dir, library) = library();
        let fake = Fake::default();
        *fake.account.borrow_mut() = "https://music.example|me".into();
        fake.lists.borrow_mut().push(remote("1", "Old", &["a"]));
        fake.lists.borrow_mut().push(remote("2", "Older", &["a"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        library
            .delete_playlist(&playlist_id("subsonic", "1"))
            .unwrap();
        forget(&library, "subsonic").unwrap();
        assert_eq!(waiting_deletions(&library).len(), 1);
        // The same server and person again: the deletion goes.
        sync(&library, "subsonic", &fake, false).unwrap();
        assert!(fake.playlist("1").is_err());
        library
            .delete_playlist(&playlist_id("subsonic", "2"))
            .unwrap();
        forget(&library, "subsonic").unwrap();
        // Another person: what waited was not theirs.
        *fake.account.borrow_mut() = "https://music.example|you".into();
        sync(&library, "subsonic", &fake, false).unwrap();
        assert!(fake.playlist("2").is_ok());
        assert!(waiting_deletions(&library).is_empty());
        assert_eq!(names(&library), ["Older"]);
    }

    #[test]
    fn a_read_only_playlist_is_never_changed_or_deleted_on_the_server() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Theirs", &["a", "b"]));
        fake.read_only.borrow_mut().insert("1".into());
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut list = server_list(&library, "subsonic", "1").unwrap();
        assert!(list.server.as_ref().unwrap().read_only);
        // An edit is saved as the person's own, and says so.
        list.track_ids.pop();
        list.description = "Notes".into();
        let saved = library.save_playlist_saved(&list).unwrap();
        let copy = saved.copy.clone().unwrap();
        assert_eq!(copy.name, "Theirs (Needle copy)");
        assert!(saved.note.unwrap().contains("read-only"));
        let now = server_list(&library, "subsonic", "1").unwrap();
        assert_eq!(now.track_ids, ids("subsonic", &["a", "b"]));
        assert_eq!(now.description, "Notes", "its own words stay");
        assert!(!now.edited_here());
        assert_eq!(
            library.playlist_by_id(&copy.id).unwrap().unwrap().track_ids,
            ids("subsonic", &["a"])
        );
        // Not deleted, not pushed.
        assert!(library.delete_playlist(&now.id).is_err());
        sync(&library, "subsonic", &fake, true).unwrap();
        assert_eq!(*fake.saved.borrow(), 0);
    }

    #[test]
    fn a_save_that_became_a_copy_says_so() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists
            .borrow_mut()
            .push(remote("1", "Mix", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        fake.lists.borrow_mut()[0].songs.push(song("d"));
        sync(&library, "subsonic", &fake, false).unwrap();
        open.track_ids.pop();
        let saved = library.save_playlist_saved(&open).unwrap();
        assert_eq!(saved.id, playlist_id("subsonic", "1"));
        assert_eq!(saved.copy.unwrap().name, "Mix (Needle copy)");
        assert!(saved.note.unwrap().contains("changed on the server"));
        // A plain save says nothing.
        let mut now = server_list(&library, "subsonic", "1").unwrap();
        now.name = "Renamed".into();
        let saved = library.save_playlist_saved(&now).unwrap();
        assert_eq!((saved.copy, saved.note), (None, None));
    }

    #[test]
    fn a_save_read_before_needles_own_push_is_no_clash() {
        let (_dir, library) = library();
        let fake = Fake::default();
        fake.lists.borrow_mut().push(remote("1", "Mix", &["a"]));
        fake.lists
            .borrow_mut()
            .push(remote("2", "All", &["a", "b", "c"]));
        sync(&library, "subsonic", &fake, false).unwrap();
        // An editor (keeping the link it read) adds b, which is pushed; then it adds c.
        let mut open = server_list(&library, "subsonic", "1").unwrap();
        open.track_ids.push(sources::track_id("subsonic", "b"));
        library.save_playlist(&open).unwrap();
        sync(&library, "subsonic", &fake, true).unwrap();
        open.track_ids.push(sources::track_id("subsonic", "c"));
        let saved = library.save_playlist_saved(&open).unwrap();
        assert_eq!(saved.copy, None);
        assert_eq!(
            server_list(&library, "subsonic", "1").unwrap().track_ids,
            ids("subsonic", &["a", "b", "c"])
        );
        assert!(copies(&library).is_empty());
    }
}
