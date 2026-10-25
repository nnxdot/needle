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
        }
    }

    /// The order and Only album artists, beside the Artists page's title.
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
