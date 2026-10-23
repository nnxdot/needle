//! Settings › Appearance, kept calm: pictures only where the choice is about how Needle
//! looks (the look and the background), small sketches for the layout choices, and plain
//! rows with a switch for the rest. Color only marks what is chosen.

use super::{
    AppView, Event, motion, pal,
    theme::Palette,
    widgets::{glyph, setting_row, small_button},
};
use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::{button::ButtonVariants, switch::Switch};

/// Space between cards.
const GAP: f32 = 12.;

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

#[derive(Clone, Copy, PartialEq)]
enum Backdrop {
    Plain,
    Ambient,
    Glass,
}

/// A small drawing of Needle's window, `w` wide, on one of the backgrounds: the sidebar,
/// grey blocks for the page, and the player. Only the background has color.
fn mini(w: f32, backdrop: Backdrop, p: &Palette) -> Div {
    let h = (w * 0.6).round();
    let side_w = (w * 0.24).round();
    let pad = (w * 0.05).max(4.);
    let back: Background = match backdrop {
        Backdrop::Plain => p.chrome.into(),
        Backdrop::Ambient => linear_gradient(
            150.,
            linear_color_stop(motion::mix(p.glow, p.chrome, 0.35), 0.),
            linear_color_stop(motion::mix(p.glow, p.chrome, 0.85), 1.),
        ),
        Backdrop::Glass => linear_gradient(
            135.,
            linear_color_stop(motion::mix(hex(0x3a4a5c), p.chrome, 0.35), 0.),
            linear_color_stop(motion::mix(hex(0x4a3a4c), p.chrome, 0.35), 1.),
        ),
    };
    let sheet = match backdrop {
        Backdrop::Plain => p.canvas,
        Backdrop::Ambient => hsla(0., 0., 0., 0.2),
        Backdrop::Glass => p.canvas.opacity(0.5),
    };
    let block = p.ink_3.opacity(0.22);
    let line = p.ink_3.opacity(0.45);
    let inset = if backdrop == Backdrop::Glass { 3. } else { 0. };
    let bar_h = (h * 0.1).max(6.);
    let block_w = (w - side_w - pad * 2. - pad * 0.6 * 2.) / 3.;
    div()
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .rounded(px(6.))
        .overflow_hidden()
        .bg(back)
        .child(
            div()
                .absolute()
                .left(px(side_w))
                .top_0()
                .w(px(w - side_w))
                .h(px(h))
                .bg(sheet),
        )
        .child(
            div()
                .absolute()
                .left(px(inset))
                .top(px(inset))
                .w(px(side_w - inset * 2.))
                .h(px(h - inset * 2.))
                .when(backdrop == Backdrop::Glass, |el| {
                    el.rounded(px(5.)).bg(hsla(0., 0., 1., 0.1))
                })
                .px(px(side_w * 0.18))
                .py(px(pad))
                .flex()
                .flex_col()
                .gap(px((h * 0.07).max(3.)))
                .children([0.8, 0.6, 0.72].map(|f| {
                    div()
                        .h(px(2.5))
                        .w(px(side_w * 0.64 * f))
                        .rounded(px(1.))
                        .bg(line)
                })),
        )
        .children((0..3).map(|i| {
            div()
                .absolute()
                .left(px(side_w + pad + i as f32 * (block_w + pad * 0.6)))
                .top(px(pad))
                .w(px(block_w))
                .h(px(h - pad * 2. - bar_h - pad * 0.6))
                .rounded(px(3.))
                .bg(block)
        }))
        .child(
            div()
                .absolute()
                .left(px(side_w + pad))
                .top(px(h - pad - bar_h))
                .w(px(w - side_w - pad * 2.))
                .h(px(bar_h))
                .rounded(px(3.))
                .bg(block),
        )
}

/// The one part of the window a layout choice changes.
#[derive(Clone, Copy, PartialEq)]
enum Focus {
    PlayerBottom,
    PlayerTop,
    SidebarHides,
    SidebarStrip,
    Roomy,
    Compact,
    SearchInSidebar,
    SearchPalette,
}

/// A tiny grey sketch of the window, with only the part `focus` is about drawn in ink, so
/// two sketches of a choice differ at a glance without color.
fn sketch(focus: Focus, active: bool, p: &Palette) -> Div {
    use Focus::*;
    let (w, h) = (64., 40.);
    let grey = p.ink_3.opacity(0.25);
    let lit = if active { p.accent } else { p.ink_2 };
    let side_w = match focus {
        SidebarHides => 0.,
        SidebarStrip => 7.,
        _ => 16.,
    };
    let bar_h = 5.;
    let bar_top = if focus == PlayerTop {
        3.
    } else {
        h - 3. - bar_h
    };
    let mut window = div()
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .rounded(px(4.))
        .overflow_hidden()
        .bg(p.canvas)
        .border_1()
        .border_color(p.line);
    if side_w > 0. {
        let strip = focus == SidebarStrip;
        window = window.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .w(px(side_w))
                .h(px(h))
                .bg(if strip { lit.opacity(0.35) } else { grey }),
        );
        if focus == SearchInSidebar {
            window = window.child(
                div()
                    .absolute()
                    .left(px(3.))
                    .top(px(4.))
                    .w(px(side_w - 6.))
                    .h(px(4.))
                    .rounded(px(1.))
                    .bg(lit),
            );
        }
    }
    let x = side_w + 4.;
    let page_w = w - x - 4.;
    match focus {
        Roomy | Compact => {
            let (row_h, gap, count) = if focus == Roomy {
                (6., 4., 3)
            } else {
                (2.5, 2.5, 6)
            };
            for i in 0..count {
                window = window.child(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(5. + i as f32 * (row_h + gap)))
                        .w(px(page_w))
                        .h(px(row_h))
                        .rounded(px(1.))
                        .bg(lit),
                );
            }
        }
        _ => {
            let top = if focus == PlayerTop {
                bar_top + bar_h + 4.
            } else {
                5.
            };
            let bottom = if focus == PlayerTop {
                h - 4.
            } else {
                bar_top - 4.
            };
            window = window.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(top))
                    .w(px(page_w))
                    .h(px(bottom - top))
                    .rounded(px(2.))
                    .bg(grey),
            );
        }
    }
    let player_lit = matches!(focus, PlayerBottom | PlayerTop);
    window = window.child(
        div()
            .absolute()
            .left(px(side_w + 3.))
            .top(px(bar_top))
            .w(px(w - side_w - 6.))
            .h(px(bar_h))
            .rounded(px(1.5))
            .bg(if player_lit { lit } else { grey }),
    );
    if focus == SearchPalette {
        window = window.child(
            div()
                .absolute()
                .left(px(16.))
                .top(px(9.))
                .w(px(32.))
                .h(px(16.))
                .rounded(px(2.))
                .bg(p.raised)
                .border_1()
                .border_color(lit)
                .child(
                    div()
                        .mt(px(3.))
                        .mx(px(3.))
                        .h(px(3.))
                        .rounded(px(1.))
                        .bg(lit),
                ),
        );
    }
    window
}

impl AppView {
    /// A group's name, with room above it to set it apart.
    fn group_title(&self, title: &str, _cx: &App) -> Div {
        div()
            .mt(px(40.))
            .mb(px(10.))
            .text_size(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(title.to_string())
    }

    /// A choice as a picture with its name: a thin line around it, and the accent line and a
    /// check for the chosen one.
    #[allow(clippy::too_many_arguments)]
    fn pick(
        &self,
        id: &str,
        width: f32,
        active: bool,
        picture: impl IntoElement,
        name: &str,
        about: Option<&str>,
        choose: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        div()
            .id(SharedString::from(format!("pick-{id}")))
            .w(px(width))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .cursor_pointer()
            .child(
                div()
                    .p(px(5.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if active { p.accent } else { p.line_soft })
                    .hover(|s| s.border_color(if active { p.accent } else { p.line }))
                    .child(picture),
            )
            .child(
                div()
                    .px(px(2.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .when(active, |el| el.font_weight(FontWeight::MEDIUM))
                                    .text_color(if active { p.ink } else { p.ink_2 })
                                    .truncate()
                                    .child(name.to_string()),
                            )
                            .when_some(about, |el, about| {
                                el.child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(p.ink_3)
                                        .truncate()
                                        .child(about.to_string()),
                                )
                            }),
                    )
                    .when(active, |el| {
                        el.child(glyph("check").size(px(14.)).text_color(p.accent))
                    }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                choose(this, window, cx);
                this.persist_settings();
                cx.notify();
            }))
            .into_any_element()
    }

    /// Two small sketches side by side, for a layout choice in a settings row.
    #[allow(clippy::too_many_arguments)]
    fn sketch_pair(
        &self,
        id: &str,
        second: bool,
        (first_focus, first_name): (Focus, &str),
        (second_focus, second_name): (Focus, &str),
        choose: impl Fn(&mut Self, bool) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let choose = std::rc::Rc::new(choose);
        let option = |index: usize, focus: Focus, name: &str, cx: &mut Context<Self>| {
            let active = (index == 1) == second;
            let choose = choose.clone();
            div()
                .id(SharedString::from(format!("{id}-{index}")))
                .w(px(84.))
                .py(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(if active {
                    p.accent
                } else {
                    transparent_black()
                })
                .when(active, |el| el.bg(p.accent_soft))
                .hover(|s| s.bg(p.raised))
                .cursor_pointer()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(sketch(focus, active, &p))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(if active { p.ink } else { p.ink_2 })
                        .child(name.to_string()),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    choose(this, index == 1);
                    this.persist_settings();
                    cx.notify();
                }))
        };
        div()
            .flex()
            .gap(px(4.))
            .child(option(0, first_focus, first_name, cx))
            .child(option(1, second_focus, second_name, cx))
    }

    /// Glass is chosen and can show (Windows' or macOS's own glass, with Ambient off).
    fn glass_chosen(&self) -> bool {
        use super::glass::Material;
        (cfg!(windows) || cfg!(target_os = "macos"))
            && !self.ambient_look()
            && Material::from_name(&self.settings.window_material) != Material::Solid
    }

    /// A settings row with a switch.
    fn switch_row(
        &self,
        id: &'static str,
        title: &str,
        about: &str,
        on: bool,
        flip: impl Fn(&mut Self, bool, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        setting_row(
            title,
            about,
            Switch::new(id).checked(on).on_click(cx.listener(
                move |this, checked: &bool, _, cx| {
                    flip(this, *checked, cx);
                    this.persist_settings();
                    cx.notify();
                },
            )),
            cx,
        )
    }

    /// Settings › Appearance, `width` wide.
    pub(super) fn appearance_page(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let s = self.settings.clone();
        let mut page = div().w(px(width)).flex().flex_col();

        // Look: the base looks and themes, as pictures.
        let mut looks: Vec<(String, String, String)> = super::theme::Base::ALL
            .iter()
            .map(|(mode, name, about)| (mode.to_string(), name.to_string(), about.to_string()))
            .collect();
        if let Some(themes) = cx.try_global::<super::themes::Themes>() {
            looks.extend(themes.list.iter().map(|t| {
                let about = if t.read_only {
                    format!("From {}", t.id.split('/').next().unwrap_or_default())
                } else if !t.author.is_empty() {
                    format!("By {}", t.author)
                } else {
                    "Your theme".to_string()
                };
                (t.mode(), t.name.clone(), about)
            }));
        }
        let cols = if width >= 520. { 4 } else { 2 };
        let card = ((width - GAP * (cols as f32 - 1.)) / cols as f32).floor();
        let mut cards = looks
            .into_iter()
            .map(|(mode, name, about)| {
                self.theme_card(mode, name, about, card, cx)
                    .into_any_element()
            })
            .peekable();
        let mut look_rows = div().flex().flex_col().gap(px(16.));
        while cards.peek().is_some() {
            look_rows = look_rows.child(
                div()
                    .flex()
                    .gap(px(GAP))
                    .children(cards.by_ref().take(cols)),
            );
        }
        // The first group sits right under the page title.
        page = page
            .child(
                div()
                    .mt(px(8.))
                    .mb(px(14.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Look"),
            )
            .child(look_rows)
            .child(div().mt(px(14.)).child(self.theme_actions(cx)))
            .children(self.theme_editor_view(cx));

        // Background: plain, the music's cover, or the system's glass, as pictures.
        let ambient = self.ambient_look();
        let glass = self.glass_chosen();
        let glass_offered = cfg!(windows) || cfg!(target_os = "macos");
        let (allowed, win11) = self.glass_system;
        let card = ((width - GAP * 3.) / 4.).floor();
        let picture = card - 12.;
        let mut backgrounds = vec![
            self.pick(
                "plain",
                card,
                !ambient && !glass,
                mini(picture, Backdrop::Plain, &p),
                "Plain",
                None,
                |this, _, _| {
                    this.settings.ambient = false;
                    if this.settings.theme == "ambient" {
                        this.settings.theme = "dark".into();
                    }
                    this.settings.window_material = "solid".into();
                },
                cx,
            ),
            self.pick(
                "ambient",
                card,
                ambient,
                mini(picture, Backdrop::Ambient, &p),
                "Ambient",
                Some("The cover, blurred"),
                |this, _, _| this.settings.ambient = true,
                cx,
            ),
        ];
        if glass_offered {
            let about = if cfg!(target_os = "macos") {
                "Liquid Glass"
            } else if !allowed {
                "Needs transparency on"
            } else if win11 {
                "Mica"
            } else {
                "Acrylic"
            };
            backgrounds.push(self.pick(
                "glass",
                card,
                glass,
                mini(picture, Backdrop::Glass, &p),
                "Glass",
                Some(about),
                move |this, _, _| {
                    this.settings.ambient = false;
                    if this.settings.theme == "ambient" {
                        this.settings.theme = "dark".into();
                    }
                    if super::glass::Material::from_name(&this.settings.window_material)
                        == super::glass::Material::Solid
                    {
                        this.settings.window_material = if cfg!(target_os = "macos") {
                            "glass"
                        } else if win11 {
                            "mica"
                        } else {
                            "acrylic"
                        }
                        .into();
                    }
                },
                cx,
            ));
        }
        let grain_value = if s.grain <= 0.001 {
            "Off".to_string()
        } else {
            format!("{:.0}%", s.grain * 100.)
        };
        page = page
            .child(self.group_title("Background", cx))
            .child(div().flex().gap(px(GAP)).children(backgrounds))
            .when(glass && cfg!(windows), |el| {
                el.child(div().mt(px(8.)).children(self.glass_settings(cx)))
            })
            .child(div().mt(px(8.)).child(self.switch_row(
                "music-colors",
                "Colors from the music",
                "Tint Needle with the cover that plays.",
                s.music_colors,
                |this, on, _| this.settings.music_colors = on,
                cx,
            )))
            .when(!s.music_colors && self.accent_choice_applies(cx), |el| {
                el.child(self.accent_settings(cx))
            })
            .child(setting_row(
                "Film grain",
                "A fine texture over the whole window.",
                div()
                    .w(px(220.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(gpui_component::slider::Slider::new(&self.grain_slider).flex_1())
                    .child(
                        div()
                            .w(px(36.))
                            .text_right()
                            .text_size(px(12.))
                            .text_color(p.ink_2)
                            .child(grain_value),
                    ),
                cx,
            ));

        // Layout: rows, each with two small sketches to choose from.
        let on_top = s.layout.player_on_top;
        let strip = s.layout.compact_sidebar;
        let compact = s.layout.row_height < 50.;
        let searchless = s.layout.search_hidden;
        page = page
            .child(self.group_title("Layout", cx))
            .child(setting_row(
                "The player",
                "At the foot of the window, or in the title bar.",
                self.sketch_pair(
                    "player",
                    on_top,
                    (Focus::PlayerBottom, "Bottom"),
                    (Focus::PlayerTop, "Top bar"),
                    |this, second| this.settings.layout.player_on_top = second,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                "A folded sidebar",
                if cfg!(target_os = "macos") {
                    "What stays when you fold it (⌘B)."
                } else {
                    "What stays when you fold it (Ctrl+B)."
                },
                self.sketch_pair(
                    "sidebar",
                    strip,
                    (Focus::SidebarHides, "Hides"),
                    (Focus::SidebarStrip, "Icons"),
                    |this, second| this.settings.layout.compact_sidebar = second,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                "Song rows",
                "Compact fits more songs on screen.",
                self.sketch_pair(
                    "rows",
                    compact,
                    (Focus::Roomy, "Roomy"),
                    (Focus::Compact, "Compact"),
                    |this, second| this.settings.layout.row_height = if second { 36. } else { 58. },
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                "The search field",
                if cfg!(target_os = "macos") {
                    "In the sidebar, or only with ⌘K."
                } else {
                    "In the sidebar, or only with Ctrl+K."
                },
                self.sketch_pair(
                    "search",
                    searchless,
                    (Focus::SearchInSidebar, "Sidebar"),
                    (Focus::SearchPalette, "Ctrl+K"),
                    |this, second| this.settings.layout.search_hidden = second,
                    cx,
                ),
                cx,
            ));

        // While music plays: plain rows.
        page = page
            .child(self.group_title("While music plays", cx))
            .child(self.switch_row(
                "immersive-cover",
                "Immersive cover",
                "In the big player, the cover fills the left side and fades into its colors.",
                s.immersive_cover,
                |this, on, _| this.settings.immersive_cover = on,
                cx,
            ))
            .child(self.switch_row(
                "player-lyrics",
                "Lyrics under the player",
                "The line being sung, under the player.",
                s.player_lyrics,
                |this, on, _| this.settings.player_lyrics = on,
                cx,
            ))
            .child(self.switch_row(
                "server-mark",
                "Mark server songs",
                "A small server icon beside the artist of a song from your music server.",
                s.server_mark,
                |this, on, _| this.settings.server_mark = on,
                cx,
            ));

        // Text and motion: the title fonts as small samples, and reduce motion.
        let fonts: &'static [_; 3] = &super::theme::DISPLAY_FONTS;
        let font_chips =
            div()
                .flex()
                .gap(px(6.))
                .children(fonts.iter().map(|(key, label, family)| {
                    let active =
                        s.display_font == *key || (s.display_font.is_empty() && *key == fonts[0].0);
                    div()
                        .id(SharedString::from(format!("font-{key}")))
                        .w(px(64.))
                        .h(px(52.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(if active { p.accent } else { p.line_soft })
                        .when(active, |el| el.bg(p.accent_soft))
                        .hover(|s| s.bg(p.raised))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(SharedString::from(*family))
                        .text_size(px(22.))
                        .text_color(if active { p.ink } else { p.ink_2 })
                        .child("Aa")
                        .tooltip({
                            let label = label.to_string();
                            move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(label.clone())
                                    .build(window, cx)
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            super::theme::set_display_font(key);
                            this.settings.display_font = (*key).into();
                            this.persist_settings();
                            cx.notify();
                        }))
                }));
        let font_name = fonts
            .iter()
            .find(|(key, _, _)| s.display_font == *key)
            .map_or(fonts[0].1, |(_, label, _)| label);
        let system_still = !motion::system_allows_animation();
        page =
            page.child(self.group_title("Text and motion", cx))
                .child(setting_row(
                    "Titles",
                    &format!("The font for page titles and big names: {font_name}."),
                    font_chips,
                    cx,
                ))
                .child(self.switch_row(
                    "reduce-motion",
                    "Reduce motion",
                    if !system_still {
                        "Turns off fades, slides, and other animations."
                    } else if cfg!(windows) {
                        "Windows has animations turned off, so Needle keeps still too."
                    } else {
                        "Your system has animations turned off, so Needle keeps still too."
                    },
                    s.reduce_motion || system_still,
                    |this, on, cx| {
                        this.settings.reduce_motion = on;
                        cx.set_global(motion::Motion {
                            enabled: !on && motion::system_allows_animation(),
                        });
                    },
                    cx,
                ))
                .child(setting_row(
                    "Layout file",
                    "Share panel widths and row height as a small JSON file.",
                    div()
                        .flex()
                        .gap_2()
                        .child(small_button("import-layout", "Import").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                if !this.can_pick(cx) {
                                    return;
                                }
                                let sender = this.sender.clone();
                                std::thread::spawn(move || {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Needle layout", &["json"])
                                        .pick_file()
                                    {
                                        let result = (|| -> Result<needle_core::model::Layout> {
                                            let layout: needle_core::model::Layout =
                                                serde_json::from_slice(&std::fs::read(path)?)?;
                                            layout.validate()?;
                                            Ok(layout)
                                        })();
                                        let _ = sender.send(match result {
                                            Ok(layout) => Event::Layout(layout),
                                            Err(e) => Event::Error(e.to_string()),
                                        });
                                    }
                                });
                            }),
                        ))
                        .child(small_button("export-layout", "Export").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                if !this.can_pick(cx) {
                                    return;
                                }
                                let layout = this.settings.layout.clone();
                                this.background(move || {
                                    let Some(path) = rfd::FileDialog::new()
                                        .set_file_name("needle-layout.json")
                                        .save_file()
                                    else {
                                        return Ok(String::new());
                                    };
                                    std::fs::write(path, serde_json::to_vec_pretty(&layout)?)?;
                                    Ok("Layout exported.".into())
                                });
                            }),
                        )),
                    cx,
                ));
        page
    }
}
