# Development

Status: Phase 1. Commands in this file were run on the owner's Mac (macOS
27.2, Xcode 27.2, Rust 1.100, Apple Silicon) unless marked otherwise.

## Prerequisites

| Need | Detail |
| --- | --- |
| Machine | Apple Silicon Mac. CEF has no universal macOS build, so Intel needs a separate x64 binary ([cef.md §1](research/cef.md)). Aeri targets arm64 first. |
| OS | macOS 14 or later. GPUI's idle-pausing display link needs it ([gpui.md Q9](research/gpui.md)). |
| Toolchain | Full Xcode, because GPUI compiles Metal shaders at build time. Xcode 26+ ships the Metal Toolchain as a separate component: `xcodebuild -downloadComponent MetalToolchain` (839 MB). Without it the `gpui_apple` build fails with "missing Metal Toolchain". Also CMake and Ninja (`brew install cmake ninja`); `cef-dll-sys` builds CEF's C++ wrapper with them. |
| Rust | Stable, edition 2024, installed with `rustup`. |
| Disk | About 2 GB free. The CEF framework is 341 MB on disk once unpacked ([cef.md §1](research/cef.md)). Debug builds, caches and bundles add to that. |
| Network | The first build downloads CEF. A first run downloads filter lists (Phase 4). |

No Apple Developer account is needed for local runs.

## CEF binaries

Aeri uses prebuilt CEF binaries from the Spotify CDN
(`cef-builds.spotifycdn.com`), stable channel: CEF 156.0.30 (Chromium 156),
through the `cef` crate pinned at `=156.6.0` ([cef.md §1, §2](research/cef.md)).

- With no `CEF_PATH`, the `cef-dll-sys` build script downloads CEF itself
  (cef-rs README). CI relies on this.
- Locally, export CEF once and point builds at it, so `cargo clean` does not
  download it again:

  ```sh
  git clone https://github.com/tauri-apps/cef-rs /tmp/cef-rs
  (cd /tmp/cef-rs && cargo run -p export-cef-dir -- --force "$HOME/.local/share/cef")
  export CEF_PATH="$HOME/.local/share/cef"
  ```

  Cargo's `[env]` cannot expand `$HOME`, so `CEF_PATH` is exported in the
  shell, not in `.cargo/config.toml`.
- Stock CEF lacks H.264/AAC and Widevine. This is expected until Phase 14
  ([ADR 0005](decisions/0005-media-codecs-and-drm.md)).

## Workspace layout

```
apps/aeri/          the binary: GPUI windows and views, AppKit glue (mac.rs)
apps/aeri-helper/   CEF subprocess binary
crates/aeri-core/   domain model, rules, persistence; no GPUI, no CEF
crates/aeri-cef/    CEF adapter for the engine seam
xtask/              `cargo xtask bundle`, `cargo xtask dev`, `cargo xtask dmg`, `cargo xtask isolation`
docs/               architecture, decisions, research, roadmap
.monozukuri/        blueprint (requirements, stack, plan) and phase specs
```

Where code goes:

| You are writing | Put it in |
| --- | --- |
| A rule, state machine, record or SQL migration | `crates/aeri-core` |
| Anything that names a CEF type | `crates/aeri-cef` |
| A GPUI view, menu, shortcut or AppKit call | `apps/aeri` |
| A renderer-process hook | `apps/aeri-helper` |

See [architecture/overview.md](architecture/overview.md). GPUI is a git
dependency pinned to a `zed-industries/zed` commit
([02-tech-stack.md](../.monozukuri/blueprint/02-tech-stack.md)).

## Build, bundle, run

A CEF app on macOS must be an `.app` bundle with helper bundles. A plain
`cargo run` of the main binary exits with "not inside an app bundle".

```sh
export CEF_PATH="$HOME/.local/share/cef"
cargo test --workspace
cargo xtask bundle                    # → target/bundle/aeri.app, ad-hoc signed
open target/bundle/aeri.app
```

`cargo xtask bundle` builds `aeri` and `aeri-helper`, then assembles the app,
the framework and the five helper bundles with cef-rs's `build_bundle`. It
copies Aeri's tab-signals extension into `Contents/Resources/tab-signals`
([ADR 0007](decisions/0007-tab-signals.md)), then ad-hoc signs the whole
bundle. It deletes the old bundle first: macOS
caches code signatures per file, and copying a new binary over a running or
recently run one gets it killed ("load code signature error 2").

### Edit loop

```sh
cargo xtask dev --use-mock-keychain   # build, refresh the bundle, run it
```

`cargo xtask dev [args…]` builds `aeri` and `aeri-helper`, replaces only the
executables that changed inside `target/bundle/aeri.app` (remove and copy,
never overwrite), re-signs the changed helpers and the bundle shallowly, and
runs the bundle binary with your arguments so logs print to the terminal. It
assembles the bundle first if there is none. After changing resources or
`Info.plist`, run `cargo xtask bundle` instead.

`cargo xtask` clears the `CARGO_MANIFEST_DIR` and `CARGO_PKG_*` variables
that `cargo run` exports before it builds. Left in, they invalidate `ring`'s
build script, so every bundle and every plain `cargo build` rebuilt `ring`
and everything above it (minutes).

To build the macOS installer:

```sh
cargo xtask bundle --release
cargo xtask dmg                     # → target/release-bundle/Aeri.dmg
```

The disk image uses Finder's standard drag to Applications. Packaging needs
a macOS desktop session with permission for the terminal to control Finder;
eject any mounted Aeri disk image before rebuilding. The build is ad-hoc
signed, not notarized for public distribution.

For the first-launch welcome, run with a new `AERI_DATA_DIR` and without
`AERI_START_URL` or `AERI_PROFILE`. A silent, full-screen film reveals
the Aeri ribbon and wordmark before the Spaces, Peek, and Split View tour.
Press **Esc** to skip the film. Reduce Motion starts at the tour, with
**Watch Intro** available.
**Skip Setup** goes straight to browsing without changing preferences.
**Start Browsing** saves changes to the tab layout and appearance. Quit
before either and the welcome appears next time.
**Help → Welcome to Aeri…** replays the flow without changing anything unless
you choose a setting and confirm it.

The [intro MP4](../apps/aeri/resources/aeri-intro.mp4) is bundled as-is.
Regenerate it with `xtask/intro/render.sh` (Node.js, npx, and FFmpeg).

New blank tabs show Aeri Home. Type a search or address directly into Home's
field and press Return to browse in that tab. Home also links to Bookmarks,
History, and Settings. **⌘T** and **⌘L** still open the floating composer.

### Profile avatars

Aeri assigns each profile a [Blobatar](https://blobatar.dev/) from a bundled
set based on its local profile ID. In **Profiles → Manage Profiles…**, use
**Choose avatar…** to select any of the 64 faces and **Use automatic avatar**
to reset it. Choices save immediately. Renaming a profile or changing its
accent does not change its face. This works offline and does not use website
accounts.

The asset pack at `apps/aeri/resources/blobatar.bin` is generated only when
updating the avatars. From the repository root, set `repo=$PWD`, then in a
scratch directory run:

```sh
bun add --exact blobatar@2.7.0 @resvg/resvg-js@2.6.2
cp "$repo/xtask/blobatar.mjs" .
bun blobatar.mjs "$repo/apps/aeri/resources/blobatar.bin"
```

Keep the avatar count and seed order fixed to preserve existing assignments.
The [upstream MIT notice](../apps/aeri/resources/Blobatar-LICENSE.txt) ships with Aeri.

Development switches:

| Variable or flag | Effect |
| --- | --- |
| `AERI_DATA_DIR=/tmp/aeri-data` | Use a throwaway data directory instead of `~/Library/Application Support/Aeri`. |
| `AERI_START_URL=https://example.com` | Open one window on this address per opened profile instead of restoring each profile's session. Without it, a profile with no session opens Aeri Home. |
| `AERI_REMOTE_DEBUGGING_PORT=9334` | Expose the DevTools protocol for scripted checks. |
| `AERI_PROFILE=Work,Client` | Open (or create) these profiles, each in its own window, instead of the last-used one. For scripted checks; the Profiles menu is the Person's way. |
| `AERI_DOWNLOAD_DIR=/tmp/aeri-downloads` | Save downloads here instead of `~/Downloads`. |
| `AERI_REST_AFTER_SECS=20` | Rest tabs unseen this many seconds, checking as often, instead of the Person's period. |
| `AERI_LOG=info,aeri=debug` | `tracing` filter (stdout; colours only on a terminal). |
| `--use-mock-keychain` | Chromium switch: skip the Keychain. Use it for scripted runs; see below. |
| `--autoplay-policy=no-user-gesture-required` | Chromium switch: let a script play sound without a click (tab-signals checks). |

Camera and microphone checks: macOS attributes the permission prompt to the
app that launched Aeri (your terminal), not to Aeri, even with Chromium's
fake-device switches. Run them from `open target/bundle/aeri.app` instead.

Run the bundle binary directly to see logs:

```sh
AERI_DATA_DIR=/tmp/aeri-data AERI_REMOTE_DEBUGGING_PORT=9334 \
  ./target/bundle/aeri.app/Contents/MacOS/aeri --use-mock-keychain
```

`kill <pid>` (SIGTERM) and Ctrl-C quit like the Quit menu: browsers close
and CEF shuts down, so cookies are written to disk, and each open profile's
session is saved. Aeri replaces Chromium's own handlers, which end the
process at once and lose recent cookies. Closing the last window does not
quit; the Dock icon opens a window again (that profile's session).

Keys (from a page or the address bar): ⌘L address bar, ⌥⌘P command
palette, ⌘T new tab, ⌘N new window (on a space no window shows), ⌘W close
tab, ⌘⇧T reopen the last closed tab, ⌃Tab / ⌃⇧Tab next and previous tab,
⌘1–⌘8 that tab (favourites first), ⌘9 the last tab, ⌘⌥← / ⌘⌥→ previous and
next space, ⌘D bookmark the page (again: remove it), ⌥⌘B bookmarks, ⌘Y
history, ⌘S show or hide the sidebar, ⌃⌘F full screen, ⌘Q quit. In the bar,
↑ / ↓ pick a suggestion. A profile's records (spaces, favourites, windows,
closed tabs, history, bookmarks) are in `Profiles/<id>/profile.db` under
the data directory; `sqlite3` reads it once Aeri has quit.

Trust (Phase 9): the strip shows **Popup blocked** while the shown tab's
page was refused a popup it opened without a click (its menu opens it once
or always allows the site, which Chrome's site settings then show), and a
downloads button once the profile has downloaded something (cancel, reveal
in Finder, Show All Downloads). The tab menu and palette have **Site
Settings…** (Chrome's per-site permissions page) and **Open in Default
Browser**; the palette has **Show Downloads** and **Clear Browsing Data…**
(Chrome's page); the library's History has **Clear History…** by range. A
certificate warning passed with Proceed shows again after relaunch.
Chrome's own history recording is off, so its `History` database keeps
only the downloads list.

Passwords (Phase 10, ADR 0009): each profile's provider is in the profile
menu (Save Passwords in Aeri, Use a Password-Manager Extension, Don't Save
Passwords) with Manage Passwords… (also in the palette), which opens
Chrome's password manager page. Chrome's "Save password?" bubble follows a
submitted sign-in; a saved one is listed when you click the field and
filled only after macOS confirms it is you. Scripted checks must not
answer that prompt; quitting Aeri dismisses it.

Resting (Phase 11): tabs no window shows rest after the period chosen in
the Resources window (palette: **Show Resources**; `app.db` setting
`rest_after_minutes`, default an hour): the page closes, the tab keeps its
place and title, and showing it loads the address again. Tabs playing
sound, using the camera or microphone, typed into since they loaded, or on
a site kept awake (tab menu; `profile.db` table `keep_awake_sites`) stay.
macOS memory pressure rests the longest-unseen tabs first; checking that
needs root: `sudo memory_pressure -S -l warn` (then `-l normal`). The
Resources window samples Chromium's task manager only while open.

Appearance (Phase 12): the chrome is page-first. The sidebar and top bar
are transparent child windows (`chrome_overlay.rs`) that slide in when the
pointer rests 120 ms in a 4 pt lane at the window's edge (`mac/reveal.rs`,
an app-local event monitor) or by ⌘S / ⌘L; Pin keeps them beside the page.
The layout is one JSON value in `app.db` (`layout`); the app theme is
`app_theme`, the light/dark choice `appearance`, the keymap `keymap` (only
changes from the defaults). A profile's theme lives in its `profile.db`
settings (`theme`), a space's in `spaces.theme`; the Person's theme files
in `<data>/Themes/*.aeritheme`. Views paint only with tokens
(`theme::tokens_in(window, cx)` for a browser window, `theme::tokens(cx)`
for others). Settings is ⌘, (also the Aeri menu and the palette); the
theme editor opens from the palette (Edit Space Theme…, Edit Profile
Theme…, Edit Theme for All Windows…), the tab editor from the tab menu
(Rename Tab…). Scripted checks that drive the GUI must confirm Aeri is
frontmost before every key or click: another app in front receives them.

Running under `lldb` (launch or attach) raises a system "Developer Tools
Access" dialog and stalls until it is answered. Aeri also needs fds 3 and 4
free at start for its private DevTools pipe (ADR 0006); if a parent process
leaves them open, Aeri logs "no DevTools pipe" and extension buttons do
nothing.

### Keychain

Chromium encrypts cookies and saved passwords with a key it keeps in the
Keychain. Aeri uses its own item, "Aeri Safe Storage" (account "Aeri"),
through CEF's `keychain_service_name` settings. The default item, "Chromium
Safe Storage", is shared by every Chromium-based app. If an old build asks
for "Chromium Safe Storage", deny it.

Each rebuild changes the ad-hoc signature, so macOS may ask again whether
`aeri` may use "Aeri Safe Storage". While that prompt is open, page loads
stall. For scripted runs, pass `--use-mock-keychain` with a throwaway
`AERI_DATA_DIR`.

## Signing

- **Local runs:** `cargo xtask bundle` ad-hoc signs the bundle
  (`codesign --force --deep --sign -`); the hardened runtime is off.
  `codesign --verify --deep --strict target/bundle/aeri.app` passes.
  Chromium logs "Signature validation of current process failed (-67030)"
  at start-up on ad-hoc builds. It has no visible effect so far; recheck it
  when real signing lands.
- **Release:** hardened runtime, the JIT entitlement on the renderer and GPU
  helpers (Reported in [cef.md §8](research/cef.md)), notarisation
  (planned, Phase 15).
- Ad-hoc builds have no passkey entitlement. Platform passkeys and Apple
  Passwords do not work in them ([cef.md §9](research/cef.md),
  [blocking-and-credentials.md Q4](research/blocking-and-credentials.md)).
- Bundle identity matters: macOS ties notification permission to the bundle
  id `io.github.kuya-egg.aeri` ([cef.md §8](research/cef.md)).

## Logs

Nothing is sent anywhere.

- Aeri's own logs: `tracing` to stderr, filtered by `AERI_LOG` (default
  `info`). A rotating log file is not set up yet.
- Chromium's log: `<data dir>/chromium.log`.
- Errors: `thiserror` in libraries, `anyhow` at the app edge.

## Test strategy

From handoff §11 and the stack map.

| Level | Where | How |
| --- | --- | --- |
| Unit | `crates/aeri-core` | Profile lifecycle, tab and space state, session serialisation, navigation state, address classification, download-path cleaning, migrations. Each runs on an **in-memory SQLite**. No GPUI, no CEF. |
| Integration | `crates/aeri-cef` and `apps/aeri` | Chromium lifecycle, request-context isolation, navigation events, downloads, permissions, popups, crashes, session restore. Run against **local test pages** served from the repo. |
| End to end | `cargo xtask isolation` (profiles); more to come | Two profiles on a local site the task serves: sign-ins, cookies, local storage, IndexedDB, HTTP cache, restart, dormancy. Runs the bundled app; needs a logged-in macOS session. Not in CI yet. |

Rules:

- Automated tests never depend on an external website.
- Each stage-1 edge case becomes a test in the module that owns the rule
  ([03-logic-to-stack-map.md](../.monozukuri/blueprint/03-logic-to-stack-map.md)).
- Chromium-owned behaviour (popups, certificates, isolation) is tested at the
  integration level, not mocked.
- Spikes S1–S7 are experiments with written pass criteria, not tests. See
  the [roadmap](roadmap.md).

CI is `.github/workflows/ci.yml` on a macOS arm64 runner: fmt, clippy with
`-D warnings`, tests, bundle, signature check. It has not run on GitHub yet.
`cargo-deny` licence and advisory checks are not added yet.

## Repository conventions

| Thing | Location |
| --- | --- |
| Architecture docs | `docs/architecture/` |
| Decisions (ADRs, numbered `NNNN-title.md`) | `docs/decisions/` |
| Research and evidence | `docs/research/` |
| Phases | [`docs/roadmap.md`](roadmap.md) |
| Blueprint: requirements, stack, stack map, phase plan | `.monozukuri/blueprint/` |

- One convention for ADRs: `docs/decisions/`, not `.monozukuri/`.
- Research claims carry a label: Observed, Reported or Inferred. Keep it when
  you cite them.
- A decision that changes the architecture needs a new ADR or an update to an
  existing one. Do not silently change a source document.
- Add a crate only when an ownership boundary appears (handoff §9).
- Never commit secrets, signing keys or notarisation credentials.
