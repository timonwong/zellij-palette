#!/usr/bin/env python3
"""Promote CHANGELOG and bump Cargo.toml for a new release.

Used by .github/workflows/release.yml on workflow_dispatch.

Idempotent: re-running after a successful promotion is a no-op (exits 0,
prints "already promoted"). Fails closed on empty [Unreleased] or
malformed version input.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import pathlib
import re
import sys

REPO = "timonwong/zellij-palette"
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+([.\-].+)?$")


def die(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def normalize_version(raw: str) -> str:
    v = raw.strip().lstrip("v")
    if not SEMVER.match(v):
        die(
            f"version {raw!r} must match X.Y.Z (optionally with a -prerelease or .build suffix)"
        )
    return v


def compute_cargo_bump(text: str, version: str) -> str | None:
    """Return new Cargo.toml content, or None if already at `version`."""
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, flags=re.MULTILINE)
    if not m:
        die("Cargo.toml has no top-level `version = \"...\"` line")
    if m.group(1) == version:
        return None
    return text[: m.start(1)] + version + text[m.end(1) :]


def compute_changelog_promotion(text: str, version: str, today: str) -> str | None:
    """Return promoted CHANGELOG, or None if `[version]` section already exists."""
    section_header = f"## [{version}]"
    if re.search(
        rf"^{re.escape(section_header)}(?:[ \t]|$)", text, flags=re.MULTILINE
    ):
        return None

    unreleased_re = re.compile(r"^## \[Unreleased\][ \t]*$", flags=re.MULTILINE)
    m = unreleased_re.search(text)
    if not m:
        die("CHANGELOG.md has no `## [Unreleased]` section header")

    next_header = re.search(r"^## \[", text[m.end():], flags=re.MULTILINE)
    body_end = m.end() + next_header.start() if next_header else len(text)
    body = text[m.end():body_end].strip()
    if not body:
        die(
            "[Unreleased] section is empty — add release notes before "
            "running the prep workflow"
        )

    insertion = f"\n\n## [{version}] - {today}"
    new_text = text[: m.end()] + insertion + text[m.end():]

    footer_re = re.compile(
        rf"^(\[Unreleased\]: https://github\.com/{re.escape(REPO)}/compare/)"
        r"(\S+?)\.\.\.HEAD\s*$",
        flags=re.MULTILINE,
    )
    fm = footer_re.search(new_text)
    if not fm:
        die(
            "could not find the `[Unreleased]: …/compare/<ref>...HEAD` footer "
            "link — promote the CHANGELOG by hand or fix the footer convention"
        )
    new_footer = f"[Unreleased]: https://github.com/{REPO}/compare/v{version}...HEAD"
    new_text = new_text[: fm.start()] + new_footer + new_text[fm.end():]

    insert_at = fm.start() + len(new_footer)
    tag_link = f"\n[{version}]: https://github.com/{REPO}/releases/tag/v{version}"
    new_text = new_text[: insert_at] + tag_link + new_text[insert_at:]

    if not new_text.endswith("\n"):
        new_text += "\n"
    return new_text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--version", required=True, help="Version to release, e.g. 0.2.0 or v0.2.0"
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Path to the repo root containing Cargo.toml and CHANGELOG.md",
    )
    parser.add_argument(
        "--date",
        default=_dt.date.today().isoformat(),
        help="Release date in YYYY-MM-DD (default: today, UTC)",
    )
    args = parser.parse_args()

    version = normalize_version(args.version)
    root = pathlib.Path(args.repo_root)
    cargo = root / "Cargo.toml"
    changelog = root / "CHANGELOG.md"

    if not cargo.is_file():
        die(f"{cargo} not found")
    if not changelog.is_file():
        die(f"{changelog} not found")

    new_cargo = compute_cargo_bump(cargo.read_text(), version)
    new_changelog = compute_changelog_promotion(
        changelog.read_text(), version, args.date
    )

    if new_cargo is not None:
        cargo.write_text(new_cargo)
    if new_changelog is not None:
        changelog.write_text(new_changelog)

    actions = []
    if new_cargo is not None:
        actions.append("bumped Cargo.toml")
    if new_changelog is not None:
        actions.append("promoted CHANGELOG")
    if actions:
        print(f"{' and '.join(actions)} for {version}")
    else:
        print(f"already promoted to {version}; no changes made")
    print(f"version={version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
