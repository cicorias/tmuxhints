# tmuxhints

A small tray app that pops up a tmux cheat sheet on a global hotkey
(default **Win+Ctrl+K** / **Ctrl+Cmd+K** / **Super+Ctrl+K**).

- Runs at logon, no admin rights needed
- Remappable hotkey with key capture and conflict checks
- Your tmux prefix (default `Ctrl+b`) is shown in place of "Prefix"
- Windows, macOS, and Linux (tested on Omarchy / Hyprland)

> Work in progress.

## Development

Tools and tasks are managed with [mise](https://mise.jdx.dev):

```sh
mise install      # Rust, tauri-cli, actionlint
mise run doctor   # check OS prerequisites
mise tasks        # list everything
mise run check    # format check, lint, tests (what CI runs)
mise run dev      # run the app
mise run dist     # build installers into ./dist
```

Project scripts are Rust, in [`scripts/`](scripts/) (`cargo xtask <cmd>`).

## License

MIT
