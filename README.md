# tmuxhints

A small tray app that pops up a tmux cheat sheet on a global hotkey
(default **Win+Ctrl+K** / **Ctrl+Cmd+K** / **Super+Ctrl+K**).

- Runs at logon, no admin rights needed
- Remappable hotkey with key capture and conflict checks
- Your tmux prefix (default `Ctrl+b`) is shown in place of "Prefix"
- Windows, macOS, and Linux (tested on Omarchy / Hyprland)

> Work in progress.

## Usage

```sh
tmuxhints                          # start in the tray (a second launch shows the sheet)
tmuxhints --toggle | --show | --hide

tmuxhints hotkey get
tmuxhints hotkey set "Win+Ctrl+K" [--force]   # checks for conflicts first
tmuxhints hotkey check "Win+Ctrl+K"           # exit 1 if it conflicts

tmuxhints prefix get
tmuxhints prefix set C-a                      # tmux or Ctrl+a notation

tmuxhints autostart status | enable | disable
tmuxhints hyprland status | install | remove [--force] [--dry-run]
tmuxhints config path
```

Add your own bindings in `cheatsheet.json` in the config folder (`tmuxhints config path`).
Sections with an existing name are extended, new ones are added; `"replace": true`
replaces the built-in sheet:

```json
{ "sections": [ { "name": "Mine", "items": [ { "keys": "Prefix r", "desc": "Reload config" } ] } ] }
```

### Omarchy / Hyprland

Wayland doesn't let apps grab global keys, so the hotkey is a Hyprland `bind`.
`tmuxhints hyprland install` writes it to `~/.config/hypr/bindings.conf`, and
`tmuxhints autostart enable` adds an `exec-once` to `~/.config/hypr/autostart.conf`.
Both live in marked blocks that tmuxhints repairs or removes without touching
anything else (the previous file is kept as `*.conf.bak`).

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
