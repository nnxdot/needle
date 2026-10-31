//! The big player's looks (Settings: `big_look`): the cover with the lyrics beside it, the
//! cover as a turning record, only the words very large, or the visualizer filling the space.
//! The Look button in the big player's bar steps through them.
use super::{AppView, lyrics::LyricsKind, pal, widgets::icon_button};
use gpui::{prelude::*, *};
use needle_core::model::{BIG_LOOKS, Track};

/// How fast the record's sheen turns: once in 12 seconds, as Android's record.
const DEGREES_A_SECOND: f32 = 30.;

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
            "visualizer" => div()
                .w(px((room - 120.).clamp(300., 960.)))
                .h(px(art))
                .flex()
                .items_center()
                .justify_center()
                .child(self.visualizer_light(art * 1.15, cx))
                .into_any_element(),
            // The words stay in their place: they scroll inside it, never over the song's name.
            _ => div()
                .w(px((room - 120.).clamp(420., 1000.)))
                .h(px(art + 60.))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(self.lyrics_view(LyricsKind::Immersive, cx))
                .into_any_element(),
        }
    }

    /// The cover as a record, as on Android: the whole cover round, fine dark grooves over it,
    /// a soft sheen, the hole, and the cream arm resting on it at the top right. The cover
    /// itself cannot turn here (pictures do not rotate in this toolkit), so the sheen turns.
    fn record_disc(
        &self,
        track: Option<&Track>,
        size: f32,
        angle: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let white = gpui::white();
        let black = gpui::black();
        let cover = track
            .and_then(|t| t.artwork.as_deref())
            .and_then(|path| super::thumbs::for_size(path, size))
            .map(|file| {
                img(file)
                    .absolute()
                    .inset_0()
                    .size(px(size))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                let tone = track.map_or(p.accent, |t| {
                    super::widgets::seed_color(&super::widgets::track_seed(t))
                });
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .bg(tone)
                    .into_any_element()
            });
        // Rings from near the edge to the label, as Android draws them.
        let grooves = (0..14).map(move |i| {
            let k = 0.95 - i as f32 * 0.045;
            let inset = size * (1. - k) / 2.;
            div()
                .absolute()
                .top(px(inset))
                .left(px(inset))
                .size(px(size * k))
                .rounded_full()
                .border_1()
                .border_color(black.opacity(0.16))
        });
        // The sheen: light from one side and less from the other, turning with the record.
        let light = |turn: f32, strength: f32| {
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .bg(linear_gradient(
                    angle + turn,
                    linear_color_stop(white.opacity(0.), 0.5),
                    linear_color_stop(white.opacity(strength), 1.),
                ))
        };
        let centred = |el: Div| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(el)
        };
        let arm = |name: &'static str, color: Hsla, drop: f32| {
            div()
                .absolute()
                .top(px(drop))
                .left_0()
                .child(super::widgets::glyph(name).size(px(size)).text_color(color))
        };
        let cream = rgb(0xF2E8D6);
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
                        color: black.opacity(0.55),
                        offset: point(px(0.), px(18.)),
                        blur_radius: px(48.),
                        spread_radius: px(-6.),
                    }]),
            )
            .child(cover)
            .children(grooves)
            .child(light(135., 0.14))
            .child(light(315., 0.08))
            .child(centred(
                div()
                    .size(px(size * 0.094))
                    .rounded_full()
                    .bg(black.opacity(0.55)),
            ))
            .child(centred(
                div()
                    .size(px((size * 0.028).max(5.)))
                    .rounded_full()
                    .bg(rgb(0x0F0E0D)),
            ))
            .child(arm("record-arm", black.opacity(0.3), 6.))
            .child(arm("record-arm", cream.into(), 0.))
            .child(arm("record-arm-dot", rgb(0x1B1816).into(), 0.))
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
