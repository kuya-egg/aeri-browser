//! The welcome film: the bundled `aeri-intro.mp4` played by AVFoundation in
//! a native view above the GPUI playback controls. The view goes away when
//! the [`IntroPlayer`] is dropped (end, skip, close).
//!
//! Stock CEF builds can lack H.264, so playback stays native.
//! AVFoundation classes are reached dynamically (no typed bindings are
//! enabled); the framework is linked here.
//!
//! Main thread only. `on_complete` runs once on the main thread: when the
//! film plays to its end, or when AVFoundation reports it cannot play it.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
use objc2_foundation::{
    NSBundle, NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue, NSPoint,
    NSRect, NSSize, NSString, NSTimer, NSURL,
};

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVPlayerItemDidPlayToEndTimeNotification: &'static NSString;
    static AVPlayerItemFailedToPlayToEndTimeNotification: &'static NSString;
}

#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {}

/// The bundled film, in the app's `Resources`.
const FILM: (&str, &str) = ("aeri-intro", "mp4");
/// How often the player item is checked for a failure no notification
/// reports (an unreadable or undecodable file).
const WATCH: f64 = 0.25;
/// Setup must not wait indefinitely on a stalled player.
const DEADLINE: Duration = Duration::from_secs(17);
/// `AVPlayerItemStatusFailed`.
const STATUS_FAILED: isize = 2;

type Observer = Retained<objc2::runtime::ProtocolObject<dyn NSObjectProtocol>>;

pub(crate) struct IntroPlayer {
    view: Retained<NSView>,
    player: Retained<AnyObject>,
    observers: [Observer; 2],
    watch: Retained<NSTimer>,
}

fn class(name: &'static std::ffi::CStr) -> Option<&'static AnyClass> {
    AnyClass::get(name)
}

impl IntroPlayer {
    /// Plays the bundled film over `parent` (an `NSView*`, the window's content
    /// view) and starts it at once. `None` when the film is not in the bundle or
    /// AVFoundation is unavailable: the caller carries on without it.
    ///
    /// # Safety
    /// `parent` must point to the live `NSView` owned by the welcome window.
    pub(crate) unsafe fn play(
        parent: *mut c_void,
        footer: f64,
        on_complete: impl Fn() + 'static,
    ) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        // SAFETY: the caller hands the window's live content view, used here on
        // the main thread only.
        let parent: &NSView = unsafe { parent.cast::<NSView>().as_ref()? };

        let path = NSBundle::mainBundle().pathForResource_ofType(
            Some(&NSString::from_str(FILM.0)),
            Some(&NSString::from_str(FILM.1)),
        )?;
        let url = NSURL::fileURLWithPath(&path);

        // SAFETY: plain AVFoundation/AppKit messages with the documented
        // argument types; every object is retained by us or a `Retained`.
        unsafe {
            let item: Retained<AnyObject> =
                msg_send![class(c"AVPlayerItem")?, playerItemWithURL: &*url];
            let player: Retained<AnyObject> =
                msg_send![class(c"AVPlayer")?, playerWithPlayerItem: &*item];
            let _: () = msg_send![&*player, setAllowsExternalPlayback: false];
            let _: () = msg_send![&*player, setActionAtItemEnd: 1isize]; // pause
            let layer: Retained<AnyObject> =
                msg_send![class(c"AVPlayerLayer")?, playerLayerWithPlayer: &*player];
            // Aspect-fit explicitly: any film shape scales uniformly into the
            // stage (never stretched) and the black window shows around it.
            let gravity = NSString::from_str("AVLayerVideoGravityResizeAspect");
            let _: () = msg_send![&*layer, setVideoGravity: &*gravity];

            let bounds = parent.bounds();
            let y = bounds.origin.y + if parent.isFlipped() { 0. } else { footer };
            let frame = NSRect::new(
                NSPoint::new(bounds.origin.x, y),
                NSSize::new(bounds.size.width, (bounds.size.height - footer).max(0.)),
            );
            let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            // A layer-hosting view: the player layer is the view's layer, so it
            // follows the view's size.
            let _: () = msg_send![&*view, setLayer: &*layer];
            view.setWantsLayer(true);
            parent.addSubview(&view);

            let done = Rc::new(on_complete);
            let fired = Rc::new(Cell::new(false));
            let finish = {
                let fired = fired.clone();
                move || {
                    if !fired.replace(true) {
                        done();
                    }
                }
            };

            let center = NSNotificationCenter::defaultCenter();
            let queue = NSOperationQueue::mainQueue();
            let observers = [
                AVPlayerItemDidPlayToEndTimeNotification,
                AVPlayerItemFailedToPlayToEndTimeNotification,
            ]
            .map(|name| {
                let finish = finish.clone();
                let block = RcBlock::new(move |_: NonNull<NSNotification>| finish());
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&item),
                    Some(&queue),
                    &block,
                )
            });

            let deadline = Instant::now() + DEADLINE;
            let block = RcBlock::new(move |_: NonNull<NSTimer>| {
                let status: isize = msg_send![&*item, status];
                if status == STATUS_FAILED || Instant::now() >= deadline {
                    finish();
                }
            });
            let watch = NSTimer::scheduledTimerWithTimeInterval_repeats_block(WATCH, true, &block);

            let _: () = msg_send![&*player, play];
            Some(Self {
                view,
                player,
                observers,
                watch,
            })
        }
    }
}

impl Drop for IntroPlayer {
    fn drop(&mut self) {
        self.watch.invalidate();
        let center = NSNotificationCenter::defaultCenter();
        for observer in &self.observers {
            // SAFETY: the observers were added to this center by `play`.
            unsafe { center.removeObserver(observer.as_ref()) };
        }
        // SAFETY: plain messages to our own player and view.
        unsafe {
            let _: () = msg_send![&*self.player, pause];
            let none: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![&*self.player, replaceCurrentItemWithPlayerItem: none];
        }
        self.view.removeFromSuperview();
    }
}
