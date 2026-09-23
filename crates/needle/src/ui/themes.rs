//! Custom themes: small TOML files in `<data>/themes` (and in the `themes` folder of enabled
//! plugins) that start from one of the base looks and change any of its colours.
//!
//! ```toml
//! name = "Sakura"
//! base = "light"            # dark (Night), midnight, or light (Day)
//! music_colors = true       # may colours from the music tint what the theme leaves alone
//! [colors]
//! page = "#fff5f7"
//! accent = "#e75480"
//! [extras]
//! grain = 0.2               # film grain, 0 to 1
//! font = "fraunces"         # title font: system, bahnschrift, or fraunces
//! ```
//!
//! A theme lists only the colours it changes; the base look gives the rest. Text colours are
//! moved just enough to stay readable (see `Palette::custom`).
use super::theme::{Base, DISPLAY_FONTS, Palette, parse_hex};
use anyhow::{Context, Result, bail};
use gpui::{Global, Hsla};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// A colour a theme can set: its key in the file, the name people see, and what it paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    Sidebar,
    Page,
    Card,
    CardHover,
    Border,
    BorderSoft,
    Text,
    TextMuted,
    TextFaint,
    Accent,
    AccentText,
    Danger,
    Success,
    Glow,
}

impl Slot {
    pub const ALL: [Slot; 14] = [
        Slot::Sidebar,
        Slot::Page,
        Slot::Card,
        Slot::CardHover,
        Slot::Border,
        Slot::BorderSoft,
        Slot::Text,
        Slot::TextMuted,
        Slot::TextFaint,
        Slot::Accent,
        Slot::AccentText,
        Slot::Danger,
        Slot::Success,
        Slot::Glow,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Slot::Sidebar => "sidebar",
            Slot::Page => "page",
            Slot::Card => "card",
            Slot::CardHover => "card_hover",
            Slot::Border => "border",
            Slot::BorderSoft => "border_soft",
            Slot::Text => "text",
            Slot::TextMuted => "text_muted",
            Slot::TextFaint => "text_faint",
            Slot::Accent => "accent",
            Slot::AccentText => "accent_text",
            Slot::Danger => "danger",
            Slot::Success => "success",
            Slot::Glow => "glow",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
    pub fn name(self) -> &'static str {
        match self {
            Slot::Sidebar => "Sidebar and bars",
            Slot::Page => "Page",
            Slot::Card => "Cards and fields",
            Slot::CardHover => "Cards on hover",
            Slot::Border => "Lines",
            Slot::BorderSoft => "Soft lines",
            Slot::Text => "Text",
            Slot::TextMuted => "Second text",
            Slot::TextFaint => "Faint text",
            Slot::Accent => "Accent",
            Slot::AccentText => "Text on accent",
            Slot::Danger => "Warnings",
            Slot::Success => "Success",
            Slot::Glow => "Glow",
        }
    }
    pub fn about(self) -> &'static str {
        match self {
            Slot::Sidebar => "The sidebar, the title bar, and the player at the bottom.",
            Slot::Page => "The sheet the songs and pages sit on.",
            Slot::Card => "Hovered rows, search fields, and cards.",
            Slot::CardHover => "Cards and buttons under the pointer.",
            Slot::Border => "Borders around fields and cards.",
            Slot::BorderSoft => "Quiet dividers between rows and sections.",
            Slot::Text => "Titles and most text.",
            Slot::TextMuted => "Artists, albums, and descriptions.",
            Slot::TextFaint => "Small print, times, and hints.",
            Slot::Accent => "Buttons, switches, the playing song, and links.",
            Slot::AccentText => "Labels on accent buttons.",
            Slot::Danger => "Errors and delete buttons.",
            Slot::Success => "Things that went well.",
            Slot::Glow => "The soft light behind covers and page headers.",
        }
    }
    pub fn get(self, p: &Palette) -> Hsla {
        match self {
            Slot::Sidebar => p.chrome,
            Slot::Page => p.canvas,
            Slot::Card => p.raised,
            Slot::CardHover => p.raised_hover,
            Slot::Border => p.line,
            Slot::BorderSoft => p.line_soft,
            Slot::Text => p.ink,
            Slot::TextMuted => p.ink_2,
            Slot::TextFaint => p.ink_3,
            Slot::Accent => p.accent,
            Slot::AccentText => p.accent_ink,
            Slot::Danger => p.danger,
            Slot::Success => p.success,
            Slot::Glow => p.glow,
        }
    }
    pub fn set(self, p: &mut Palette, color: Hsla) {
        let slot = match self {
            Slot::Sidebar => &mut p.chrome,
            Slot::Page => &mut p.canvas,
            Slot::Card => &mut p.raised,
            Slot::CardHover => &mut p.raised_hover,
            Slot::Border => &mut p.line,
            Slot::BorderSoft => &mut p.line_soft,
            Slot::Text => &mut p.ink,
            Slot::TextMuted => &mut p.ink_2,
            Slot::TextFaint => &mut p.ink_3,
            Slot::Accent => &mut p.accent,
            Slot::AccentText => &mut p.accent_ink,
            Slot::Danger => &mut p.danger,
            Slot::Success => &mut p.success,
            Slot::Glow => &mut p.glow,
        };
        *slot = color;
    }
}

/// One theme file.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomTheme {
    /// Settings value without the `custom:` prefix: the file name, or `plugin/file` for a
    /// plugin's theme.
    pub id: String,
    pub name: String,
    pub author: String,
    pub base: Base,
    /// Colours from the music may tint what the theme leaves alone.
    pub music_colors: bool,
    pub colors: BTreeMap<Slot, Hsla>,
    pub grain: Option<f32>,
    pub font: Option<String>,
    pub path: PathBuf,
    /// Comes with a plugin: make a copy to change it.
    pub read_only: bool,
}

impl CustomTheme {
    /// The settings value that chooses this theme.
    pub fn mode(&self) -> String {
        format!("custom:{}", self.id)
    }

    /// A new theme with this name, starting from `base` and no colours of its own.
    pub fn new(id: &str, name: &str, base: Base, folder: &Path) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            author: String::new(),
            base,
            music_colors: true,
            colors: BTreeMap::new(),
            grain: None,
            font: None,
            path: folder.join(format!("{id}.toml")),
            read_only: false,
        }
    }

    pub fn parse(text: &str, id: &str, path: &Path) -> Result<Self> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            name: Option<String>,
            #[serde(default)]
            author: String,
            #[serde(default)]
            base: Option<String>,
            #[serde(default)]
            music_colors: Option<bool>,
            #[serde(default)]
            colors: BTreeMap<String, String>,
            #[serde(default)]
            extras: Extras,
        }
        #[derive(serde::Deserialize, Default)]
        #[serde(deny_unknown_fields)]
        struct Extras {
            grain: Option<f32>,
            font: Option<String>,
        }
        let file: File = toml::from_str(text).map_err(|e| anyhow::anyhow!("{}", e.message()))?;
        let base = match file.base.as_deref() {
            None | Some("dark") | Some("night") => Base::Night,
            Some("midnight") => Base::Midnight,
            Some("light") | Some("day") => Base::Day,
            Some(other) => bail!("base \"{other}\" is not dark, midnight, or light"),
        };
        let mut colors = BTreeMap::new();
        for (key, value) in file.colors {
            let slot = Slot::from_key(&key).with_context(|| {
                format!(
                    "\"{key}\" is not a colour a theme can set (try {})",
                    Slot::ALL.map(Slot::key).join(", ")
                )
            })?;
            let color = parse_hex(&value)
                .with_context(|| format!("{key} = \"{value}\" is not a colour like \"#e75480\""))?;
            colors.insert(slot, color);
        }
        if let Some(font) = &file.extras.font
            && !DISPLAY_FONTS.iter().any(|(key, _, _)| key == font)
        {
            bail!(
                "font \"{font}\" is not one of {}",
                DISPLAY_FONTS.map(|f| f.0).join(", ")
            );
        }
        let name = file
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| id.rsplit('/').next().unwrap_or(id).to_string());
        Ok(Self {
            id: id.into(),
            name,
            author: file.author,
            base,
            music_colors: file.music_colors.unwrap_or(true),
            colors,
            grain: file.extras.grain.map(|g| g.clamp(0., 1.)),
            font: file.extras.font,
            path: path.into(),
            read_only: false,
        })
    }

    /// The file's text, with a note on how to edit it.
    pub fn to_toml(&self) -> String {
        let quote = |s: &str| toml::Value::String(s.into()).to_string();
        let mut out = String::from(
            "# A Needle theme. Colours are \"#rrggbb\"; leave one out to keep the base look's.\n\
             # Keys: https://needle.nnx.fyi/themes\n",
        );
        out += &format!("name = {}\n", quote(&self.name));
        if !self.author.is_empty() {
            out += &format!("author = {}\n", quote(&self.author));
        }
        out += &format!("base = \"{}\"\n", base_key(self.base));
        out += &format!("music_colors = {}\n", self.music_colors);
        out += "\n[colors]\n";
        for (slot, color) in &self.colors {
            out += &format!("{} = \"{}\"\n", slot.key(), hex(*color));
        }
        if self.grain.is_some() || self.font.is_some() {
            out += "\n[extras]\n";
            if let Some(grain) = self.grain {
                out += &format!("grain = {}\n", (grain * 100.).round() / 100.);
            }
            if let Some(font) = &self.font {
                out += &format!("font = {}\n", quote(font));
            }
        }
        out
    }

    pub fn save(&self) -> Result<()> {
        if self.read_only {
            bail!("This theme comes with a plugin. Make a copy to change it.");
        }
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&self.path, self.to_toml())
            .with_context(|| format!("Could not save {}", self.path.display()))
    }
}

pub fn base_key(base: Base) -> &'static str {
    match base {
        Base::Night => "dark",
        Base::Midnight => "midnight",
        Base::Day => "light",
    }
}

pub fn hex(color: Hsla) -> String {
    let c = color.to_rgb();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// The user's theme folder.
pub fn folder(data: &Path) -> PathBuf {
    data.join("themes")
}

/// A file name made from a theme's name: lowercase letters, digits, and dashes.
pub fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "theme".into()
    } else {
        slug
    }
}

/// An id for `name` that no theme in `folder` uses yet.
pub fn free_id(folder: &Path, name: &str) -> String {
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while folder.join(format!("{id}.toml")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Every theme Needle can see, and the files it could not read.
#[derive(Clone, Debug, Default)]
pub struct Themes {
    pub list: Vec<CustomTheme>,
    /// File name and what is wrong with it.
    pub problems: Vec<(String, String)>,
    /// The files as last read, to notice edits.
    stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
}
impl Global for Themes {}

impl Themes {
    pub fn find(&self, mode: &str) -> Option<&CustomTheme> {
        let id = mode.strip_prefix("custom:")?;
        self.list.iter().find(|t| t.id == id)
    }
}

/// Where themes are read from: the user's folder, then the `themes` folder of each enabled
/// plugin (with the plugin's id).
pub fn sources(library: &needle_core::database::Library) -> Vec<(PathBuf, Option<String>)> {
    let data = &library.directory;
    let mut out = vec![(folder(data), None)];
    let enabled: std::collections::BTreeSet<String> = library
        .get_json("plugins_enabled")
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut plugins: Vec<PathBuf> = std::fs::read_dir(data.join("plugins"))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    plugins.sort();
    for dir in plugins {
        if let Ok(manifest) = needle_core::plugins::read_manifest(&dir)
            && enabled.contains(&manifest.id)
            && dir.join("themes").is_dir()
        {
            out.push((dir.join("themes"), Some(manifest.id)));
        }
    }
    out
}

fn files(sources: &[(PathBuf, Option<String>)]) -> Vec<(PathBuf, Option<String>)> {
    let mut out = vec![];
    for (dir, plugin) in sources {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
            })
            .collect();
        found.sort();
        out.extend(found.into_iter().map(|p| (p, plugin.clone())));
    }
    out
}

fn stamp(files: &[(PathBuf, Option<String>)]) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
    files
        .iter()
        .map(|(path, _)| {
            let meta = std::fs::metadata(path).ok();
            (
                path.clone(),
                meta.as_ref().and_then(|m| m.modified().ok()),
                meta.map_or(0, |m| m.len()),
            )
        })
        .collect()
}

/// Read every theme.
pub fn load(sources: &[(PathBuf, Option<String>)]) -> Themes {
    let found = files(sources);
    let mut themes = Themes {
        stamp: stamp(&found),
        ..Default::default()
    };
    for (path, plugin) in found {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let id = match &plugin {
            Some(plugin) => format!("{plugin}/{stem}"),
            None => stem,
        };
        let label = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|text| CustomTheme::parse(&text, &id, &path))
        {
            Ok(mut theme) => {
                theme.read_only = plugin.is_some();
                themes.list.push(theme);
            }
            Err(error) => themes.problems.push((label, format!("{error:#}"))),
        }
    }
    themes
}

/// Whether any theme file was added, removed, or changed since `themes` was read.
pub fn changed(themes: &Themes, sources: &[(PathBuf, Option<String>)]) -> bool {
    stamp(&files(sources)) != themes.stamp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::contrast;

    const SAKURA: &str = r##"
name = "Sakura"
author = "Willow"
base = "light"
music_colors = false
[colors]
page = "#fff5f7"
accent = "#e75480"
text = "#3a2230"
[extras]
grain = 0.2
font = "fraunces"
"##;

    #[test]
    fn reads_and_writes_a_theme() {
        let theme = CustomTheme::parse(SAKURA, "sakura", Path::new("sakura.toml")).unwrap();
        assert_eq!(theme.name, "Sakura");
        assert_eq!(theme.base, Base::Day);
        assert!(!theme.music_colors);
        assert_eq!(theme.colors.len(), 3);
        assert_eq!(hex(theme.colors[&Slot::Accent]), "#e75480");
        assert_eq!(theme.grain, Some(0.2));
        assert_eq!(theme.font.as_deref(), Some("fraunces"));
        let again =
            CustomTheme::parse(&theme.to_toml(), "sakura", Path::new("sakura.toml")).unwrap();
        assert_eq!(again, theme);
    }

    #[test]
    fn explains_what_is_wrong() {
        let error = |text: &str| {
            format!(
                "{:#}",
                CustomTheme::parse(text, "x", Path::new("x.toml")).unwrap_err()
            )
        };
        assert!(error("[colors]\nbackground = \"#000000\"").contains("\"background\" is not"));
        assert!(error("[colors]\naccent = \"pink\"").contains("not a colour like"));
        assert!(error("base = \"sepia\"").contains("not dark, midnight, or light"));
        assert!(error("[extras]\nfont = \"Comic Sans\"").contains("is not one of"));
        // A theme with nothing in it is the dark look, named after its file.
        let empty = CustomTheme::parse("", "plugin/cosy", Path::new("cosy.toml")).unwrap();
        assert_eq!((empty.name.as_str(), empty.base), ("cosy", Base::Night));
    }

    #[test]
    fn names_become_file_names() {
        assert_eq!(slug("Sakura Night!"), "sakura-night");
        assert_eq!(slug("  "), "theme");
        assert_eq!(slug("키키 Pink"), "키키-pink");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sakura.toml"), "").unwrap();
        assert_eq!(free_id(dir.path(), "Sakura"), "sakura-2");
    }

    #[test]
    fn loads_user_and_plugin_themes_and_notices_edits() {
        let data = tempfile::tempdir().unwrap();
        let library = needle_core::database::Library::open(data.path()).unwrap();
        let mine = folder(data.path());
        let plugin = data.path().join("plugins").join("pastel").join("themes");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(
            data.path().join("plugins/pastel/plugin.toml"),
            "id = \"pastel\"
name = \"Pastel\"
version = \"1.0\"",
        )
        .unwrap();
        // A plugin's themes count only while it is turned on.
        assert_eq!(sources(&library).len(), 1);
        library.set_json("plugins_enabled", &["pastel"]).unwrap();
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(mine.join("sakura.toml"), SAKURA).unwrap();
        std::fs::write(mine.join("broken.toml"), "[colors]\nink = \"#fff\"").unwrap();
        std::fs::write(plugin.join("mint.toml"), "name = \"Mint\"").unwrap();
        let sources = sources(&library);
        let themes = load(&sources);
        let ids: Vec<_> = themes.list.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["sakura", "pastel/mint"]);
        assert!(themes.find("custom:pastel/mint").unwrap().read_only);
        assert!(themes.find("custom:sakura").is_some() && themes.find("dark").is_none());
        assert_eq!(themes.problems.len(), 1);
        assert!(themes.problems[0].1.contains("\"ink\""));
        assert!(!changed(&themes, &sources));
        std::fs::write(mine.join("sakura.toml"), format!("{SAKURA}\n# edited")).unwrap();
        assert!(changed(&themes, &sources));
        // Plugin themes cannot be saved over.
        assert!(themes.find("custom:pastel/mint").unwrap().save().is_err());
    }

    /// However wild the colours, text stays readable (WCAG AA, 4.5:1) on every surface.
    #[test]
    fn custom_colours_stay_readable() {
        let greys = [0x000000, 0x202020, 0x777777, 0xbbbbbb, 0xffffff];
        let wild = [0xff0000, 0xffff00, 0x00ff88, 0x3355ff, 0xff66cc];
        for base in [Base::Night, Base::Midnight, Base::Day] {
            for (i, surface) in greys.iter().chain(&wild).enumerate() {
                for text in greys.iter().chain(&wild) {
                    let mut theme = CustomTheme::new("t", "T", base, Path::new("."));
                    let c = |v: u32| -> Hsla { gpui::rgb(v).into() };
                    theme.colors.insert(Slot::Page, c(*surface));
                    theme
                        .colors
                        .insert(Slot::Sidebar, c(greys[i % greys.len()]));
                    theme.colors.insert(Slot::Text, c(*text));
                    theme.colors.insert(Slot::Accent, c(*text));
                    let (p, _) = Palette::custom(&theme, None, false);
                    for s in [p.chrome, p.canvas, p.raised] {
                        for t in [p.ink, p.ink_2, p.ink_3, p.accent, p.danger] {
                            assert!(contrast(t, s) >= 4.5, "{} on {} ({base:?})", hex(t), hex(s));
                        }
                    }
                    assert!(contrast(p.accent_ink, p.accent) >= 4.5);
                }
            }
        }
    }

    #[test]
    fn chosen_colours_are_kept_when_readable() {
        let theme = CustomTheme::parse(SAKURA, "sakura", Path::new("sakura.toml")).unwrap();
        let (p, adjusted) = Palette::custom(&theme, None, false);
        assert_eq!(hex(p.canvas), "#fff5f7");
        assert_eq!(hex(p.ink), "#3a2230");
        assert!(!p.dark);
        // The accent is too light for text on this page, so it is darkened, and said so.
        assert!(adjusted.contains(&Slot::Accent));
        assert!(contrast(p.accent, p.canvas) >= 4.5);
        // A dark page makes the look dark, whatever the base.
        let mut night = theme.clone();
        night.colors.insert(Slot::Page, gpui::rgb(0x101014).into());
        night
            .colors
            .insert(Slot::Sidebar, gpui::rgb(0x08080a).into());
        night.colors.remove(&Slot::Text);
        let (p, _) = Palette::custom(&night, None, false);
        assert!(p.dark);
    }
}
