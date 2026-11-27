//! Window layout (Phase 12, design §4): where the chrome sits (app-wide,
//! in `app.db`), ⌘S, pinning, resizing the sidebar, the layout choices in
//! the palette and Aeri's own window buttons. By default the sidebar is
//! pinned beside the page, as in Search.

use aeri_core::layout::{BarPlace, FavouriteStyle, Layout, Preset, Side, TabPlace};
use aeri_core::store::AppDb;
use gpui::{
    App, AppContext, Bounds, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, point, px, size,
};

use super::bar::{BarRow, Pick};
use super::palette::matches;
use super::{ChromeMsg, Shell};
use crate::chrome_overlay::ChromeOverlay;
use crate::mac::Edge;
use crate::window_frame::WindowFrame;

const LAYOUT: &str = "layout";

/// The layout every window shows.
pub(super) struct Chrome {
    pub layout: Layout,
}

impl Chrome {
    pub fn load(db: &AppDb) -> Self {
        let get = |key| db.setting(key).ok().flatten();
        Self {
            layout: get(LAYOUT).map_or_else(Layout::default, |json| Layout::from_json(&json)),
        }
    }
}

/// A layout choice offered in the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutChoice {
    Preset(Preset),
    Side(Side),
    Favourites(FavouriteStyle),
    Bar(BarPlace),
    WindowButtons,
}

impl LayoutChoice {
    const ALL: [LayoutChoice; 12] = [
        Self::Preset(Preset::PageFirst),
        Self::Preset(Preset::Sidebar),
        Self::Preset(Preset::SidebarAndTop),
        Self::Preset(Preset::Collapsed),
        Self::Preset(Preset::TopTabs),
        Self::Side(Side::Left),
        Self::Side(Side::Right),
        Self::Favourites(FavouriteStyle::Grid),
        Self::Favourites(FavouriteStyle::Rows),
        Self::Bar(BarPlace::Centre),
        Self::Bar(BarPlace::Upper),
        Self::WindowButtons,
    ];

    fn title(self) -> String {
        match self {
            Self::Preset(preset) => format!("Layout: {}", preset.name()),
            Self::Side(Side::Left) => "Sidebar on the Left".to_owned(),
            Self::Side(Side::Right) => "Sidebar on the Right".to_owned(),
            Self::Favourites(FavouriteStyle::Grid) => "Favourites as a Grid".to_owned(),
            Self::Favourites(FavouriteStyle::Rows) => "Favourites as Rows".to_owned(),
            Self::Bar(BarPlace::Centre) => "Address Bar in the Centre".to_owned(),
            Self::Bar(BarPlace::Upper) => "Address Bar Near the Top".to_owned(),
            Self::WindowButtons => "Show Window Buttons".to_owned(),
        }
    }

    /// Whether `layout` already has this choice (toggles: is on).
    fn chosen(self, layout: &Layout) -> bool {
        match self {
            Self::Preset(preset) => Preset::of(layout) == Some(preset),
            Self::Side(side) => layout.side == side,
            Self::Favourites(style) => layout.favourites == style,
            Self::Bar(place) => layout.bar == place,
            Self::WindowButtons => layout.window_buttons,
        }
    }

    fn apply(self, layout: &mut Layout) {
        match self {
            Self::Preset(preset) => preset.apply(layout),
            Self::Side(side) => layout.side = side,
            Self::Favourites(style) => layout.favourites = style,
            Self::Bar(place) => layout.bar = place,
            Self::WindowButtons => layout.window_buttons = !layout.window_buttons,
        }
    }
}

/// The two windows a frame's chrome slides in with (sidebar, strip),
/// unplaced and unseen until `mac::Frame` adopts them.
pub(super) fn open_overlays(
    frame: &WindowHandle<WindowFrame>,
    cx: &mut App,
) -> Option<[WindowHandle<ChromeOverlay>; 2]> {
    let view = frame.entity(cx).ok()?;
    let mut open = |edge: Edge| {
        let options = WindowOptions {
            titlebar: None,
            kind: WindowKind::PopUp,
            focus: false,
            show: true,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(240.), px(44.)),
            ))),
            ..Default::default()
        };
        let view = view.clone();
        cx.open_window(options, |_, cx| cx.new(|_| ChromeOverlay::new(view, edge)))
            .inspect_err(|err| tracing::error!(%err, ?edge, "could not open a chrome overlay"))
            .ok()
    };
    let side = open(Edge::Side)?;
    let Some(strip) = open(Edge::Top) else {
        let _ = side.update(cx, |_, window, _| window.remove_window());
        return None;
    };
    Some([side, strip])
}

impl Shell {
    /// Shows window `window_id`'s chrome as the layout says: the frame view
    /// draws what is pinned, the AppKit frame fits the page beside it.
    pub(super) fn apply_layout(&mut self, window_id: u64, cx: &mut App) {
        crate::motion::set_prefs(&self.chrome.layout, cx);
        let Some(w) = self.window(window_id) else {
            return;
        };
        let mut layout = self.chrome.layout;
        layout.sidebar_pinned &= !w.sidebar_hidden;
        layout.bookmarks_bar = self.sync_bookmarks_bar(w, cx);
        let _ = w
            .frame_window
            .update(cx, |frame, _, cx| frame.set_layout(layout, cx));
        let frame = w.frame.clone();
        self.later(move |_| frame.set_layout(layout));
    }

    fn apply_layout_everywhere(&mut self, cx: &mut App) {
        let ids: Vec<u64> = self.windows.iter().map(|w| w.id).collect();
        for id in ids {
            self.apply_layout(id, cx);
        }
    }

    /// Changes the layout for every window, keeps it, and shows it.
    pub(super) fn change_layout(&mut self, change: impl FnOnce(&mut Layout), cx: &mut App) {
        let before = self.chrome.layout;
        change(&mut self.chrome.layout);
        if let Err(err) = self.db.set_setting(LAYOUT, &self.chrome.layout.to_json()) {
            tracing::warn!(%err, "could not save the layout");
        }
        // A column put away with ⌘S comes back only when where it lives
        // changes, not for a colour or motion preference.
        let after = self.chrome.layout;
        let placement = |l: Layout| (l.tabs, l.side, l.rail, l.sidebar_pinned, l.fold_icons);
        if placement(before) != placement(after) {
            for w in &mut self.windows {
                w.sidebar_hidden = false;
            }
        }
        self.apply_layout_everywhere(cx);
    }

    /// ⌘ went down or up: the frames show or hide the tab numbers.
    pub(super) fn command_held(&mut self, held: bool, cx: &mut App) {
        for w in &self.windows {
            let _ = w
                .frame_window
                .update(cx, |frame, _, cx| frame.set_command_held(held, cx));
        }
    }

    /// ⌘S: a pinned sidebar steps aside until the next ⌘S (the layout
    /// stays); an unpinned one (or a rail) slides in, or away.
    ///
    /// Folding gives the page the window at once and the sidebar leaves
    /// from its place. Unfolding slides it out over the page first; when it
    /// lands the frame asks again (a shortcut message), the layout pins it
    /// and the page shrinks once. ⌘S while it slides out turns it round.
    pub(super) fn toggle_sidebar(&mut self, window_id: u64, cx: &mut App) {
        let layout = self.chrome.layout;
        // "⌘S makes the column icons only": the sidebar stays and its
        // width folds to the column of icons and back (Search
        // `Preferences.shrinks`, `Fold.swift`).
        if layout.fold_icons && layout.sidebar_pinned && layout.tabs == TabPlace::Side {
            self.change_layout(|layout| layout.rail = !layout.rail, cx);
            return;
        }
        let Some(w) = self.window_mut(window_id) else {
            return;
        };
        let frame = w.frame.clone();
        if !layout.sidebar_pinned {
            self.later(move |_| {
                frame.call(Edge::Side);
            });
        } else if !w.sidebar_hidden {
            w.sidebar_hidden = true;
            // Before the layout unpins it: the overlay takes the column's
            // place as the page covers it.
            self.later(move |_| frame.fold());
            self.apply_layout(window_id, cx);
        } else if frame.take_landed() {
            w.sidebar_hidden = false;
            self.apply_layout(window_id, cx);
            self.later(move |_| frame.end_fold());
        } else if frame.is_unfolding() {
            self.later(move |_| frame.cancel_unfold());
        } else {
            self.later(move |_| frame.unfold());
        }
    }

    pub(super) fn chrome_msg(&mut self, msg: ChromeMsg, cx: &mut App) {
        match msg {
            ChromeMsg::Pin { edge, pinned } => {
                tracing::info!(?edge, pinned, "chrome pinned");
                self.change_layout(
                    |layout| match edge {
                        Edge::Side => layout.sidebar_pinned = pinned,
                        Edge::Top => layout.top_pinned = pinned,
                    },
                    cx,
                );
            }
            ChromeMsg::Width { width, rail, done } => {
                // Live while dragging; kept when the edge is let go.
                self.chrome.layout.sidebar_width = width;
                if let Some(rail) = rail {
                    self.chrome.layout.rail = rail;
                }
                if done {
                    tracing::info!(width, "sidebar resized");
                    self.change_layout(|_| {}, cx);
                } else {
                    self.apply_layout_everywhere(cx);
                }
            }
            ChromeMsg::PointerLeft { window } => {
                if let Some(w) = self.window(window)
                    && !w.frame.pointer_in_chrome()
                {
                    let _ = w.frame_window.update(cx, |view, window, cx| {
                        view.clear_tab_hover(window, cx);
                    });
                }
            }
        }
    }

    pub(super) fn choose_layout(&mut self, choice: LayoutChoice, cx: &mut App) {
        tracing::info!(?choice, "layout chosen");
        self.change_layout(|layout| choice.apply(layout), cx);
    }

    /// Palette rows for the layout choices matching `words`; chosen ones
    /// (and toggles that are on) are ticked.
    pub(super) fn layout_rows(&self, words: &[String]) -> Vec<BarRow> {
        LayoutChoice::ALL
            .into_iter()
            .map(|choice| (choice, choice.title()))
            .filter(|(_, title)| matches(words, title))
            .map(|(choice, title)| BarRow {
                title,
                detail: "Layout".to_owned(),
                hint: if choice.chosen(&self.chrome.layout) {
                    "✓"
                } else {
                    ""
                }
                .to_owned(),
                pick: Pick::Layout(choice),
            })
            .collect()
    }
}
