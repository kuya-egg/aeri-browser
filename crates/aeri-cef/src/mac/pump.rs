//! Drives CEF's message loop from the main run loop (`external_message_pump`).
//!
//! GPUI owns `[NSApp run]`, so CEF cannot run its own loop. CEF tells us when
//! it wants work via `OnScheduleMessagePumpWork(delay)`, from any thread. We
//! keep one `CFRunLoopTimer` on the main run loop in the common modes, which
//! on the main thread include event tracking and modal panels, so pages keep
//! running during live resize and open menus. A thread-safe run-loop source
//! carries "work wanted sooner" requests to the main thread.
//!
//! `do_message_loop_work` is only called from the timer callback, never from
//! inside GPUI code, so Chromium tasks never run while GPUI state is borrowed.
//!
//! The timer also fires at least every [`MAX_DELAY`] while CEF is busy,
//! following CEF's cefclient external pump, and every [`IDLE_DELAY`] once it
//! has gone quiet, so an idle browser does not wake the Mac 30 times a
//! second.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, Ordering};

use core_foundation::base::{TCFType, kCFAllocatorDefault};
use core_foundation::date::CFAbsoluteTimeGetCurrent;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceContext, CFRunLoopSourceCreate,
    CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopTimer, CFRunLoopTimerContext,
    CFRunLoopTimerGetNextFireDate, CFRunLoopTimerRef, CFRunLoopTimerSetNextFireDate,
    CFRunLoopWakeUp, kCFRunLoopCommonModes,
};

/// Longest wait between pumps, in seconds (cefclient's `kMaxTimerDelay`).
const MAX_DELAY: f64 = 1.0 / 30.0;

/// The wait between pumps once CEF has asked for no immediate work for
/// [`IDLE_AFTER`] ticks (about a second): an idle browser wakes 4 times a
/// second instead of 30. Any immediate request brings the next tick
/// forward at once and returns to [`MAX_DELAY`]; a delayed one still fires
/// on time.
const IDLE_DELAY: f64 = 0.25;
const IDLE_AFTER: u32 = 30;

/// No pending request.
const NONE: i64 = i64::MAX;

/// Shortest delay (ms) requested since the main thread last looked.
static REQUESTED_MS: AtomicI64 = AtomicI64::new(NONE);
/// The wake source, created once and kept for the life of the process.
static SOURCE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
/// CEF asked for work at once since the last tick.
static IMMEDIATE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static TIMER: RefCell<Option<CFRunLoopTimer>> = const { RefCell::new(None) };
    /// Set while `do_message_loop_work` runs; a nested run loop inside it
    /// must not pump again.
    static PUMPING: Cell<bool> = const { Cell::new(false) };
    /// A request arrived while pumping; fire again right after.
    static RERUN: Cell<bool> = const { Cell::new(false) };
    /// Ticks in a row with no immediate work asked for.
    static IDLE_TICKS: Cell<u32> = const { Cell::new(0) };
}

/// Installs the timer and source on the main run loop. Main thread only; call
/// once before `cef::initialize`.
pub(crate) fn install() {
    TIMER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return;
        }
        let mut timer_context = CFRunLoopTimerContext {
            version: 0,
            info: std::ptr::null_mut(),
            retain: None,
            release: None,
            copyDescription: None,
        };
        // SAFETY: plain CoreFoundation calls on the main thread.
        let now = unsafe { CFAbsoluteTimeGetCurrent() };
        let timer = CFRunLoopTimer::new(
            now + MAX_DELAY,
            MAX_DELAY,
            0,
            0,
            on_tick,
            &mut timer_context,
        );
        unsafe { CFRunLoop::get_main().add_timer(&timer, kCFRunLoopCommonModes) };

        let mut source_context = CFRunLoopSourceContext {
            version: 0,
            info: std::ptr::null_mut(),
            retain: None,
            release: None,
            copyDescription: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: on_request,
        };
        // SAFETY: the source is retained for the process lifetime (never released).
        unsafe {
            let source = CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &mut source_context);
            assert!(!source.is_null(), "CFRunLoopSourceCreate failed");
            CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
            SOURCE.store(source.cast(), Ordering::Release);
        }
        *slot = Some(timer);
    });
}

/// CEF's `OnScheduleMessagePumpWork`. Callable from any thread.
pub(crate) fn schedule(delay_ms: i64) {
    let delay_ms = delay_ms.max(0);
    if delay_ms == 0 {
        IMMEDIATE.store(true, Ordering::Release);
    }
    REQUESTED_MS.fetch_min(delay_ms, Ordering::AcqRel);
    let source = SOURCE.load(Ordering::Acquire);
    if !source.is_null() {
        // SAFETY: signalling a source and waking a run loop are thread-safe.
        unsafe {
            CFRunLoopSourceSignal(source as CFRunLoopSourceRef);
            CFRunLoopWakeUp(CFRunLoopGetMain());
        }
    }
}

/// Stops pumping. Call before `cef::shutdown`; CEF must not be pumped after.
pub(crate) fn uninstall() {
    SOURCE.store(std::ptr::null_mut(), Ordering::Release);
    TIMER.with(|slot| {
        if let Some(timer) = slot.borrow_mut().take() {
            // SAFETY: invalidating our own timer on its run loop's thread.
            unsafe {
                core_foundation::runloop::CFRunLoopTimerInvalidate(timer.as_concrete_TypeRef())
            };
        }
    });
}

/// Main thread: bring the timer forward if CEF asked for work sooner.
extern "C" fn on_request(_info: *const c_void) {
    let delay_ms = REQUESTED_MS.swap(NONE, Ordering::AcqRel);
    if delay_ms == NONE {
        return;
    }
    if PUMPING.with(Cell::get) {
        RERUN.with(|rerun| rerun.set(true));
        return;
    }
    // Never later than the idle wait: a delayed task is still run on time
    // while the pump has slowed down.
    let delay = (delay_ms as f64 / 1000.0).min(IDLE_DELAY);
    TIMER.with(|slot| {
        if let Some(timer) = slot.borrow().as_ref() {
            let timer = timer.as_concrete_TypeRef();
            // SAFETY: valid timer on its own run loop's thread.
            unsafe {
                let target = CFAbsoluteTimeGetCurrent() + delay;
                if target < CFRunLoopTimerGetNextFireDate(timer) {
                    CFRunLoopTimerSetNextFireDate(timer, target);
                }
            }
        }
    });
}

extern "C" fn on_tick(timer: CFRunLoopTimerRef, _info: *mut c_void) {
    if PUMPING.with(|pumping| pumping.replace(true)) {
        return;
    }
    cef::do_message_loop_work();
    PUMPING.with(|pumping| pumping.set(false));
    let rerun = RERUN.with(|rerun| rerun.replace(false));
    let busy = rerun || IMMEDIATE.swap(false, Ordering::AcqRel);
    let idle = IDLE_TICKS.with(|ticks| {
        let next = if busy {
            0
        } else {
            ticks.get().saturating_add(1)
        };
        ticks.set(next);
        next >= IDLE_AFTER
    });
    let delay = if rerun {
        0.0
    } else if idle {
        IDLE_DELAY
    } else {
        MAX_DELAY
    };
    // SAFETY: called by the run loop with its own valid timer.
    unsafe { CFRunLoopTimerSetNextFireDate(timer, CFAbsoluteTimeGetCurrent() + delay) };
}
