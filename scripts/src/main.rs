//! Project scripts for tmuxhints, run as `cargo xtask <command>`.
//! mise tasks (see ../mise.toml) are the usual entry point.

mod msix;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Package name of the Tauri app crate.
const APP: &str = "tmuxhints";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("doctor") => return doctor(),
        Some("dist") => dist(&args[1..]),
        Some("msix") => msix::build(&args[1..]),
        Some("msix-register") => msix::register(),
        Some("msix-unregister") => msix::unregister(),
        None | Some("help" | "-h" | "--help") => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command: {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "Usage: cargo xtask <command>

Commands:
  doctor                       Check the toolchain and OS prerequisites
  dist [--target <triple>]     Build release bundles into ./dist
                               (default target: this machine)
  msix [--target <triple>]...  Build the Microsoft Store package into ./dist
                               (several targets make one .msixbundle)
  msix-register                Install the unsigned package for this machine
                               (needs Windows Developer Mode)
  msix-unregister              Remove that local install"
    );
}

fn workspace_root() -> PathBuf {
    // This crate lives in <root>/scripts.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("scripts/ has a parent")
        .to_path_buf()
}

/// Runs a program from the workspace root, echoing it, and fails on a non-zero exit.
fn run(program: &str, args: &[&str]) -> Result<(), String> {
    println!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|e| format!("could not run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed ({status})"))
    }
}

fn host_triple() -> Result<String, String> {
    output("rustc", &["-vV"])
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix("host: ").map(str::to_string))
        })
        .ok_or_else(|| "could not read the host triple from `rustc -vV`".to_string())
}

fn app_version() -> Result<String, String> {
    // `cargo pkgid` prints e.g. `path+file:///.../src-tauri#tmuxhints@0.1.0`.
    let pkgid = output("cargo", &["pkgid", "--package", APP]).ok_or_else(|| {
        format!("app crate `{APP}` not found in the workspace; scaffold it first")
    })?;
    pkgid
        .rsplit(['#', '@'])
        .next()
        .map(str::to_string)
        .ok_or_else(|| format!("unexpected `cargo pkgid` output: {pkgid}"))
}

/// Builds the Tauri bundles for one target and copies the shippable files into ./dist.
fn dist(args: &[String]) -> Result<(), String> {
    let target = match args {
        [] => host_triple()?,
        [flag, triple] if flag == "--target" => triple.clone(),
        _ => return Err("usage: cargo xtask dist [--target <triple>]".to_string()),
    };
    let version = app_version()?;

    let (rust_targets, bundles, extensions): (Vec<&str>, &str, &[&str]) =
        if target == "universal-apple-darwin" {
            (
                vec!["aarch64-apple-darwin", "x86_64-apple-darwin"],
                "dmg",
                &["dmg"],
            )
        } else if target.contains("-windows-") {
            (vec![&target], "nsis", &["exe"])
        } else if target.contains("-apple-") {
            (vec![&target], "dmg", &["dmg"])
        } else if target.contains("-linux-") {
            (
                vec![&target],
                "appimage,deb,rpm",
                &["AppImage", "deb", "rpm"],
            )
        } else {
            return Err(format!("unsupported target: {target}"));
        };

    run(
        "rustup",
        &[&["target", "add"], rust_targets.as_slice()].concat(),
    )?;
    run(
        "cargo",
        &["tauri", "build", "--target", &target, "--bundles", bundles],
    )?;

    let root = workspace_root();
    let release = root.join("target").join(&target).join("release");
    let dist = root.join("dist");
    if dist.exists() {
        fs::remove_dir_all(&dist).map_err(|e| format!("clearing {}: {e}", dist.display()))?;
    }
    fs::create_dir_all(&dist).map_err(|e| format!("creating {}: {e}", dist.display()))?;

    let mut copied = Vec::new();
    for kind in bundles.split(',') {
        let dir = release.join("bundle").join(kind);
        let entries = fs::read_dir(&dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            let wanted = path.is_file()
                && path
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| extensions.contains(&x));
            if wanted {
                copied.push(copy_into(&path, &dist, None)?);
            }
        }
    }

    // Windows also ships the bare exe: WebView2 is part of the OS, so it runs as-is.
    if target.contains("-windows-") {
        let arch = if target.starts_with("aarch64") {
            "arm64"
        } else {
            "x64"
        };
        let name = format!("{APP}_{version}_{arch}-portable.exe");
        copied.push(copy_into(
            &release.join(format!("{APP}.exe")),
            &dist,
            Some(&name),
        )?);
    }

    if copied.is_empty() {
        return Err(format!("no bundles found under {}", release.display()));
    }
    println!("\ndist/");
    for name in copied {
        println!("  {name}");
    }
    Ok(())
}

/// Copies `src` into `dir` (optionally renamed) and returns the new file name.
fn copy_into(src: &Path, dir: &Path, name: Option<&str>) -> Result<String, String> {
    let name = match name {
        Some(n) => n.to_string(),
        None => src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("bad file name: {}", src.display()))?
            .to_string(),
    };
    fs::copy(src, dir.join(&name)).map_err(|e| format!("copying {}: {e}", src.display()))?;
    Ok(name)
}

/// Runs a program and returns its trimmed stdout, or `None` if it is missing or fails.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

#[derive(Default)]
struct Report {
    missing: usize,
}

impl Report {
    fn check(&mut self, what: &str, found: Option<String>, fix: &str) {
        match found {
            Some(detail) => println!("  ok       {what}: {detail}"),
            None => {
                self.missing += 1;
                println!("  MISSING  {what}\n           fix: {fix}");
            }
        }
    }

    #[cfg_attr(any(windows, target_os = "macos"), expect(dead_code))]
    fn note(&self, what: &str, detail: &str) {
        println!("  note     {what}: {detail}");
    }
}

fn doctor() -> ExitCode {
    let mut r = Report::default();
    println!("toolchain");
    r.check("rustc", output("rustc", &["--version"]), "mise install");
    r.check(
        "tauri-cli",
        output("cargo", &["tauri", "--version"]),
        "mise install",
    );

    println!("{}", std::env::consts::OS);
    os_checks(&mut r);

    if r.missing == 0 {
        println!("\nall prerequisites found");
        ExitCode::SUCCESS
    } else {
        println!("\n{} prerequisite(s) missing", r.missing);
        ExitCode::FAILURE
    }
}

#[cfg(windows)]
fn os_checks(r: &mut Report) {
    // WebView2 Evergreen runtime: machine-wide or per-user install.
    const WEBVIEW2: &str = r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let webview2 = [
        format!(r"HKLM\SOFTWARE\WOW6432Node\{WEBVIEW2}"),
        format!(r"HKCU\Software\{WEBVIEW2}"),
    ]
    .iter()
    .find_map(|key| output("reg", &["query", key, "/v", "pv"]))
    .and_then(|text| text.split_whitespace().last().map(str::to_string));
    r.check(
        "WebView2 runtime",
        webview2,
        "https://developer.microsoft.com/microsoft-edge/webview2/",
    );

    let vswhere = std::env::var("ProgramFiles(x86)")
        .map(|p| format!(r"{p}\Microsoft Visual Studio\Installer\vswhere.exe"))
        .unwrap_or_default();
    r.check(
        "MSVC C++ build tools",
        output(
            &vswhere,
            &[
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "displayName",
            ],
        ),
        "add the \"Desktop development with C++\" workload in Visual Studio Installer",
    );
}

#[cfg(target_os = "macos")]
fn os_checks(r: &mut Report) {
    r.check(
        "Xcode command line tools",
        output("xcode-select", &["-p"]),
        "xcode-select --install",
    );
}

#[cfg(target_os = "linux")]
fn os_checks(r: &mut Report) {
    const ARCH_DEPS: &str = "sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file \
                             openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool";
    r.check(
        "webkit2gtk-4.1",
        output("pkg-config", &["--modversion", "webkit2gtk-4.1"]),
        ARCH_DEPS,
    );
    let appindicator = ["ayatana-appindicator3-0.1", "appindicator3-0.1"]
        .iter()
        .find_map(|lib| output("pkg-config", &["--modversion", lib]));
    r.check("appindicator (tray)", appindicator, ARCH_DEPS);

    match output("hyprctl", &["version", "-j"]) {
        Some(_) => r.note(
            "Hyprland",
            "detected; the hotkey is bound in Hyprland config, not by the app",
        ),
        None => r.note("Hyprland", "not detected"),
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn os_checks(r: &mut Report) {
    r.note(std::env::consts::OS, "unsupported OS");
}
