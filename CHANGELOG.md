# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.1.1-cliff-test] - 2026-05-14

### Fixed

- restore select pane navigation (#9)

## [0.1.0] - 2026-05-14

### Added
- README now credits `tmux-palette` (by @eduwass) up front as the design
  source, and carries an explicit "AI slop" disclaimer so readers know
  the codebase is mostly AI-generated.
- The Themes palette now lists all 41 themes Zellij 0.44 ships built-in
  (`ansi`, `ao`, `atelier`, `ayu-*`, `catppuccin-*`, `dracula`, `gruvbox-*`,
  `nord`, `tokyo-night-*`, etc.) on top of any user theme files the
  plugin is pointed at. Built-in and user themes appear in separate
  `Built-in Themes` / `User Themes` groups, and a same-named user theme
  shadows the built-in entry. The list is hard-coded to match Zellij
  0.44's `include_dir!` set because zellij-tile 0.44 does not expose a
  runtime API to enumerate themes; bump alongside the zellij-tile dep.
- New `theme_dir` plugin parameter (set in the Zellij KDL alias block
  alongside `palette` and `category`) tells the plugin where to scan
  for user `*.kdl` theme files. Accepts absolute paths or `~/...`
  expanded against the session `$HOME`. When unset, the User Themes
  group is hidden and only built-ins are listed.
- Theme names interpolated into the `reconfigure(...)` KDL fragment are
  now escaped (`"` and `\`), so a user-supplied theme name with quotes
  or backslashes can no longer break out of the KDL string value.
- User config files can now be authored as TOML, YAML, or JSON. When
  multiple variants of the same name exist, the loader prefers TOML,
  then YAML, then JSON; a broken higher-priority file no longer
  silently falls through to a lower-priority sibling. Snake_case keys
  (`icon_color`, `from_category`, `from_group`, `empty_text`) are now
  accepted alongside the existing camelCase forms. Existing JSON
  configs keep working unchanged.
- A repo-local `mise.toml` now pins Rust 1.86, installs the
  `wasm32-wasip1` target plus `rustfmt` / `clippy`, and exposes
  `mise run` tasks for `build`, `test`, `fmt`, `clippy`, and local
  `install` into `~/.config/zellij/plugins/zellij-palette.wasm`. The
  host-side tasks resolve the active Rust host triple at runtime, so
  the same task names work on macOS and Linux.

### Fixed
- Mouse clicks in the palette list now resolve to the correct item regardless
  of the palette pane height. `select_line` previously computed its scroll
  offset against a hard-coded constant of 20 rows, so clicks on any pane
  shorter or taller than 20 rows could activate the wrong row.
- `Find Pane` / `Select Pane` now jumps correctly inside the current session.
  Targets in the active session use direct pane focus, which switches tab/layer
  by pane id; cross-session targets still use session switching with focus.
- `Find Pane` / `Select Pane` no longer lists the palette plugin pane itself.
  The chooser now filters out the current plugin pane while keeping other
  selectable panes, including other plugin panes.

### Changed
- README and `examples/config.kdl` now load the plugin straight from
  `https://github.com/timonwong/zellij-palette/releases/latest/download/zellij-palette.wasm`
  instead of a hand-edited local path. Zellij caches remote plugins by
  URL hash under `$ZELLIJ_CACHE_DIR`, so `latest` is fetched once and
  reused; pin to a versioned URL (`/releases/download/vX.Y.Z/...`) or
  clear the matching cache entry to upgrade.
- Example launcher keybinding switched from `Ctrl p` (normal mode only —
  also collides with Zellij's default Pane-mode prefix) to `Ctrl Shift p`
  under the `shared` scope, so the palette opens from any input mode.
  `Alt t` (themes) and `Alt o` (tools) stay in `normal`.
- User theme discovery is now opt-in via the `theme_dir` plugin
  parameter. The previous implicit scan of `~/.config/zellij/themes/`
  has been removed because we cannot reliably reproduce Zellij's full
  `ZELLIJ_CONFIG_DIR` / `config.kdl theme_dir` / XDG resolution from a
  plugin (zellij-tile 0.44 exposes no `get_theme_dir()`, and parsing
  `config.kdl` would need a KDL parser). Users who relied on the
  implicit scan should add `theme_dir "~/.config/zellij/themes"` to
  the `zellij-palette-themes` alias block — the bundled
  `examples/config.kdl` shows the new shape.
- Selection logic (`next_selectable`, `normalize_selection`, `list_offset`)
  moved to a new `selection` module as pure functions with dedicated unit
  tests. `State`'s methods now delegate to them. No behaviour change.
- Bundled samples under `examples/` were rewritten from JSON to TOML
  (`commands`, `shortcuts`, `aliases`, `hidden`, `palettes/github-prs`).
  TOML is now the documented default in the README; JSON remains a
  supported authoring format.

### Removed
- Dead `PaletteState` / `PaletteSnapshot` implementation in `src/state.rs`
  along with the two `lib.rs` tests that exercised it. The plugin always
  used the parallel implementation living on `State` in `src/main.rs`; the
  removed code was only reachable from tests.
- Unused `last_cols` field on `State` (introduced and removed within the
  same Unreleased cycle).

[Unreleased]: https://github.com/timonwong/zellij-palette/compare/v0.1.1-cliff-test...HEAD
[0.1.1-cliff-test]: https://github.com/timonwong/zellij-palette/releases/tag/v0.1.1-cliff-test
[0.1.0]: https://github.com/timonwong/zellij-palette/releases/tag/v0.1.0

