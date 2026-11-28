//! Profiles (ADR 0004): opening one with its windows, the profile menu and
//! manager, creating and deleting profiles, and the blocker offer.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use aeri_cef::ProfileContext;
use aeri_core::credentials::PasswordProvider;
use aeri_core::ids::{ProfileId, TabId};
use aeri_core::profile_db::ProfileDb;
use aeri_core::store::{ACCENTS, AppDb};
use aeri_core::workspace::TabList;
use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use super::passwords::{apply_password_provider, provider_title};
use super::tabs::{Place, TabStart};
use super::{AppMsg, NEW_TAB_URL, OpenProfile, Plan, Shell, content_view, menus};
use crate::mac::{self};
use crate::native_menu::{Align, Entry};
use crate::profile_manager::{self, ProfileManager};

/// Aeri blocks ads through uBlock Origin Lite (ADR 0008). Its store page
/// opens once per profile, at the profile's first launch, unless it is
/// already installed.
const BLOCKER_ID: &str = "ddkjiahejlhfcafbddmgiahcphecmpfh";
const BLOCKER_STORE_URL: &str =
    "https://chromewebstore.google.com/detail/ublock-origin-lite/ddkjiahejlhfcafbddmgiahcphecmpfh";

/// Removes the directories of deleted profiles. Only while CEF is down.
pub(super) fn remove_deleted_profiles(db: &AppDb, data_dir: &Path) {
    match db.remove_deleted_data(data_dir) {
        Ok(pending) => {
            for (profile, error) in pending {
                tracing::warn!(%profile, error, "deleted profile's data not removed yet");
            }
        }
        Err(err) => tracing::warn!(%err, "could not remove deleted profiles' data"),
    }
}

fn item(title: &str) -> Entry {
    Entry::Item {
        title: title.to_owned(),
        checked: false,
    }
}

impl Shell {
    /// Opens a window of `profile` (its session, or an address), waking the
    /// profile first if it is dormant (its context initializes
    /// asynchronously).
    pub(super) fn open_profile_window(&mut self, profile: ProfileId, plan: Plan, cx: &mut App) {
        if !self.engine_ready {
            self.launch.push((profile, plan));
            return;
        }
        if let Some(open) = self.open.get_mut(&profile) {
            if open.ready {
                self.open_planned(profile, plan, cx);
            } else {
                open.plans.push(plan);
            }
            return;
        }
        let tx = self.tx.clone();
        let ready = move || {
            let _ = tx.unbounded_send(AppMsg::ProfileReady { profile });
        };
        match ProfileContext::open(&self.root_cache_path, profile, ready) {
            Ok(context) => {
                tracing::info!(%profile, "opening profile");
                let path = self
                    .data_dir
                    .join("Profiles")
                    .join(profile.to_string())
                    .join("profile.db");
                let db = ProfileDb::open(&path)
                    .inspect_err(|err| {
                        tracing::error!(%err, %profile, "could not open profile.db; nothing is recorded")
                    })
                    .ok();
                let history = db.as_ref().and_then(|_| {
                    super::history_writer::HistoryWriter::spawn(&path)
                        .inspect_err(|err| {
                            tracing::error!(%err, %profile, "no history writer; history is not recorded")
                        })
                        .ok()
                });
                let keep_awake = db
                    .as_ref()
                    .and_then(|db| db.keep_awake_sites().ok())
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                let theme = db.as_ref().and_then(|db| {
                    let file = db.theme().ok().flatten();
                    super::themes::read_theme(file.as_deref(), "profile")
                });
                self.open.insert(
                    profile,
                    OpenProfile {
                        context,
                        db,
                        history,
                        ready: false,
                        plans: vec![plan],
                        extensions: Vec::new(),
                        _extensions_watch: None,
                        opened_at: Instant::now(),
                        spaces: Vec::new(),
                        parked: HashMap::new(),
                        favourites: TabList::default(),
                        favourite_home: HashMap::new(),
                        downloads: Vec::new(),
                        keep_awake,
                        theme,
                        icons: super::site_icons::SiteIcons::new(
                            self.data_dir
                                .join("Profiles")
                                .join(profile.to_string())
                                .join("icons"),
                        ),
                    },
                );
            }
            Err(err) => tracing::error!(%err, %profile, "could not open the profile"),
        }
    }

    /// Opens a ready profile's window(s) for `plan`.
    fn open_planned(&mut self, profile: ProfileId, plan: Plan, cx: &mut App) {
        match plan {
            Plan::Url(url) => self.open_url_window(profile, url, cx),
            Plan::Restore => self.restore_windows(profile, cx),
            Plan::Little(url) => self.open_little_now(profile, url, cx),
            Plan::Fresh => self.fresh_windows(profile, cx),
        }
    }

    pub(super) fn profile_ready(&mut self, profile: ProfileId, cx: &mut App) {
        let tx = self.tx.clone();
        let Some(open) = self.open.get_mut(&profile) else {
            return;
        };
        open.ready = true;
        open._extensions_watch = open.context.watch_extensions(move || {
            let _ = tx.unbounded_send(AppMsg::ExtensionsChanged { profile });
        });
        open.extensions = open.context.extensions();
        apply_password_provider(profile, open);
        self.apply_profile_privacy(profile);
        self.apply_profile_links(profile);
        self.apply_profile_browsing(profile);
        self.apply_profile_zoom(profile);
        self.load_workspace(profile);
        self.adopt_spaces_default(profile, cx);
        let Some(open) = self.open.get_mut(&profile) else {
            return;
        };
        tracing::info!(
            %profile,
            ms = open.opened_at.elapsed().as_millis() as u64,
            "profile ready"
        );
        for plan in std::mem::take(&mut open.plans) {
            self.open_planned(profile, plan, cx);
        }
        self.offer_blocker(profile, cx);
    }

    pub(super) fn extensions_changed(&mut self, profile: ProfileId, cx: &mut App) {
        let Some(open) = self.open.get_mut(&profile) else {
            return;
        };
        let extensions = open.context.extensions();
        if extensions != open.extensions {
            // The foot's Extensions door lists them when it is pressed, and
            // shows only while there are some.
            tracing::info!(%profile, count = extensions.len(), "extensions changed");
            open.extensions = extensions;
            self.refresh_profile_windows(profile, cx);
        }
    }

    /// Offers uBlock Origin Lite in a new tab, once per profile, if ad
    /// blocking is on and it is not installed. Never replace Home on launch.
    pub(super) fn offer_blocker(&mut self, profile: ProfileId, cx: &mut App) {
        if !self.prefs.privacy.block_ads {
            return;
        }
        let installed = self
            .open
            .get(&profile)
            .is_some_and(|open| open.extensions.iter().any(|e| e.id == BLOCKER_ID));
        if installed {
            return;
        }
        let Some(window) = self.windows.iter().rfind(|w| w.profile == profile) else {
            return;
        };
        let window_id = window.id;
        let foreground = self
            .shown_model(window)
            .is_none_or(|tab| tab.url != NEW_TAB_URL);
        match self.db.take_blocker_offer(profile) {
            Ok(true) => {}
            Ok(false) => return,
            Err(err) => {
                tracing::warn!(%err, "could not record the ad-blocker offer");
                return;
            }
        }
        tracing::info!(%profile, "offering uBlock Origin Lite");
        let start = TabStart::Load(BLOCKER_STORE_URL.to_owned());
        self.add_tab(window_id, TabId::next(), start, Place::End, foreground, cx);
    }

    pub(super) fn mark_used(&self, profile: ProfileId) {
        if let Err(err) = self.db.mark_used(profile) {
            tracing::warn!(%err, %profile, "could not record the profile's use");
        }
    }

    /// Switches to `profile`: its newest window comes forward, or a window
    /// opens (stage 1, flow 8).
    pub(super) fn show_profile(&mut self, profile: ProfileId, cx: &mut App) {
        if !self.profiles.iter().any(|p| p.id == profile) {
            return;
        }
        let existing = self
            .windows
            .iter()
            .rfind(|w| w.profile == profile && !w.closing)
            .map(|w| w.frame.clone());
        match existing {
            Some(frame) => {
                self.mark_used(profile);
                cx.activate(true);
                self.later(move |_| frame.bring_forward());
            }
            None => self.open_profile_window(profile, Plan::Restore, cx),
        }
    }

    /// The native profile menu under the strip's badge: profiles, then the
    /// window's profile's password provider. The menu is modal: while it is
    /// open the shell handles no messages (they queue), the same as during
    /// any AppKit menu tracking.
    pub(super) fn profile_menu(&mut self, window_id: u64, x: f32, y: f32, align: Align) {
        let Some(w) = self.window(window_id) else {
            return;
        };
        let profile = w.profile;
        let provider = self.password_provider(profile);
        let mut rows: Vec<(Entry, Option<AppMsg>)> = Vec::new();
        for p in &self.profiles {
            let entry = Entry::Item {
                title: p.name.clone(),
                checked: p.id == profile,
            };
            rows.push((entry, Some(AppMsg::OpenProfile { profile: p.id })));
        }
        rows.push((Entry::Separator, None));
        rows.push((
            item("New Profile…"),
            Some(AppMsg::ShowProfiles { new: true }),
        ));
        rows.push((
            item("Manage Profiles…"),
            Some(AppMsg::ShowProfiles { new: false }),
        ));
        rows.push((Entry::Separator, None));
        for choice in PasswordProvider::ALL {
            let entry = Entry::Item {
                title: provider_title(choice).to_owned(),
                checked: choice == provider,
            };
            let msg = AppMsg::SetPasswordProvider {
                profile,
                provider: choice,
            };
            rows.push((entry, Some(msg)));
        }
        // Shown for every provider: sign-ins saved earlier stay offered on
        // a field the Person picks until deleted there (Observed).
        let msg = AppMsg::ManagePasswords { window: window_id };
        rows.push((item("Manage Passwords…"), Some(msg)));
        let (entries, msgs): (Vec<Entry>, Vec<Option<AppMsg>>) = rows.into_iter().unzip();
        let msgs = std::cell::RefCell::new(msgs);
        let tx = self.tx.clone();
        let frame = w.frame.clone();
        self.later(move |_| {
            let (x, y) = (f64::from(x), f64::from(y));
            frame.pop_up_menu(x, y, align, &entries, move |ix| {
                if let Some(msg) = msgs.borrow_mut().get_mut(ix).and_then(Option::take) {
                    let _ = tx.unbounded_send(msg);
                }
            });
        });
    }

    pub(super) fn show_profile_manager(&mut self, new: bool, cx: &mut App) {
        if let Some(manager) = &self.manager {
            let shown = manager.update(cx, |manager, window, cx| {
                if new {
                    manager.focus_new(window, cx);
                }
                content_view(window)
            });
            if let Ok(view) = shown {
                cx.activate(true);
                if let Some(view) = view {
                    self.later(move |_| mac::bring_to_front(view));
                }
                return;
            }
        }
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Profiles".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(600.), px(560.)),
                cx,
            ))),
            window_min_size: Some(size(px(480.), px(360.))),
            ..Default::default()
        };
        let listed = self.listed();
        let suggested = self.db.suggest_accent().unwrap_or(ACCENTS[0]);
        let tx = self.tx.clone();
        match cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let mut manager = ProfileManager::new(listed, suggested, tx, window, cx);
                if new {
                    manager.focus_new(window, cx);
                }
                manager
            })
        }) {
            Ok(handle) => self.manager = Some(handle),
            Err(err) => tracing::error!(%err, "could not open the profile manager"),
        }
    }

    fn listed(&self) -> Vec<profile_manager::Listed> {
        profile_manager::listed(&self.profiles, |id| {
            self.windows.iter().any(|w| w.profile == id)
        })
    }

    /// Re-reads the profile list and shows it everywhere: menus, strips,
    /// window titles, the manager (stage 1: a rename shows at once).
    pub(super) fn refresh_profiles(&mut self, cx: &mut App) {
        match self.db.profiles() {
            Ok(profiles) => self.profiles = profiles,
            Err(err) => tracing::warn!(%err, "could not read profiles"),
        }
        cx.set_menus(menus(&self.profiles));
        for w in &self.windows {
            if let Some(profile) = self.profiles.iter().find(|p| p.id == w.profile) {
                let profile = profile.clone();
                let _ = w.frame_window.update(cx, |frame, window, cx| {
                    window.set_window_title(&profile.name);
                    frame.set_profile(profile, cx);
                });
            }
        }
        for w in &self.windows {
            self.refresh_window(w.id, cx);
        }
        self.refresh_manager(cx);
    }

    pub(super) fn refresh_manager(&mut self, cx: &mut App) {
        let Some(manager) = self.manager else {
            return;
        };
        let listed = self.listed();
        let suggested = self.db.suggest_accent().unwrap_or(ACCENTS[0]);
        let updated = manager.update(cx, |manager, window, cx| {
            manager.set_profiles(listed, suggested, window, cx)
        });
        if updated.is_err() {
            self.manager = None;
        }
    }

    /// Creates a profile and opens its first window (stage 1, flow 7).
    pub(super) fn create_profile(&mut self, name: &str, accent: u32, cx: &mut App) {
        match self.db.create_profile(name, accent) {
            Ok(profile) => {
                tracing::info!(profile = %profile.id, "profile created");
                self.refresh_profiles(cx);
                self.open_profile_window(profile.id, Plan::Restore, cx);
            }
            Err(err) => tracing::warn!(%err, "could not create the profile"),
        }
    }

    /// Deletes a profile the Person confirmed: it leaves the list at once,
    /// its windows close, and its data is removed once CEF has shut down
    /// (at quit, or at the next launch).
    pub(super) fn delete_profile(&mut self, profile: ProfileId, cx: &mut App) {
        if let Err(err) = self.db.delete_profile(profile) {
            tracing::warn!(%err, %profile, "could not delete the profile");
            return;
        }
        tracing::info!(%profile, "profile deleted; its data is removed when CEF is down");
        self.refresh_profiles(cx);
        let windows: Vec<u64> = self
            .windows
            .iter()
            .filter(|w| w.profile == profile)
            .map(|w| w.id)
            .collect();
        let others = self.windows.iter().any(|w| w.profile != profile);
        if windows.is_empty() {
            // Still waking up, or already dormant.
            let released = self.open.remove(&profile);
            self.later(move |_| drop(released));
        }
        for window in windows {
            self.close_window(window, cx);
        }
        if !others {
            match self.db.launch_profile() {
                Ok(next) => self.open_profile_window(next.id, Plan::Restore, cx),
                Err(err) => tracing::error!(%err, "no profile to open"),
            }
        }
    }

    /// Runs an extension's action on the window's active tab, as Chrome's
    /// toolbar button would; its popup opens at the page's top right.
    pub(super) fn extension_action(&mut self, window_id: u64, extension: String, cx: &mut App) {
        self.hide_bar(window_id, cx);
        let (Some(devtools), Some(page)) = (self.devtools.clone(), self.shown_cef(window_id))
        else {
            tracing::warn!(extension, "extension actions are unavailable");
            return;
        };
        self.later(move |_| page.open_extension_action(&extension, &devtools));
    }

    /// The native menu of the profile's extensions at (`x`, `y`): picking
    /// one runs its action on the shown tab, as the toolbar button would.
    pub(super) fn extensions_menu(&mut self, window_id: u64, x: f32, y: f32, align: Align) {
        let Some(w) = self.window(window_id) else {
            return;
        };
        let Some(open) = self.open.get(&w.profile) else {
            return;
        };
        let mut entries = Vec::new();
        let mut picks = Vec::new();
        for extension in &open.extensions {
            entries.push(item(&extension.name));
            picks.push(Some(AppMsg::ExtensionAction {
                window: window_id,
                extension: extension.id.clone(),
            }));
        }
        if entries.is_empty() {
            entries.push(item("No Extensions"));
            picks.push(None);
        }
        let tx = self.tx.clone();
        let frame = w.frame.clone();
        let picks = std::cell::RefCell::new(picks);
        self.later(move |_| {
            let (x, y) = (f64::from(x), f64::from(y));
            frame.pop_up_menu(x, y, align, &entries, move |ix| {
                if let Some(msg) = picks.borrow_mut().get_mut(ix).and_then(Option::take) {
                    let _ = tx.unbounded_send(msg);
                }
            });
        });
    }
}
