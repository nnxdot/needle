//! The sidebar's playlist pictures: a playlist's own picture, or a mosaic of four of its
//! songs' covers (one cover when it has fewer). The covers are gathered on another thread,
//! and again only when the playlists change, so drawing the sidebar reads nothing.

use super::widgets::{cover, track_seed};
use super::{AppView, Event};
use gpui::{prelude::*, *};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// How many songs of a playlist are looked at for covers.
const LOOKED_AT: usize = 60;

impl AppView {
    /// Gathers the covers again when a playlist was added, changed, or removed (from `poll`).
    pub(super) fn follow_playlist_art(&mut self) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for playlist in &self.playlists {
            (
                &playlist.id,
                playlist.updated_at,
                &playlist.cover,
                &playlist.query,
            )
                .hash(&mut hasher);
        }
        let key = hasher.finish();
        if key == self.playlist_art_key {
            return;
        }
        self.playlist_art_key = key;
        let (library, sender, playlists) = (
            self.library.clone(),
            self.sender.clone(),
            self.playlists.clone(),
        );
        std::thread::spawn(move || {
            let mut art = HashMap::new();
            for playlist in playlists {
                let covers = match &playlist.cover {
                    Some(picture) => vec![picture.clone()],
                    None => {
                        let tracks = match &playlist.query {
                            Some(query) => library
                                .search_page(query, 0, LOOKED_AT)
                                .map(|page| page.tracks),
                            None => library.tracks_by_ids(
                                &playlist.track_ids[..playlist.track_ids.len().min(LOOKED_AT)],
                            ),
                        }
                        .unwrap_or_default();
                        // Different albums first, so the four squares differ.
                        let mut seen = HashSet::new();
                        tracks
                            .iter()
                            .filter(|t| t.artwork.is_some() && seen.insert(track_seed(t)))
                            .filter_map(|t| t.artwork.clone())
                            .take(4)
                            .collect()
                    }
                };
                art.insert(playlist.id, covers);
            }
            let _ = sender.send(Event::PlaylistArt(art));
        });
    }

    /// The picture for a playlist in the sidebar, `size` points wide, or none (its icon).
    pub(super) fn playlist_icon(&self, id: &str, size: f32, cx: &App) -> Option<AnyElement> {
        let covers = self.playlist_art.get(id)?;
        let radius = px((size * 0.18).clamp(3., 6.));
        match covers.len() {
            0 => None,
            1..=3 => Some(
                div()
                    .flex_shrink_0()
                    .rounded(radius)
                    .overflow_hidden()
                    .child(cover(Some(&covers[0]), "", size, cx))
                    .into_any_element(),
            ),
            _ => {
                let half = size / 2.;
                let tile = |i: usize| cover(Some(&covers[i]), "", half, cx);
                Some(
                    div()
                        .flex_shrink_0()
                        .size(px(size))
                        .rounded(radius)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(div().flex().child(tile(0)).child(tile(1)))
                        .child(div().flex().child(tile(2)).child(tile(3)))
                        .into_any_element(),
                )
            }
        }
    }
}
