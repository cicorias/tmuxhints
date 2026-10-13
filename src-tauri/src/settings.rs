//! User settings (`settings.json`) and the config directory they live in.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const DEFAULT_HOTKEY: &str = "Super+Ctrl+K";
pub const DEFAULT_PREFIX: &str = "Ctrl+b";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Global hotkey, e.g. `Super+Ctrl+K` (Super = Win on Windows, Cmd on macOS).
    pub hotkey: String,
    /// tmux prefix in display form, e.g. `Ctrl+b`.
    pub prefix: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            prefix: DEFAULT_PREFIX.to_string(),
        }
    }
}

/// `%APPDATA%\tmuxhints`, `~/Library/Application Support/tmuxhints` or `~/.config/tmuxhints`.
/// `TMUXHINTS_CONFIG_DIR` overrides it (tests, screenshots, portable setups).
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TMUXHINTS_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tmuxhints")
}

/// `config_dir()` as seen from outside the app: the same path, except for the Microsoft
/// Store build, whose %APPDATA% writes Windows may redirect into the package's folder.
/// Use this when showing the folder to the user or opening it in a file manager.
pub fn visible_config_dir() -> PathBuf {
    // Creating it first settles where Windows puts it for a packaged app.
    let _ = fs::create_dir_all(config_dir());
    crate::packaged::real_roaming_path(config_dir())
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// Optional user cheat sheet, merged over the built-in one.
pub fn cheatsheet_path() -> PathBuf {
    config_dir().join("cheatsheet.json")
}

impl Settings {
    /// Loads settings, falling back to defaults when the file does not exist yet.
    pub fn load() -> Result<Self, String> {
        let path = settings_path();
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&settings_path(), &(text + "\n"))
    }
}

/// Writes via a temp file and rename, so a crash never leaves a half-written file.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Accepts tmux notation (`C-a`, `M-x`, `C-M-Space`) or `Ctrl+a` style, and returns
/// the display form used in the cheat sheet (`Ctrl+a`).
pub fn normalize_prefix(input: &str) -> Result<String, String> {
    const MODIFIERS: &[(&str, usize)] = &[
        ("c-", 0),
        ("ctrl+", 0),
        ("ctrl-", 0),
        ("control+", 0),
        ("control-", 0),
        ("m-", 1),
        ("alt+", 1),
        ("alt-", 1),
        ("meta+", 1),
        ("meta-", 1),
        ("option+", 1),
        ("s-", 2),
        ("shift+", 2),
        ("shift-", 2),
    ];
    const LABELS: [&str; 3] = ["Ctrl", "Alt", "Shift"];

    let mut rest = input.trim();
    let mut held = [false; 3];
    while let Some((prefix, index)) = MODIFIERS.iter().find(|(p, _)| {
        rest.len() > p.len() && rest.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
    }) {
        held[*index] = true;
        rest = &rest[prefix.len()..];
    }

    let key = rest;
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err(format!(
            "invalid tmux prefix `{input}`; try e.g. `C-b` or `Ctrl+a`"
        ));
    }
    if key.len() > 1 && (key.ends_with('-') || key.ends_with('+')) {
        return Err(format!("invalid tmux prefix `{input}`: missing key"));
    }

    let mut parts: Vec<&str> = LABELS
        .iter()
        .zip(held)
        .filter_map(|(label, on)| on.then_some(*label))
        .collect();
    parts.push(key);
    Ok(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_accepts_tmux_and_plus_notation() {
        assert_eq!(normalize_prefix("C-b").unwrap(), "Ctrl+b");
        assert_eq!(normalize_prefix("c-a").unwrap(), "Ctrl+a");
        assert_eq!(normalize_prefix("Ctrl+a").unwrap(), "Ctrl+a");
        assert_eq!(normalize_prefix("ctrl-Space").unwrap(), "Ctrl+Space");
        assert_eq!(normalize_prefix("M-x").unwrap(), "Alt+x");
        assert_eq!(normalize_prefix("C-M-x").unwrap(), "Ctrl+Alt+x");
        assert_eq!(normalize_prefix(" ` ").unwrap(), "`");
        assert_eq!(normalize_prefix("C--").unwrap(), "Ctrl+-");
    }

    #[test]
    fn prefix_rejects_garbage() {
        assert!(normalize_prefix("").is_err());
        assert!(normalize_prefix("C-").is_err());
        assert!(normalize_prefix("Ctrl+").is_err());
        assert!(normalize_prefix("C-b x").is_err());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{ "prefix": "Ctrl+a" }"#).unwrap();
        assert_eq!(s.prefix, "Ctrl+a");
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
    }
}
