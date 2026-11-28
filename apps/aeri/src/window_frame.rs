//! The GPUI frame window of an Aeri window. It owns the window's size,
//! position, full screen and Space, and paints the window's ground and the
//! chrome that is pinned: the sidebar (Search's column: the band with the
//! window's own traffic lights and the history doors, the tabs, the foot)
//! or the strip along the top. The page sits beside pinned chrome as a
//! rounded card whose edge this view draws, or covers the frame when
//! nothing is pinned (`mac::Frame::layout`); unpinned chrome slides in over
//! the page in its own windows (`chrome_overlay`), drawn by the same view.

mod bookmarks_bar;
mod doors;
mod give;
mod glide;
mod home;
mod icons;
mod player;
mod sidebar;
mod strip;
mod tab_hover;
mod tab_items;
mod tab_list;

pub use bookmarks_bar::BarMark;
pub use glide::column_glides;
pub use player::{PlayerTab, TabLook};

use aeri_core::appearance::Rgba;
use aeri_core::ids::{SpaceId, TabId};
use aeri_core::layout::{DEFAULT_SIDEBAR, Layout, Side, TabPlace, clamp_width};
use aeri_core::store::Profile;
use futures::channel::mpsc::UnboundedSender;
use gpui::{
    App, BoxShadow, ClickEvent, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, SharedString, Window, div, img, point, prelude::*, px,
};

use crate::native_menu::Align;
use crate::shell::{AppMsg, ChromeMsg};
use crate::theme;

/// Height of the band and the strip, in points (Search `Metrics.strip`);
/// `mac::Frame` insets the page below the strip. [`strip_height`] is the
/// one in effect.
pub const STRIP_HEIGHT: f64 = 52.0;
/// The strip with tabs along the top and "Shorter tab bar" on (Search
/// `Metrics.strip` compact, `Design.swift`).
const COMPACT_STRIP: f64 = 36.0;
/// The close button's frame origin. Search centres it at (26, strip / 2)
/// (`Lights.swift`); AppKit's button is about 14 x 14, so its origin is the
/// centre less 7. The integrator checks this against a screenshot.
pub const TRAFFIC_LIGHTS: gpui::Point<Pixels> = point(px(19.), px(19.));
/// Where the strip's controls start after the three lights (Search
/// `sideLights` 80 inside a 10 pt padding).
const CONTROLS_LEFT: f32 = 90.;
/// The page card's gap to the window and the sidebar's padding (Search
/// `Metrics.cardGap`, `Side.swift` `.padding(.horizontal, 10)`).
const PAGE_EDGE: f32 = 8.;
const COLUMN_PAD: f32 = 10.;
/// The page card's corner radius (Search `Metrics.cardRadius`).
const CARD_RADIUS: f32 = 10.;

/// One row of the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarTab {
    pub id: TabId,
    pub label: String,
    /// The tab address's host (`www.github.com`); empty for a blank or
    /// internal page. The tab's letter comes from it.
    pub host: String,
    /// The tab's address, for the site it says on the shown row.
    pub url: String,
    /// The Person's emoji or symbol for the tab.
    pub icon: Option<String>,
    pub favicon: Option<std::sync::Arc<gpui::RenderImage>>,
    pub pinned: bool,
    /// One of the profile's favourites rather than the space's own.
    pub favourite: bool,
    pub active: bool,
    pub loading: bool,
    pub audible: bool,
    /// It holds no page (it rests, or was not opened yet): Dim sleeping tabs.
    pub asleep: bool,
    /// How far its page is scrolled, nought to one: Show how far you've read.
    pub reading: f32,
}

/// One of the profile's spaces, as the sidebar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct SpaceChip {
    pub id: SpaceId,
    pub name: String,
    pub colour: u32,
    pub icon: String,
    /// The space this window shows.
    pub current: bool,
}

pub struct WindowFrame {
    window_id: u64,
    tx: UnboundedSender<AppMsg>,
    profile: Profile,
    can_go_back: bool,
    can_go_forward: bool,
    tabs: Vec<SidebarTab>,
    spaces: Vec<SpaceChip>,
    /// Where the chrome sits; what is pinned stays beside the page.
    layout: Layout,
    /// The shown tab's page had a popup refused.
    popup_blocked: bool,
    /// The profile's downloads this session, as the strip labels them.
    downloads: Option<String>,
    /// The profile has extensions installed (Search `ExtensionSlot`).
    has_extensions: bool,
    /// The shown page is bookmarked: the bookmark door is filled.
    bookmarked: bool,
    /// The frame is in full screen, and its width (overlays draw from this
    /// view but can't ask their own window).
    fullscreen: bool,
    window_width: f32,
    /// Dragging the sidebar's edge: where the press was, and the width then.
    resizing: Option<(f32, f32)>,
    /// The pointer is on the sidebar's edge.
    on_edge: bool,
    /// The pointer has been on the edge: its pill may fade from now on.
    grip_seen: bool,
    /// What the tab list remembers between frames (`tab_list`).
    tab_list: tab_list::TabListState,
    tab_hovers: tab_hover::TabHovers,
    /// ⌘ is held: the first tabs show their numbers.
    command_held: bool,
    /// Where the window's own buttons were last put, from the strip's top.
    lights_y: f32,
    /// The pinned column gliding between its full width and icons only.
    glide: Option<glide::Glide>,
    /// The profile's top-level bookmarks, for the bar above the page.
    marks: bookmarks_bar::Marks,
    /// How the tab rows are drawn by the tab preferences, and the player's rows.
    look: TabLook,
    player: Vec<PlayerTab>,
    home: home::Home,
}

/// Which window draws a piece of chrome: the frame (pinned) or an overlay
/// (sliding in over the page, with Aeri's own window buttons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Frame,
    Overlay,
}

impl WindowFrame {
    pub fn new(
        window_id: u64,
        profile: Profile,
        tx: UnboundedSender<AppMsg>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let close = tx.clone();
        window.on_window_should_close(cx, move |_, _| {
            // The shell closes the tabs first, then removes this window.
            let _ = close.unbounded_send(AppMsg::CloseWindow { window: window_id });
            false
        });
        Self {
            window_id,
            tx,
            profile,
            can_go_back: false,
            can_go_forward: false,
            tabs: Vec::new(),
            spaces: Vec::new(),
            layout: Layout::default(),
            popup_blocked: false,
            downloads: None,
            has_extensions: false,
            bookmarked: false,
            fullscreen: false,
            window_width: 1280.,
            resizing: None,
            on_edge: false,
            grip_seen: false,
            command_held: false,
            lights_y: TRAFFIC_LIGHTS.y.into(),
            glide: None,
            marks: bookmarks_bar::Marks::default(),
            tab_list: tab_list::TabListState::default(),
            tab_hovers: tab_hover::TabHovers::default(),
            look: TabLook::default(),
            player: Vec::new(),
            home: home::Home::new(cx),
        }
    }

    /// Mirrors the window's tabs, in order.
    pub fn set_tabs(&mut self, tabs: Vec<SidebarTab>, cx: &mut Context<Self>) {
        if self.tabs != tabs {
            self.tabs = tabs;
            cx.notify();
        }
    }

    /// Replaces one tab's row in place, when it is listed and changed: a
    /// page event on a tab the window does not show touches nothing else.
    /// Returns whether the row is listed.
    pub fn set_tab_row(&mut self, row: SidebarTab, cx: &mut Context<Self>) -> bool {
        let Some(old) = self.tabs.iter_mut().find(|tab| tab.id == row.id) else {
            return false;
        };
        if *old != row {
            *old = row;
            cx.notify();
        }
        true
    }

    /// Mirrors the profile's spaces, in order.
    pub fn set_spaces(&mut self, spaces: Vec<SpaceChip>, cx: &mut Context<Self>) {
        if self.spaces != spaces {
            self.spaces = spaces;
            cx.notify();
        }
    }

    /// Where the chrome sits. Swapping the full column for icons only (or
    /// back) glides ([`column_glides`]).
    pub fn set_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        if self.layout != layout {
            let from = self.drawn_width();
            let before = std::mem::replace(&mut self.layout, layout);
            self.follow_column(&before, from, cx);
            cx.notify();
        }
    }

    /// ⌘ went down or up anywhere in the app.
    pub fn set_command_held(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.command_held != held {
            self.command_held = held;
            if self.layout.tab_numbers {
                cx.notify();
            }
        }
    }

    /// The page is a card: the preference, and not full screen unless the
    /// card is kept there (Search `asCards`).
    fn cards_now(&self) -> bool {
        cards_in(&self.layout, self.fullscreen)
    }

    /// The sidebar is on the right.
    pub fn on_right(&self) -> bool {
        self.layout.side == Side::Right
    }

    /// The column's width in this window when it is at rest: fixed with
    /// icons only. While it glides ([`WindowFrame::drawn_width`]) the full
    /// column keeps its own width, which the glide only reveals.
    fn column_width(&self) -> f32 {
        let window = f64::from(self.window_width);
        if self.glide.is_some() {
            self.layout.sidebar_width_in(window) as f32
        } else {
            self.layout.column_width(window) as f32
        }
    }

    /// The pointer moved while the sidebar's edge is held: follow it.
    fn resize_to(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((from, width)) = self.resizing else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.end_resize(cx);
            return;
        }
        let moved = f32::from(event.position.x) - from;
        let moved = if self.layout.side == Side::Right {
            -moved
        } else {
            moved
        };
        let wanted = clamp_width((width + moved).round().max(0.) as u16);
        if wanted != self.layout.sidebar_width {
            self.layout.sidebar_width = wanted;
            self.send(AppMsg::Chrome(ChromeMsg::Width {
                width: wanted,
                done: false,
            }));
        }
        cx.notify();
    }

    /// The edge is let go: the width is kept.
    fn end_resize(&mut self, cx: &mut Context<Self>) {
        if self.resizing.take().is_some() {
            self.send(AppMsg::Chrome(ChromeMsg::Width {
                width: self.layout.sidebar_width,
                done: true,
            }));
            cx.notify();
        }
    }

    /// A double-click on the edge puts the width back (Search `Metrics.side`).
    fn reset_width(&mut self, cx: &mut Context<Self>) {
        self.resizing = None;
        self.layout.sidebar_width = DEFAULT_SIDEBAR;
        self.send(AppMsg::Chrome(ChromeMsg::Width {
            width: DEFAULT_SIDEBAR,
            done: true,
        }));
        cx.notify();
    }

    /// Shows the profile's current name and colour.
    pub fn set_profile(&mut self, profile: Profile, cx: &mut Context<Self>) {
        if self.profile != profile {
            self.profile = profile;
            cx.notify();
        }
    }

    /// The trust notices: a refused popup, the downloads label.
    pub fn set_notices(
        &mut self,
        popup_blocked: bool,
        downloads: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if (self.popup_blocked, &self.downloads) != (popup_blocked, &downloads) {
            self.popup_blocked = popup_blocked;
            self.downloads = downloads;
            cx.notify();
        }
    }

    /// Updates extension and bookmark doors for the active page.
    pub fn set_doors(&mut self, has_extensions: bool, bookmarked: bool, cx: &mut Context<Self>) {
        let now = (has_extensions, bookmarked);
        if (self.has_extensions, self.bookmarked) != now {
            (self.has_extensions, self.bookmarked) = now;
            cx.notify();
        }
    }

    /// The browser window this frame belongs to.
    pub fn window_id(&self) -> u64 {
        self.window_id
    }

    /// A plain text button in the strip; a click sends `msg` with the point
    /// clicked, where its menu hangs.
    fn notice(
        &self,
        id: &'static str,
        label: String,
        msg: fn(u64, f32, f32, Align) -> AppMsg,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let window = self.window_id;
        let strip = strip_height(&self.layout) as f32;
        let t = theme::tokens_in(self.window_id, cx);
        div()
            .id(id)
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(label.clone()))
            .flex()
            .flex_none()
            .items_center()
            .h(px(28.))
            .px(px(10.))
            .rounded(px(8.))
            .text_size(px(13.))
            .text_color(theme::c(t.icon))
            .hover(move |style| style.bg(theme::c(t.hover)))
            .active(move |style| style.bg(theme::c(t.pressed)))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, _| {
                let x = f32::from(event.position().x);
                this.send(msg(window, x, strip - 6., Align::Right));
            }))
            .child(SharedString::from(label))
    }

    /// Mirrors the active tab's history.
    pub fn show_history(
        &mut self,
        can_go_back: bool,
        can_go_forward: bool,
        cx: &mut Context<Self>,
    ) {
        if (self.can_go_back, self.can_go_forward) != (can_go_back, can_go_forward) {
            self.can_go_back = can_go_back;
            self.can_go_forward = can_go_forward;
            cx.notify();
        }
    }

    fn send(&self, msg: AppMsg) {
        let _ = self.tx.unbounded_send(msg);
    }
}

/// A stable Blobatar with the profile's current accent as its surround.
pub(crate) fn avatar(profile: &Profile, size: f32) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(theme::c(Rgba::rgb(profile.accent)))
        .child(img(crate::blobatar::image(profile.avatar_slot())).size(px(size * 0.8)))
}

/// The strip's height in `layout`: 52 pt, or 36 with tabs along the top
/// and "Shorter tab bar" on (Search `Metrics.strip`).
pub fn strip_height(layout: &Layout) -> f64 {
    if layout.compact_strip && layout.tabs == TabPlace::Top {
        COMPACT_STRIP
    } else {
        STRIP_HEIGHT
    }
}

/// Whether the page is a card in `layout` (Search `asCards`, `Browser.swift:2799`:
/// `sidebar && cards && (!fullScreen || cardsInFullScreen)`): tabs in the
/// sidebar, the preference, and in full screen only where the card is kept.
/// With tabs along the top the page runs edge to edge under the strip.
pub fn cards_in(layout: &Layout, fullscreen: bool) -> bool {
    layout.tabs == TabPlace::Side && layout.cards && (!fullscreen || layout.cards_full_screen)
}

/// Width a pinned sidebar (or column of icons) keeps beside the page; 0
/// when the sidebar slides in.
pub fn pinned_side_width(layout: &Layout, window_width: f64) -> f64 {
    if layout.sidebar_pinned {
        layout.column_width(window_width)
    } else {
        0.0
    }
}

/// Where the page sits in the frame, in points from each edge, and its
/// corner radius. `mac::Frame` places the page window by it and the frame
/// view draws the card's edge by it, so the two always agree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageInsets {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub radius: f64,
    /// The page is a card with room around it (its edge is drawn).
    pub card: bool,
    /// The bookmarks bar's height above the page, 0 when it is not up, and
    /// the points from the window's top to its top edge.
    pub bar: f64,
    pub bar_top: f64,
}

/// The page's place beside whatever is pinned, where `side` is the width
/// the pinned column takes ([`pinned_side_width`], or less or more while it
/// glides). Full screen pins nothing; the page then covers the frame, or is
/// a card with a gap all round where the card is kept there. As a card it
/// has Search's 8 pt gap and radius 10; edge to edge it meets the sidebar
/// (a hairline on the sidebar's edge) and the strip with no gap and square
/// corners (`Metrics.cardGap`, `cardRadius`, `Browser.asCards`).
pub fn page_insets(layout: &Layout, fullscreen: bool, side: f64) -> PageInsets {
    let cards = cards_in(layout, fullscreen);
    let radius = if cards { f64::from(CARD_RADIUS) } else { 0. };
    let gap = f64::from(PAGE_EDGE);
    let (side, top_pinned) = if fullscreen {
        (0., false)
    } else {
        (side, layout.top_pinned)
    };
    if side == 0. && !top_pinned {
        let kept = fullscreen && cards;
        let edge = if kept { gap } else { 0. };
        return PageInsets {
            left: edge,
            right: edge,
            top: edge,
            bottom: edge,
            radius,
            card: kept,
            bar: 0.,
            bar_top: 0.,
        };
    }
    let far = if cards { gap } else { 0. };
    let (left, right) = match layout.side {
        _ if side == 0. => (far, far),
        Side::Left => (side, far),
        Side::Right => (far, side),
    };
    // Search `App.swift:656-663`: with the card, the bar takes the place of
    // the card's top gap; without it, it comes on top of the strip's band.
    let bar = if bookmarks_bar::shown(layout, fullscreen) {
        bookmarks_bar::HEIGHT
    } else {
        0.
    };
    let band = if top_pinned { strip_height(layout) } else { 0. };
    let top = match (bar > 0., top_pinned) {
        (true, _) => band + bar,
        (false, true) => band,
        (false, false) => far,
    };
    PageInsets {
        left,
        right,
        top,
        bottom: far,
        radius,
        card: cards,
        bar,
        bar_top: band,
    }
}

impl WindowFrame {
    /// The page card's edge, behind the page (Search `CardEdge`,
    /// `Design.swift`): a rounded rectangle at the page's place with a 1 pt
    /// hairline just outside it and a soft shadow. The page window, rounded
    /// to the same radius, covers its inside. `None` while the page covers
    /// the frame or runs edge to edge.
    fn card_edge(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let insets = page_insets(
            &self.layout,
            self.fullscreen,
            f64::from(self.pinned_width()),
        );
        if !insets.card {
            return None;
        }
        let t = theme::tokens_in(self.window_id, cx);
        Some(
            div()
                .absolute()
                .left(px(insets.left as f32 - 1.))
                .right(px(insets.right as f32 - 1.))
                .top(px(insets.top as f32 - 1.))
                .bottom(px(insets.bottom as f32 - 1.))
                .rounded(px(insets.radius as f32 + 1.))
                .border_1()
                .border_color(theme::c(t.card_border))
                .bg(theme::c(t.panel))
                // Search: black at 0.07, radius 6, y 1.
                .shadow(vec![BoxShadow {
                    color: theme::c(t.card_shadow).into(),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(6.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
    }
}

/// Moves the window by the press that is under way; a double-click does what
/// a title bar's does (zooms, or minimises, or nothing, per System Settings)
/// once the handler is done: it resizes the frame's windows, which GPUI
/// must not hear of while it is dispatching the click.
fn begin_move(host: Host, double: bool, window: &mut Window, cx: &mut App) {
    let Some(view) = crate::shell::content_view(window) else {
        return;
    };
    if double {
        cx.spawn(async move |_| crate::mac::title_bar_double_click(view))
            .detach();
    } else {
        match host {
            Host::Frame => window.start_window_move(),
            Host::Overlay => crate::mac::drag_parent(view),
        }
    }
}

/// An empty part of the chrome that moves the window.
fn drag(host: Host) -> gpui::Div {
    div().h_full().on_mouse_down(
        MouseButton::Left,
        move |event: &MouseDownEvent, window, cx| {
            begin_move(host, event.click_count >= 2, window, cx);
        },
    )
}

impl Render for WindowFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width);
        if (self.fullscreen, self.window_width) != (window.is_fullscreen(), width) {
            // The overlays draw from this view too: let them catch up.
            self.fullscreen = window.is_fullscreen();
            self.window_width = width;
            cx.notify();
        }
        // The column's glide between full and icons only moves on here.
        self.step_glide(window, cx);
        // The window's own buttons centre in the strip: lower in the full
        // one, higher in the shorter one (Search `Lights.swift`).
        let lights_y = if self.layout.top_pinned {
            strip_height(&self.layout) as f32 / 2. - 7.
        } else {
            f32::from(TRAFFIC_LIGHTS.y)
        };
        if lights_y != self.lights_y {
            self.lights_y = lights_y;
            window.set_traffic_light_position(point(TRAFFIC_LIGHTS.x, px(lights_y)));
        }
        let frame = theme::backdrop_in(
            div().relative().size_full().flex().flex_col(),
            self.window_id,
            cx,
        )
        .id("browser-frame")
        .line_height(gpui::relative(theme::LINE_HEIGHT))
        .track_focus(&self.home.root)
        .on_key_down(cx.listener(Self::home_key_down))
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
            this.resize_to(event, cx);
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_resize(cx)),
        );
        // With nothing pinned the page covers the frame: draw nothing more.
        let side = self
            .layout
            .sidebar_pinned
            .then(|| self.sidebar(Host::Frame, cx));
        frame
            .children(self.card_edge(cx))
            .when(self.home.tab.is_some(), |frame| {
                frame.child(self.home(window, cx))
            })
            .children(self.bookmarks_bar(window, cx))
            .when(self.layout.top_pinned, |frame| {
                frame.child(self.strip(Host::Frame, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.layout.side == Side::Right, |row| {
                        row.flex_row_reverse()
                    })
                    .children(side),
            )
    }
}
