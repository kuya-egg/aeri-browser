//! Window layout (Phase 12, design §4): where the chrome sits and how it
//! shows. One choice for every window, kept in `app.db` as a JSON value;
//! missing keys take their defaults, so older values keep reading. The
//! defaults and sizes are Search's (`Design.swift` `Metrics`).

use serde::{Deserialize, Serialize};

/// The narrowest, default and widest sidebar, in points.
pub const MIN_SIDEBAR: u16 = 176;
pub const DEFAULT_SIDEBAR: u16 = 232;
pub const MAX_SIDEBAR: u16 = 440;
/// The page keeps at least this much width beside the sidebar.
pub const MIN_PAGE: f64 = 320.;
/// The column as icons only: its fixed width, one tab's tile in it, and the
/// band the three window buttons take stacked at its top (Search
/// `Metrics.iconColumn`, `iconTile`, `iconLights`; `SideIcons.swift`).
pub const ICON_COLUMN: f64 = 52.;
pub const ICON_TILE: f64 = 32.;
pub const ICON_LIGHTS: f64 = 80.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    Left,
    Right,
}

/// Where the tabs are listed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabPlace {
    #[default]
    Side,
    Top,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FavouriteStyle {
    #[default]
    Grid,
    Rows,
}

/// Where the floating address bar opens over the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarPlace {
    #[default]
    Centre,
    Upper,
}

/// What a tab wears beside its title and on a pinned square (Search
/// Settings › Appearance › Tabs show).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Glyph {
    #[default]
    Letters,
    Icons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    /// The sidebar keeps its place beside the page (else it slides in).
    pub sidebar_pinned: bool,
    /// The strip keeps its place above the page (else it slides in).
    pub top_pinned: bool,
    pub side: Side,
    /// The full sidebar's width, in points.
    pub sidebar_width: u16,
    /// The column shows only icons (Search `sidebar.icons`, `Prefs.swift`):
    /// a tile per tab at a fixed width. Nothing opens under the pointer.
    pub rail: bool,
    pub tabs: TabPlace,
    pub favourites: FavouriteStyle,
    pub bar: BarPlace,
    /// The window's close, minimise and zoom buttons.
    pub window_buttons: bool,
    // Search's Settings › Appearance and Tabs (`Prefs.swift` keys in the
    // comments).
    /// The page as a card beside the sidebar (`layout.cards`); off, edge
    /// to edge with a hairline.
    pub cards: bool,
    /// The card stays in full screen (`layout.cardsFullScreen`).
    pub cards_full_screen: bool,
    /// The sidebar slides in and out eased, not on the springs
    /// (`sidebar.slides`).
    pub slides: bool,
    /// ⌘S makes the sidebar an icon column instead of hiding it
    /// (`sidebar.foldIcons`).
    pub fold_icons: bool,
    /// Back, forward and reload beside the window buttons, top tabs only
    /// (`toolbar.left`).
    pub nav_left: bool,
    /// A shorter strip, 36 pt (`tabs.compact`).
    pub compact_strip: bool,
    /// Taller rows (36) and larger titles (14) in the sidebar
    /// (`sidebar.roomy`).
    pub roomy: bool,
    pub glyph: Glyph,
    /// Tab titles in ink, not soft grey (`tabs.highContrast`).
    pub high_contrast: bool,
    /// The shown tab says its site, not its title; the title comes back
    /// under the pointer (`tabs.address`).
    pub address_on_tab: bool,
    /// The shown tab's full address in a field across the top bar.
    pub address_strip: bool,
    /// Numbers on the first nine tabs while ⌘ is held (`tabs.shortcuts`).
    pub tab_numbers: bool,
    /// Everything happens at once (`motion.instant`).
    pub reduce_animations: bool,
    /// Springs become a short ease-out in Low Power Mode or when the Mac
    /// runs hot (`motion.light`).
    pub light_motion: bool,
    /// The bookmarks bar is up above the page (Search `barShown`,
    /// `App.swift:716`): asked for, with bookmarks to show. The shell sets
    /// it per window when it hands the layout over; it is never stored.
    #[serde(skip)]
    pub bookmarks_bar: bool,
}

impl Default for Layout {
    /// Search's: the sidebar beside the page, which is a card; ⌘S folds
    /// the sidebar away and it slides back out at the window's edge.
    fn default() -> Self {
        Self {
            sidebar_pinned: true,
            top_pinned: false,
            side: Side::Left,
            sidebar_width: DEFAULT_SIDEBAR,
            rail: false,
            tabs: TabPlace::Side,
            favourites: FavouriteStyle::Grid,
            bar: BarPlace::Centre,
            window_buttons: true,
            cards: true,
            cards_full_screen: false,
            slides: false,
            fold_icons: false,
            nav_left: false,
            compact_strip: false,
            roomy: false,
            glyph: Glyph::Letters,
            high_contrast: false,
            address_on_tab: true,
            address_strip: false,
            tab_numbers: false,
            reduce_animations: false,
            light_motion: false,
            bookmarks_bar: false,
        }
    }
}

/// Named arrangements, each setting several choices at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Nothing pinned: the page has the whole window.
    PageFirst,
    /// Search's: the sidebar carries the window buttons and history; the
    /// strip slides in.
    Sidebar,
    SidebarAndTop,
    /// The column as icons only, at a fixed width.
    Collapsed,
    /// Tabs across the top, as in a classic browser.
    TopTabs,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Self::PageFirst,
        Self::Sidebar,
        Self::SidebarAndTop,
        Self::Collapsed,
        Self::TopTabs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::PageFirst => "Page First",
            Self::Sidebar => "Sidebar",
            Self::SidebarAndTop => "Sidebar and Top Bar",
            Self::Collapsed => "Collapsed Sidebar",
            Self::TopTabs => "Top Tabs",
        }
    }

    /// The choices this preset makes; the rest of `layout` stays.
    pub fn apply(self, layout: &mut Layout) {
        let (sidebar, top, rail, tabs) = self.shape();
        layout.sidebar_pinned = sidebar;
        layout.top_pinned = top;
        layout.rail = rail;
        layout.tabs = tabs;
    }

    /// The preset `layout` is in, if any.
    pub fn of(layout: &Layout) -> Option<Preset> {
        let shape = (
            layout.sidebar_pinned,
            layout.top_pinned,
            layout.rail && layout.sidebar_pinned,
            layout.tabs,
        );
        Self::ALL.into_iter().find(|p| p.shape() == shape)
    }

    fn shape(self) -> (bool, bool, bool, TabPlace) {
        match self {
            Self::PageFirst => (false, false, false, TabPlace::Side),
            Self::Sidebar => (true, false, false, TabPlace::Side),
            Self::SidebarAndTop => (true, true, false, TabPlace::Side),
            Self::Collapsed => (true, false, true, TabPlace::Side),
            Self::TopTabs => (false, true, false, TabPlace::Top),
        }
    }
}

impl Layout {
    /// Reads a stored layout; anything unreadable is the default.
    pub fn from_json(json: &str) -> Self {
        let mut layout: Self = serde_json::from_str(json).unwrap_or_default();
        layout.sidebar_width = clamp_width(layout.sidebar_width);
        layout
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a layout always serialises")
    }

    /// The sidebar's width in a window `window_width` points wide: the
    /// chosen width, narrowed to leave the page [`MIN_PAGE`], never below
    /// the minimum. The chosen width is kept for wider windows.
    pub fn sidebar_width_in(&self, window_width: f64) -> f64 {
        f64::from(self.sidebar_width)
            .min(window_width - MIN_PAGE)
            .max(f64::from(MIN_SIDEBAR))
    }

    /// The column is showing as icons now: tabs down a side, and asked for
    /// (Search `Preferences.iconColumn`).
    pub fn icon_column(&self) -> bool {
        self.rail && self.tabs == TabPlace::Side
    }

    /// The column's width as drawn: fixed with icons only, the chosen one
    /// otherwise (Search `Preferences.columnWidth`).
    pub fn column_width(&self, window_width: f64) -> f64 {
        if self.icon_column() {
            ICON_COLUMN
        } else {
            self.sidebar_width_in(window_width)
        }
    }
}

/// A width the Person dragged to, kept in range.
pub fn clamp_width(width: u16) -> u16 {
    width.clamp(MIN_SIDEBAR, MAX_SIDEBAR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_searchs_sidebar() {
        let layout = Layout::default();
        assert_eq!(Preset::of(&layout), Some(Preset::Sidebar));
        assert_eq!(layout.sidebar_width, DEFAULT_SIDEBAR);
    }

    #[test]
    fn a_preset_is_recognised_after_it_is_applied_and_keeps_other_choices() {
        for preset in Preset::ALL {
            let mut layout = Layout {
                side: Side::Right,
                sidebar_width: 300,
                ..Layout::default()
            };
            preset.apply(&mut layout);
            assert_eq!(Preset::of(&layout), Some(preset));
            assert_eq!((layout.side, layout.sidebar_width), (Side::Right, 300));
        }
    }

    #[test]
    fn stored_layouts_survive_missing_and_bad_values() {
        let partial = Layout::from_json(r#"{"side":"right","sidebarWidth":999}"#);
        assert_eq!(partial.side, Side::Right);
        assert_eq!(partial.sidebar_width, MAX_SIDEBAR);
        assert!(partial.window_buttons);
        assert_eq!(Layout::from_json("not json"), Layout::default());
        let layout = Layout {
            rail: true,
            favourites: FavouriteStyle::Rows,
            ..Layout::default()
        };
        assert_eq!(Layout::from_json(&layout.to_json()), layout);
    }

    #[test]
    fn widths_stay_in_range_and_leave_the_page_room() {
        assert_eq!(clamp_width(100), MIN_SIDEBAR);
        assert_eq!(clamp_width(243), 243);
        assert_eq!(clamp_width(999), MAX_SIDEBAR);
        let layout = Layout {
            sidebar_width: 360,
            ..Layout::default()
        };
        assert_eq!(layout.sidebar_width_in(1280.), 360.);
        assert_eq!(layout.sidebar_width_in(600.), 280.);
        assert_eq!(layout.sidebar_width_in(400.), f64::from(MIN_SIDEBAR));
    }

    #[test]
    fn the_column_of_icons_is_fixed_and_only_beside_side_tabs() {
        let icons = Layout {
            rail: true,
            ..Layout::default()
        };
        assert!(icons.icon_column());
        assert_eq!(icons.column_width(1280.), ICON_COLUMN);
        assert_eq!(icons.column_width(300.), ICON_COLUMN);
        let top = Layout {
            tabs: TabPlace::Top,
            ..icons
        };
        assert!(!top.icon_column());
        assert_eq!(top.column_width(1280.), f64::from(DEFAULT_SIDEBAR));
    }
}
