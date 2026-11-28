//! The gap between two CEF child windows belongs to the GPUI frame. A native
//! view in that gap can track a drag even as the child windows move under it.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use aeri_core::ids::TabId;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCursor, NSEvent, NSView, NSWindow, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::frame::{Frame, round_card};
use crate::shell::AppMsg;
use crate::window_frame::PageInsets;

const GUTTER: f64 = 7.;
const MIN: f64 = 250.;

#[derive(Default)]
pub(super) struct Stage {
    pair: Cell<Option<[TabId; 2]>>,
    shares: RefCell<Vec<([TabId; 2], f64)>>,
    pub(super) secondary: RefCell<Option<Retained<NSWindow>>>,
    divider: RefCell<Option<Retained<Divider>>>,
}

pub(super) struct Grip {
    frame: Weak<Frame>,
    grab: Cell<Option<(f64, bool)>>,
}

define_class!(
    // SAFETY: NSView's overrides use the AppKit signatures and main thread.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AeriSplitDivider"]
    #[ivars = Grip]
    pub(super) struct Divider;

    impl Divider {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool { true }

        #[unsafe(method(resetCursorRects))]
        #[allow(deprecated)] // The column-resize replacement needs macOS 15.
        fn reset_cursor_rects(&self) {
            self.addCursorRect_cursor(self.bounds(), &NSCursor::resizeLeftRightCursor());
        }

        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let b = self.bounds();
            let dragging = self.ivars().grab.get().is_some();
            let height = if dragging { 40. } else { 28. };
            NSColor::labelColor().colorWithAlphaComponent(if dragging { 0.45 } else { 0.25 }).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                NSRect::new(NSPoint::new((b.size.width - 4.) / 2., (b.size.height - height) / 2.), NSSize::new(4., height)),
                2., 2.,
            ).fill();
        }

        #[unsafe(method(mouseDown:))]
        fn press(&self, event: &NSEvent) {
            if let Some(frame) = self.ivars().frame.upgrade() {
                if event.clickCount() == 2 {
                    frame.resize_split(0.5);
                } else if let Some(window) = self.window() {
                    let bounds = self.convertRect_toView(self.bounds(), None);
                    let x = window.convertRectToScreen(bounds).origin.x + GUTTER / 2.;
                    let pointer = window.convertPointToScreen(event.locationInWindow()).x;
                    self.ivars().grab.set(Some((pointer - x, false)));
                    self.setNeedsDisplay(true);
                }
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn drag(&self, event: &NSEvent) {
            let Some((offset, snapped)) = self.ivars().grab.get() else { return };
            let Some(frame) = self.ivars().frame.upgrade() else { return };
            let Some(window) = self.window() else { return };
            let page = frame.page_rect(&window);
            let room = page.size.width - GUTTER;
            if room < 2. * MIN { return; }
            let pointer = window.convertPointToScreen(event.locationInWindow()).x;
            let x = pointer - offset - page.origin.x - GUTTER / 2.;
            let snapped = (x - room / 2.).abs() <= if snapped { 20. } else { 15. };
            self.ivars().grab.set(Some((offset, snapped)));
            frame.resize_split(if snapped { 0.5 } else { x / room });
        }

        #[unsafe(method(mouseUp:))]
        fn release(&self, _event: &NSEvent) {
            self.ivars().grab.set(None);
            self.setNeedsDisplay(true);
        }
    }
);

impl Frame {
    pub(super) fn install_divider(self: &Rc<Self>, frame: &NSWindow) {
        let Some(content) = frame.contentView() else {
            return;
        };
        let divider = Divider::alloc(frame.mtm()).set_ivars(Grip {
            frame: Rc::downgrade(self),
            grab: Cell::new(None),
        });
        // SAFETY: NSView's designated initializer; the frame retains its subview.
        let divider: Retained<Divider> =
            unsafe { msg_send![super(divider), initWithFrame: NSRect::ZERO] };
        divider.setHidden(true);
        content.addSubview(&divider);
        self.split.divider.replace(Some(divider));
    }

    pub fn set_split(&self, pair: Option<[TabId; 2]>) {
        self.split.pair.set(pair);
    }

    fn resize_split(&self, share: f64) {
        let Some(pair) = self.split.pair.get() else {
            return;
        };
        let mut shares = self.split.shares.borrow_mut();
        let share = share.clamp(0.2, 0.8);
        if let Some(entry) = shares.iter_mut().find(|(p, _)| *p == pair) {
            entry.1 = share;
        } else {
            shares.push((pair, share));
        }
        drop(shares);
        if let Some(frame) = self.frame.borrow().clone() {
            self.layout_panes(&frame);
        }
    }

    /// The left and right frames, only when both browsers and enough room exist.
    fn panes(&self, page: NSRect) -> Option<([TabId; 2], [NSRect; 2])> {
        let pair = self.split.pair.get()?;
        if !self.active.get().is_some_and(|tab| pair.contains(&tab)) {
            return None;
        }
        let tabs = self.tabs.borrow();
        if !pair.iter().all(|id| tabs.iter().any(|(tab, _)| tab == id)) {
            return None;
        }
        let ratio = self
            .split
            .shares
            .borrow()
            .iter()
            .find(|(p, _)| *p == pair)
            .map_or(0.5, |entry| entry.1);
        let left = pane_width(page.size.width, ratio)?;
        let right_x = page.origin.x + left + GUTTER;
        Some((
            pair,
            [
                NSRect::new(page.origin, NSSize::new(left, page.size.height)),
                NSRect::new(
                    NSPoint::new(right_x, page.origin.y),
                    NSSize::new(page.size.width - left - GUTTER, page.size.height),
                ),
            ],
        ))
    }

    pub(super) fn focused_rect(&self, page: NSRect) -> NSRect {
        self.panes(page).map_or(page, |(pair, rects)| {
            rects[usize::from(self.active.get() == Some(pair[1]))]
        })
    }

    pub(super) fn sync_secondary(&self, frame: &NSWindow, page: NSRect, insets: &PageInsets) {
        let next = self.panes(page).and_then(|(pair, rects)| {
            let index = usize::from(self.active.get() == Some(pair[0]));
            let tabs = self.tabs.borrow();
            let window = tabs.iter().find(|(tab, _)| *tab == pair[index])?.1.clone();
            if window.frame() != rects[index] {
                window.setFrame_display(rects[index], true);
            }
            round_card(&window, insets);
            Some((window, rects[1].origin.x - GUTTER))
        });
        if let Some(divider) = self.split.divider.borrow().as_ref() {
            if let Some((_, x)) = &next {
                let rect = NSRect::new(
                    NSPoint::new(*x, page.origin.y),
                    NSSize::new(GUTTER, page.size.height),
                );
                let frame_rect = frame.convertRectFromScreen(rect);
                let view_rect = frame.contentView().map_or(frame_rect, |view| {
                    view.convertRect_fromView(frame_rect, None)
                });
                if divider.frame() != view_rect {
                    divider.setFrame(view_rect);
                }
                divider.setHidden(false);
            } else {
                divider.setHidden(true);
            }
        }
        let next = next.map(|(window, _)| window);
        let previous = self.split.secondary.replace(next.clone());
        if let Some(old) = previous.as_ref()
            && !next.as_ref().is_some_and(|window| same(window, old))
            && !self
                .active_window()
                .as_ref()
                .is_some_and(|window| same(window, old))
        {
            frame.removeChildWindow(old);
            old.orderOut(None);
        }
        if let Some(window) = next
            && !previous.as_ref().is_some_and(|old| same(&window, old))
        {
            // SAFETY: main-thread AppKit windows; the frame retains the child.
            unsafe { frame.addChildWindow_ordered(&window, NSWindowOrderingMode::Above) };
            window.orderFront(None);
            self.raise_children();
        }
    }

    pub(super) fn split_keyed(&self, key: &NSWindow) {
        if !self
            .split
            .secondary
            .borrow()
            .as_ref()
            .is_some_and(|window| same(window, key))
        {
            return;
        }
        if let Some((tab, _)) = self
            .tabs
            .borrow()
            .iter()
            .find(|(_, window)| same(window, key))
        {
            let _ = self.tx.unbounded_send(AppMsg::SplitFocus {
                window: self.window_id,
                tab: *tab,
            });
        }
    }
}

pub(super) fn same(a: &NSWindow, b: &NSWindow) -> bool {
    std::ptr::eq(a, b)
}

fn pane_width(width: f64, share: f64) -> Option<f64> {
    let room = width - GUTTER;
    (room >= 2. * MIN).then(|| (room * share).round().clamp(MIN, room - MIN))
}

#[cfg(test)]
mod tests {
    use super::pane_width;

    #[test]
    fn tight_windows_fall_back_to_one_pane_instead_of_squeezing_pages() {
        assert_eq!(pane_width(506., 0.5), None);
        assert_eq!(pane_width(507., 0.8), Some(250.));
        assert_eq!(pane_width(707., 0.8), Some(450.));
    }
}
