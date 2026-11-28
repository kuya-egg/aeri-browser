//! Native new-tab content, shown while the blank Chromium page is ordered out.

use aeri_core::appearance::{Mode, Rgba};
use aeri_core::ids::TabId;
use aeri_core::keymap::Shortcut;
use gpui::{
    Context, CursorStyle, Entity, FocusHandle, Focusable, FontWeight, ImageSource, KeyDownEvent,
    MouseButton, ObjectFit, Window, div, img, prelude::*, px,
};

use super::{WindowFrame, page_insets, tab_hover::blend};
use crate::shell::AppMsg;
use crate::symbol::{Weight, symbol};
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{Theme, c};

const SEARCH_PROMPT: &str = "Search or enter an address";

pub(super) struct Home {
    pub(super) tab: Option<TabId>,
    pub(super) root: FocusHandle,
    input: Entity<TextInput>,
    actions: [FocusHandle; ACTIONS.len()],
}

impl Home {
    pub(super) fn new(cx: &mut Context<WindowFrame>) -> Self {
        let input = cx.new(|cx| TextInput::new(SEARCH_PROMPT, cx).with_tab_stop());
        cx.subscribe(&input, |this, input, event: &InputEvent, cx| match event {
            InputEvent::Submitted(text) | InputEvent::SubmittedShifted(text) => {
                if !text.trim().is_empty()
                    && let Some(tab) = this.home.tab
                {
                    this.send(AppMsg::HomeSubmit {
                        window: this.window_id,
                        tab,
                        text: text.clone(),
                    });
                }
            }
            InputEvent::Cancelled => input.update(cx, |field, cx| field.set_text("", cx)),
            InputEvent::Edited(_) | InputEvent::Up | InputEvent::Down => {}
        })
        .detach();
        Self {
            tab: None,
            root: cx.focus_handle(),
            input,
            actions: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
        }
    }
}

const ACTIONS: [(Shortcut, &str, &str); 3] = [
    (Shortcut::ShowBookmarks, "bookmark", "Bookmarks"),
    (Shortcut::ShowHistory, "clock", "History"),
    (Shortcut::Settings, "gearshape", "Settings"),
];

impl WindowFrame {
    pub(crate) fn set_home(
        &mut self,
        tab: Option<TabId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.home.tab != tab {
            self.home.tab = tab;
            self.home
                .input
                .update(cx, |field, cx| field.set_text("", cx));
            if tab.is_some() {
                window.focus(&self.home.root, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn home_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if self.home.tab.is_none()
            || key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
        {
            return;
        }
        match key.key.as_str() {
            "tab" => {
                cx.stop_propagation();
                if key.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
            "enter" | "space" => {
                if let Some(index) = self.home.actions.iter().position(|f| f.is_focused(window)) {
                    cx.stop_propagation();
                    self.send(AppMsg::Shortcut {
                        window: self.window_id,
                        shortcut: ACTIONS[index].0,
                    });
                }
            }
            _ => {}
        }
    }

    pub(super) fn home(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let insets = page_insets(
            &self.layout,
            self.fullscreen,
            f64::from(self.pinned_width()),
        );
        let dark = cx.global::<Theme>().resolved_in(self.window_id).mode == Mode::Dark;
        let [background, ink, subdued, outline, input, hovered] = if dark {
            [0x141414, 0xF5F5F5, 0xB3B3B3, 0x666666, 0x202020, 0x373737]
        } else {
            [0xF5F5F5, 0x161616, 0x565656, 0x858585, 0xFFFFFF, 0xDEDEDE]
        }
        .map(Rgba::rgb);
        let still = crate::motion::still(self.window_id, cx);
        let focus = self.home.input.focus_handle(cx);
        let search = div()
            .id("home-search")
            .role(gpui::Role::TextInput)
            .aria_label(SEARCH_PROMPT)
            .cursor(CursorStyle::IBeam)
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(46.))
            .w_full()
            .px(px(14.))
            .mt(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(c(if focus.is_focused(window) {
                ink
            } else {
                outline
            }))
            .bg(c(input))
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(ink))
            .capture_any_mouse_down(move |event, window, cx| {
                if event.button == MouseButton::Left {
                    window.activate_window();
                    window.focus(&focus, cx);
                }
            })
            .child(symbol("magnifyingglass", 16., Weight::Medium, subdued))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.home.input.clone()),
            );
        let control = |index: usize| {
            let (shortcut, icon, label) = ACTIONS[index];
            let (button, hover) = self.tab_hover(
                div().id(("home-action", index)),
                ("home", index as u64),
                still,
                cx,
            );
            button
                .role(gpui::Role::Button)
                .aria_label(label)
                .track_focus(&self.home.actions[index])
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(px(7.))
                .h(px(38.))
                .px(px(10.))
                .rounded(px(8.))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(subdued))
                .border_1()
                .border_color(c(Rgba::rgba(0)))
                .bg(c(blend(hovered.with_alpha(0.), hovered, hover)))
                .active(|s| s.bg(c(hovered)))
                .focus_visible(|s| s.border_color(c(ink)))
                .on_click(cx.listener(move |this, _, _, _| {
                    this.send(AppMsg::Shortcut {
                        window: this.window_id,
                        shortcut,
                    });
                }))
                .child(symbol(icon, 14., Weight::Medium, subdued))
                .child(label)
        };

        div()
            .id("aeri-home")
            .absolute()
            .left(px(insets.left as f32))
            .right(px(insets.right as f32))
            .top(px(insets.top as f32))
            .bottom(px(insets.bottom as f32))
            .rounded(px(insets.radius as f32))
            .when(insets.flush_top, |home| {
                home.rounded_tl(px(0.)).rounded_tr(px(0.))
            })
            .overflow_hidden()
            .bg(c(background))
            .flex()
            .items_center()
            .justify_center()
            .px(px(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w_full()
                    .max_w(px(384.))
                    .children(crate::brand::ribbon(dark).map(|image| {
                        img(ImageSource::from(image))
                            .size(px(88.))
                            .object_fit(ObjectFit::Contain)
                    }))
                    .child(search)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .justify_center()
                            .gap(px(6.))
                            .mt(px(8.))
                            .children((0..ACTIONS.len()).map(control)),
                    ),
            )
    }
}
