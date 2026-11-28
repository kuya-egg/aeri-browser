//! Two things done to the pages themselves, both over the page's own
//! DevTools session (`aeri_cef::TabWindow`):
//!
//! - **Dusk** (Settings › Appearance › Darken pages that stay light): while
//!   the app is dark, Chromium's auto dark mode darkens a page that has no
//!   dark look of its own, and leaves one that answers `prefers-color-scheme`
//!   alone. ⇧⌘D turns it off, or on, for the site you are on and is kept
//!   (`dusk.sites` in `app.db`, the sites it is off for); ⌥⇧⌘D pauses it for
//!   every site until pressed again or Aeri quits.
//! - **Capture** (Settings › Browsing › Capture pages): Capture… in the
//!   palette puts a layer over the page to drag an area, click an element or
//!   take the whole page. The picture goes on this Mac's clipboard only (not
//!   Universal Clipboard) and into the downloads folder as a PNG.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::palette::PaletteCommand;
use aeri_cef::{Capture, TabWindow};
use aeri_core::appearance::Mode;
use aeri_core::engine::Event;
use aeri_core::ids::TabId;
use aeri_core::prefs::AppPrefs;
use gpui::App;
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSBitmapImageRep, NSColor, NSColorSpace, NSPasteboard, NSPasteboardContentsOptions,
    NSPasteboardTypePNG, NSPasteboardTypeTIFF, NSSound,
};
use objc2_foundation::{NSData, NSString};

use super::Shell;
use super::windows::host_of;
use crate::theme::Theme;

/// The sites Dusk is off for, one host a line.
const SITES: &str = "dusk.sites";
/// Search's own accent until the Mac says otherwise.
const FALLBACK_ACCENT: &str = "10, 132, 255";

#[derive(Default)]
struct Dusk {
    /// For now, not for good: forgotten when Aeri quits.
    paused: bool,
    /// Loaded on first use.
    off_for: Option<BTreeSet<String>>,
}

thread_local! {
    static DUSK: RefCell<Dusk> = RefCell::default();
}

/// The host Dusk keeps a site by: no `www.`.
fn site_of(url: &str) -> Option<String> {
    let host = host_of(url).to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    (!host.is_empty()).then(|| host.to_owned())
}

impl Shell {
    /// Follows what changed in `appearance` and `browsing` between `before`
    /// and now.
    pub(super) fn apply_pages(&mut self, before: &AppPrefs, cx: &mut App) {
        if self.prefs.appearance.dusk != before.appearance.dusk {
            self.apply_dusk(cx);
        }
    }

    /// Whether Aeri is dark: pages follow it.
    fn app_is_dark(cx: &App) -> bool {
        cx.global::<Theme>().resolved.mode == Mode::Dark
    }

    fn dusk_off_for(&self, site: &str) -> bool {
        DUSK.with_borrow_mut(|dusk| {
            dusk.off_for
                .get_or_insert_with(|| {
                    let kept = self.db.setting(SITES).ok().flatten().unwrap_or_default();
                    kept.lines().map(str::to_owned).collect()
                })
                .contains(site)
        })
    }

    /// Whether a page at `url` is darkened: Dusk on, not paused, Aeri dark,
    /// and the site not turned off. Pages that are not on the web get
    /// nothing.
    fn dusk_wants(&self, url: &str, cx: &App) -> bool {
        let Some(site) = site_of(url) else {
            return false;
        };
        self.prefs.appearance.dusk
            && !DUSK.with_borrow(|dusk| dusk.paused)
            && Self::app_is_dark(cx)
            && !self.dusk_off_for(&site)
    }

    /// Every tab's page, with its address, wherever the tab lives.
    fn every_page(&self) -> Vec<(TabId, Rc<TabWindow>, String)> {
        let mut pages = Vec::new();
        let mut take = |tabs: &aeri_core::workspace::TabList<super::tabs::TabEntry>| {
            for item in tabs.iter() {
                if let Some(page) = &item.value.cef {
                    pages.push((item.id, page.clone(), item.value.url().to_owned()));
                }
            }
        };
        for w in &self.windows {
            take(&w.tabs);
        }
        for open in self.open.values() {
            take(&open.favourites);
            for parked in open.parked.values() {
                take(&parked.tabs);
            }
        }
        pages
    }

    /// Tells every page whether it is darkened. Outside the `App` borrow:
    /// CEF may answer at once.
    pub(super) fn apply_dusk(&mut self, cx: &App) {
        let wanted: Vec<_> = self
            .every_page()
            .into_iter()
            .map(|(_, page, url)| {
                let on = self.dusk_wants(&url, cx);
                (page, on)
            })
            .collect();
        self.later(move |_| {
            for (page, on) in wanted {
                page.set_dusk(on);
            }
        });
    }

    /// A page was made, or went to another address: it gets what Dusk says
    /// for it.
    pub(super) fn dusk_event(&mut self, event: &Event, cx: &App) {
        let (tab, address) = match event {
            Event::Created { tab } => (*tab, None),
            Event::AddressChanged { tab, url } => (*tab, Some(url.as_str())),
            _ => return,
        };
        // Found in place: this runs on every address change, so no list of
        // every page is built for it.
        let found = self
            .windows
            .iter()
            .map(|w| &w.tabs)
            .chain(self.open.values().flat_map(|open| {
                std::iter::once(&open.favourites).chain(open.parked.values().map(|p| &p.tabs))
            }))
            .find_map(|tabs| tabs.get(tab));
        let Some((page, url)) =
            found.and_then(|item| Some((item.value.cef.clone()?, item.value.url())))
        else {
            return;
        };
        let on = self.dusk_wants(address.unwrap_or(url), cx);
        self.later(move |_| page.set_dusk(on));
    }

    /// ⇧⌘D: Dusk off, or on, for the site the window shows. Keeps nothing
    /// for a site that agrees with the default (darkened).
    pub(super) fn toggle_dusk_site(&mut self, window_id: u64, cx: &mut App) {
        let url = self
            .window(window_id)
            .and_then(|w| self.shown_model(w))
            .map(|tab| tab.url);
        let Some(site) = url.as_deref().and_then(site_of) else {
            return;
        };
        let usable = self.prefs.appearance.dusk
            && !DUSK.with_borrow(|dusk| dusk.paused)
            && Self::app_is_dark(cx);
        if !usable {
            return;
        }
        let off = self.dusk_off_for(&site);
        let kept = DUSK.with_borrow_mut(|dusk| {
            let sites = dusk.off_for.get_or_insert_with(BTreeSet::new);
            if off {
                sites.remove(&site);
            } else {
                sites.insert(site);
            }
            sites.iter().cloned().collect::<Vec<_>>().join("\n")
        });
        if let Err(err) = self.db.set_setting(SITES, &kept) {
            tracing::warn!(%err, "could not save the sites Dusk is off for");
        }
        self.apply_dusk(cx);
    }

    /// ⌥⇧⌘D: Dusk paused for every site, or running again.
    pub(super) fn pause_dusk(&mut self, cx: &mut App) {
        if !self.prefs.appearance.dusk {
            return;
        }
        DUSK.with_borrow_mut(|dusk| dusk.paused = !dusk.paused);
        self.apply_dusk(cx);
    }

    /// Whether the palette lists `command`: Capture and the Dusk commands
    /// only while their settings are on.
    pub(super) fn pages_command_listed(&self, command: PaletteCommand) -> bool {
        match command {
            PaletteCommand::Capture => self.prefs.browsing.capture,
            PaletteCommand::DarkenSite | PaletteCommand::PauseDarkening => {
                self.prefs.appearance.dusk
            }
            _ => true,
        }
    }

    /// Whether Dusk is paused, for the palette's title.
    pub(super) fn dusk_paused(&self) -> bool {
        DUSK.with_borrow(|dusk| dusk.paused)
    }

    /// Capture…: puts the layer over the shown page, or, if it is already
    /// up, takes it down.
    pub(super) fn capture_page(&mut self, window_id: u64) {
        if !self.prefs.browsing.capture {
            return;
        }
        let Some(page) = self.shown_cef(window_id) else {
            return;
        };
        let folder = super::prefs::download_dir(&self.prefs, &self.default_download_dir);
        self.later(move |_| {
            if page.capturing() {
                page.cancel_capture();
                return;
            }
            page.capture(&accent(), move |taken| keep(taken, &folder));
        });
    }
}

/// What a capture came to: on the clipboard and in the folder, or why not.
/// Runs inside a CEF callback: AppKit and the disk only.
fn keep(taken: Capture, folder: &Path) {
    match taken {
        Capture::Taken { png, cropped } => {
            copy(&png);
            if let Some(sound) = NSSound::soundNamed(&NSString::from_str("Pop")) {
                sound.play();
            }
            let file = free_name(folder);
            tracing::info!(
                file = %file.display(),
                cropped,
                "page captured to the clipboard"
            );
            // A whole page can be large: off the main thread.
            std::thread::spawn(move || {
                if let Err(err) = std::fs::write(&file, &png) {
                    tracing::warn!(%err, file = %file.display(), "could not save the capture");
                }
            });
        }
        Capture::Cancelled => {}
        Capture::NothingThere => tracing::info!("nothing there to capture"),
        Capture::Failed(error) => tracing::warn!(error, "could not capture the page"),
    }
}

/// The picture on this Mac's clipboard only: Universal Clipboard would hand
/// it to the Person's other devices.
fn copy(png: &[u8]) {
    let board = NSPasteboard::generalPasteboard();
    board.prepareForNewContentsWithOptions(NSPasteboardContentsOptions::CurrentHostOnly);
    let data = NSData::with_bytes(png);
    // SAFETY: AppKit's constant type names, valid for the process's life.
    let (png_type, tiff_type) = unsafe { (NSPasteboardTypePNG, NSPasteboardTypeTIFF) };
    board.setData_forType(Some(&data), png_type);
    // Some apps take only TIFF from the clipboard.
    let tiff = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &data)
        .and_then(|rep| rep.TIFFRepresentation());
    if let Some(tiff) = tiff {
        board.setData_forType(Some(&tiff), tiff_type);
    }
}

/// The Mac's accent colour as "r, g, b", for the layer's outline.
fn accent() -> String {
    let colour =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace());
    let Some(colour) = colour else {
        return FALLBACK_ACCENT.to_owned();
    };
    let byte = |part: f64| (part * 255.0).round() as u8;
    format!(
        "{}, {}, {}",
        byte(colour.redComponent()),
        byte(colour.greenComponent()),
        byte(colour.blueComponent())
    )
}

/// "Capture 2026-11-17 at 14.03.22.png": a time, as the Mac names its own
/// screenshots, and not the site, which would give it away in the folder.
fn file_name() -> String {
    // SAFETY: `time` takes no pointer; `localtime_r` fills the zeroed `tm`
    // it is given from the valid `time_t` beside it.
    let tm = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    format!(
        "Capture {:04}-{:02}-{:02} at {:02}.{:02}.{:02}.png",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

/// The capture's file in `folder`, numbered if the name is taken.
fn free_name(folder: &Path) -> PathBuf {
    let name = file_name();
    let stem = name.trim_end_matches(".png");
    let mut file = folder.join(&name);
    let mut n = 2;
    while file.exists() {
        file = folder.join(format!("{stem} {n}.png"));
        n += 1;
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_is_kept_without_www_and_case() {
        assert_eq!(
            site_of("https://www.Example.com/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            site_of("https://news.example.com").as_deref(),
            Some("news.example.com")
        );
        assert_eq!(site_of("chrome://settings"), None);
        assert_eq!(site_of("about:blank"), None);
    }

    #[test]
    fn a_taken_name_is_numbered() {
        let dir = std::env::temp_dir().join(format!("aeri-capture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let first = free_name(&dir);
        std::fs::write(&first, b"x").expect("write");
        let second = free_name(&dir);
        assert_ne!(first, second);
        assert!(second.to_string_lossy().ends_with(" 2.png"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
