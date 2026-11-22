# toolbelt

A small, fast snippet picker. Press **Ctrl+S**, pick a saved command from a list, and it's
inserted at your cursor, ready to edit or run. Works in PowerShell (Windows) and bash/zsh (Linux).

```
 Pick a tool  type to filter
 ❯ Show folder sizes in GB
      powershell -NoProfile -File C:\Tools\show-folder-sizes.ps1
   SSH ubuntu vm
   Show folder tree
  ↑↓ navigate · enter select · esc cancel
```

## Install

**Windows** (PowerShell):
```powershell
irm https://raw.githubusercontent.com/asger118/toolbelt/main/install.ps1 | iex
```

**Linux** (x64 and arm64):
```sh
curl -fsSL https://raw.githubusercontent.com/asger118/toolbelt/main/install.sh | sh
```

Run the same command again to update. Or build from source: `cargo install --git https://github.com/asger118/toolbelt`.

## Enable Ctrl+S

| Shell | Add to | Line |
|---|---|---|
| PowerShell 7 and 5.1 | `$PROFILE` (both, if you use both) | `tb init powershell \| Out-String \| Invoke-Expression` |
| bash | `~/.bashrc` | `eval "$(tb init bash)"` |
| zsh | `~/.zshrc` | `eval "$(tb init zsh)"` |

On Linux this also turns off terminal flow control (`stty -ixon`), which normally makes Ctrl+S freeze the terminal.

Without the binding, `tb` prints the picked command instead.

## Usage

| Command | |
|---|---|
| `tb` | Open the picker. Type to filter, arrows to move, Enter to pick, Esc to cancel. |
| `tb new` | Add a tool: asks for description, command and optional help. |
| `tb edit` | Open the snippet file in `$VISUAL` / `$EDITOR` / VS Code / notepad / nano, then check it for errors. |
| `tb init <shell>` | Print the Ctrl+S binding for `powershell`, `bash` or `zsh`. |
| `tb path` | Print where the snippet file is. |

The list shows **recently used tools first**, then the rest in file order. Filtering matches every
typed word against the description and the command, ignoring case.

## Snippet file

`~/.config/toolbelt/toolbelt.json` on every OS (override with `TOOLBELT_FILE`):

```json
{
  "$schema": "https://raw.githubusercontent.com/asger118/toolbelt/main/schema.json",
  "snippets": [
    {
      "description": "Show folder tree",
      "command": "tree -L <level=1> -I \"bin|obj|node_modules\"",
      "help": "level: how many folders deep to show",
      "os": "windows"
    },
    { "description": "SSH ubuntu vm", "command": "ssh admin@192.168.1.10" }
  ]
}
```

| Field | |
|---|---|
| `description` | Shown in the list. Must be unique (recent-use tracking is keyed on it). |
| `command` | The command to insert. |
| `help` | Optional. Shown under the highlighted tool and while filling in placeholders. |
| `os` | Optional, `"windows"` or `"linux"`. Without it, the tool shows everywhere. |

### Placeholders

`<name>` or `<name=default>` in a command is asked for after you pick it, with the default
pre-filled. A name is letters, digits, `_` and `-`, so `cmd > out.txt` is left alone. A name used
twice is only asked once.

Recent-use data lives outside the config folder, so syncing the snippet file (e.g. with chezmoi)
stays clean: `%LOCALAPPDATA%\toolbelt\usage.json` on Windows, `~/.local/state/toolbelt/usage.json` on Linux.

## Releasing

Push a `v*` tag. GitHub Actions builds Windows x64, Linux x64 and Linux arm64 and attaches them to a release.

```sh
git tag v0.1.0 && git push origin v0.1.0
```
