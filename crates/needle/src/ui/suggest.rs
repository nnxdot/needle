use super::{AppView, pal, widgets::faint};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, RopeExt,
    input::{Enter, Escape, IndentInline, MoveDown, MoveUp},
};
use needle_core::query::{self, SuggestionKind};

impl AppView {
    pub(super) fn update_suggestions(&mut self, cx: &mut Context<Self>) {
        let state = self.search.read(cx);
        if !self.search_focused {
            self.suggestions.clear();
            return;
        }
        let text = state.value().to_string();
        let cursor = state.cursor().min(text.len());
        self.suggestions = query::suggest_with_library(&self.library, &text, cursor, 8);
        self.suggestion_active = None;
    }

    pub(super) fn accept_suggestion(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(suggestion) = self.suggestions.get(index).cloned() else {
            return;
        };
        let text = self.search.read(cx).value().to_string();
        let range = suggestion.replace.start.min(text.len())..suggestion.replace.end.min(text.len());
        let next = format!("{}{}{}", &text[..range.start], suggestion.insert, &text[range.end..]);
        let cursor = range.start + suggestion.insert.len();
        self.search.update(cx, |s, cx| {
            s.set_value(next, window, cx);
            let position = s.text().offset_to_position(cursor);
            s.set_cursor_position(position, window, cx);
        });
        self.update_suggestions(cx);
        cx.notify();
    }

    /// Keys the dropdown claims before the text field sees them.
    pub(super) fn suggestion_keys(&self, field: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        field
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                if !this.suggestions.is_empty() {
                    let n = this.suggestions.len();
                    this.suggestion_active = Some(this.suggestion_active.map_or(0, |i| (i + 1) % n));
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                if !this.suggestions.is_empty() {
                    let n = this.suggestions.len();
                    this.suggestion_active = Some(this.suggestion_active.map_or(n - 1, |i| (i + n - 1) % n));
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                if !this.suggestions.is_empty() {
                    this.accept_suggestion(this.suggestion_active.unwrap_or(0), window, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Enter, window, cx| {
                if let Some(index) = this.suggestion_active.filter(|_| !this.suggestions.is_empty()) {
                    this.accept_suggestion(index, window, cx);
                    cx.stop_propagation();
                } else {
                    this.suggestions.clear();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, _, cx| {
                if !this.suggestions.is_empty() {
                    this.suggestions.clear();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }

    pub(super) fn suggestion_list(&self, width: Pixels, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.suggestions.is_empty() {
            return None;
        }
        let p = pal(cx);
        let rows = self.suggestions.iter().enumerate().map(|(index, s)| {
            let active = self.suggestion_active == Some(index);
            let kind = match s.kind {
                SuggestionKind::Field => "field",
                SuggestionKind::Operator => "compare",
                SuggestionKind::Keyword => "keyword",
                SuggestionKind::Function => "function",
                SuggestionKind::Value => "value",
                SuggestionKind::Example => "example",
            };
            div()
                .id(("suggestion", index))
                .h(px(32.))
                .px_2()
                .rounded(px(5.))
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .when(active, |el| el.bg(p.raised_hover))
                .hover(|s| s.bg(p.raised))
                .child(faint(kind, cx).w(px(58.)).flex_shrink_0())
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w(relative(0.6))
                        .truncate()
                        .text_size(px(13.))
                        .font_family(cx.theme().mono_font_family.clone())
                        .child(s.label.clone()),
                )
                .child(faint(s.detail.clone(), cx).flex_1().min_w_0().truncate().text_right())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.accept_suggestion(index, window, cx);
                    }),
                )
        });
        Some(
            div().absolute().top(px(32.)).left_0().child(
                deferred(
                    anchored().snap_to_window_with_margin(px(8.)).child(
                        div()
                            .id("suggestions")
                            .occlude()
                            .mt_1()
                            .w(width)
                            .p_1()
                            .rounded(px(8.))
                            .bg(cx.theme().popover)
                            .border_1()
                            .border_color(p.line)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .children(rows)
                            .child(
                                faint("Tab completes · ↑ ↓ choose · Esc closes", cx)
                                    .mt_1()
                                    .px_2()
                                    .py_1()
                                    .border_t_1()
                                    .border_color(p.line_soft),
                            ),
                    ),
                )
                .with_priority(1),
            ),
        )
    }
}
