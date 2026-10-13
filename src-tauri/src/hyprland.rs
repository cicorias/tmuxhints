//! Hyprland (Omarchy) integration: managed blocks in `~/.config/hypr/*.conf`.
//!
//! Under Wayland the app can't grab global keys, so the hotkey is a Hyprland `bind` that
//! runs `tmuxhints --toggle`, and autostart is an `exec-once`. Each lives in a marked block
//! that tmuxhints adds, repairs and removes; nothing outside the markers is touched.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;

/// One managed block: which file it lives in and its marker id.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub file: &'static str,
    pub id: &'static str,
}

pub const HOTKEY: Block = Block {
    file: "bindings.conf",
    id: "hotkey",
};
pub const AUTOSTART: Block = Block {
    file: "autostart.conf",
    id: "autostart",
};

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    Current,
    Outdated,
}

/// Running inside a Hyprland session.
pub fn detected() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

pub fn require() -> Result<(), String> {
    if detected() {
        Ok(())
    } else {
        Err("Hyprland is not running in this session".to_string())
    }
}

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("hypr")
}

pub fn path(block: Block) -> PathBuf {
    config_dir().join(block.file)
}

fn begin_marker(id: &str) -> String {
    format!("# >>> tmuxhints:{id} (managed by `tmuxhints`; do not edit) >>>")
}

fn end_marker(id: &str) -> String {
    format!("# <<< tmuxhints:{id} <<<")
}

/// Line range of the block (begin and end marker, inclusive), if present.
fn find(lines: &[&str], id: &str) -> Option<(usize, usize)> {
    let begin = format!("# >>> tmuxhints:{id} ");
    let end = end_marker(id);
    let start = lines
        .iter()
        .position(|l| l.trim_end().starts_with(&begin))?;
    let len = lines[start..].iter().position(|l| l.trim_end() == end)?;
    Some((start, start + len))
}

pub fn state(content: &str, id: &str, body: &str) -> State {
    let lines: Vec<&str> = content.lines().collect();
    match find(&lines, id) {
        None => State::Missing,
        Some((start, end)) if lines[start + 1..end].join("\n") == body => State::Current,
        Some(_) => State::Outdated,
    }
}

/// Adds the block, or replaces its contents if it already exists.
pub fn upsert(content: &str, id: &str, body: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let begin = begin_marker(id);
    let end = end_marker(id);
    let block = [begin.as_str(), body, end.as_str()].join("\n");
    let mut out = match find(&lines, id) {
        Some((start, stop)) => [
            lines[..start].join("\n"),
            block,
            lines[stop + 1..].join("\n"),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n"),
        None if content.trim().is_empty() => block,
        None => format!("{}\n\n{block}", content.trim_end()),
    };
    out.push('\n');
    out
}

/// Removes the block; `None` if there was nothing to remove.
pub fn remove(content: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, stop) = find(&lines, id)?;
    let mut kept: Vec<&str> = lines[..start].to_vec();
    // Drop the blank line that `upsert` put in front of the block.
    if kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    kept.extend(&lines[stop + 1..]);
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    Some(out)
}

/// Lines outside our blocks that mention tmuxhints, e.g. a hand-written bind.
pub fn stray_lines(content: &str) -> Vec<String> {
    let mut inside = false;
    let mut found = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("# >>> tmuxhints:") {
            inside = true;
        } else if t.starts_with("# <<< tmuxhints:") {
            inside = false;
        } else if !inside && !t.starts_with('#') && t.contains("tmuxhints") {
            found.push(t.to_string());
        }
    }
    found
}

/// Command line that starts this executable (the AppImage itself when run from one).
pub fn launch_command() -> String {
    let exe = std::env::var("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_exe())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "tmuxhints".to_string());
    if exe.contains(' ') {
        format!("'{}'", exe.replace('\'', r"'\''"))
    } else {
        exe
    }
}

const SHIFT: u32 = 1;
const CTRL: u32 = 4;
const ALT: u32 = 8;
const SUPER: u32 = 64;

/// Hyprland's modifier bitmask for `hotkey`, as reported by `hyprctl binds -j`.
pub fn modmask(hotkey: &HotKey) -> u32 {
    [
        (Modifiers::SHIFT, SHIFT),
        (Modifiers::CONTROL, CTRL),
        (Modifiers::ALT, ALT),
        (Modifiers::SUPER, SUPER),
    ]
    .iter()
    .filter(|(m, _)| hotkey.mods.contains(*m))
    .map(|(_, bit)| bit)
    .sum()
}

/// Key name in Hyprland's (xkb) spelling.
pub fn key_name(code: Code) -> String {
    match code {
        Code::Enter => "Return".into(),
        Code::Space => "space".into(),
        Code::Escape => "Escape".into(),
        Code::ArrowLeft => "left".into(),
        Code::ArrowRight => "right".into(),
        Code::ArrowUp => "up".into(),
        Code::ArrowDown => "down".into(),
        other => crate::hotkey::key_label(other),
    }
}

/// `bind = SUPER CTRL, K, exec, <tmuxhints> --toggle`
pub fn bind_line(hotkey: &HotKey) -> String {
    let mods: Vec<&str> = [
        (Modifiers::SUPER, "SUPER"),
        (Modifiers::CONTROL, "CTRL"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::SHIFT, "SHIFT"),
    ]
    .iter()
    .filter(|(m, _)| hotkey.mods.contains(*m))
    .map(|(_, name)| *name)
    .collect();
    format!(
        "bind = {}, {}, exec, {} --toggle",
        mods.join(" "),
        key_name(hotkey.key),
        launch_command()
    )
}

pub fn autostart_line() -> String {
    format!("exec-once = {}", launch_command())
}

#[derive(Debug, Deserialize)]
struct Bind {
    modmask: u32,
    key: String,
    #[serde(default)]
    dispatcher: String,
    #[serde(default)]
    arg: String,
}

/// Existing Hyprland binds (other than ours) on the same keys, from `hyprctl binds -j`.
pub fn conflicts_in(binds_json: &str, hotkey: &HotKey) -> Result<Vec<String>, String> {
    let binds: Vec<Bind> =
        serde_json::from_str(binds_json).map_err(|e| format!("unexpected hyprctl output: {e}"))?;
    let (mask, key) = (modmask(hotkey), key_name(hotkey.key));
    Ok(binds
        .into_iter()
        .filter(|b| b.modmask == mask && b.key.eq_ignore_ascii_case(&key))
        .filter(|b| !b.arg.contains("tmuxhints"))
        .map(|b| {
            format!("already bound in Hyprland: {} {}", b.dispatcher, b.arg)
                .trim_end()
                .to_string()
        })
        .collect())
}

pub fn conflicts(hotkey: &HotKey) -> Result<Vec<String>, String> {
    let out = Command::new("hyprctl")
        .args(["binds", "-j"])
        .output()
        .map_err(|e| format!("could not run hyprctl: {e}"))?;
    if !out.status.success() {
        return Err("`hyprctl binds -j` failed".to_string());
    }
    conflicts_in(&String::from_utf8_lossy(&out.stdout), hotkey)
}

pub fn read(block: Block) -> Result<String, String> {
    let path = path(block);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Whether `hyprland.conf` sources `block.file` (Omarchy's default config does).
pub fn is_sourced(block: Block) -> bool {
    fs::read_to_string(config_dir().join("hyprland.conf")).is_ok_and(|conf| {
        conf.lines().any(|l| {
            let l = l.trim();
            l.starts_with("source") && l.contains(block.file)
        })
    })
}

/// Writes `content` to the block's file, keeping a `.bak` of the previous version,
/// then asks Hyprland to reload.
pub fn write(block: Block, content: &str) -> Result<(), String> {
    let path = path(block);
    if path.exists() {
        let backup = path.with_extension("conf.bak");
        fs::copy(&path, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
    }
    crate::settings::write_atomic(&path, content)?;
    let _ = Command::new("hyprctl").arg("reload").output();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "hotkey";

    #[test]
    fn upsert_adds_then_is_idempotent() {
        let original = "bind = SUPER, Q, killactive\n";
        let once = upsert(original, ID, "bind = A");
        assert!(once.starts_with(original));
        assert_eq!(state(&once, ID, "bind = A"), State::Current);
        assert_eq!(upsert(&once, ID, "bind = A"), once);
    }

    #[test]
    fn upsert_repairs_an_outdated_block_in_place() {
        let content = format!(
            "a\n{}\nbind = OLD\n{}\nb\n",
            begin_marker(ID),
            end_marker(ID)
        );
        assert_eq!(state(&content, ID, "bind = NEW"), State::Outdated);
        let fixed = upsert(&content, ID, "bind = NEW");
        assert_eq!(
            fixed,
            format!(
                "a\n{}\nbind = NEW\n{}\nb\n",
                begin_marker(ID),
                end_marker(ID)
            )
        );
    }

    #[test]
    fn remove_restores_the_original_file() {
        let original = "bind = SUPER, Q, killactive\n";
        let added = upsert(original, ID, "bind = A");
        assert_eq!(remove(&added, ID).unwrap(), original);
        assert_eq!(remove(original, ID), None);
        assert_eq!(remove(&upsert("", ID, "x"), ID).unwrap(), "");
    }

    #[test]
    fn blocks_with_different_ids_are_independent() {
        let both = upsert(&upsert("", "hotkey", "x"), "autostart", "y");
        assert_eq!(state(&both, "hotkey", "x"), State::Current);
        assert_eq!(state(&both, "autostart", "y"), State::Current);
        let one = remove(&both, "hotkey").unwrap();
        assert_eq!(state(&one, "hotkey", "x"), State::Missing);
        assert_eq!(state(&one, "autostart", "y"), State::Current);
    }

    #[test]
    fn finds_stray_lines_outside_blocks() {
        let content = upsert(
            "bind = SUPER, T, exec, tmuxhints --toggle\n",
            ID,
            "bind = x tmuxhints",
        );
        assert_eq!(
            stray_lines(&content),
            vec!["bind = SUPER, T, exec, tmuxhints --toggle"]
        );
    }

    #[test]
    fn bind_line_uses_hyprland_names() {
        let hk = crate::hotkey::parse("Super+Ctrl+K").unwrap();
        assert!(bind_line(&hk).starts_with("bind = SUPER CTRL, K, exec, "));
        assert!(bind_line(&hk).ends_with(" --toggle"));
        assert_eq!(modmask(&hk), SUPER | CTRL);
    }

    #[test]
    fn conflicts_ignore_our_own_bind() {
        let hk = crate::hotkey::parse("Super+Ctrl+K").unwrap();
        let json = r#"[
            {"modmask":68,"key":"K","dispatcher":"exec","arg":"/usr/bin/tmuxhints --toggle"},
            {"modmask":68,"key":"k","dispatcher":"exec","arg":"kitty"},
            {"modmask":64,"key":"K","dispatcher":"exec","arg":"other"}
        ]"#;
        assert_eq!(
            conflicts_in(json, &hk).unwrap(),
            vec!["already bound in Hyprland: exec kitty"]
        );
    }
}
