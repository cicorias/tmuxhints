// Release builds on Windows are GUI apps (no console window); see `attach_console`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod cheatsheet;
mod cli;
mod hotkey;
mod hyprland;
mod packaged;
mod settings;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::args_os().len() > 1 {
        attach_console();
    }

    let cli = cli::Cli::parse();
    match cli.command {
        Some(cmd) => cli::run(cmd),
        None => {
            #[cfg(windows)]
            allow_running_instance_to_focus();
            app::run(cli.action());
            ExitCode::SUCCESS
        }
    }
}

/// A GUI-subsystem exe has no console, so CLI output would vanish. Borrow the console of
/// the shell that started us, if any.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: plain Win32 call with no pointers; failure (no parent console) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// When this launch only forwards `--show`/`--toggle` to an already running instance,
/// Windows would refuse to let that instance bring the popup to the front. We were
/// started by the user (from a shell, shortcut or launcher), so pass that right along.
#[cfg(windows)]
fn allow_running_instance_to_focus() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    // SAFETY: plain Win32 call with no pointers; failure just means no focus hand-off.
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}
