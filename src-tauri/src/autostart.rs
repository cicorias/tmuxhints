//! Start at logon. Windows: per-user `Run` registry key. macOS: LaunchAgent.
//! Hyprland: an `exec-once` managed block. Other Linux desktops: XDG autostart entry.

use crate::hyprland;
use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode, WindowsEnableMode};

fn launcher() -> Result<AutoLaunch, String> {
    let exe = std::env::var("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::current_exe())
        .map_err(|e| format!("cannot find this executable: {e}"))?;
    AutoLaunchBuilder::new()
        .set_app_name("tmuxhints")
        .set_app_path(&exe.display().to_string())
        .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .build()
        .map_err(|e| e.to_string())
}

pub fn is_enabled() -> Result<bool, String> {
    if hyprland::detected() {
        let content = hyprland::read(hyprland::AUTOSTART)?;
        let body = hyprland::autostart_line();
        return Ok(
            hyprland::state(&content, hyprland::AUTOSTART.id, &body) == hyprland::State::Current
        );
    }
    launcher()?.is_enabled().map_err(|e| e.to_string())
}

pub fn set(enabled: bool) -> Result<(), String> {
    if hyprland::detected() {
        let block = hyprland::AUTOSTART;
        let content = hyprland::read(block)?;
        let updated = if enabled {
            Some(hyprland::upsert(
                &content,
                block.id,
                &hyprland::autostart_line(),
            ))
        } else {
            hyprland::remove(&content, block.id)
        };
        return match updated {
            Some(new) if new != content => hyprland::write(block, &new),
            _ => Ok(()),
        };
    }
    let launcher = launcher()?;
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| e.to_string())
}

/// Where autostart is configured, for status output.
pub fn location() -> String {
    if hyprland::detected() {
        format!(
            "exec-once in {}",
            hyprland::path(hyprland::AUTOSTART).display()
        )
    } else if cfg!(windows) {
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".to_string()
    } else if cfg!(target_os = "macos") {
        "~/Library/LaunchAgents".to_string()
    } else {
        "~/.config/autostart".to_string()
    }
}
