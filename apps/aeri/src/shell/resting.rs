//! Resting tabs (Phase 11): releasing the pages of tabs the Person is not
//! looking at, after the Person's period or while macOS is short of
//! memory. The rules are [`aeri_core::resources`]; a resting tab is a
//! put-away tab ([`TabEntry::put_away`]) whose page is closed and loads
//! again when shown.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use aeri_cef::TaskMonitor;
use aeri_core::ids::{ProfileId, TabId};
use aeri_core::resources::{
    Candidate, SAVING_POWER_REST_AFTER, awake_reason, idle_rests, pressure_rests,
};
use aeri_core::store::AppDb;
use aeri_core::trust::site_of;
use aeri_core::workspace::Item;
use futures::channel::mpsc::UnboundedSender;
use gpui::{App, WindowHandle};

use super::tabs::TabEntry;
use super::windows::Spot;
use super::{AppMsg, NEW_TAB_URL, Shell};
use crate::mac::{self, Pressure};
use crate::resource_view::ResourceView;

/// How often the Person's period is checked.
const CHECK_EVERY: Duration = Duration::from_secs(60);

pub(super) struct Resting {
    /// Rest tabs unseen this long; `None`: never.
    pub(super) after: Option<Duration>,
    /// A development override of `after` is in force.
    pub(super) fixed: bool,
    pub(super) pressure: Pressure,
    /// Pages closing because their tab rests: their close is not the tab's.
    pub(super) closing: HashSet<TabId>,
    pub(super) view: Option<WindowHandle<ResourceView>>,
    /// Chromium's sampler, alive only while the view is open.
    pub(super) monitor: Option<TaskMonitor>,
}

impl Resting {
    /// Reads the Person's period, then checks it each minute (or as often as
    /// `fixed`, a development override) and watches memory pressure.
    pub(super) fn start(
        db: &AppDb,
        fixed: Option<Duration>,
        tx: UnboundedSender<AppMsg>,
        cx: &mut App,
    ) -> Self {
        let after = match fixed {
            Some(after) => Some(after),
            None => db.rest_after().unwrap_or_else(|err| {
                tracing::warn!(%err, "could not read the rest period");
                aeri_core::resources::DEFAULT_REST_AFTER
            }),
        };
        let every = fixed.map_or(CHECK_EVERY, |after| after.min(CHECK_EVERY));
        let tick = tx.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(every).await;
                if tick.unbounded_send(AppMsg::RestCheck).is_err() {
                    break;
                }
            }
        })
        .detach();
        mac::watch_memory_pressure(move |level| {
            let _ = tx.unbounded_send(AppMsg::MemoryPressure(level));
        });
        Self {
            after,
            fixed: fixed.is_some(),
            pressure: Pressure::Normal,
            closing: HashSet::new(),
            view: None,
            monitor: None,
        }
    }
}

/// Whether a tab left alone this long closes (Settings › Tabs › Close tabs
/// you leave alone). What stays is what the Person said to keep: a pinned
/// tab, a named one, a site kept awake; and what is in use: on screen,
/// playing, capturing, holding something typed. A blank tab holds nothing.
fn left_alone(
    item: &Item<TabEntry>,
    shown: &HashSet<TabId>,
    keep_awake: &HashSet<String>,
    now: Instant,
    wait: Duration,
) -> bool {
    let entry = &item.value;
    let url = entry.url();
    let kept = site_of(url).is_some_and(|site| keep_awake.contains(&site));
    !item.pinned
        && entry.name.is_none()
        && !shown.contains(&item.id)
        && url != NEW_TAB_URL
        && entry.internal().is_none()
        && awake_reason(&entry.model, false, kept).is_none()
        && now.duration_since(entry.last_seen) >= wait
}

/// A tab with a page: shown tabs are seen now; others may be candidates.
fn consider(
    entry: &mut TabEntry,
    shown: &HashSet<TabId>,
    keep_awake: &HashSet<String>,
    now: Instant,
    out: &mut Vec<Candidate>,
) {
    if entry.cef.is_none() {
        return;
    }
    if shown.contains(&entry.model.id) {
        entry.last_seen = now;
        return;
    }
    let kept = site_of(&entry.model.url).is_some_and(|site| keep_awake.contains(&site));
    if awake_reason(&entry.model, false, kept).is_none() {
        out.push(Candidate {
            tab: entry.model.id,
            unseen_for: now.duration_since(entry.last_seen),
        });
    }
}

impl Shell {
    /// Tabs some window shows now, and the one whose video floats: it is
    /// on screen too.
    pub(super) fn shown_tabs(&self) -> HashSet<TabId> {
        self.windows
            .iter()
            .filter(|w| !w.closing)
            .flat_map(|w| {
                w.shown()
                    .into_iter()
                    .chain(w.active_split().into_iter().flatten())
            })
            .chain(crate::mac::float::floating().map(|(tab, _)| tab))
            .collect()
    }

    /// Marks the window's shown tab as seen now. Call before it stops being
    /// shown.
    pub(super) fn note_seen(&mut self, window_id: u64) {
        let Some(shown) = self.window(window_id).and_then(|w| w.shown()) else {
            return;
        };
        if let Some(spot) = self.spot(shown)
            && let Some(item) = self.list_mut(spot).and_then(|list| list.get_mut(shown))
        {
            item.value.last_seen = Instant::now();
        }
    }

    pub(super) fn kept_awake(&self, profile: ProfileId, url: &str) -> bool {
        site_of(url).is_some_and(|site| {
            self.open
                .get(&profile)
                .is_some_and(|open| open.keep_awake.contains(&site))
        })
    }

    /// Every tab with a page that may rest, with how long it went unseen.
    fn rest_candidates(&mut self) -> Vec<Candidate> {
        let shown = self.shown_tabs();
        let now = Instant::now();
        let mut out = Vec::new();
        for w in self.windows.iter_mut().filter(|w| !w.closing) {
            let Some(open) = self.open.get(&w.profile) else {
                continue;
            };
            for item in w.tabs.iter_mut() {
                consider(&mut item.value, &shown, &open.keep_awake, now, &mut out);
            }
        }
        for open in self.open.values_mut() {
            let keep_awake = &open.keep_awake;
            let parked = open.parked.values_mut().flat_map(|p| p.tabs.iter_mut());
            for item in open.favourites.iter_mut().chain(parked) {
                consider(&mut item.value, &shown, keep_awake, now, &mut out);
            }
        }
        out
    }

    /// Each minute: tabs unseen past the period rest, and while memory stays
    /// short, more of them.
    pub(super) fn rest_check(&mut self, cx: &mut App) {
        self.close_left_alone(cx);
        let candidates = self.rest_candidates();
        let mut picks = match self.resting.after {
            // While the Mac saves power, tabs rest sooner (never later).
            Some(after) if mac::saving_power() => {
                idle_rests(&candidates, after.min(SAVING_POWER_REST_AFTER))
            }
            Some(after) => idle_rests(&candidates, after),
            None => Vec::new(),
        };
        if self.resting.pressure != Pressure::Normal {
            let critical = self.resting.pressure == Pressure::Critical;
            picks.extend(pressure_rests(&candidates, critical));
        }
        for tab in picks {
            self.rest_tab(tab, cx);
        }
    }

    pub(super) fn memory_pressure(&mut self, level: Pressure, cx: &mut App) {
        tracing::info!(?level, "memory pressure");
        self.resting.pressure = level;
        if level == Pressure::Normal {
            return;
        }
        let candidates = self.rest_candidates();
        for tab in pressure_rests(&candidates, level == Pressure::Critical) {
            self.rest_tab(tab, cx);
        }
    }

    /// Closes the tabs left alone past the Person's time, as ⌘W would: each
    /// goes to the closed stack, where ⇧⌘T finds it. A window keeps its last
    /// tab, and what is on screen stays.
    fn close_left_alone(&mut self, cx: &mut App) {
        let Some(hours) = self.prefs().tabs.close_after_hours.hours() else {
            return;
        };
        let wait = Duration::from_secs(u64::from(hours) * 3600);
        let shown = self.shown_tabs();
        let now = Instant::now();
        let mut due: Vec<(Spot, TabId)> = Vec::new();
        for w in self.windows.iter().filter(|w| !w.closing) {
            let Some(open) = self.open.get(&w.profile) else {
                continue;
            };
            let going = w
                .tabs
                .iter()
                .filter(|item| left_alone(item, &shown, &open.keep_awake, now, wait))
                .map(|item| (Spot::Window(w.id), item.id));
            due.extend(going.take(w.tabs.len().saturating_sub(1)));
        }
        for (&profile, open) in &self.open {
            for (&space, parked) in &open.parked {
                let going = parked
                    .tabs
                    .iter()
                    .filter(|item| left_alone(item, &shown, &open.keep_awake, now, wait))
                    .map(|item| (Spot::Parked(profile, space), item.id));
                due.extend(going);
            }
        }
        let mut windows = Vec::new();
        for (spot, tab) in due {
            let Some((item, _)) = self.take_tab(spot, tab) else {
                continue;
            };
            tracing::info!(%tab, "tab left alone closes");
            if let Spot::Window(id) = spot
                && !windows.contains(&id)
            {
                windows.push(id);
            }
            if let Some(page) = item.value.cef {
                self.later(move |_| page.close());
            }
        }
        for window in windows {
            self.refresh_window(window, cx);
        }
    }

    /// Releases a tab's page; it keeps its place and loads again when shown.
    /// A shown tab never rests. Returns whether it rested.
    pub(super) fn rest_tab(&mut self, tab: TabId, cx: &mut App) -> bool {
        if self.shown_tabs().contains(&tab) {
            return false;
        }
        let Some(spot) = self.spot(tab) else {
            return false;
        };
        let Some(page) = self
            .list_mut(spot)
            .and_then(|list| list.get_mut(tab))
            .and_then(|item| item.value.put_away())
        else {
            return false;
        };
        tracing::info!(%tab, "tab rests");
        self.resting.closing.insert(tab);
        self.later(move |_| page.close());
        match spot {
            Spot::Window(window_id) => self.refresh_window(window_id, cx),
            Spot::Favourite(profile) => {
                if let Some(open) = self.open.get_mut(&profile) {
                    open.favourite_home.remove(&tab);
                }
                self.refresh_profile_windows(profile, cx);
            }
            Spot::Parked(..) => {}
        }
        true
    }

    /// Keeps (or stops keeping) the site of `url` awake in `profile`.
    pub(super) fn set_keep_awake(&mut self, profile: ProfileId, url: &str, awake: bool) {
        let Some(site) = site_of(url) else {
            return;
        };
        let Some(open) = self.open.get_mut(&profile) else {
            return;
        };
        if let Some(db) = &open.db
            && let Err(err) = db.set_keep_awake(&site, awake)
        {
            tracing::warn!(%err, "could not save the keep-awake site");
        }
        tracing::info!(%site, awake, "keep awake");
        if awake {
            open.keep_awake.insert(site);
        } else {
            open.keep_awake.remove(&site);
        }
    }

    /// The Person's period for resting unseen tabs.
    pub(super) fn set_rest_after(&mut self, after: Option<Duration>) {
        if let Err(err) = self.db.set_rest_after(after) {
            tracing::warn!(%err, "could not save the rest period");
        }
        if !self.resting.fixed {
            self.resting.after = after;
        }
    }
}
