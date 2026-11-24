# Contributing to Aeri

Bug reports, documentation fixes, and code contributions are welcome. Be respectful and keep feedback focused on the work.

## Open an issue

- Search [existing issues](https://github.com/kuya-egg/aeri-browser/issues) before opening a new one.
- For a bug, include the Aeri version or commit, your operating system and version, steps to reproduce, and what you expected versus what happened. For a site-specific issue, include a public URL or a minimal example if you can share it.
- For a larger feature or architecture change, open an issue first to discuss the scope. Small fixes can go straight to a pull request.
- Don’t post passwords, cookies, tokens, private URLs, or unsanitized logs and screenshots. For a suspected security or privacy vulnerability, don’t post the details publicly. Use the repository’s [Security tab](https://github.com/kuya-egg/aeri-browser/security) if private reporting is available; otherwise, open an issue asking for a private contact without describing the vulnerability.

## Prepare a pull request

1. Fork the repo, branch from the latest upstream `dev`, and open your PR against `dev`. Use the setup for your platform below.
2. Keep the PR focused on one change. Add a regression test for bug fixes where practical, and use local fixtures instead of live websites in automated tests.
3. Run the checks below for code changes. For UI changes, launch the bundle and include screenshots or a short recording. Describe what you tested and anything you couldn’t verify.
4. Link the related issue if there is one. Update the relevant docs or changelog for user-visible changes. Keep third-party license notices and attribute borrowed code and assets.

`dev` is the default branch for contributions. `main` is the release branch:
maintainers promote `dev` to `main` by pull request using a merge commit so
the branches keep their shared history. Both branches block force pushes and
deletion, and require passing Windows core CI and resolved conversations
before merging.

## Run the checks

### Windows

You can contribute to documentation and [`aeri-core`](crates/aeri-core),
the Rust crate for profiles, Spaces, session state, and persistence. It
doesn't require GPUI or CEF. Install [stable Rust via rustup](https://rustup.rs/)
with the default MSVC toolchain and [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio)
with the **Desktop development with C++** workload. From the repository root
in PowerShell, run the same test command as the Windows job in
[CI](.github/workflows/ci.yml):

```powershell
cargo test --locked -p aeri-core
```

The full browser, workspace-wide checks, and app bundling still require an
Apple Silicon Mac. If you can't run those locally, say so in the PR.

### macOS

Follow the [development guide](docs/development.md) to build Aeri on an Apple
Silicon Mac. These are the macOS checks in [CI](.github/workflows/ci.yml):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask bundle
codesign --verify --deep --strict target/bundle/aeri.app
```

For profile or cookie-isolation changes, also run `cargo xtask isolation` from a logged-in macOS session after bundling. Documentation-only changes don’t need a full build; check the rendered Markdown, relative links, and commands you changed.

By submitting a contribution, you agree to license it under the project’s [Apache License 2.0](LICENSE).
