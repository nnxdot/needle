use super::{
    AppView, Event, motion, pal,
    widgets::{faint, glyph, meta, small_button},
};
use gpui::{prelude::*, *};
use gpui_component::button::ButtonVariants;
use needle_core::{
    audio::Command,
    media::{self, Lyrics, LyricsSource},
    model::Track,
};

/// Where lyrics show, which sets their size and which scroll position they keep.
#[derive(Clone, Copy, PartialEq)]
pub enum LyricsKind {
    Big,
    Side,
    Mini,
    Immersive,
}

/// The bundled plugin that finds lyrics on NetEase Cloud Music.
const NETEASE: &str = "netease-lyrics";

impl AppView {
    /// Install (if needed) and turn on the NetEase lyrics plugin, then look again.
    fn add_netease(&mut self, cx: &mut Context<Self>) {
        use needle_core::plugins::{self, PluginEvent};
        if let Err(error) = plugins::install_example(&self.library, NETEASE) {
            self.fail(format!("Could not add the NetEase plugin: {error:#}"));
            cx.notify();
            return;
        }
        self.plugins.send(PluginEvent::Reload);
        self.plugins.send(PluginEvent::Enable(NETEASE.into(), true));
        self.notify("NetEase lyrics is on. Looking for lyrics there…");
        // Look again once the plugin has loaded.
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(item) = this.playback.current.clone() {
                    this.lookup_media(&item.track);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Look up lyrics, missing album art, and the artist photo for a newly playing track.
    pub(super) fn track_started(&mut self, track: &Track) {
        self.plugins
            .send(needle_core::plugins::PluginEvent::TrackStarted(Box::new(
                track.clone(),
            )));
        self.lookup_media(track);
    }

    /// Find lyrics and any missing cover for `track`, in the background.
    pub(super) fn lookup_media(&mut self, track: &Track) {
        self.lyrics = None;
        self.lyric_line = None;
        let library = self.library.clone();
        let sender = self.sender.clone();
        let online = self.settings.online_media;
        let plugins = self.plugins.clone();
        let track = track.clone();
        std::thread::spawn(move || {
            let lyrics =
                media::lyrics(&library, &track, online, Some(&plugins)).unwrap_or_else(|e| {
                    eprintln!("Lyrics lookup failed: {e:#}");
                    None
                });
            let _ = sender.send(Event::Lyrics(track.id.clone(), lyrics));
            if online
                && track.artwork.is_none()
                && media::fetch_album_art(&library, &track).unwrap_or(0) > 0
            {
                let _ = sender.send(Event::ArtFetched);
            }
        });
    }

    pub(super) fn fetched_art(&mut self, track: Track) {
        if let Some(path) = track.artwork {
            self.art_override.insert(track.id, path);
        }
    }

    /// Ask for an artist photo. Grids pass `online: false` so scrolling never floods the web
    /// services; the artist page itself may look online.
    pub(super) fn request_artist_image(&mut self, name: &str, online: bool) {
        if name.is_empty() || self.artist_images.contains_key(name) {
            return;
        }
        self.artist_images.insert(name.to_string(), None);
        let library = self.library.clone();
        let sender = self.sender.clone();
        let online = online && self.settings.online_media;
        let name = name.to_string();
        std::thread::spawn(move || {
            let path = media::artist_image(&library, &name, online).ok().flatten();
            let _ = sender.send(Event::ArtistImage(
                name,
                path.map(|p| p.to_string_lossy().into()),
            ));
        });
    }

    /// Photos already saved on this computer, for a whole grid at once. Never goes online.
    pub(super) fn cached_artist_images(&mut self, names: Vec<String>) {
        let names: Vec<String> = names
            .into_iter()
            .filter(|n| !self.artist_images.contains_key(n))
            .collect();
        if names.is_empty() {
            return;
        }
        let library = self.library.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let found = names
                .into_iter()
                .map(|name| {
                    let path = media::artist_image(&library, &name, false).ok().flatten();
                    (name, path.map(|p| p.to_string_lossy().into()))
                })
                .collect();
            let _ = sender.send(Event::ArtistImages(found));
        });
    }

    /// Keep the sung line in view as playback moves.
    pub(super) fn follow_lyrics(&mut self) {
        let Some((id, Some(lyrics))) = &self.lyrics else {
            return;
        };
        if self
            .playback
            .current
            .as_ref()
            .is_none_or(|c| &c.track.id != id)
        {
            return;
        }
        let line = lyrics.line_at(self.playback.position + 0.15);
        if line != self.lyric_line {
            self.lyric_line = line;
            self.lyric_glide = line.is_some();
            self.mini_lyric_glide = line.is_some();
        }
    }

    /// Ease the main window's lyrics (the big player's and the side panel's) a step toward
    /// the sung line; called every frame.
    pub(super) fn glide_lyrics(&mut self, window: &mut Window, cx: &App) {
        if self.lyric_glide {
            let big = glide(&self.lyrics_scroll, self.lyric_line, cx);
            let side = glide(&self.panel_lyrics_scroll, self.lyric_line, cx);
            let moving = big == Glide::Moving || side == Glide::Moving;
            // Lyrics that just opened have no layout for a frame or two: wait for it (up to
            // half a second), or the sung line would stay out of view until the next one.
            // Only for lyrics on screen: hidden ones never get a layout, and waiting for them
            // redrew the window 30 times after every line.
            let big_shown = self.big || self.immersive;
            let side_shown = self.panel_shown == Some(super::Panel::Lyrics);
            let waiting = !moving
                && ((big_shown && big == Glide::NotLaidOut)
                    || (side_shown && side == Glide::NotLaidOut))
                && self.lyric_glide_waits < 30;
            self.lyric_glide_waits = if waiting {
                self.lyric_glide_waits + 1
            } else {
                0
            };
            self.lyric_glide = moving || waiting;
            if self.lyric_glide {
                window.request_animation_frame();
            }
        }
    }

    /// The same for the mini player's lyrics, which scroll on their own. Returns whether
    /// they are still moving.
    pub(super) fn glide_mini_lyrics(&mut self, cx: &App) -> bool {
        if self.mini_lyric_glide {
            self.mini_lyric_glide =
                glide(&self.mini_lyrics_scroll, self.lyric_line, cx) == Glide::Moving;
        }
        self.mini_lyric_glide
    }

    /// Lyrics for the big player, the side panel, the mini player (which keeps its own scroll
    /// position), or the immersive full screen.
    pub(super) fn lyrics_view(&self, kind: LyricsKind, cx: &mut Context<Self>) -> impl IntoElement {
        let big = matches!(kind, LyricsKind::Big | LyricsKind::Immersive);
        let mini = kind == LyricsKind::Mini;
        let huge = kind == LyricsKind::Immersive;
        let p = pal(cx);
        let current = self.playback.current.as_ref().map(|c| c.track.id.clone());
        let lyrics: Option<&Lyrics> = match (&self.lyrics, &current) {
            (Some((id, lyrics)), Some(cur)) if id == cur => lyrics.as_ref(),
            _ => None,
        };
        let loading = current.is_some()
            && self
                .lyrics
                .as_ref()
                .is_none_or(|(id, _)| Some(id) != current.as_ref());
        // The side panel has room for larger lines than the mini player.
        let side = kind == LyricsKind::Side;
        let size = if huge {
            40.
        } else if big {
            22.
        } else if side {
            19.
        } else {
            15.
        };
        let message = |title: &str, detail: &str, cx: &mut Context<Self>| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .px_4()
                .child(glyph("lyrics").size(px(26.)).text_color(p.ink_3))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title.to_string()),
                )
                .child(
                    meta(detail.to_string(), cx)
                        .text_center()
                        .line_height(relative(1.5)),
                )
        };
        let Some(lyrics) = lyrics else {
            if current.is_none() {
                return message(
                    "Nothing playing",
                    "Lyrics show here while a song plays.",
                    cx,
                )
                .into_any_element();
            }
            if loading {
                return message("Looking for lyrics…", "", cx).into_any_element();
            }
            return message(
                "No lyrics for this song",
                if self.settings.online_media {
                    "None were found next to the file, inside it, or on LRCLIB. Put an .lrc file beside the song to add your own."
                } else {
                    "Needle checked for an .lrc file and the file's own tags. Turn on online lookups to search LRCLIB too."
                },
                cx,
            )
            .when(!mini, |el| {
                el.child(
                    small_button("write-lyrics", "Write lyrics")
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| this.open_timing(window, cx))),
                )
            })
            .when(!self.settings.online_media, |el| {
                el.child(
                    small_button("enable-online", "Turn on online lookups")
                        .ghost()
                        // Turn lookups on right here and search again, without leaving the player.
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.online_media = true;
                            this.persist_settings();
                            if let Some(item) = this.playback.current.clone() {
                                this.lookup_media(&item.track);
                            }
                            this.notify("Online lookups are on. Looking for lyrics on LRCLIB…");
                            cx.notify();
                        })),
                )
            })
            // NetEase has timed lyrics for many songs LRCLIB lacks: offer its plugin here.
            .when(
                self.settings.online_media
                    && !mini
                    && !self
                        .plugins
                        .plugins()
                        .iter()
                        .any(|p| p.manifest.id == NETEASE && p.enabled),
                |el| {
                    el.child(
                        small_button("add-netease", "Also look on NetEase")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.add_netease(cx))),
                    )
                },
            )
            .into_any_element();
        };
        if lyrics.instrumental {
            return message("Instrumental", "This track has no words.", cx).into_any_element();
        }
        let source = match lyrics.source {
            LyricsSource::Sidecar => "From a file beside the song".to_string(),
            LyricsSource::Embedded => "From the song's tags".to_string(),
            LyricsSource::Lrclib => "From LRCLIB".to_string(),
            LyricsSource::Plugin => format!("From {}", lyrics.provider),
        };
        // Lines are direct children of the scroll area so the view can scroll to one of them.
        let gap = if huge {
            26.
        } else if big {
            14.
        } else if side {
            12.
        } else {
            8.
        };
        let lines: Vec<AnyElement> = if lyrics.lines.is_empty() {
            lyrics
                .plain
                .lines()
                .map(|l| {
                    div()
                        .min_h(px(size * 1.6))
                        .text_size(px(size))
                        .line_height(relative(1.6))
                        .child(l.to_string())
                        .into_any_element()
                })
                .collect()
        } else {
            let active = self.lyric_line;
            let position = self.playback.position + 0.1;
            lyrics
                .lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let time = line.time;
                    let state = match active {
                        Some(a) if a == i => 0,
                        Some(a) if i < a => 1,
                        _ => 2,
                    };
                    // Like Apple Music: the sung line is bold and full strength; the rest fade
                    // back, the sung-past most.
                    let line_el = div()
                        .id(("lyric", i))
                        .pb(px(gap))
                        .text_size(px(size))
                        .line_height(relative(1.35))
                        .font_weight(match (state, big) {
                            (0, _) => FontWeight::BOLD,
                            (_, true) => FontWeight::SEMIBOLD,
                            _ => FontWeight::MEDIUM,
                        })
                        .cursor_pointer()
                        .text_color(match state {
                            0 => p.ink,
                            // Light backgrounds need darker faded lines to stay legible.
                            1 => p.ink.opacity(if p.dark { 0.28 } else { 0.4 }),
                            _ => p.ink.opacity(if p.dark { 0.45 } else { 0.58 }),
                        })
                        .hover(|s| s.text_color(p.ink.opacity(0.8)));
                    // Karaoke: in the sung line, words light up as they are sung.
                    let end = lyrics.lines.get(i + 1).map_or(line.time + 6., |l| l.time);
                    let karaoke = (state == 0).then(|| line.word_at(position, end)).flatten();
                    let line_el = match karaoke {
                        Some((word, progress)) => {
                            // Highlights paint over the line's colour, so dimming uses fade_out.
                            let dim = if p.dark { 0.62 } else { 0.55 };
                            let mut at = 0;
                            let highlights: Vec<_> = line
                                .words
                                .iter()
                                .enumerate()
                                .map(|(w, part)| {
                                    let range = at..at + part.text.len();
                                    at = range.end;
                                    let fade = match w.cmp(&word) {
                                        std::cmp::Ordering::Less => 0.,
                                        std::cmp::Ordering::Equal => dim * (1. - progress.sqrt()),
                                        std::cmp::Ordering::Greater => dim,
                                    };
                                    (
                                        range,
                                        HighlightStyle {
                                            fade_out: Some(fade),
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect();
                            line_el.child(
                                StyledText::new(line.text.clone()).with_highlights(highlights),
                            )
                        }
                        // Text straight in the line, so long lines wrap instead of running off.
                        None => line_el.child(if line.text.is_empty() {
                            "♪".to_string()
                        } else {
                            line.text.clone()
                        }),
                    };
                    let line_el = line_el.on_click(
                        cx.listener(move |this, _, _, _| this.player.send(Command::Seek(time))),
                    );
                    if state == 0 && karaoke.is_none() {
                        // The new line brightens in as it arrives.
                        let (dim, ink) = (p.ink.opacity(0.4), p.ink);
                        motion::animate(line_el, ("lyric-on", i), 380, cx, move |el, t| {
                            el.text_color(motion::mix(dim, ink, t))
                        })
                    } else {
                        line_el.into_any_element()
                    }
                })
                .collect()
        };
        div()
            .id("lyrics")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(if mini {
                &self.mini_lyrics_scroll
            } else if side {
                &self.panel_lyrics_scroll
            } else {
                &self.lyrics_scroll
            })
            .pr_2()
            // Room above and below so any line can glide to the reading spot.
            .when(!lyrics.lines.is_empty(), |el| {
                // Enough room below for the last lines to move up, not a screenful of nothing.
                el.pt(px(if huge {
                    260.
                } else if big {
                    140.
                } else {
                    48.
                }))
                .pb(px(if huge {
                    320.
                } else if big {
                    140.
                } else {
                    60.
                }))
            })
            .when(lyrics.lines.is_empty(), |el| el.pb_20())
            .children(lines)
            .child(
                div()
                    .mt_6()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(faint(source, cx))
                    .when(!mini, |el| {
                        el.child(
                            small_button(
                                "edit-timing",
                                if lyrics.lines.is_empty() {
                                    "Time these lyrics"
                                } else {
                                    "Edit timing"
                                },
                            )
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_timing(window, cx)),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }
}

/// Where `scroll` should be so the sung line sits a third of the way down.
fn lyric_target(scroll: &ScrollHandle, line: usize) -> Option<f32> {
    let item = scroll.bounds_for_item(line)?;
    let view = scroll.bounds();
    let max = f32::from(scroll.max_offset().height);
    Some(f32::from(view.top() + view.size.height * 0.3 - item.top()).clamp(-max, 0.))
}

/// Move `scroll` a step toward the sung line. Returns whether it still has further to go.
/// How a glide went this frame.
#[derive(Clone, Copy, PartialEq)]
enum Glide {
    Moving,
    Done,
    /// The line has no place yet (the lyrics were just opened, or are not shown).
    NotLaidOut,
}

fn glide(scroll: &ScrollHandle, line: Option<usize>, cx: &App) -> Glide {
    let Some(line) = line else {
        return Glide::Done;
    };
    let Some(target) = lyric_target(scroll, line) else {
        return Glide::NotLaidOut;
    };
    let mut offset = scroll.offset();
    let now = f32::from(offset.y);
    let next = if motion::enabled(cx) {
        now + (target - now) * 0.14
    } else {
        target
    };
    let done = (target - next).abs() < 0.5;
    offset.y = px(if done { target } else { next });
    scroll.set_offset(offset);
    if done { Glide::Done } else { Glide::Moving }
}
