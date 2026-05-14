#!/usr/bin/env python3
"""Bump Cargo.toml and regenerate CHANGELOG.md for a new release.

Used by .github/workflows/release.yml on workflow_dispatch.

Idempotent: re-running after a successful prep is a no-op (exits 0,
prints "already prepared"). Fails closed on malformed version input or
on a CHANGELOG that lacks any user-visible commits to release.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import pathlib
import re
import shutil
import subprocess
import sys

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


def regenerate_changelog(repo_root: pathlib.Path, version: str) -> bool:
    """Run git-cliff to rewrite CHANGELOG.md.

    Returns True if CHANGELOG.md actually changed on disk. Dies if the
    resulting file does not contain a `## [<version>] - …` section —
    i.e. there are no user-visible commits to release.
    """
    if shutil.which("git-cliff") is None:
        die("`git-cliff` not found on PATH — install it or use orhun/git-cliff-action in CI")

    changelog = repo_root / "CHANGELOG.md"
    before = changelog.read_text() if changelog.is_file() else ""
    subprocess.run(
        [
            "git-cliff",
            "--config",
            str(repo_root / "cliff.toml"),
            "--tag",
            f"v{version}",
            "-o",
            str(changelog),
        ],
        check=True,
        cwd=repo_root,
    )
    after = changelog.read_text()
    if f"## [{version}] - " not in after:
        die(
            f"CHANGELOG.md has no `## [{version}] - …` section after running "
            "git-cliff — there are no user-visible commits to release. Add "
            "feat/fix/refactor/docs commits or pick a different version."
        )
    return before != after


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
        help="Unused, retained for backwards compat with old CI invocations",
    )
    args = parser.parse_args()

    version = normalize_version(args.version)
    root = pathlib.Path(args.repo_root).resolve()
    cargo = root / "Cargo.toml"
    cliff = root / "cliff.toml"

    if not cargo.is_file():
        die(f"{cargo} not found")
    if not cliff.is_file():
        die(f"{cliff} not found")

    # Regenerate CHANGELOG first — it fails closed if there are no
    # user-visible commits to release, and we don't want to leave a
    # half-bumped Cargo.toml behind on failure.
    changelog_changed = regenerate_changelog(root, version)

    new_cargo = compute_cargo_bump(cargo.read_text(), version)
    if new_cargo is not None:
        cargo.write_text(new_cargo)

    actions = []
    if new_cargo is not None:
        actions.append("bumped Cargo.toml")
    if changelog_changed:
        actions.append("regenerated CHANGELOG.md")
    if actions:
        print(f"{' and '.join(actions)} for {version}")
    else:
        print(f"already prepared for {version}; no changes made")
    print(f"version={version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
