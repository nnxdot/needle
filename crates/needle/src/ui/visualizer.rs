//! The visualizer: bars that move with the music, in the music's colour, under the cover in
//! the big player and full screen, a glow behind the cover that breathes with it, and the
//! Visualizer look's light. It reads
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

    /// The Visualizer look, as on Android: light that moves with the music, in the music's
    /// colours, `side` square. A soft glow in the middle swells with the bass, and rays all
    /// round are each as long as their band is loud (low bands at the top, mirrored down both
    /// sides, so the picture is even).
    pub(super) fn visualizer_light(&self, side: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let (inner, outer) = (p.accent, p.glow);
        canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                let bands = needle_core::scope::spectrum(BARS);
                let average = |from: usize, to: usize| {
                    bands[from..to].iter().sum::<f32>() / (to - from) as f32
                };
                let bass = average(0, 6).clamp(0., 1.);
                let level = average(0, BARS).clamp(0., 1.);
                let c = b.center();
                let r = f32::from(b.size.width.min(b.size.height)) / 2.;
                // The glow: rings from wide and faint to small and bright, as a soft light.
                let glow = r * (0.55 + 0.45 * bass);
                const RINGS: usize = 14;
                for i in 0..RINGS {
                    let t = i as f32 / RINGS as f32;
                    let radius = glow * (1. - 0.85 * t);
                    let color = super::motion::mix(outer, inner, t)
                        .opacity((0.015 + 0.03 * level) + (0.02 + 0.05 * bass) * t);
                    window.paint_quad(
                        fill(
                            Bounds::centered_at(c, size(px(2. * radius), px(2. * radius))),
                            color,
                        )
                        .corner_radii(px(radius)),
                    );
                }
                // The rays.
                let base = r * 0.34;
                let rays = BARS * 2;
                let stroke = (std::f32::consts::TAU * base / rays as f32 * 0.55).max(2.);
                for i in 0..rays {
                    let band = if i < BARS { i } else { rays - 1 - i };
                    let v = bands[band].clamp(0., 1.);
                    let angle = i as f32 / rays as f32 * std::f32::consts::TAU
                        - std::f32::consts::FRAC_PI_2;
                    let (dx, dy) = (angle.cos(), angle.sin());
                    let length = base + r * 0.62 * (0.06 + v);
                    let start = point(c.x + px(dx * base), c.y + px(dy * base));
                    let end = point(c.x + px(dx * length), c.y + px(dy * length));
                    let color = super::motion::mix(inner, outer, band as f32 / BARS as f32)
                        .opacity(0.55 + 0.45 * v);
                    let mut line = PathBuilder::stroke(px(stroke));
                    line.move_to(start);
                    line.line_to(end);
                    if let Ok(path) = line.build() {
                        window.paint_path(path, color);
                    }
                    // Round ends.
                    for at in [start, end] {
                        window.paint_quad(
                            fill(Bounds::centered_at(at, size(px(stroke), px(stroke))), color)
                                .corner_radii(px(stroke / 2.)),
                        );
                    }
                }
                window.request_animation_frame();
            },
        )
        .size(px(side))
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
