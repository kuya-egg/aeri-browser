<p align="center">
  <img src="docs/assets/aeri-icon-light.png" width="96" height="96" alt="Black Aeri ribbon">
</p>

<h1 align="center">Aeri</h1>

<p align="center">
  An open-source, native-first desktop browser built around how you work.
</p>

<p align="center">
  <a href="#build-it-yourself">Build from source</a> · <a href="CONTRIBUTING.md">Contribute</a> · <a href="CHANGELOG.md">What’s new</a>
</p>

## What is Aeri?

Aeri is a desktop browser built with Rust and GPUI for the interface and
Chromium through CEF for web rendering. We're building it for developers,
engineers, creators, and anyone who spends much of their day in a browser and
wants more control over how they work.

## Why we're building it

We want the productivity of Arc, the web compatibility of Chrome, and the
responsiveness of a native desktop app, without tying that experience to one
company's ecosystem. We're not trying to put another skin on Chromium. We're
rethinking the application around the engine.

| Browser | What we want to take further |
| --- | --- |
| Chrome | Keep broad web compatibility while making it easier to organize work by project. |
| Arc | Keep the focus on workspaces while letting people inspect and shape the code that powers them. |
| Safari | Keep the feel of a native app while targeting Chromium compatibility and Chrome Web Store extensions. |
| Brave | Give people privacy controls without keeping the conventional tab-first layout. |
| Zen | Share its focus on better workflows, with a native Rust interface and a Chromium engine. |

## Organize by context, not just by tab

**The browser should adapt to how you work, not force you to adapt to how it manages tabs.**

- **Profiles** separate cookies, sessions, history, extensions, and saved credentials. Keep work, personal, and development accounts apart.
- **Spaces** organize projects and activities within a profile. Group tabs and favourites for a project or a research thread without starting another browser window.
- **Customization** lets you choose where tabs go, change themes by Space, remap shortcuts, and adjust how browsing behaves.
- **Tools for staying in context** include Split View for two live pages, Peek for a quick link preview, Reading Mode, floating video, and a command palette for jumping between tabs, Spaces, and profiles.

The goal isn't to beat Chrome in market share. It's to make a browser a
community can inspect, contribute to, and shape around its own work.

## How it's built

| Layer | What it does |
| --- | --- |
| **Rust + GPUI** | Draws Aeri's interface and handles interactions such as tabs, shortcuts, and window controls. |
| **Aeri core** | Owns profiles, Spaces, session state, and tab and resource-management rules. |
| **Chromium + CEF** | Renders web pages and provides the browser engine and extension runtime. |
| **Platform integration** | Connects Aeri to operating system services like windows, menus, and credential storage. |

Using Chromium lets us focus on how people browse instead of rewriting the web
engine. Rust and GPUI alone don't make Aeri faster or lighter than Chrome.
Those are engineering goals that need benchmarks, deliberate tab lifecycle
management, and careful CEF integration. See the
[architecture overview](docs/architecture/overview.md) for the implementation.

## Platform support

**macOS, Windows, and Linux** are the long-term targets. Aeri is in early
development. The current source build runs on Apple Silicon Macs; Intel Mac,
Windows, and Linux builds aren't available yet. There is no published installer
yet.

## Media compatibility

The standard build uses stock CEF and does not support H.264/AAC, so some
videos won’t play. DRM playback is a separate limitation: services that require
signed Widevine verification may refuse to play. See the
[media compatibility decision](docs/decisions/0005-media-codecs-and-drm.md).

---

## Build it yourself

These commands build the current macOS version of Aeri.

```text
apps/
  aeri/               Native macOS GPUI app, views, menus, shortcuts, AppKit glue
  aeri-helper/        CEF subprocess helper binary
crates/
  aeri-core/          Domain model, layout/theme rules, SQLite persistence (no GPUI, no CEF)
  aeri-cef/           CEF engine adapter
xtask/                Bundle, DMG installer, and isolation tooling
docs/                 Architecture, ADRs, research, and development notes
```

### Prerequisites

- **Apple Silicon Mac** running **macOS 14 Sonoma** or later
- **Full Xcode** with the **Metal Toolchain** installed (`xcodebuild -downloadComponent MetalToolchain` on Xcode 26 or newer)
- **Rust** stable (edition 2024)
- **CMake** and **Ninja** (`brew install cmake ninja`)

### Build & run

CEF apps on macOS must run from a signed `.app` bundle (plain `cargo run` will exit):

```bash
git clone https://github.com/kuya-egg/aeri-browser.git
cd aeri-browser
cargo xtask bundle                         # -> target/bundle/aeri.app (ad-hoc signed)
open target/bundle/aeri.app
```

The first build downloads the pinned CEF binaries automatically. You don’t need
to compile Chromium from source for the standard build.

To build an optimised Aeri and install it in /Applications (quitting and
reopening a running Aeri):

```bash
cargo xtask install
```

To build the release macOS installer DMG:

```bash
cargo xtask bundle --release
cargo xtask dmg                            # -> target/release-bundle/Aeri.dmg
```

See the [development guide](docs/development.md) for CEF caching, local testing, and troubleshooting.

## Contributing

Bug reports, documentation fixes, and code contributions are welcome. Read
[CONTRIBUTING.md](CONTRIBUTING.md) before opening an issue or pull request.

## License

Aeri is licensed under [Apache License 2.0](LICENSE). Portions adapted from
Search retain their [MIT license notice](apps/aeri/resources/Search-LICENSE.txt).
Profile avatars from [Blobatar](https://blobatar.dev/) retain their
[MIT license notice](apps/aeri/resources/Blobatar-LICENSE.txt).
