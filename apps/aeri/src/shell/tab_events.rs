//! The page events that update a window's tabs.

use aeri_core::engine::Event;
use aeri_core::ids::TabId;
use aeri_core::profile_db::records_history;
use aeri_core::tab::LoadState;
use gpui::App;

use super::Shell;
use super::session::now;
use super::tabs::SiteIcon;
use super::windows::{Spot, host_of};

impl Shell {
    pub(super) fn engine_event(&mut self, event: Event, cx: &mut App) {
        self.dusk_event(&event, cx);
        self.site_styles_event(&event);
        self.find_event(&event, cx);
        self.zoom_event(&event);
        self.peek_event(&event);
        if self.little_event(&event, cx) {
            return;
        }
        self.reader_event(&event);
        let tab_id = event.tab();
        let Some(spot) = self.spot(tab_id) else {
            return;
        };
        match &event {
            Event::LoadFailed {
                url, error_text, ..
            } => tracing::info!(%tab_id, %url, %error_text, "load failed"),
            Event::RendererGone { reason, .. } => tracing::warn!(%tab_id, %reason, "renderer gone"),
            // Typing is something done since the last close: ⌘Z is the page's.
            Event::Typed { .. } => self.undo_close_in(None),
            _ => {}
        }
        let Some(profile) = self.spot_profile(spot) else {
            return;
        };
        // A rested page's close belongs to the old page, not the tab (which
        // may already have a new one).
        if matches!(event, Event::Closed { .. }) && self.resting.closing.contains(&tab_id) {
            return;
        }
        let Some(item) = self.list_mut(spot).and_then(|list| list.get_mut(tab_id)) else {
            return;
        };
        if item.value.cef.is_none() && item.value.unseen.is_some() {
            // Put away: whatever its old page still reports is moot.
            return;
        }
        let mut changed = false;
        // An icon replaced or cleared leaves GPUI's sprite atlas too.
        let mut dropped = None;
        let mut kept = None;
        match &event {
            Event::AddressChanged { url, .. } => {
                changed |= item.value.blocked_popup.take().is_some();
                // A site's icon stays until the tab goes to another site.
                if item
                    .value
                    .favicon
                    .as_ref()
                    .is_some_and(|icon| icon.host != host_of(url))
                {
                    dropped = item.value.favicon.take();
                    changed = true;
                }
            }
            Event::FaviconChanged {
                page_url, favicon, ..
            } => {
                // An icon fetched for a page the tab has since left is stale.
                let host = host_of(&item.value.model.url);
                if host == host_of(page_url) {
                    dropped = std::mem::replace(
                        &mut item.value.favicon,
                        favicon.as_ref().and_then(|icon| SiteIcon::new(host, icon)),
                    );
                    changed = true;
                    kept = item
                        .value
                        .favicon
                        .as_ref()
                        .map(|icon| (icon.host.clone(), icon.image.clone()));
                }
            }
            _ => {}
        }
        if let Some(old) = dropped {
            cx.drop_image(old.image, None);
        }
        let model = &mut item.value.model;
        let activity = model.activity;
        changed |= model.apply(&event);
        if model.activity != activity {
            tracing::info!(%tab_id, activity = ?model.activity, "tab activity");
        }
        let finished = matches!(
            event,
            Event::LoadingChanged {
                is_loading: false,
                ..
            }
        ) && model.load == LoadState::Loaded;
        let (url, title) = (model.url.clone(), model.title.clone());
        // A fresh icon is kept for its site, on disk too.
        if let (
            Event::FaviconChanged {
                favicon: Some(icon),
                ..
            },
            Some(shown),
        ) = (&event, kept)
            && let Some(open) = self.open.get(&profile)
        {
            open.icons.put(&shown.0, shown.1, icon, cx);
        }
        if !changed {
            return;
        }
        match spot {
            // A tab the window does not show changes only its own row.
            Spot::Window(window_id) if row_only(&event) && !self.on_screen(window_id, tab_id) => {
                self.refresh_tab_row(window_id, tab_id, cx);
            }
            Spot::Window(window_id) => self.refresh_window(window_id, cx),
            Spot::Favourite(profile) => self.refresh_profile_windows(profile, cx),
            Spot::Parked(..) => {}
        }
        if matches!(
            event,
            Event::AddressChanged { .. } | Event::TitleChanged { .. }
        ) {
            self.touch(profile);
        }
        // History (stage 1, Continuity): a finished top-level load is a
        // visit; a later title belongs to it.
        let title_changed = matches!(event, Event::TitleChanged { .. });
        if (finished || title_changed)
            && records_history(&url)
            && let Some(history) = self
                .open
                .get(&profile)
                .and_then(|open| open.history.as_ref())
        {
            if finished {
                history.record_visit(url, title, now());
            } else {
                history.set_title(url, title);
            }
        }
    }
}

/// Page events that change nothing about a tab but its sidebar row while
/// the window does not show it. Sound changes the player, so it is not one.
fn row_only(event: &Event) -> bool {
    matches!(
        event,
        Event::TitleChanged { .. }
            | Event::FaviconChanged { .. }
            | Event::LoadingChanged { .. }
            | Event::AddressChanged { .. }
    )
}

impl Shell {
    /// Whether `window_id` shows `tab`, alone or in its split.
    fn on_screen(&self, window_id: u64, tab: TabId) -> bool {
        self.window(window_id).is_some_and(|w| {
            w.shown() == Some(tab) || w.active_split().is_some_and(|pair| pair.contains(&tab))
        })
    }
}
