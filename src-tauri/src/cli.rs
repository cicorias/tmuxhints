//! Command-line interface. With no subcommand the tray app runs; subcommands do their
//! work and exit without starting the GUI.

use crate::settings::{self, Settings};
use crate::{autostart, hotkey, hyprland};
use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "tmuxhints",
    version,
    about = "Tray app that pops up a tmux cheat sheet on a global hotkey",
    after_help = "Without a subcommand, starts the tray app (or signals the running one)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,

    /// Toggle the cheat sheet in the running app
    #[arg(long, conflicts_with_all = ["show", "hide"])]
    pub toggle: bool,

    /// Show the cheat sheet
    #[arg(long, conflicts_with = "hide")]
    pub show: bool,

    /// Hide the cheat sheet
    #[arg(long)]
    pub hide: bool,

    /// Quit the running app
    #[arg(long, conflicts_with_all = ["toggle", "show", "hide"])]
    pub quit: bool,

    /// Make the running app re-read its settings (used by `hotkey set` / `prefix set`)
    #[arg(long, hide = true)]
    pub reload: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Plain start: just sit in the tray.
    Start,
    Toggle,
    Show,
    Hide,
    Quit,
    Reload,
}

impl Cli {
    pub fn action(&self) -> Action {
        if self.toggle {
            Action::Toggle
        } else if self.show {
            Action::Show
        } else if self.hide {
            Action::Hide
        } else if self.quit {
            Action::Quit
        } else if self.reload {
            Action::Reload
        } else {
            Action::Start
        }
    }
}

/// What a second launch asks the running instance to do. Starting the app again without
/// flags shows the cheat sheet.
pub fn forwarded_action(argv: &[String]) -> Action {
    match Cli::try_parse_from(argv).map(|cli| cli.action()) {
        Ok(Action::Start) | Err(_) => Action::Show,
        Ok(action) => action,
    }
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Show, change or check the global hotkey
    Hotkey {
        #[command(subcommand)]
        cmd: HotkeyCmd,
    },
    /// Show or change the tmux prefix shown in the cheat sheet
    Prefix {
        #[command(subcommand)]
        cmd: PrefixCmd,
    },
    /// Start tmuxhints at logon
    Autostart {
        #[command(subcommand)]
        cmd: AutostartCmd,
    },
    /// Manage the Hyprland key binding (Omarchy)
    Hyprland {
        #[command(subcommand)]
        cmd: HyprlandCmd,
    },
    /// Settings and cheat-sheet files
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
}

#[derive(Debug, Subcommand)]
pub enum HotkeyCmd {
    /// Print the current hotkey
    Get,
    /// Change the hotkey, e.g. `Win+Ctrl+K` (checks for conflicts first)
    Set {
        combo: String,
        /// Save it even if a conflict is found
        #[arg(long)]
        force: bool,
    },
    /// Report conflicts for a hotkey without changing anything (exit 1 if any)
    Check { combo: String },
}

#[derive(Debug, Subcommand)]
pub enum PrefixCmd {
    /// Print the current prefix
    Get,
    /// Change the prefix, e.g. `C-a` or `Ctrl+a`
    Set { prefix: String },
}

#[derive(Debug, Subcommand)]
pub enum AutostartCmd {
    /// Print whether autostart is on (exit 1 if off)
    Status,
    Enable,
    Disable,
}

#[derive(Debug, Subcommand)]
pub enum HyprlandCmd {
    /// Show the state of the managed bind (exit 1 unless installed and current)
    Status,
    /// Add or repair the bind for the configured hotkey
    Install {
        /// Install even if the keys are already bound in Hyprland
        #[arg(long)]
        force: bool,
        /// Print what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove the managed bind
    Remove {
        /// Print what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the config folder and the files in it
    Path,
}

pub fn run(cmd: Cmd) -> ExitCode {
    let result = match cmd {
        Cmd::Hotkey { cmd } => hotkey_cmd(cmd),
        Cmd::Prefix { cmd } => prefix_cmd(cmd),
        Cmd::Autostart { cmd } => autostart_cmd(cmd),
        Cmd::Hyprland { cmd } => hyprland_cmd(cmd),
        Cmd::Config {
            cmd: ConfigCmd::Path,
        } => {
            let dir = settings::visible_config_dir();
            println!("{}", dir.display());
            println!("  settings:   {}", dir.join("settings.json").display());
            println!("  cheatsheet: {}", dir.join("cheatsheet.json").display());
            Ok(true)
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Asks a running instance to re-read settings. If none is running this starts one,
/// which notices the `--reload` flag and exits straight away.
fn notify_running_app() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--reload").spawn();
    }
}

/// All conflicts for `hk`: OS-reserved shortcuts, then Hyprland binds or a live
/// registration probe. `current` is skipped for the probe since we hold it ourselves.
fn conflicts(
    hk: &global_hotkey::hotkey::HotKey,
    current: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut found = hotkey::known_conflicts(hk);
    if hyprland::detected() {
        found.extend(hyprland::conflicts(hk)?);
    } else if hotkey::app_registers_hotkey() {
        let is_current = current
            .and_then(|c| hotkey::parse(c).ok())
            .is_some_and(|c| c.id() == hk.id());
        if !is_current && let Err(e) = hotkey::probe(*hk) {
            found.push(e);
        }
    }
    Ok(found)
}

// Each command returns Ok(true) for success, Ok(false) for "ran fine, but the answer is no".
fn hotkey_cmd(cmd: HotkeyCmd) -> Result<bool, String> {
    let mut s = Settings::load()?;
    match cmd {
        HotkeyCmd::Get => {
            println!("{}", hotkey::display(&hotkey::parse(&s.hotkey)?));
            Ok(true)
        }
        HotkeyCmd::Check { combo } => {
            let hk = hotkey::parse(&combo)?;
            let found = conflicts(&hk, Some(&s.hotkey))?;
            if found.is_empty() {
                println!("{}: no conflicts found", hotkey::display(&hk));
            }
            for c in &found {
                println!("conflict: {c}");
            }
            Ok(found.is_empty())
        }
        HotkeyCmd::Set { combo, force } => {
            let hk = hotkey::parse(&combo)?;
            let found = conflicts(&hk, Some(&s.hotkey))?;
            for c in &found {
                eprintln!("conflict: {c}");
            }
            if !found.is_empty() && !force {
                return Err("not changed; pass --force to use it anyway".to_string());
            }
            s.hotkey = hotkey::display(&hk);
            s.save()?;
            println!("hotkey set to {}", s.hotkey);
            if hyprland::detected() {
                let content = hyprland::read(hyprland::HOTKEY)?;
                if hyprland::state(&content, hyprland::HOTKEY.id, "") != hyprland::State::Missing {
                    let updated =
                        hyprland::upsert(&content, hyprland::HOTKEY.id, &hyprland::bind_line(&hk));
                    hyprland::write(hyprland::HOTKEY, &updated)?;
                    println!("updated {}", hyprland::path(hyprland::HOTKEY).display());
                } else {
                    println!("run `tmuxhints hyprland install` to bind it in Hyprland");
                }
            }
            notify_running_app();
            Ok(true)
        }
    }
}

fn prefix_cmd(cmd: PrefixCmd) -> Result<bool, String> {
    let mut s = Settings::load()?;
    match cmd {
        PrefixCmd::Get => println!("{}", s.prefix),
        PrefixCmd::Set { prefix } => {
            s.prefix = settings::normalize_prefix(&prefix)?;
            s.save()?;
            println!("prefix set to {}", s.prefix);
            notify_running_app();
        }
    }
    Ok(true)
}

fn autostart_cmd(cmd: AutostartCmd) -> Result<bool, String> {
    match cmd {
        AutostartCmd::Status => {
            let on = autostart::is_enabled()?;
            println!(
                "{} ({})",
                if on { "enabled" } else { "disabled" },
                autostart::location()
            );
            Ok(on)
        }
        AutostartCmd::Enable | AutostartCmd::Disable => {
            let on = matches!(cmd, AutostartCmd::Enable);
            autostart::set(on)?;
            println!(
                "autostart {} ({})",
                if on { "enabled" } else { "disabled" },
                autostart::location()
            );
            Ok(true)
        }
    }
}

fn hyprland_cmd(cmd: HyprlandCmd) -> Result<bool, String> {
    hyprland::require()?;
    let block = hyprland::HOTKEY;
    let path = hyprland::path(block);
    let content = hyprland::read(block)?;
    let hk = hotkey::parse(&Settings::load()?.hotkey)?;
    let body = hyprland::bind_line(&hk);

    if !hyprland::is_sourced(block) {
        eprintln!(
            "warning: {} is not sourced from hyprland.conf; add `source = {}`",
            block.file,
            path.display()
        );
    }

    match cmd {
        HyprlandCmd::Status => {
            let state = hyprland::state(&content, block.id, &body);
            let label = match state {
                hyprland::State::Missing => "not installed",
                hyprland::State::Current => "installed",
                hyprland::State::Outdated => "out of date (run `tmuxhints hyprland install`)",
            };
            println!("{label}: {}", path.display());
            println!("  {body}");
            for line in hyprland::stray_lines(&content) {
                println!("  note: unmanaged line mentions tmuxhints: {line}");
            }
            for c in hyprland::conflicts(&hk)? {
                println!("  conflict: {c}");
            }
            Ok(state == hyprland::State::Current)
        }
        HyprlandCmd::Install { force, dry_run } => {
            if hyprland::state(&content, block.id, &body) == hyprland::State::Current {
                println!("already installed: {}", path.display());
                return Ok(true);
            }
            let found = hyprland::conflicts(&hk)?;
            for c in &found {
                eprintln!("conflict: {c}");
            }
            if !found.is_empty() && !force {
                return Err("not installed; pass --force to bind it anyway".to_string());
            }
            if dry_run {
                println!("would write to {}:\n  {body}", path.display());
                return Ok(true);
            }
            hyprland::write(block, &hyprland::upsert(&content, block.id, &body))?;
            println!("installed: {}\n  {body}", path.display());
            Ok(true)
        }
        HyprlandCmd::Remove { dry_run } => match hyprland::remove(&content, block.id) {
            None => {
                println!("nothing to remove in {}", path.display());
                Ok(true)
            }
            Some(_) if dry_run => {
                println!("would remove the tmuxhints block from {}", path.display());
                Ok(true)
            }
            Some(updated) => {
                hyprland::write(block, &updated)?;
                println!("removed from {}", path.display());
                Ok(true)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn second_launch_without_flags_shows_the_sheet() {
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(forwarded_action(&argv(&["tmuxhints"])), Action::Show);
        assert_eq!(
            forwarded_action(&argv(&["tmuxhints", "--toggle"])),
            Action::Toggle
        );
        assert_eq!(
            forwarded_action(&argv(&["tmuxhints", "--reload"])),
            Action::Reload
        );
        assert_eq!(
            forwarded_action(&argv(&["tmuxhints", "--quit"])),
            Action::Quit
        );
    }
}
