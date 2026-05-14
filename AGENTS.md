# AGENTS.md

Project-specific guidance for agents working in this repo. This file is
the canonical source; `CLAUDE.md` is a symlink to it so Claude Code,
Codex, Gemini, and any other AGENTS.md-aware tooling read the same text.

Cross-repo conventions (conversation language, English commits/PRs,
Markdown sanitization, Mermaid version handling, …) live in the
contributor's user-global or workspace-level `AGENTS.md` / `CLAUDE.md`
and are inherited automatically by the agent runtime — do not duplicate
them here.

## What this is

A Rust → WASM Zellij plugin that renders a fuzzy-searchable command
palette. Built artifact:

```
target/wasm32-wasip1/release/zellij-palette.wasm
```

Distributed as source on GitHub (`timonwong/zellij-palette`); not yet
published to crates.io.

## Module layout

- `src/model.rs` — `PaletteItem`, `PaletteAction`, `PaletteId`,
  `PaneTarget`, `CommandAction`, `PopupCoordinates`. Pure data types.
- `src/fuzzy.rs` — `filter_items` and scoring helpers. Pure.
- `src/selection.rs` — `next_selectable`, `normalize_selection`,
  `list_offset`. Pure helpers driving cursor movement and viewport
  scrolling, fully unit-tested.
- `src/state.rs` — `PermissionState` and `permission_placeholder_items`.
  Tiny on purpose; do not turn it back into a kitchen sink.
- `src/user_config.rs` — `~/.config/zellij-palette/**` loader (commands,
  custom palettes, shortcuts, aliases, hidden), plus
  `parse_command_palette_output` for shell-backed sources.
- `src/main.rs` — `State` and the `ZellijPlugin` impl (event handling,
  rendering, action dispatch). The only file that talks to `zellij-tile`
  host APIs. Keep host-coupled logic here; push pure logic into the
  modules above.
- `src/lib.rs` — module declarations and integration-ish tests for the
  pure modules.

## Build and test

```bash
# Wasm build (this is the artifact consumers load)
mise install
mise run build

# Host tests — wasm32-wasip1 is the default target via .cargo/config.toml,
# so tests need an explicit host-target override. `mise run test` resolves
# the active Rust host triple at runtime, so the same repo-local command
# works on macOS and Linux.
mise run test
```

`cargo test` without `--target` will fail on `core` because wasm32-wasip1
cannot host the test runner — this is expected, not a bug to fix by
removing the default target.

## Zellij plugin permission gating

Privileged host calls (`rename_plugin_pane`, `get_session_list`,
`get_session_environment_variables`, `run_command`, `reconfigure`, …)
must wait for `PermissionStatus::Granted`. Calling them inside `load()`
or before the grant event lands triggers a startup panic.

The pattern in `main.rs` is:

1. `load()` only subscribes, calls `request_permission`, and records a
   "waiting for permissions" message.
2. `permission_placeholder_items` from `state.rs` renders a non-selectable
   placeholder while gated.
3. `finish_startup_after_permissions` runs the first batch of host calls
   once `PermissionRequestResult(Granted)` arrives.

Do not add new privileged calls outside that path without re-checking
the permission gate.

## Coding conventions

- Edition 2024 (`Cargo.toml`); minimum Rust toolchain ≥ 1.85.
- Prefer pushing logic into the pure modules and unit-testing it there
  rather than growing `State`.
- No "guess" / placeholder functions that return a hard-coded constant
  dressed up as dynamic state. The original `last_rendered_rows_guess`
  always returned 20 regardless of input and silently broke mouse-click
  resolution on any palette pane sized differently.
- No unused struct fields kept "for later". If a field is not read by
  any code path in the current commit, do not introduce it.

## Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` / `fix:` / `refactor:` / `chore:` / `docs:` / `test:` / `perf:`
- Optional scope: `feat(selection): …`, `fix(render): …`
- `!` suffix for breaking changes: `refactor!: …`
- Subject in imperative mood, lowercase, no trailing period, ≤ 72 chars.
- Body wraps at ~72 cols and explains the *why*, not the *what*.
- All commit messages and PR titles/descriptions are in **English**,
  regardless of the conversation language.

Recent history is the reference style (`git log --oneline`).

`CHANGELOG.md` is generated end-to-end by the `Release` workflow from
this commit history (via [git-cliff](https://git-cliff.org/) +
`cliff.toml`); do not hand-edit it.

## Known open items (not yet scheduled)

These were called out in review but are *not* in flight. Confirm scope
before touching them:

- `visible_items()` rebuilds the full item vector on every render and
  every key — needs a cache keyed on `(active_palette, session
  revision, user_config revision)`.
- `apply_theme(ThemeAction::SetNamed(name))` formats `name` straight into
  a KDL string — escape or restrict before shipping multi-user configs.
- `main.rs` is ~1.2k lines; rendering / builders / actions / input
  ideally split into sibling modules.
- `PaletteId::Custom` enum variant is unreachable (custom palettes use
  `ActivePalette::Custom(String)`).
- `load_user_config` and `parse_command_palette_output` swallow all IO
  and parse errors silently — surface them through `State.message`.
