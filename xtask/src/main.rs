//! Developer tasks.
//!
//! - `cargo xtask bundle [--release]` — builds `aeri` and `aeri-helper` and
//!   assembles `target/bundle/aeri.app` (development build) or, with
//!   `--release`, an optimised `target/release-bundle/aeri.app`, the one to
//!   use day to day: motion in a development build stutters. The CEF
//!   framework and helper apps are put in place; CEF finds them through
//!   this layout, so the app cannot run outside it on macOS.
//! - `cargo xtask dmg` — packages the already built, signed
//!   `target/release-bundle/aeri.app` (run `bundle --release` first) into
//!   the read-only `target/release-bundle/Aeri.dmg`: the bundle, an
//!   `Applications` link and a static Finder background with install
//!   instructions (see `dmg.rs`). Needs the terminal to be allowed to
//!   control Finder; the previous image is replaced only once the new one
//!   verifies.
//! - `cargo xtask dev [args…]` — the edit loop: builds `aeri` and
//!   `aeri-helper`, refreshes the executables inside an existing
//!   `target/bundle/aeri.app` (assembling it first if missing), re-signs
//!   the bundle shallowly and runs it with `args`. Run `bundle` instead
//!   after changing resources or the Info.plist.
//! - `cargo xtask isolation` — profile isolation checks against a local
//!   test site, on the bundled app (see `isolation.rs`).

mod dmg;
mod isolation;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    // `cargo run` (the `xtask` alias) exports the xtask's CARGO_MANIFEST_DIR
    // and CARGO_PKG_* to us, and the nested `cargo build` inherits them.
    // Build scripts that track those (ring) then rebuild, and everything
    // above them, on every bundle and again on every plain `cargo build`:
    // minutes lost per run. Nothing here reads them at run time.
    let leaked: Vec<_> = std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| {
            let key = key.to_string_lossy();
            key == "CARGO_MANIFEST_DIR"
                || key == "CARGO_CRATE_NAME"
                || key.starts_with("CARGO_PKG_")
                || key.starts_with("CARGO_BIN_")
        })
        .collect();
    for key in leaked {
        // SAFETY: single-threaded; nothing has been spawned yet.
        unsafe { std::env::remove_var(key) };
    }
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "bundle" => bundle(std::env::args().nth(2).as_deref() == Some("--release")),
        "dev" => dev(std::env::args().skip(2).collect()),
        "dmg" => dmg::run(),
        "isolation" => isolation::run(),
        _ => bail!("usage: cargo xtask bundle [--release] | dev [args…] | dmg | isolation"),
    }
}

/// Edit loop: refresh the executables of an existing development bundle
/// and run it. Executables are replaced by remove-and-copy, never
/// overwritten (see `bundle`), and only the changed ones.
#[cfg(target_os = "macos")]
fn dev(args: Vec<String>) -> Result<()> {
    use std::path::Path;

    for bin in ["aeri", "aeri-helper"] {
        let status = std::process::Command::new(env!("CARGO"))
            .args(["build", "--bin", bin])
            .status()?;
        if !status.success() {
            bail!("build of {bin} failed: {status}");
        }
    }
    let app = Path::new("target/bundle/aeri.app");
    if !app.join("Contents/MacOS/aeri").exists() {
        bundle(false)?;
        return run_bundle(app, &args);
    }
    let sign = |path: &Path| -> Result<()> {
        let status = std::process::Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(path)
            .status()?;
        if !status.success() {
            bail!("codesign of {} failed: {status}", path.display());
        }
        Ok(())
    };
    // Replaced by remove-and-copy, and only when the build is newer.
    let refresh = |from: &Path, to: &Path| -> Result<bool> {
        let newer = to.metadata()?.modified()? < from.metadata()?.modified()?;
        if newer {
            std::fs::remove_file(to)?;
            std::fs::copy(from, to)?;
        }
        Ok(newer)
    };
    let mut changed = refresh(
        Path::new("target/debug/aeri"),
        &app.join("Contents/MacOS/aeri"),
    )?;
    for entry in std::fs::read_dir(app.join("Contents/Frameworks"))? {
        let helper = entry?.path();
        let Some(stem) = helper.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with("aeri Helper") {
            let exe = helper.join("Contents/MacOS").join(stem);
            if refresh(Path::new("target/debug/aeri-helper"), &exe)? {
                sign(&helper)?;
                changed = true;
            }
        }
    }
    if changed {
        // Shallow: signs the main executable and reseals the bundle; the
        // framework and untouched helpers keep their signatures.
        sign(app)?;
    }
    run_bundle(app, &args)
}

/// Replaces this process with the bundle's executable, for its logs.
#[cfg(target_os = "macos")]
fn run_bundle(app: &std::path::Path, args: &[String]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(app.join("Contents/MacOS/aeri"))
        .args(args)
        .exec();
    bail!("could not run the bundle: {err}")
}

#[cfg(not(target_os = "macos"))]
fn dev(_args: Vec<String>) -> Result<()> {
    bail!("bundling is only needed on macOS")
}

#[cfg(target_os = "macos")]
fn bundle(release: bool) -> Result<()> {
    use cef::build_util::mac::{BundleInfo, build_bundle};
    use cef::build_util::metadata::get_cargo_metadata;

    let out_dir = std::path::PathBuf::from(if release {
        "target/release-bundle"
    } else {
        "target/bundle"
    });
    // Never overwrite executables in place: macOS caches code signatures per
    // file and kills the new binary ("load code signature error 2").
    let app = out_dir.join("aeri.app");
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
    }
    std::fs::create_dir_all(&out_dir)?;
    let info = BundleInfo::new(
        "aeri",
        "io.github.kuya-egg.aeri",
        "Aeri",
        "en",
        semver::Version::parse(env!("CARGO_PKG_VERSION"))?,
    );
    let built = if release {
        // cef-rs's `build_bundle` only builds the development profile.
        let metadata = get_cargo_metadata().map_err(|err| anyhow::anyhow!("{err:?}"))?;
        let helper = metadata
            .parse_bundle_metadata("aeri")
            .map_err(|err| anyhow::anyhow!("{err:?}"))?;
        let status = std::process::Command::new(env!("CARGO"))
            .args(["build", "--release", "--bin", "aeri", "--bin"])
            .arg(&helper.helper_name)
            .status()?;
        if !status.success() {
            bail!("release build failed: {status}");
        }
        cef::build_util::mac::bundle(
            &out_dir,
            &metadata.target_directory().join("release"),
            "aeri",
            &helper.helper_name,
            helper.resources_path,
            info,
        )
    } else {
        build_bundle(&out_dir, "aeri", info)
    };
    let app = match built {
        Ok(app) => app,
        Err(err) => bail!("bundling failed: {err:?}"),
    };
    // Aeri's own extension (ADR 0007), loaded from the bundle at startup.
    let signals = app.join("Contents/Resources/tab-signals");
    std::fs::create_dir_all(&signals)?;
    for entry in std::fs::read_dir("apps/aeri/resources/tab-signals")? {
        let entry = entry?;
        std::fs::copy(entry.path(), signals.join(entry.file_name()))?;
    }
    std::fs::copy(
        "apps/aeri/resources/Search-LICENSE.txt",
        app.join("Contents/Resources/Search-LICENSE.txt"),
    )?;
    std::fs::copy(
        "apps/aeri/resources/Blobatar-LICENSE.txt",
        app.join("Contents/Resources/Blobatar-LICENSE.txt"),
    )?;
    std::fs::copy(
        "apps/aeri/resources/aeri-intro.mp4",
        app.join("Contents/Resources/aeri-intro.mp4"),
    )?;
    // What macOS shows when a site the Person allowed first uses a device
    // (cef-rs writes the bare app name; location had none).
    let plist = app.join("Contents/Info.plist");
    for (key, text) in [
        ("CFBundleName", "Aeri"),
        (
            "NSCameraUsageDescription",
            "Websites you allow can then use your camera.",
        ),
        (
            "NSMicrophoneUsageDescription",
            "Websites you allow can then use your microphone.",
        ),
        (
            "NSLocationUsageDescription",
            "Websites you allow can then know your location.",
        ),
        (
            "NSLocationWhenInUseUsageDescription",
            "Websites you allow can then know your location.",
        ),
        (
            "NSBluetoothAlwaysUsageDescription",
            "Websites you allow can then connect to Bluetooth devices.",
        ),
        (
            "NSWebBrowserPublicKeyCredentialUsageDescription",
            "Websites can then let you sign in with a passkey.",
        ),
    ] {
        let status = std::process::Command::new("plutil")
            .args(["-replace", key, "-string", text])
            .arg(&plist)
            .status()?;
        if !status.success() {
            bail!("could not set {key}: {status}");
        }
    }
    // The links Aeri can be sent (http, https, mailto) and the pages it can
    // open (Finder's .html): without these macOS never offers Aeri as the
    // default browser or email app, and Settings' "Make default" cannot
    // work. Documents are an alternate, so Aeri never takes .html over.
    for (key, json) in [
        (
            "CFBundleURLTypes",
            r#"[{"CFBundleURLName":"Web site URL","CFBundleTypeRole":"Viewer","CFBundleURLSchemes":["http","https"]},{"CFBundleURLName":"Email URL","CFBundleTypeRole":"Viewer","CFBundleURLSchemes":["mailto"]}]"#,
        ),
        (
            "CFBundleDocumentTypes",
            r#"[{"CFBundleTypeName":"HTML document","CFBundleTypeRole":"Viewer","LSHandlerRank":"Alternate","LSItemContentTypes":["public.html","public.xhtml"]}]"#,
        ),
    ] {
        let status = std::process::Command::new("plutil")
            .args(["-replace", key, "-json", json])
            .arg(&plist)
            .status()?;
        if !status.success() {
            bail!("could not set {key}: {status}");
        }
    }
    // App icon (Light/Dark/Tinted): the Icon Composer document is compiled
    // by actool into Assets.car + AppIcon.icns inside the bundle; the two
    // Info.plist keys point macOS at them. Regenerate the glyph asset with
    // apps/aeri/resources/icon/make_icon.py.
    let partial = out_dir.join("AppIcon-partial.plist");
    let status = std::process::Command::new("xcrun")
        .args(["actool", "apps/aeri/resources/AppIcon.icon", "--compile"])
        .arg(app.join("Contents/Resources"))
        .args([
            "--platform",
            "macosx",
            "--minimum-deployment-target",
            "11.0",
            "--app-icon",
            "AppIcon",
            "--output-partial-info-plist",
        ])
        .arg(&partial)
        .stdout(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        bail!("actool failed: {status}");
    }
    std::fs::remove_file(&partial)?;
    for key in ["CFBundleIconName", "CFBundleIconFile"] {
        let status = std::process::Command::new("plutil")
            .args(["-replace", key, "-string", "AppIcon"])
            .arg(&plist)
            .status()?;
        if !status.success() {
            bail!("could not set {key}: {status}");
        }
    }
    // Ad-hoc signature over the whole bundle (helpers and framework
    // included); the per-binary linker signatures leave it unverifiable.
    // Hardened runtime stays off for development builds.
    let status = std::process::Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(&app)
        .status()?;
    if !status.success() {
        bail!("codesign failed: {status}");
    }
    println!("{}", app.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn bundle() -> Result<()> {
    bail!("bundling is only needed on macOS")
}
