//! Developer tasks.
//!
//! - `cargo xtask bundle [--release]` — builds `aeri` and `aeri-helper` and
//!   assembles `target/bundle/aeri.app` (development build) or, with
//!   `--release`, an optimised `target/release-bundle/aeri.app`, the one to
//!   use day to day: motion in a development build stutters. The CEF
//!   framework and helper apps are put in place; CEF finds them through
//!   this layout, so the app cannot run outside it on macOS.
//! - `cargo xtask install` — `bundle --release`, then quits a running Aeri,
//!   replaces `/Applications/Aeri.app` with the new build and opens it.
//! - `cargo xtask dmg` — packages the already built, signed
//!   `target/release-bundle/aeri.app` (run `bundle --release` first) into
//!   the read-only `target/release-bundle/Aeri.dmg`: the bundle, an
//!   `Applications` link and a static Finder background with install
//!   instructions (see `dmg.rs`). Needs the terminal to be allowed to
//!   control Finder; the previous image is replaced only once the new one
//!   verifies.
//! - `cargo xtask isolation` — profile isolation checks against a local
//!   test site, on the bundled app (see `isolation.rs`).

mod dmg;
mod isolation;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "bundle" => bundle(std::env::args().nth(2).as_deref() == Some("--release")),
        "install" => install(),
        "dmg" => dmg::run(),
        "isolation" => isolation::run(),
        _ => bail!("usage: cargo xtask bundle [--release] | install | dmg | isolation"),
    }
}

/// The certificate bundles are signed with; `-` is ad hoc.
#[cfg(target_os = "macos")]
fn sign_identity() -> String {
    const LOCAL: &str = "Aeri Local Signing";
    if let Ok(identity) = std::env::var("AERI_SIGN_IDENTITY") {
        return identity;
    }
    let listed = std::process::Command::new("security")
        .args(["find-identity", "-p", "codesigning"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(&format!("\"{LOCAL}\"")))
        .unwrap_or(false);
    if listed { LOCAL.into() } else { "-".into() }
}

/// Builds the release bundle, quits a running Aeri (an orderly quit, so
/// its windows come back), puts the new app in /Applications and opens it.
#[cfg(target_os = "macos")]
fn install() -> Result<()> {
    const INSTALLED: &str = "/Applications/Aeri.app";
    let main = format!("{INSTALLED}/Contents/MacOS/aeri$");
    bundle(true)?;
    let running = || {
        std::process::Command::new("pgrep")
            .args(["-f", &main])
            .output()
            .is_ok_and(|out| out.status.success())
    };
    if running() {
        println!("quitting Aeri");
        // SIGTERM takes Aeri's orderly quit (`shell::quit`). An Apple
        // Event "quit" may reach a helper instead: the helper apps carry
        // the app's own bundle id.
        std::process::Command::new("pkill")
            .args(["-TERM", "-f", &main])
            .status()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while running() {
            if std::time::Instant::now() > deadline {
                bail!("Aeri did not quit; quit it and run `cargo xtask install` again");
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    let installed = std::path::Path::new(INSTALLED);
    if installed.exists() {
        std::fs::remove_dir_all(installed)?;
    }
    let status = std::process::Command::new("ditto")
        .args(["target/release-bundle/aeri.app", INSTALLED])
        .status()?;
    if !status.success() {
        bail!("copying to {INSTALLED} failed: {status}");
    }
    std::process::Command::new("open").arg(INSTALLED).status()?;
    println!("installed {INSTALLED}");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn install() -> Result<()> {
    bail!("installing is only needed on macOS")
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
    // One signature over the whole bundle (helpers and framework included);
    // the per-binary linker signatures leave it unverifiable. Hardened
    // runtime stays off for development builds.
    //
    // Ad hoc unless a certificate is found. An ad hoc signature changes with
    // every build, and the Keychain item "Aeri Safe Storage" trusts one
    // signature, so each new build asks for the login password again. A
    // code-signing certificate (a self-signed one is enough) keeps the
    // signature the same across builds: `AERI_SIGN_IDENTITY`, or else
    // "Aeri Local Signing" when the keychain has it.
    let identity = sign_identity();
    let status = std::process::Command::new("codesign")
        .args(["--force", "--deep", "--sign", &identity])
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
