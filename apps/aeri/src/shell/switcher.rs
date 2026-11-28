//! ⌃Tab's switcher (Search `TabSwitcher.swift`, `Browser.swift` :5903-5985):
//! the shell's side. One gesture at a time, app-wide, as the keyboard is
//! one. `mac::switcher` reads the keys and tells what they did; here is the
//! gesture's state (`aeri_core::switcher`), what a card does when acted on,
//! the pictures on the cards, and the panel's window.
//!
//! Pictures (`:51`): the tab you leave is pictured as it goes (Search waits
//! 0.4 s after the switching stops; Aeri asks at once, while the page is
//! still on screen, since Chromium cannot picture a page that is hidden); a
//! gesture asks for the pictures the cards lack once the panel is up, the
//! pick first, 0.04 s apart. A picture holds only while the page is at the
//! address it was taken at, and only the ten tabs left last (and the cards
//! on screen) keep one, in memory.
//!
//! Left out, since Aeri has neither: Search's small windows ("moons", and
//! ⌃O) and a split pair as one card. Pinned tabs are never put down by ⌘W
//! here, so they are not left out of the grid for being asleep; groups do
//! not exist yet.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aeri_core::engine::Command;
use aeri_core::ids::TabId;
use aeri_core::layout::Glyph;
use aeri_core::switcher::{Gesture, Removal, ordered};
use futures::channel::mpsc::UnboundedSender;
use gpui::{
    App, AppContext, Bounds, RenderImage, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, point, px, size,
};

use super::windows::host_of;
use super::{AppMsg, Shell, content_view};
use crate::mac::{CardKey, KeyInput};
use crate::tab_switcher::{Card, TabSwitcher};

/// The panel waits this long for a second press before it draws
/// (`TabSwitcher.swift:230`): a quick tap never shows anything.
const REVEAL: Duration = Duration::from_millis(150);
/// The pictures are asked for this long after the pick settled (`:459`),
/// and this far apart (`:366`).
const SETTLE: Duration = Duration::from_millis(120);
const STAGGER: Duration = Duration::from_millis(40);
/// A card's picture is taken 180 CSS pixels across (`:379`).
const PICTURE_WIDTH: f64 = 180.;
/// The tabs whose pictures are kept between gestures (`:318`).
const KEPT: usize = 10;
/// A picture asked for that is not answered by now is asked for afresh next time.
const PICTURE_TIMEOUT: Duration = Duration::from_secs(2);
/// A picture of the page a tab still shows, younger than this, is not taken
/// again when the Person leaves the tab.
const FRESH: Duration = Duration::from_secs(30);

/// What the switcher's keys, panel and pictures tell the shell.
#[derive(Debug)]
pub enum SwitcherMsg {
    Key(KeyInput),
    /// The panel's first draw is due, for gesture `0`.
    Reveal(u64),
    /// The pick has stayed: ask for the pictures the cards lack.
    Settled(u64),
    /// Picture this card now, if gesture `0` is still on.
    Shoot(u64, TabId),
    /// The pointer moved onto a card.
    Select {
        tab: TabId,
    },
    /// A click on a card.
    Pick {
        tab: TabId,
    },
    /// Draw the cards again: something on one changed.
    Redraw,
    /// A picture asked for with `token` has had no answer in time: ask again if
    /// need be.
    Unstick {
        tab: TabId,
        token: u64,
    },
    /// A click outside the panel.
    Dismiss,
    /// A page's picture: a PNG, or none if it could not be taken. `url` is
    /// where the page was.
    Picture {
        tab: TabId,
        url: String,
        png: Option<Vec<u8>>,
    },
}

/// The gesture on.
struct Session {
    window: u64,
    id: u64,
    gesture: Gesture,
    /// The panel is up.
    visible: bool,
    /// The pictures were asked for.
    asked: bool,
}

/// A page's picture and the address it shows.
struct Picture {
    url: String,
    image: Arc<RenderImage>,
    /// When the page was pictured.
    taken: Instant,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    static PANELS: RefCell<HashMap<u64, WindowHandle<TabSwitcher>>> = RefCell::default();
    static PICTURES: RefCell<HashMap<TabId, Picture>> = RefCell::default();
    /// Pictures asked for and not yet answered.
    static ASKED: RefCell<HashMap<TabId, u64>> = RefCell::default();
    static TOKENS: Cell<u64> = const { Cell::new(0) };
    static GESTURES: Cell<u64> = const { Cell::new(0) };
}

fn session<R>(f: impl FnOnce(&mut Session) -> R) -> Option<R> {
    SESSION.with_borrow_mut(|session| session.as_mut().map(f))
}

/// Starts reading the switcher's keys.
pub(super) fn start(tx: &UnboundedSender<AppMsg>) {
    let tx = tx.clone();
    crate::mac::watch_switcher(move |key| {
        let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Key(key)));
    });
}

/// A picture's PNG as a sprite: GPUI's are BGRA.
fn decode(png: &[u8]) -> Option<Arc<RenderImage>> {
    let mut pixels = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([image::Frame::new(pixels)])))
}

fn is_web(url: &str) -> bool {
    !host_of(url).is_empty()
}

impl Shell {
    pub(super) fn switcher_msg(&mut self, msg: SwitcherMsg, cx: &mut App) {
        match msg {
            SwitcherMsg::Key(key) => self.switcher_key(key, cx),
            SwitcherMsg::Reveal(id) => {
                if session(|s| s.id == id && !s.visible) == Some(true) {
                    self.switcher_show(cx);
                }
            }
            SwitcherMsg::Settled(id) => self.switcher_settled(id, cx),
            SwitcherMsg::Shoot(id, tab) => {
                if session(|s| s.id == id && s.visible) == Some(true) {
                    self.switcher_shoot(tab, cx);
                }
            }
            SwitcherMsg::Select { tab } => {
                if session(|s| s.gesture.select(tab)).is_some() {
                    self.switcher_draw(cx);
                }
            }
            SwitcherMsg::Redraw => self.switcher_draw(cx),
            SwitcherMsg::Unstick { tab, token } => ASKED.with_borrow_mut(|asked| {
                if asked.get(&tab) == Some(&token) {
                    asked.remove(&tab);
                }
            }),
            SwitcherMsg::Pick { tab } => self.switcher_commit(Some(tab), cx),
            SwitcherMsg::Dismiss => self.switcher_end(cx),
            SwitcherMsg::Picture { tab, url, png } => self.switcher_picture(tab, &url, png, cx),
        }
    }

    fn switcher_key(&mut self, key: KeyInput, cx: &mut App) {
        match key {
            KeyInput::Begin { backwards } => self.switcher_begin(backwards, cx),
            KeyInput::Walk { backwards } => {
                if session(|s| s.gesture.walk(backwards)).is_some() {
                    self.switcher_show(cx);
                }
            }
            KeyInput::Move(direction) => {
                if session(|s| s.gesture.step(direction)).is_some() {
                    self.switcher_show(cx);
                }
            }
            KeyInput::Act(act) => self.switcher_act(act, cx),
            KeyInput::Cancel => self.switcher_end(cx),
            KeyInput::Commit => self.switcher_commit(None, cx),
        }
    }

    /// The tabs a window's grid draws from, in the row's order: favourites,
    /// then the space's tabs (`switcherRow`, `Browser.swift:5912`). With
    /// Settings › Tabs › Leave pins out of the switcher, pinned tabs wait
    /// outside it unless it is the one on screen.
    fn switcher_row(&self, window_id: u64) -> Vec<TabId> {
        let Some(w) = self.window(window_id) else {
            return Vec::new();
        };
        let shown = w.shown();
        let leave_out = self.prefs.tabs.switcher_leaves_out_pins;
        let favourites = self
            .open
            .get(&w.profile)
            .map(|open| {
                open.favourites
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        favourites
            .into_iter()
            .chain(
                w.tabs
                    .iter()
                    .filter(|item| !(leave_out && item.pinned && Some(item.id) != shown))
                    .map(|item| item.id),
            )
            .collect()
    }

    fn switcher_begin(&mut self, backwards: bool, cx: &mut App) {
        let Some(w) = self.windows.iter().find(|w| w.frame.is_key()) else {
            crate::mac::release_keyboard();
            return;
        };
        let window_id = w.id;
        let Some(current) = w.shown().filter(|_| !w.closing) else {
            crate::mac::release_keyboard();
            return;
        };
        // A favourite on screen: the space's tab it replaced is the tab left.
        let mut recent: Vec<TabId> = Vec::new();
        let mut used: Vec<TabId> = Vec::new();
        if w.favourite.is_some()
            && let Some(active) = w.tabs.active()
        {
            recent.push(active);
            used.push(active);
        }
        recent.extend_from_slice(w.tabs.recent());
        used.extend_from_slice(w.tabs.used());
        let row = self.switcher_row(window_id);
        let ring = ordered(
            &row,
            current,
            &recent,
            &used,
            self.prefs.tabs.switcher_behind_first,
        );
        let Some(gesture) = Gesture::begin(current, ring, backwards) else {
            // One tab and nothing else: nothing happens.
            crate::mac::release_keyboard();
            return;
        };
        if self.switcher_panel(window_id, cx).is_none() {
            crate::mac::release_keyboard();
            return;
        }
        let id = GESTURES.get() + 1;
        GESTURES.set(id);
        SESSION.with_borrow_mut(|slot| {
            *slot = Some(Session {
                window: window_id,
                id,
                gesture,
                visible: false,
                asked: false,
            });
        });
        // The tab on screen is pictured afresh, whatever picture it has.
        self.switcher_shoot(current, cx);
        let tx = self.tx.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(REVEAL).await;
            let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Reveal(id)));
        })
        .detach();
        self.switcher_draw(cx);
    }

    /// The panel's window of `window_id`, made on first use.
    fn switcher_panel(
        &mut self,
        window_id: u64,
        cx: &mut App,
    ) -> Option<WindowHandle<TabSwitcher>> {
        if let Some(handle) = PANELS.with_borrow(|panels| panels.get(&window_id).copied()) {
            return Some(handle);
        }
        let frame = self.window(window_id)?.frame.clone();
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
                size(px(800.), px(600.)),
            ))),
            ..Default::default()
        };
        let tx = self.tx.clone();
        let handle = cx
            .open_window(options, |_, cx| cx.new(|_| TabSwitcher::new(window_id, tx)))
            .inspect_err(|err| tracing::error!(%err, "could not open the tab switcher"))
            .ok()?;
        let view = handle
            .update(cx, |_, window, _| content_view(window))
            .ok()
            .flatten()?;
        self.later(move |_| frame.attach_switcher(view));
        PANELS.with_borrow_mut(|panels| panels.insert(window_id, handle));
        Some(handle)
    }

    /// The panel comes up now (a second press, an arrow, or the wait is
    /// over) and the cards are drawn; once, the pictures are asked for.
    fn switcher_show(&mut self, cx: &mut App) {
        let Some((window, id, was_visible)) = session(|s| {
            let was = s.visible;
            s.visible = true;
            (s.window, s.id, was)
        }) else {
            return;
        };
        if !was_visible {
            if let Some(frame) = self.window(window).map(|w| w.frame.clone()) {
                self.later(move |_| frame.show_switcher());
            }
            let tx = self.tx.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(SETTLE).await;
                let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Settled(id)));
            })
            .detach();
        }
        self.switcher_draw(cx);
    }

    /// A card's page, wherever its tab lives.
    fn switcher_card(&self, window_id: u64, tab: TabId) -> Option<Card> {
        let w = self.window(window_id)?;
        let open = self.open.get(&w.profile)?;
        let item = w.tabs.get(tab).or_else(|| open.favourites.get(tab))?;
        let url = item.value.url();
        let host = host_of(url);
        let label = item.value.label();
        let emoji = item.value.icon.clone();
        let letter = host
            .strip_prefix("www.")
            .unwrap_or(&host)
            .chars()
            .find(|c| c.is_alphanumeric())
            .or_else(|| label.chars().find(|c| c.is_alphanumeric()))
            .map_or_else(|| "\u{b7}".to_owned(), |c| c.to_uppercase().collect());
        let blank = emoji.is_none()
            && host.is_empty()
            && matches!(label.trim(), "" | "New Tab" | "about:blank");
        let favicon = item
            .value
            .favicon
            .as_ref()
            .map(|icon| icon.image.clone())
            .or_else(|| open.icons.get(&host));
        let preview = PICTURES.with_borrow(|pictures| {
            pictures
                .get(&tab)
                .filter(|picture| picture.url == url)
                .map(|picture| picture.image.clone())
        });
        Some(Card {
            id: tab,
            label,
            letter,
            emoji,
            favicon,
            blank,
            site_icons: self.chrome.layout.glyph == Glyph::Icons,
            preview,
            muted: item.value.cef.as_ref().is_some_and(|page| page.muted()),
        })
    }

    /// Tells the panel what to draw.
    fn switcher_draw(&mut self, cx: &mut App) {
        let Some((window, ring, selected, visible)) = session(|s| {
            (
                s.window,
                s.gesture.ring().to_vec(),
                s.gesture.index(),
                s.visible,
            )
        }) else {
            return;
        };
        let cards: Vec<Card> = ring
            .iter()
            .filter_map(|&tab| self.switcher_card(window, tab))
            .collect();
        if let Some(handle) = PANELS.with_borrow(|panels| panels.get(&window).copied()) {
            let _ = handle.update(cx, |view, _, cx| view.set(cards, selected, visible, cx));
        }
    }

    /// The gesture is over, whatever came of it: the panel goes, the keys
    /// are the page's again.
    fn switcher_end(&mut self, cx: &mut App) {
        let Some(ended) = SESSION.with_borrow_mut(Option::take) else {
            return;
        };
        crate::mac::release_keyboard();
        if let Some(handle) = PANELS.with_borrow(|panels| panels.get(&ended.window).copied()) {
            let _ = handle.update(cx, |view, _, cx| view.set(Vec::new(), 0, false, cx));
        }
        if ended.visible
            && let Some(frame) = self.window(ended.window).map(|w| w.frame.clone())
        {
            self.later(move |_| frame.hide_switcher(true));
        }
        self.switcher_prune(cx);
    }

    /// ⌃ was let go of (or a card clicked): go to the card picked.
    fn switcher_commit(&mut self, picking: Option<TabId>, cx: &mut App) {
        let Some((window, home, selected)) =
            session(|s| (s.window, s.gesture.home(), s.gesture.selected()))
        else {
            return;
        };
        let target = picking.unwrap_or(selected);
        let valid = session(|s| s.gesture.ring().contains(&target)) == Some(true);
        self.switcher_end(cx);
        if valid && target != home {
            self.switch_to(window, target, cx);
        }
    }

    /// ⌃W, ⌃R, ⌃M on the card picked (`Browser.act(_:onCard:in:)`); the
    /// switcher stays up.
    fn switcher_act(&mut self, act: CardKey, cx: &mut App) {
        let Some((window, tab)) = session(|s| (s.window, s.gesture.selected())) else {
            return;
        };
        match act {
            CardKey::Close => {
                let removal = session(|s| s.gesture.remove(tab));
                self.close_tab(window, tab, cx);
                ASKED.with_borrow_mut(|asked| asked.remove(&tab));
                if removal == Some(Removal::Ended) {
                    self.switcher_end(cx);
                } else {
                    self.switcher_draw(cx);
                }
            }
            CardKey::Reload => {
                if let Some(page) = self.page_of(tab) {
                    self.later(move |_| page.execute(&Command::Reload { tab }));
                }
            }
            CardKey::Mute => {
                if let Some(page) = self.page_of(tab) {
                    self.later(move |_| page.set_muted(!page.muted()));
                    // Drawn again once the page took it.
                    let tx = self.tx.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(30))
                            .await;
                        let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Redraw));
                    })
                    .detach();
                }
            }
        }
    }

    /// The pick has stayed: picture the cards that have none, the pick
    /// first, then the rest in the ring's order (`capturePreviews`, `:357`).
    fn switcher_settled(&mut self, id: u64, cx: &mut App) {
        let Some((window, home, ring, selected)) = session(|s| {
            let first = !std::mem::replace(&mut s.asked, true);
            (s.id == id && first).then(|| {
                (
                    s.window,
                    s.gesture.home(),
                    s.gesture.ring().to_vec(),
                    s.gesture.selected(),
                )
            })
        })
        .flatten() else {
            return;
        };
        let mut order = vec![selected];
        order.extend(ring.into_iter().filter(|&tab| tab != selected));
        let wanted: Vec<TabId> = order
            .into_iter()
            .filter(|&tab| {
                tab != home
                    && self
                        .switcher_card(window, tab)
                        .is_some_and(|card| card.preview.is_none())
            })
            .collect();
        for (index, tab) in wanted.into_iter().enumerate() {
            let tx = self.tx.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(STAGGER * index as u32).await;
                let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Shoot(id, tab)));
            })
            .detach();
        }
    }

    /// Asks `tab`'s page for a picture of itself.
    fn switcher_shoot(&self, tab: TabId, cx: &mut App) {
        let Some(page) = self.page_of(tab) else {
            return;
        };
        let Some(url) = self
            .windows
            .iter()
            .find_map(|w| w.tabs.get(tab))
            .or_else(|| self.open.values().find_map(|o| o.favourites.get(tab)))
            .map(|item| item.value.url().to_owned())
            .filter(|url| is_web(url))
        else {
            return;
        };
        if ASKED.with_borrow(|asked| asked.contains_key(&tab)) {
            return;
        }
        let token = TOKENS.get() + 1;
        TOKENS.set(token);
        ASKED.with_borrow_mut(|asked| asked.insert(tab, token));
        let tx = self.tx.clone();
        let timeout = self.tx.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(PICTURE_TIMEOUT).await;
            let _ = timeout.unbounded_send(AppMsg::Switcher(SwitcherMsg::Unstick { tab, token }));
        })
        .detach();
        self.later(move |_| {
            page.preview(PICTURE_WIDTH, move |png| {
                let _ = tx.unbounded_send(AppMsg::Switcher(SwitcherMsg::Picture { tab, url, png }));
            });
        });
    }

    /// Whether `tab` is still open somewhere.
    fn tab_alive(&self, tab: TabId) -> bool {
        self.windows.iter().any(|w| w.tabs.get(tab).is_some())
            || self.open.values().any(|o| o.favourites.get(tab).is_some())
    }

    /// A page was pictured. Kept if the page is still where it was, and
    /// the tab is one of the ten left last or on a card (`cachePreview`).
    fn switcher_picture(&mut self, tab: TabId, url: &str, png: Option<Vec<u8>>, cx: &mut App) {
        ASKED.with_borrow_mut(|asked| asked.remove(&tab));
        if !self.tab_alive(tab) {
            return;
        }
        let Some(image) = png.as_deref().and_then(decode) else {
            return;
        };
        let here = self
            .windows
            .iter()
            .find_map(|w| w.tabs.get(tab))
            .or_else(|| self.open.values().find_map(|o| o.favourites.get(tab)))
            .is_some_and(|item| item.value.url() == url);
        if !here {
            cx.drop_image(image, None);
            return;
        }
        let old = PICTURES.with_borrow_mut(|pictures| {
            pictures.insert(
                tab,
                Picture {
                    url: url.to_owned(),
                    image,
                    taken: Instant::now(),
                },
            )
        });
        if let Some(old) = old {
            cx.drop_image(old.image, None);
        }
        self.switcher_prune(cx);
        if session(|_| ()).is_some() {
            self.switcher_draw(cx);
        }
    }

    /// Only the pictures of the ten tabs left last and of the cards on
    /// screen stay (`prune`, `:332`).
    fn switcher_prune(&mut self, cx: &mut App) {
        let mut keep: HashSet<TabId> = HashSet::new();
        for w in &self.windows {
            keep.extend(w.tabs.recent().iter().take(KEPT));
            keep.extend(w.shown());
        }
        SESSION.with_borrow(|session| {
            if let Some(session) = session {
                keep.extend(session.gesture.ring());
            }
        });
        let gone: Vec<Picture> = PICTURES.with_borrow_mut(|pictures| {
            let drop: Vec<TabId> = pictures
                .keys()
                .copied()
                .filter(|tab| !keep.contains(tab))
                .collect();
            drop.into_iter()
                .filter_map(|tab| pictures.remove(&tab))
                .collect()
        });
        for picture in gone {
            cx.drop_image(picture.image, None);
        }
    }

    /// Going to another tab: the one on screen is pictured as it goes,
    /// unless the switcher already holds a picture of the same page taken
    /// within [`FRESH`]. Each picture makes the page draw, encode and send a
    /// PNG that is then decoded on the main thread; switching back and
    /// forth should not pay that every time.
    pub(super) fn switcher_leaving(&self, window_id: u64, cx: &mut App) {
        let Some(w) = self.window(window_id) else {
            return;
        };
        let Some(shown) = w.shown() else {
            return;
        };
        let url = w.tabs.get(shown).map(|item| item.value.url());
        let fresh = PICTURES.with_borrow(|pictures| {
            pictures.get(&shown).is_some_and(|picture| {
                Some(picture.url.as_str()) == url && picture.taken.elapsed() < FRESH
            })
        });
        if !fresh {
            self.switcher_shoot(shown, cx);
        }
    }

    /// The window goes: its panel with it.
    pub(super) fn close_switcher_window(&mut self, window_id: u64, cx: &mut App) {
        if session(|s| s.window == window_id) == Some(true) {
            self.switcher_end(cx);
        }
        let Some(handle) = PANELS.with_borrow_mut(|panels| panels.remove(&window_id)) else {
            return;
        };
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        if let Some(frame) = self.window(window_id).map(|w| w.frame.clone()) {
            self.later(move |_| frame.forget_switcher());
        }
    }
}
