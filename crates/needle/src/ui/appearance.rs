//! Settings › Appearance: every choice as a small picture of what it does, a preview of the
//! window as chosen at the top, and the choices in groups (look, background, layout, while
//! music plays, text and motion).

use super::{
    AppView, Event, motion, pal,
    theme::Palette,
    widgets::{glyph, meta, small_button},
};
use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::button::ButtonVariants;

/// Space between cards.
const GAP: f32 = 10.;

/// Made-up covers for the small windows' album grids.
const COVERS: [u32; 8] = [
    0xc0563a, 0x5f7a4e, 0xd9735d, 0xa8343f, 0x45627a, 0xb98559, 0x6a5279, 0x3a6896,
];

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Full,
    Strip,
    Hidden,
}

#[derive(Clone, Copy, PartialEq)]
enum Backdrop {
    Plain,
    Ambient,
    Glass,
}

/// What a small window shows.
#[derive(Clone, Copy)]
struct Mini {
    side: Side,
    search: bool,
    player_on_top: bool,
    compact: bool,
    lyrics: bool,
    backdrop: Backdrop,
}

impl Default for Mini {
    fn default() -> Self {
        Self {
            side: Side::Full,
            search: false,
            player_on_top: false,
            compact: false,
            lyrics: false,
            backdrop: Backdrop::Plain,
        }
    }
}

/// A small drawing of Needle's window, `w` wide: the sidebar, a grid of covers, and the player.
fn mini(w: f32, m: Mini, p: &Palette) -> Div {
    let h = (w * 0.625).round();
    let side_w = match m.side {
        Side::Full => (w * 0.24).round(),
        Side::Strip => (w * 0.08).round(),
        Side::Hidden => 0.,
    };
    let pad = (w * 0.035).max(4.);
    let back: Background = match m.backdrop {
        Backdrop::Plain => p.chrome.into(),
        Backdrop::Ambient => linear_gradient(
            150.,
            linear_color_stop(p.glow.opacity(0.95), 0.),
            linear_color_stop(motion::mix(p.glow, hsla(0., 0., 0.05, 1.), 0.8), 0.9),
        ),
        Backdrop::Glass => linear_gradient(
            135.,
            linear_color_stop(hex(0x2a3746), 0.),
            linear_color_stop(hex(0x3e2b3c), 1.),
        ),
    };
    let sheet = match m.backdrop {
        Backdrop::Plain => p.canvas,
        Backdrop::Ambient => hsla(0., 0., 0., 0.18),
        Backdrop::Glass => p.canvas.opacity(0.55),
    };
    let panel = match m.backdrop {
        Backdrop::Plain => p.raised,
        Backdrop::Ambient => hsla(0., 0., 0., 0.3),
        Backdrop::Glass => hsla(0., 0., 1., 0.14),
    };
    let dim = p.ink_3.opacity(0.7);

    // The player: at the foot, or in the top bar.
    let bar_h = (h * 0.1).max(6.);
    let bar_left = side_w + w * 0.04;
    let bar_w = w - bar_left - w * 0.04;
    let lyric_room = if m.lyrics { bar_h * 0.6 } else { 0. };
    let bar_top = if m.player_on_top {
        h * 0.05
    } else {
        h - h * 0.05 - bar_h - lyric_room
    };

    // The covers: a grid in the room the player leaves.
    let cols = if m.compact { 6 } else { 4 };
    let main_x = side_w + pad;
    let main_w = w - side_w - pad * 2.;
    let tile_gap = if m.compact { 2. } else { (w * 0.02).max(2.) };
    let tile = (main_w - tile_gap * (cols as f32 - 1.)) / cols as f32;
    let grid_top = if m.player_on_top {
        bar_top + bar_h + pad
    } else {
        pad
    };
    let grid_room = if m.player_on_top {
        h - grid_top - pad
    } else {
        bar_top - pad - grid_top
    };
    let rows = (((grid_room + tile_gap) / (tile + tile_gap)).floor() as usize).clamp(1, 3);

    let mut window = div()
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .rounded(px(7.))
        .overflow_hidden()
        .bg(back);
    // The page sheet beside the sidebar.
    window = window.child(
        div()
            .absolute()
            .left(px(side_w))
            .top_0()
            .w(px(w - side_w))
            .h(px(h))
            .bg(sheet),
    );
    // The sidebar.
    match m.side {
        Side::Full => {
            let mut side = div()
                .absolute()
                .left(px(if m.backdrop == Backdrop::Glass {
                    3.
                } else {
                    0.
                }))
                .top(px(if m.backdrop == Backdrop::Glass {
                    3.
                } else {
                    0.
                }))
                .w(px(side_w
                    - if m.backdrop == Backdrop::Glass {
                        6.
                    } else {
                        0.
                    }))
                .h(px(h - if m.backdrop == Backdrop::Glass {
                    6.
                } else {
                    0.
                }))
                .when(m.backdrop == Backdrop::Glass, |el| {
                    el.rounded(px(5.)).bg(panel)
                })
                .px(px(side_w * 0.16))
                .py(px(pad * 1.3))
                .flex()
                .flex_col()
                .gap(px((h * 0.055).max(3.)));
            if m.search {
                side = side.child(
                    div()
                        .w_full()
                        .h(px((h * 0.06).max(4.)))
                        .rounded(px(3.))
                        .bg(p.ink_3.opacity(0.35)),
                );
            }
            for fraction in [0.8, 0.6, 0.72, 0.5] {
                side = side.child(
                    div()
                        .h(px(2.5))
                        .w(px((side_w * 0.68 * fraction).max(4.)))
                        .rounded(px(1.))
                        .bg(dim),
                );
            }
            window = window.child(side);
        }
        Side::Strip => {
            window = window.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(side_w))
                    .h(px(h))
                    .pt(px(pad * 1.3))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px((h * 0.07).max(3.)))
                    .children((0..4).map(|_| div().size(px(3.5)).rounded_full().bg(dim))),
            );
        }
        Side::Hidden => {}
    }
    // The covers.
    for row in 0..rows {
        for col in 0..cols {
            let color = COVERS[(row * cols + col) % COVERS.len()];
            window = window.child(
                div()
                    .absolute()
                    .left(px(main_x + col as f32 * (tile + tile_gap)))
                    .top(px(grid_top + row as f32 * (tile + tile_gap)))
                    .size(px(tile))
                    .rounded(px((tile * 0.12).clamp(1.5, 4.)))
                    .bg(hex(color)),
            );
        }
    }
    // The player.
    window = window.child(
        div()
            .absolute()
            .left(px(bar_left))
            .top(px(bar_top))
            .w(px(bar_w))
            .h(px(bar_h))
            .rounded(px(bar_h * 0.45))
            .bg(panel)
            .px(px(bar_h * 0.35))
            .flex()
            .items_center()
            .gap(px(bar_h * 0.4))
            .child(
                div()
                    .w(px(bar_w * 0.09))
                    .h(px(bar_h * 0.5))
                    .rounded(px(2.))
                    .bg(p.accent),
            )
            .child(div().flex_1().h(px(2.)).rounded(px(1.)).bg(dim)),
    );
    // The line being sung, under the player.
    if m.lyrics {
        window = window.child(
            div()
                .absolute()
                .left(px(bar_left + bar_w * 0.3))
                .top(px(bar_top + bar_h + lyric_room * 0.45))
                .w(px(bar_w * 0.4))
                .h(px(2.))
                .rounded(px(1.))
                .bg(p.ink.opacity(0.85)),
        );
    }
    window
}

/// A switch drawn as a picture of one: the whole tile around it is what is clicked.
fn switch_mark(on: bool, p: &Palette) -> Div {
    div()
        .relative()
        .flex_shrink_0()
        .w(px(32.))
        .h(px(19.))
        .rounded_full()
        .bg(if on { p.accent } else { p.line })
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(if on { 15. } else { 2. }))
                .size(px(15.))
                .rounded_full()
                .bg(white())
                .shadow_sm(),
        )
}

/// The check on the chosen card's corner.
fn badge(p: &Palette) -> Div {
    div()
        .absolute()
        .top(px(-7.))
        .right(px(-7.))
        .size(px(22.))
        .rounded_full()
        .bg(p.accent)
        .border_2()
        .border_color(p.canvas)
        .flex()
        .items_center()
        .justify_center()
        .child(glyph("check").size(px(15.)).text_color(hsla(0., 0., 0.1, 1.)))
}

/// Cards in rows of `cols` (rows of their own: a wrapping row grows too tall in GPUI).
fn rows(cards: Vec<AnyElement>, cols: usize) -> Div {
    let mut cards = cards.into_iter().peekable();
    let mut column = div().flex().flex_col().gap(px(GAP));
    while cards.peek().is_some() {
        column = column.child(
            div()
                .flex()
                .gap(px(GAP))
                .children(cards.by_ref().take(cols)),
        );
    }
    column
}

/// The width of each of `cols` cards across `width`.
fn each(width: f32, cols: usize) -> f32 {
    ((width - GAP * (cols as f32 - 1.)) / cols as f32).floor()
}

impl AppView {
    /// A group's name in the display face, and what it is for.
    fn group_title(&self, title: &str, about: &str, cx: &App) -> Div {
        let p = pal(cx);
        div()
            .mt(px(30.))
            .mb(px(12.))
            .flex()
            .items_end()
            .gap_3()
            .child(
                div()
                    .font_family(SharedString::from(super::theme::display_font()))
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .pb(px(2.))
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child(about.to_string()),
            )
    }

    /// A choice as a picture: the chosen one has an accent edge and a check on its corner.
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
            .relative()
            .w(px(width))
            .flex_shrink_0()
            .p(px(6.))
            .pb(px(9.))
            .rounded(px(14.))
            .bg(p.raised)
            .border_2()
            .border_color(if active {
                p.accent
            } else {
                transparent_black()
            })
            .cursor_pointer()
            .hover(|s| s.bg(p.raised_hover))
            .flex()
            .flex_col()
            .gap(px(7.))
            .child(picture)
            .child(
                div()
                    .px(px(3.))
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(name.to_string()),
                    )
                    .when_some(about, |el, about| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(p.ink_2)
                                .truncate()
                                .child(about.to_string()),
                        )
                    }),
            )
            .when(active, |el| el.child(badge(&p)))
            .on_click(cx.listener(move |this, _, window, cx| {
                choose(this, window, cx);
                this.persist_settings();
                cx.notify();
            }))
            .into_any_element()
    }

    /// An on/off setting as a tile: a picture of what it does, its name, and a switch. The
    /// whole tile turns it on or off.
    #[allow(clippy::too_many_arguments)]
    fn toggle_tile(
        &self,
        id: &str,
        width: f32,
        on: bool,
        picture: Option<AnyElement>,
        name: &str,
        about: &str,
        flip: impl Fn(&mut Self, bool, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        div()
            .id(SharedString::from(format!("tile-{id}")))
            .w(px(width))
            .flex_shrink_0()
            .p(px(10.))
            .rounded(px(14.))
            .bg(p.raised)
            .border_2()
            .border_color(if on {
                p.accent.opacity(0.55)
            } else {
                transparent_black()
            })
            .cursor_pointer()
            .hover(|s| s.bg(p.raised_hover))
            .flex()
            .flex_col()
            .gap(px(9.))
            .children(picture)
            .child(
                div()
                    .px(px(3.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name.to_string()),
                            )
                            .when(!about.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.ink_2)
                                        .child(about.to_string()),
                                )
                            }),
                    )
                    .child(switch_mark(on, &p)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                flip(this, !on, window, cx);
                this.persist_settings();
                cx.notify();
            }))
    }

    /// The window as chosen now, small, with the choices named beside it.
    fn appearance_preview(&self, width: f32, cx: &App) -> Div {
        let p = pal(cx);
        let s = &self.settings;
        let ambient = self.ambient_look();
        let glass = self.glass_chosen();
        let m = Mini {
            side: if s.layout.compact_sidebar {
                Side::Strip
            } else {
                Side::Full
            },
            search: !s.layout.search_hidden,
            player_on_top: s.layout.player_on_top,
            compact: s.layout.row_height < 50.,
            lyrics: s.player_lyrics,
            backdrop: if ambient {
                Backdrop::Ambient
            } else if glass {
                Backdrop::Glass
            } else {
                Backdrop::Plain
            },
        };
        let narrow = width < 460.;
        let picture_w = if narrow {
            width - 36.
        } else {
            (width * 0.56).min(330.)
        };
        let look = super::theme::Base::ALL
            .iter()
            .find(|(mode, _, _)| {
                super::theme::Base::from_name(mode) == super::theme::Base::from_name(&s.theme)
            })
            .map(|(_, name, _)| name.to_string())
            .unwrap_or_else(|| "Your theme".into());
        let names = [
            look,
            match m.backdrop {
                Backdrop::Plain => "Plain",
                Backdrop::Ambient => "Ambient",
                Backdrop::Glass => "Glass",
            }
            .into(),
            if s.layout.player_on_top {
                "Player on top"
            } else {
                "Player at the bottom"
            }
            .into(),
            if m.compact { "Compact" } else { "Roomy" }.into(),
        ];
        let chip = |text: String| {
            div()
                .px(px(8.))
                .py(px(3.))
                .rounded(px(6.))
                .bg(p.raised_hover)
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        div()
            .mt(px(14.))
            .p(px(18.))
            .rounded(px(18.))
            .bg(linear_gradient(
                100.,
                linear_color_stop(p.glow.opacity(0.22), 0.),
                linear_color_stop(p.raised.opacity(0.5), 0.7),
            ))
            .flex()
            .when(narrow, |el| el.flex_col())
            .gap(px(20.))
            .items_center()
            .child(
                div()
                    .rounded(px(8.))
                    .shadow_lg()
                    .border_1()
                    .border_color(p.line_soft)
                    .child(mini(picture_w, m, &p)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .font_family(SharedString::from(super::theme::display_font()))
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Your Needle"),
                    )
                    .child(meta("What you choose below shows here first.", cx))
                    .child(
                        div()
                            .mt(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .children(names[..2].iter().cloned().map(chip)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .children(names[2..].iter().cloned().map(chip)),
                            ),
                    ),
            )
    }

    /// Glass is chosen and can show (Windows' or macOS's own glass, with Ambient off).
    fn glass_chosen(&self) -> bool {
        use super::glass::Material;
        (cfg!(windows) || cfg!(target_os = "macos"))
            && !self.ambient_look()
            && Material::from_name(&self.settings.window_material) != Material::Solid
    }

    /// Settings › Appearance, `width` wide.
    pub(super) fn appearance_page(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let s = self.settings.clone();
        let mut page = div()
            .w(px(width))
            .flex()
            .flex_col()
            .child(self.appearance_preview(width, cx));

        // Look: the base looks and themes.
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
        let card = each(width, cols);
        let look_cards = looks
            .into_iter()
            .map(|(mode, name, about)| {
                self.theme_card(mode, name, about, card, cx)
                    .into_any_element()
            })
            .collect();
        page = page
            .child(self.group_title("Look", "Colors for the whole window", cx))
            .child(rows(look_cards, cols))
            .child(div().mt(px(12.)).child(self.theme_actions(cx)))
            .children(self.theme_editor_view(cx));

        // Background: plain, the music's cover, or the system's glass.
        let ambient = self.ambient_look();
        let glass = self.glass_chosen();
        let glass_offered = cfg!(windows) || cfg!(target_os = "macos");
        let (allowed, win11) = self.glass_system;
        let cols = if glass_offered { 3 } else { 2 };
        let card = each(width, cols);
        let picture = card - 16.;
        let mut backgrounds = vec![
            self.pick(
                "plain",
                card,
                !ambient && !glass,
                mini(picture, Mini::default(), &p),
                "Plain",
                Some("The look's own colors"),
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
                mini(
                    picture,
                    Mini {
                        backdrop: Backdrop::Ambient,
                        ..Mini::default()
                    },
                    &p,
                ),
                "Ambient",
                Some("The playing cover, blurred"),
                |this, _, _| this.settings.ambient = true,
                cx,
            ),
        ];
        if glass_offered {
            let about = if cfg!(target_os = "macos") {
                "Liquid Glass"
            } else if !allowed {
                "Needs Windows' transparency effects"
            } else {
                "Mica or Acrylic"
            };
            backgrounds.push(self.pick(
                "glass",
                card,
                glass,
                mini(
                    picture,
                    Mini {
                        backdrop: Backdrop::Glass,
                        ..Mini::default()
                    },
                    &p,
                ),
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
        page = page
            .child(self.group_title("Background", "What sits behind the pages", cx))
            .child(rows(backgrounds, cols))
            .when(glass && cfg!(windows), |el| {
                el.child(div().mt(px(6.)).children(self.glass_settings(cx)))
            });

        // Colors from the music, and film grain, side by side.
        let wide = width >= 520.;
        let tint_w = if wide { each(width, 2) + 60. } else { width };
        let grain_w = if wide { width - tint_w - GAP } else { width };
        let tint_art = {
            let on = s.music_colors;
            div()
                .h(px(56.))
                .rounded(px(9.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(10.))
                .bg(if on {
                    linear_gradient(
                        90.,
                        linear_color_stop(p.glow.opacity(0.4), 0.),
                        linear_color_stop(p.canvas, 1.),
                    )
                } else {
                    p.canvas.into()
                })
                .child(div().size(px(38.)).rounded(px(6.)).bg(p.glow))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .h(px(5.))
                                .rounded(px(3.))
                                .bg(if on { p.glow } else { p.line }),
                        )
                        .child(
                            div()
                                .h(px(5.))
                                .w(px(tint_w * 0.3))
                                .rounded(px(3.))
                                .bg(p.line),
                        ),
                )
                .into_any_element()
        };
        let tint = self.toggle_tile(
            "music-colors",
            tint_w,
            s.music_colors,
            Some(tint_art),
            "Colors from the music",
            "Tint Needle with the cover that plays.",
            |this, on, _, _| this.settings.music_colors = on,
            cx,
        );
        let grain_strength = s.grain.clamp(0., 1.);
        let grain_tile = self.grain_file.clone().flatten();
        let swatch = div()
            .relative()
            .size(px(56.))
            .flex_shrink_0()
            .rounded(px(9.))
            .overflow_hidden()
            .bg(linear_gradient(
                135.,
                linear_color_stop(p.glow, 0.),
                linear_color_stop(motion::mix(p.glow, hsla(0., 0., 0., 1.), 0.5), 1.),
            ))
            .when_some(grain_tile, |el, path| {
                el.child(
                    img(path)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(px(128.))
                        .opacity(grain_strength * 0.6),
                )
            });
        let grain = div()
            .w(px(grain_w))
            .flex_shrink_0()
            .p(px(10.))
            .rounded(px(14.))
            .bg(p.raised)
            .flex()
            .flex_col()
            .justify_between()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(swatch)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Film grain"),
                            )
                            .child(meta(
                                if s.grain <= 0.001 {
                                    "Off".to_string()
                                } else {
                                    format!("{:.0}%", s.grain * 100.)
                                },
                                cx,
                            )),
                    ),
            )
            .child(gpui_component::slider::Slider::new(&self.grain_slider));
        page = page
            .child(
                div()
                    .mt(px(GAP))
                    .flex()
                    .when(!wide, |el| el.flex_col())
                    .gap(px(GAP))
                    .child(tint)
                    .child(grain),
            )
            .when(!s.music_colors && self.accent_choice_applies(cx), |el| {
                el.child(self.accent_settings(cx))
            });

        // Layout: four questions, each with two pictures.
        let two_up = width >= 560.;
        let group_w = if two_up { each(width, 2) } else { width };
        let card = each(group_w, 2);
        let picture = card - 16.;
        let question = |title: &str, a: AnyElement, b: AnyElement| {
            div()
                .w(px(group_w))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.ink_2)
                        .child(title.to_string()),
                )
                .child(div().flex().gap(px(GAP)).child(a).child(b))
                .into_any_element()
        };
        let on_top = s.layout.player_on_top;
        let strip = s.layout.compact_sidebar;
        let compact = s.layout.row_height < 50.;
        let searchless = s.layout.search_hidden;
        let questions = vec![
            question(
                "The player",
                self.pick(
                    "player-bottom",
                    card,
                    !on_top,
                    mini(picture, Mini::default(), &p),
                    "At the bottom",
                    None,
                    |this, _, _| this.settings.layout.player_on_top = false,
                    cx,
                ),
                self.pick(
                    "player-top",
                    card,
                    on_top,
                    mini(
                        picture,
                        Mini {
                            player_on_top: true,
                            ..Mini::default()
                        },
                        &p,
                    ),
                    "In the top bar",
                    None,
                    |this, _, _| this.settings.layout.player_on_top = true,
                    cx,
                ),
            ),
            question(
                "A folded sidebar (Ctrl+B)",
                self.pick(
                    "sidebar-hides",
                    card,
                    !strip,
                    mini(
                        picture,
                        Mini {
                            side: Side::Hidden,
                            ..Mini::default()
                        },
                        &p,
                    ),
                    "Hides",
                    None,
                    |this, _, _| this.settings.layout.compact_sidebar = false,
                    cx,
                ),
                self.pick(
                    "sidebar-strip",
                    card,
                    strip,
                    mini(
                        picture,
                        Mini {
                            side: Side::Strip,
                            ..Mini::default()
                        },
                        &p,
                    ),
                    "Icon strip",
                    None,
                    |this, _, _| this.settings.layout.compact_sidebar = true,
                    cx,
                ),
            ),
            question(
                "Density",
                self.pick(
                    "roomy",
                    card,
                    !compact,
                    mini(picture, Mini::default(), &p),
                    "Roomy",
                    None,
                    |this, _, _| this.settings.layout.row_height = 58.,
                    cx,
                ),
                self.pick(
                    "compact",
                    card,
                    compact,
                    mini(
                        picture,
                        Mini {
                            compact: true,
                            ..Mini::default()
                        },
                        &p,
                    ),
                    "Compact",
                    None,
                    |this, _, _| this.settings.layout.row_height = 36.,
                    cx,
                ),
            ),
            question(
                "The search field",
                self.pick(
                    "search-shown",
                    card,
                    !searchless,
                    mini(
                        picture,
                        Mini {
                            search: true,
                            ..Mini::default()
                        },
                        &p,
                    ),
                    "In the sidebar",
                    None,
                    |this, _, _| this.settings.layout.search_hidden = false,
                    cx,
                ),
                self.pick(
                    "search-hidden",
                    card,
                    searchless,
                    mini(picture, Mini::default(), &p),
                    if cfg!(target_os = "macos") {
                        "⌘K only"
                    } else {
                        "Ctrl+K only"
                    },
                    None,
                    |this, _, _| this.settings.layout.search_hidden = true,
                    cx,
                ),
            ),
        ];
        let mut layout = div().flex().flex_col().gap(px(18.));
        let mut questions = questions.into_iter().peekable();
        while questions.peek().is_some() {
            layout = layout.child(
                div()
                    .flex()
                    .gap(px(GAP))
                    .children(questions.by_ref().take(if two_up { 2 } else { 1 })),
            );
        }
        page = page
            .child(self.group_title("Layout", "Where things sit in the window", cx))
            .child(layout);

        // While music plays: the immersive cover large, lyrics and server marks beside it.
        let side_by_side = width >= 520.;
        let big_w = if side_by_side {
            (width * 0.55).floor()
        } else {
            width
        };
        let small_w = if side_by_side {
            width - big_w - GAP
        } else {
            width
        };
        let immersive_art = {
            let on = s.immersive_cover;
            let art_w = big_w - 24.;
            let h = 150.;
            let cover = if on {
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(art_w * 0.55))
                    .h(px(h))
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(p.glow, 0.55),
                        linear_color_stop(p.glow.opacity(0.), 1.),
                    ))
            } else {
                div()
                    .absolute()
                    .left(px(art_w * 0.1))
                    .top(px(h * 0.24))
                    .size(px(h * 0.52))
                    .rounded(px(6.))
                    .bg(p.glow)
            };
            div()
                .relative()
                .w(px(art_w))
                .h(px(h))
                .rounded(px(9.))
                .overflow_hidden()
                .bg(if on {
                    linear_gradient(
                        90.,
                        linear_color_stop(motion::mix(p.glow, p.canvas, 0.75), 0.),
                        linear_color_stop(p.canvas, 0.8),
                    )
                } else {
                    p.canvas.into()
                })
                .child(cover)
                .child(
                    div()
                        .absolute()
                        .left(px(art_w * 0.58))
                        .top(px(h * 0.36))
                        .w(px(art_w * 0.32))
                        .flex()
                        .flex_col()
                        .gap(px(7.))
                        .child(div().h(px(7.)).rounded(px(3.)).bg(p.ink))
                        .child(
                            div()
                                .h(px(5.))
                                .w(px(art_w * 0.22))
                                .rounded(px(3.))
                                .bg(p.ink_3),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(art_w * 0.58))
                        .top(px(h * 0.72))
                        .w(px(art_w * 0.32))
                        .h(px(3.))
                        .rounded(px(2.))
                        .bg(p.line)
                        .child(
                            div()
                                .h_full()
                                .w(px(art_w * 0.13))
                                .rounded(px(2.))
                                .bg(p.accent),
                        ),
                )
                .into_any_element()
        };
        let lyrics_art = {
            let on = s.player_lyrics;
            div()
                .relative()
                .h(px(64.))
                .rounded(px(9.))
                .bg(p.canvas)
                .flex()
                .flex_col()
                .items_center()
                .pt(px(12.))
                .gap(px(6.))
                .child(
                    div()
                        .w(px(small_w - 48.))
                        .h(px(20.))
                        .rounded(px(7.))
                        .bg(p.raised_hover)
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().size(px(12.)).rounded(px(3.)).bg(p.glow))
                        .child(div().flex_1().h(px(3.)).rounded(px(2.)).bg(p.line)),
                )
                .when(on, |el| {
                    el.child(
                        div()
                            .font_family(SharedString::from(super::theme::display_font()))
                            .italic()
                            .text_size(px(11.))
                            .text_color(p.ink)
                            .child("and the harbor lights come on"),
                    )
                })
                .into_any_element()
        };
        let server_art = {
            let on = s.server_mark;
            let song = |color: u32, title: &'static str| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .text_size(px(11.))
                    .child(div().size(px(16.)).rounded(px(4.)).bg(hex(color)))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.ink)
                            .child(title),
                    )
                    .child(div().text_color(p.ink_2).child("Mara Quinn"))
                    .when(on, |el| {
                        el.child(glyph("server").size(px(11.)).text_color(p.ink_3))
                    })
            };
            div()
                .h(px(64.))
                .rounded(px(9.))
                .bg(p.canvas)
                .px(px(10.))
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(8.))
                .child(song(COVERS[0], "Opening"))
                .child(song(COVERS[4], "Low Tide"))
                .into_any_element()
        };
        let immersive = self.toggle_tile(
            "immersive",
            big_w,
            s.immersive_cover,
            Some(immersive_art),
            "Immersive cover",
            "In the big player, the cover fills the left side and fades into its colors.",
            |this, on, _, _| this.settings.immersive_cover = on,
            cx,
        );
        let lyrics = self.toggle_tile(
            "player-lyrics",
            small_w,
            s.player_lyrics,
            Some(lyrics_art),
            "Lyrics under the player",
            "",
            |this, on, _, _| this.settings.player_lyrics = on,
            cx,
        );
        let server = self.toggle_tile(
            "server-mark",
            small_w,
            s.server_mark,
            Some(server_art),
            "Mark server songs",
            "",
            |this, on, _, _| this.settings.server_mark = on,
            cx,
        );
        page = page
            .child(self.group_title("While music plays", "The player and the songs", cx))
            .child(
                div()
                    .flex()
                    .when(!side_by_side, |el| el.flex_col())
                    .gap(px(GAP))
                    .child(immersive)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .gap(px(GAP))
                            .child(lyrics)
                            .child(server),
                    ),
            );

        // Text and motion: the title fonts as samples, and reduce motion.
        let fonts: &'static [_; 3] = &super::theme::DISPLAY_FONTS;
        let card = each(width, fonts.len());
        let font_cards = fonts
            .iter()
            .map(|(key, label, family)| {
                let active =
                    s.display_font == *key || (s.display_font.is_empty() && *key == fonts[0].0);
                self.pick(
                    &format!("font-{key}"),
                    card,
                    active,
                    div()
                        .h(px(52.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(SharedString::from(*family))
                        .text_size(px(32.))
                        .child("Aa"),
                    label,
                    None,
                    move |this, _, _| {
                        super::theme::set_display_font(key);
                        this.settings.display_font = (*key).into();
                    },
                    cx,
                )
            })
            .collect();
        let system_still = !motion::system_allows_animation();
        let reduce = self.toggle_tile(
            "reduce-motion",
            width,
            s.reduce_motion || system_still,
            None,
            "Reduce motion",
            if !system_still {
                "No fades, slides, or other animations."
            } else if cfg!(windows) {
                "Windows has animations turned off, so Needle keeps still too."
            } else {
                "Your system has animations turned off, so Needle keeps still too."
            },
            |this, on, _, cx| {
                this.settings.reduce_motion = on;
                cx.set_global(motion::Motion {
                    enabled: !on && motion::system_allows_animation(),
                });
            },
            cx,
        );
        page = page
            .child(self.group_title("Text and motion", "Titles and animations", cx))
            .child(rows(font_cards, fonts.len()))
            .child(div().mt(px(GAP)).child(reduce));

        // The layout file, to share.
        page.child(
            div()
                .mt(px(28.))
                .pt(px(14.))
                .border_t_1()
                .border_color(p.line_soft)
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(p.ink_2)
                        .child("Layout file: panel widths and row height, to share."),
                )
                .child(
                    small_button("import-layout", "Import")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| {
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
                        })),
                )
                .child(
                    small_button("export-layout", "Export")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| {
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
                        })),
                ),
        )
    }
}
