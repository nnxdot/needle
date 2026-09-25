//! Searching the music servers as you type (experimental, for octo-fiesta): songs a server can
//! bring that are not in the library show above the Songs list, and play from there. Only
//! for sources whose "search_server" switch is on (off by default).

use super::widgets::{faint, small_button};
use super::{AppView, Page};
use gpui::{prelude::*, *};
use gpui_component::button::ButtonVariants;
use needle_core::audio::{Command, QueueItem};
use needle_core::model::format_duration;
use needle_core::plugins::PluginEvent;
use std::time::Duration;

/// How long typing pauses before the servers are asked.
const WAIT: Duration = Duration::from_millis(450);
/// The most server songs shown.
const SHOWN: usize = 8;

impl AppView {
    /// Whether any signed-in source searches its server.
    fn server_search_on(&self) -> bool {
        self.plugins.plugins().into_iter().any(|p| {
            p.enabled
                && p.source.is_some_and(|s| {
                    s.signed_in && s.switches.iter().any(|w| w.id == "search_server" && w.on)
                })
        })
    }

    /// After the search text changed: ask the servers once typing pauses.
    pub(super) fn search_servers(&mut self, cx: &mut Context<Self>) {
        self.server_search += 1;
        self.server_songs.clear();
        let query = self.search_text(cx);
        if query.is_empty() || !self.server_search_on() {
            return;
        }
        let generation = self.server_search;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(WAIT).await;
            let _ = this.update(cx, |this, _| {
                if this.server_search == generation {
                    this.plugins
                        .send(PluginEvent::SourceSearch { query, generation });
                }
            });
        })
        .detach();
    }

    /// The servers' answer; an answer to an older search is dropped.
    pub(super) fn server_songs_found(
        &mut self,
        generation: u64,
        tracks: Vec<needle_core::model::Track>,
    ) {
        if generation == self.server_search {
            self.server_songs = tracks;
        }
    }

    /// The songs a server found, above the Songs list while searching.
    pub(super) fn server_songs_block(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.server_songs.is_empty()
            || self.page != Page::Songs
            || self.search_text(cx).is_empty()
        {
            return None;
        }
        let p = super::theme::pal(cx);
        let rows = self
            .server_songs
            .iter()
            .take(SHOWN)
            .enumerate()
            .map(|(i, track)| {
                let (play, queue) = (track.clone(), track.clone());
                let item = |track: needle_core::model::Track| QueueItem {
                    track,
                    reason: "From your server".into(),
                };
                div()
                    .id(("server-song", i))
                    .h(px(40.))
                    .px_2()
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .hover(|s| s.bg(p.raised))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .truncate()
                                    .child(track.title.clone()),
                            )
                            .child(
                                faint(
                                    if track.album.is_empty() {
                                        track.display_artist().to_string()
                                    } else {
                                        format!("{} · {}", track.display_artist(), track.album)
                                    },
                                    cx,
                                )
                                .truncate(),
                            ),
                    )
                    .child(faint(format_duration(track.duration), cx).flex_shrink_0())
                    .child(
                        small_button(("server-play", i), "Play").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.player.send(Command::Play(vec![item(play.clone())]));
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        small_button(("server-queue", i), "Add to queue")
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.player
                                    .send(Command::Enqueue(vec![item(queue.clone())]));
                                this.notify(format!("Added “{}” to the queue.", queue.title));
                                cx.notify();
                            })),
                    )
            });
        Some(
            div()
                .mx_4()
                .mb_3()
                .p_2()
                .rounded(px(10.))
                .bg(p.raised.opacity(0.5))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .px_2()
                        .pt_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .child("On your server"),
                        )
                        .child(
                            div()
                                .px(px(6.))
                                .rounded(px(4.))
                                .bg(p.accent_soft)
                                .text_size(px(10.5))
                                .text_color(p.accent)
                                .child("Experimental"),
                        )
                        .child(faint(
                            "Not in your library yet. Playing one fetches it.",
                            cx,
                        )),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}
