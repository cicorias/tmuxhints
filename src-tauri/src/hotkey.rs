//! Parsing, display and conflict checks for the global hotkey.

use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

/// Parses `Win+Ctrl+K`, `Super+Ctrl+K`, `Ctrl+Cmd+K`, ... (case-insensitive).
pub fn parse(input: &str) -> Result<HotKey, String> {
    let normalized = input
        .split('+')
        .map(|token| match token.trim().to_ascii_lowercase().as_str() {
            "win" | "windows" | "meta" => "Super".to_string(),
            "opt" => "Alt".to_string(),
            _ => token.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join("+");
    let hotkey: HotKey = normalized
        .parse()
        .map_err(|e| format!("invalid hotkey `{input}`: {e}"))?;
    if hotkey.mods.is_empty() {
        return Err(format!(
            "hotkey `{input}` needs at least one modifier (Ctrl, Alt, Shift, Super)"
        ));
    }
    Ok(hotkey)
}

/// Name of the Super modifier on this OS.
fn super_label() -> &'static str {
    if cfg!(windows) {
        "Win"
    } else if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Super"
    }
}

/// Human-readable form in the platform's usual modifier order, e.g. `Win+Ctrl+K`.
pub fn display(hotkey: &HotKey) -> String {
    let mods = [
        (Modifiers::SUPER, super_label()),
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
    ];
    let mut parts: Vec<&str> = mods
        .iter()
        .filter(|(m, _)| hotkey.mods.contains(*m))
        .map(|(_, label)| *label)
        .collect();
    if cfg!(target_os = "macos") {
        // macOS lists Cmd last: Ctrl+Alt+Shift+Cmd.
        parts.rotate_left(usize::from(hotkey.mods.contains(Modifiers::SUPER)));
    }
    let key = key_label(hotkey.key);
    parts.push(&key);
    parts.join("+")
}

/// `KeyK` -> `K`, `Digit1` -> `1`, `ArrowLeft` -> `Left`; other codes unchanged.
pub fn key_label(code: Code) -> String {
    let name = code.to_string();
    ["Key", "Digit", "Arrow"]
        .iter()
        .find_map(|p| name.strip_prefix(p).filter(|rest| !rest.is_empty()))
        .unwrap_or(&name)
        .to_string()
}

/// Whether this app registers the hotkey itself. Under Wayland (Hyprland on Omarchy) apps
/// can't grab global keys; the compositor binding runs `tmuxhints --toggle` instead.
pub fn app_registers_hotkey() -> bool {
    !(cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some())
}

/// Shortcuts the OS keeps for itself (not exhaustive).
fn reserved() -> &'static [(&'static str, &'static str)] {
    if cfg!(windows) {
        &[
            ("Win+Ctrl+D", "new virtual desktop"),
            ("Win+Ctrl+F4", "close virtual desktop"),
            ("Win+Ctrl+Left", "previous virtual desktop"),
            ("Win+Ctrl+Right", "next virtual desktop"),
            ("Win+Ctrl+Enter", "Narrator"),
            ("Win+Ctrl+C", "color filters"),
            ("Win+Ctrl+O", "on-screen keyboard"),
            ("Win+Ctrl+Q", "Quick Assist"),
            ("Win+Ctrl+F", "find computers"),
            ("Win+Ctrl+Shift+B", "graphics driver reset"),
            ("Win+Shift+S", "Snipping Tool"),
        ]
    } else if cfg!(target_os = "macos") {
        &[
            ("Cmd+Space", "Spotlight"),
            ("Ctrl+Space", "input source"),
            ("Ctrl+Cmd+Space", "emoji picker"),
            ("Ctrl+Cmd+Q", "lock screen"),
            ("Ctrl+Cmd+F", "full screen"),
            ("Cmd+Tab", "app switcher"),
            ("Shift+Cmd+3", "screenshot"),
            ("Shift+Cmd+4", "screenshot"),
            ("Shift+Cmd+5", "screenshot"),
        ]
    } else {
        &[]
    }
}

/// Problems with `hotkey` that are known without trying to register it.
pub fn known_conflicts(hotkey: &HotKey) -> Vec<String> {
    let mut problems: Vec<String> = reserved()
        .iter()
        .filter(|(combo, _)| parse(combo).is_ok_and(|r| r.id() == hotkey.id()))
        .map(|(combo, what)| format!("{combo} is used by the OS ({what})"))
        .collect();
    if cfg!(windows) && hotkey.mods == Modifiers::SUPER {
        problems.push("Windows reserves most Win+<key> shortcuts; add Ctrl, Alt or Shift".into());
    }
    problems
}

/// Tries to register `hotkey` system-wide and releases it again. Fails when another app
/// holds it. Apps that read keys through low-level hooks cannot be detected this way.
pub fn probe(hotkey: HotKey) -> Result<(), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| format!("hotkey check failed: {e}"))?;
    manager
        .register(hotkey)
        .map_err(|e| format!("{} is already taken by another app ({e})", display(&hotkey)))?;
    let _ = manager.unregister(hotkey);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_to_the_same_hotkey() {
        let a = parse("Win+Ctrl+K").unwrap();
        let b = parse("super+control+k").unwrap();
        let c = parse("Ctrl+Cmd+K").unwrap();
        assert_eq!(a.id(), b.id());
        assert_eq!(a.id(), c.id());
        assert_eq!(a.mods, Modifiers::SUPER | Modifiers::CONTROL);
        assert_eq!(a.key, Code::KeyK);
    }

    #[test]
    fn requires_a_modifier() {
        assert!(parse("K").is_err());
        assert!(parse("Ctrl+").is_err());
    }

    #[test]
    fn display_round_trips() {
        for input in ["Win+Ctrl+K", "Ctrl+Alt+Shift+F5", "Super+Left", "Ctrl+1"] {
            let hk = parse(input).unwrap();
            assert_eq!(parse(&display(&hk)).unwrap().id(), hk.id(), "{input}");
        }
        assert!(display(&parse("Ctrl+1").unwrap()).ends_with("+1"));
        assert!(display(&parse("Super+Left").unwrap()).ends_with("+Left"));
    }

    #[test]
    fn default_hotkey_has_no_known_conflict() {
        let hk = parse(crate::settings::DEFAULT_HOTKEY).unwrap();
        assert!(known_conflicts(&hk).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn flags_windows_reserved_shortcuts() {
        assert!(!known_conflicts(&parse("Ctrl+Win+D").unwrap()).is_empty());
        assert!(!known_conflicts(&parse("Win+E").unwrap()).is_empty());
    }
}
