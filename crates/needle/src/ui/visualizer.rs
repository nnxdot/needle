//! The visualizer: bars that move with the music, in the music's colour, under the cover in
//! the big player and full screen, and a glow behind the cover that breathes with it. It reads
//! the sound on its way to the speakers (needle_core::scope) only while it shows.
use super::{AppView, pal};
use gpui::{prelude::*, *};

/// Bars across the strip.
const BARS: usize = 48;

impl AppView {
    /// The strip of bars, `width` by `height`, mirrored about its middle line. It draws again
    /// each frame while it shows.
    pub(super) fn visualizer_strip(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = pal(cx);
        let color = p.accent;
        let soft = p.glow;
        canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                let bands = needle_core::scope::spectrum(BARS);
                let step = b.size.width / BARS as f32;
                let bar = (step * 0.56).max(px(2.));
                let middle = b.top() + b.size.height / 2.;
                for (i, value) in bands.iter().enumerate() {
                    // Higher bands are quieter: lift them a little, so the strip moves across.
                    let lift = 1. + 0.35 * i as f32 / BARS as f32;
                    let v = (value * lift).clamp(0.04, 1.);
                    let tall = (b.size.height * v).max(px(3.));
                    let x = b.left() + step * i as f32 + (step - bar) / 2.;
                    let mix = i as f32 / BARS as f32;
                    let shade = super::motion::mix(color, soft, mix * 0.6);
                    window.paint_quad(
                        fill(
                            Bounds::new(point(x, middle - tall / 2.), size(bar, tall)),
                            shade.opacity(0.9),
                        )
                        .corner_radii(bar / 2.),
                    );
                }
                window.request_animation_frame();
            },
        )
        .w(px(width))
        .h(px(height))
        .into_any_element()
    }

    /// A glow behind the cover that grows with how loud the music is now.
    pub(super) fn visualizer_glow(&self, cx: &App) -> Vec<BoxShadow> {
        let p = pal(cx);
        let level = needle_core::scope::level();
        vec![BoxShadow {
            color: p.glow.opacity(0.35 + 0.5 * level.min(1.)),
            offset: point(px(0.), px(0.)),
            blur_radius: px(48. + 140. * level.min(1.)),
            spread_radius: px(4. + 30. * level.min(1.)),
        }]
    }
}
