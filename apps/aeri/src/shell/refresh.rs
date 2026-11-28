//! Refreshing windows: mirroring a window's favourites, tabs and spaces in
//! its sidebar, strip and address bar.

use aeri_core::ids::{ProfileId, TabId};
use aeri_core::profile_db::records_history;
use aeri_core::workspace::Item;
use gpui::App;

use super::tabs::TabEntry;
use super::windows::host_of;
use super::{NEW_TAB_URL, OpenProfile, Shell};
use crate::window_frame::{SidebarTab, SpaceChip};

impl Shell {
    pub(super) fn refresh_profile_windows(&self, profile: ProfileId, cx: &mut App) {
        let ids: Vec<u64> = self
            .windows
            .iter()
            .filter(|w| w.profile == profile)
            .map(|w| w.id)
            .collect();
        for id in ids {
            self.refresh_window(id, cx);
        }
    }

    /// The sidebar's row for `item`.
    fn sidebar_row(
        &self,
        open: &OpenProfile,
        item: &Item<TabEntry>,
        favourite: bool,
        shown: Option<TabId>,
    ) -> SidebarTab {
        SidebarTab {
            id: item.id,
            label: item.value.label(),
            host: host_of(item.value.url()),
            url: item.value.url().to_owned(),
            icon: item.value.icon.clone(),
            favicon: item
                .value
                .favicon
                .as_ref()
                .map(|icon| icon.image.clone())
                .or_else(|| open.icons.get(&host_of(item.value.url()))),
            pinned: item.pinned,
            favourite,
            active: Some(item.id) == shown,
            loading: item.value.model.is_loading(),
            audible: item.value.model.activity.audible,
            asleep: item.value.cef.is_none(),
            reading: self.reading_of(item.id),
        }
    }

    /// Mirrors one of the window's own tabs, not the one it shows, in its
    /// sidebar row only: its title, icon, address or loading changed, and
    /// nothing else the window draws depends on that. Falls back to
    /// [`Shell::refresh_window`] when the row is not listed yet.
    pub(super) fn refresh_tab_row(&self, window_id: u64, tab: TabId, cx: &mut App) {
        let Some(w) = self.window(window_id) else {
            return;
        };
        let (Some(open), Some(item)) = (self.open.get(&w.profile), w.tabs.get(tab)) else {
            return;
        };
        let row = self.sidebar_row(open, item, false, w.shown());
        let listed = w
            .frame_window
            .update(cx, |frame, _, cx| frame.set_tab_row(row, cx))
            .unwrap_or(false);
        if !listed {
            self.refresh_window(window_id, cx);
        }
    }

    /// Mirrors the window's favourites, tabs and spaces in the sidebar and
    /// the shown tab in the address bar and the strip.
    pub(super) fn refresh_window(&self, window_id: u64, cx: &mut App) {
        self.find_follow_shown(window_id, cx);
        self.peek_follow_shown(window_id);
        let Some(w) = self.window(window_id) else {
            return;
        };
        let Some(open) = self.open.get(&w.profile) else {
            return;
        };
        let shown = w.shown();
        let row =
            |item: &Item<TabEntry>, favourite: bool| self.sidebar_row(open, item, favourite, shown);
        let tabs: Vec<SidebarTab> = open
            .favourites
            .iter()
            .map(|item| row(item, true))
            .chain(w.tabs.iter().map(|item| row(item, false)))
            .collect();
        let look = self.tab_look();
        let player = self.player_rows(&tabs);
        self.ensure_reading(cx);
        let spaces: Vec<SpaceChip> = open
            .spaces
            .iter()
            // With spaces off the window shows only its own: no dot, no
            // switcher (Search `usesSpaces`, Side.swift:1413).
            .filter(|space| self.prefs.spaces.on() || space.id == w.space)
            .map(|space| SpaceChip {
                id: space.id,
                name: space.name.clone(),
                colour: space.tint(),
                icon: space.icon.clone(),
                current: space.id == w.space,
            })
            .collect();
        let model = self.shown_model(w);
        let home = shown.filter(|_| {
            w.active_split().is_none() && model.as_ref().is_some_and(|tab| tab.url == NEW_TAB_URL)
        });
        if w.frame.home_tab() != home {
            let frame = w.frame.clone();
            self.later(move |_| frame.set_home_tab(home));
        }
        let (popup_blocked, downloads) = self.strip_notices(window_id);
        // Only web and file pages can be bookmarked.
        let bookmarked = model.as_ref().and_then(|tab| {
            records_history(&tab.url).then(|| {
                open.db
                    .as_ref()
                    .is_some_and(|db| db.is_bookmarked(&tab.url).unwrap_or(false))
            })
        });
        let extensions = !open.extensions.is_empty();
        let _ = w.frame_window.update(cx, |frame, window, cx| {
            frame.set_home(home, window, cx);
            frame.set_tabs(tabs, cx);
            frame.set_tab_look(look, cx);
            frame.set_player(player, cx);
            frame.set_spaces(spaces, cx);
            frame.set_notices(popup_blocked, downloads, cx);
            frame.set_doors(extensions, bookmarked == Some(true), cx);
            if let Some(tab) = &model {
                frame.show_history(tab.can_go_back, tab.can_go_forward, cx);
            }
        });
        if let Some(tab) = model {
            let _ = w.bar.update(cx, |bar, _, cx| bar.show_tab(&tab, cx));
        }
    }
}
