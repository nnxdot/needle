use anyhow::Result;
use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

/// Needle's own icon set: 24px grid, 1.7px round strokes. Filled glyphs opt in with `fill`.
pub struct Assets;

const FILLED: &str = "fill='currentColor' stroke='none'";

fn shape(name: &str) -> Option<String> {
    let heart = "M12 20s-7.2-4.4-9.1-9.1C1.5 7.4 3.9 4 7.4 4c2 0 3.5 1.1 4.6 2.7C13.1 5.1 14.6 4 16.6 4c3.5 0 5.9 3.4 4.5 6.9C19.2 15.6 12 20 12 20z";
    let star = "M12 3.6l2.6 5.3 5.8.8-4.2 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.2-4.1 5.8-.8z";
    Some(match name {
        "play" => format!("<path d='M7.5 4.8v14.4a.8.8 0 0 0 1.2.7l11.6-7.2a.8.8 0 0 0 0-1.4L8.7 4.1a.8.8 0 0 0-1.2.7z' {FILLED}/>"),
        "pause" => format!("<rect x='6' y='4.5' width='4.2' height='15' rx='1.2' {FILLED}/><rect x='13.8' y='4.5' width='4.2' height='15' rx='1.2' {FILLED}/>"),
        "next" => format!("<path d='M5 5.6v12.8a.8.8 0 0 0 1.2.7l9.6-6.4a.8.8 0 0 0 0-1.4L6.2 4.9a.8.8 0 0 0-1.2.7z' {FILLED}/><path d='M19 5v14' stroke-width='2.2'/>"),
        "previous" => format!("<path d='M19 5.6v12.8a.8.8 0 0 1-1.2.7l-9.6-6.4a.8.8 0 0 1 0-1.4l9.6-6.4a.8.8 0 0 1 1.2.7z' {FILLED}/><path d='M5 5v14' stroke-width='2.2'/>"),
        "shuffle" => "<path d='M16 3h5v5M4 20 21 3M21 16v5h-5M15 15l6 6M4 4l5 5'/>".into(),
        "repeat" => "<path d='m17 2 4 4-4 4'/><path d='M3 11v-1a4 4 0 0 1 4-4h14'/><path d='m7 22-4-4 4-4'/><path d='M21 13v1a4 4 0 0 1-4 4H3'/>".into(),
        "repeat-one" => "<path d='m17 2 4 4-4 4'/><path d='M3 11v-1a4 4 0 0 1 4-4h14'/><path d='m7 22-4-4 4-4'/><path d='M21 13v1a4 4 0 0 1-4 4H3'/><path d='M11 10.5l1.5-1v5'/>".into(),
        "volume" => "<path d='M11 5 6 9H3v6h3l5 4z'/><path d='M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13'/>".into(),
        "volume-low" => "<path d='M11 5 6 9H3v6h3l5 4z'/><path d='M15.5 8.5a5 5 0 0 1 0 7'/>".into(),
        "volume-off" => "<path d='M11 5 6 9H3v6h3l5 4z'/><path d='m16 9 6 6M22 9l-6 6'/>".into(),
        "queue" => "<path d='M3 6h14M3 12h10M3 18h7'/><path d='M16 14.5v6.5l5-3.2z' fill='currentColor'/>".into(),
        "songs" => "<path d='M9 18V5l11-2v13'/><circle cx='6' cy='18' r='3'/><circle cx='17' cy='16' r='3'/>".into(),
        "albums" => "<circle cx='12' cy='12' r='9'/><circle cx='12' cy='12' r='2.5'/>".into(),
        "artists" => "<rect x='9' y='2.5' width='6' height='11.5' rx='3'/><path d='M5 11a7 7 0 0 0 14 0M12 18v3.5M8.5 21.5h7'/>".into(),
        "genres" => "<path d='M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4z'/><circle cx='16.5' cy='16.5' r='3.5'/>".into(),
        "heart" => format!("<path d='{heart}'/>"),
        "heart-fill" => format!("<path d='{heart}' {FILLED}/>"),
        "star" => format!("<path d='{star}'/>"),
        "star-fill" => format!("<path d='{star}' {FILLED}/>"),
        "recent" => "<circle cx='12' cy='12' r='9'/><path d='M12 7v5l3.2 2'/>".into(),
        "history" => "<path d='M3 20h18'/><path d='M6.5 16v-4M11 16V6M15.5 16V9.5M20 16v-8'/>".into(),
        "plus" => "<path d='M12 5v14M5 12h14'/>".into(),
        "search" => "<circle cx='11' cy='11' r='6.5'/><path d='m20 20-4.4-4.4'/>".into(),
        "settings" => "<path d='M3 6h11M18 6h3M3 12h5M12 12h9M3 18h13M20 18h1'/><circle cx='16' cy='6' r='2'/><circle cx='10' cy='12' r='2'/><circle cx='18' cy='18' r='2'/>".into(),
        "folder" => "<path d='M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2.5h7.5A2.5 2.5 0 0 1 21 10v7.5a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5z'/>".into(),
        "playlist" => "<path d='M3 6h13M3 11h13M3 16h7'/><circle cx='16.5' cy='18' r='2.5'/><path d='M19 18V9.5l2.5-1'/>".into(),
        "smart" => "<path d='M11 3.5l1.7 4.6 4.8 1.6-4.8 1.6L11 16l-1.7-4.7-4.8-1.6 4.8-1.6z'/><path d='M18 14.5l.8 2.1 2.2.8-2.2.8-.8 2.3-.8-2.3-2.2-.8 2.2-.8z'/>".into(),
        "more" => format!("<circle cx='5.5' cy='12' r='1.6' {FILLED}/><circle cx='12' cy='12' r='1.6' {FILLED}/><circle cx='18.5' cy='12' r='1.6' {FILLED}/>"),
        "close" => "<path d='M6 6l12 12M18 6 6 18'/>".into(),
        "loop" => "<path d='M4 12a8 8 0 0 1 13.7-5.6L20 8.5'/><path d='M20 4v4.5h-4.5'/><path d='M20 12a8 8 0 0 1-13.7 5.6L4 15.5'/><path d='M4 20v-4.5h4.5'/>".into(),
        "check" => "<path d='m5 12.5 4.5 4.5L19 7'/>".into(),
        "chevron-right" => "<path d='m9.5 6 6 6-6 6'/>".into(),
        "chevron-left" => "<path d='m14.5 6-6 6 6 6'/>".into(),
        "chevron-down" => "<path d='m6 9.5 6 6 6-6'/>".into(),
        "arrow-up" => "<path d='M12 19V5M6 11l6-6 6 6'/>".into(),
        "arrow-down" => "<path d='M12 5v14M6 13l6 6 6-6'/>".into(),
        "signal" => "<path d='M2 12h3.5l2.5-7 4 14 3-10 2 5h5'/>".into(),
        "tag" => "<path d='M3.5 12.2V4.5a1 1 0 0 1 1-1h7.7l8.3 8.3a1 1 0 0 1 0 1.4l-7.3 7.3a1 1 0 0 1-1.4 0z'/><circle cx='8' cy='8' r='1.4'/>".into(),
        "external" => "<path d='M14 4h6v6M20 4l-8.5 8.5M18 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10'/>".into(),
        "copy" => "<rect x='8.5' y='8.5' width='12' height='12' rx='2'/><path d='M15.5 8.5V5a1.5 1.5 0 0 0-1.5-1.5H5A1.5 1.5 0 0 0 3.5 5v9A1.5 1.5 0 0 0 5 15.5h3.5'/>".into(),
        "panel" => "<rect x='3' y='4' width='18' height='16' rx='2.5'/><path d='M15 4v16'/>".into(),
        "trash" => "<path d='M4 7h16M9.5 7V4.5h5V7M6 7l1 12.5a1 1 0 0 0 1 .9h8a1 1 0 0 0 1-.9L18 7'/>".into(),
        "edit" => "<path d='M4 20h4.5L19.3 9.2a2 2 0 0 0 0-2.8l-1.7-1.7a2 2 0 0 0-2.8 0L4 15.5z'/><path d='m13.5 6 4.5 4.5'/>".into(),
        "speaker" => "<rect x='5.5' y='3' width='13' height='18' rx='2.5'/><circle cx='12' cy='14.5' r='3'/><circle cx='12' cy='7.5' r='1'/>".into(),
        "account" => "<circle cx='12' cy='8.5' r='4'/><path d='M4.5 20.5a7.5 7.5 0 0 1 15 0'/>".into(),
        "info" => "<circle cx='12' cy='12' r='9'/><path d='M12 11v5.5M12 7.6v.1'/>".into(),
        "alert" => "<path d='M12 3.5 2.5 20h19z'/><path d='M12 10v4.5M12 17.2v.1'/>".into(),
        "globe" => "<circle cx='12' cy='12' r='9'/><path d='M3 12h18M12 3c2.5 2.6 3.7 5.6 3.7 9s-1.2 6.4-3.7 9c-2.5-2.6-3.7-5.6-3.7-9S9.5 5.6 12 3z'/>".into(),
        "undo" => "<path d='M9 14 4 9l5-5'/><path d='M4 9h10.5a5.5 5.5 0 0 1 0 11H11'/>".into(),
        "logo" => "<circle cx='11' cy='13' r='8.5'/><circle cx='11' cy='13' r='2.2' fill='currentColor'/><path d='M21.5 2.5 14.8 9.2'/><path d='M7 9.8a5.3 5.3 0 0 1 2.6-2' opacity='.6'/>".into(),
        _ => return None,
    })
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("needle/")
            .and_then(|p| p.strip_suffix(".svg"))
            && let Some(shape) = shape(name)
        {
            return Ok(Some(Cow::Owned(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>{shape}</svg>").into_bytes())));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}
