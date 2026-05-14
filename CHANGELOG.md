# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Mouse clicks in the palette list now resolve to the correct item regardless
  of the palette pane height. `select_line` previously computed its scroll
  offset against a hard-coded constant of 20 rows, so clicks on any pane
  shorter or taller than 20 rows could activate the wrong row.

### Changed
- Selection logic (`next_selectable`, `normalize_selection`, `list_offset`)
  moved to a new `selection` module as pure functions with dedicated unit
  tests. `State`'s methods now delegate to them. No behaviour change.

### Removed
- Dead `PaletteState` / `PaletteSnapshot` implementation in `src/state.rs`
  along with the two `lib.rs` tests that exercised it. The plugin always
  used the parallel implementation living on `State` in `src/main.rs`; the
  removed code was only reachable from tests.
- Unused `last_cols` field on `State` (introduced and removed within the
  same Unreleased cycle).

[Unreleased]: https://github.com/timonwong/zellij-palette/compare/HEAD...HEAD
