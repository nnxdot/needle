use super::{
    AppView, Page, Panel, pal,
    widgets::{artwork, faint, glyph, icon, icon_button, quality},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable, TitleBar,
    button::{Button, ButtonVariants},
    input::Input,
    slider::Slider,
};
use needle_core::{audio::Command, audio::Repeat, model::format_duration};

/// The height of the strip with the sung line under the player.
const LYRIC_STRIP: f32 = 22.;

/// The player bar's small round buttons on the right (loop, volume, sound, speakers, lyrics,
/// mini player, queue): their icons are three quarters of this, about 20 px, so they read
/// from across the room.
pub(super) const BAR_ICON: f32 = 26.;

impl AppView {
    /// The title bar's height: taller with the player in it. Over the big player it keeps its
    /// height, so the player's contents do not move when it opens.
    pub(super) fn title_height(&self, window: &Window) -> f32 {
        let server_frame =
            cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Server);
        if self.settings.layout.player_on_top && !server_frame {
            60.
        } else {
            48.
        }
    }

    pub(super) fn title_bar(
        &self,
        sidebar: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let width = f32::from(super::widgets::content_size(window).width);
        let narrow = width < 1100.;
        let hidden = self.settings.layout.sidebar_hidden;
        // With the sidebar folded to its strip, the corner is as wide, with the logo alone;
        // the button to unfold it is the strip's first icon. Hidden altogether, a small corner
        // keeps the logo and the button.
        let corner = match (hidden, self.settings.layout.compact_sidebar) {
            (false, _) => sidebar,
            (true, true) => RAIL,
            (true, false) => 92.,
        };
        // macOS draws its window buttons at the top left: the corner makes room for them.
        let corner = match (
            cfg!(target_os = "macos"),
            hidden,
            self.settings.layout.compact_sidebar,
        ) {
            // The buttons alone, above the strip of icons.
            (true, true, true) => MAC_BUTTONS + 8.,
            // The buttons, then the one to show the sidebar.
            (true, true, false) => MAC_BUTTONS + 48.,
            _ => corner,
        };
        // The title bar does not shrink its contents, so size the search field from the room
        // left beside the sidebar column, back button, palette button, and window buttons.
        let search_width =
            (width - corner - 32. - 36. - if narrow { 44. } else { 96. } - 170.).clamp(140., 520.);
        // The sidebar corner, back button, search, and palette button; or, as in Apple Music,
        // the player (Settings › Appearance).
        // Over the big player, its own buttons: one bar, with the player showing through.
        let over_big = self.big && !self.big_moving;
        let content = if over_big {
            self.big_bar(cx)
        } else if self.settings.layout.player_on_top && !self.big {
            self.top_player(corner, hidden, width, cx)
        } else {
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .child(self.title_corner(corner, hidden, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(self.nav_group(narrow, cx))
                        .map(|el| {
                            if hidden && !self.settings.layout.search_hidden {
                                el.child(self.search_group(
                                    search_width,
                                    narrow,
                                    false,
                                    false,
                                    window,
                                    cx,
                                ))
                            } else {
                                el.child(
                                    div()
                                        .ml_1()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink_2)
                                        .truncate()
                                        .child(self.page.title()),
                                )
                            }
                        })
                        .child(div().flex_1()),
                )
        };
        // Some Linux desktops (and WSL) draw their own frame with window buttons: then show
        // the bar without Needle's own buttons.
        if cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Server) {
            return div()
                .h(px(48.))
                .flex_shrink_0()
                .flex()
                .when(!over_big, |el| el.bg(p.back))
                .child(content)
                .into_any_element();
        }
        TitleBar::new()
            .h(px(self.title_height(window)))
            .pl_0()
            .bg(if over_big {
                transparent_black()
            } else {
                p.back
            })
            .border_b_0()
            // A thin strip that is not a drag area, so Windows offers top-edge resizing.
            .when(!window.is_maximized(), |el| {
                el.child(
                    div()
                        .id("resize-top")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(5.))
                        .occlude(),
                )
            })
            .child(content)
            .into_any_element()
    }

    fn nav_item(
        &self,
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        glyph_name: &'static str,
        page: Page,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.nav_entry(id, name, glyph_name, None, page, cx)
    }

    /// A sidebar entry, with a picture (a playlist's covers) in place of its icon when given.
    fn nav_entry(
        &self,
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        glyph_name: &'static str,
        picture: Option<AnyElement>,
        page: Page,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let glass = p.back.a < 1.;
        let active = self.page == page
            || matches!(
                (&self.page, &page),
                (Page::Album { .. }, Page::Albums)
                    | (Page::Artist(_), Page::Artists)
                    | (Page::Folder(_), Page::Folders)
            );
        if self.settings.layout.sidebar_hidden {
            let name: SharedString = name.into();
            return rail_item(id, glyph_name, picture, name, active, cx).on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            );
        }
        div()
            .id(id)
            .flex_shrink_0()
            .h(px(34.))
            .mx_2()
            .px(px(10.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .gap(px(10.))
            .cursor_pointer()
            .text_size(px(13.5))
            .when(active, |el| {
                el.bg(p.accent_soft)
                    .text_color(p.ink)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!active, |el| {
                // On glass, grey text over a see-through layer loses its edges: use full ink
                // and a touch more weight there so labels stay crisp.
                el.text_color(if glass { p.ink } else { p.ink_2 })
                    .when(glass, |el| el.font_weight(FontWeight::MEDIUM))
                    .hover(|s| s.bg(p.raised.opacity(0.6)).text_color(p.ink))
            })
            .child(picture.unwrap_or_else(|| {
                glyph(glyph_name)
                    .size(px(17.))
                    .text_color(if active { p.accent } else { p.ink_3 })
                    .into_any_element()
            }))
            .child(div().flex_1().min_w_0().truncate().child(name.into()))
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            )
    }

    fn section(&self, label: &'static str, cx: &App) -> Div {
        // On the strip, a group starts with a thin line instead of its name.
        if self.settings.layout.sidebar_hidden {
            return div()
                .flex_shrink_0()
                .mx(px(14.))
                .my_2()
                .h(px(1.))
                .bg(pal(cx).line_soft);
        }
        div()
            .flex_shrink_0()
            .mt_5()
            .mb_1()
            .px_5()
            .h(px(22.))
            .flex()
            .items_center()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(pal(cx).ink_3)
            .child(label)
    }

    /// The Collections heading, with a + to add more collections (Rated, Most played, …).
    fn collections_section(&self, cx: &mut Context<Self>) -> Div {
        if self.settings.layout.sidebar_hidden {
            return self.section("Collections", cx);
        }
        let p = pal(cx);
        self.section("Collections", cx)
            .group("collections-head")
            .justify_between()
            .pr(px(14.))
            .child(
                div()
                    .id("add-collection")
                    .size(px(20.))
                    .rounded(px(5.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover("collections-head", |s| s.opacity(1.))
                    .hover(|s| s.bg(p.raised_hover))
                    .child(glyph("plus").size(px(13.)).text_color(p.ink_2))
                    .tooltip(|window, cx| {
                        gpui_component::tooltip::Tooltip::new("Add a collection").build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_collections_menu(event.position, cx)
                        }),
                    ),
            )
    }

    pub(super) fn sidebar(
        &self,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let rail = self.settings.layout.sidebar_hidden;
        let floating = self.on_glass();
        div()
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .relative()
            .flex()
            .flex_col()
            // macOS: the sidebar floats on glass from the window's top to its foot, the window
            // buttons in it.
            // Its rows keep clear of the glass's edges.
            .when(floating, |el| {
                el.pl(px(8.))
                    .pr(px(6.))
                    .pb(px(18.))
                    .child(super::glass::probe(
                        "sidebar",
                        (8., 8., 6., 8.),
                        true,
                        20.,
                        true,
                    ))
            })
            // The search field at the top, as on the Mac (with the sidebar folded, it sits in
            // the title bar).
            .when(
                !rail && !self.settings.layout.search_hidden && !self.big,
                |el| {
                    el.child(
                        div()
                            .px(px(10.))
                            .pt(px(4.))
                            .pb(px(8.))
                            .child(self.search_group(width - 20., true, false, false, window, cx)),
                    )
                },
            )
            // Everything above the bottom buttons scrolls together, so a short window never
            // pushes Settings and the others out of reach.
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .pt_1()
                    .when(rail, |el| el.gap_1())
                    .when(rail, |el| {
                        el.child(
                            rail_item(
                                "unfold-sidebar",
                                "menu",
                                None,
                                "Unfold the sidebar (Ctrl+B)".into(),
                                false,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_sidebar();
                                cx.notify();
                            })),
                        )
                    })
                    .child(self.nav_item("nav-home", "Home", "home", Page::Home, cx))
                    .child(self.nav_item("nav-songs", "Songs", "songs", Page::Songs, cx))
                    .child(self.nav_item("nav-albums", "Albums", "albums", Page::Albums, cx))
                    .child(self.nav_item("nav-artists", "Artists", "artists", Page::Artists, cx))
                    .child(self.nav_item("nav-folders", "Folders", "folder", Page::Folders, cx))
                    .child(self.collections_section(cx))
                    .child(
                        self.nav_item("nav-favorites", "Favorites", "heart", Page::Favorites, cx)
                            .drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| {
                                s.bg(p.accent_soft)
                            })
                            .on_drop(cx.listener(
                                |this, dragged: &super::flow::DraggedTracks, _, cx| {
                                    this.set_rating(&dragged.ids, 5);
                                    this.notify(if dragged.ids.len() == 1 {
                                        "Added to favorites.".to_string()
                                    } else {
                                        format!("Added {} songs to favorites.", dragged.ids.len())
                                    });
                                    cx.notify();
                                },
                            )),
                    )
                    .child(self.nav_item(
                        "nav-recent",
                        "Recently added",
                        "recent",
                        Page::Recent,
                        cx,
                    ))
                    .child(self.nav_item(
                        "nav-history",
                        "Listening history",
                        "history",
                        Page::History,
                        cx,
                    ))
                    .children(
                        self.settings
                            .layout
                            .collections
                            .clone()
                            .into_iter()
                            .filter_map(|key| {
                                let &(_, name, icon, _) = super::more_collection(&key)?;
                                Some(self.nav_item(
                                    SharedString::from(format!("nav-collection-{key}")),
                                    name,
                                    icon,
                                    Page::Collection(key),
                                    cx,
                                ))
                            })
                            .collect::<Vec<_>>(),
                    )
                    .child(if rail {
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(self.section("Playlists", cx).w(px(width - 28.)))
                            .child(
                                icon_button("new-playlist", "plus", "New playlist")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_playlist_editor(None, vec![], None, window, cx);
                                    })),
                            )
                    } else {
                        self.section("Playlists", cx)
                            .justify_between()
                            .pr_3()
                            .child(
                                icon_button("new-playlist", "plus", "New playlist")
                                    .xsmall()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_playlist_editor(None, vec![], None, window, cx);
                                    })),
                            )
                    })
                    .child(
                        div()
                            .id("playlists-list")
                            .flex_shrink_0()
                            .pb_2()
                            .when(rail, |el| el.flex().flex_col().gap_1())
                            .when(self.playlists.is_empty() && !rail, |el| {
                                el.child(
                                    faint(
                                        "Save a search or a set of tracks and it will appear here.",
                                        cx,
                                    )
                                    .px_5()
                                    .py_1()
                                    .line_height(relative(1.45)),
                                )
                            })
                            .children(self.playlists.iter().map(|playlist| {
                                let glyph_name = if playlist.query.is_some() {
                                    "smart"
                                } else {
                                    "playlist"
                                };
                                self.nav_entry(
                                    SharedString::from(format!("playlist-{}", playlist.id)),
                                    playlist.name.clone(),
                                    glyph_name,
                                    self.playlist_icon(
                                        &playlist.id,
                                        if rail { 26. } else { 20. },
                                        cx,
                                    ),
                                    Page::Playlist(playlist.id.clone()),
                                    cx,
                                )
                                .on_mouse_down(MouseButton::Right, {
                                    let id = playlist.id.clone();
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.open_playlist_menu(id.clone(), event.position, cx)
                                    })
                                })
                                .when(
                                    playlist.query.is_none(),
                                    |el| {
                                        let id = playlist.id.clone();
                                        el.drag_over::<super::flow::DraggedTracks>(
                                            move |s, _, _, _| s.bg(p.accent_soft),
                                        )
                                        .on_drop(cx.listener(
                                            move |this,
                                                  dragged: &super::flow::DraggedTracks,
                                                  _,
                                                  cx| {
                                                let tracks = this
                                                    .library
                                                    .tracks_by_ids(&dragged.ids)
                                                    .unwrap_or_default();
                                                this.add_to_playlist(&id, tracks);
                                                cx.notify();
                                            },
                                        ))
                                    },
                                )
                            })),
                    ),
            )
            .child(
                div()
                    .py_2()
                    .when(rail, |el| el.flex().flex_col().gap_1())
                    .when_some(self.scan.as_ref().filter(|_| rail), |el, scan| {
                        let cancel = self.cancel.clone();
                        let busy = if scan.scanned == 0 {
                            "Importing… Click to stop".to_string()
                        } else {
                            format!("Importing · {} files. Click to stop", scan.scanned)
                        };
                        el.child(
                            rail_item("importing", "import", None, busy.into(), true, cx).on_click(
                                move |_, _, _| {
                                    cancel.store(true, std::sync::atomic::Ordering::Relaxed)
                                },
                            ),
                        )
                    })
                    .when_some(self.scan.as_ref().filter(|_| !rail), |el, scan| {
                        let cancel = self.cancel.clone();
                        el.child(
                            div()
                                .mx_2()
                                .mb_1()
                                .px(px(10.))
                                .py_2()
                                .rounded(px(6.))
                                .bg(p.raised)
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(if scan.scanned == 0 {
                                                    "Importing…".to_string()
                                                } else {
                                                    format!("Importing · {} files", scan.scanned)
                                                }),
                                        )
                                        .child(faint(scan.current.clone(), cx).truncate()),
                                )
                                .child(
                                    Button::new("cancel-import")
                                        .ghost()
                                        .xsmall()
                                        .label("Stop")
                                        .on_click(move |_, _, _| {
                                            cancel.store(true, std::sync::atomic::Ordering::Relaxed)
                                        }),
                                ),
                        )
                    })
                    .when(rail, |el| {
                        el.child(
                            rail_item(
                                "add-folder",
                                "folder",
                                None,
                                "Add music folder".into(),
                                false,
                                cx,
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.import_folder(window, cx)),
                            ),
                        )
                    })
                    .when(!rail, |el| {
                        el.child(
                            div()
                                .id("add-folder")
                                .h(px(34.))
                                .mx_2()
                                .px(px(10.))
                                .rounded(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .cursor_pointer()
                                .text_size(px(13.5))
                                .text_color(p.ink_2)
                                .hover(|s| s.bg(p.raised.opacity(0.6)).text_color(p.ink))
                                .child(glyph("folder").size(px(17.)).text_color(p.ink_3))
                                .child("Add music folder")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.import_folder(window, cx)
                                })),
                        )
                    })
                    .child(self.nav_item(
                        "nav-doctor",
                        "Fix my library",
                        "wrench",
                        Page::Doctor,
                        cx,
                    ))
                    .child(self.nav_item("nav-import", "Import", "import", Page::Import, cx))
                    .child(self.nav_item(
                        "nav-settings",
                        "Settings",
                        "settings",
                        Page::Settings,
                        cx,
                    )),
            )
    }

    pub(super) fn signal_path(&self, cx: &App) -> (String, String, bool) {
        let Some(item) = &self.playback.current else {
            return (String::new(), String::new(), false);
        };
        let track = &item.track;
        let source = quality(track);
        if self.playback.output_rate == 0 {
            return (
                source.clone(),
                format!("{source} · the output opens when playback starts"),
                false,
            );
        }
        let rate = self.playback.output_rate as f64 / 1000.;
        let rate = if rate.fract() == 0. {
            format!("{rate:.0} kHz")
        } else {
            format!("{rate:.1} kHz")
        };
        let _ = cx;
        if self.playback.exclusive {
            (
                format!("{source} → Exclusive {rate}"),
                format!(
                    "{source} → no processing → WASAPI exclusive at {rate} → {}. Volume and ReplayGain are bypassed.",
                    self.playback.output
                ),
                true,
            )
        } else {
            let resampled = track.sample_rate as u32 != self.playback.output_rate;
            (
                // Short enough for the chip; the tooltip tells the whole path.
                format!("{source} → {rate}"),
                format!(
                    "{source} →{} {} → Windows mixer at {rate} → {}",
                    if resampled { " resampled →" } else { "" },
                    if self.playback.replay_gain {
                        "ReplayGain and volume"
                    } else {
                        "volume"
                    },
                    self.playback.output
                ),
                false,
            )
        }
    }

    /// Settings > Appearance > Lyrics under the player: the line being sung, in a strip at the
    /// window's foot. Only for a song with timed lyrics; a note while no line is sung.
    pub(super) fn player_lyrics(&self, cx: &App) -> Option<AnyElement> {
        if !self.settings.player_lyrics {
            return None;
        }
        let p = pal(cx);
        let current = self.playback.current.as_ref()?;
        let lyrics = match &self.lyrics {
            Some((id, Some(lyrics))) if *id == current.track.id && !lyrics.lines.is_empty() => {
                lyrics
            }
            _ => return None,
        };
        let line = self
            .lyric_line
            .and_then(|i| lyrics.lines.get(i))
            .map(|l| l.text.trim())
            .filter(|t| !t.is_empty());
        let text = div()
            .w_full()
            .px_6()
            .text_center()
            .truncate()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if line.is_some() { p.ink } else { p.ink_3 })
            .child(line.unwrap_or("♪").to_string());
        // Each new line fades in.
        let serial = self.lyric_line.map_or(0, |i| i as u64 + 1);
        Some(
            div()
                .id("player-lyrics")
                .h(px(LYRIC_STRIP))
                .flex_shrink_0()
                .flex()
                .items_center()
                .bg(p.back)
                .child(super::motion::animate(
                    text,
                    ("player-lyric", serial),
                    220,
                    cx,
                    |el, t| el.opacity(t),
                ))
                .into_any_element(),
        )
    }

    pub(super) fn player_bar(&self, width: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        // With the sung line under it, the bar gives up the room under its controls.
        let lyrics_under = !self.settings.layout.player_on_top && self.player_lyrics(cx).is_some();
        let current = self.playback.current.as_ref().map(|i| i.track.clone());
        let wide = width > px(1180.);
        let (path, path_detail, exclusive) = self.signal_path(cx);
        let floating = self.on_glass();
        div()
            .h(px(match (floating, lyrics_under) {
                (true, _) => 76.,
                (false, true) => 70.,
                (false, false) => 80.,
            }))
            .flex_shrink_0()
            .relative()
            // macOS: the player floats on its own pane of glass, clear of the window's edges, over
            // the page's foot (and takes the pointer there).
            .when(floating, |el| {
                el.occlude()
                    .mx(px(12.))
                    .mb(px(12.))
                    .mt(px(4.))
                    .rounded(px(28.))
                    .child(super::glass::probe(
                        "player",
                        (0., 0., 0., 0.),
                        false,
                        28.,
                        true,
                    ))
            })
            // The playing cover's colour glows in from the left.
            .bg(linear_gradient(
                90.,
                linear_color_stop(
                    super::motion::mix(
                        p.back,
                        p.glow.opacity(p.back.a),
                        if p.dark { 0.2 } else { 0.16 },
                    ),
                    0.,
                ),
                linear_color_stop(p.back, 0.55),
            ))
            // On glass the buttons keep well clear of its round ends.
            .px(px(if floating { 26. } else { 16. }))
            .flex()
            .items_center()
            .gap_4()
            // Now playing (right-click for the song's menu)
            .child(
                div()
                    .w(px(if wide { 380. } else { 250. }))
                    .when_some(current.clone(), |el, track| {
                        el.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.open_playing_menu(track.clone(), event.position, cx)
                            }),
                        )
                    })
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("open-big")
                            .cursor_pointer()
                            .rounded(px(4.))
                            .hover(|s| s.opacity(0.85))
                            .child(artwork(current.as_ref(), 56., cx))
                            .tooltip(|window, cx| {
                                gpui_component::tooltip::Tooltip::new(super::widgets::keys(
                                    "Open the big player · Ctrl+P",
                                ))
                                .build(window, cx)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.big = true;
                                cx.notify();
                            })),
                    )
                    .child(match &current {
                        None => div()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .text_color(p.ink_2)
                                    .child("Nothing playing"),
                            )
                            .child(faint("Double-click a track to start.", cx)),
                        Some(track) => {
                            let album = super::album_page(track);
                            let artist = Page::Artist(track.artist.clone());
                            div()
                                .flex_shrink()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .id("now-title")
                                        .text_size(px(13.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.underline())
                                        .child(track.title.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.navigate(album.clone(), window, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id("now-artist")
                                        .text_size(px(12.))
                                        .text_color(p.ink_2)
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.underline().text_color(p.ink))
                                        .child(track.display_artist().to_string())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.navigate(artist.clone(), window, cx)
                                        })),
                                )
                        }
                    })
                    .when_some(current.clone(), |el, track| {
                        el.child(self.favorite_button(track, cx))
                    }),
            )
            // Transport
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(self.transport_buttons(38., cx))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(620.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                faint(
                                    format_duration(if current.is_some() {
                                        self.playback.position
                                    } else {
                                        0.
                                    }),
                                    cx,
                                )
                                .w(px(40.))
                                .text_right(),
                            )
                            .child(self.seek_bar("seek-bar", current.is_none(), cx))
                            .child(
                                faint(
                                    current
                                        .as_ref()
                                        .map(|t| format_duration(t.duration))
                                        .unwrap_or_else(|| "0:00".into()),
                                    cx,
                                )
                                .w(px(40.)),
                            ),
                    ),
            )
            // Output and volume: what is playing and how on top, the controls below it, so
            // the volume slider has room to be a useful length.
            .child(
                div()
                    .w(px(if wide { 380. } else { 300. }))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_end()
                    .justify_center()
                    .gap(px(4.))
                    .when(!path.is_empty(), |el| {
                        el.child(
                            div()
                                .id("signal-path")
                                .mr_1()
                                .px_2()
                                .py(px(1.))
                                .rounded(px(5.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .max_w_full()
                                .text_size(px(11.))
                                .text_color(if exclusive { p.accent } else { p.ink_2 })
                                .bg(if exclusive { p.accent_soft } else { p.raised })
                                .cursor_pointer()
                                .child(glyph("signal").size(px(12.)).text_color(if exclusive {
                                    p.accent
                                } else {
                                    p.ink_2
                                }))
                                .child(div().truncate().child(path.clone()))
                                .tooltip(move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(path_detail.clone())
                                        .build(window, cx)
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(Page::Settings, window, cx)
                                })),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_1()
                            .child(self.loop_button(cx))
                            .child(self.volume_control(if wide { 150. } else { 110. }, cx))
                            .child(self.sound_button(cx))
                            .child(self.speakers_button(cx))
                            .child(self.lyrics_button(cx))
                            .child(self.mini_button(cx))
                            .child(self.queue_button(cx)),
                    ),
            )
    }

    /// The heart for the song playing.
    pub(super) fn favorite_button(
        &self,
        track: needle_core::model::Track,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let favorite = self
            .tracks
            .iter()
            .find(|t| t.id == track.id)
            .map_or(track.rating, |t| t.rating)
            >= 4;
        div()
            .id("now-favorite")
            .size(px(28.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(p.raised_hover))
            .child(self.heart(
                &track.id,
                favorite,
                15.,
                if favorite { p.accent } else { p.ink_2 },
                cx,
            ))
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(if favorite {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                })
                .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_rating(
                    std::slice::from_ref(&track.id),
                    if favorite { 0 } else { 5 },
                );
                if let Some(item) = this.playback.current.as_mut() {
                    item.track.rating = if favorite { 0 } else { 5 };
                }
                cx.notify();
            }))
    }

    /// The A–B loop: set A, then B, and that part repeats; again to clear it.
    pub(super) fn loop_button(&self, cx: &mut Context<Self>) -> Button {
        let p = pal(cx);
        let current = &self.playback.current;
        let looping = self.playback.loop_range.is_some();
        Button::new("ab-loop")
            .ghost()
            .with_size(px(BAR_ICON))
            .icon(icon("loop"))
            .when(looping || self.loop_start.is_some(), |b| {
                b.text_color(p.accent)
            })
            .disabled(current.is_none())
            .tooltip(if looping {
                "A–B loop on · click to clear"
            } else if self.loop_start.is_some() {
                "Point A set · click again to set B and start looping"
            } else {
                "A–B loop · click to set point A"
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.step_loop();
                cx.notify();
            }))
    }

    /// The A–B loop's next step: set A, set B and loop, or clear it.
    pub(super) fn step_loop(&mut self) {
        if self.playback.loop_range.is_some() {
            self.loop_start = None;
            self.player.send(Command::Loop(None));
        } else if let Some(a) = self.loop_start.take() {
            let b = self.playback.position;
            if b > a + 0.5 {
                self.player.send(Command::Loop(Some((a, b))));
                self.notify(format!(
                    "Looping {} – {}",
                    format_duration(a),
                    format_duration(b)
                ));
            } else {
                self.fail("Point B must come after point A.");
            }
        } else {
            self.loop_start = Some(self.playback.position);
            self.notify(format!(
                "Point A set at {}. Click the loop button again at point B.",
                format_duration(self.playback.position)
            ));
        }
    }

    /// Mutes, or puts back the volume from before.
    pub(super) fn toggle_mute(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let volume = match self.muted_volume.take() {
            Some(v) => v,
            None => {
                self.muted_volume = Some(self.playback.volume.max(0.05));
                0.
            }
        };
        self.player.send(Command::Volume(volume));
        self.volume
            .update(cx, |s, cx| s.set_value(volume, window, cx));
    }

    /// Opens the equalizer and sound tools (lit while any is on).
    pub(super) fn sound_button(&self, cx: &mut Context<Self>) -> Button {
        let p = pal(cx);
        icon_button("open-sound", "eq", "Equalizer and sound tools")
            .with_size(px(BAR_ICON))
            .when(
                self.settings.dsp.eq || !self.settings.dsp.is_transparent(),
                |b| b.text_color(p.accent),
            )
            .on_click(cx.listener(|this, _, window, cx| this.navigate(Page::Sound, window, cx)))
    }

    /// Chooses the speaker to play on (lit while one is used).
    pub(super) fn speakers_button(&self, cx: &mut Context<Self>) -> Button {
        let p = pal(cx);
        let speaker = self.current_speaker();
        icon_button(
            "open-speakers",
            "cast",
            match &speaker {
                Some(s) => format!("Playing on {}", s.name),
                None => "Play on another speaker".into(),
            },
        )
        .with_size(px(BAR_ICON))
        .ml_1()
        .when(speaker.is_some(), |b| b.text_color(p.accent))
        .on_click(cx.listener(|this, _, window, cx| {
            let position = window.mouse_position() - point(px(0.), px(14.));
            this.open_speaker_menu(position, cx)
        }))
    }

    /// Opens the mini player.
    pub(super) fn mini_button(&self, cx: &mut Context<Self>) -> Button {
        icon_button("open-mini", "mini", "Mini player · Ctrl+M")
            .with_size(px(BAR_ICON))
            .ml_1()
            .on_click(cx.listener(|this, _, window, cx| this.open_mini(window, cx)))
    }

    /// Shuffle, previous, play or pause, next, and repeat; `play` is the play button's size.
    fn transport_buttons(&self, play: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let playing = self.playback.playing;
        let repeat = self.playback.repeat;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                icon_button("shuffle", "shuffle", "Shuffle this view")
                    .small()
                    .disabled(self.tracks.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.play_view(0, true, cx))),
            )
            .child(
                icon_button("previous", "previous", "Previous · Ctrl+Left")
                    .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Previous))),
            )
            .child(
                div()
                    .id("play-pause")
                    .size(px(play))
                    .rounded_full()
                    .bg(p.ink)
                    .text_color(p.canvas)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.88))
                    // Pressed: shrink inside the same space so nothing around it moves.
                    .active(move |s| s.size(px(play - 4.)).m(px(2.)).opacity(0.8))
                    .child(
                        glyph(if playing { "pause" } else { "play" })
                            .size(px((play * 0.45).round()))
                            .text_color(p.canvas),
                    )
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(if playing {
                            "Pause · Space"
                        } else {
                            "Play · Space"
                        })
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx))),
            )
            .child(
                icon_button("next", "next", "Next · Ctrl+Right")
                    .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Next))),
            )
            .child(
                icon_button(
                    "repeat",
                    if repeat == Repeat::One {
                        "repeat-one"
                    } else {
                        "repeat"
                    },
                    match repeat {
                        Repeat::Off => "Repeat is off",
                        Repeat::All => "Repeating the queue",
                        Repeat::One => "Repeating this track",
                    },
                )
                .small()
                .when(repeat != Repeat::Off, |b| b.text_color(p.accent))
                .on_click(cx.listener(|this, _, _, _| {
                    let next = match this.playback.repeat {
                        Repeat::Off => Repeat::All,
                        Repeat::All => Repeat::One,
                        Repeat::One => Repeat::Off,
                    };
                    this.player.send(Command::Repeat(next));
                })),
            )
    }

    /// Mute and the volume slider (`slider` points long); the wheel over them changes it.
    pub(super) fn volume_control(&self, slider: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let volume_glyph = if self.playback.volume <= 0.001 {
            "volume-off"
        } else if self.playback.volume < 0.5 {
            "volume-low"
        } else {
            "volume"
        };
        div()
            .id("volume-area")
            .flex()
            .items_center()
            .gap_1()
            // The wheel over the volume changes it, while Needle is the active window.
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if this.playback.exclusive || !window.is_window_active() {
                    return;
                }
                let steps = match event.delta {
                    // A wheel: one step per notch.
                    ScrollDelta::Lines(lines) => lines.y.signum(),
                    // A touchpad: steps as the fingers travel.
                    ScrollDelta::Pixels(pixels) => {
                        this.volume_scroll += f32::from(pixels.y) / 40.;
                        let whole = this.volume_scroll.trunc();
                        this.volume_scroll -= whole;
                        whole
                    }
                };
                if steps != 0. {
                    this.nudge_volume(steps * 0.05, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                icon_button(
                    "mute",
                    volume_glyph,
                    if self.playback.exclusive {
                        "Exclusive output: use your device's volume"
                    } else {
                        "Mute"
                    },
                )
                .with_size(px(BAR_ICON))
                .disabled(self.playback.exclusive)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_mute(window, cx))),
            )
            .child(
                Slider::new(&self.volume)
                    .w(px(slider))
                    .disabled(self.playback.exclusive),
            )
    }

    /// Opens and closes the lyrics beside the page.
    fn lyrics_button(&self, cx: &mut Context<Self>) -> Button {
        let lyrics_open = self.settings.show_inspector && self.panel == Panel::Lyrics;
        Button::new("lyrics-toggle")
            .ghost()
            .with_size(px(BAR_ICON))
            .ml_1()
            .icon(icon("lyrics"))
            .selected(lyrics_open)
            .tooltip(super::widgets::keys("Lyrics · Ctrl+L"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_panel(Panel::Lyrics);
                cx.notify();
            }))
    }

    /// Opens and closes the queue beside the page; songs dropped on it join the queue.
    pub(super) fn queue_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        let queue_open = self.settings.show_inspector && self.panel == Panel::Queue;
        div()
            .id("queue-drop")
            .rounded(px(6.))
            .drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| s.bg(p.accent_soft))
            .on_drop(
                cx.listener(|this, dragged: &super::flow::DraggedTracks, _, _| {
                    let tracks = this.library.tracks_by_ids(&dragged.ids).unwrap_or_default();
                    this.enqueue(tracks);
                }),
            )
            .child(
                Button::new("queue-toggle")
                    .ghost()
                    .with_size(px(BAR_ICON))
                    .ml_1()
                    .icon(icon("queue"))
                    .selected(queue_open)
                    .tooltip(super::widgets::keys("Queue · Ctrl+J"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_panel(Panel::Queue);
                        cx.notify();
                    })),
            )
    }

    /// The search field (unless hidden in Settings) and the command palette button. With
    /// `above`, the field sits at the bottom of the window and its suggestions open upward.
    pub(super) fn search_group(
        &self,
        search_width: f32,
        narrow: bool,
        above: bool,
        palette: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let search_focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let rule = self
            .explanation
            .as_deref()
            .is_some_and(|e| e.starts_with("Matches rule"))
            && !self.search_text(cx).is_empty();
        let field = !self.settings.layout.search_hidden;
        let glass_field = self.on_glass();
        div()
            .flex()
            .items_center()
            .gap_2()
            .when(field, |el| {
                el.child(
                    self.suggestion_keys(div().id("search-field"), cx)
                        .relative()
                        // One clean field: our own fill, edge, and height; the input inside
                        // draws no box of its own.
                        .h(px(32.))
                        .rounded(px(if glass_field { 16. } else { 8. }))
                        // macOS: a capsule of glass, as in macOS's own toolbars.
                        .when(glass_field, |el| el.glass())
                        .when(!glass_field, |el| {
                            el.bg(if p.back.a < 1. {
                                p.raised.opacity(0.75)
                            } else {
                                p.raised
                            })
                        })
                        .border_1()
                        .border_color(if search_focused {
                            p.accent.opacity(0.6)
                        } else if glass_field {
                            gpui::transparent_black()
                        } else {
                            p.line_soft
                        })
                        .flex()
                        .items_center()
                        .w(px(search_width))
                        .flex_shrink_0()
                        .children(self.suggestion_list(px(search_width.max(420.)), above, cx))
                        .child(
                            Input::new(&self.search)
                                .appearance(false)
                                .cleanable(true)
                                .prefix(glyph("search").size(px(15.)).text_color(p.ink_3))
                                .when(rule, |el| {
                                    el.suffix(
                                        div()
                                            .px(px(6.))
                                            .rounded(px(4.))
                                            .bg(p.accent_soft)
                                            .text_size(px(11.))
                                            .text_color(p.accent)
                                            .child("Rule"),
                                    )
                                }),
                        ),
                )
            })
            .when(palette, |el| {
                el.child(
                    div()
                        .id("open-palette")
                        .occlude()
                        .h(px(28.))
                        .px_2()
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(p.ink_3)
                        .hover(|s| s.bg(p.raised).text_color(p.ink))
                        .flex_shrink_0()
                        // On macOS the shortcut text starts with the same ⌘, so the icon only
                        // stands alone.
                        .when(!cfg!(target_os = "macos") || narrow, |el| {
                            el.child(glyph("command").size(px(14.)).text_color(p.ink_3))
                        })
                        .when(!narrow, |el| el.child(super::widgets::keys("Ctrl K")))
                        .tooltip(|window, cx| {
                            gpui_component::tooltip::Tooltip::new(
                                "Command palette: go anywhere, do anything",
                            )
                            .build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx))),
                )
            })
    }

    /// Back, Forward, and the command palette in one small rounded group, as on the Mac.
    fn nav_group(&self, narrow: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        div()
            .id("nav-group")
            .occlude()
            .flex_shrink_0()
            .h(px(32.))
            .px(px(4.))
            .rounded(px(16.))
            .bg(if p.dark {
                hsla(0., 0., 1., 0.055)
            } else {
                hsla(0., 0., 0., 0.04)
            })
            .border_1()
            .border_color(if p.dark {
                hsla(0., 0., 1., 0.06)
            } else {
                hsla(0., 0., 0., 0.06)
            })
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                icon_button("back", "chevron-left", "Back · Alt+Left")
                    .small()
                    .disabled(self.back.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx))),
            )
            .child(
                icon_button("forward", "chevron-right", "Forward · Alt+Right")
                    .small()
                    .disabled(self.forward.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.go_forward(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("open-palette")
                    .h(px(26.))
                    .px(px(8.))
                    .rounded(px(13.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(p.ink_3)
                    .hover(|s| s.bg(p.raised_hover).text_color(p.ink))
                    .child(glyph("command").size(px(13.)).text_color(p.ink_2))
                    .when(!narrow, |el| el.child("Ctrl K"))
                    .tooltip(|window, cx| {
                        gpui_component::tooltip::Tooltip::new(
                            "Command palette: go anywhere, do anything · Ctrl+K",
                        )
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx))),
            )
    }

    /// The title bar's corner above the sidebar: the logo, the name, and the fold button.
    fn title_corner(&self, corner: f32, hidden: bool, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let rail = hidden && self.settings.layout.compact_sidebar;
        let mac = cfg!(target_os = "macos");
        div()
            .w(px(corner))
            .flex_shrink_0()
            .h_full()
            .when(rail && !mac, |el| el.justify_center())
            .when(!rail || mac, |el| el.pl_4().pr_2())
            // Room for macOS's window buttons, left of the logo.
            .when(mac, |el| el.pl(px(MAC_BUTTONS)))
            .flex()
            .items_center()
            .gap_2()
            // On macOS a folded sidebar's corner holds only the window buttons.
            .when(!(mac && hidden), |el| {
                el.child(glyph("logo").size(px(20.)).text_color(p.accent))
            })
            // Not on macOS, where the window buttons take the room (the menu bar names it).
            .when(!hidden && !mac, |el| {
                el.child(
                    div()
                        .flex_1()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Needle"),
                )
            })
            .when(!hidden && mac, |el| el.child(div().flex_1()))
            // Not over the big player, where the sidebar is not shown.
            .when(!rail && !self.big, |el| {
                el.child(
                    icon_button(
                        "toggle-sidebar",
                        "panel",
                        if hidden {
                            "Show the sidebar (Ctrl+B)"
                        } else {
                            "Fold the sidebar (Ctrl+B)"
                        },
                    )
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_sidebar();
                        cx.notify();
                    })),
                )
            })
    }

    /// The title bar with the player in it, as in Apple Music: the controls on the left, the
    /// playing song and its time in the middle, and the volume, lyrics, and queue on the right.
    fn top_player(&self, corner: f32, hidden: bool, width: f32, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let current = self.playback.current.as_ref().map(|i| i.track.clone());
        let wide = width > 1180.;
        let time = |seconds: f64, cx: &App| {
            faint(format_duration(seconds), cx)
                .text_size(px(11.))
                .w(px(34.))
        };
        div()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .child(self.title_corner(corner, hidden, cx))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl_2()
                    // macOS Liquid Glass: the buttons on a capsule of glass.
                    .when(self.on_glass(), |el| {
                        el.ml_2().pr_2().py(px(4.)).rounded(px(22.)).glass()
                    })
                    // Each group claims the mouse, or Windows takes presses there (on the
                    // sliders too) as the title bar's, and never reports letting go.
                    .occlude()
                    // Back, Forward, and the palette in their group, as without the player here.
                    .child(self.nav_group(!wide, cx))
                    .child(div().w(px(6.)))
                    .child(self.transport_buttons(32., cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(600.))
                            .occlude()
                            .h(px(48.))
                            // macOS Liquid Glass: the song on a pane of glass.
                            .map(|el| {
                                if self.on_glass() {
                                    el.rounded(px(16.)).glass()
                                } else {
                                    el.rounded(px(8.)).bg(p.raised.opacity(if p.back.a < 1. {
                                        0.75
                                    } else {
                                        1.
                                    }))
                                }
                            })
                            .flex()
                            .items_center()
                            .gap_2()
                            .pl(px(4.))
                            .pr_3()
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("open-big")
                                    .flex_shrink_0()
                                    // Title bar clicks move the window unless something claims
                                    // them.
                                    .occlude()
                                    .when_some(current.clone(), |el, track| {
                                        el.on_mouse_down(
                                            MouseButton::Right,
                                            cx.listener(
                                                move |this, event: &MouseDownEvent, _, cx| {
                                                    this.open_playing_menu(
                                                        track.clone(),
                                                        event.position,
                                                        cx,
                                                    )
                                                },
                                            ),
                                        )
                                    })
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .child(artwork(current.as_ref(), 40., cx))
                                    .tooltip(|window, cx| {
                                        gpui_component::tooltip::Tooltip::new(super::widgets::keys(
                                            "Open the big player · Ctrl+P",
                                        ))
                                        .build(window, cx)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.big = true;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(match &current {
                                        None => div()
                                            .text_size(px(12.5))
                                            .text_color(p.ink_2)
                                            .text_center()
                                            .child("Nothing playing")
                                            .into_any_element(),
                                        Some(track) => {
                                            let album = super::album_page(track);
                                            // Centred by its row, so the hover style (which
                                            // knows nothing of alignment) cannot move it.
                                            div()
                                                .flex()
                                                .justify_center()
                                                .min_w_0()
                                                .child(
                                                    div()
                                                        .id("top-now-title")
                                                        .occlude()
                                                        .min_w_0()
                                                        .text_size(px(12.5))
                                                        .truncate()
                                                        .cursor_pointer()
                                                        .hover(|s| s.underline())
                                                        .child(format!(
                                                            "{} · {}",
                                                            track.title,
                                                            track.display_artist()
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.navigate(
                                                                    album.clone(),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                )
                                                .into_any_element()
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                time(
                                                    if current.is_some() {
                                                        self.playback.position
                                                    } else {
                                                        0.
                                                    },
                                                    cx,
                                                )
                                                .text_right(),
                                            )
                                            .child(self.seek_bar("seek-bar", current.is_none(), cx))
                                            .child(time(
                                                current.as_ref().map_or(0., |t| t.duration),
                                                cx,
                                            )),
                                    ),
                            )
                            .when_some(current.clone(), |el, track| {
                                el.child(self.favorite_button(track, cx).occlude())
                            }),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .pr_2()
                    .occlude()
                    .when(wide, |el| el.child(self.loop_button(cx)))
                    .child(self.volume_control(if wide { 100. } else { 70. }, cx))
                    .when(wide, |el| {
                        el.child(self.sound_button(cx))
                            .child(self.speakers_button(cx))
                    })
                    .child(self.lyrics_button(cx))
                    .child(self.mini_button(cx))
                    .child(self.queue_button(cx)),
            )
    }

    /// With the player in the title bar: the search field and command palette button along
    /// the bottom of the window.
    pub(super) fn search_strip(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let width = f32::from(super::widgets::content_size(window).width);
        div()
            .h(px(48.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .bg(p.back)
            .child(self.search_group(
                (width - 200.).clamp(140., 520.),
                width < 1100.,
                true,
                true,
                window,
                cx,
            ))
    }

    pub(super) fn toast(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        let p = pal(cx);
        Some(
            div()
                .absolute()
                // Above the player bar (or the search field, with the player on top); in the
                // big player, at the top, clear of its controls.
                .when(self.big, |el| el.top(px(104.)))
                .when(!self.big, |el| {
                    el.bottom(px(if !self.settings.layout.player_on_top {
                        100.
                    } else if self.settings.layout.search_hidden {
                        24.
                    } else {
                        68.
                    }))
                })
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("toast")
                        .occlude()
                        .max_w(px(560.))
                        .pl_4()
                        .pr_2()
                        .py_2()
                        .map(|el| super::glass::pane(el, 10., 18., cx.theme().popover, cx))
                        .border_1()
                        .border_color(if toast.error {
                            p.danger.opacity(0.5)
                        } else {
                            p.line
                        })
                        .shadow_lg()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            glyph(if toast.error { "alert" } else { "info" })
                                .size(px(16.))
                                .text_color(if toast.error { p.danger } else { p.accent }),
                        )
                        .child(
                            div().flex_1().min_w_0().child(
                                // Wrapped text needs its own width or it is measured as one line.
                                div()
                                    .w_full()
                                    .text_size(px(13.))
                                    .line_height(relative(1.4))
                                    .child(toast.text.clone()),
                            ),
                        )
                        .child(
                            icon_button("dismiss-toast", "close", "Dismiss")
                                .xsmall()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toast = None;
                                    cx.notify();
                                })),
                        )
                        .with_animation(
                            ElementId::Name(format!("toast-{:?}", toast.shown).into()),
                            Animation::new(std::time::Duration::from_millis(180))
                                .with_easing(ease_out_quint()),
                            |el, t| el.opacity(t).mt(px(8. * (1. - t))),
                        ),
                ),
        )
    }
}

/// The sidebar folded to a strip of icons, as in Apple Music.
pub(super) const RAIL: f32 = 60.;

/// macOS: the room its window buttons take at the title bar's left (they end near 70 px; see
/// `traffic_light_position`), with a clear gap after them.
const MAC_BUTTONS: f32 = 92.;

/// One icon of the folded sidebar: its name shows as a tip, and the page shown is marked by
/// a soft square and a short accent bar at the strip's edge.
fn rail_item(
    id: impl Into<ElementId>,
    glyph_name: &'static str,
    picture: Option<AnyElement>,
    name: SharedString,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let p = pal(cx);
    let glass = p.back.a < 1.;
    div()
        .id(id)
        .relative()
        .flex_shrink_0()
        .size(px(40.))
        .mx_auto()
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(active, |el| {
            el.bg(p.accent_soft).child(
                div()
                    .absolute()
                    .left(px(-(RAIL - 40.) / 2.))
                    .top(px(11.))
                    .w(px(3.))
                    .h(px(18.))
                    .rounded(px(2.))
                    .bg(p.accent),
            )
        })
        .when(!active, |el| el.hover(|s| s.bg(p.raised.opacity(0.6))))
        .child(picture.unwrap_or_else(|| {
            glyph(glyph_name)
                .size(px(18.))
                .text_color(if active {
                    p.accent
                } else if glass {
                    p.ink
                } else {
                    p.ink_2
                })
                .into_any_element()
        }))
        .tooltip(move |window, cx| {
            gpui_component::tooltip::Tooltip::new(super::widgets::keys(&name)).build(window, cx)
        })
}
