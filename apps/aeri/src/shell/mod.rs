//! The application shell: owns windows and tabs, routes engine signals to
//! the model and the GPUI interface, and turns the Person's input into
//! engine commands.
//!
//! All state lives in one [`Shell`] driven by a single message loop on the
//! main thread. CEF callbacks and GPUI views only enqueue [`AppMsg`]s.
//!
//! Re-entrancy rule: AppKit and CEF calls can synchronously fire GPUI window
//! callbacks (resize, key status), which need the `App` borrow. So while
//! handling a message inside `cx.update`, the shell only touches GPUI and its
//! own model; every AppKit/CEF side effect is queued with [`Shell::later`]
//! and runs after the borrow is released.
//!
//! Each tab is its own CEF window (one Chrome-style browser per window);
//! [`Frame`](crate::mac::Frame) stacks a window's tabs under one GPUI frame
//! window. One child module per concern: [`windows`], [`tabs`] and the
//! [`tab_menu`], [`spaces`] and [`favourites`], the [`bar`] and its
//! [`palette`], the [`library`], [`profiles`] and their [`passwords`],
//! [`trust`] (popups, downloads, clearing), [`resting`] tabs and the
//! [`resources`] view, [`shortcuts`], the [`session`] records and [`quit`].
//! The messages are in [`msg`].
//!
//! Profiles (ADR 0004): every window belongs to one profile. A profile with
//! a window is open and holds its request context; when its last window
//! goes, it turns dormant and the context is released. Aeri keeps running
//! with no windows, like other Mac browsers.

mod about;
mod appearance;
mod bar;
mod bookmarks_bar;
mod browsing;
mod chrome;
mod favourites;
mod find;
mod float;
mod float_js;
mod general_prefs;
mod internal;
mod launcher;
mod library;
mod links;
mod little;
mod little_life;
mod menu_bar;
mod msg;
mod pages;
mod palette;
mod passwords;
mod peek;
mod prefs;
mod privacy;
mod profiles;
mod proxy;
mod quit;
mod reader;
mod refresh;
mod resources;
mod resting;
mod session;
mod settings;
pub mod settings_index;
mod settings_rows;
mod shortcuts;
mod site_icons;
mod site_styles;
mod space_prefs;
mod space_settings;
mod spaces;
mod split;
mod suggest;
mod switcher;
mod tab_events;
mod tab_look;
mod tab_menu;
mod tab_open;
mod tab_signals;
mod tabs;
mod theme_files;
mod themes;
mod trust;
mod welcome;
mod windows;
mod zoom;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use aeri_cef::{
    DevTools, Extension, ExtensionsWatch, ProfileContext, Runtime, RuntimeConfig, Signal,
};
use aeri_core::ids::{ProfileId, SpaceId, TabId};
use aeri_core::profile_db::ProfileDb;
use aeri_core::store::{AppDb, Profile};
use aeri_core::workspace::TabList;
use anyhow::{Context as _, Result};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{App, WindowHandle};

pub use self::bar::{BarRow, Pick};
pub use self::bookmarks_bar::BarMsg;
pub use self::find::FindMsg;
pub use self::float::FloatMsg;
pub use self::links::open_urls;
use self::menu_bar::{install_actions, menus};
pub use self::msg::{AppMsg, ChromeMsg, Nav, TabCommand, ThemeEditorMsg};
pub use self::peek::PeekMsg;
use self::profiles::remove_deleted_profiles;
use self::quit::quit_on_signals;
use self::resting::Resting;
pub use self::settings::{KeyRow, SettingsModel, SettingsMsg};
pub use self::settings_rows::{ArcState, RowsMsg};
pub use self::spaces::SPACE_ICONS;
use self::spaces::{Parked, SpaceInfo};
pub use self::switcher::SwitcherMsg;
pub use self::tab_signals::{PlayerAct, TabSignal};
use self::tabs::TabEntry;
use self::trust::DownloadEntry;
use self::windows::AeriWindow;
use crate::library::Library;
use crate::mac;
use crate::profile_manager::ProfileManager;
use crate::space_editor::SpaceEditor;

const NEW_TAB_URL: &str = "about:blank";

pub struct Config {
    pub data_dir: PathBuf,
    /// Development and tests: open (creating if needed) the profiles with
    /// these names, each in a window, instead of the last-used profile.
    pub profiles: Vec<String>,
    /// Development and tests: open one window on this address instead of
    /// restoring each profile's session.
    pub start_url: Option<String>,
    pub remote_debugging_port: Option<u16>,
    /// Where downloads are saved.
    pub download_dir: PathBuf,
    /// Development and tests: rest tabs unseen this long, checking as
    /// often, instead of the Person's period.
    pub rest_after: Option<std::time::Duration>,
}

type Effect = Box<dyn FnOnce(&mut Shell)>;

/// What to open once a profile is ready.
enum Plan {
    /// Its saved windows (one window on its first space if none).
    Restore,
    /// Its windows as a fresh start leaves them: pins, favourites and named
    /// tabs stay, the other tabs do not come back (Start with a fresh
    /// window).
    Fresh,
    /// A window on this address: a new tab in the first space no window
    /// shows.
    Url(String),
    /// A small window on this address (a link from another app that
    /// woke its profile).
    Little(String),
}

/// A profile that has (or is about to have) a window.
struct OpenProfile {
    context: ProfileContext,
    /// `Profiles/<id>/profile.db`: session, closed tabs, history. Absent if
    /// it could not be opened (logged); the profile still browses.
    db: Option<ProfileDb>,
    /// Browsers can be created only once the context has initialized.
    ready: bool,
    /// Windows to open once ready.
    plans: Vec<Plan>,
    extensions: Vec<Extension>,
    _extensions_watch: Option<ExtensionsWatch>,
    opened_at: Instant,
    /// The profile's spaces, in the Person's order.
    spaces: Vec<SpaceInfo>,
    /// The tabs of spaces no window shows.
    parked: HashMap<SpaceId, Parked>,
    /// Shown at the top of every space; one page each.
    favourites: TabList<TabEntry>,
    /// The window whose frame holds each live favourite's page.
    favourite_home: HashMap<TabId, u64>,
    /// This session's downloads, oldest first.
    downloads: Vec<DownloadEntry>,
    /// Sites (`trust::site_of`) whose tabs never rest.
    keep_awake: HashSet<String>,
    /// The profile's own theme, over the app's.
    theme: Option<aeri_core::appearance::Recipe>,
    /// Site icons kept on disk, by host.
    icons: site_icons::SiteIcons,
}

struct Shell {
    db: AppDb,
    data_dir: PathBuf,
    root_cache_path: PathBuf,
    /// Every profile, in the Person's order (mirrors `app.db`).
    profiles: Vec<Profile>,
    open: HashMap<ProfileId, OpenProfile>,
    engine_ready: bool,
    /// Profiles to open once the engine is ready and setup is dismissed.
    launch: Vec<(ProfileId, Plan)>,
    welcome: welcome::Onboarding,
    devtools: Option<DevTools>,
    /// Profiles whose session changed while handling the current message.
    dirty: HashSet<ProfileId>,
    windows: Vec<AeriWindow>,
    /// Small windows (`little`).
    little: little::Littles,
    next_window: u64,
    manager: Option<WindowHandle<ProfileManager>>,
    space_editor: Option<WindowHandle<SpaceEditor>>,
    theme_editor: Option<themes::Editing>,
    tab_editor: Option<WindowHandle<crate::tab_editor::TabEditor>>,
    /// Windows drawn see-through over the desktop's blur (the macOS
    /// material).
    see_through: HashSet<u64>,
    /// The Person's own themes, by name (`<data>/Themes`).
    user_themes: Vec<aeri_core::appearance::library::Saved>,
    /// The library window and the profile it shows.
    library: Option<(WindowHandle<Library>, ProfileId)>,
    /// Resting tabs and the resource view.
    resting: Resting,
    /// Where the chrome sits (Phase 12).
    chrome: chrome::Chrome,
    /// App-wide preferences (`prefs.rs`).
    prefs: aeri_core::prefs::AppPrefs,
    /// Where downloads go when no folder is chosen.
    #[allow(dead_code, reason = "read by the preference effects")]
    default_download_dir: PathBuf,
    /// Every command's keys (Phase 12).
    keymap: aeri_core::keymap::Keymap,
    settings: settings::Settings,
    tx: UnboundedSender<AppMsg>,
    quitting: bool,
    effects: RefCell<Vec<Effect>>,
    /// The floating video's waits (`float.rs`).
    float: float::FloatState,
}

thread_local! {
    /// Where [`reopen`] sends, once the shell runs.
    static SHELL_TX: RefCell<Option<UnboundedSender<AppMsg>>> = RefCell::default();
}

/// The Dock icon was clicked: with no window open, open one.
pub fn reopen() {
    SHELL_TX.with_borrow(|tx| {
        if let Some(tx) = tx {
            let _ = tx.unbounded_send(AppMsg::Reopen);
        }
    });
}

/// Starts CEF and the shell. Call inside GPUI's `run` callback.
pub fn start(runtime: Rc<Runtime>, config: Config, cx: &mut App) -> Result<()> {
    std::fs::create_dir_all(&config.data_dir).context("creating the data directory")?;
    let app_db = config.data_dir.join("app.db");
    let existing_install = app_db.exists();
    let db = AppDb::open(&app_db).context("opening app.db")?;
    let prefs = prefs::load(&db, existing_install);
    let welcome = welcome::Onboarding::load(
        &db,
        existing_install,
        config.start_url.is_none() && config.profiles.is_empty(),
    );
    // Chromium is not running yet, so deleted profiles can go now.
    remove_deleted_profiles(&db, &config.data_dir);
    // An explicit start address opens one window on it; otherwise each
    // profile restores its session.
    let fresh = prefs.spaces.start_fresh;
    let plan = || match &config.start_url {
        Some(url) => Plan::Url(url.clone()),
        None if fresh => Plan::Fresh,
        None => Plan::Restore,
    };
    let mut launch = Vec::new();
    for name in &config.profiles {
        let existing = db.profiles()?.into_iter().find(|p| &p.name == name);
        let profile = match existing {
            Some(profile) => profile,
            None => db.create_profile(name, db.suggest_accent()?)?,
        };
        launch.push((profile.id, plan()));
    }
    if launch.is_empty() {
        launch.push((db.launch_profile()?.id, plan()));
    }
    let root_cache_path = config.data_dir.join("Chromium");

    let (tx, mut rx) = unbounded::<AppMsg>();
    SHELL_TX.with_borrow_mut(|slot| *slot = Some(tx.clone()));
    // Before any window opens: every view paints from the theme.
    appearance::start(&db, tx.clone(), cx);
    launcher::startup_switches(&prefs);
    browsing::start(&prefs.browsing);
    little::start(&prefs);
    let runtime_config = RuntimeConfig {
        root_cache_path: root_cache_path.clone(),
        log_file: Some(config.data_dir.join("chromium.log")),
        remote_debugging_port: config.remote_debugging_port,
        download_dir: prefs::download_dir(&prefs, &config.download_dir),
    };
    let ready = tx.clone();
    let signals = tx.clone();
    let sink: aeri_cef::SignalSink = Arc::new(move |signal| {
        let _ = signals.unbounded_send(AppMsg::Signal(signal));
    });
    runtime
        .start(&runtime_config, sink, move || {
            let _ = ready.unbounded_send(AppMsg::EngineReady);
        })
        .context("starting CEF")?;
    aeri_cef::set_download_ask(prefs.general.downloads_ask);
    links::start(&prefs, &tx);

    let mut keymap = settings::load_keymap(&db);
    keymap.set_space_keys(prefs.spaces.keys());
    settings::install_keymap(&keymap, cx);
    install_actions(&tx, cx);
    let quit = tx.clone();
    mac::intercept_termination(move || {
        let _ = quit.unbounded_send(AppMsg::Quit);
    });
    let command = tx.clone();
    mac::watch_command(move |held| {
        let _ = command.unbounded_send(AppMsg::CommandHeld(held));
    });
    quit_on_signals(tx.clone());
    launcher::start(&tx);
    float::start(&prefs, &tx);
    switcher::start(&tx);
    let resting = Resting::start(&db, config.rest_after, tx.clone(), cx);
    let chrome = chrome::Chrome::load(&db);

    let mut shell = Shell {
        profiles: db.profiles()?,
        db,
        data_dir: config.data_dir,
        root_cache_path,
        open: HashMap::new(),
        engine_ready: false,
        launch,
        welcome,
        devtools: runtime.devtools().ok().flatten().cloned(),
        dirty: HashSet::new(),
        windows: Vec::new(),
        little: little::Littles::default(),
        next_window: 1,
        manager: None,
        space_editor: None,
        theme_editor: None,
        tab_editor: None,
        see_through: HashSet::new(),
        user_themes: Vec::new(),
        library: None,
        resting,
        chrome,
        prefs,
        default_download_dir: config.download_dir,
        keymap,
        settings: settings::Settings::default(),
        tx,
        quitting: false,
        effects: RefCell::default(),
        float: float::FloatState::default(),
    };
    cx.set_menus(menus(&shell.profiles));
    shell.load_user_themes();
    cx.spawn(async move |cx| {
        while let Some(msg) = rx.next().await {
            let done = cx.update(|cx| shell.handle(msg, cx));
            shell.run_effects();
            // Views of internal tabs follow the tabs the message left.
            cx.update(|cx| shell.sync_internal_views(cx));
            shell.run_effects();
            cx.update(|cx| shell.flush_sessions(cx));
            if done {
                tracing::info!("all windows closed; shutting CEF down");
                shell.open.clear();
                runtime.shutdown();
                remove_deleted_profiles(&shell.db, &shell.data_dir);
                mac::allow_termination();
                cx.update(|cx| cx.quit());
                break;
            }
        }
    })
    .detach();
    Ok(())
}

impl Shell {
    /// Queues an AppKit/CEF side effect to run outside the `App` borrow.
    fn later(&self, effect: impl FnOnce(&mut Shell) + 'static) {
        self.effects.borrow_mut().push(Box::new(effect));
    }

    /// Runs the queued effects, and those they queue, outside the `App`
    /// borrow.
    fn run_effects(&mut self) {
        loop {
            let effects = self.effects.take();
            if effects.is_empty() {
                break;
            }
            for effect in effects {
                effect(self);
            }
        }
    }

    /// Returns true once quitting and every window has closed.
    fn handle(&mut self, msg: AppMsg, cx: &mut App) -> bool {
        let changes_settings = self.windows.iter().any(|w| w.settings.is_some())
            && matches!(
                msg,
                AppMsg::BarPick { .. }
                    | AppMsg::Chrome(_)
                    | AppMsg::ThemeEditor(_)
                    | AppMsg::UseTheme { .. }
                    | AppMsg::ImportTheme { .. }
                    | AppMsg::ExportTheme { .. }
                    | AppMsg::SetPasswordProvider { .. }
                    | AppMsg::SystemAppearanceChanged
            );
        match msg {
            AppMsg::EngineReady => self.engine_ready(cx),
            AppMsg::ProfileReady { profile } => self.profile_ready(profile, cx),
            AppMsg::Signal(Signal::Engine(event)) => self.engine_event(event, cx),
            AppMsg::Signal(Signal::Shortcut { tab, shortcut }) => {
                if let Some(window) = self.window_of_tab(self.peek_opener(tab)) {
                    self.shortcut(window, shortcut, cx);
                } else {
                    self.little_shortcut(tab, shortcut, cx);
                }
            }
            AppMsg::Signal(Signal::TabOpened {
                tab,
                opener,
                foreground,
            }) => self.tab_opened(tab, self.peek_opener(opener), foreground, cx),
            AppMsg::Signal(Signal::PopupWindow {
                tab,
                opener,
                width,
                height,
            }) => self.popup_window(tab, opener, width, height, cx),
            AppMsg::Signal(Signal::OpenUrl { profile, url }) => self.open_url(profile, url, cx),
            AppMsg::Signal(Signal::OpenLink {
                opener,
                url,
                foreground,
            }) => self.open_link(self.peek_opener(opener), url, foreground, cx),
            AppMsg::Signal(Signal::PopupBlocked { opener, url }) => {
                self.popup_blocked(self.peek_opener(opener), url, cx)
            }
            AppMsg::Signal(Signal::Download(download)) => self.download_updated(download, cx),
            AppMsg::Signal(Signal::Find {
                tab,
                count,
                ordinal,
                done,
            }) => self.find_result(tab, count, ordinal, done, cx),
            AppMsg::Signal(Signal::Peek(signal)) => self.peek_signal(signal, cx),
            AppMsg::Signal(Signal::ReaderLook { look, .. }) => self.reader_look_chosen(look, cx),
            AppMsg::Reader(msg) => self.reader_msg(msg, cx),
            AppMsg::TabClosed { window, tab } => {
                if !self.little_tab_closed(window, cx) {
                    self.tab_closed(window, tab, cx);
                }
            }
            AppMsg::CloseWindow { window } => self.close_window(window, cx),
            AppMsg::BarQuery { window, text } => self.bar_query(window, text, cx),
            AppMsg::BarSuggestions {
                window,
                text,
                words,
            } => self.bar_suggested(window, text, words, cx),
            AppMsg::BarPick { window, pick } => self.bar_pick(window, pick, cx),
            AppMsg::Library(msg) => self.library_msg(msg, cx),
            AppMsg::AddressDismissed { window, session } => self.dismiss_bar(window, session, cx),
            AppMsg::Shortcut { window, shortcut } => self.shortcut(window, shortcut, cx),
            AppMsg::NewHome { window } => self.add_new_tab(window, true, cx),
            AppMsg::HomeSubmit { window, tab, text } => {
                if self
                    .window(window)
                    .is_some_and(|w| w.frame.home_tab() == Some(tab))
                {
                    self.go(window, &text, false, cx);
                }
            }
            AppMsg::Tab {
                window,
                tab,
                command,
            } => self.tab_command(window, tab, command, cx),
            AppMsg::TabMenu { window, tab, x, y } => self.tab_menu(window, tab, x, y),
            AppMsg::BookmarksBar(msg) => self.bookmarks_bar_msg(msg, cx),
            AppMsg::OpenInProfile { profile, url } => self.open_in_profile(profile, url, cx),
            AppMsg::ExtensionsChanged { profile } => self.extensions_changed(profile, cx),
            AppMsg::Nav { window, nav } => self.nav(window, nav),
            AppMsg::ExtensionAction { window, extension } => {
                self.extension_action(window, extension, cx)
            }
            AppMsg::ProfileMenu {
                window,
                x,
                y,
                align,
            } => self.profile_menu(window, x, y, align),
            AppMsg::SetPasswordProvider { profile, provider } => {
                self.set_password_provider(profile, provider);
            }
            AppMsg::ManagePasswords { window } => {
                self.open_page(window, self::passwords::PASSWORDS_PAGE, cx);
            }
            AppMsg::PopupMenu {
                window,
                x,
                y,
                align,
            } => self.popup_menu(window, x, y, align),
            AppMsg::DownloadsMenu {
                window,
                x,
                y,
                align,
            } => self.downloads_menu(window, x, y, align),
            AppMsg::ExtensionsMenu {
                window,
                x,
                y,
                align,
            } => self.extensions_menu(window, x, y, align),
            AppMsg::DownloadPick { window, pick } => self.download_pick(window, pick, cx),
            AppMsg::OpenProfile { profile } => self.show_profile(profile, cx),
            AppMsg::CreateProfile { name, accent } => self.create_profile(&name, accent, cx),
            AppMsg::RenameProfile { profile, name } => {
                if let Err(err) = self.db.rename_profile(profile, &name) {
                    tracing::warn!(%err, %profile, "could not rename the profile");
                }
                self.refresh_profiles(cx);
            }
            AppMsg::SetAccent { profile, accent } => {
                if let Err(err) = self.db.set_accent(profile, accent) {
                    tracing::warn!(%err, %profile, "could not recolour the profile");
                }
                self.refresh_profiles(cx);
            }
            AppMsg::DeleteProfile { profile } => self.delete_profile(profile, cx),
            AppMsg::ShowProfiles { new } => self.show_profile_manager(new, cx),
            AppMsg::ProfileManagerClosed => self.manager = None,
            AppMsg::SpaceMenu {
                window,
                x,
                y,
                align,
            } => self.space_menu(window, x, y, align, cx),
            AppMsg::SwitchSpace { window, space } => self.switch_space(window, space, cx),
            AppMsg::EditSpace { window, space } => self.show_space_editor(window, space, cx),
            AppMsg::SaveSpace {
                profile,
                space,
                name,
                colour,
                icon,
                window,
            } => self.save_space(profile, space, name, colour, icon, window, cx),
            AppMsg::DeleteSpace {
                window,
                space,
                confirmed,
            } => self.delete_space(window, space, confirmed, cx),
            AppMsg::SpaceEditorClosed => self.space_editor = None,
            AppMsg::SpaceLook(msg) => self.space_look_msg(msg, cx),
            AppMsg::SaveSiteStyle { profile, host, css } => {
                self.save_site_style(profile, &host, &css);
            }
            AppMsg::SetTabLook { tab, name, icon } => self.set_tab_look(tab, name, icon, cx),
            AppMsg::TabEditorClosed => self.tab_editor = None,
            AppMsg::WindowKey { window } => self.window_key(window, cx),
            AppMsg::SplitFocus { window, tab } => self.focus_split_pane(window, tab, cx),
            AppMsg::OpenExternal(urls) => {
                if !urls.is_empty() {
                    self.set_welcome_aside(cx);
                }
                self.open_external(urls, cx);
            }
            AppMsg::DefaultsChanged => self.refresh_settings(cx),
            AppMsg::ProxyLogin => self.proxy_login_prompt(),
            AppMsg::ProxyUser(name) => self.set_proxy_user(name, cx),
            AppMsg::Reopen => {
                if self.welcome.window.is_some() {
                    self.show_welcome(cx);
                } else if self.windows.is_empty() && self.launch.is_empty() && !self.quitting {
                    match self.db.launch_profile() {
                        Ok(profile) => self.open_profile_window(profile.id, Plan::Restore, cx),
                        Err(err) => tracing::error!(%err, "no profile to open"),
                    }
                }
            }
            AppMsg::RestCheck => self.rest_check(cx),
            AppMsg::TabSignal(signal) => self.tab_signal(signal, cx),
            AppMsg::MemoryPressure(level) => self.memory_pressure(level, cx),
            AppMsg::Resources(msg) => self.resources_msg(msg, cx),
            AppMsg::SystemAppearanceChanged => self.system_appearance_changed(cx),
            AppMsg::CommandHeld(held) => self.command_held(held, cx),
            AppMsg::Launcher(msg) => self.launcher_msg(msg, cx),
            AppMsg::Little(msg) => self.little_msg(msg, cx),
            AppMsg::Find(msg) => self.find_msg(msg, cx),
            AppMsg::Float(msg) => self.float_msg(msg, cx),
            AppMsg::Peek(msg) => self.peek_msg(msg, cx),
            AppMsg::Switcher(msg) => self.switcher_msg(msg, cx),
            AppMsg::Chrome(msg) => self.chrome_msg(msg, cx),
            AppMsg::ThemeEditor(msg) => self.theme_editor_msg(msg, cx),
            AppMsg::ImportTheme { window, path } => self.import_theme(window, &path, cx),
            AppMsg::ExportTheme { window, path } => self.export_theme(window, &path, cx),
            AppMsg::UseTheme {
                window,
                target,
                recipe,
            } => self.set_target_theme(window, target, recipe, cx),
            AppMsg::Settings { window, msg } => self.settings_msg(window, msg, cx),
            AppMsg::Menu(command) => self.menu_command(command, cx),
            AppMsg::ShowWelcome => {
                self.show_welcome(cx);
            }
            AppMsg::WelcomeFilmFinished(id, run) => self.welcome_film_finished(id, run, cx),
            AppMsg::FinishWelcome(choices) => self.finish_welcome(choices, cx),
            AppMsg::Quit => self.request_quit(cx),
            AppMsg::QuitNow => self.quit(cx),
            AppMsg::SetDownloadsDir(dir) => {
                self.change_prefs(|p| p.general.downloads_dir = Some(dir), cx);
            }
        }
        // Settings shows what the palette, the theme editor and the system
        // can also change.
        if changes_settings {
            self.refresh_settings(cx);
        }
        // Wash rasters the views let go of while drawing.
        for image in crate::space_look::paint::take_evicted() {
            cx.drop_image(image, None);
        }
        self.quitting && self.windows.is_empty() && self.littles_gone()
    }

    fn engine_ready(&mut self, cx: &mut App) {
        self.engine_ready = true;
        if self.welcome.first_launch && self.show_welcome(cx) {
            return;
        }
        self.open_launch_windows(cx);
    }

    fn open_launch_windows(&mut self, cx: &mut App) {
        for (profile, plan) in std::mem::take(&mut self.launch) {
            self.open_profile_window(profile, plan, cx);
        }
    }

    fn window(&self, id: u64) -> Option<&AeriWindow> {
        self.windows.iter().find(|w| w.id == id)
    }

    fn window_mut(&mut self, id: u64) -> Option<&mut AeriWindow> {
        self.windows.iter_mut().find(|w| w.id == id)
    }
}

/// The GPUI window's content `NSView`.
pub(crate) fn content_view(window: &gpui::Window) -> Option<*mut std::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr()),
        _ => None,
    }
}
