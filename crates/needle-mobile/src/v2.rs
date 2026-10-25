//! Needle 2.0 on Android: what its Home asks for beside the shared 2.0 calls (core2.rs).
//! Only additions; the older calls stay as they are for the iPhone.
use crate::{Album, Artist, Needle, Result};

/// Home's shelves, as the Mac's Home has them.
#[derive(Clone, uniffi::Record)]
pub struct HomeShelves {
    /// Albums played most recently, newest first.
    pub recent: Vec<Album>,
    /// Albums most recently added.
    pub added: Vec<Album>,
    /// Albums with the most favorite songs.
    pub favorites: Vec<Album>,
    /// Albums with the most plays.
    pub most_played: Vec<Album>,
    /// Artists with the most plays.
    pub artists: Vec<Artist>,
}

#[uniffi::export]
impl Needle {
    /// Home's shelves: albums played lately, new, loved, and most played, and the artists
    /// played most.
    pub fn home_shelves(&self) -> Result<HomeShelves> {
        let home = self.library.home(20)?;
        let albums = |list: Vec<needle_core::browse::AlbumSummary>| list.into_iter().map(Album::from).collect();
        Ok(HomeShelves {
            recent: albums(home.recent),
            added: albums(home.added),
            favorites: albums(home.favorites),
            most_played: albums(home.most_played),
            artists: home
                .artists
                .into_iter()
                .map(|a| Artist {
                    name: a.name,
                    albums: a.albums as u32,
                    songs: a.tracks as u32,
                    artwork: a.artwork,
                    letter: a.letter,
                    plays: a.play_count,
                })
                .collect(),
        })
    }
}
