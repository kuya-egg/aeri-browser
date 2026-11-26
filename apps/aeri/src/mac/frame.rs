//! AppKit layering for one Aeri window (ADR 0002).
//!
//! ```text
//!   address bar   GPUI child window, above everything, shown when called
//!   strip overlay GPUI child window: the top chrome when it slides in
//!   side overlay  GPUI child window: the sidebar when it slides in
//!   active tab    CEF window (one browser), child window
//!   other tabs    CEF windows, ordered out
//!   frame         GPUI window: pinned chrome with the traffic lights; owns
//!                 resize, minimise, full screen, Spaces
//! ```
//!
//! Child windows move with the frame and follow it into full screen. With
//! no chrome pinned (Phase 12's page-first default) the active tab's window
//! covers the frame; pinned chrome reserves its side and the page sits
//! beside it as an inset card. Tab windows have no title bar buttons.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use aeri_cef::TabObserver;
use aeri_core::appearance::Motion;
use aeri_core::ids::TabId;
use aeri_core::layout::{Layout, Side};
use block2::RcBlock;
use futures::channel::mpsc::UnboundedSender;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, ProtocolObject, Sel};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDidBecomeActiveNotification,
    NSApplicationDidResignActiveNotification, NSColor, NSWindow, NSWindowButton,
    NSWindowCollectionBehavior, NSWindowDidBecomeKeyNotification,
    NSWindowDidBecomeMainNotification, NSWindowDidEnterFullScreenNotification,
    NSWindowDidExitFullScreenNotification, NSWindowDidResignKeyNotification,
    NSWindowDidResizeNotification, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize,
    NSUserDefaults, ns_string,
};

use super::fold::Fold;
use super::lights::Lights;
use super::reveal::{self, Reveal};
use super::window_of;
use crate::native_menu;
use crate::shell::AppMsg;
pub(super) use crate::window_frame::pinned_side_width;
use crate::window_frame::{PageInsets, column_glides, page_insets, strip_height};

/// The address bar's input row, in points, as Zen's floating URL bar has it
/// (see `address_bar`): the bar's window covers the whole page and paints
/// the card itself, so only the row's height is shared with the view (it
/// sizes the window before the first layout).
pub const BAR_HEIGHT: f64 = 62.0;
/// The side overlay's width beyond the sidebar: room for the fold shadow
/// (Search's `FoldShadow`: radius 20, 4 pt away from the page).
pub const SIDE_OVERLAY_EXTRA: f64 = 28.0;

pub struct Frame {
    pub(super) window_id: u64,
    pub(super) tx: UnboundedSender<AppMsg>,
    pub(super) frame: RefCell<Option<Retained<NSWindow>>>,
    bar: RefCell<Option<Retained<NSWindow>>>,
    /// The sidebar and the strip as they slide in, by [`Edge`] index.
    pub(super) overlays: RefCell<Option<[Retained<NSWindow>; 2]>>,
    /// The native window buttons over the chrome: side overlay, strip
    /// overlay, frame (see [`super::lights`]).
    pub(super) lights: RefCell<Option<[Lights; 3]>>,
    pub(super) tabs: RefCell<Vec<(TabId, Retained<NSWindow>)>>,
    pub(super) active: Cell<Option<TabId>>,
    /// A solo blank tab has native Home content below its Chromium window.
    home_tab: Cell<Option<TabId>>,
    pub(super) split: super::split::Stage,
    /// The tab window currently ordered in, which can lag `active` until the
    /// active tab's window exists.
    pub(super) shown: RefCell<Option<Retained<NSWindow>>>,
    bar_visible: Cell<bool>,
    /// Where the chrome sits (Phase 12).
    pub(super) layout: Cell<Layout>,
    /// How chrome comes and goes: the theme's motion, off under Reduce
    /// Motion.
    pub(super) motion: Cell<Motion>,
    pub(super) reveal: Reveal,
    /// The sidebar sliding over the page and back.
    pub(super) fold: Fold,
    notifications: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
    /// The pinned column's width while it glides between its full width and
    /// icons only; `None` at rest, when the layout says.
    column: Cell<Option<f64>>,
}

impl Frame {
    pub fn new(window_id: u64, tx: UnboundedSender<AppMsg>) -> Rc<Self> {
        Rc::new(Self {
            window_id,
            tx,
            frame: RefCell::default(),
            bar: RefCell::default(),
            tabs: RefCell::default(),
            active: Cell::new(None),
            home_tab: Cell::new(None),
            split: super::split::Stage::default(),
            shown: RefCell::default(),
            bar_visible: Cell::new(false),
            layout: Cell::new(Layout::default()),
            motion: Cell::new(Motion::default()),
            lights: RefCell::default(),
            overlays: RefCell::default(),
            reveal: Reveal::default(),
            fold: Fold::default(),
            column: Cell::new(None),
            notifications: RefCell::default(),
        })
    }

    /// Adopts the GPUI frame window, the address bar window and the two
    /// overlay windows (sidebar, strip).
    pub fn attach(
        self: &Rc<Self>,
        frame_view: *mut c_void,
        bar_view: *mut c_void,
        overlay_views: [*mut c_void; 2],
    ) {
        let (Some(frame), Some(bar), Some(side), Some(strip)) = (
            window_of(frame_view),
            window_of(bar_view),
            window_of(overlay_views[0]),
            window_of(overlay_views[1]),
        ) else {
            tracing::error!("frame, address bar or overlay has no NSWindow");
            return;
        };
        for child in [&side, &strip, &bar] {
            // GPUI pop-ups float above every app; as children, follow
            // the frame.
            child.setLevel(frame.level());
            child.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenAuxiliary);
            child.setHasShadow(false);
            share_parent_key_state(child);
        }
        for overlay in [&side, &strip] {
            make_overlay_mouse_only(overlay);
            overlay.setAlphaValue(0.0);
            overlay.setIgnoresMouseEvents(true);
            // SAFETY: AppKit calls on the main thread.
            unsafe { frame.addChildWindow_ordered(overlay, NSWindowOrderingMode::Above) };
        }
        // A transparent panel can still become key. Keep the address panel
        // out of the window order until a shortcut actually shows it.
        bar.orderOut(None);
        self.make_lights(&frame, &side, &strip);
        *self.frame.borrow_mut() = Some(frame.clone());
        *self.bar.borrow_mut() = Some(bar);
        *self.overlays.borrow_mut() = Some([side, strip]);
        self.install_divider(&frame);
        self.observe(&frame);
        reveal::register(self);
        self.layout();
    }

    fn observe(self: &Rc<Self>, frame: &NSWindow) {
        let center = NSNotificationCenter::defaultCenter();
        let mut tokens = self.notifications.borrow_mut();
        // SAFETY: AppKit notification names; the blocks run on the main
        // thread (the posting thread) and hold the frame weakly.
        unsafe {
            // Home keeps the frame key. For web pages or the address bar,
            // the corresponding child takes the keyboard. Becoming main
            // marks the Person bringing this window forward.
            for name in [
                NSWindowDidResizeNotification,
                NSWindowDidEnterFullScreenNotification,
                NSWindowDidExitFullScreenNotification,
                NSWindowDidBecomeKeyNotification,
                NSWindowDidBecomeMainNotification,
            ] {
                let this = Rc::downgrade(self);
                let resized = std::ptr::eq(name, NSWindowDidResizeNotification);
                let activated = std::ptr::eq(name, NSWindowDidBecomeKeyNotification)
                    || std::ptr::eq(name, NSWindowDidBecomeMainNotification);
                let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                    let Some(this) = this.upgrade() else { return };
                    if !activated {
                        this.layout();
                    }
                    // A full-screen transition can leave the frame key
                    // without a fresh becomes-key, e.g. after the green
                    // button: hand the keyboard back either way.
                    if !resized {
                        this.forward_key();
                    }
                    if activated {
                        this.redraw_lights();
                        let _ = this.tx.unbounded_send(AppMsg::WindowKey {
                            window: this.window_id,
                        });
                    }
                });
                tokens.push(center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(frame),
                    None,
                    &block,
                ));
            }
            // The window buttons' colour follows the app's and any of the
            // window's family's key state.
            for name in [
                NSApplicationDidBecomeActiveNotification,
                NSApplicationDidResignActiveNotification,
                NSWindowDidBecomeKeyNotification,
                NSWindowDidResignKeyNotification,
            ] {
                let this = Rc::downgrade(self);
                let became_key = std::ptr::eq(name, NSWindowDidBecomeKeyNotification);
                let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                    if let Some(this) = this.upgrade() {
                        this.redraw_lights();
                        if became_key
                            && let Some(mtm) = MainThreadMarker::new()
                            && let Some(key) = NSApplication::sharedApplication(mtm).keyWindow()
                        {
                            this.split_keyed(&key);
                        }
                    }
                });
                tokens.push(center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    None,
                    &block,
                ));
            }
        }
    }

    /// Hands the keyboard to the visible child, or leaves it with Home.
    fn forward_key(&self) {
        // A peek is in sight: the keyboard is its page's.
        if let Some(peek) = self.peek_key_window() {
            if !peek.isKeyWindow() {
                peek.makeKeyWindow();
            }
            return;
        }
        // The find bar is up: the keyboard is the page's if it is there
        // (the Person clicked it), else the bar's. Taking it for the page
        // whenever the frame turns main sent the first ⌘F's typing away.
        if let Some(find) = self.find_window() {
            if !self.active_window().is_some_and(|tab| tab.isKeyWindow()) && !find.isKeyWindow() {
                find.makeKeyWindow();
            }
            return;
        }
        if self.bar_visible.get() {
            if let Some(bar) = self.bar.borrow().as_ref() {
                bar.makeKeyWindow();
            }
        } else if self.home_tab.get() != self.active.get()
            && let Some(tab) = self.active_window()
        {
            tab.makeKeyWindow();
        }
    }

    /// The layout in effect: full screen pins nothing (design §4.1).
    pub(super) fn layout_now(&self, frame: &NSWindow) -> Layout {
        let mut layout = self.layout.get();
        if frame.styleMask().contains(NSWindowStyleMask::FullScreen) {
            layout.sidebar_pinned = false;
            layout.top_pinned = false;
        }
        layout
    }

    /// Where the page sits beside the chrome (see [`page_insets`]): beside
    /// the pinned column as wide as it is now, mid-glide or not.
    pub(super) fn insets(&self, frame: &NSWindow) -> PageInsets {
        let fullscreen = frame.styleMask().contains(NSWindowStyleMask::FullScreen);
        let layout = self.layout.get();
        let side = self
            .column
            .get()
            .unwrap_or_else(|| pinned_side_width(&layout, frame.frame().size.width));
        page_insets(&layout, fullscreen, side)
    }

    /// Where the active tab's window goes, in screen coordinates: the whole
    /// window with nothing pinned, else a card, or the page edge to edge,
    /// beside the pinned chrome.
    pub(super) fn page_rect(&self, frame: &NSWindow) -> NSRect {
        let rect = frame.frame();
        let i = self.insets(frame);
        NSRect::new(
            NSPoint::new(rect.origin.x + i.left, rect.origin.y + i.bottom),
            NSSize::new(
                (rect.size.width - i.left - i.right).max(1.0),
                (rect.size.height - i.top - i.bottom).max(1.0),
            ),
        )
    }

    /// How chrome moves in this window: the theme's motion, and none at
    /// all under "Reduce animations".
    pub(super) fn motion_now(&self) -> Motion {
        if self.layout.get().reduce_animations {
            Motion::Off
        } else {
            self.motion.get()
        }
    }

    /// Takes a new layout: refits the page, the overlays and the bar.
    pub fn set_layout(&self, layout: Layout) {
        let previous = self.layout.replace(layout);
        if layout.sidebar_pinned {
            self.fold.folded.set(false);
        }
        if previous == layout {
            return;
        }
        self.hold_page(&previous, &layout);
        self.hide_chrome();
        self.layout();
    }

    /// A column that glides between its full width and icons only
    /// ([`column_glides`]) is drawn by the frame view, which tells where it
    /// is on every frame ([`Frame::set_column`]): the page stays at the old
    /// width until the view's first report, so it does not jump to the new one and
    /// back. The view always ends the glide, even when it does not make
    /// one.
    fn hold_page(&self, before: &Layout, after: &Layout) {
        if !after.sidebar_pinned {
            self.column.set(None);
        } else if self.column.get().is_none()
            && column_glides(before, after)
            && let Some(frame) = self.frame.borrow().as_ref()
        {
            let width = frame.frame().size.width;
            self.column.set(Some(before.column_width(width)));
        }
    }

    /// The pinned column is `width` points wide on this frame of its glide,
    /// and the page and the address bar follow it; `None` ends the glide and
    /// fits everything to the layout.
    pub(super) fn set_column(&self, width: Option<f64>) {
        let previous = self.column.replace(width);
        if width.is_none() {
            if previous.is_some() {
                self.layout();
            }
        } else if let Some(frame) = self.frame.borrow().clone() {
            self.layout_panes(&frame);
        }
    }

    /// Takes the theme's motion for sliding chrome.
    pub fn set_motion(&self, motion: Motion) {
        self.motion.set(motion);
    }

    pub(super) fn active_window(&self) -> Option<Retained<NSWindow>> {
        let active = self.active.get()?;
        self.tabs
            .borrow()
            .iter()
            .find(|(id, _)| *id == active)
            .map(|(_, w)| w.clone())
    }

    /// Resizes only the live panes and their controls along a drag or glide.
    pub(super) fn layout_panes(&self, frame: &NSWindow) {
        let page = self.page_rect(frame);
        let radius = self.insets(frame).radius;
        self.sync_secondary(frame, page, radius);
        let focused = self.focused_rect(page);
        if let Some(window) = self.active_window()
            && window.frame() != focused
        {
            window.setFrame_display(focused, true);
        }
        self.place_page_controls(focused);
    }

    fn place_page_controls(&self, focused: NSRect) {
        if let Some(bar) = self.bar.borrow().as_ref()
            && bar.frame() != focused
        {
            bar.setFrame_display(focused, self.bar_visible.get());
        }
        self.place_find(focused);
    }

    /// Fits every tab window to the page, places the overlays along their
    /// edges and the address bar over the page.
    pub fn layout(&self) {
        let Some(frame) = self.frame.borrow().clone() else {
            return;
        };
        let page = self.page_rect(&frame);
        let radius = self.insets(&frame).radius;
        self.sync_secondary(&frame, page, radius);
        let focused = self.focused_rect(page);
        let active = self.active.get();
        for (id, tab) in self.tabs.borrow().iter() {
            if self
                .split
                .secondary
                .borrow()
                .as_ref()
                .is_some_and(|peer| super::split::same(peer, tab))
            {
                continue;
            }
            let visible = active == Some(*id);
            let rect = if visible { focused } else { page };
            if tab.frame() != rect {
                tab.setFrame_display(rect, visible);
            }
            round_page(tab, radius);
        }
        if let Some([side, strip]) = self.overlays.borrow().as_ref() {
            let rect = frame.frame();
            let layout = self.layout_now(&frame);
            // A pinned strip stays; the sliding sidebar comes in below it.
            let strip_h = strip_height(&layout);
            let below = if layout.top_pinned { strip_h } else { 0.0 };
            let width = layout.column_width(rect.size.width) + SIDE_OVERLAY_EXTRA;
            let x = match layout.side {
                Side::Left => rect.origin.x,
                Side::Right => rect.origin.x + rect.size.width - width,
            };
            let side_rect = NSRect::new(
                NSPoint::new(x, rect.origin.y),
                NSSize::new(width, (rect.size.height - below).max(1.0)),
            );
            let strip_rect = NSRect::new(
                NSPoint::new(
                    rect.origin.x,
                    reveal::TopLane::new(&frame, rect).strip_top - strip_h,
                ),
                NSSize::new(rect.size.width, strip_h),
            );
            // Out of sight they are refitted without being drawn, and left
            // alone when they are already there (a live resize comes here
            // on every frame).
            for (window, wanted, shown) in [
                (
                    side,
                    side_rect,
                    self.reveal.shown[reveal::Edge::Side as usize].get(),
                ),
                (
                    strip,
                    strip_rect,
                    self.reveal.shown[reveal::Edge::Top as usize].get(),
                ),
            ] {
                if window.frame() != wanted {
                    window.setFrame_display(wanted, shown);
                }
            }
        }
        self.replace_side();
        self.place_lights(&frame);
        self.place_page_controls(focused);
        self.place_stage(page);
        self.place_peek(page);
        self.place_switcher(frame.frame());
    }

    pub(crate) fn home_tab(&self) -> Option<TabId> {
        self.home_tab.get()
    }

    /// Keep Chromium available for navigation, but let Home own drawing and input
    /// while the active, unsplit tab still points to about:blank.
    pub(crate) fn set_home_tab(&self, home: Option<TabId>) {
        let previous = self.home_tab.replace(home);
        let active = self.active.get();
        if previous == home || (active != home && active != previous) {
            return;
        }
        let (Some(frame), Some(page)) = (self.frame.borrow().clone(), self.active_window()) else {
            return;
        };
        let had_key = self.is_key();
        let show_home = active == home;
        self.order_page(&frame, &page, show_home);
        self.raise_children();
        if had_key && !self.bar_visible.get() {
            if show_home {
                frame.makeKeyWindow();
            } else {
                page.makeKeyWindow();
            }
        }
    }

    fn order_page(&self, frame: &NSWindow, page: &NSWindow, home: bool) {
        if home {
            // Keep the browser alive for navigation without letting a
            // transparent CEF window take the keyboard from native Home.
            if let Some(parent) = page.parentWindow() {
                parent.removeChildWindow(page);
            }
            page.orderOut(None);
        } else {
            // SAFETY: AppKit calls on the main thread.
            unsafe { frame.addChildWindow_ordered(page, NSWindowOrderingMode::Above) };
            page.orderFront(None);
        }
    }

    /// Shows `tab`'s window over the frame and hides the previous one.
    pub fn activate(&self, tab: TabId) {
        self.active.set(Some(tab));
        // A video floating from this tab lands, a picture in picture ends.
        self.float_arrive(tab);
        let (Some(frame), Some(next)) = (self.frame.borrow().clone(), self.active_window()) else {
            // Not created yet: `created` activates it.
            self.float_after_activate();
            return;
        };
        let home = self.home_tab.get() == Some(tab);
        let page = self.page_rect(&frame);
        self.sync_secondary(&frame, page, self.insets(&frame).radius);
        next.setFrame_display(self.focused_rect(page), !home);
        self.order_page(&frame, &next, home);
        let previous = self.shown.replace(Some(next.clone()));
        if let Some(previous) = previous.filter(|p| {
            !super::split::same(p, &next)
                && !self
                    .split
                    .secondary
                    .borrow()
                    .as_ref()
                    .is_some_and(|peer| super::split::same(peer, p))
        }) {
            // A tab that may float waits where it is for the page's answer.
            if !self.float_depart_window(&previous) {
                frame.removeChildWindow(&previous);
                previous.orderOut(None);
            }
        }
        self.layout();
        self.raise_children();
        if !self.bar_visible.get() {
            if home {
                frame.makeKeyWindow();
            } else {
                next.makeKeyWindow();
            }
        }
        self.float_after_activate();
    }

    /// Keeps the overlays and then the address bar above whichever tab
    /// window is in front.
    pub(super) fn raise_children(&self) {
        let Some(frame) = self.frame.borrow().clone() else {
            return;
        };
        let overlays = self.overlays.borrow().clone();
        let bar = self.bar.borrow().clone();
        let find = self.find_window();
        let switcher = self.switcher_window();
        let peek = self.peek_windows();
        for child in peek
            .iter()
            .chain(find.iter())
            .chain(overlays.iter().flatten())
            .chain(bar.iter().filter(|_| self.bar_visible.get()))
            .chain(switcher.iter())
        {
            frame.removeChildWindow(child);
            // SAFETY: AppKit call on the main thread.
            unsafe { frame.addChildWindow_ordered(child, NSWindowOrderingMode::Above) };
        }
    }

    /// Shows the address bar and gives it the keyboard.
    pub fn show_bar(&self) {
        let (Some(frame), Some(bar)) = (self.frame.borrow().clone(), self.bar.borrow().clone())
        else {
            return;
        };
        if !self.bar_visible.replace(true) {
            // SAFETY: AppKit calls on the main thread.
            unsafe { frame.addChildWindow_ordered(&bar, NSWindowOrderingMode::Above) };
            bar.orderFront(None);
        }
        bar.makeKeyWindow();
    }

    /// Hides the address bar at once (Zen closes its bar without motion).
    /// With `focus_page`, the page takes the keyboard.
    pub fn hide_bar(&self, focus_page: bool) {
        if !self.bar_visible.replace(false) {
            return;
        }
        let Some(bar) = self.bar.borrow().clone() else {
            return;
        };
        if let Some(frame) = bar.parentWindow() {
            frame.removeChildWindow(&bar);
        }
        bar.orderOut(None);
        if focus_page {
            if self.home_tab.get().is_some() && self.home_tab.get() == self.active.get() {
                if let Some(frame) = self.frame.borrow().as_ref() {
                    frame.makeKeyWindow();
                }
            } else if let Some(tab) = self.active_window() {
                tab.makeKeyWindow();
            }
        }
    }

    pub fn toggle_fullscreen(&self) {
        if let Some(frame) = self.frame.borrow().as_ref() {
            frame.toggleFullScreen(None);
        }
    }

    /// Forgets a tab window that is leaving: its space is no longer shown
    /// or it moves to another window. A video playing in it may float.
    pub fn remove_tab(&self, tab: TabId) {
        self.drop_tab_window(tab, true);
    }

    /// Forgets a tab window; `leaving`: it is still alive, so it may
    /// float out of the frame before it goes.
    fn drop_tab_window(&self, tab: TabId, leaving: bool) {
        self.peek_origin_gone(tab);
        let removed = {
            let mut tabs = self.tabs.borrow_mut();
            tabs.iter()
                .position(|(id, _)| *id == tab)
                .map(|index| tabs.remove(index).1)
        };
        let Some(window) = removed else { return };
        let mut shown = self.shown.borrow_mut();
        let was_shown = shown.as_ref().is_some_and(|s| std::ptr::eq(&**s, &*window));
        if was_shown {
            shown.take();
        }
        drop(shown);
        if self
            .split
            .secondary
            .borrow()
            .as_ref()
            .is_some_and(|peer| super::split::same(peer, &window))
        {
            self.split.secondary.take();
        }
        if was_shown && leaving && self.float_depart(tab, &window) {
            return;
        }
        if let Some(frame) = self.frame.borrow().as_ref() {
            frame.removeChildWindow(&window);
        }
        window.orderOut(None);
        self.layout();
    }

    /// Brings the window to the front and gives it the keyboard, out of the
    /// Dock if it was in it.
    pub fn bring_forward(&self) {
        if let Some(frame) = self.frame.borrow().as_ref() {
            if frame.isMiniaturized() {
                frame.deminiaturize(None);
            }
            frame.makeKeyAndOrderFront(None);
        }
    }

    /// Whether this window, or one of its tab or bar windows, is key.
    pub fn is_key(&self) -> bool {
        let Some(frame) = self.frame.borrow().clone() else {
            return false;
        };
        let Some(key) =
            NSApplication::sharedApplication(MainThreadMarker::from(&*frame)).keyWindow()
        else {
            return false;
        };
        std::ptr::eq(&*key, &*frame)
            || key
                .parentWindow()
                .is_some_and(|parent| std::ptr::eq(&*parent, &*frame))
    }

    /// Whether the active Chromium page, rather than a chrome overlay, has the keyboard.
    pub(crate) fn page_has_key(&self) -> bool {
        self.active_window().is_some_and(|page| {
            NSApplication::sharedApplication(MainThreadMarker::from(&*page))
                .keyWindow()
                .is_some_and(|key| std::ptr::eq(&*page, &*key))
        })
    }

    /// The Window menu operates on the frame, not the focused child page.
    pub(crate) fn window_menu(&self, minimize: bool) {
        if let Some(frame) = self.frame.borrow().clone() {
            if minimize {
                frame.performMiniaturize(None);
            } else {
                frame.performZoom(None);
            }
        }
    }

    /// Pops up a native menu at (`x`, `y`) points from the frame's top-left.
    pub fn pop_up_menu(
        &self,
        x: f64,
        y: f64,
        align: native_menu::Align,
        entries: &[native_menu::Entry],
        on_pick: impl Fn(usize) + 'static,
    ) {
        let Some(view) = self.frame.borrow().as_ref().and_then(|f| f.contentView()) else {
            return;
        };
        native_menu::pop_up(&view, x, y, align, entries, on_pick);
    }

    /// Releases AppKit hooks once the window is closing.
    pub fn detach(&self) {
        let center = NSNotificationCenter::defaultCenter();
        for token in self.notifications.borrow_mut().drain(..) {
            // SAFETY: removing our own observer tokens on the main thread.
            unsafe { center.removeObserver(token.as_ref()) };
        }
        self.remove_lights();
        self.forget_stage();
    }
}

/// What a double-click on a title bar does, per System Settings › Desktop &
/// Dock (Search `Strip.mouseUp`, `Stage.swift`): `AppleActionOnDoubleClick`
/// is "Minimize" (miniaturize), "None" (nothing) or, unset or anything
/// else, "Maximize" (zoom). Unset, the older `AppleMiniaturizeOnDoubleClick`
/// still asks for a miniaturize. Full screen has no title bar to click.
pub(super) fn double_click_action(window: &NSWindow) {
    if window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        return;
    }
    let defaults = NSUserDefaults::standardUserDefaults();
    let action = defaults.stringForKey(ns_string!("AppleActionOnDoubleClick"));
    match action.as_deref().map(ToString::to_string).as_deref() {
        Some("Minimize") => window.miniaturize(None),
        Some("None") => {}
        Some(_) => window.zoom(None),
        None if defaults.boolForKey(ns_string!("AppleMiniaturizeOnDoubleClick")) => {
            window.miniaturize(None);
        }
        None => window.zoom(None),
    }
}

/// Shows or hides the frame's own close, minimise and zoom buttons
/// ([`Frame::place_lights`] decides).
pub(super) fn show_window_buttons(frame: &NSWindow, shown: bool) {
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = frame.standardWindowButton(kind) {
            button.setHidden(!shown);
            button.setAlphaValue(1.0);
        }
    }
}

/// Puts the frame's own buttons out of sight while the stacked group
/// stands in for them ([`super::lights`]). The zoom button is only made
/// transparent: AppKit hides every standalone zoom button of a window whose
/// own one is hidden. It stays where GPUI keeps it, under the page.
pub(super) fn stand_in_window_buttons(frame: &NSWindow) {
    show_window_buttons(frame, true);
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
    ] {
        if let Some(button) = frame.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
    if let Some(zoom) = frame.standardWindowButton(NSWindowButton::ZoomButton) {
        zoom.setAlphaValue(0.0);
    }
}

/// Makes a tab window a card like Search's: every corner rounded (by
/// [`round_page`]; the page's scrollbars are overlays, see
/// [`prefer_overlay_scrollers`](super::prefer_overlay_scrollers), so the
/// corners clip nothing), clear around them, no shadow of its own (the
/// frame casts the window's). CEF's
/// frameless windows are still titled, and macOS rounds a titled window's
/// corners its own (wider) way, so drop the title bar.
pub(super) fn style_card(window: &NSWindow) {
    window.setStyleMask(NSWindowStyleMask::Borderless);
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
    let Some(view) = window.contentView() else {
        return;
    };
    view.setWantsLayer(true);
    // SAFETY: CALayer messages on the main thread; `layer` is live while the
    // view is, and a layer-backed view always has one.
    unsafe {
        let layer: *mut AnyObject = msg_send![&*view, layer];
        if let Some(layer) = layer.as_ref() {
            // `kCACornerCurveContinuous`, Search's `.continuous` style.
            let curve = objc2_foundation::ns_string!("continuous");
            let _: () = msg_send![layer, setCornerCurve: curve];
            let _: () = msg_send![layer, setMasksToBounds: true];
        }
    }
}

/// The page's corner radius: 10 as a card, 0 edge to edge (Search
/// `Metrics.cardRadius`, passed as `cards ? 10 : 0`, `Stage.swift`).
pub(super) fn round_page(window: &NSWindow, radius: f64) {
    let Some(view) = window.contentView() else {
        return;
    };
    // SAFETY: a CALayer message on the main thread; `layer` is live while
    // the view is.
    unsafe {
        let layer: *mut AnyObject = msg_send![&*view, layer];
        if let Some(layer) = layer.as_ref() {
            let _: () = msg_send![layer, setCornerRadius: radius];
        }
    }
}

/// The slide-out chrome has no editor. GPUI's panel class is key and main
/// eligible even when transparent; these two overlays must not take focus
/// from the frame, page, or floating address bar.
fn make_overlay_mouse_only(window: &NSWindow) {
    extern "C-unwind" fn no(_: &AnyObject, _: Sel) -> Bool {
        Bool::NO
    }

    let window: &AnyObject = window;
    let class = AnyClass::get(c"AeriChromeOverlayPanel").unwrap_or_else(|| {
        let mut class = ClassBuilder::new(c"AeriChromeOverlayPanel", window.class())
            .expect("could not create Aeri chrome overlay subclass");
        // SAFETY: Both AppKit selectors return BOOL and take no arguments.
        unsafe {
            class.add_method(
                sel!(canBecomeKeyWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
            class.add_method(
                sel!(canBecomeMainWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
        }
        class.register()
    });
    // SAFETY: The subclass adds no fields or destruction behavior.
    unsafe { AnyObject::set_class(window, class) };
}

extern "C-unwind" fn shares_parent_key_state(
    this: *mut AnyObject,
    _sel: objc2::runtime::Sel,
) -> objc2::runtime::Bool {
    // SAFETY: AppKit sends this to a live window on the main thread.
    let parent: *mut AnyObject = unsafe { msg_send![this, parentWindow] };
    objc2::runtime::Bool::new(!parent.is_null())
}

/// While a child window of `window`'s class (a tab or the address bar) is
/// key, its parent keeps the key appearance, so the frame's traffic lights
/// stay coloured. Overrides AppKit's private `_sharesParentKeyState` (NO by
/// default; Chromium uses the same hook for its child widgets) once per
/// class; windows without a parent are unaffected.
pub(super) fn share_parent_key_state(window: &NSWindow) {
    // SAFETY: objc runtime calls on the main thread; the IMP matches the
    // `B@:` encoding (BOOL return, id self, SEL).
    unsafe {
        let class = objc2::ffi::object_getClass((window as *const NSWindow).cast());
        let imp = std::mem::transmute::<*const c_void, unsafe extern "C-unwind" fn()>(
            shares_parent_key_state as *const c_void,
        );
        // Fails harmlessly when the class already has it (a later window).
        objc2::ffi::class_addMethod(
            class as *mut objc2::runtime::AnyClass,
            sel!(_sharesParentKeyState),
            imp,
            c"B@:".as_ptr(),
        );
    }
}

impl TabObserver for Frame {
    fn created(&self, tab: TabId, content_view: *mut c_void) {
        // A floating video's page moved here from another window.
        if self.float_rehome(tab) {
            return;
        }
        let Some(window) = window_of(content_view) else {
            return;
        };
        window.setExcludedFromWindowsMenu(true);
        window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenAuxiliary);
        style_card(&window);
        share_parent_key_state(&window);
        // A tab moving in from another window may have been here before.
        let mut tabs = self.tabs.borrow_mut();
        tabs.retain(|(id, _)| *id != tab);
        tabs.push((tab, window));
        drop(tabs);
        self.layout();
        if self.active.get() == Some(tab) {
            self.activate(tab);
        }
        // A tab's window may have taken the keyboard before it was ours.
        self.redraw_lights();
    }

    fn destroyed(&self, tab: TabId) {
        super::float::forget(tab);
        self.drop_tab_window(tab, false);
        let _ = self.tx.unbounded_send(AppMsg::TabClosed {
            window: self.window_id,
            tab,
        });
    }
}
