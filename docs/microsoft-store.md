# Publishing to the Microsoft Store

tmuxhints ships to the Store as an **MSIX bundle** (x64 + ARM64). The package is built
unsigned; the Store signs it during certification, so no code-signing certificate is
needed.

## How the Store build differs

The same exe runs packaged and unpackaged; it detects the package at runtime.

| | Installer / portable build | Store build |
|---|---|---|
| Start at logon | `HKCU\…\Run` registry value | Manifest `StartupTask`, listed in **Settings › Apps › Startup** |
| `tmuxhints` on the command line | the exe on `PATH` | App execution alias (`tmuxhints.exe`) |
| Config folder | `%APPDATA%\tmuxhints` | `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming\tmuxhints` for a fresh install; an existing `%APPDATA%\tmuxhints` is used in place |

`tmuxhints config path` and the tray's **Open config folder** always show the real
location.

## One-time setup in Partner Center

1. **Reserve the app name** (Apps and games › New product › MSIX or PWA app).
2. Open **Product management › Product identity** and copy the three values into
   [`packaging/msix/identity.env`](../packaging/msix/identity.env):

   ```ini
   IDENTITY_NAME=12345Publisher.tmuxhints        # Package/Identity/Name
   PUBLISHER=CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX   # Package/Identity/Publisher
   PUBLISHER_DISPLAY_NAME=Your Publisher Name    # Package/Properties/PublisherDisplayName
   DISPLAY_NAME=tmuxhints                        # must match the reserved name
   ```

   These values are public (every installed package shows them), so commit them.

## Build the package

```sh
mise run msix    # -> dist/tmuxhints_<version>.0.msixbundle
```

CI also builds it on every push: download the **store-msixbundle** artifact from the
workflow run. Every submission needs a higher version than the last one, so bump
`version` in `src-tauri/Cargo.toml` first (the package version is that plus `.0`).

### Test it locally first

With Windows **Developer Mode** on (Settings › System › For developers):

```sh
mise run msix:register     # install the unsigned x64 package from its folder
tmuxhints --show           # in a new terminal: the alias should resolve
mise run msix:unregister   # remove it (also deletes its private data)
```

Quit any other running tmuxhints first; a running copy receives the launch instead.

## Submission

| Section | What to enter |
|---|---|
| **Packages** | Upload `tmuxhints_<version>.0.msixbundle`. |
| **Pricing and availability** | Free (or your choice). |
| **Properties** | Category: *Developer tools*. Privacy policy URL: link to [`PRIVACY.md`](../PRIVACY.md) on GitHub. |
| **Age ratings** | Complete the questionnaire (no user content, no data collection). |
| **Store listing** | See below. |
| **Submission options › restricted capabilities** | `runFullTrust` needs a justification, e.g. *"tmuxhints is a desktop tray app. It registers a global hotkey (RegisterHotKey) to show a tmux cheat-sheet window, and provides a command-line interface through an app execution alias."* |

### Store listing

**Description**

> tmuxhints puts a tmux cheat sheet one keystroke away. It sits in your system tray, and
> pressing Win+Ctrl+K pops up every tmux binding (sessions, windows, panes, copy mode,
> commands), showing *your* prefix key instead of "Prefix". Press the hotkey again, or
> Esc, and it's gone.
>
> Scroll with the arrow keys or vim keys, change the hotkey with built-in conflict
> checks, add your own bindings in a small JSON file, and start it at sign-in. Includes a
> `tmuxhints` command for scripting.

**Features** (one per line)

- Global hotkey (default Win+Ctrl+K) shows the cheat sheet over any app
- Shows your own tmux prefix, e.g. Ctrl+a
- Keyboard scrolling with arrows or vim keys
- Remappable hotkey with conflict checks
- Add your own bindings with a JSON file
- Starts at sign-in (optional)
- Command-line interface: `tmuxhints prefix set C-a`

**Search terms**: tmux, terminal, cheat sheet, keyboard shortcuts, hotkey, developer tools, vim

**Screenshots**: desktop screenshots must be at least **1366 × 768**. The README
screenshot (1198 × 783) is too small, so capture the popup on a full-screen desktop
background instead.
