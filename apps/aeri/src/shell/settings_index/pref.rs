//! The preferences Settings changes: what each reads from the model and
//! how a choice edits the layout or the app-wide preferences
//! (`aeri_core::prefs`).

use aeri_core::layout::{FavouriteStyle, Glyph, Layout, Side, TabPlace};
use aeri_core::prefs::{
    AppPrefs, CloseAfter, Engine, FloatLeave, NewTabPlace, Peek, Webmail, clamp_zoom,
};
use aeri_core::proxy::ProxyMode;

use super::Value;
use crate::shell::settings::SettingsModel;

/// One preference Settings changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pref {
    /// Light, Dark, Auto.
    Look,
    /// Top, Left, Right, Icons only.
    TabsLayout,
    HideColumn,
    Slides,
    FoldIcons,
    /// As a card, Edge to edge.
    CardLook,
    CardsFullScreen,
    NavLeft,
    CompactStrip,
    Roomy,
    /// Letters, Site icons.
    Glyph,
    HighContrast,
    LightMotion,
    ReduceAnimations,
    /// Never, Later, Normal, Sooner.
    Sleep,
    TabNumbers,
    AddressOnTab,
    AddressStrip,
    /// Aeri, an extension, off.
    Provider,
    ConfirmQuit,
    DownloadsAsk,
    DownloadsButton,
    /// After current, then the two ends.
    NewPlace,
    FrontBeside,
    NewAfterLast,
    CloseGoesBack,
    UndoReopens,
    LazyBackground,
    /// Never, then [`CloseAfter::ALL`].
    CloseAfter,
    /// The named engines, then Custom.
    Engine,
    CrossSite,
    ClickIds,
    Gpc,
    Notifications,
    /// Your mail app, then [`WEBMAIL`].
    Webmail,
    /// Off, HTTP, SOCKS5.
    ProxyMode,
    Launcher,
    /// Off, the named engines, then Custom.
    ShiftEngine,
    Suggest,
    NewtabRecent,
    AddressCommands,
    Dusk,
    Capture,
    /// The percent every site starts at.
    PageZoom,
    LinksTabs,
    MediaClick,
    EscapeTwice,
    ShowLinks,
    Autocorrect,
    SiteStyles,
    SidebarPlayer,
    ReadingProgress,
    TabsGive,
    BookmarksBar,
    SleepDim,
    Spaces,
    SpacesCommand,
    LinksToSpaces,
    /// Favourites as rows: a layout preference, not an app-wide one.
    PinnedRows,
    PinCommand,
    StartFresh,
    SwitcherBehind,
    SwitcherPins,
    PopupWindows,
    LinksLittle,
    LauncherLittle,
    ReaderTwins,
    ReaderFollows,
    /// Off, then the clicks that peek.
    PeekMenu,
    PeekStays,
    /// Never, Switching tabs, Tabs or apps.
    FloatOwn,
    FloatMinimize,
}

/// Search's `SleepPace` waits, in minutes (Sleep.swift:57-63).
pub const SLEEP_MINUTES: [Option<u64>; 4] = [None, Some(120), Some(30), Some(10)];

/// The place of Custom in the engine menu.
pub const CUSTOM_ENGINE: usize = Engine::NAMED.len();

const NEW_PLACES: [NewTabPlace; 3] = [NewTabPlace::Next, NewTabPlace::Top, NewTabPlace::End];
const PEEKS: [Peek; 4] = [
    Peek::Off,
    Peek::Shift,
    Peek::OptionShift,
    Peek::OptionCommand,
];
const FLOAT_LEAVES: [FloatLeave; 3] = [FloatLeave::Never, FloatLeave::Tabs, FloatLeave::TabsOrApps];

const WEBMAIL: [Webmail; 5] = [
    Webmail::None,
    Webmail::Gmail,
    Webmail::Outlook,
    Webmail::Fastmail,
    Webmail::Proton,
];

/// The place of Custom in the Shift-Return menu: Off, the named engines,
/// then Custom.
pub const CUSTOM_SHIFT: usize = Engine::NAMED.len() + 1;

impl Pref {
    pub fn get(self, m: &SettingsModel) -> Value {
        let l = &m.layout;
        let p = &m.prefs;
        match self {
            Self::Look => Value::Pick(m.look),
            Self::TabsLayout => Value::Pick(match (l.tabs, l.rail, l.side) {
                (TabPlace::Top, _, _) => 0,
                (TabPlace::Side, true, _) => 3,
                (TabPlace::Side, false, Side::Left) => 1,
                (TabPlace::Side, false, Side::Right) => 2,
            }),
            Self::HideColumn => Value::On(!l.sidebar_pinned),
            Self::Slides => Value::On(l.slides),
            Self::FoldIcons => Value::On(l.fold_icons),
            Self::CardLook => Value::Pick(usize::from(!l.cards)),
            Self::CardsFullScreen => Value::On(l.cards_full_screen),
            Self::NavLeft => Value::On(l.nav_left),
            Self::CompactStrip => Value::On(l.compact_strip),
            Self::Roomy => Value::On(l.roomy),
            Self::Glyph => Value::Pick(usize::from(l.glyph == Glyph::Icons)),
            Self::HighContrast => Value::On(l.high_contrast),
            Self::LightMotion => Value::On(l.light_motion),
            Self::ReduceAnimations => Value::On(l.reduce_animations),
            Self::Sleep => Value::Pick(m.sleep),
            Self::TabNumbers => Value::On(l.tab_numbers),
            Self::AddressOnTab => Value::On(l.address_on_tab),
            Self::AddressStrip => Value::On(l.address_strip),
            Self::Provider => Value::Pick(m.provider),
            Self::ConfirmQuit => Value::On(p.general.confirm_quit),
            Self::DownloadsAsk => Value::On(p.general.downloads_ask),
            Self::DownloadsButton => Value::On(p.general.downloads_button_always),
            Self::NewPlace => Value::Pick(
                NEW_PLACES
                    .iter()
                    .position(|place| *place == p.tabs.new_place)
                    .unwrap_or(0),
            ),
            Self::FrontBeside => Value::On(p.tabs.front_beside),
            Self::NewAfterLast => Value::On(p.tabs.new_after_last),
            Self::CloseGoesBack => Value::On(p.tabs.close_goes_back),
            Self::UndoReopens => Value::On(p.tabs.undo_reopens),
            Self::LazyBackground => Value::On(p.tabs.lazy_background),
            Self::CloseAfter => Value::Pick(
                CloseAfter::ALL
                    .iter()
                    .position(|after| *after == p.tabs.close_after_hours)
                    .unwrap_or(0),
            ),
            Self::Engine => Value::Pick(
                Engine::NAMED
                    .iter()
                    .position(|engine| *engine == p.browsing.engine)
                    .unwrap_or(CUSTOM_ENGINE),
            ),
            Self::CrossSite => Value::On(p.privacy.prevent_cross_site_tracking),
            Self::ClickIds => Value::On(p.privacy.strip_click_ids),
            Self::Gpc => Value::On(p.privacy.gpc),
            Self::Notifications => Value::On(p.privacy.notifications_ask),
            Self::Webmail => Value::Pick(
                WEBMAIL
                    .iter()
                    .position(|w| *w == p.general.webmail)
                    .unwrap_or(0),
            ),
            Self::ProxyMode => Value::Pick(
                ProxyMode::ALL
                    .iter()
                    .position(|m| *m == p.proxy.mode)
                    .unwrap_or(0),
            ),
            Self::Launcher => Value::On(p.launcher.on),
            Self::ShiftEngine => Value::Pick(match &p.browsing.shift_engine {
                None => 0,
                Some(engine) => Engine::NAMED
                    .iter()
                    .position(|named| named == engine)
                    .map_or(CUSTOM_SHIFT, |ix| ix + 1),
            }),
            Self::Suggest => Value::On(p.browsing.suggest),
            Self::NewtabRecent => Value::On(p.spaces.newtab_recent),
            Self::AddressCommands => Value::On(p.spaces.address_commands),
            Self::Dusk => Value::On(p.appearance.dusk),
            Self::Capture => Value::On(p.browsing.capture),
            Self::PageZoom => Value::Pick(usize::from(p.browsing.page_zoom_percent)),
            Self::LinksTabs => Value::On(p.browsing.links_little_tabs),
            Self::MediaClick => Value::On(p.browsing.media_click),
            Self::EscapeTwice => Value::On(p.browsing.escape_twice),
            Self::ShowLinks => Value::On(p.browsing.show_link_targets),
            Self::Autocorrect => Value::On(p.browsing.autocorrect),
            Self::SiteStyles => Value::On(p.browsing.site_styles),
            Self::SidebarPlayer => Value::On(p.appearance.sidebar_player),
            Self::ReadingProgress => Value::On(p.appearance.reading_progress),
            Self::TabsGive => Value::On(p.appearance.tabs_give),
            Self::BookmarksBar => Value::On(p.appearance.bookmarks_bar),
            Self::SleepDim => Value::On(p.tabs.sleep_dim),
            Self::Spaces => Value::On(p.spaces.on()),
            Self::SpacesCommand => Value::On(p.spaces.spaces_command_numbers),
            Self::LinksToSpaces => Value::On(p.spaces.links_to_spaces),
            Self::PinnedRows => Value::On(l.favourites == FavouriteStyle::Rows),
            Self::PinCommand => Value::On(p.spaces.pin_command_new_tab),
            Self::StartFresh => Value::On(p.spaces.start_fresh),
            Self::SwitcherBehind => Value::On(p.tabs.switcher_behind_first),
            Self::SwitcherPins => Value::On(p.tabs.switcher_leaves_out_pins),
            Self::PopupWindows => Value::On(p.tabs.popup_windows),
            Self::LinksLittle => Value::On(p.browsing.links_little),
            Self::LauncherLittle => Value::On(p.launcher.little),
            Self::ReaderTwins => Value::On(p.browsing.reader_twins),
            Self::ReaderFollows => Value::On(p.browsing.reader_follows),
            Self::PeekMenu => {
                Value::Pick(PEEKS.iter().position(|k| *k == p.mouse.peek).unwrap_or(0))
            }
            Self::PeekStays => Value::On(p.mouse.peek_stays),
            Self::FloatOwn => Value::Pick(
                FLOAT_LEAVES
                    .iter()
                    .position(|k| *k == p.browsing.float_leave)
                    .unwrap_or(0),
            ),
            Self::FloatMinimize => Value::On(p.browsing.float_minimize),
        }
    }

    /// Changes `layout` if this preference lives there; false if not.
    pub fn edit_layout(self, layout: &mut Layout, value: Value) -> bool {
        let on = value.on();
        match self {
            Self::TabsLayout => {
                let was_top = layout.tabs == TabPlace::Top;
                match value.index() {
                    0 => {
                        layout.tabs = TabPlace::Top;
                        layout.top_pinned = true;
                        layout.rail = false;
                    }
                    ix => {
                        layout.tabs = TabPlace::Side;
                        layout.rail = ix == 3;
                        match ix {
                            1 => layout.side = Side::Left,
                            2 => layout.side = Side::Right,
                            _ => {}
                        }
                        // The column was never hidden by choice up there.
                        if was_top {
                            layout.sidebar_pinned = true;
                        }
                    }
                }
            }
            Self::HideColumn => layout.sidebar_pinned = !on,
            Self::Slides => layout.slides = on,
            Self::FoldIcons => layout.fold_icons = on,
            Self::CardLook => layout.cards = value.index() == 0,
            Self::CardsFullScreen => layout.cards_full_screen = on,
            Self::NavLeft => layout.nav_left = on,
            Self::CompactStrip => layout.compact_strip = on,
            Self::Roomy => layout.roomy = on,
            Self::Glyph => {
                layout.glyph = if value.index() == 1 {
                    Glyph::Icons
                } else {
                    Glyph::Letters
                }
            }
            Self::HighContrast => layout.high_contrast = on,
            Self::LightMotion => layout.light_motion = on,
            Self::ReduceAnimations => layout.reduce_animations = on,
            Self::TabNumbers => layout.tab_numbers = on,
            Self::AddressOnTab => layout.address_on_tab = on,
            Self::AddressStrip => {
                layout.address_strip = on;
                layout.top_pinned |= on;
            }
            Self::PinnedRows => {
                layout.favourites = if on {
                    FavouriteStyle::Rows
                } else {
                    FavouriteStyle::Grid
                }
            }
            _ => return false,
        }
        true
    }

    /// Changes the app-wide preferences if this preference lives there;
    /// false if not.
    pub fn edit_prefs(self, prefs: &mut AppPrefs, value: Value) -> bool {
        let on = value.on();
        let ix = value.index();
        match self {
            Self::ConfirmQuit => prefs.general.confirm_quit = on,
            Self::DownloadsAsk => prefs.general.downloads_ask = on,
            Self::DownloadsButton => prefs.general.downloads_button_always = on,
            Self::NewPlace => {
                prefs.tabs.new_place = NEW_PLACES.get(ix).copied().unwrap_or_default();
            }
            Self::FrontBeside => prefs.tabs.front_beside = on,
            Self::NewAfterLast => prefs.tabs.new_after_last = on,
            Self::CloseGoesBack => prefs.tabs.close_goes_back = on,
            Self::UndoReopens => prefs.tabs.undo_reopens = on,
            Self::LazyBackground => prefs.tabs.lazy_background = on,
            Self::CloseAfter => {
                prefs.tabs.close_after_hours = CloseAfter::ALL.get(ix).copied().unwrap_or_default();
            }
            Self::Engine => match Engine::NAMED.get(ix) {
                Some(engine) => prefs.browsing.engine = engine.clone(),
                None if matches!(prefs.browsing.engine, Engine::Custom(_)) => {}
                None => prefs.browsing.engine = Engine::Custom(String::new()),
            },
            Self::CrossSite => prefs.privacy.prevent_cross_site_tracking = on,
            Self::ClickIds => prefs.privacy.strip_click_ids = on,
            Self::Gpc => prefs.privacy.gpc = on,
            Self::Notifications => prefs.privacy.notifications_ask = on,
            Self::Webmail => prefs.general.webmail = WEBMAIL.get(ix).copied().unwrap_or_default(),
            Self::ProxyMode => {
                prefs.proxy.mode = ProxyMode::ALL.get(ix).copied().unwrap_or_default();
            }
            Self::Launcher => prefs.launcher.on = on,
            Self::ShiftEngine => match ix {
                0 => prefs.browsing.shift_engine = None,
                CUSTOM_SHIFT => {
                    if !matches!(prefs.browsing.shift_engine, Some(Engine::Custom(_))) {
                        prefs.browsing.shift_engine = Some(Engine::Custom(String::new()));
                    }
                }
                _ => {
                    prefs.browsing.shift_engine = Engine::NAMED.get(ix - 1).cloned();
                }
            },
            Self::Suggest => prefs.browsing.suggest = on,
            Self::NewtabRecent => prefs.spaces.newtab_recent = on,
            Self::AddressCommands => prefs.spaces.address_commands = on,
            Self::Dusk => prefs.appearance.dusk = on,
            Self::Capture => prefs.browsing.capture = on,
            Self::PageZoom => {
                prefs.browsing.page_zoom_percent = clamp_zoom(u16::try_from(ix).unwrap_or(100))
            }
            Self::LinksTabs => prefs.browsing.links_little_tabs = on,
            Self::MediaClick => prefs.browsing.media_click = on,
            Self::EscapeTwice => prefs.browsing.escape_twice = on,
            Self::ShowLinks => prefs.browsing.show_link_targets = on,
            Self::Autocorrect => prefs.browsing.autocorrect = on,
            Self::SiteStyles => prefs.browsing.site_styles = on,
            Self::SidebarPlayer => prefs.appearance.sidebar_player = on,
            Self::ReadingProgress => prefs.appearance.reading_progress = on,
            Self::TabsGive => prefs.appearance.tabs_give = on,
            Self::BookmarksBar => prefs.appearance.bookmarks_bar = on,
            Self::SleepDim => prefs.tabs.sleep_dim = on,
            Self::Spaces => prefs.spaces.enabled = Some(on),
            Self::SpacesCommand => prefs.spaces.spaces_command_numbers = on,
            Self::LinksToSpaces => prefs.spaces.links_to_spaces = on,
            Self::PinCommand => prefs.spaces.pin_command_new_tab = on,
            Self::StartFresh => prefs.spaces.start_fresh = on,
            Self::SwitcherBehind => prefs.tabs.switcher_behind_first = on,
            Self::SwitcherPins => prefs.tabs.switcher_leaves_out_pins = on,
            Self::PopupWindows => prefs.tabs.popup_windows = on,
            Self::LinksLittle => prefs.browsing.links_little = on,
            Self::LauncherLittle => prefs.launcher.little = on,
            Self::ReaderTwins => prefs.browsing.reader_twins = on,
            Self::ReaderFollows => prefs.browsing.reader_follows = on,
            Self::PeekMenu => prefs.mouse.peek = PEEKS.get(ix).copied().unwrap_or_default(),
            Self::PeekStays => prefs.mouse.peek_stays = on,
            Self::FloatOwn => {
                prefs.browsing.float_leave = FLOAT_LEAVES.get(ix).copied().unwrap_or_default();
            }
            Self::FloatMinimize => prefs.browsing.float_minimize = on,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_round_trip_through_the_preferences() {
        let mut prefs = AppPrefs::default();
        let mut model = SettingsModel::default();
        for (pref, value) in [
            (Pref::NewPlace, Value::Pick(2)),
            (Pref::CloseAfter, Value::Pick(3)),
            (Pref::Engine, Value::Pick(1)),
            (Pref::Gpc, Value::On(false)),
        ] {
            assert!(pref.edit_prefs(&mut prefs, value));
            model.prefs = prefs.clone();
            assert_eq!(pref.get(&model), value);
        }
        assert_eq!(prefs.tabs.new_place, NewTabPlace::End);
        assert_eq!(prefs.tabs.close_after_hours, CloseAfter::Week);
        assert_eq!(prefs.browsing.engine, Engine::DuckDuckGo);
    }

    #[test]
    fn custom_keeps_its_template_and_layout_prefs_are_not_stored() {
        let mut prefs = AppPrefs::default();
        prefs.browsing.engine = Engine::Custom("https://x.test/?q=%s".into());
        assert!(Pref::Engine.edit_prefs(&mut prefs, Value::Pick(CUSTOM_ENGINE)));
        assert_eq!(
            prefs.browsing.engine,
            Engine::Custom("https://x.test/?q=%s".into())
        );
        let before = prefs.clone();
        assert!(!Pref::Roomy.edit_prefs(&mut prefs, Value::On(true)));
        assert_eq!(prefs, before);
        assert!(!Pref::ConfirmQuit.edit_layout(&mut Layout::default(), Value::On(true)));
    }
}
