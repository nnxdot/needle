//! An artist's page: their picture wide across the top, fading into the page; their logo (or
//! name) near its foot over Shuffle, a large Play, Radio, and Blend; then their newest album
//! beside their top songs, and their albums.
use super::{
    AppView, Event, pal,
    widgets::{cover, display, glyph, heading, icon, icon_button, meta},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use needle_core::media::pictures::{self, ArtistPicture};
use needle_core::{
    media,
    model::{Track, format_duration},
};
use std::path::{Path, PathBuf};

/// What the artist page shows besides their albums, looked up when it opens.
#[derive(Default)]
pub(super) struct ArtistView {
    pub name: String,
    pub top: Vec<Track>,
    /// A wide picture of them from fanart.tv.
    pub banner: Option<String>,
    pub logo: Option<Logo>,
    /// The pictures editor is open (the pencil on the banner).
    pub editing: bool,
    /// fanart.tv's pictures of one kind, to choose from: (picture, small copy).
    pub choices: Option<(ArtistPicture, Vec<(String, String)>)>,
}

/// An artist's logo from fanart.tv, its clear edges cut off: as it comes (mostly white), and in
/// dark ink for light themes; with its size, so the page draws it whole in the room it has.
#[derive(Clone)]
pub(super) struct Logo {
    pub white: String,
    pub ink: String,
    pub size: (u32, u32),
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        super::widgets::count(n),
        if n == 1 { one } else { many }
    )
}

/// Songs without an artist: nothing is looked up for them.
fn placeholder(name: &str) -> bool {
    name.trim().is_empty() || name == "Unknown artist"
}

/// The logo with its clear edges cut off (`<logo>.white.png`), and the same in dark ink
/// (`<logo>.ink.png`): fanart.tv's logos are mostly white and do not show on a light page.
/// Made again when fanart.tv's logo is newer.
fn prepare_logo(logo: &Path) -> Option<Logo> {
    let white = PathBuf::from(format!("{}.white.png", logo.display()));
    let ink = PathBuf::from(format!("{}.ink.png", logo.display()));
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let fresh = |p: &Path| modified(p).is_some() && modified(p) >= modified(logo);
    let done = |size| Logo {
        white: white.to_string_lossy().into(),
        ink: ink.to_string_lossy().into(),
        size,
    };
    if fresh(&white) && fresh(&ink) {
        return image::image_dimensions(&white).ok().map(done);
    }
    let bytes = std::fs::read(logo).ok()?;
    let picture = image::load_from_memory(&bytes).ok()?.to_rgba8();
    // The smallest box around what can be seen.
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, pixel) in picture.enumerate_pixels() {
        if pixel.0[3] > 8 {
            (left, top) = (left.min(x), top.min(y));
            (right, bottom) = (right.max(x), bottom.max(y));
        }
    }
    if left > right {
        return None;
    }
    let mut trimmed =
        image::imageops::crop_imm(&picture, left, top, right - left + 1, bottom - top + 1)
            .to_image();
    trimmed
        .save_with_format(&white, image::ImageFormat::Png)
        .ok()?;
    for pixel in trimmed.pixels_mut() {
        pixel.0[..3].copy_from_slice(&[28, 28, 30]);
    }
    trimmed
        .save_with_format(&ink, image::ImageFormat::Png)
        .ok()?;
    Some(done(trimmed.dimensions()))
}

impl AppView {
    /// Look up what the artist page shows: their top songs at once, their picture and logo
    /// from fanart.tv (when online lookups are on) as they come.
    pub(super) fn load_artist_view(&mut self, name: &str) {
        self.artist_view = ArtistView {
            name: name.to_string(),
            ..Default::default()
        };
        self.artist_scroll.set_offset(point(px(0.), px(0.)));
        let library = self.library.clone();
        let sender = self.sender.clone();
        let who = name.to_string();
        std::thread::spawn(move || {
            let top = library
                .top_tracks(&who, 12)
                .unwrap_or_default()
                .into_iter()
                .filter(|t| !t.missing)
                .take(8)
                .collect();
            let _ = sender.send(Event::ArtistTop(who, top));
        });
        if placeholder(name) {
            return;
        }
        let online = self.settings.online_media;
        let (library, sender, who) = (self.library.clone(), self.sender.clone(), name.to_string());
        std::thread::spawn(move || {
            let banner = media::artist_banner(&library, &who, online)
                .inspect_err(|e| {
                    needle_core::logfile::warn(format!("Artist picture for {who}: {e:#}"))
                })
                .ok()
                .flatten();
            let _ = sender.send(Event::ArtistBanner(
                who,
                banner.map(|p| p.to_string_lossy().into()),
            ));
        });
        let (library, sender, who) = (self.library.clone(), self.sender.clone(), name.to_string());
        std::thread::spawn(move || {
            let logo = media::artist_logo(&library, &who, online)
                .inspect_err(|e| {
                    needle_core::logfile::warn(format!("Artist logo for {who}: {e:#}"))
                })
                .ok()
                .flatten()
                .and_then(|path| prepare_logo(&path));
            let _ = sender.send(Event::ArtistLogo(who, logo));
        });
    }

    pub(super) fn artist_page(
        &self,
        name: &str,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let height = f32::from(window.viewport_size().height);
        let view = Some(&self.artist_view).filter(|v| v.name == name);
        let narrow = width < 780.;
        div()
            .id(SharedString::from(format!("artist-page-{name}")))
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.artist_scroll)
            .pb(px(40.) + self.foot_room())
            .child(self.artist_banner(name, view, width, height, cx))
            .when(view.is_some_and(|v| v.editing), |el| {
                el.child(self.pictures_editor(name, cx))
            })
            .child(
                div()
                    .px_6()
                    .pt_6()
                    .flex()
                    .flex_col()
                    .gap(px(32.))
                    .child(
                        div()
                            .flex()
                            .when(narrow, |el| el.flex_col().gap(px(28.)))
                            .when(!narrow, |el| el.items_start().gap(px(40.)))
                            .children(self.latest_release(cx))
                            .children(
                                view.and_then(|v| self.top_songs(name, &v.top, width, narrow, cx)),
                            ),
                    )
                    .when(!self.groups.is_empty(), |el| {
                        el.child(heading("Albums"))
                            .child(self.artist_albums(width, cx))
                    }),
            )
            .into_any_element()
    }

    /// Their picture across the top, fading into the page, with their logo or name and the
    /// buttons at its foot.
    fn artist_banner(
        &self,
        name: &str,
        view: Option<&ArtistView>,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = pal(cx);
        let tall = (height * 0.55).clamp(340., 620.);
        // A wide picture of them, or their photo.
        let picture = view
            .and_then(|v| v.banner.clone())
            .or_else(|| self.artist_images.get(name).cloned().flatten());
        let logo = view.and_then(|v| v.logo.clone());
        let albums = self.groups.len();
        let tracks: usize = self.groups.iter().map(|g| g.tracks).sum();
        let can_play = !self.groups.is_empty();
        let summary = [
            (albums > 0).then(|| plural(albums, "album", "albums")),
            (tracks > 0).then(|| plural(tracks, "track", "tracks")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let (radio, blend) = (name.to_string(), name.to_string());
        let editing = view.is_some_and(|v| v.editing);
        div()
            .relative()
            .w_full()
            .h(px(tall))
            .overflow_hidden()
            .when_some(picture, |el, path| {
                el.child(
                    img(PathBuf::from(path))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
            })
            // Into the page, so the name and buttons sit on its own colour.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(relative(0.62))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(p.canvas.opacity(0.), 0.),
                        linear_color_stop(p.canvas, 1.),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .px_6()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(match logo {
                        // Drawn at its own shape, as large as fits: wide logos to the width,
                        // tall ones to the height.
                        Some(logo) => {
                            let (w, h) = (logo.size.0.max(1) as f32, logo.size.1.max(1) as f32);
                            let scale = ((width - 64.).clamp(200., 440.) / w).min(130. / h);
                            img(PathBuf::from(if p.dark { logo.white } else { logo.ink }))
                                .w(px(w * scale))
                                .h(px(h * scale))
                                .flex_shrink_0()
                                .into_any_element()
                        }
                        None => display(
                            if name.is_empty() {
                                "Unknown artist"
                            } else {
                                name
                            }
                            .to_string(),
                            if width < 760. { 44. } else { 60. },
                        )
                        .text_center()
                        .line_clamp(2)
                        .into_any_element(),
                    })
                    .when(!summary.is_empty(), |el| el.child(meta(summary, cx)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("artist-shuffle")
                                    .icon(icon("shuffle"))
                                    .label("Shuffle")
                                    .disabled(!can_play)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.play_view(0, true, cx)),
                                    ),
                            )
                            .child(
                                div()
                                    .id("artist-play")
                                    .size(px(60.))
                                    .rounded_full()
                                    .bg(p.accent)
                                    .shadow_lg()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(can_play, |el| {
                                        el.cursor_pointer()
                                            .hover(|s| s.bg(p.accent.opacity(0.88)))
                                            .active(|s| s.bg(p.accent.opacity(0.7)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.play_view(0, false, cx)
                                            }))
                                    })
                                    .when(!can_play, |el| el.opacity(0.5))
                                    .tooltip(|window, cx| {
                                        gpui_component::tooltip::Tooltip::new("Play")
                                            .build(window, cx)
                                    })
                                    .child(
                                        glyph("play")
                                            .size(px(24.))
                                            .ml(px(2.))
                                            .text_color(p.accent_ink),
                                    ),
                            )
                            .child(
                                Button::new("artist-radio")
                                    .icon(icon("radio"))
                                    .label("Radio")
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.start_artist_radio(radio.clone())
                                    })),
                            )
                            .child(
                                icon_button("artist-blend", "blend", "Blend with another artist")
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            // Below the button, clear of its tooltip.
                                            let position =
                                                window.mouse_position() + point(px(0.), px(22.));
                                            this.open_blend_menu(blend.clone(), position, cx)
                                        },
                                    )),
                            )
                            // Their photo, wide picture, and logo: the user's own, or another
                            // of fanart.tv's.
                            .when(!placeholder(name), |el| {
                                el.child(
                                    Button::new("artist-pictures")
                                        .icon(icon("edit"))
                                        .label(if editing { "Done" } else { "Pictures" })
                                        .selected(editing)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.artist_view.editing = !this.artist_view.editing;
                                            this.artist_view.choices = None;
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
    }

    /// Their newest album (of those with a year): its cover, then when, what, and how many songs.
    fn latest_release(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let p = pal(cx);
        let album = self
            .groups
            .iter()
            .filter(|g| g.year > 0)
            .max_by(|a, b| (a.year, &a.title).cmp(&(b.year, &b.title)))?;
        let (open, play) = (album.page.clone(), album.page.clone());
        let title = album.title.clone();
        Some(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_3()
                .child(heading("Latest release"))
                .child(
                    div()
                        .id("latest-release")
                        .group("latest")
                        .flex()
                        .items_center()
                        .gap_4()
                        .cursor_pointer()
                        .child(
                            div()
                                .rounded(px(8.))
                                .shadow_lg()
                                .group_hover("latest", |s| s.opacity(0.9))
                                .child(cover(album.artwork.as_deref(), &album.seed, 168., cx)),
                        )
                        .child(
                            div()
                                .w(px(210.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    meta(album.year.to_string(), cx)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.ink_2),
                                )
                                .child(display(album.title.clone(), 20.).line_clamp(3))
                                .child(meta(plural(album.tracks, "song", "songs"), cx))
                                .child(
                                    div().pt_2().flex().child(
                                        Button::new("latest-play")
                                            .small()
                                            .primary()
                                            .icon(icon("play"))
                                            .label("Play")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.play_rule(
                                                    play.base(),
                                                    format!("From the album {title}"),
                                                );
                                            })),
                                    ),
                                ),
                        )
                        .on_mouse_down(MouseButton::Right, {
                            let page = open.clone();
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.open_group_menu(page.clone(), event.position, cx)
                            })
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.navigate(open.clone(), window, cx)
                        })),
                ),
        )
    }

    /// Their most played songs, in one or two columns; a double click plays from there.
    fn top_songs(
        &self,
        name: &str,
        top: &[Track],
        width: f32,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if top.is_empty() {
            return None;
        }
        let p = pal(cx);
        // Beside the newest album, the room left of the page's width.
        let room = if narrow {
            width - 48.
        } else {
            width - 48. - 440.
        };
        let columns = if room >= 600. { 2 } else { 1 };
        let per_column = top.len().div_ceil(columns);
        let reason = format!("From {name}");
        let row = |index: usize, track: &Track, cx: &mut Context<Self>| {
            let (all, reason, menu) = (top.to_vec(), reason.clone(), track.clone());
            div()
                .id(("top-song", index))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(5.))
                .rounded(px(6.))
                .hover(|s| s.bg(p.raised_hover))
                .child(cover(
                    track.artwork.as_deref(),
                    &super::widgets::track_seed(track),
                    40.,
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .child(track.title.clone()),
                        )
                        .child(meta(track.album.clone(), cx).truncate()),
                )
                .child(
                    meta(format_duration(track.duration), cx)
                        .text_color(p.ink_3)
                        .flex_shrink_0(),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.open_playing_menu(menu.clone(), event.position, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                    if event.click_count() == 2 {
                        let items = all
                            .iter()
                            .cloned()
                            .map(|track| super::QueueItem {
                                track,
                                reason: reason.clone(),
                            })
                            .collect();
                        let _ = this.sender.send(Event::PlayAt(items, index, None));
                    }
                }))
        };
        let mut list = div().flex().gap_5();
        for column in top.chunks(per_column).enumerate() {
            let (c, tracks) = column;
            let mut rows = div().flex_1().min_w_0().flex().flex_col();
            for (i, track) in tracks.iter().enumerate() {
                rows = rows.child(row(c * per_column + i, track, cx));
            }
            list = list.child(rows);
        }
        Some(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(heading("Top songs"))
                .child(list),
        )
    }

    /// Their albums as tiles, in rows that fill the page's width.
    fn artist_albums(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let gap = 22.;
        let inner = (width - 48.).max(200.);
        let columns = ((inner + gap) / (172. + gap)).floor().max(1.) as usize;
        let tile = (inner - gap * (columns as f32 - 1.)) / columns as f32;
        let mut grid = div().flex().flex_col().gap(px(26.));
        for start in (0..self.groups.len()).step_by(columns) {
            let end = (start + columns).min(self.groups.len());
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(gap))
                    .children((start..end).map(|i| self.tile(i, tile, false, cx))),
            );
        }
        grid
    }
}

impl AppView {
    /// Under the banner while editing: the photo, the wide picture, and the logo, each with
    /// Choose a file…, From fanart.tv, and Reset; and fanart.tv's choices when asked.
    fn pictures_editor(&self, name: &str, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let kinds = [
            (ArtistPicture::Photo, "Photo"),
            (ArtistPicture::Banner, "Wide picture"),
            (ArtistPicture::Logo, "Logo"),
        ];
        let online =
            self.settings.online_media && needle_core::integrations::fanart_key().is_some();
        let choices = self.artist_view.choices.clone();
        div()
            .mx_6()
            .mt_4()
            .p_4()
            .rounded(px(14.))
            .bg(p.raised)
            .border_1()
            .border_color(p.line_soft)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .gap_4()
                    .children(kinds.into_iter().map(|(kind, label)| {
                        let own = pictures::own_picture(&self.library, name, kind).is_some();
                        let id = |what: &str| SharedString::from(format!("{what}-{label}"));
                        let (pick, from, reset) =
                            (name.to_string(), name.to_string(), name.to_string());
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label)
                                    .when(own, |el| {
                                        el.child(
                                            div()
                                                .text_size(px(11.5))
                                                .font_weight(FontWeight::NORMAL)
                                                .text_color(p.ink_3)
                                                .child("Your choice"),
                                        )
                                    }),
                            )
                            .child(
                                Button::new(id("choose"))
                                    .small()
                                    .label("Choose a file…")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.choose_artist_picture(pick.clone(), kind, window, cx)
                                    })),
                            )
                            .when(online, |el| {
                                el.child(
                                    Button::new(id("fanart"))
                                        .small()
                                        .label("From fanart.tv")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.load_fanart_choices(from.clone(), kind);
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(own, |el| {
                                el.child(
                                    Button::new(id("reset"))
                                        .small()
                                        .ghost()
                                        .label("Reset")
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            let _ = pictures::reset_picture(
                                                &this.library,
                                                &reset,
                                                kind,
                                            );
                                            let _ = this
                                                .sender
                                                .send(Event::ArtistPicturesChanged(reset.clone()));
                                        })),
                                )
                            })
                    })),
            )
            .when_some(choices, |el, (kind, choices)| {
                let who = name.to_string();
                el.child(if choices.is_empty() {
                    meta(
                        "fanart.tv has no pictures of this kind for this artist.",
                        cx,
                    )
                    .into_any_element()
                } else {
                    div()
                        .id("fanart-choices")
                        .overflow_x_scroll()
                        .flex()
                        .gap_3()
                        .children(choices.into_iter().enumerate().map(|(i, (url, preview))| {
                            let who = who.clone();
                            let wide = kind != ArtistPicture::Photo;
                            div()
                                .id(("fanart-choice", i))
                                .flex_shrink_0()
                                .w(px(if wide { 220. } else { 120. }))
                                .h(px(if wide { 124. } else { 120. }))
                                .rounded(px(8.))
                                .overflow_hidden()
                                .bg(if kind == ArtistPicture::Logo {
                                    p.canvas
                                } else {
                                    p.raised_hover
                                })
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .child(
                                    img(PathBuf::from(preview))
                                        .size_full()
                                        .rounded(px(8.))
                                        .object_fit(if kind == ArtistPicture::Logo {
                                            ObjectFit::Contain
                                        } else {
                                            ObjectFit::Cover
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.notify("Using that picture…");
                                    let (library, sender, who, url) = (
                                        this.library.clone(),
                                        this.sender.clone(),
                                        who.clone(),
                                        url.clone(),
                                    );
                                    std::thread::spawn(move || {
                                        let _ = sender.send(
                                            match pictures::pick_fanart(&library, &who, kind, &url)
                                            {
                                                Ok(_) => Event::ArtistPicturesChanged(who),
                                                Err(e) => Event::Error(format!("fanart.tv: {e:#}")),
                                            },
                                        );
                                    });
                                    this.artist_view.choices = None;
                                    cx.notify();
                                }))
                        }))
                        .into_any_element()
                })
            })
    }

    /// A picture file of the listener's own, as this artist's photo, wide picture, or logo.
    fn choose_artist_picture(
        &mut self,
        name: String,
        kind: ArtistPicture,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use this picture".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let _ = this.update(cx, |this, _| {
                if let Some(path) = paths.first() {
                    match pictures::set_own_picture(&this.library, &name, kind, path) {
                        Ok(_) => {
                            let _ = this.sender.send(Event::ArtistPicturesChanged(name.clone()));
                        }
                        Err(e) => this.fail(format!("{e:#}")),
                    }
                }
            });
        })
        .detach();
    }

    /// fanart.tv's pictures of one kind for the artist, with small copies to choose from.
    fn load_fanart_choices(&mut self, name: String, kind: ArtistPicture) {
        let (library, sender) = (self.library.clone(), self.sender.clone());
        let online = self.settings.online_media;
        self.notify("Asking fanart.tv…");
        std::thread::spawn(move || {
            let found = match pictures::fanart_choices(&library, &name, kind, online) {
                Ok(found) => found,
                Err(e) => {
                    let _ = sender.send(Event::Error(format!("fanart.tv: {e:#}")));
                    return;
                }
            };
            let choices = found
                .into_iter()
                .take(24)
                .filter_map(|c| {
                    let preview = pictures::fanart_preview(&library, &c.preview)
                        .or_else(|_| pictures::fanart_preview(&library, &c.url))
                        .ok()?;
                    Some((c.url, preview.to_string_lossy().to_string()))
                })
                .collect();
            let _ = sender.send(Event::ArtistChoices(name, kind, choices));
        });
    }
}
