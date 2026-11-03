//! The Artists page's own controls: its order (A to Z, most played, most songs, newest), Only
//! album artists, and the A–Z bar along its right edge.
use super::{AppView, Page, pal, widgets::segmented};
use gpui::{prelude::*, *};
use gpui_component::{Selectable, button::Button};
use needle_core::artists::{ArtistChoice, ArtistOrder};

/// The orders, as the control names them.
const ORDERS: [(&str, &str); 4] = [
    ("name", "A–Z"),
    ("most_played", "Most played"),
    ("most_songs", "Most songs"),
    ("recently_added", "Newest"),
];

impl AppView {
    /// What the Artists page lists, from the settings.
    pub(super) fn artist_choice(&self) -> ArtistChoice {
        ArtistChoice {
            order: ArtistOrder::from_name(&self.settings.artist_order),
            album_artists_only: self.settings.album_artists_only,
            compilations_hidden: self.settings.hide_compilation_artists,
        }
    }

    /// The order, Only album artists, and Hide Various Artists-only, beside the Artists page's
    /// title.
    pub(super) fn artist_sort_controls(&self, tight: bool, cx: &mut Context<Self>) -> Div {
        let selected = ORDERS
            .iter()
            .position(|(key, _)| *key == self.settings.artist_order)
            .unwrap_or(0);
        let weak = cx.entity().downgrade();
        let labels: Vec<&'static str> = ORDERS.iter().map(|(_, label)| *label).collect();
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .child(segmented(
                "artist-order",
                &labels,
                selected,
                cx,
                move |index, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.settings.artist_order = ORDERS[index].0.into();
                        this.persist_settings();
                        this.refresh(cx);
                    });
                },
            ))
            .child(
                Button::new("album-artists-only")
                    .label(if tight {
                        "Album artists"
                    } else {
                        "Only album artists"
                    })
                    .selected(self.settings.album_artists_only)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.album_artists_only = !this.settings.album_artists_only;
                        this.persist_settings();
                        this.refresh(cx);
                    })),
            )
            .child(
                Button::new("hide-compilation-artists")
                    .label(if tight {
                        "No VA-only"
                    } else {
                        "Hide Various Artists-only"
                    })
                    .tooltip("Hide artists who are only on Various Artists albums")
                    .selected(self.settings.hide_compilation_artists)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.hide_compilation_artists =
                            !this.settings.hide_compilation_artists;
                        this.persist_settings();
                        this.refresh(cx);
                    })),
            )
    }

    /// The A–Z bar: each letter the artists start with, top to bottom; a letter scrolls to the
    /// first artist filed under it. Shown in A to Z order with enough artists to need it.
    pub(super) fn letter_bar(&self, columns: usize, cx: &mut Context<Self>) -> Option<Div> {
        if self.page != Page::Artists
            || ArtistOrder::from_name(&self.settings.artist_order) != ArtistOrder::Name
            || self.groups.len() < 24
        {
            return None;
        }
        let p = pal(cx);
        let mut letters: Vec<(String, usize)> = vec![];
        for (index, group) in self.groups.iter().enumerate() {
            if !group.letter.is_empty() && letters.last().is_none_or(|(l, _)| *l != group.letter) {
                letters.push((group.letter.clone(), index));
            }
        }
        // As many letters as fit beside the grid (below the title bar and the page's head,
        // above the player); with more, every second or third shows, the first and last
        // always, and each still jumps to its own place.
        let room = self.window_height - LETTER_BAR_ABOVE - f32::from(self.foot_room()) - 16.;
        let letters = fit_letters(letters, (room / LETTER_PITCH).floor().max(3.) as usize);
        Some(
            div()
                .absolute()
                .top(px(8.))
                .bottom(self.foot_room() + px(8.))
                .right(px(2.))
                .w(px(18.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(1.))
                .children(letters.into_iter().map(|(letter, index)| {
                    div()
                        .id(SharedString::from(format!("letter-{letter}")))
                        .w_full()
                        .flex()
                        .justify_center()
                        .rounded(px(4.))
                        .text_size(px(10.5))
                        .line_height(px(LETTER_PITCH - 1.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.ink_3)
                        .cursor_pointer()
                        .hover(|s| s.text_color(p.accent).bg(p.raised))
                        .child(letter)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.grid_scroll
                                .scroll_to_item(index / columns.max(1), gpui::ScrollStrategy::Top);
                            cx.notify();
                        }))
                })),
        )
    }
}

/// The height of one letter in the A–Z bar, with its gap.
const LETTER_PITCH: f32 = 15.;

/// The room above the A–Z bar's grid: the title bar and the Artists page's head.
const LETTER_BAR_ABOVE: f32 = 210.;

/// At most `room` letters, spread evenly: the first and the last always.
fn fit_letters<T>(letters: Vec<T>, room: usize) -> Vec<T> {
    let n = letters.len();
    if n <= room || room < 3 {
        return letters;
    }
    // One place kept for the last letter, which may fall between steps.
    let step = (n - 1).div_ceil((room - 2).max(1));
    letters
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % step == 0 || *i == n - 1)
        .map(|(_, l)| l)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::fit_letters;

    #[test]
    fn letters_fit_the_room() {
        let all: Vec<usize> = (0..52).collect();
        assert_eq!(fit_letters(all.clone(), 60), all);
        let fit = fit_letters(all.clone(), 20);
        assert!(fit.len() <= 20, "{} letters", fit.len());
        assert_eq!(fit.first(), Some(&0));
        assert_eq!(fit.last(), Some(&51));
    }
}
