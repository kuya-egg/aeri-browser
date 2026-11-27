//! AppKit calls on windows CEF or Chrome created.

use std::ffi::c_void;

use objc2::msg_send;
use objc2::runtime::AnyObject;

/// Makes the window holding `view` (an `NSView`) invisible right away.
pub(crate) fn hide_window_of(view: *mut c_void) {
    if view.is_null() {
        return;
    }
    // SAFETY: `view` is a live NSView from CEF; called on the main thread.
    unsafe {
        let window: *mut AnyObject = msg_send![view.cast::<AnyObject>(), window];
        if let Some(window) = window.as_ref() {
            let _: () = msg_send![window, setAlphaValue: 0.0f64];
            // Chrome may order the window back in; invisible, it must not
            // take the clicks meant for the windows under it.
            let _: () = msg_send![window, setIgnoresMouseEvents: true];
            let _: () = msg_send![window, orderOut: std::ptr::null::<AnyObject>()];
        }
    }
}
