#!/usr/bin/env python3
"""Build the cargo-packager-updater payload for this platform.

Requires a packaged dist/ from `cargo packager --release`.
Sign with CARGO_PACKAGER_SIGN_PRIVATE_KEY (and optional password).

Usage:
  scripts/prepare-updater-assets.py [version] [notes]
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import tarfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
GITHUB_REMOTE = re.compile(r"github\.com[:/](?P<repo>[^/]+/[^/.]+)(?:\.git)?$")


def packager_version() -> str:
    for line in (ROOT / "Packager.toml").read_text().splitlines():
        if line.startswith("version = "):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("could not read version from Packager.toml")


def normalize_arch(machine: str) -> str:
    if machine in {"arm64", "aarch64"}:
        return "aarch64"
    if machine in {"x86_64", "amd64", "AMD64"}:
        return "x86_64"
    return machine


def repo_from_text(text: str) -> str | None:
    match = GITHUB_REMOTE.search(text.strip())
    return match.group("repo") if match else None


def github_repo() -> str:
    env = os.environ.get("GITHUB_REPOSITORY", "").strip()
    if env:
        return env
    try:
        origin = subprocess.check_output(
            ["git", "remote", "get-url", "origin"],
            cwd=ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        repo = repo_from_text(origin)
        if repo:
            return repo
    except (OSError, subprocess.CalledProcessError):
        pass
    for path, keys in (
        (ROOT / "Packager.toml", ("homepage",)),
        (ROOT / "Cargo.toml", ("repository", "homepage")),
    ):
        if not path.is_file():
            continue
        for line in path.read_text().splitlines():
            for key in keys:
                prefix = f"{key} = "
                if line.startswith(prefix):
                    repo = repo_from_text(line.split("=", 1)[1].strip().strip('"'))
                    if repo:
                        return repo
    raise SystemExit(
        "could not determine GitHub repository; set GITHUB_REPOSITORY or configure git origin"
    )


def sign(path: Path) -> None:
    if not os.environ.get("CARGO_PACKAGER_SIGN_PRIVATE_KEY"):
        print(
            f"warning: CARGO_PACKAGER_SIGN_PRIVATE_KEY is unset; {path.name}.sig will be missing",
            file=sys.stderr,
        )
        return
    subprocess.run(
        ["cargo", "packager", "signer", "sign", str(path)],
        cwd=ROOT,
        check=True,
    )


def signature_for(path: Path) -> str:
    sig = path.with_name(path.name + ".sig")
    if sig.is_file():
        return sig.read_text()
    return ""


def write_fragment(
    *,
    version: str,
    notes: str,
    pub_date: str,
    platform_name: str,
    fmt: str,
    asset: Path,
    github_base: str,
) -> Path:
    fragment = {
        "version": version,
        "notes": notes,
        "pub_date": pub_date,
        "platforms": {
            platform_name: {
                "signature": signature_for(asset),
                "url": f"{github_base}/{asset.name}",
                "format": fmt,
            }
        },
    }
    path = DIST / f"latest-{platform_name}.json"
    path.write_text(json.dumps(fragment, indent=2) + "\n")
    print(f"wrote {path}")
    return path


def tar_macos_app(app: Path, dest: Path) -> None:
    with tarfile.open(dest, "w:gz") as archive:
        archive.add(app, arcname="Imprint.app")


def first_match(pattern: str) -> Path | None:
    matches = sorted(DIST.glob(pattern))
    return matches[0] if matches else None


def prepare_macos(
    arch: str, version: str, notes: str, pub_date: str, github_base: str
) -> None:
    app = DIST / "Imprint.app"
    if not app.is_dir():
        raise SystemExit(f"missing {app}; run cargo packager --release first")
    asset = DIST / f"Imprint_{arch}.app.tar.gz"
    tar_macos_app(app, asset)
    sign(asset)
    write_fragment(
        version=version,
        notes=notes,
        pub_date=pub_date,
        platform_name=f"macos-{arch}",
        fmt="app",
        asset=asset,
        github_base=github_base,
    )


def prepare_linux(
    arch: str, version: str, notes: str, pub_date: str, github_base: str
) -> None:
    asset = first_match("*.AppImage")
    if asset is None:
        raise SystemExit(f"missing AppImage in {DIST}; run cargo packager --release first")
    sign(asset)
    write_fragment(
        version=version,
        notes=notes,
        pub_date=pub_date,
        platform_name=f"linux-{arch}",
        fmt="appimage",
        asset=asset,
        github_base=github_base,
    )


def prepare_windows(
    arch: str, version: str, notes: str, pub_date: str, github_base: str
) -> None:
    msi = first_match("*.msi")
    nsis = first_match("*setup.exe") or first_match("*.exe")
    if msi is not None:
        asset, fmt = msi, "wix"
    elif nsis is not None:
        asset, fmt = nsis, "nsis"
    else:
        raise SystemExit(f"missing WiX/NSIS installer in {DIST}")
    sign(asset)
    write_fragment(
        version=version,
        notes=notes,
        pub_date=pub_date,
        platform_name=f"windows-{arch}",
        fmt=fmt,
        asset=asset,
        github_base=github_base,
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build the cargo-packager-updater payload for this platform.",
    )
    parser.add_argument(
        "version",
        nargs="?",
        help="Package version (default: Packager.toml)",
    )
    parser.add_argument(
        "notes",
        nargs="?",
        default="",
        help="Release notes stored on the fragment",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    DIST.mkdir(parents=True, exist_ok=True)
    version = args.version or packager_version()
    notes = args.notes or ""
    arch = normalize_arch(platform.machine())
    repo = github_repo()
    github_base = f"https://github.com/{repo}/releases/download/v{version}"
    pub_date = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    system = platform.system()

    if system == "Darwin":
        prepare_macos(arch, version, notes, pub_date, github_base)
    elif system == "Linux":
        prepare_linux(arch, version, notes, pub_date, github_base)
    elif system == "Windows":
        prepare_windows(arch, version, notes, pub_date, github_base)
    else:
        raise SystemExit(f"unsupported OS: {system}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
