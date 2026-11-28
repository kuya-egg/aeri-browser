//! The profile's records of its windows: the workspace snapshot, written
//! shortly after it changes for every profile that changed.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aeri_core::appearance::Recipe;
use aeri_core::ids::{ProfileId, TabId};
use aeri_core::profile_db::{Placement, SavedSpace, SavedTab, SavedWindow, Shown, Workspace};
use aeri_core::workspace::TabList;
use gpui::App;

use super::tabs::TabEntry;
use super::windows::AeriWindow;
use super::{AppMsg, Shell};

/// How long after a change the changed workspaces are written. Changes in
/// between join the same write.
const SAVE_AFTER: Duration = Duration::from_secs(1);

pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl Shell {
    /// Notes that a profile's workspace changed; it is written soon after
    /// ([`Shell::flush_sessions`]).
    pub(super) fn touch(&mut self, profile: ProfileId) {
        self.dirty.insert(profile);
    }

    /// After each batch of messages: schedules one write of the changed
    /// workspaces [`SAVE_AFTER`] from now. A page load changes its tab's
    /// address and title several times; each change rewrote the whole
    /// workspace on the main thread, which CEF's navigation work waits on.
    /// Quitting and closing a profile's last window still write at once.
    pub(super) fn flush_sessions(&mut self, cx: &mut App) {
        if self.dirty.is_empty() || self.flush_scheduled {
            return;
        }
        self.flush_scheduled = true;
        let tx = self.tx.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(SAVE_AFTER).await;
            let _ = tx.unbounded_send(AppMsg::FlushSessions);
        })
        .detach();
    }

    pub(super) fn save_dirty_sessions(&mut self, cx: &mut App) {
        self.flush_scheduled = false;
        for profile in std::mem::take(&mut self.dirty) {
            self.save_session(profile, cx);
        }
    }

    /// Writes the profile's workspace: every space with its tabs (shown or
    /// parked), the favourites, and the open windows. With no window open
    /// (all closing), the last saved workspace stays: that is what reopens.
    pub(super) fn save_session(&self, profile: ProfileId, cx: &mut App) {
        let Some(open) = self.open.get(&profile) else {
            return;
        };
        let Some(db) = open.db.as_ref() else {
            return;
        };
        let windows: Vec<&AeriWindow> = self
            .windows
            .iter()
            .filter(|w| w.profile == profile && !w.closing)
            .collect();
        if windows.is_empty() {
            return;
        }
        let started = Instant::now();
        let saved_list = |list: &TabList<TabEntry>| -> (Vec<SavedTab>, Vec<TabId>) {
            list.iter()
                .filter(|item| !item.value.url().is_empty())
                .map(|item| (item.value.saved(item.pinned), item.id))
                .unzip()
        };
        let (favourites, favourite_ids) = saved_list(&open.favourites);
        let mut spaces = Vec::with_capacity(open.spaces.len());
        for info in &open.spaces {
            let (tabs, favourite) = match windows.iter().find(|w| w.space == info.id) {
                Some(w) => (&w.tabs, w.favourite),
                None => match open.parked.get(&info.id) {
                    Some(parked) => (&parked.tabs, parked.favourite),
                    None => continue,
                },
            };
            let (saved, ids) = saved_list(tabs);
            let shown = favourite
                .and_then(|f| favourite_ids.iter().position(|&id| id == f))
                .map(Shown::Favourite)
                .or_else(|| {
                    let active = tabs.active()?;
                    ids.iter().position(|&id| id == active).map(Shown::Tab)
                });
            spaces.push(SavedSpace {
                id: info.id,
                name: info.name.clone(),
                colour: info.colour,
                icon: info.icon.clone(),
                theme: info.theme.as_ref().map(Recipe::to_file),
                tabs: saved,
                shown,
            });
        }
        let windows: Vec<SavedWindow> = windows
            .iter()
            .map(|w| {
                let placement = w
                    .frame_window
                    .update(cx, |_, window, _| window.window_bounds().get_bounds())
                    .ok()
                    .map(|b| Placement {
                        x: f64::from(f32::from(b.origin.x)),
                        y: f64::from(f32::from(b.origin.y)),
                        width: f64::from(f32::from(b.size.width)),
                        height: f64::from(f32::from(b.size.height)),
                    });
                SavedWindow {
                    placement,
                    space: w.space,
                }
            })
            .collect();
        let workspace = Workspace {
            spaces,
            favourites,
            windows,
        };
        match db.save_workspace(&workspace) {
            Ok(()) => tracing::debug!(
                %profile,
                spaces = workspace.spaces.len(),
                windows = workspace.windows.len(),
                us = started.elapsed().as_micros() as u64,
                "session saved"
            ),
            Err(err) => tracing::warn!(%err, %profile, "could not save the session"),
        }
    }
}
