//! The big player's looks (Settings: `big_look`): the cover with the lyrics beside it, the
//! cover as a turning record, only the words very large, or the visualizer filling the space.
//! The Look button in the big player's bar steps through them.
use super::{AppView, lyrics::LyricsKind, pal, widgets::icon_button};
use gpui::{prelude::*, *};
use needle_core::model::{BIG_LOOKS, Track};

/// How fast the record turns: 33⅓ turns a minute.
const DEGREES_A_SECOND: f32 = 200.;

impl AppView {
    /// The look the big player shows now: the chosen one, or the cover when it cannot show
    /// (no lyrics for the Lyrics look; Reduce motion for the visualizer).
    pub(super) fn big_look(&self) -> &'static str {
        let chosen = BIG_LOOKS
            .iter()
            .copied()
            .find(|l| *l == self.settings.big_look)
            .unwrap_or("cover");
        match chosen {
            "lyrics" if !self.has_lyrics() => "cover",
            "visualizer" if !self.motion_allowed() => "cover",
            look => look,
        }
    }

    pub(super) fn motion_allowed(&self) -> bool {
        !self.settings.reduce_motion && super::motion::system_allows_animation()
    }

    /// The record's angle now: it turns while the music plays and stops where it is when it
    /// pauses (and keeps still with Reduce motion).
    pub(super) fn record_angle(&mut self) -> f32 {
        let turning = self.playback.playing && self.motion_allowed();
        let (base, since) = self.record_spin;
        match (turning, since) {
            (true, None) => {
                self.record_spin = (base, Some(std::time::Instant::now()));
                base
            }
            (true, Some(at)) => (base + at.elapsed().as_secs_f32() * DEGREES_A_SECOND) % 360.,
            (false, Some(at)) => {
                let angle = (base + at.elapsed().as_secs_f32() * DEGREES_A_SECOND) % 360.;
                self.record_spin = (angle, None);
                angle
            }
            (false, None) => base,
        }
    }

    /// What stands where the cover does in the big player, for a look other than the cover.
    pub(super) fn look_stage(
        &self,
        look: &str,
        art: f32,
        room: f32,
        angle: f32,
        track: Option<&Track>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match look {
            "record" => self.record_disc(track, art, angle, cx),
            "visualizer" => {
                let width = (room - 120.).clamp(300., 960.);
                div()
                    .w(px(width))
                    .h(px(art))
                    .flex()
                    .items_center()
                    .child(self.visualizer_strip(width, art * 0.8, cx))
                    .into_any_element()
            }
            _ => div()
                .w(px((room - 120.).clamp(420., 1000.)))
                .h(px(art + 60.))
                .flex()
                .flex_col()
                .child(self.lyrics_view(LyricsKind::Immersive, cx))
                .into_any_element(),
        }
    }

    /// The cover as a record: grooves, the cover as its label, light that turns with it, and
    /// the arm resting on it.
    fn record_disc(
        &self,
        track: Option<&Track>,
        size: f32,
        angle: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let label = size * 0.38;
        let white = gpui::white();
        let label_picture = track
            .and_then(|t| t.artwork.as_deref())
            .and_then(|path| super::thumbs::for_size(path, label))
            .map(|file| {
                img(file)
                    .size(px(label))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .size(px(label))
                    .rounded_full()
                    .bg(p.accent)
                    .into_any_element()
            });
        let grooves = (1..=8).map(move |i| {
            let inset = size * 0.04 + i as f32 * size * 0.032;
            div()
                .absolute()
                .top(px(inset))
                .left(px(inset))
                .size(px(size - 2. * inset))
                .rounded_full()
                .border_1()
                .border_color(white.opacity(0.05))
        });
        // Two soft lights on opposite sides, as a lamp makes on a turning record.
        let light = |turn: f32| {
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .bg(linear_gradient(
                    angle + turn,
                    linear_color_stop(white.opacity(0.), 0.55),
                    linear_color_stop(white.opacity(0.11), 1.),
                ))
        };
        div()
            .size(px(size))
            .flex_shrink_0()
            .relative()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .bg(hsla(0., 0., 0.05, 1.))
                    .shadow(vec![BoxShadow {
                        color: gpui::black().opacity(0.55),
                        offset: point(px(0.), px(18.)),
                        blur_radius: px(48.),
                        spread_radius: px(-6.),
                    }]),
            )
            .children(grooves)
            .child(light(0.))
            .child(light(180.))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(label_picture),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px((size * 0.035).max(6.)))
                            .rounded_full()
                            .bg(hsla(0., 0., 0.08, 1.)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-size * 0.06))
                    .right(px(-size * 0.1))
                    .child(
                        super::widgets::glyph("tonearm")
                            .size(px(size * 0.6))
                            .text_color(hsla(38. / 360., 0.45, 0.9, 1.)),
                    ),
            )
            .into_any_element()
    }

    /// The Look button: the look's icon; it steps to the next look.
    pub(super) fn look_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let now = self.settings.big_look.as_str();
        let (icon, name) = match now {
            "record" => ("logo", "Record"),
            "lyrics" => ("lyrics", "Lyrics"),
            "visualizer" => ("wave", "Visualizer"),
            _ => ("albums", "Cover"),
        };
        icon_button(
            "big-look",
            icon,
            format!("Look: {name}. Click for the next look."),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            let at = BIG_LOOKS
                .iter()
                .position(|l| *l == this.settings.big_look)
                .unwrap_or(0);
            this.settings.big_look = BIG_LOOKS[(at + 1) % BIG_LOOKS.len()].into();
            this.persist_settings();
            cx.notify();
        }))
    }
}
