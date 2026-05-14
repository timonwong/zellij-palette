# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- The Themes palette now lists all 41 themes Zellij 0.44 ships built-in
  (`ansi`, `ao`, `atelier`, `ayu-*`, `catppuccin-*`, `dracula`, `gruvbox-*`,
  `nord`, `tokyo-night-*`, etc.) on top of the existing scan of
  `~/.config/zellij/themes/*.kdl`. Built-in and user themes appear in
  separate `Built-in Themes` / `User Themes` groups, and a same-named
  user theme shadows the built-in entry. The list is hard-coded to
  match Zellij 0.44's `include_dir!` set because zellij-tile 0.44 does
  not expose a runtime API to enumerate themes; bump alongside the
  zellij-tile dep.
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

### Fixed
- Mouse clicks in the palette list now resolve to the correct item regardless
  of the palette pane height. `select_line` previously computed its scroll
  offset against a hard-coded constant of 20 rows, so clicks on any pane
  shorter or taller than 20 rows could activate the wrong row.

### Changed
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

[Unreleased]: https://github.com/timonwong/zellij-palette/compare/HEAD...HEAD
