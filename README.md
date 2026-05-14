# zellij-palette

A Rust/WASM command palette for Zellij, modeled after [`tmux-palette`](https://github.com/eduwass/tmux-palette).

It opens as a Zellij plugin pane, lets you fuzzy-search actions, and dispatches pane/tab/session/theme commands without leaving the keyboard.

## Current feature set

- Searchable `Commands` palette with pane, tab, session, and appearance actions
- `Find Pane` palette that jumps across sessions, tabs, and panes
- `Move Pane` palette that sends the caller pane into another tab or a new tab
- `Themes` palette for dark/light/toggle and user theme names from `~/.config/zellij/themes/*.kdl`
- Custom commands from `~/.config/zellij-palette/commands.json`
- Custom palettes from `~/.config/zellij-palette/palettes/*.json`
- `hidden.json`, `shortcuts.json`, and `aliases.json` overlays
- Category-aware custom palettes via `fromCategory`
- Shell-backed palette sources that emit JSON items or plain lines with optional icon metadata
- Focused launch bindings for a built-in palette, a custom palette, or a single category

## Build

```bash
mise exec rust@stable -- cargo build --release
```

The plugin artifact is:

```bash
target/wasm32-wasip1/release/zellij-palette.wasm
```

## Bind It In Zellij

Add a keybinding that launches the plugin as a floating pane.

Example snippet:

```kdl
plugins {
    zellij-palette location="file:/ABS/PATH/TO/zellij-palette/target/wasm32-wasip1/release/zellij-palette.wasm"
    zellij-palette-themes location="file:/ABS/PATH/TO/zellij-palette/target/wasm32-wasip1/release/zellij-palette.wasm" {
        palette "themes"
    }
    zellij-palette-tools location="file:/ABS/PATH/TO/zellij-palette/target/wasm32-wasip1/release/zellij-palette.wasm" {
        category "Tools"
    }
}

keybinds {
    normal {
        bind "Ctrl p" {
            LaunchOrFocusPlugin "zellij-palette" {
                floating true
                move_to_focused_tab true
            }
        }
        bind "Alt t" {
            LaunchOrFocusPlugin "zellij-palette-themes" {
                floating true
                move_to_focused_tab true
            }
        }
        bind "Alt o" {
            LaunchOrFocusPlugin "zellij-palette-tools" {
                floating true
                move_to_focused_tab true
            }
        }
    }
}
```

The plugin reads two launch keys from the alias configuration block:

- `palette`: `commands`, `find-pane`, `move-pane`, `sessions`, `themes`, or a custom palette filename
- `category`: filters the root commands palette to one category such as `Tools`

There is a ready-to-edit example in [examples/config.kdl](examples/config.kdl).

## Runtime behavior

- `Esc` clears the query first, then goes back one palette level, then closes the plugin
- `Ctrl-C` follows the same close path
- `Enter` runs the highlighted action
- `Up` / `Down` and `Ctrl-P` / `Ctrl-N` move selection
- Mouse wheel and hover update selection

The plugin tracks the caller pane through Zellij's pane history, so actions such as `Move Pane`, `Toggle Fullscreen`, `Float / Embed Pane`, and `Close Pane` target the pane that was focused before the palette opened.

## User config

Config lives under:

```text
~/.config/zellij-palette/
```

### Extra commands

Path:

```text
~/.config/zellij-palette/commands.json
```

Example:

```json
[
  {
    "title": "lazygit",
    "description": "open lazygit in a floating command pane",
    "category": "Tools",
    "icon": "󰊢",
    "aliases": ["git", "lg"],
    "shortcut": "Ctrl-G",
    "action": {
      "popup": "lazygit",
      "width": "80%",
      "height": "80%",
      "borderless": true
    }
  },
  {
    "title": "Reload shell rc",
    "group": "Tools",
    "action": { "shell": "exec $SHELL -lc 'source ~/.zshrc'" }
  }
]
```

Supported item fields:

- `title`
- `description`
- `category` or `group`
- `aliases`
- `shortcut`
- `icon`
- `iconColor`
- popup action sizing keys: `x`, `y`, `width`, `height`, `pinned`, `borderless`
- `action`

`group` stays as a compatibility alias for `category`.

### Hidden items

Path:

```text
~/.config/zellij-palette/hidden.json
```

Example:

```json
["Previous Tab", "Detach Session"]
```

### Shortcut labels

Path:

```text
~/.config/zellij-palette/shortcuts.json
```

Example:

```json
{
  "Find Pane": "Ctrl-F",
  "lazygit": "Ctrl-G"
}
```

### Visible alias chips

Path:

```text
~/.config/zellij-palette/aliases.json
```

Example:

```json
{
  "Find Pane": ["locator"],
  "Switch Theme...": ["appearance"]
}
```

### Custom palettes

Path:

```text
~/.config/zellij-palette/palettes/<name>.json
```

Example:

```json
{
  "title": "GitHub PRs",
  "fromCategory": "Tools",
  "icon": "󰘬",
  "iconColor": "#58a6ff",
  "command": "gh pr list --limit 20 --json number,title --jq '.[] | \"#\\(.number) \\(.title)\"'",
  "action": { "popup": "open {}" }
}
```

Supported item actions:

- `{ "palette": "themes" }`
- `{ "palette": "find-pane" }`
- `{ "palette": "<custom-name>" }`
- `{ "shell": "..." }`
- `{ "popup": "..." }`
- `{ "popup": "...", "x": "10%", "y": "10%", "width": "80%", "height": "80%", "pinned": true, "borderless": true }`
- `{ "theme": "dark" }`
- `{ "theme": "light" }`
- `{ "theme": "toggle" }`
- `{ "theme": "<theme-name>" }`

For shell-backed palette sources:

- If the command prints a JSON array, each entry should match the same item schema as `commands.json`
- If the command prints plain lines, pair it with an `action` template and use `{}` as the selected line placeholder
- Plain-line mode also accepts tab-separated icon fields:
  - `<title>`
  - `<icon>\t<title>`
  - `<icon>\t<iconColor>\t<title>`

Custom palette keys:

- `title`
- `from`
- `fromCategory`
- `fromGroup`
- `command`
- `action`
- `icon`
- `iconColor`
- `grouped`
- `emptyText`
- `items`

## Smoke test

1. Build the plugin.
2. Copy [examples/config.kdl](examples/config.kdl) and replace `__WASM__` with the absolute wasm path.
3. Copy any wanted example JSON files from [examples/](examples/) into `~/.config/zellij-palette/`.
4. Start Zellij with that config.
5. Press `Ctrl-p`, `Alt-t`, and `Alt-o`.

There is also a pane-layout based loader in [examples/smoke-layout.kdl](examples/smoke-layout.kdl).
