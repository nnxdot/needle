//! An artist's photo, logo, and wide picture beyond the automatic ones: a picture someone
//! chose (their own file, or another of fanart.tv's), pictures kept in the artist's own folder
//! (as Kodi and Jellyfin keep them), and a slow sweep that finds every artist's pictures in the
//! background.
use super::{
    IMAGE_LIMIT, WEEK, artist_banner, artist_image, artist_logo, cached, download_image, fanart,
    remember,
};
use crate::database::Library;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// One of an artist's three pictures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtistPicture {
    Photo,
    Logo,
    /// The wide picture across the top of their page.
    Banner,
}

impl ArtistPicture {
    /// The kept file's extension (as the automatic pictures are kept).
    fn extension(self) -> &'static str {
        match self {
            Self::Photo => "img",
            Self::Logo => "logo",
            Self::Banner => "banner",
        }
    }

    /// fanart.tv's kinds of picture for it, best first.
    fn fanart_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Photo => &["artistthumb"],
            Self::Logo => &["hdmusiclogo", "musiclogo"],
            Self::Banner => &["artistbackground", "musicbanner"],
        }
    }

    /// Names of files in an artist's folder that hold it, in the order they are looked for.
    /// `folder.*` counts only in a folder named after the artist (it is an album's cover in an
    /// album's folder).
    fn folder_names(self) -> &'static [&'static str] {
        match self {
            Self::Photo => &["artist", "folder"],
            Self::Logo => &["logo", "clearlogo"],
            Self::Banner => &["fanart", "backdrop", "banner"],
        }
    }
}

/// Where the automatic picture of this kind is kept.
pub(super) fn kept_file(library: &Library, name: &str, kind: ArtistPicture) -> PathBuf {
    library
        .directory
        .join("artwork")
        .join("artists")
        .join(format!(
            "{}.{}",
            blake3::hash(name.to_lowercase().as_bytes()).to_hex(),
            kind.extension()
        ))
}

/// Where a chosen picture is kept: beside the automatic one, never replaced by it.
fn own_file(library: &Library, name: &str, kind: ArtistPicture) -> PathBuf {
    let auto = kept_file(library, name, kind);
    auto.with_extension(format!("{}.own", kind.extension()))
}

/// The picture someone chose for this artist, if they chose one.
pub fn own_picture(library: &Library, name: &str, kind: ArtistPicture) -> Option<PathBuf> {
    let file = own_file(library, name, kind);
    file.is_file().then_some(file)
}

/// Whether the bytes are a picture Needle shows: JPEG, PNG, WebP, or GIF.
fn is_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || (bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
        || bytes.starts_with(b"GIF8")
}

/// Use a picture file as this artist's photo, logo, or wide picture. Needle keeps its own copy,
/// and the monthly lookups never replace it.
pub fn set_own_picture(
    library: &Library,
    name: &str,
    kind: ArtistPicture,
    from: &Path,
) -> Result<PathBuf> {
    if name.trim().is_empty() {
        bail!("The artist has no name");
    }
    let size = std::fs::metadata(from)
        .context("The picture could not be read")?
        .len();
    if size > IMAGE_LIMIT {
        bail!("The picture is larger than 12 MB");
    }
    let bytes = std::fs::read(from).context("The picture could not be read")?;
    if !is_picture(&bytes) {
        bail!("The file is not a JPEG, PNG, WebP, or GIF picture");
    }
    let file = own_file(library, name, kind);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let partial = file.with_extension("own.part");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, &file)?;
    Ok(file)
}

/// Forget the chosen picture: the automatic one (or the folder's) shows again.
pub fn reset_picture(library: &Library, name: &str, kind: ArtistPicture) -> Result<()> {
    let file = own_file(library, name, kind);
    if file.is_file() {
        std::fs::remove_file(file)?;
    }
    Ok(())
}

/// One of fanart.tv's pictures of an artist, to choose from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FanartChoice {
    /// The full picture.
    pub url: String,
    /// A small copy, for the list to choose from.
    pub preview: String,
    pub likes: i64,
    /// Its language ("en", or "" for none), for logos with words.
    pub language: String,
}

/// Every picture fanart.tv has of this kind for the artist, best first. Empty without online
/// lookups or a fanart.tv key.
pub fn fanart_choices(
    library: &Library,
    name: &str,
    kind: ArtistPicture,
    online: bool,
) -> Result<Vec<FanartChoice>> {
    let Some(data) = fanart(library, name, online)? else {
        return Ok(vec![]);
    };
    Ok(choices_in(&data, kind))
}

fn choices_in(data: &Value, kind: ArtistPicture) -> Vec<FanartChoice> {
    let mut found = vec![];
    for (rank, fanart_kind) in kind.fanart_kinds().iter().enumerate() {
        for picture in data[*fanart_kind].as_array().into_iter().flatten() {
            let Some(url) = picture["url"].as_str().map(https) else {
                continue;
            };
            if !url.starts_with("https://assets.fanart.tv/") {
                continue;
            }
            let likes = picture["likes"]
                .as_str()
                .and_then(|n| n.parse::<i64>().ok())
                .unwrap_or(0);
            found.push((
                rank,
                FanartChoice {
                    preview: url.replacen("/fanart/", "/preview/", 1),
                    url,
                    likes,
                    language: picture["lang"].as_str().unwrap_or("").to_string(),
                },
            ));
        }
    }
    found.sort_by_key(|(rank, c)| (*rank, -c.likes));
    found.into_iter().map(|(_, c)| c).collect()
}

/// fanart.tv's answers can give pictures as http://: the same, over https.
fn https(url: &str) -> String {
    match url.strip_prefix("http://assets.fanart.tv/") {
        Some(rest) => format!("https://assets.fanart.tv/{rest}"),
        None => url.to_string(),
    }
}

/// Use one of fanart.tv's pictures (from [`fanart_choices`]) as the artist's: kept as a choice,
/// never replaced.
pub fn pick_fanart(
    library: &Library,
    name: &str,
    kind: ArtistPicture,
    url: &str,
) -> Result<PathBuf> {
    let url = https(url);
    if !url.starts_with("https://assets.fanart.tv/") {
        bail!("Only fanart.tv's own pictures can be picked");
    }
    let file = own_file(library, name, kind);
    download_image(&url, &file)?;
    Ok(file)
}

/// A small copy of one of fanart.tv's pictures (a [`FanartChoice`]'s `preview`), kept for the
/// list to choose from.
pub fn fanart_preview(library: &Library, url: &str) -> Result<PathBuf> {
    let url = https(url);
    if !url.starts_with("https://assets.fanart.tv/") {
        bail!("Only fanart.tv's own pictures are shown");
    }
    let file = library
        .directory
        .join("artwork")
        .join("artists")
        .join("previews")
        .join(format!("{}.img", blake3::hash(url.as_bytes()).to_hex()));
    if !file.is_file() {
        download_image(&url, &file)?;
    }
    Ok(file)
}

/// Lowercase, accents folded, without a leading "The ", for matching a folder to an artist.
fn plain(name: &str) -> String {
    crate::artists::sort_key(name, None)
}

/// A picture of this kind in the artist's own folder: the folder above their albums' folders
/// (Music/Artist/Album/song.flac), when it is named after them, or any folder above their
/// albums that holds an `artist.*` picture. Looked for again each day.
pub fn folder_picture(library: &Library, name: &str, kind: ArtistPicture) -> Option<PathBuf> {
    let key = format!("artist-folder:{}:{}", kind.extension(), name.to_lowercase());
    if let Ok(Some(found)) = cached::<Option<String>>(library, &key) {
        return found.map(PathBuf::from).filter(|p| p.is_file());
    }
    let found = find_in_folders(library, name, kind);
    let _ = remember(
        library,
        &key,
        &found.as_ref().map(|p| p.to_string_lossy().to_string()),
        86_400,
    );
    found
}

fn find_in_folders(library: &Library, name: &str, kind: ArtistPicture) -> Option<PathBuf> {
    let songs = library.credited(name).ok()?;
    let db = library.connection().ok()?;
    let mut stmt = db.prepare("SELECT path FROM tracks WHERE id=?1").ok()?;
    let mut folders: Vec<PathBuf> = vec![];
    // Their own songs (not those they are a guest on) say where their folder is.
    for song in songs
        .iter()
        .filter(|s| {
            crate::artists::credits(&s.album_artist)
                .first()
                .is_some_and(|a| a.eq_ignore_ascii_case(name))
                || crate::artists::credits(&s.artist)
                    .first()
                    .is_some_and(|a| a.eq_ignore_ascii_case(name))
        })
        .take(200)
    {
        let Ok(path) = stmt.query_row([&song.id], |r| r.get::<_, String>(0)) else {
            continue;
        };
        if path.starts_with(crate::sources::SCHEME) {
            continue;
        }
        let file = PathBuf::from(path.split('#').next().unwrap_or(&path));
        // The album's folder, then the one above it.
        for folder in file.ancestors().skip(1).take(2) {
            if !folders.iter().any(|f| f == folder) {
                folders.push(folder.to_path_buf());
            }
        }
    }
    let wanted = plain(name);
    for folder in &folders {
        let named = folder
            .file_name()
            .is_some_and(|f| plain(&f.to_string_lossy()) == wanted);
        for stem in kind.folder_names() {
            // `folder.*` is an album cover unless the folder is the artist's own.
            if (*stem == "folder" || kind != ArtistPicture::Photo) && !named {
                continue;
            }
            for extension in ["jpg", "jpeg", "png", "webp"] {
                let file = folder.join(format!("{stem}.{extension}"));
                if file.is_file() && std::fs::metadata(&file).is_ok_and(|m| m.len() <= IMAGE_LIMIT)
                {
                    return Some(file);
                }
            }
        }
    }
    None
}

/// What the background sweep is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SweepProgress {
    /// Artists whose pictures have been looked for (in the last four weeks).
    pub done: usize,
    pub total: usize,
    /// It may look now (online lookups and the setting on, and the device allows it).
    pub allowed: bool,
    /// "Check all now" was pressed: no waiting between artists beyond the services' own pace.
    pub hurry: bool,
}

/// How long the sweep waits between artists, unhurried: slow enough to go unnoticed.
const PACE: Duration = Duration::from_secs(20);
/// Between artists when hurried: MusicBrainz allows one question a second, and an artist takes
/// a few.
const HURRIED: Duration = Duration::from_millis(1500);

fn sweep_key(name: &str) -> String {
    format!("artist-sweep:{}", name.to_lowercase())
}

/// Finds every artist's photo, logo, and wide picture, one artist at a time, slowly. Each app
/// starts one and tells it when it may look (online lookups and the Artist pictures setting
/// on; on a phone, on Wi-Fi; not while a library scan runs).
pub struct PictureSweep {
    state: Arc<SweepState>,
}

struct SweepState {
    allowed: AtomicBool,
    hurry: AtomicBool,
    stop: AtomicBool,
    done: AtomicUsize,
    total: AtomicUsize,
    /// Wakes the thread early (allowed again, hurried, or stopped).
    wake: Mutex<Option<std::thread::Thread>>,
}

impl PictureSweep {
    pub fn start(library: Library) -> Self {
        let state = Arc::new(SweepState {
            allowed: AtomicBool::new(false),
            hurry: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            done: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            wake: Mutex::new(None),
        });
        let worker = state.clone();
        let handle = std::thread::Builder::new()
            .name("artist-pictures".into())
            .spawn(move || sweep(library, worker))
            .ok();
        if let Some(handle) = handle {
            *state.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.thread().clone());
        }
        Self { state }
    }

    fn wake(&self) {
        if let Some(thread) = &*self.state.wake.lock().unwrap_or_else(|e| e.into_inner()) {
            thread.unpark();
        }
    }

    /// Whether it may look now.
    pub fn set_allowed(&self, allowed: bool) {
        if self.state.allowed.swap(allowed, Ordering::SeqCst) != allowed {
            self.wake();
        }
    }

    /// Look for every artist now, at the services' own pace, then go slowly again.
    pub fn check_all_now(&self) {
        self.state.hurry.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub fn progress(&self) -> SweepProgress {
        SweepProgress {
            done: self.state.done.load(Ordering::Relaxed),
            total: self.state.total.load(Ordering::Relaxed),
            allowed: self.state.allowed.load(Ordering::Relaxed),
            hurry: self.state.hurry.load(Ordering::Relaxed),
        }
    }
}

impl Drop for PictureSweep {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        self.wake();
    }
}

fn sweep(library: Library, state: Arc<SweepState>) {
    // Let the app start before the first look.
    std::thread::park_timeout(Duration::from_secs(30));
    let mut names: Vec<String> = vec![];
    let mut listed = Instant::now() - Duration::from_secs(3600);
    while !state.stop.load(Ordering::SeqCst) {
        if !state.allowed.load(Ordering::SeqCst) {
            std::thread::park_timeout(Duration::from_secs(60));
            continue;
        }
        // The list of artists, again every ten minutes (the library changes).
        if listed.elapsed() > Duration::from_secs(600) || names.is_empty() {
            names = library
                .artists("")
                .map(|a| a.into_iter().map(|a| a.name).collect())
                .unwrap_or_default();
            listed = Instant::now();
        }
        let waiting: Vec<&String> = names
            .iter()
            .filter(|n| {
                cached::<bool>(&library, &sweep_key(n))
                    .ok()
                    .flatten()
                    .is_none()
            })
            .collect();
        state.total.store(names.len(), Ordering::Relaxed);
        state
            .done
            .store(names.len() - waiting.len(), Ordering::Relaxed);
        let Some(name) = waiting.first().map(|n| n.to_string()) else {
            state.hurry.store(false, Ordering::SeqCst);
            // All done: look again in an hour for new artists and month-old answers.
            std::thread::park_timeout(Duration::from_secs(3600));
            listed = Instant::now() - Duration::from_secs(3600);
            continue;
        };
        let mut failed = false;
        for result in [
            artist_image(&library, &name, true).map(|_| ()),
            artist_logo(&library, &name, true).map(|_| ()),
            artist_banner(&library, &name, true).map(|_| ()),
        ] {
            if let Err(problem) = result {
                crate::logfile::warn(format!("artist pictures for {name}: {problem:#}"));
                failed = true;
            }
        }
        // A failure (no network, a service down) is tried again in a day, not at once.
        let _ = remember(
            &library,
            &sweep_key(&name),
            &true,
            if failed { 86_400 } else { WEEK * 4 },
        );
        state.done.fetch_add(1, Ordering::Relaxed);
        let pace = if state.hurry.load(Ordering::SeqCst) {
            HURRIED
        } else {
            PACE
        };
        std::thread::park_timeout(pace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_list_every_picture_best_first() {
        let data = serde_json::json!({
            "hdmusiclogo": [
                {"url": "http://assets.fanart.tv/fanart/music/x/hdmusiclogo/a.png", "likes": "2", "lang": "en"},
                {"url": "https://assets.fanart.tv/fanart/music/x/hdmusiclogo/b.png", "likes": "9", "lang": ""},
                {"url": "https://elsewhere.example/c.png", "likes": "50"}
            ],
            "musiclogo": [{"url": "https://assets.fanart.tv/fanart/music/x/musiclogo/d.png", "likes": "99"}]
        });
        let urls: Vec<String> = choices_in(&data, ArtistPicture::Logo)
            .into_iter()
            .map(|c| c.url)
            .collect();
        assert_eq!(
            urls,
            [
                "https://assets.fanart.tv/fanart/music/x/hdmusiclogo/b.png",
                "https://assets.fanart.tv/fanart/music/x/hdmusiclogo/a.png",
                "https://assets.fanart.tv/fanart/music/x/musiclogo/d.png",
            ]
        );
        assert_eq!(
            choices_in(&data, ArtistPicture::Logo)[0].preview,
            "https://assets.fanart.tv/preview/music/x/hdmusiclogo/b.png"
        );
    }

    #[test]
    fn own_pictures_and_folder_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir
            .path()
            .join("Music")
            .join("The Night Owls")
            .join("First Album");
        std::fs::create_dir_all(&music).unwrap();
        let png = b"\x89PNG\r\n\x1a\nrest";
        std::fs::write(
            music.parent().unwrap().join("fanart.jpg"),
            [0xFF, 0xD8, 0xFF, 0],
        )
        .unwrap();
        std::fs::write(music.join("folder.jpg"), [0xFF, 0xD8, 0xFF, 0]).unwrap();
        let song = music.join("01.flac");
        library
            .upsert(&crate::model::Track {
                id: "s1".into(),
                path: song.to_string_lossy().into(),
                title: "One".into(),
                artist: "Night Owls".into(),
                album_artist: "Night Owls".into(),
                album: "First Album".into(),
                ..Default::default()
            })
            .unwrap();
        // The artist's folder ("The Night Owls" matches "Night Owls") holds the wide picture;
        // the album's folder.jpg is not their photo.
        assert_eq!(
            folder_picture(&library, "Night Owls", ArtistPicture::Banner),
            Some(music.parent().unwrap().join("fanart.jpg"))
        );
        assert_eq!(
            folder_picture(&library, "Night Owls", ArtistPicture::Photo),
            None
        );

        let chosen = dir.path().join("me.png");
        std::fs::write(&chosen, png).unwrap();
        let kept = set_own_picture(&library, "Night Owls", ArtistPicture::Photo, &chosen).unwrap();
        assert_eq!(
            own_picture(&library, "night owls", ArtistPicture::Photo),
            Some(kept)
        );
        assert_eq!(
            artist_image(&library, "Night Owls", false).unwrap(),
            own_picture(&library, "Night Owls", ArtistPicture::Photo)
        );
        reset_picture(&library, "Night Owls", ArtistPicture::Photo).unwrap();
        assert_eq!(
            own_picture(&library, "Night Owls", ArtistPicture::Photo),
            None
        );

        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        assert!(set_own_picture(&library, "Night Owls", ArtistPicture::Logo, &text).is_err());
    }

    /// The wide picture and the logo: the listener's choice, then fanart.tv's while Needle may
    /// ask it, then the artist's folder.
    #[test]
    fn fanart_comes_before_the_folder_while_online() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let music = dir
            .path()
            .join("Music")
            .join("Night Owls")
            .join("First Album");
        std::fs::create_dir_all(&music).unwrap();
        let jpeg = [0xFF, 0xD8, 0xFF, 0];
        let in_folder = music.parent().unwrap().join("fanart.jpg");
        std::fs::write(&in_folder, jpeg).unwrap();
        library
            .upsert(&crate::model::Track {
                id: "s1".into(),
                path: music.join("01.flac").to_string_lossy().into(),
                title: "One".into(),
                artist: "Night Owls".into(),
                album_artist: "Night Owls".into(),
                album: "First Album".into(),
                ..Default::default()
            })
            .unwrap();
        // fanart.tv's picture, fetched before, with its answer still fresh.
        let from_fanart = kept_file(&library, "Night Owls", ArtistPicture::Banner);
        std::fs::create_dir_all(from_fanart.parent().unwrap()).unwrap();
        std::fs::write(&from_fanart, jpeg).unwrap();
        crate::media::remember(
            &library,
            &crate::media::fanart_kept("Night Owls"),
            &serde_json::json!({}),
            3_600,
        )
        .unwrap();

        // Online lookups off: the folder's picture.
        assert_eq!(
            crate::media::artist_banner(&library, "Night Owls", false).unwrap(),
            Some(in_folder.clone())
        );
        // While fanart.tv may be asked: its picture, though the folder has one.
        let banner = |may_ask| {
            crate::media::picture_in_order(
                &library,
                "Night Owls",
                may_ask,
                may_ask,
                "banner",
                |_| None,
            )
            .unwrap()
        };
        assert_eq!(banner(true), Some(from_fanart.clone()));
        assert_eq!(banner(false), Some(in_folder.clone()));
        // The listener's choice beats both.
        let chosen = dir.path().join("mine.jpg");
        std::fs::write(&chosen, jpeg).unwrap();
        let own = set_own_picture(&library, "Night Owls", ArtistPicture::Banner, &chosen).unwrap();
        assert_eq!(banner(true), Some(own.clone()));
        assert_eq!(banner(false), Some(own));
    }
}
