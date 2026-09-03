#!/usr/bin/env python3
"""Tag the current version and/or bump Packager.toml / Cargo.toml.

The git tag is always `v` plus Packager.toml `version` (must match
Cargo.toml `[workspace.package]`). Release CI runs on a push of `v*`.

Usage:
  scripts/tag-release.py                    # tag v<Packager.toml>, push
  scripts/tag-release.py 0.1.1              # tag v<current>, push, then bump files
  scripts/tag-release.py 0.1.1 --no-tag     # bump Packager.toml + Cargo.toml only
  scripts/tag-release.py --dry-run
  scripts/tag-release.py --skip-push
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGER = ROOT / "Packager.toml"
CARGO = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"

# Path crates in this workspace (Cargo.lock `[[package]]` names).
WORKSPACE_CRATES = {
    "imprint-app",
    "imprint-build",
    "imprint-cli",
    "imprint-core",
    "imprint-device",
    "imprint-flash",
    "imprint-image",
    "imprint-rpi",
    "imprint-ui",
}

SEMVER = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?"
    r"(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)
VERSION_LINE = re.compile(r'^version\s*=\s*"(.*)"\s*$')


def die(message: str, code: int = 1) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(code)


def git(*args: str, capture: bool = True) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=capture,
    )
    if result.returncode != 0:
        err = (result.stderr or result.stdout or "").strip()
        die(f"git {' '.join(args)} failed" + (f": {err}" if err else ""))
    return (result.stdout or "").strip()


def packager_version(text: str | None = None) -> str:
    body = PACKAGER.read_text() if text is None else text
    for line in body.splitlines():
        match = VERSION_LINE.match(line)
        if match:
            return match.group(1)
    die("could not read version from Packager.toml")


def cargo_workspace_version(text: str | None = None) -> str:
    body = CARGO.read_text() if text is None else text
    in_pkg = False
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_pkg = stripped == "[workspace.package]"
            continue
        if not in_pkg:
            continue
        match = VERSION_LINE.match(line)
        if match:
            return match.group(1)
    die("could not read [workspace.package] version from Cargo.toml")


def set_packager_version(text: str, version: str) -> str:
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    replaced = False
    for line in lines:
        if not replaced and VERSION_LINE.match(line.rstrip("\n")):
            nl = "\n" if line.endswith("\n") else ""
            out.append(f'version = "{version}"{nl}')
            replaced = True
        else:
            out.append(line)
    if not replaced:
        die("could not find version line in Packager.toml")
    return "".join(out)


def set_cargo_workspace_version(text: str, version: str) -> str:
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    in_pkg = False
    replaced = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("["):
            in_pkg = stripped == "[workspace.package]"
            out.append(line)
            continue
        if in_pkg and not replaced and VERSION_LINE.match(line.rstrip("\n")):
            nl = "\n" if line.endswith("\n") else ""
            out.append(f'version = "{version}"{nl}')
            replaced = True
            continue
        out.append(line)
    if not replaced:
        die("could not find [workspace.package] version in Cargo.toml")
    return "".join(out)


def set_lockfile_workspace_versions(text: str, version: str) -> str:
    """Update version= of workspace path crates only, not registry deps."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    pending_name: str | None = None
    replaced = 0
    for line in lines:
        stripped = line.strip()
        if stripped == "[[package]]":
            pending_name = None
            out.append(line)
            continue
        name_match = re.match(r'^name\s*=\s*"(.*)"\s*$', stripped)
        if name_match:
            pending_name = name_match.group(1)
            out.append(line)
            continue
        if (
            pending_name in WORKSPACE_CRATES
            and VERSION_LINE.match(stripped)
        ):
            nl = "\n" if line.endswith("\n") else ""
            out.append(f'version = "{version}"{nl}')
            replaced += 1
            pending_name = None
            continue
        pending_name = None if stripped else pending_name
        out.append(line)
    if replaced != len(WORKSPACE_CRATES):
        die(
            f"Cargo.lock: expected {len(WORKSPACE_CRATES)} workspace crate "
            f"versions, updated {replaced}"
        )
    return "".join(out)


def require_clean_tree() -> None:
    status = git("status", "--porcelain")
    if status:
        die("working tree is not clean; commit or stash before tagging")


def require_tag_absent(tag: str) -> None:
    existing = git("tag", "-l", tag)
    if existing:
        die(f"git tag {tag} already exists locally")
    # Best-effort: origin may not have the tag either.
    ls = subprocess.run(
        ["git", "ls-remote", "--tags", "origin", f"refs/tags/{tag}"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if ls.returncode == 0 and ls.stdout.strip():
        die(f"git tag {tag} already exists on origin")


def write_versions(version: str) -> None:
    PACKAGER.write_text(set_packager_version(PACKAGER.read_text(), version))
    CARGO.write_text(set_cargo_workspace_version(CARGO.read_text(), version))
    if CARGO_LOCK.is_file():
        CARGO_LOCK.write_text(
            set_lockfile_workspace_versions(CARGO_LOCK.read_text(), version)
        )
    if packager_version() != version or cargo_workspace_version() != version:
        die("version write did not stick; files were not updated correctly")


def print_plan(current: str, new_version: str | None, tag: str | None) -> None:
    print(f"Packager.toml:   {current}")
    if tag is None:
        print("git tag:         (skipped)")
    else:
        print(f"git tag:         {tag} (HEAD, from Packager.toml)")
    if new_version is None:
        print("new version:     (unchanged)")
        return
    print(f"new version:     {new_version}")
    print(f"write:           {PACKAGER.relative_to(ROOT)}")
    print(f"write:           {CARGO.relative_to(ROOT)}")
    if CARGO_LOCK.is_file():
        print(f"write:           {CARGO_LOCK.relative_to(ROOT)}")


def print_commit_hint(new_version: str) -> None:
    print(f"set version {new_version} in Packager.toml and Cargo.toml")
    print("commit the version bump when ready:")
    print("  git add Packager.toml Cargo.toml Cargo.lock")
    print(f'  git commit -m "chore: bump version to {new_version}"')


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create and push git tag v<version> read from Packager.toml. "
            "Pass NEW_VERSION to also bump Packager.toml and Cargo.toml. "
            "Use --no-tag to only bump those files."
        )
    )
    parser.add_argument(
        "new_version",
        nargs="?",
        metavar="NEW_VERSION",
        help=(
            "Optional next version to write into Packager.toml and Cargo.toml "
            "(e.g. 0.1.1). Omit to tag the current Packager.toml version only."
        ),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print actions without tagging, pushing, or writing files",
    )
    tag_group = parser.add_mutually_exclusive_group()
    tag_group.add_argument(
        "--no-tag",
        action="store_true",
        help="Only set NEW_VERSION in Packager.toml and Cargo.toml; do not tag",
    )
    tag_group.add_argument(
        "--skip-push",
        action="store_true",
        help="Create the local tag but do not push to origin",
    )
    return parser.parse_args()


def parse_new_version(raw: str | None) -> str | None:
    if raw is None:
        return None
    version = raw.removeprefix("v")
    if not SEMVER.match(version):
        die(f"not a semver version: {raw}")
    return version


def main() -> None:
    args = parse_args()
    new_version = parse_new_version(args.new_version)
    if args.no_tag and new_version is None:
        die("--no-tag requires NEW_VERSION")

    current = packager_version()
    cargo_current = cargo_workspace_version()
    if current != cargo_current and not args.no_tag:
        die(
            f"version mismatch: Packager.toml={current} "
            f"Cargo.toml [workspace.package]={cargo_current}"
        )
    if not SEMVER.match(current):
        die(f"Packager.toml version is not semver: {current}")
    if new_version is not None and new_version == current and new_version == cargo_current:
        die(f"new version {new_version} is the same as the current version")

    tag = None if args.no_tag else f"v{current}"
    if args.dry_run:
        status = git("status", "--porcelain")
        if status and not args.no_tag:
            print(
                "warning: working tree is not clean (ok for --dry-run)",
                file=sys.stderr,
            )
    elif tag is not None:
        require_clean_tree()
        require_tag_absent(tag)

    print_plan(current, new_version, tag)

    if args.dry_run:
        print("dry-run: no tag, push, or file writes")
        return

    if tag is not None:
        git("tag", "-a", tag, "-m", tag, capture=False)
        print(f"created tag {tag}")
        if args.skip_push:
            print("skip-push: tag is local only")
        else:
            git("push", "origin", f"refs/tags/{tag}", capture=False)
            print(f"pushed {tag} to origin")

    if new_version is not None:
        write_versions(new_version)
        print_commit_hint(new_version)


if __name__ == "__main__":
    main()
