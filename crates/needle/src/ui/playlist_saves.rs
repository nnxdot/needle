//! Playlist saves (and deletions) off the window's thread. Saving a playlist can wait for a
//! sync with a music server (the core lets one playlist write through at a time), and the
//! window must not stop while it waits. Saves go to one worker thread and run there one at a time, in the order
//! they were made; their results come back in that order (`drain_playlist_saves`, from the
//! poll). Meanwhile the window shows each change at once in its own copy of the playlists, so
//! the next change builds on it; it reads them again from the library when no save waits.
use super::{AppView, Page};
use gpui::{Context, Window};
use needle_core::{
    database::{Library, Saved},
    model::Playlist,
};
use std::collections::VecDeque;

/// What to do when a save is done: the result says where it went (`Saved`: the id it was
/// saved under, a copy the change went to instead, and a note; a deletion gives the deleted
/// one's id, a year's playlist its name as the id), or the error.
type Done =
    Box<dyn FnOnce(&mut AppView, Result<Saved, String>, &mut Window, &mut Context<AppView>)>;

/// The save's note for the person, as a sentence ("“Mix” is read-only on the server; your
/// version is kept as “Mix 2”."), when it did not go just as asked.
pub(super) fn saved_note(saved: &Saved) -> Option<String> {
    let note = saved.note.as_deref()?.trim();
    let mut chars = note.chars();
    let first = chars.next()?;
    let end = if note.ends_with(['.', '!', '?']) {
        ""
    } else {
        "."
    };
    Some(format!("{}{}{end}", first.to_uppercase(), chars.as_str()))
}

/// The name of the playlist the change landed in: the copy's when it went to a copy.
pub(super) fn landed_name(saved: &Saved, name: &str) -> String {
    saved
        .copy
        .as_ref()
        .map_or_else(|| name.to_string(), |c| c.name.clone())
}

/// `message`, then the save's note after it when there is one.
pub(super) fn with_note(message: String, saved: &Saved) -> String {
    match saved_note(saved) {
        Some(note) => format!("{message} {note}"),
        None => message,
    }
}

/// A save: the playlist as shown when the change was made (`None` for a new one), and after;
/// or a deletion.
enum Job {
    Save(Box<(Option<Playlist>, Playlist)>),
    Delete(String),
    /// The playlist of a year's most played songs (Wrapped).
    Year(i32),
}

pub struct PlaylistSaves {
    jobs: crossbeam_channel::Sender<Job>,
    results: crossbeam_channel::Receiver<Result<Saved, String>>,
    /// What to do with each result, in the order the saves were made.
    waiting: VecDeque<Done>,
}

impl PlaylistSaves {
    pub fn new(library: Library) -> Self {
        let (jobs, inbox) = crossbeam_channel::unbounded::<Job>();
        let (outbox, results) = crossbeam_channel::unbounded();
        std::thread::Builder::new()
            .name("playlist-saves".into())
            .spawn(move || {
                for job in inbox {
                    let only = |id: String| Saved {
                        id,
                        ..Default::default()
                    };
                    let saved = match job {
                        Job::Save(save) => match *save {
                            (Some(before), after) => {
                                library.save_playlist_changes_saved(&before, &after)
                            }
                            (None, after) => library.save_playlist_saved(&after),
                        },
                        Job::Delete(id) => library.delete_playlist(&id).map(|()| only(id)),
                        Job::Year(year) => library.save_year_playlist(year).map(|p| only(p.name)),
                    };
                    if outbox.send(saved.map_err(|e| format!("{e:#}"))).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self {
            jobs,
            results,
            waiting: VecDeque::new(),
        }
    }

    /// Saves made and not yet done.
    pub fn pending(&self) -> bool {
        !self.waiting.is_empty()
    }
}

impl AppView {
    /// Save a change to a playlist in the background, after the saves before it. `before` is
    /// the playlist as shown when the change was made (only what changed is saved, see
    /// `Library::save_playlist_changes`), or `None` for a new playlist. The window's copy
    /// shows `after` at once; `done` runs when it is saved.
    pub(super) fn save_playlist_later(
        &mut self,
        before: Option<Playlist>,
        after: Playlist,
        done: impl FnOnce(&mut AppView, Result<Saved, String>, &mut Window, &mut Context<AppView>)
        + 'static,
    ) {
        match self.playlists.iter_mut().find(|p| p.id == after.id) {
            Some(shown) => *shown = after.clone(),
            None if before.is_none() => self.playlists.push(after.clone()),
            None => {}
        }
        // A playlist page in its own order shows the change at once (songs added from
        // elsewhere come with the next refresh).
        if matches!(&self.page, Page::Playlist(id) if *id == after.id) && after.query.is_none() {
            let place: std::collections::HashMap<&str, usize> = after
                .track_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();
            self.tracks.retain(|t| place.contains_key(t.id.as_str()));
            if self.sort == super::Sort::Default {
                self.tracks.sort_by_key(|t| place[t.id.as_str()]);
            }
        }
        self.queue_playlist_job(Job::Save(Box::new((before, after))), Box::new(done));
    }

    fn queue_playlist_job(&mut self, job: Job, done: Done) {
        if self.playlist_saves.jobs.send(job).is_err() {
            self.fail("Playlists cannot be saved now. Restart Needle and try again.");
            return;
        }
        self.playlist_saves.waiting.push_back(done);
    }

    /// Save (or bring up to date) a year's playlist of its most played songs, in the background.
    pub(super) fn save_year_playlist_later(&mut self, year: i32) {
        self.queue_playlist_job(
            Job::Year(year),
            Box::new(|this, saved, _, cx| {
                match saved {
                    Ok(saved) => this.notify(format!("Saved “{}”.", saved.id)),
                    Err(e) => this.fail(e),
                }
                cx.notify();
            }),
        );
    }

    /// Delete a playlist in the background, after the saves before it. It leaves the window's
    /// copy at once, and its page goes to `leave_to`; the notice comes when it is deleted.
    pub(super) fn delete_playlist_later(
        &mut self,
        id: String,
        on_server: bool,
        leave_to: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playlists.retain(|p| p.id != id);
        let gone = Page::Playlist(id.clone());
        self.back.retain(|page| *page != gone);
        self.forward.retain(|page| *page != gone);
        if self.page == gone {
            self.back.clear();
            self.open(leave_to, window, cx);
        }
        self.queue_playlist_job(
            Job::Delete(id),
            Box::new(move |this, deleted, _, cx| {
                match deleted {
                    Ok(_) => this.notify(if on_server {
                        "Playlist deleted, here and on the server. The songs stay."
                    } else {
                        "Playlist deleted. Your music files are untouched."
                    }),
                    Err(e) => this.fail(e),
                }
                cx.notify();
            }),
        );
    }

    /// Take the results of the saves that are done, in order. When none waits any more, the
    /// playlists are read again from the library. Returns whether anything came.
    pub(super) fn drain_playlist_saves(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut any = false;
        while let Ok(result) = self.playlist_saves.results.try_recv() {
            any = true;
            let Some(done) = self.playlist_saves.waiting.pop_front() else {
                continue;
            };
            if !self.playlist_saves.pending() {
                self.playlists = self.library.playlists().unwrap_or_default();
            }
            done(self, result, window, cx);
        }
        any
    }

    /// A save of the playlist `old` landed elsewhere: under another id (a server playlist
    /// that left Needle is kept as the person's own), or the change went to a copy (the
    /// server's playlist changed meanwhile, or it is read-only there). The playlist's page
    /// and an editor open on it follow to where the change is. Returns that id.
    pub(super) fn follow_saved_playlist(
        &mut self,
        old: &str,
        saved: &Saved,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> String {
        let new = saved
            .copy
            .as_ref()
            .map_or_else(|| saved.id.clone(), |c| c.id.clone());
        if old == new {
            return new;
        }
        // An editor open on it now edits the playlist the change is in, as stored.
        if let Some(editor) = self.editor.as_mut() {
            editor.follow(old, self.library.playlist_by_id(&new).ok().flatten());
        }
        if self.page == Page::Playlist(old.to_string()) {
            self.open(Page::Playlist(new.clone()), window, cx);
        }
        // Moved under a new id, the old one is gone: Back and Forward follow too. A copy
        // leaves the old one there.
        if saved.copy.is_none() {
            for page in self.back.iter_mut().chain(self.forward.iter_mut()) {
                if *page == Page::Playlist(old.to_string()) {
                    *page = Page::Playlist(new.clone());
                }
            }
        }
        new
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that would bring in GPUI's own `test` attribute in place of Rust's.
    use super::{Job, PlaylistSaves, landed_name, saved_note, with_note};
    use needle_core::{
        database::{Library, Saved, SavedCopy},
        model::Playlist,
    };

    fn songs(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// Quick edits, each made on the one before it, are saved in the order they were made
    /// and none is lost, though the window did not wait for any of them.
    #[test]
    fn saves_run_in_order() {
        let data = tempfile::tempdir().unwrap();
        let library = Library::open(data.path()).unwrap();
        let saves = PlaylistSaves::new(library.clone());
        let made = Playlist {
            id: "list".into(),
            name: "Mine".into(),
            query: None,
            track_ids: songs(&["a", "b", "c"]),
            updated_at: 1,
            description: String::new(),
            cover: None,
            rules: None,
            server: None,
        };
        let moved = Playlist {
            track_ids: songs(&["c", "a", "b"]),
            ..made.clone()
        };
        let removed = Playlist {
            track_ids: songs(&["c", "b"]),
            ..moved.clone()
        };
        let added = Playlist {
            track_ids: songs(&["c", "b", "d"]),
            ..removed.clone()
        };
        for job in [
            Job::Save(Box::new((None, made.clone()))),
            Job::Save(Box::new((Some(made), moved.clone()))),
            Job::Save(Box::new((Some(moved), removed.clone()))),
            Job::Save(Box::new((Some(removed), added.clone()))),
        ] {
            saves.jobs.send(job).unwrap();
        }
        for _ in 0..4 {
            let result = saves
                .results
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            assert_eq!(result.map(|saved| saved.id).as_deref(), Ok("list"));
        }
        let stored = library.playlist_by_id("list").unwrap().unwrap();
        assert_eq!(stored.track_ids, added.track_ids);
    }

    /// The words after a save that went somewhere else say where.
    #[test]
    fn notes_say_where_the_change_went() {
        let plain = Saved {
            id: "a".into(),
            ..Default::default()
        };
        assert_eq!(saved_note(&plain), None);
        assert_eq!(landed_name(&plain, "Mix"), "Mix");
        assert_eq!(
            with_note("Added 1 track to “Mix”.".into(), &plain),
            "Added 1 track to “Mix”."
        );
        let copied = Saved {
            id: "a".into(),
            copy: Some(SavedCopy {
                id: "b".into(),
                name: "Mix 2".into(),
            }),
            note: Some("“Mix” is read-only on the server; your version is kept as “Mix 2”".into()),
        };
        assert_eq!(landed_name(&copied, "Mix"), "Mix 2");
        assert_eq!(
            with_note("Added 1 track to “Mix 2”.".into(), &copied),
            "Added 1 track to “Mix 2”. “Mix” is read-only on the server; your version is kept as “Mix 2”."
        );
        let own = Saved {
            id: "c".into(),
            copy: None,
            note: Some("your version is kept as “Mix 2”".into()),
        };
        assert_eq!(
            saved_note(&own).as_deref(),
            Some("Your version is kept as “Mix 2”.")
        );
    }
}
