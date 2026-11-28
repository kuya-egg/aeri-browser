//! What the shell's message loop handles: [`AppMsg`], sent by CEF
//! callbacks, GPUI views and native menus.

use crate::welcome::Choices;
use aeri_cef::Signal;
use aeri_core::appearance::Recipe;
use aeri_core::credentials::PasswordProvider;
use aeri_core::ids::{ProfileId, SpaceId, TabId};
use aeri_core::keymap::Shortcut;
use aeri_core::store::AvatarSlot;

use super::Pick;
use super::trust::DownloadPick;
use crate::library::LibraryMsg;
use crate::mac::{Edge, Pressure};
use crate::native_menu::Align;
use crate::resource_view::ResourceMsg;

pub enum AppMsg {
    EngineReady,
    ProfileReady {
        profile: ProfileId,
    },
    Signal(Signal),
    TabClosed {
        window: u64,
        tab: TabId,
    },
    CloseWindow {
        window: u64,
    },
    /// The Person typed in the bar: suggest for `text`.
    BarQuery {
        window: u64,
        text: String,
    },
    /// The search engine answered for `text`: its suggested `words`.
    BarSuggestions {
        window: u64,
        text: String,
        words: Vec<String>,
    },
    /// Return or a click on a bar row.
    BarPick {
        window: u64,
        pick: Pick,
    },
    Library(LibraryMsg),
    /// The bookmarks bar's clicks and menus.
    BookmarksBar(super::BarMsg),
    /// The bar asked to go away (Escape, a click outside, losing the
    /// keyboard); `session` is the reveal it belongs to, so a late request
    /// from an earlier one is ignored.
    AddressDismissed {
        window: u64,
        session: u64,
    },
    Shortcut {
        window: u64,
        shortcut: Shortcut,
    },
    NewHome {
        window: u64,
    },
    HomeSubmit {
        window: u64,
        tab: TabId,
        text: String,
    },
    SplitFocus {
        window: u64,
        tab: TabId,
    },
    /// A command on one tab, from the sidebar or its context menu.
    Tab {
        window: u64,
        tab: TabId,
        command: TabCommand,
    },
    /// A tab in the sidebar was right-clicked at (`x`, `y`) in the window.
    TabMenu {
        window: u64,
        tab: TabId,
        x: f32,
        y: f32,
    },
    ExtensionsChanged {
        profile: ProfileId,
    },
    ExtensionAction {
        window: u64,
        extension: String,
    },
    Nav {
        window: u64,
        nav: Nav,
    },
    /// "Open in" another profile: the address only (stage 1).
    OpenInProfile {
        profile: ProfileId,
        url: String,
    },
    /// The strip's profile badge was clicked at (`x`, `y`) in the window.
    ProfileMenu {
        window: u64,
        x: f32,
        y: f32,
        align: Align,
    },
    /// The strip's "Popup blocked" notice was clicked at (`x`, `y`).
    PopupMenu {
        window: u64,
        x: f32,
        y: f32,
        align: Align,
    },
    /// The strip's downloads button was clicked at (`x`, `y`).
    DownloadsMenu {
        window: u64,
        x: f32,
        y: f32,
        align: Align,
    },
    /// The sidebar's extensions door was clicked at (`x`, `y`): the
    /// profile's extensions, to run on the shown tab.
    ExtensionsMenu {
        window: u64,
        x: f32,
        y: f32,
        align: Align,
    },
    DownloadPick {
        window: u64,
        pick: DownloadPick,
    },
    /// Switch to a profile: bring its window forward or open one.
    OpenProfile {
        profile: ProfileId,
    },
    CreateProfile {
        name: String,
        accent: u32,
    },
    RenameProfile {
        profile: ProfileId,
        name: String,
    },
    SetAccent {
        profile: ProfileId,
        accent: u32,
    },
    SetAvatar {
        profile: ProfileId,
        avatar: Option<AvatarSlot>,
    },
    /// Confirmed by the Person.
    DeleteProfile {
        profile: ProfileId,
    },
    /// Show the profile manager; with `new`, ready to name a new profile.
    ShowProfiles {
        new: bool,
    },
    ProfileManagerClosed,
    /// From the profile menu: where the profile's sign-ins are saved.
    SetPasswordProvider {
        profile: ProfileId,
        provider: PasswordProvider,
    },
    /// Chrome's password manager page, in a new tab of the window's profile.
    ManagePasswords {
        window: u64,
    },
    /// The sidebar's space header was clicked at (`x`, `y`) in the window.
    SpaceMenu {
        window: u64,
        x: f32,
        y: f32,
        align: Align,
    },
    SwitchSpace {
        window: u64,
        space: SpaceId,
    },
    /// Open the space editor: on `space`, or for a new space.
    EditSpace {
        window: u64,
        space: Option<SpaceId>,
    },
    /// From the space editor. A new space (`space: None`) is shown in
    /// `window` once made.
    SaveSpace {
        profile: ProfileId,
        space: Option<SpaceId>,
        name: String,
        colour: u32,
        icon: String,
        window: Option<u64>,
    },
    /// Delete a space; `confirmed` once the Person agreed (asked only if the
    /// space has tabs).
    DeleteSpace {
        window: u64,
        space: SpaceId,
        confirmed: bool,
    },
    SpaceEditorClosed,
    /// The space's icon and colour menu and popover.
    SpaceLook(crate::space_look::SpaceLookMsg),
    /// From the site style editor: the CSS for a site, blank to forget it.
    SaveSiteStyle {
        profile: ProfileId,
        host: String,
        css: String,
    },
    /// The tab editor: the tab's own name and icon; `None` for the page's.
    SetTabLook {
        tab: TabId,
        name: Option<String>,
        icon: Option<String>,
    },
    TabEditorClosed,
    /// A window's frame became key.
    WindowKey {
        window: u64,
    },
    /// The Dock icon was clicked.
    Reopen,
    /// Another app handed Aeri links (`shell::open_urls`).
    OpenExternal(Vec<String>),
    /// Whether Aeri is the default browser or email app may have changed.
    DefaultsChanged,
    /// The proxy asks for a login: the sheet.
    ProxyLogin,
    /// The login the Person gave the proxy: its user name.
    ProxyUser(String),
    /// Once a minute: rest tabs unseen past the Person's period, or while
    /// memory stays short.
    RestCheck,
    /// The changed sessions are due to be written.
    FlushSessions,
    /// What the tab rows read from their pages and the player does.
    TabSignal(super::tab_signals::TabSignal),
    /// Reading mode's answers from the page.
    Reader(super::reader::ReaderMsg),
    /// macOS reported a change in memory pressure.
    MemoryPressure(Pressure),
    Resources(ResourceMsg),
    /// macOS switched light/dark or an accessibility display setting.
    SystemAppearanceChanged,
    /// ⌘ went down or up anywhere in the app (tab numbers).
    CommandHeld(bool),
    Launcher(super::launcher::LauncherMsg),
    /// A small window's line and frame (`crate::little`).
    Little(crate::little::LittleMsg),
    Find(super::find::FindMsg),
    /// The floating video's window and page (`float.rs`).
    Float(super::float::FloatMsg),
    /// Peek at a link: the panel's dim, knobs and keys.
    Peek(super::peek::PeekMsg),
    /// ⌃Tab's switcher: the keys, the panel's pointer and the pictures.
    Switcher(super::switcher::SwitcherMsg),
    Chrome(ChromeMsg),
    ThemeEditor(ThemeEditorMsg),
    /// The open panel chose a theme file to add to the library.
    ImportTheme {
        window: u64,
        path: std::path::PathBuf,
    },
    /// The save panel chose where to write the window's theme.
    ExportTheme {
        window: u64,
        path: std::path::PathBuf,
    },
    /// An imported theme's sheet chose where it goes.
    UseTheme {
        window: u64,
        target: super::themes::ThemeTarget,
        recipe: Recipe,
    },
    /// A Settings tab's message, from the window that shows it.
    Settings {
        window: u64,
        msg: super::settings::SettingsMsg,
    },
    Menu(super::menu_bar::MenuCommand),
    ShowWelcome,
    WelcomeFilmFinished(gpui::WindowId, u64),
    FinishWelcome(Option<Choices>),
    /// Quit; asks first when the preference says so.
    Quit,
    /// Quit without asking: after the confirmation, and for signals.
    QuitNow,
    /// The downloads folder the Person chose in the folder panel.
    SetDownloadsDir(std::path::PathBuf),
}

/// The page-first chrome (Phase 12).
#[derive(Clone, Copy, Debug)]
pub enum ChromeMsg {
    /// Keep the chrome at `edge` beside the page, or let it slide in.
    Pin { edge: Edge, pinned: bool },
    /// The sidebar's edge was dragged to `width`; `done` when let go.
    Width { width: u16, done: bool },
    /// The pointer left the GPUI chrome for another native window.
    PointerLeft { window: u64 },
}

/// The theme editor window (Phase 12). Every change is previewed live in
/// the windows the edited theme reaches; nothing is kept until Save.
#[derive(Clone, Debug)]
pub enum ThemeEditorMsg {
    /// The draft changed: show it.
    Preview(Recipe),
    /// Keep the draft as the scope's theme.
    Save(Recipe),
    /// Drop the scope's own theme: it follows the one above it again.
    Inherit,
    /// Close without keeping: the previous look returns.
    Cancel,
}

/// The history doors beside the band, and Reload (Stop while loading).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Back,
    Forward,
    Reload,
    Stop,
}

/// What can be done to one tab from the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabCommand {
    Select,
    Split,
    /// ⌘ on a pin: with Settings › Pins › ⌘-click on a pin opens a new tab,
    /// the pin's page opens in a new tab; otherwise it is selected.
    PinCommandClick,
    Close,
    CloseOthers,
    Duplicate,
    TogglePin,
    ToggleFavourite,
    /// Bookmark the tab's page, or remove its bookmarks.
    ToggleBookmark,
    /// Open the popup Aeri refused the tab's page, as a tab.
    OpenBlockedPopup,
    /// Always allow (or stop allowing) popups from the tab's site.
    AllowPopups(bool),
    /// Chrome's settings page for the tab's site (its permissions).
    SiteSettings,
    OpenInDefaultBrowser,
    MoveToSpace(SpaceId),
    /// Drag and drop: move to this position among the favourites or the
    /// space's tabs, (un)favouriting it if it came from the other list.
    MoveTo {
        index: usize,
        favourite: bool,
    },
    /// Release the tab's page now (Phase 11); it loads again when shown.
    Rest,
    /// Keep (or stop keeping) the tab's site awake: its tabs never rest.
    KeepAwake(bool),
    /// The Person's own name and icon for the tab.
    Rename,
}
