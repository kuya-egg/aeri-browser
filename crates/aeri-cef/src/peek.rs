//! Peek at a link (Search `Peek.swift`, `Browser.swift:7936-7945`): the
//! adapter's part. Chromium gives `OnOpenURLFromTab` and `OnBeforePopup`
//! only a window disposition, which cannot tell ⌥⇧-click from ⌥⌘-click, so
//! the click is read in the page: a capture-phase `click` listener, run in
//! an isolated world of Aeri's own for each document ([`set_click`]),
//! compares the modifier set exactly with the chosen one (⌃ never peeks),
//! cancels the click and reports the link through a `Runtime.addBinding`
//! function that exists only in that world (the payload arrives as
//! `Runtime.bindingCalled`, [`event`]).
//!
//! The peek's own browser is never given the listener: links in a peek
//! just navigate it. Its keys are read in [`pre_key`] (Esc closes it, ⌘↩
//! keeps it), and its first paint is reported through `Page` lifecycle
//! events ([`watch_paint`]) so the application can show a skeleton until
//! then.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aeri_core::ids::TabId;
use cef::{BrowserHost, KeyEvent, KeyEventType};
use serde_json::{Value, json};

use crate::cdp_pages::Cdp;
use crate::{Signal, SignalSink};

/// The modifier set of the click that peeks, as bits ([`set_click`]).
pub const SHIFT: u8 = 1;
pub const OPTION: u8 = 2;
pub const COMMAND: u8 = 4;

/// The name of the function the listener reports through. It exists only
/// in Aeri's isolated world ([`WORLD`]), never in the page's.
const BINDING: &str = "aeriPeek";

/// The isolated world the listener runs in.
const WORLD: &str = "aeri-peek";

/// Search `App.swift:1590-1596` (Esc) and `1676-1689` (⌘↩).
const VK_ESCAPE: i32 = 27;
const VK_RETURN: i32 = 13;

const MODIFIERS: u32 = cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0
    | cef::sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0
    | cef::sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0
    | cef::sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;
const COMMAND_DOWN: u32 = cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0;
const KEEP_BESIDE: u32 = COMMAND_DOWN | cef::sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;

/// What a peek's page or a link click tells the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeekSignal {
    /// A link was clicked with the chosen modifiers in `tab`'s page: it
    /// was not followed.
    Link { tab: TabId, url: String },
    /// A key the peek takes in front of its page.
    Key { tab: TabId, key: PeekKey },
    /// The peek's page drew for the first time.
    Painted { tab: TabId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeekKey {
    /// Esc.
    Close,
    /// ⌘↩ outside an editable field: the peek becomes a tab.
    Keep,
    /// ⌥⌘↩ outside an editable field: keep it beside its opener.
    KeepBeside,
}

thread_local! {
    /// The tabs whose keys [`pre_key`] takes: the open peeks' pages.
    static KEYED: RefCell<Vec<TabId>> = const { RefCell::new(Vec::new()) };
}

/// Names the tabs of the open peeks, whose Esc and ⌘↩ go to the
/// application instead of the page.
pub fn set_peek_tabs(tabs: &[TabId]) {
    KEYED.with_borrow_mut(|keyed| {
        keyed.clear();
        keyed.extend_from_slice(tabs);
    });
}

/// What a tab was last told about the click.
#[derive(Default)]
pub(crate) struct State {
    /// Bumped by every new document, so an answer about an old one is
    /// dropped.
    document: Cell<u32>,
    /// The mask the page was last told.
    mask: Cell<u8>,
    /// Aeri's world in the current document, holding the function and the
    /// listener, once made.
    world: Cell<Option<i64>>,
    /// A world is being made for the current document.
    making: Cell<bool>,
}

/// The page's script for `mask`: installs the listener once per document
/// and, called again, only changes the mask it compares with. The click is
/// compared exactly: Shift is `shiftKey` and nothing else.
fn script(mask: u8) -> String {
    format!(
        "(function (mask) {{
  var key = Symbol.for('aeri.peek');
  if (window[key]) {{ window[key].mask = mask; return; }}
  var send = window.{BINDING};
  if (typeof send !== 'function') return;
  var held = {{ mask: mask }};
  Object.defineProperty(window, key, {{ value: held, configurable: true }});
  try {{ delete window.{BINDING}; }} catch (e) {{}}
  document.addEventListener('click', function (e) {{
    var m = held.mask;
    if (!m || e.button !== 0 || !e.isTrusted) return;
    if (e.shiftKey !== !!(m & 1) || e.altKey !== !!(m & 2) || e.metaKey !== !!(m & 4) || e.ctrlKey) return;
    var path = e.composedPath ? e.composedPath() : [e.target];
    for (var i = 0; i < path.length; i++) {{
      var el = path[i];
      if (!el || el.nodeType !== 1 || !el.tagName || el.tagName.toLowerCase() !== 'a') continue;
      var href = el.href;
      if (href && typeof href === 'object') href = href.baseVal ? new URL(href.baseVal, document.baseURI).href : '';
      if (!href) continue;
      if (!/^https?:/i.test(href)) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      send(href);
      return;
    }}
  }}, true);
}})({mask});"
    )
}

fn log(what: &'static str) -> impl FnOnce(Result<Value, String>) + 'static {
    move |reply| {
        if let Err(error) = reply {
            tracing::warn!(error, "peek: {what} failed");
        }
    }
}

/// Makes the page peek at a link clicked with `mask` (bits of [`SHIFT`],
/// [`OPTION`] and [`COMMAND`]; 0 for none). The listener and the function
/// it reports through live in an isolated world of Aeri's own
/// (`Page.createIsolatedWorld`, made again for each document): the page
/// cannot reach either, and `Runtime` is never enabled, so the page's
/// console calls and script contexts are not streamed to the browser for
/// the tab's whole life. A no-op when the page was last told the same.
pub(crate) fn set_click(cdp: &Rc<Cdp>, host: &BrowserHost, mask: u8) {
    if cdp.peek.mask.replace(mask) == mask {
        return;
    }
    install(cdp, host);
}

/// `Page.frameNavigated`: a new document in the main frame gets the
/// listener again, in a new world.
pub(crate) fn navigated(cdp: &Rc<Cdp>, host: &BrowserHost, params: Option<&[u8]>) {
    let Some(params) = parse(params) else { return };
    if !params["frame"]["parentId"].is_null() {
        return;
    }
    let state = &cdp.peek;
    state.document.set(state.document.get().wrapping_add(1));
    state.world.set(None);
    state.making.set(false);
    install(cdp, host);
}

/// Tells the current document's world the mask, making the world first
/// if it has none.
fn install(cdp: &Rc<Cdp>, host: &BrowserHost) {
    let state = &cdp.peek;
    let mask = state.mask.get();
    if let Some(world) = state.world.get() {
        cdp.call(
            host,
            "Runtime.evaluate",
            &json!({ "expression": script(mask), "contextId": world }),
            log("applying to the page"),
        );
        return;
    }
    if mask == 0 || state.making.replace(true) {
        return;
    }
    let document = state.document.get();
    let (weak, owner) = (Rc::downgrade(cdp), host.clone());
    cdp.call(host, "Page.getFrameTree", &json!({}), move |reply| {
        let Some(cdp) = weak
            .upgrade()
            .filter(|cdp| cdp.peek.document.get() == document)
        else {
            return;
        };
        let frame = match reply {
            Ok(tree) => tree["frameTree"]["frame"]["id"].as_str().map(str::to_owned),
            Err(error) => {
                tracing::warn!(error, "peek: no frame to listen in");
                None
            }
        };
        let Some(frame) = frame else {
            cdp.peek.making.set(false);
            return;
        };
        make_world(&cdp, &owner, frame, document);
    });
}

fn make_world(cdp: &Rc<Cdp>, host: &BrowserHost, frame: String, document: u32) {
    let (weak, owner) = (Rc::downgrade(cdp), host.clone());
    cdp.call(
        host,
        "Page.createIsolatedWorld",
        &json!({ "frameId": frame, "worldName": WORLD }),
        move |reply| {
            let Some(cdp) = weak
                .upgrade()
                .filter(|cdp| cdp.peek.document.get() == document)
            else {
                return;
            };
            cdp.peek.making.set(false);
            let world = match reply {
                Ok(result) => result["executionContextId"].as_i64(),
                Err(error) => {
                    tracing::warn!(error, "peek: no world of our own");
                    None
                }
            };
            let Some(world) = world else { return };
            // Commands on a session run in order: the function exists
            // before the script that takes it runs.
            cdp.call(
                &owner,
                "Runtime.addBinding",
                &json!({ "name": BINDING, "executionContextId": world }),
                log("adding the binding"),
            );
            cdp.peek.world.set(Some(world));
            install(&cdp, &owner);
        },
    );
}

/// Has the page report its first paint ([`PeekSignal::Painted`]).
pub(crate) fn watch_paint(cdp: &Cdp, host: &BrowserHost) {
    cdp.call(
        host,
        "Page.setLifecycleEventsEnabled",
        &json!({ "enabled": true }),
        log("watching paint"),
    );
}

/// A DevTools event on `tab`'s own session: the click's report and the
/// first paint. Everything else is not ours.
pub(crate) fn event(tab: TabId, sink: &SignalSink, method: &str, params: Option<&[u8]>) {
    let signal = match method {
        "Runtime.bindingCalled" => {
            let Some(params) = parse(params) else { return };
            if params["name"].as_str() != Some(BINDING) {
                return;
            }
            let url = params["payload"].as_str().unwrap_or_default().to_owned();
            // The script only sends web addresses; a page cannot reach
            // the function, but check anyway.
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return;
            }
            PeekSignal::Link { tab, url }
        }
        "Page.lifecycleEvent" => {
            let Some(params) = parse(params) else { return };
            if !matches!(
                params["name"].as_str(),
                Some("firstPaint" | "firstContentfulPaint")
            ) {
                return;
            }
            PeekSignal::Painted { tab }
        }
        _ => return,
    };
    sink(Signal::Peek(signal));
}

fn parse(params: Option<&[u8]>) -> Option<Value> {
    serde_json::from_slice(params?).ok()
}

/// Esc and ⌘↩ in the open peek's page go to the application. Returns
/// whether the key was taken (Search guards ⌘↩ with "the caret is not in
/// an editable", so comment boxes still send with it).
pub(crate) fn pre_key(tab: TabId, event: &KeyEvent, sink: &SignalSink) -> bool {
    if event.type_ != KeyEventType::RAWKEYDOWN || !KEYED.with_borrow(|keyed| keyed.contains(&tab)) {
        return false;
    }
    let key = match (event.windows_key_code, event.modifiers & MODIFIERS) {
        (VK_ESCAPE, 0) => PeekKey::Close,
        (VK_RETURN, COMMAND_DOWN) if event.focus_on_editable_field == 0 => PeekKey::Keep,
        (VK_RETURN, KEEP_BESIDE) if event.focus_on_editable_field == 0 => PeekKey::KeepBeside,
        _ => return false,
    };
    sink(Signal::Peek(PeekSignal::Key { tab, key }));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_carries_the_mask() {
        assert!(script(SHIFT).ends_with("})(1);"));
        assert!(script(OPTION | COMMAND).ends_with("})(6);"));
    }

    #[test]
    fn the_binding_is_taken_out_of_the_pages_reach() {
        let source = script(SHIFT);
        assert!(source.contains("delete window.aeriPeek"));
    }
}
