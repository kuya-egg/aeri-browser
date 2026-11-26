//! Page-first chrome (Phase 12, design §4.2): unpinned chrome slides in
//! over the page when the pointer rests at its edge, or when called from
//! the keyboard, and goes away once the pointer has left it. The numbers
//! are Search's (`Fold.swift`, see `search-sidebar-layout.md` §5): a 6 pt
//! lane at the window's side edge, which a pointer that came from inside
//! keeps up to 48 pt beyond it; no dwell for a sidebar folded with ⌘S, a
//! dwell of 0.15 s for a layout that has none pinned; a grace of 0.065 s
//! before it goes back; while out its reach is its width. The sidebar's
//! slide itself is in [`super::fold`]; the top strip fades and slides in
//! here.
//!
//! One app-local event monitor watches mouse moves and clicks in Aeri's
//! own windows (never consuming an event). The existing global mouse-move
//! monitor bridges the side overshoot and the system fullscreen menu band.
//! Each window keeps its own one-shot open and close timers. Nothing runs while
//! the mouse is still.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplication,
    NSApplicationDidResignActiveNotification, NSEvent, NSEventMask, NSEventType, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSRect, NSTimer};

use super::frame::Frame;
use crate::window_frame::strip_height;
use aeri_core::appearance::Motion;
use aeri_core::layout::{Layout, Side, TabPlace};

/// Width of the side edge lane (Search `Fold.edge`), and of the top one.
const EDGE: f64 = 6.0;
const TOP_EDGE: f64 = 8.0;
/// How far beyond the window's side edge a pointer that came from inside
/// still counts as at the edge (Search `Fold.overshoot`).
const OVERSHOOT: f64 = 48.0;
/// How long the pointer rests at the edge before chrome that is not folded
/// for good opens (Search `Fold.dwell`).
const DWELL: f64 = 0.15;
/// How long after the pointer leaves before the chrome goes back (Search
/// `Fold.grace`).
const GRACE: f64 = 0.065;
/// How far the top strip travels as it slides in, in points.
const SLIDE: f64 = 14.0;

/// An edge chrome slides in from; the index of its overlay window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Side = 0,
    Top = 1,
}

const EDGES: [Edge; 2] = [Edge::Top, Edge::Side];

/// The pointer can enter from the physical display edge in full screen;
/// the strip itself begins inside usable content, below the system menu.
pub(super) struct TopLane {
    edge: f64,
    pub(super) strip_top: f64,
}

impl TopLane {
    pub(super) fn new(frame: &NSWindow, rect: NSRect) -> Self {
        let top = rect.origin.y + rect.size.height;
        let fullscreen = frame.styleMask().contains(NSWindowStyleMask::FullScreen);
        if fullscreen && let Some(screen) = frame.screen() {
            let bounds = screen.frame();
            let visible = screen.visibleFrame();
            let screen_top = bounds.origin.y + bounds.size.height;
            let safe_top = screen_top - screen.safeAreaInsets().top;
            let ceiling = top
                .min(safe_top)
                .min(visible.origin.y + visible.size.height);
            let usable = frame.convertRectToScreen(frame.contentLayoutRect());
            // AppKit reveals a native title bar below the menu, over a
            // full-size content view. Give it room even before it appears.
            let style = frame.styleMask() & NSWindowStyleMask::Titled;
            let mtm = MainThreadMarker::from(frame);
            let content = NSWindow::contentRectForFrameRect_styleMask(rect, style, mtm);
            let title_height = rect.size.height - content.size.height;
            return Self {
                edge: screen_top,
                strip_top: (ceiling - title_height).min(usable.origin.y + usable.size.height),
            };
        }
        Self {
            edge: top,
            strip_top: top,
        }
    }

    fn hits(&self, y: f64, strip_height: f64) -> (bool, bool) {
        (
            (self.strip_top - TOP_EDGE..=self.edge).contains(&y),
            (self.strip_top - strip_height..=self.edge).contains(&y),
        )
    }
}

/// A one-shot timer, and whether it opens (else closes).
type Pending = Option<(bool, Retained<NSTimer>)>;

#[derive(Default)]
pub struct Reveal {
    pub(super) shown: [Cell<bool>; 2],
    /// Called from the keyboard: stays until the pointer has been in and
    /// out, a click on the page, or another call.
    called: [Cell<bool>; 2],
    entered: [Cell<bool>; 2],
    timer: [RefCell<Pending>; 2],
    /// The pointer is over one of the GPUI chrome windows.
    pointer_in_chrome: Cell<bool>,
    /// The pointer is in the window, or just past its side edge having
    /// come from inside.
    inside: Cell<bool>,
}

thread_local! {
    static FRAMES: RefCell<Vec<Weak<Frame>>> = RefCell::default();
    static HOOKS: RefCell<Vec<Retained<AnyObject>>> = RefCell::default();
    static RESIGN: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>> =
        RefCell::default();
}

/// Starts watching `frame`'s windows; the first call installs the monitors.
pub(super) fn register(frame: &Rc<Frame>) {
    frame.fold.bind(frame);
    FRAMES.with_borrow_mut(|frames| {
        frames.retain(|f| f.strong_count() > 0);
        frames.push(Rc::downgrade(frame));
    });
    HOOKS.with_borrow_mut(|hooks| {
        if hooks.is_empty() {
            hooks.extend(install());
        }
    });
}

fn install() -> Vec<Retained<AnyObject>> {
    let local = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a live event for the handler's duration.
        on_event(unsafe { event.as_ref() });
        event.as_ptr()
    });
    let global = RcBlock::new(|_: NonNull<NSEvent>| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        if !app.isActive() {
            return;
        }
        let key = app.keyWindow().or_else(|| app.mainWindow());
        for frame in frames() {
            if frame.is_fullscreen() && !key.as_ref().is_some_and(|key| frame.owns(key)) {
                continue;
            }
            frame.pointer(NSEventType::MouseMoved, None);
        }
    });
    // A drag (a tab, a selection) never opens or closes chrome: not heard.
    let mask = NSEventMask::MouseMoved | NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown;
    // Leaving the app hides whatever slid in.
    let resign = RcBlock::new(|_: NonNull<NSNotification>| {
        for frame in frames() {
            frame.note_chrome_pointer(None);
            frame.reveal.inside.set(false);
            frame.hide_chrome();
        }
    });
    // SAFETY: AppKit calls on the main thread; the handlers run on the main
    // thread and the local one returns the event it was given.
    unsafe {
        RESIGN.set(Some(
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidResignActiveNotification),
                None,
                None,
                &resign,
            ),
        ));
        [
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local),
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
                NSEventMask::MouseMoved,
                &global,
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

pub(super) fn frames() -> Vec<Rc<Frame>> {
    FRAMES.with_borrow(|frames| frames.iter().filter_map(Weak::upgrade).collect())
}

fn on_event(event: &NSEvent) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let window = event.window(mtm);
    let kind = event.r#type();
    let frame = FRAMES.with_borrow(|frames| {
        frames
            .iter()
            .filter_map(Weak::upgrade)
            .find(|f| window.as_ref().is_some_and(|window| f.owns(window)))
    });
    if let Some(frame) = frame {
        frame.pointer(kind, window.as_deref());
    } else if kind == NSEventType::MouseMoved {
        // AppKit's own fullscreen title bar isn't one of our child
        // windows, but the pointer is still approaching the top tabs.
        let app = NSApplication::sharedApplication(mtm);
        if app.isActive()
            && let Some(key) = app.keyWindow().or_else(|| app.mainWindow())
            && let Some(front) = FRAMES.with_borrow(|frames| {
                frames
                    .iter()
                    .filter_map(Weak::upgrade)
                    .find(|f| f.owns(&key) && f.is_fullscreen())
            })
        {
            front.pointer(kind, None);
        }
    }
}

impl Frame {
    fn is_fullscreen(&self) -> bool {
        self.frame
            .borrow()
            .as_ref()
            .is_some_and(|frame| frame.styleMask().contains(NSWindowStyleMask::FullScreen))
    }

    /// The frame itself or one of its child windows.
    fn owns(&self, window: &NSWindow) -> bool {
        let Some(frame) = self.frame.borrow().clone() else {
            return false;
        };
        std::ptr::eq(window, &*frame)
            || window
                .parentWindow()
                .is_some_and(|parent| std::ptr::eq(&*parent, &*frame))
    }

    fn is_page(&self, window: Option<&NSWindow>) -> bool {
        window.is_some_and(|window| {
            self.tabs
                .borrow()
                .iter()
                .any(|(_, tab)| std::ptr::eq(&**tab, window))
        })
    }

    fn note_chrome_pointer(&self, target: Option<&NSWindow>) {
        let over = target.is_some_and(|target| {
            self.frame
                .borrow()
                .as_ref()
                .is_some_and(|frame| std::ptr::eq(&**frame, target))
                || self.overlays.borrow().as_ref().is_some_and(|overlays| {
                    overlays
                        .iter()
                        .any(|overlay| std::ptr::eq(&**overlay, target))
                })
        });
        if self.reveal.pointer_in_chrome.replace(over) && !over {
            let _ = self.tx.unbounded_send(crate::shell::AppMsg::Chrome(
                crate::shell::ChromeMsg::PointerLeft {
                    window: self.window_id,
                },
            ));
        }
    }

    pub(crate) fn pointer_in_chrome(&self) -> bool {
        self.reveal.pointer_in_chrome.get()
    }

    /// The pointer moved or clicked, in one of this window's windows or (a
    /// move, `window` none) elsewhere on screen.
    fn pointer(self: &Rc<Self>, kind: NSEventType, window: Option<&NSWindow>) {
        self.note_chrome_pointer(window);
        let Some(frame) = self.frame.borrow().clone() else {
            return;
        };
        // Pinned chrome has nothing to open: the default layout does no work
        // for the pointer, which moves hundreds of times a second.
        let layout = self.layout_now(&frame);
        if !EDGES.into_iter().any(|edge| slides(&layout, edge)) {
            return;
        }
        let rect = frame.frame();
        let at = NSEvent::mouseLocation();
        let y = rect.origin.y + rect.size.height - at.y;
        // How far the pointer is inside the sidebar's own window edge,
        // whichever side it is on; negative beyond it.
        let x = match layout.side {
            Side::Left => at.x - rect.origin.x,
            Side::Right => rect.origin.x + rect.size.width - at.x,
        };
        let tall = y >= 0.0 && y <= rect.size.height;
        let in_window = tall && x >= 0.0 && x <= rect.size.width;
        // Past the edge, a pointer that came from inside keeps counting.
        if !in_window && !(tall && (-OVERSHOOT..0.0).contains(&x)) {
            self.reveal.inside.set(false);
        } else if in_window {
            self.reveal.inside.set(true);
        }
        let at_edge = |reach: f64| x <= reach && (x >= 0.0 || self.reveal.inside.get());
        let clicked = kind != NSEventType::MouseMoved;
        let on_page = self.is_page(window);
        let strip_h = strip_height(&layout);
        let side_top = if layout.top_pinned { strip_h } else { 0.0 };
        let top_hit = if slides(&layout, Edge::Top) && (0.0..=rect.size.width).contains(&x) {
            TopLane::new(&frame, rect).hits(at.y, strip_h)
        } else {
            (false, false)
        };
        let reach = layout.column_width(rect.size.width);
        for edge in EDGES {
            if !slides(&layout, edge) {
                continue;
            }
            // A sidebar sliding out to be pinned is not the pointer's.
            if edge == Edge::Side && self.is_unfolding() {
                continue;
            }
            // In the shared corner the top wins.
            let (lane, region) = match edge {
                Edge::Top => top_hit,
                Edge::Side => {
                    let below = tall && y >= side_top && !top_hit.0;
                    (below && at_edge(EDGE), below && at_edge(reach))
                }
            };
            let i = edge as usize;
            if !self.reveal.shown[i].get() {
                if lane && !clicked {
                    self.schedule(edge, true);
                } else {
                    self.cancel(edge);
                }
            } else if region && !(edge == Edge::Side && self.reveal.shown[1].get() && top_hit.1) {
                self.reveal.entered[i].set(true);
                self.cancel(edge);
            } else if clicked && on_page {
                self.set_revealed(edge, false);
            } else if !self.reveal.called[i].get() || self.reveal.entered[i].get() {
                self.schedule(edge, false);
            }
        }
    }

    /// The keyboard calls (or sends away) the chrome at `edge`. Returns
    /// false when that chrome is pinned, so nothing slides.
    pub fn call(self: &Rc<Self>, edge: Edge) -> bool {
        let Some(frame) = self.frame.borrow().clone() else {
            return false;
        };
        if !slides(&self.layout_now(&frame), edge) {
            return false;
        }
        let i = edge as usize;
        let show = !self.reveal.shown[i].get();
        self.set_revealed(edge, show);
        if show {
            self.reveal.called[i].set(true);
        }
        true
    }

    /// Sends away whatever slid in.
    pub fn hide_chrome(&self) {
        for edge in EDGES {
            self.set_revealed(edge, false);
        }
    }

    /// Forgets that the sidebar was called or peeked at, with no slide:
    /// the fold takes it from here.
    pub(super) fn quiet_side(&self) {
        self.cancel(Edge::Side);
        self.reveal.shown[Edge::Side as usize].set(false);
        self.reveal.called[Edge::Side as usize].set(false);
        self.reveal.entered[Edge::Side as usize].set(false);
    }

    fn set_revealed(&self, edge: Edge, shown: bool) {
        let i = edge as usize;
        self.cancel(edge);
        self.reveal.called[i].set(false);
        self.reveal.entered[i].set(false);
        if self.reveal.shown[i].replace(shown) == shown {
            return;
        }
        tracing::debug!(window = self.window_id, ?edge, shown, "chrome slides");
        if edge == Edge::Side {
            self.glide(shown);
            return;
        }
        let Some(overlay) = self.overlays.borrow().as_ref().map(|o| o[i].clone()) else {
            return;
        };
        overlay.setIgnoresMouseEvents(!shown);
        crate::chrome_overlay::set_overlay_shown(self.window_id, edge, shown);
        let duration = match self.motion_now() {
            Motion::Off => {
                overlay.setAlphaValue(if shown { 1.0 } else { 0.0 });
                return;
            }
            Motion::Subtle => 0.12,
            Motion::Standard => 0.18,
        };
        // Standard motion slides in from the edge; leaving only fades, so
        // the overlay ends where the layout put it. In full screen a slide
        // would pass through the system menu and title bar.
        let target = overlay.frame();
        let fullscreen = self
            .frame
            .borrow()
            .as_ref()
            .is_some_and(|frame| frame.styleMask().contains(NSWindowStyleMask::FullScreen));
        let slide = shown && self.motion_now() == Motion::Standard && !fullscreen;
        if slide {
            let mut start = target;
            start.origin.y += SLIDE;
            overlay.setFrame_display(start, false);
        }
        let changes = RcBlock::new(move |context: NonNull<NSAnimationContext>| {
            // SAFETY: AppKit passes the live context for this group.
            unsafe { context.as_ref() }.setDuration(duration);
            let animator = overlay.animator();
            animator.setAlphaValue(if shown { 1.0 } else { 0.0 });
            if slide {
                animator.setFrame_display(target, true);
            }
        });
        NSAnimationContext::runAnimationGroup(&changes);
    }

    /// How long the pointer rests at `edge` before it opens: not at all for
    /// a sidebar folded with ⌘S, else a dwell.
    fn dwell(&self, edge: Edge) -> f64 {
        if edge == Edge::Side && self.fold.folded.get() {
            0.0
        } else {
            DWELL
        }
    }

    fn schedule(self: &Rc<Self>, edge: Edge, open: bool) {
        let i = edge as usize;
        if self.reveal.timer[i]
            .borrow()
            .as_ref()
            .is_some_and(|(opens, _)| *opens == open)
        {
            return;
        }
        self.cancel(edge);
        let after = if open { self.dwell(edge) } else { GRACE };
        if after == 0.0 {
            self.set_revealed(edge, open);
            return;
        }
        let this = Rc::downgrade(self);
        let block = RcBlock::new(move |_: NonNull<NSTimer>| {
            if let Some(this) = this.upgrade() {
                this.reveal.timer[i].borrow_mut().take();
                this.set_revealed(edge, open);
            }
        });
        // SAFETY: a one-shot timer on the main run loop; the block holds
        // the frame weakly.
        let timer =
            unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(after, false, &block) };
        *self.reveal.timer[i].borrow_mut() = Some((open, timer));
    }

    fn cancel(&self, edge: Edge) {
        if let Some((_, timer)) = self.reveal.timer[edge as usize].borrow_mut().take() {
            timer.invalidate();
        }
    }
}

/// Whether the chrome at `edge` slides in under this layout: it is not
/// pinned. As in Search, a strip only exists for tabs along the top: with
/// tabs in the sidebar, the sidebar's band carries the lights and history,
/// and nothing comes down from the top edge.
fn slides(layout: &Layout, edge: Edge) -> bool {
    match edge {
        Edge::Side => !layout.sidebar_pinned,
        Edge::Top => !layout.top_pinned && layout.tabs == TabPlace::Top,
    }
}

#[cfg(test)]
mod tests {
    use super::TopLane;

    #[test]
    fn fullscreen_top_lane_bridges_the_menu_and_keeps_the_tabs_open() {
        let top = TopLane {
            edge: 1169.0,
            strip_top: 1098.0,
        };
        assert_eq!(top.hits(1168.0, 52.0), (true, true));
        assert_eq!(top.hits(1120.0, 52.0), (true, true));
        assert_eq!(top.hits(1094.0, 52.0), (true, true));
        assert_eq!(top.hits(1076.0, 52.0), (false, true));
        assert_eq!(top.hits(1030.0, 52.0), (false, false));
        assert_eq!(top.hits(1170.0, 52.0), (false, false));
    }
}
