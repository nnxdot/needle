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

impl AppView {
    pub(super) fn title_bar(
        &self,
        sidebar: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let rule = self
            .explanation
            .as_deref()
            .is_some_and(|e| e.starts_with("Matches rule"))
            && !self.search_text(cx).is_empty();
        TitleBar::new()
            .h(px(48.))
            .pl_0()
            .bg(p.chrome)
            .border_b_1()
            .border_color(p.line_soft)
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
            .child(
                div()
                    .w(px(sidebar))
                    .flex_shrink_0()
                    .h_full()
                    .pl_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(glyph("logo").size(px(20.)).text_color(p.accent))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Needle"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        icon_button("back", "chevron-left", "Back · Alt+Left")
                            .small()
                            .disabled(self.back.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx))),
                    )
                    .child(
                        self.suggestion_keys(div().id("search-field"), cx)
                            .relative()
                            .w(px(520.))
                            .max_w_full()
                            .flex_shrink()
                            .children(self.suggestion_list(px(520.), cx))
                            .child(
                                Input::new(&self.search)
                                    .small()
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
                    .child(
                        div()
                            .id("open-palette")
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
                            .child(glyph("command").size(px(14.)).text_color(p.ink_3))
                            .child("Ctrl K")
                            .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("Command palette: go anywhere, do anything").build(window, cx))
                            .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx))),
                    )
                    .child(div().flex_1()),
            )
    }

    fn nav_item(
        &self,
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        glyph_name: &'static str,
        page: Page,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = pal(cx);
        let active = self.page == page
            || matches!(
                (&self.page, &page),
                (Page::Album { .. }, Page::Albums) | (Page::Artist(_), Page::Artists)
            );
        div()
            .id(id)
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
                el.bg(p.raised)
                    .text_color(p.ink)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!active, |el| {
                el.text_color(p.ink_2)
                    .hover(|s| s.bg(p.raised.opacity(0.6)).text_color(p.ink))
            })
            .child(glyph(glyph_name).size(px(17.)).text_color(if active {
                p.accent
            } else {
                p.ink_3
            }))
            .child(div().flex_1().min_w_0().truncate().child(name.into()))
            .on_click(
                cx.listener(move |this, _, window, cx| this.navigate(page.clone(), window, cx)),
            )
    }

    fn section(&self, label: &'static str, cx: &App) -> Div {
        div()
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

    pub(super) fn sidebar(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(p.chrome)
            .border_r_1()
            .border_color(p.line_soft)
            .pt_2()
            .child(self.nav_item("nav-songs", "Songs", "songs", Page::Songs, cx))
            .child(self.nav_item("nav-albums", "Albums", "albums", Page::Albums, cx))
            .child(self.nav_item("nav-artists", "Artists", "artists", Page::Artists, cx))
            .child(self.section("Collections", cx))
            .child(
                self.nav_item("nav-favorites", "Favorites", "heart", Page::Favorites, cx)
                    .drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| s.bg(p.accent_soft))
                    .on_drop(cx.listener(|this, dragged: &super::flow::DraggedTracks, _, cx| {
                        this.set_rating(&dragged.ids, 5);
                        this.notify(if dragged.ids.len() == 1 { "Added to favorites.".to_string() } else { format!("Added {} songs to favorites.", dragged.ids.len()) });
                        cx.notify();
                    })),
            )
            .child(self.nav_item("nav-recent", "Recently added", "recent", Page::Recent, cx))
            .child(self.nav_item(
                "nav-history",
                "Listening history",
                "history",
                Page::History,
                cx,
            ))
            .child(
                self.section("Playlists", cx)
                    .justify_between()
                    .pr_3()
                    .child(
                        icon_button("new-playlist", "plus", "New playlist from this view")
                            .xsmall()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if !this.page.is_tracks() {
                                    this.navigate(Page::Songs, window, cx);
                                }
                                this.show_save = true;
                                this.playlist_name.update(cx, |s, cx| {
                                    s.set_value("", window, cx);
                                    s.focus(window, cx);
                                });
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("playlists-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_2()
                    .when(self.playlists.is_empty(), |el| {
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
                        self.nav_item(
                            SharedString::from(format!("playlist-{}", playlist.id)),
                            playlist.name.clone(),
                            glyph_name,
                            Page::Playlist(playlist.id.clone()),
                            cx,
                        )
                        .when(playlist.query.is_none(), |el| {
                            let id = playlist.id.clone();
                            el.drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| s.bg(p.accent_soft))
                                .on_drop(cx.listener(move |this, dragged: &super::flow::DraggedTracks, _, cx| {
                                    let tracks = this.library.tracks_by_ids(&dragged.ids).unwrap_or_default();
                                    this.add_to_playlist(&id, tracks);
                                    cx.notify();
                                }))
                        })
                    })),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(p.line_soft)
                    .py_2()
                    .when_some(self.scan.as_ref(), |el, scan| {
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
                    .child(
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
                            .on_click(cx.listener(|this, _, _, cx| this.import_folder(cx))),
                    )
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

    fn signal_path(&self, cx: &App) -> (String, String, bool) {
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
                format!(
                    "{source} → {rate}{}",
                    if resampled { " (resampled)" } else { "" }
                ),
                format!(
                    "{source} → {} → Windows mixer at {rate} → {}",
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

    pub(super) fn player_bar(&self, width: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let current = self.playback.current.as_ref().map(|i| i.track.clone());
        let playing = self.playback.playing;
        let wide = width > px(1180.);
        let (path, path_detail, exclusive) = self.signal_path(cx);
        let repeat = self.playback.repeat;
        let looping = self.playback.loop_range.is_some();
        let volume_glyph = if self.playback.volume <= 0.001 {
            "volume-off"
        } else if self.playback.volume < 0.5 {
            "volume-low"
        } else {
            "volume"
        };
        let queue_open = self.settings.show_inspector && self.panel == Panel::Queue;
        div()
            .h(px(84.))
            .flex_shrink_0()
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.line_soft)
            .px_4()
            .flex()
            .items_center()
            .gap_4()
            // Now playing
            .child(
                div()
                    .w(px(if wide { 320. } else { 250. }))
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
                            .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("Open the big player · Ctrl+P").build(window, cx))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.big = true;
                                cx.notify();
                            })),
                    )
                    .child(match &current {
                        None => div()
                            .flex_1()
                            .child(div().text_size(px(13.5)).text_color(p.ink_2).child("Nothing playing"))
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
                                        .on_click(cx.listener(move |this, _, window, cx| this.navigate(album.clone(), window, cx))),
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
                                        .on_click(cx.listener(move |this, _, window, cx| this.navigate(artist.clone(), window, cx))),
                                )
                        }
                    })
                    .when_some(current.clone(), |el, track| {
                        let favorite = self
                            .tracks
                            .iter()
                            .find(|t| t.id == track.id)
                            .map_or(track.rating, |t| t.rating)
                            >= 4;
                        el.child(
                            icon_button("now-favorite", if favorite { "heart-fill" } else { "heart" }, if favorite { "Remove from favorites" } else { "Add to favorites" })
                                .small()
                                .when(favorite, |b| b.text_color(p.accent))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_rating(std::slice::from_ref(&track.id), if favorite { 0 } else { 5 });
                                    if let Some(item) = this.playback.current.as_mut() {
                                        item.track.rating = if favorite { 0 } else { 5 };
                                    }
                                    cx.notify();
                                })),
                        )
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
                    .child(
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
                                    .size(px(38.))
                                    .rounded_full()
                                    .bg(p.ink)
                                    .text_color(p.canvas)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.88))
                                    .active(|s| s.opacity(0.75))
                                    .child(glyph(if playing { "pause" } else { "play" }).size(px(17.)).text_color(p.canvas))
                                    .tooltip(move |window, cx| {
                                        gpui_component::tooltip::Tooltip::new(if playing { "Pause · Space" } else { "Play · Space" }).build(window, cx)
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
                                    if repeat == Repeat::One { "repeat-one" } else { "repeat" },
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
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(620.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(faint(format_duration(self.playback.position), cx).w(px(40.)).text_right())
                            .child(Slider::new(&self.seek).flex_1().disabled(current.is_none()))
                            .child(faint(current.as_ref().map(|t| format_duration(t.duration)).unwrap_or_else(|| "0:00".into()), cx).w(px(40.))),
                    ),
            )
            // Output and volume
            .child(
                div()
                    .w(px(if wide { 320. } else { 250. }))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .when(!path.is_empty() && wide, |el| {
                        el.child(
                            div()
                                .id("signal-path")
                                .mr_2()
                                .px_2()
                                .py(px(3.))
                                .rounded(px(5.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .max_w(px(150.))
                                .text_size(px(11.))
                                .text_color(if exclusive { p.accent } else { p.ink_2 })
                                .bg(if exclusive { p.accent_soft } else { p.raised })
                                .cursor_pointer()
                                .child(glyph("signal").size(px(13.)).text_color(if exclusive { p.accent } else { p.ink_2 }))
                                .child(div().truncate().child(path.clone()))
                                .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(path_detail.clone()).build(window, cx))
                                .on_click(cx.listener(|this, _, window, cx| this.navigate(Page::Settings, window, cx))),
                        )
                    })
                    .child(
                        Button::new("ab-loop")
                            .ghost()
                            .small()
                            .icon(icon("loop"))
                            .when(looping || self.loop_start.is_some(), |b| b.text_color(p.accent))
                            .disabled(current.is_none())
                            .tooltip(if looping {
                                "A–B loop on · click to clear"
                            } else if self.loop_start.is_some() {
                                "Point A set · click again to set B and start looping"
                            } else {
                                "A–B loop · click to set point A"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.playback.loop_range.is_some() {
                                    this.loop_start = None;
                                    this.player.send(Command::Loop(None));
                                } else if let Some(a) = this.loop_start.take() {
                                    let b = this.playback.position;
                                    if b > a + 0.5 {
                                        this.player.send(Command::Loop(Some((a, b))));
                                        this.notify(format!("Looping {} – {}", format_duration(a), format_duration(b)));
                                    } else {
                                        this.fail("Point B must come after point A.");
                                    }
                                } else {
                                    this.loop_start = Some(this.playback.position);
                                    this.notify(format!("Point A set at {}. Click the loop button again at point B.", format_duration(this.playback.position)));
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        icon_button("mute", volume_glyph, if self.playback.exclusive { "Exclusive output: use your device's volume" } else { "Mute" })
                            .small()
                            .disabled(self.playback.exclusive)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let restore = this.muted_volume.take();
                                let volume = match restore {
                                    Some(v) => v,
                                    None => {
                                        this.muted_volume = Some(this.playback.volume.max(0.05));
                                        0.
                                    }
                                };
                                this.player.send(Command::Volume(volume));
                                this.volume.update(cx, |s, cx| s.set_value(volume, window, cx));
                            })),
                    )
                    .child(Slider::new(&self.volume).w(px(if wide { 96. } else { 72. })).disabled(self.playback.exclusive))
                    .child(
                        icon_button("open-sound", "eq", "Equalizer and sound tools")
                            .small()
                            .when(self.settings.dsp.eq || !self.settings.dsp.is_transparent(), |b| b.text_color(p.accent))
                            .on_click(cx.listener(|this, _, window, cx| this.navigate(Page::Sound, window, cx))),
                    )
                    .child(
                        icon_button("open-mini", "mini", "Mini player · Ctrl+M")
                            .small()
                            .ml_1()
                            .on_click(cx.listener(|this, _, window, cx| this.open_mini(window, cx))),
                    )
                    .child(
                        div()
                            .id("queue-drop")
                            .rounded(px(6.))
                            .drag_over::<super::flow::DraggedTracks>(move |s, _, _, _| s.bg(p.accent_soft))
                            .on_drop(cx.listener(|this, dragged: &super::flow::DraggedTracks, _, _| {
                                let tracks = this.library.tracks_by_ids(&dragged.ids).unwrap_or_default();
                                this.enqueue(tracks);
                            }))
                            .child(Button::new("queue-toggle")
                            .ghost()
                            .small()
                            .ml_1()
                            .icon(icon("queue"))
                            .selected(queue_open)
                            .tooltip("Queue · Ctrl+J")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if queue_open {
                                    this.settings.show_inspector = false;
                                } else {
                                    this.settings.show_inspector = true;
                                    this.panel = Panel::Queue;
                                }
                                this.persist_settings();
                                cx.notify();
                            }))),
                    ),
            )
    }

    pub(super) fn toast(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        let p = pal(cx);
        Some(
            div()
                .absolute()
                .bottom(px(100.))
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
                        .rounded(px(10.))
                        .bg(cx.theme().popover)
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
                            div()
                                .flex_1()
                                .text_size(px(13.))
                                .line_height(relative(1.4))
                                .child(toast.text.clone()),
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
