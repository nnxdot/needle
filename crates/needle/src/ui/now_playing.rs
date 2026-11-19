use super::{
    AppView, Page,
    menus::Entry,
    motion, pal,
    player_tools::Tool,
    widgets::{artwork, faint, glyph, meta, quality},
};
use gpui::{prelude::*, *};
use needle_core::{
    audio::{Command, Repeat},
    model::{BIG_LOOKS, Track, format_duration},
};

/// "44 minutes", "1 h 5 min": how long the songs up next play.
fn minutes(seconds: f64) -> String {
    let all = (seconds / 60.).round() as u64;
    match (all / 60, all % 60) {
        (0, 1) => "1 minute".into(),
        (0, m) => format!("{m} minutes"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The immersive cover, as the Mac's and Apple Music's: the cover fills the window's left side
/// from top to foot and fades, to its right and at its foot, into a tone of its own colour.
/// `foot` is how tall the words and controls over its foot are: the tone is solid behind all of
/// them, so no cover makes them hard to read, and the cover fades in above.
/// With a pane beside it (`alone` false), the cover is the window's left side, at most `widest`
/// wide. Alone, it spans the window, and grows and shrinks with it; in a window much wider
/// than tall it stops at 1.4 times the height, in the middle, fading into the tone each side.
/// Gives the layer, how wide it is (the room for the words and controls over its foot), and
/// the tone (which the rest of the player takes).
#[allow(clippy::too_many_arguments)]
pub(super) fn immersive_cover(
    path: &str,
    moving: bool,
    look: Option<&super::ambient::Look>,
    w: f32,
    h: f32,
    widest: f32,
    foot: f32,
    dark: bool,
    alone: bool,
) -> (AnyElement, f32, Hsla) {
    let (side, cover_w) = if alone {
        (w.max(1.), w.min(h * 1.4).max(1.))
    } else {
        let side = h.min(w * 0.62).min(widest).max(1.);
        (side, side)
    };
    let left = (side - cover_w) / 2.;
    let fade = cover_w * 0.25;
    let tone = match look.map(|l| l.vivid) {
        Some(c) if dark => hsla(c.h, (c.s * 0.8).min(0.5), 0.15, 1.),
        Some(c) => hsla(c.h, (c.s * 0.7).min(0.45), 0.88, 1.),
        None if dark => hsla(0., 0., 0.1, 1.),
        None => hsla(0., 0., 0.92, 1.),
    };
    // A square cover fills its box, so it is drawn as large as the box's longer edge. A
    // moving picture is drawn as it is (a smaller copy would be a still one).
    // A moving cover as prepared to fit in memory (see `moving`); until it is, or for a
    // still cover, a copy of the picture at the size it is drawn.
    let file: ImageSource = match moving
        .then(|| super::moving::image(std::path::Path::new(path)))
        .flatten()
    {
        Some(frames) => frames.into(),
        None => super::thumbs::for_size(path, cover_w.max(h))
            .unwrap_or_else(|| path.into())
            .into(),
    };
    let edge_fade = |at: f32, into_tone: bool| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(at))
            .w(px(fade))
            .bg(linear_gradient(
                90.,
                linear_color_stop(tone.opacity(if into_tone { 0. } else { 1. }), 0.),
                linear_color_stop(tone.opacity(if into_tone { 1. } else { 0. }), 1.),
            ))
    };
    let layer = div()
        .absolute()
        .left_0()
        .top_0()
        .w(px(side))
        .h(px(h))
        .overflow_hidden()
        .bg(tone)
        .child(
            img(file)
                // GPUI steps through a moving picture's frames only for an image with an id.
                .id("immersive-cover")
                .absolute()
                .top_0()
                .left(px(left))
                .w(px(cover_w))
                .h(px(h))
                .object_fit(ObjectFit::Cover),
        )
        // Beside a pane: into the tone to its right. Alone and narrower than the window: into
        // the tone on both sides.
        .when(!alone, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(side * 0.45))
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(tone.opacity(0.), 0.),
                        linear_color_stop(tone, 1.),
                    )),
            )
        })
        .when(alone && cover_w < side, |el| {
            el.child(edge_fade(left, false))
                .child(edge_fade(left + cover_w - fade, true))
        })
        // ...and solid behind the words and controls at its foot.
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(foot.min(h)))
                .bg(tone),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(foot.min(h) - 1.))
                .h(px((h * 0.42).max(200.)))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(tone.opacity(0.), 0.),
                    linear_color_stop(tone, 1.),
                )),
        )
        // At its top, so the buttons over it read.
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .h(px(140.))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(tone.opacity(0.8), 0.),
                    linear_color_stop(tone.opacity(0.), 1.),
                )),
        )
        .into_any_element();
    (layer, side, tone)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Lyrics,
    Queue,
    Stems,
    None,
}

/// From this width the pane (Up next, the lyrics) sits beside the player; below, it is a
/// sheet over its foot.
const PANE_BESIDE: f32 = 1000.;

/// What the big player is drawn at, worked out once per frame.
struct Frame {
    w: f32,
    h: f32,
    /// The title bar's height: the player runs under it.
    top: f32,
    look: &'static str,
    side: Side,
    /// The pane sits beside the player (else a sheet over it, when open).
    beside: bool,
    /// Settings › Appearance › Immersive cover.
    immersive: bool,
    moving: Option<String>,
    angle: f32,
    blur: Option<std::path::PathBuf>,
    strength: f32,
    tone: Hsla,
    dark: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Centre,
}

/// Needle's glass on the big player: the cover's colour from the left, the edge light. Only
/// the blurred backdrop is under it, never sharp content, so it may stay clear; `solid` (the
/// pane) is darker, as text lies on it.
fn player_glass<E: Styled>(el: E, radius: f32, solid: bool, cx: &App) -> E {
    let p = pal(cx);
    let (from, to) = match (p.dark, solid) {
        (true, false) => (p.glow.opacity(0.26), hsla(0., 0., 1., 0.09)),
        (true, true) => (
            motion::mix(p.glow, gpui::black(), 0.6).opacity(0.6),
            hsla(0., 0., 0.08, 0.5),
        ),
        (false, false) => (p.glow.opacity(0.18), hsla(0., 0., 1., 0.5)),
        (false, true) => (p.glow.opacity(0.2), hsla(0., 0., 1., 0.72)),
    };
    super::glass::edge(
        el.rounded(px(radius)).bg(linear_gradient(
            90.,
            linear_color_stop(from, 0.),
            linear_color_stop(to, 0.8),
        )),
        super::glass::Lift::Mid,
        cx,
    )
}

/// A round glass button over the big player.
fn glass_round(
    id: &'static str,
    icon: &'static str,
    tip: &'static str,
    on: bool,
    cx: &App,
) -> Stateful<Div> {
    let p = pal(cx);
    div()
        .id(id)
        .size(px(36.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .map(|el| player_glass(el, 18., false, cx))
        .hover(|s| s.opacity(0.85))
        .active(|s| super::glass::press_round(s, 36.))
        .child(
            glyph(icon)
                .size(px(18.))
                .text_color(if on { p.accent } else { p.ink }),
        )
        .tooltip(move |window, cx| {
            gpui_component::tooltip::Tooltip::new(super::widgets::keys(tip)).build(window, cx)
        })
}

/// Behind all: the cover far out of focus (the blurred copy kept per cover, see `ambient`),
/// drawn larger than the window, under a veil in the cover's own colour, a little darker
/// below and at the edges. Without a cover, only the veil's colour shows (the player's own
/// fill, under it).
fn big_backdrop(f: &Frame) -> Vec<AnyElement> {
    let mut layers = vec![];
    if let Some(blur) = f.blur.clone() {
        layers.push(
            img(blur)
                .absolute()
                .top(px(-f.h * 0.15))
                .left(px(-f.w * 0.15))
                .w(px(f.w * 1.3))
                .h(px(f.h * 1.3))
                .object_fit(ObjectFit::Cover)
                .opacity(f.strength * if f.dark { 0.95 } else { 0.7 })
                .into_any_element(),
        );
    }
    layers.push(
        div()
            .absolute()
            .inset_0()
            .bg(linear_gradient(
                180.,
                linear_color_stop(f.tone.opacity(0.4), 0.),
                linear_color_stop(f.tone.opacity(0.7), 1.),
            ))
            .into_any_element(),
    );
    let edge = gpui::black().opacity(if f.dark { 0.3 } else { 0.07 });
    for angle in [0., 90., 180., 270.] {
        layers.push(
            div()
                .absolute()
                .inset_0()
                .bg(linear_gradient(
                    angle,
                    linear_color_stop(edge, 0.),
                    linear_color_stop(edge.opacity(0.), 0.3),
                ))
                .into_any_element(),
        );
    }
    layers
}

impl AppView {
    /// Grow the big player out of the cover in the bottom-left corner, or shrink it back.
    /// The player keeps its full size inside a growing clip, so nothing re-flows mid-way.
    /// The big player growing from the player bar's cover (or shrinking back), in a space `w`
    /// by `h`. The player itself is `lift` taller and starts that far above the space (under the
    /// title bar), so it is drawn the same as once it is open, when it runs under the title bar.
    pub(super) fn big_reveal(
        &self,
        player: AnyElement,
        w: f32,
        h: f32,
        lift: f32,
        cx: &App,
    ) -> AnyElement {
        let canvas = pal(cx).canvas;
        // Open and still: drawn in its own place, without the opening again (the move from the
        // layer it grew in started the animation a second time).
        if !self.big_moving {
            return div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .bg(canvas)
                .child(
                    div()
                        .absolute()
                        .w(px(w))
                        .h(px(h + lift))
                        .top(px(-lift))
                        .child(player),
                )
                .into_any_element();
        }
        // Where the cover that opened it sits, measured from the body's edges (as last
        // drawn); the player bar's usual place if it was not drawn yet.
        let art = self
            .big_art_edges(w, h)
            .unwrap_or([22., h - 77., w - 78., 21.]);
        let shown = self.big_shown_toward(self.big);
        let edge = move |i: usize| art[i] * (1. - shown);
        let inner = div()
            .absolute()
            .w(px(w))
            .h(px(h + lift))
            .left(px(-edge(0)))
            .top(px(-edge(1) - lift))
            .child(player);
        div()
            .absolute()
            .overflow_hidden()
            .bg(canvas)
            .left(px(edge(0)))
            .top(px(edge(1)))
            .right(px(edge(2)))
            .bottom(px(edge(3)))
            .rounded(px(8. * (1. - shown)))
            .opacity((shown * 1.6).min(1.))
            .child(inner)
            .into_any_element()
    }

    /// How far open the big player is (0 to 1) on its way to `open` (the side it moves to).
    pub(super) fn big_shown_toward(&self, open: bool) -> f32 {
        let target = if open { 1. } else { 0. };
        if !self.big_moving {
            return target;
        }
        let (from, since, ms) = self.big_from;
        let t = if ms > 0. {
            (since.elapsed().as_secs_f32() * 1000. / ms).min(1.)
        } else {
            1.
        };
        from + (target - from) * motion::ease_out(t)
    }

    /// Notes where the cover that opens the big player is drawn (put inside that cover).
    pub(super) fn big_art_probe(&self) -> impl IntoElement {
        let cell = self.big_art.clone();
        canvas(move |bounds, _, _| cell.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .size_full()
    }

    /// The cover's place as distances from the body's left, top, right and bottom edges, in a
    /// body `w` by `h`.
    fn big_art_edges(&self, w: f32, h: f32) -> Option<[f32; 4]> {
        let (art, body) = (self.big_art.get()?, self.body_bounds.get()?);
        let left = f32::from(art.left() - body.left());
        let top = f32::from(art.top() - body.top());
        let right = w - f32::from(art.right() - body.left());
        let bottom = h - f32::from(art.bottom() - body.top());
        Some([left, top, right, bottom])
    }

    /// The big player (Now Playing, 2026-11-18): the cover far out of focus behind all, under a
    /// veil in its own colour. With Immersive cover on, the cover at the left and the song
    /// large at its right; off, the cover as a plain card in the middle. Up next and the
    /// lyrics share one glass pane at the right (a sheet over the foot where the window is
    /// too narrow for it). The window is never narrower than 900, so the phones' stacked
    /// order has no place here.
    pub(super) fn big_player(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let look = self.now_look();
        let tint = look.as_ref().map(|l| l.vivid);
        let size = super::widgets::content_size(window);
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let big_look = self.big_look();
        // The Lyrics look shows the words itself: no lyrics beside them too.
        let side = match self.big_side {
            Side::Lyrics if big_look == "lyrics" => Side::None,
            side => side,
        };
        let angle = self.record_angle();
        // The record turns while the music plays; the Lyrics look lights its words as they
        // are sung. Neither needs frames otherwise: the line changes come with playback.
        if big_look == "record" && self.playback.playing && self.motion_allowed()
            || big_look == "lyrics" && self.words_lighting()
        {
            window.request_animation_frame();
        }
        let current = self.shown_item();
        let track = current.as_ref().map(|c| c.track.clone());
        let moving = track.as_ref().and_then(|t| self.moving_cover(t));
        let tone = match tint {
            Some(c) if p.dark => hsla(c.h, (c.s * 0.8).min(0.45), 0.2, 1.),
            Some(c) => hsla(c.h, (c.s * 0.7).min(0.4), 0.86, 1.),
            None => p.raised,
        };
        // Fade the background to the new cover's colour instead of snapping.
        if self.big_tint.2 == 0 {
            self.big_tint = (tone, tone, 1);
        } else if self.big_tint.1 != tone {
            self.big_tint = (self.big_tint.1, tone, self.big_tint.2 + 1);
        }
        let (from, to, tint_serial) = self.big_tint;
        // Ambient: the cover at full strength.
        let strength = if self.ambient_look() {
            1.
        } else {
            look.as_ref().map_or(1., |l| l.strength(p.dark))
        };
        let blur = look.as_ref().and_then(|l| l.blur.clone());
        let frame = Frame {
            w,
            h,
            top: self.title_height(window),
            look: big_look,
            side,
            beside: side != Side::None && w >= PANE_BESIDE,
            immersive: self.settings.immersive_cover,
            moving,
            angle,
            blur,
            strength,
            tone,
            dark: p.dark,
        };
        let parts = self.big_wide(&frame, track.as_ref(), cx);
        let player = div()
            .id("big-player")
            .size_full()
            .relative()
            .overflow_hidden()
            .children(big_backdrop(&frame))
            .children(parts)
            // The pane as a sheet where there is no room beside the player.
            .when(
                side != Side::None && !frame.beside && self.big_sheet,
                |el| el.children(self.big_sheet(&frame, cx)),
            );
        motion::animate(player, ("big-tint", tint_serial), 900, cx, move |el, t| {
            el.bg(motion::mix(from, to, t))
        })
    }

    /// Immersive (the cover at the left, the song at its right) or the card (the cover in the
    /// middle); the speaker and the volume at the bottom right; the pane.
    fn big_wide(
        &mut self,
        f: &Frame,
        track: Option<&Track>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let pane_w = if f.beside {
            (f.w * 0.27).clamp(300., 380.)
        } else {
            0.
        };
        // The room left of the pane.
        let room = f.w - if f.beside { pane_w + 20. } else { 0. };
        let foot = 24.;
        let free = f.h - f.top - foot;
        let mut out = vec![];
        if f.immersive {
            let cover = (free - 60.).min(room * 0.4).clamp(200., 560.);
            let stage = if f.look == "lyrics" {
                cover * 1.3
            } else {
                cover
            };
            let column = (room * 0.34).clamp(280., 440.);
            let gap = (cover * 0.14).clamp(32., 64.);
            let left = ((room - stage - gap - column) / 2.).max(24.);
            out.push(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(f.top))
                    .bottom(px(foot))
                    .w(px(stage))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.big_stage(f, track, cover, stage, cx))
                    .into_any_element(),
            );
            out.push(
                div()
                    .absolute()
                    .left(px(left + stage + gap))
                    .top(px(f.top))
                    .bottom(px(foot + 56.))
                    .w(px(column))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(self.big_song(track, Align::Left, (column * 0.13).clamp(30., 50.), cx))
                    .when(f.look != "lyrics", |el| {
                        el.children(self.big_line(Align::Left, 16., cx))
                    })
                    .child(div().h(px(14.)))
                    .child(self.big_seek("big-seek", track.is_none(), 46., cx))
                    .child(div().h(px(10.)))
                    .child(self.big_buttons(column + 16., 72., cx).ml(px(-8.)))
                    .into_any_element(),
            );
        } else {
            let cover = (f.h * 0.47)
                .min(free - 260.)
                .min(room * 0.5)
                .clamp(160., 460.);
            let seek = (room * 0.36).clamp(300., 440.);
            out.push(
                div()
                    .absolute()
                    .left_0()
                    .w(px(room))
                    .top(px(f.top))
                    .bottom(px(foot + 40.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(self.big_stage(f, track, cover, (room - 160.).min(cover * 1.6), cx))
                    .when(self.visualizer && f.look != "visualizer", |el| {
                        el.child(div().pt_4().child(self.visualizer_strip(cover, 40., cx)))
                    })
                    .child(div().h(px(26.)))
                    .child(div().w(px(seek + 120.)).child(self.big_song(
                        track,
                        Align::Centre,
                        24.,
                        cx,
                    )))
                    .child(div().h(px(18.)))
                    .child(div().w(px(seek)).child(self.big_seek(
                        "big-seek",
                        track.is_none(),
                        30.,
                        cx,
                    )))
                    .child(div().h(px(6.)))
                    .child(self.big_buttons(seek + 16., 64., cx))
                    .into_any_element(),
            );
        }
        out.push(
            div()
                .absolute()
                .right(px(if f.beside { pane_w + 20. + 24. } else { 24. }))
                .bottom(px(foot))
                .child(self.speaker_pill(cx))
                .into_any_element(),
        );
        if f.beside {
            out.push(
                div()
                    .absolute()
                    .right(px(20.))
                    .top(px(f.top + 8.))
                    .bottom(px(foot))
                    .w(px(pane_w))
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .map(|el| player_glass(el, 24., true, cx))
                    .child(self.big_pane(f, track, cx))
                    .into_any_element(),
            );
        }
        out
    }

    /// What stands in the cover's place: the cover as a plain card, the record, the
    /// visualizer, or the words (`wide` across).
    fn big_stage(
        &self,
        f: &Frame,
        track: Option<&Track>,
        size: f32,
        wide: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match f.look {
            "cover" => self.big_card(f, track, size, cx),
            "lyrics" => div()
                .w(px(wide))
                .h(px((f.h - f.top - 80.).max(size)))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(self.lyrics_view(super::lyrics::LyricsKind::Immersive, cx))
                .into_any_element(),
            look => self.look_stage(look, size, size + 120., f.angle, track, cx),
        }
    }

    /// The cover as a plain card: 12 corner, a deep soft shadow, no glass round it. With the
    /// visualizer on, a glow that breathes with the music.
    fn big_card(
        &self,
        f: &Frame,
        track: Option<&Track>,
        size: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let radius = px(12.);
        let picture = match f
            .moving
            .as_deref()
            .and_then(|path| super::moving::image(std::path::Path::new(path)))
        {
            // Settings › Appearance › Moving covers.
            Some(frames) => img(frames)
                .id("big-cover")
                .size(px(size))
                .rounded(radius)
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => match track.and_then(|t| t.artwork.as_deref()) {
                Some(path) => {
                    img(super::thumbs::for_size(path, size).unwrap_or_else(|| path.into()))
                        .size(px(size))
                        .rounded(radius)
                        .object_fit(ObjectFit::Cover)
                        .into_any_element()
                }
                None => {
                    let seed = track.map(super::widgets::track_seed).unwrap_or_default();
                    super::widgets::generated_cover(&seed, size, false, &p).into_any_element()
                }
            },
        };
        let shadow = if self.visualizer {
            self.visualizer_glow(cx)
        } else {
            vec![
                BoxShadow {
                    color: gpui::black().opacity(if p.dark { 0.45 } else { 0.2 }),
                    offset: point(px(0.), px(size * 0.11)),
                    blur_radius: px(size * 0.23),
                    spread_radius: px(-6.),
                },
                BoxShadow {
                    color: gpui::black().opacity(if p.dark { 0.26 } else { 0.1 }),
                    offset: point(px(0.), px(4.)),
                    blur_radius: px(12.),
                    spread_radius: px(0.),
                },
            ]
        };
        div()
            .size(px(size))
            .flex_shrink_0()
            .rounded(radius)
            .bg(p.raised)
            .shadow(shadow)
            .child(picture)
            .into_any_element()
    }

    /// The title, bold and large, the artist under it, and the heart at the right.
    fn big_song(
        &self,
        track: Option<&Track>,
        align: Align,
        title: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let artist_size = (title * 0.42).clamp(15., 21.);
        let Some(track) = track else {
            return div()
                .w_full()
                .flex()
                .flex_col()
                .when(align == Align::Centre, |el| el.items_center())
                .child(
                    super::widgets::display("Nothing playing", title).font_weight(FontWeight::BOLD),
                )
                .child(
                    div()
                        .text_size(px(artist_size))
                        .text_color(p.ink_2)
                        .child("Pick something from your library."),
                );
        };
        let artist = Page::Artist(track.artist.clone());
        let heart = self.big_heart(track, cx);
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .when(align == Align::Centre, |el| el.items_center())
            .child(
                super::widgets::display(track.title.clone(), title)
                    .font_weight(FontWeight::BOLD)
                    .max_w_full()
                    .truncate(),
            )
            .child(
                div()
                    .id("big-artist")
                    .max_w_full()
                    .mt(px(2.))
                    .text_size(px(artist_size))
                    .text_color(p.ink_2)
                    .truncate()
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(track.display_artist().to_string())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.big = false;
                        this.navigate(artist.clone(), window, cx);
                    })),
            );
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            // Centred, the heart is balanced by as much room at the left.
            .when(align == Align::Centre, |el| el.child(div().size(px(44.))))
            .child(words)
            .child(heart)
    }

    /// The favorite heart, in the accent.
    fn big_heart(&self, track: &Track, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = pal(cx);
        let favorite = self
            .tracks
            .iter()
            .find(|t| t.id == track.id)
            .map_or(track.rating, |t| t.rating)
            >= 4;
        let id = track.id.clone();
        div()
            .id("big-heart")
            .size(px(44.))
            .flex_shrink_0()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.opacity(0.8))
            .active(|s| super::glass::press_round(s, 44.))
            .child(self.heart(&track.id, favorite, 26., p.accent, cx))
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(if favorite {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                })
                .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_rating(std::slice::from_ref(&id), if favorite { 0 } else { 5 });
                if let Some(item) = this.playback.current.as_mut() {
                    item.track.rating = if favorite { 0 } else { 5 };
                }
                cx.notify();
            }))
    }

    /// The line of the lyrics sung now, bold, as the mini player has it: a press opens the
    /// lyrics. None for a song without timed lyrics (the parts close up); between lines the
    /// line keeps its room, so nothing jumps.
    fn big_line(&self, align: Align, size: f32, cx: &mut Context<Self>) -> Option<AnyElement> {
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
            .map(|l| l.text.trim().to_string())
            .unwrap_or_default();
        let serial = self.lyric_line.map_or(0, |i| i as u64 + 1);
        let text = div()
            .w_full()
            .text_size(px(size))
            .font_weight(FontWeight::BOLD)
            .truncate()
            .when(align == Align::Centre, |el| el.text_center())
            .child(line);
        Some(
            div()
                .id("big-line")
                .w_full()
                .h(px(size * 1.3))
                .mt(px(14.))
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .tooltip(|window, cx| {
                    gpui_component::tooltip::Tooltip::new("Lyrics").build(window, cx)
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.big_side = Side::Lyrics;
                    this.big_sheet = true;
                    cx.notify();
                }))
                .child(motion::fade(
                    text,
                    ("big-lyric", serial),
                    220,
                    cx,
                    |el, f, _| el.opacity(f),
                ))
                .into_any_element(),
        )
    }

    /// The seek bar in the listener's style, and under it the two times with the file's
    /// format between them.
    fn big_seek(
        &self,
        id: &'static str,
        disabled: bool,
        height: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = pal(cx);
        let track = self.playback.current.as_ref().map(|c| &c.track);
        let duration = track.map_or(0., |t| t.duration);
        let position = if track.is_some() {
            self.playback.position.min(duration.max(0.))
        } else {
            0.
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .child(self.wave_seek_bar(id, disabled, height, cx)),
            )
            .child(
                div()
                    .w_full()
                    .mt(px(4.))
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child(format_duration(position))
                    .child(track.map(quality).unwrap_or_default())
                    .child(format!(
                        "-{}",
                        format_duration((duration - position).max(0.))
                    )),
            )
    }

    /// The main buttons, in the listener's order (Settings › Appearance › Player buttons),
    /// across `wide`: plain icons, and Play as a circle in the accent.
    fn big_buttons(&self, wide: f32, play: f32, cx: &mut Context<Self>) -> Div {
        let mut names: Vec<&'static str> = super::player2::chosen_buttons(&self.settings)
            .into_iter()
            .filter(|b| super::player2::TRANSPORT.contains(b))
            .collect();
        if !names.contains(&"play") {
            names.push("play");
        }
        let count = names.len() as f32;
        let gap = ((wide - (count - 1.) * 54. - play) / (count - 1.).max(1.)).clamp(8., 40.);
        div()
            .w(px(wide))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(gap))
            .children(names.iter().map(|name| self.big_button(name, play, cx)))
    }

    fn big_button(&self, name: &str, play: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let plain =
            |id: &'static str, icon: &'static str, tip: SharedString, size: f32, on: bool| {
                div()
                    .id(id)
                    .size(px(54.))
                    .flex_shrink_0()
                    .relative()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.75))
                    .active(|s| super::glass::press_round(s, 54.))
                    .child(glyph(icon).size(px(size)).text_color(if on {
                        p.accent
                    } else if size < 30. {
                        p.ink_2
                    } else {
                        p.ink
                    }))
                    // On: a small dot under it.
                    .when(on, |el| {
                        el.child(
                            div()
                                .absolute()
                                .bottom(px(5.))
                                .left(px(25.))
                                .size(px(4.))
                                .rounded_full()
                                .bg(p.accent),
                        )
                    })
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
            };
        match name {
            "shuffle" => {
                let stays = self.settings.shuffle_stays;
                let on = stays && self.playback.shuffle;
                plain(
                    "big-shuffle",
                    "shuffle",
                    match (stays, on) {
                        (false, _) => "Shuffle this view".into(),
                        (true, true) => "Shuffle is on".into(),
                        (true, false) => "Shuffle is off".into(),
                    },
                    22.,
                    on,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if stays {
                        this.toggle_shuffle(cx)
                    } else {
                        this.play_view(0, true, cx)
                    }
                }))
                .into_any_element()
            }
            "previous" => plain(
                "big-previous",
                "previous",
                super::widgets::keys("Previous · Ctrl+Left"),
                32.,
                false,
            )
            .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Previous)))
            .into_any_element(),
            "next" => plain(
                "big-next",
                "next",
                super::widgets::keys("Next · Ctrl+Right"),
                32.,
                false,
            )
            .on_click(cx.listener(|this, _, _, _| this.player.send(Command::Next)))
            .into_any_element(),
            "repeat" => {
                let repeat = self.playback.repeat;
                plain(
                    "big-repeat",
                    if repeat == Repeat::One {
                        "repeat-one"
                    } else {
                        "repeat"
                    },
                    match repeat {
                        Repeat::Off => "Repeat is off".into(),
                        Repeat::All => "Repeating the queue".into(),
                        Repeat::One => "Repeating this song".into(),
                    },
                    22.,
                    repeat != Repeat::Off,
                )
                .on_click(cx.listener(|this, _, _, _| {
                    let next = match this.playback.repeat {
                        Repeat::Off => Repeat::All,
                        Repeat::All => Repeat::One,
                        Repeat::One => Repeat::Off,
                    };
                    this.player.send(Command::Repeat(next));
                }))
                .into_any_element()
            }
            _ => {
                let playing = self.playback.playing;
                div()
                    .id("big-play")
                    .size(px(play))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(p.accent)
                    .shadow(vec![BoxShadow {
                        color: gpui::black().opacity(if p.dark { 0.28 } else { 0.14 }),
                        offset: point(px(0.), px(play * 0.13)),
                        blur_radius: px(play * 0.34),
                        spread_radius: px(0.),
                    }])
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .active(move |s| super::glass::press_round(s, play))
                    .child(
                        glyph(if playing { "pause" } else { "play" })
                            .size(px(play * 0.46))
                            .text_color(p.accent_ink),
                    )
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(super::widgets::keys(if playing {
                            "Pause · Space"
                        } else {
                            "Play · Space"
                        }))
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx)))
                    .into_any_element()
            }
        }
    }

    /// The one glass pill at the bottom right: the speaker in use (a press picks another),
    /// and the volume.
    fn speaker_pill(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let speaker = self.current_speaker();
        let name = speaker
            .as_ref()
            .map_or_else(|| "This computer".to_string(), |s| s.name.clone());
        div()
            .h(px(46.))
            .pl(px(16.))
            .pr(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
            .map(|el| player_glass(el, 23., false, cx))
            .child(
                div()
                    .id("big-speaker")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .child(
                        glyph("cast")
                            .size(px(19.))
                            .text_color(if speaker.is_some() { p.accent } else { p.ink }),
                    )
                    .child(
                        div()
                            .max_w(px(160.))
                            .truncate()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(name),
                    )
                    .tooltip(|window, cx| {
                        gpui_component::tooltip::Tooltip::new("Play on another speaker")
                            .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        let at = window.mouse_position() - point(px(0.), px(14.));
                        this.open_speaker_menu(at, cx)
                    })),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(p.ink.opacity(0.2)))
            .child(self.volume_control(110., cx))
    }

    /// Up next and the lyrics (and the stems, when open) in one pane, with a tab for each.
    fn big_pane(&self, f: &Frame, track: Option<&Track>, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let mut tabs = vec![("big-tab-queue", "Up next", Side::Queue)];
        if f.look != "lyrics" {
            tabs.push(("big-tab-lyrics", "Lyrics", Side::Lyrics));
        }
        let one = tabs.len() == 1 && f.side == Side::Queue;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            // With the words in the cover's place, Up next alone: its name, not one tab.
            .when(one, |el| {
                el.child(
                    div()
                        .px_1()
                        .text_size(px(17.))
                        .font_weight(FontWeight::BOLD)
                        .child("Up next"),
                )
            })
            .when(!one, |el| {
                el.child(
                    div()
                        .flex()
                        .p(px(3.))
                        .rounded_full()
                        .bg(gpui::black().opacity(if p.dark { 0.2 } else { 0.06 }))
                        .children(tabs.into_iter().map(|(id, label, side)| {
                            let on = f.side == side;
                            div()
                                .id(id)
                                .flex_1()
                                .h(px(30.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .cursor_pointer()
                                .map(|el| {
                                    if on {
                                        el.bg(p.accent).text_color(p.accent_ink)
                                    } else {
                                        el.text_color(p.ink_2).hover(|s| s.text_color(p.ink))
                                    }
                                })
                                .child(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.big_side = side;
                                    cx.notify();
                                }))
                        })),
                )
            })
            .child(match f.side {
                Side::Lyrics => div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.lyrics_view(super::lyrics::LyricsKind::Big, cx))
                    .into_any_element(),
                Side::Stems => div()
                    .flex_1()
                    .min_h_0()
                    .pt_2()
                    .child(self.stems_view(track.cloned(), true, cx))
                    .into_any_element(),
                _ => self.big_queue(cx).into_any_element(),
            })
    }

    /// Up next in the pane: how many songs and how long, Shuffle in the accent, Clear, the
    /// song playing, then the rows (each can move up or go).
    fn big_queue(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let count = self.playback.queue.len();
        let remaining: f64 = self.playback.queue.iter().map(|i| i.track.duration).sum();
        let pill = |id: &'static str, icon: &'static str, label: &'static str, accent: bool| {
            div()
                .id(id)
                .h(px(30.))
                .pl(px(10.))
                .pr(px(13.))
                .rounded_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .map(|el| {
                    if accent {
                        el.bg(p.accent).text_color(p.accent_ink)
                    } else {
                        el.bg(p.ink.opacity(0.1)).text_color(p.ink)
                    }
                })
                .child(glyph(icon).size(px(15.)).text_color(if accent {
                    p.accent_ink
                } else {
                    p.ink
                }))
                .child(label)
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_1()
                    .pt_1()
                    .text_size(px(12.5))
                    .text_color(p.ink_2)
                    .child(if count == 0 {
                        "Nothing queued".to_string()
                    } else {
                        format!(
                            "{count} {} · {}",
                            if count == 1 { "song" } else { "songs" },
                            minutes(remaining)
                        )
                    }),
            )
            .when(count > 0, |el| {
                el.child(
                    div()
                        .flex()
                        .gap_2()
                        .py_2()
                        .px_1()
                        .child(
                            pill("big-queue-shuffle", "shuffle", "Shuffle", true).on_click(
                                cx.listener(|this, _, _, _| this.player.send(Command::Shuffle)),
                            ),
                        )
                        .child(pill("big-queue-clear", "close", "Clear", false).on_click(
                            cx.listener(|this, _, _, _| this.player.send(Command::ClearQueue)),
                        )),
                )
            })
            .when_some(self.playback.current.as_ref(), |el, item| {
                el.child(
                    div()
                        .h(px(48.))
                        .px_2()
                        .mx_2()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(artwork(Some(&item.track), 34., cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(p.accent)
                                        .truncate()
                                        .child(item.track.title.clone()),
                                )
                                .child(
                                    meta(item.track.display_artist().to_string(), cx).truncate(),
                                ),
                        ),
                )
            })
            .child(self.queue_rows("big-queue-rows", cx).flex_1())
    }

    /// The pane as a sheet over the foot of the player, where there is no room beside it.
    fn big_sheet(&self, f: &Frame, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = pal(cx);
        let track = self.playback.current.as_ref().map(|c| c.track.clone());
        vec![
            div()
                .id("big-sheet-dim")
                .absolute()
                .inset_0()
                .bg(gpui::black().opacity(0.34))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.big_side = Side::None;
                    this.big_sheet = false;
                    cx.notify();
                }))
                .into_any_element(),
            div()
                .id("big-sheet")
                .occlude()
                .absolute()
                .left(px(((f.w - 560.) / 2.).max(0.)))
                .w(px(f.w.min(560.)))
                .bottom_0()
                .h(px((f.h - f.top - 40.).min(f.h * 0.72)))
                .pt(px(10.))
                .px(px(14.))
                .pb(px(14.))
                .flex()
                .flex_col()
                .gap_2()
                // Over the player's own parts, which GPUI cannot blur: nearly solid, the
                // cover's colour from the left, and the edge light.
                .map(|el| {
                    let fill = super::glass::tinted(&p, 0.97);
                    let from = motion::mix(fill, p.glow, if p.dark { 0.2 } else { 0.12 });
                    super::glass::edge(
                        el.bg(linear_gradient(
                            90.,
                            linear_color_stop(from, 0.),
                            linear_color_stop(fill, 0.6),
                        )),
                        super::glass::Lift::High,
                        cx,
                    )
                })
                .rounded_t(px(28.))
                .child(
                    div()
                        .mx_auto()
                        .w(px(36.))
                        .h(px(4.))
                        .rounded_full()
                        .bg(p.ink.opacity(0.45)),
                )
                .child(self.big_pane(f, track.as_ref(), cx))
                .into_any_element(),
        ]
    }

    /// The window is wide enough for the pane beside the big player (as last drawn).
    fn pane_beside(&self) -> bool {
        self.body_bounds
            .get()
            .is_none_or(|b| f32::from(b.size.width) >= PANE_BESIDE)
    }

    /// The pane shows on `side` (beside the player, or as the sheet asked for).
    fn pane_on(&self, side: Side) -> bool {
        let shown = match self.big_side {
            Side::Lyrics if self.big_look() == "lyrics" => Side::None,
            shown => shown,
        };
        shown == side && (self.pane_beside() || self.big_sheet)
    }

    /// Opens or closes the pane on `side`.
    fn toggle_big_side(&mut self, side: Side, cx: &mut Context<Self>) {
        self.big_side = if self.pane_on(side) { Side::None } else { side };
        self.big_sheet = self.big_side != Side::None;
        cx.notify();
    }

    /// Opens (or, `toggle`, closes) the big player's More at `position`.
    pub(super) fn open_big_more(
        &mut self,
        position: Point<Pixels>,
        toggle: bool,
        cx: &mut Context<Self>,
    ) {
        if toggle && self.menu.as_ref().is_some_and(|m| m.more) {
            self.menu = None;
            cx.notify();
            return;
        }
        let Some(track) = self
            .playback
            .current
            .as_ref()
            .or_else(|| self.last_item.as_ref().map(|(item, _)| item))
            .map(|item| item.track.clone())
        else {
            return;
        };
        self.menu_serial += 1;
        self.menu = Some(super::menus::TrackMenu {
            position,
            index: 0,
            sub: None,
            highlight: None,
            serial: self.menu_serial,
            track: Some(track),
            more: true,
        });
        cx.notify();
    }

    /// More's first part: the look as one row of four, above the menu.
    pub(super) fn big_look_row(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        let now = self.settings.big_look.clone();
        div()
            .px_1()
            .pt_1()
            .pb_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .px_1()
                    .text_size(px(12.))
                    .text_color(p.ink_3)
                    .child("Look"),
            )
            .child(
                div()
                    .flex()
                    .p(px(3.))
                    .rounded_full()
                    .bg(gpui::black().opacity(if p.dark { 0.22 } else { 0.06 }))
                    .children(BIG_LOOKS.iter().enumerate().map(|(i, look)| {
                        let on = now == *look;
                        div()
                            .id(("big-look", i))
                            .flex_1()
                            .h(px(28.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .map(|el| {
                                if on {
                                    el.bg(p.accent).text_color(p.accent_ink)
                                } else {
                                    el.text_color(p.ink_2).hover(|s| s.text_color(p.ink))
                                }
                            })
                            .child(match *look {
                                "record" => "Record",
                                "lyrics" => "Lyrics",
                                "visualizer" => "Visualizer",
                                _ => "Cover",
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings.big_look = look.to_string();
                                this.persist_settings();
                                cx.notify();
                            }))
                    })),
            )
            .child(div().mt_1().mx_1().h(px(1.)).bg(p.line))
    }

    /// More's own entries, above the song's: the buttons the main row and the pill leave
    /// out, the sleep timer, the A–B loop, full screen, and the rest of the player.
    pub(super) fn big_more_entries(&self, position: Point<Pixels>) -> Vec<Entry> {
        let main: Vec<&'static str> = super::player2::chosen_buttons(&self.settings)
            .into_iter()
            .filter(|b| super::player2::TRANSPORT.contains(b))
            .collect();
        let playing = self.playback.current.is_some();
        let mut entries = vec![];
        if !main.contains(&"shuffle") {
            entries.push(if self.settings.shuffle_stays {
                Entry::item(
                    "shuffle",
                    if self.playback.shuffle {
                        "Turn shuffle off"
                    } else {
                        "Turn shuffle on"
                    },
                    None,
                    |this, _, cx| this.toggle_shuffle(cx),
                )
            } else {
                Entry::item("shuffle", "Shuffle this view", None, |this, _, cx| {
                    this.play_view(0, true, cx)
                })
            });
        }
        if !main.contains(&"repeat") {
            let repeat = self.playback.repeat;
            entries.push(Entry::item(
                if repeat == Repeat::One {
                    "repeat-one"
                } else {
                    "repeat"
                },
                match repeat {
                    Repeat::Off => "Repeat",
                    Repeat::All => "Repeating the queue",
                    Repeat::One => "Repeating this song",
                },
                None,
                move |this, _, _| {
                    this.player.send(Command::Repeat(match repeat {
                        Repeat::Off => Repeat::All,
                        Repeat::All => Repeat::One,
                        Repeat::One => Repeat::Off,
                    }))
                },
            ));
        }
        entries.push(Entry::item(
            "moon",
            match self.playback.sleep {
                Some(s) => format!(
                    "Sleep timer · {} left",
                    super::player_tools::clock(s.remaining)
                ),
                None => "Sleep timer".into(),
            },
            None,
            move |this, _, cx| this.open_tool(Tool::Sleep, position, cx),
        ));
        if playing {
            entries.push(Entry::item(
                "loop",
                if self.playback.loop_range.is_some() {
                    "Clear the A–B loop"
                } else if self.loop_start.is_some() {
                    "A–B loop: set point B"
                } else {
                    "A–B loop: set point A"
                },
                None,
                |this, _, _| this.step_loop(),
            ));
        }
        entries.extend([
            Entry::item("speed", "Speed and pitch", None, move |this, _, cx| {
                this.open_tool(Tool::Speed, position, cx)
            }),
            Entry::item(
                "eq",
                "Equalizer and sound tools",
                None,
                |this, window, cx| {
                    this.big = false;
                    this.navigate(Page::Sound, window, cx)
                },
            ),
            Entry::item("stems", "Stems", None, |this, _, cx| {
                this.big_side = Side::Stems;
                this.big_sheet = true;
                cx.notify();
            }),
            Entry::item(
                "wave",
                if self.visualizer {
                    "Stop the glow"
                } else {
                    "Glow with the music"
                },
                None,
                |this, _, cx| {
                    this.visualizer = !this.visualizer;
                    cx.notify();
                },
            ),
            Entry::item(
                "fullscreen",
                "Full screen",
                Some("F11"),
                |this, window, cx| this.set_immersive(true, window, cx),
            ),
            Entry::item("mini", "Mini player", Some("Ctrl+M"), |this, window, cx| {
                this.big = false;
                this.open_mini(window, cx);
            }),
        ]);
        entries
    }

    /// The big player's own buttons, in the title bar while it is open (one bar, not two):
    /// close at the left, why this song plays, and round glass buttons at the right: full
    /// screen (the Visualizer look), the lyrics, Up next, and More.
    pub(super) fn big_bar(&self, cx: &mut Context<Self>) -> Div {
        let p = pal(cx);
        // The song the big player shows (it was drawn first, and noted it).
        let current = self
            .playback
            .current
            .clone()
            .or_else(|| self.last_item.as_ref().map(|(item, _)| item.clone()));
        let more_open = self.menu.as_ref().is_some_and(|m| m.more);
        div()
            .flex_1()
            .h_full()
            .pl(px(14.))
            .pr_2()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .child(
                glass_round(
                    "close-big",
                    "chevron-down",
                    "Close the player · Esc",
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.big = false;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_center()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(p.ink_2)
                    .child(
                        current
                            .as_ref()
                            .map(|i| i.reason.clone())
                            .unwrap_or_default(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .when(self.big_look() == "visualizer", |el| {
                        el.child(
                            glass_round("big-full", "fullscreen", "Full screen · F11", false, cx)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_immersive(true, window, cx)
                                })),
                        )
                    })
                    // The Lyrics look shows the words itself.
                    .when(self.big_look() != "lyrics", |el| {
                        el.child(
                            glass_round(
                                "big-lyrics",
                                "lyrics",
                                "Lyrics",
                                self.pane_on(Side::Lyrics),
                                cx,
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_big_side(Side::Lyrics, cx)
                                }),
                            ),
                        )
                    })
                    .child(
                        glass_round(
                            "big-queue",
                            "queue",
                            "Up next",
                            self.pane_on(Side::Queue),
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, _, cx| this.toggle_big_side(Side::Queue, cx)),
                        ),
                    )
                    .child(
                        glass_round("big-more", "more", "More", more_open, cx).on_click(
                            cx.listener(|this, _, window, cx| {
                                let at = window.mouse_position();
                                this.open_big_more(at + point(px(-282.), px(26.)), true, cx)
                            }),
                        ),
                    ),
            )
    }

    /// The upcoming queue as a simple list, shared by the big and mini players.
    pub(super) fn up_next(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.playback.queue.len();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(faint(
                if count == 0 {
                    "Nothing queued".to_string()
                } else {
                    format!("Up next · {count}")
                },
                cx,
            ))
            .child(
                uniform_list(
                    "big-queue-list",
                    count,
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let p = pal(cx);
                        range
                            .map(|index| {
                                let item = &this.playback.queue[index];
                                div()
                                    .id(("big-queue-row", index))
                                    .h(px(48.))
                                    .w_full()
                                    .px_2()
                                    .rounded(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(p.ink.opacity(0.06)))
                                    .child(artwork(Some(&item.track), 34., cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .truncate()
                                                    .child(item.track.title.clone()),
                                            )
                                            .child(
                                                meta(item.track.display_artist().to_string(), cx)
                                                    .truncate(),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                                        if event.click_count() == 2 {
                                            this.player.send(Command::Jump(index));
                                        }
                                    }))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1(),
            )
    }
}
