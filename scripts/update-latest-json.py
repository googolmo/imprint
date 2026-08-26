#!/usr/bin/env python3
"""Merge cargo-packager-updater fragments and optionally upload latest.json.

Fragments are written by scripts/prepare-updater-assets.py as
  dist/latest-<platform>.json

`--upload TAG` downloads that release's latest.json, merges the fragments
into it, then uploads. Merge happens only at upload time.

Examples:
  scripts/update-latest-json.py --upload v0.1.0
  scripts/update-latest-json.py -o dist/latest.json dist/latest-macos-aarch64.json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "dist" / "latest.json"
GITHUB_REMOTE = re.compile(r"github\.com[:/](?P<repo>[^/]+/[^/.]+)(?:\.git)?$")


def packager_version() -> str:
    for line in (ROOT / "Packager.toml").read_text().splitlines():
        if line.startswith("version = "):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("could not read version from Packager.toml")


def repo_from_text(text: str) -> str | None:
    match = GITHUB_REMOTE.search(text.strip())
    return match.group("repo") if match else None


def github_repo(explicit: str | None = None) -> str | None:
    if explicit:
        return explicit
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
    return None


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def empty_manifest() -> dict:
    return {"version": "", "notes": "", "pub_date": "", "platforms": {}}


def merge_into(manifest: dict, data: dict) -> None:
    if data.get("version"):
        manifest["version"] = data["version"]
    if data.get("notes"):
        manifest["notes"] = data["notes"]
    if data.get("pub_date") and not manifest["pub_date"]:
        manifest["pub_date"] = data["pub_date"]
    manifest["platforms"].update(data.get("platforms") or {})


def discover_fragments(dist: Path) -> list[Path]:
    return sorted(
        path for path in dist.glob("latest-*.json") if path.name != "latest.json"
    )


def download_release_latest(tag: str, dest_dir: Path, repo: str | None) -> Path | None:
    env = os.environ.copy()
    if repo:
        env["GH_REPO"] = repo
    result = subprocess.run(
        [
            "gh",
            "release",
            "download",
            tag,
            "--pattern",
            "latest.json",
            "--dir",
            str(dest_dir),
        ],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    downloaded = dest_dir / "latest.json"
    if result.returncode == 0 and downloaded.is_file():
        return downloaded
    print(
        f"no latest.json on release {tag}; starting a new manifest",
        file=sys.stderr,
    )
    return None


def upload_release_latest(tag: str, path: Path, repo: str | None) -> None:
    env = os.environ.copy()
    if repo:
        env["GH_REPO"] = repo
    subprocess.run(
        ["gh", "release", "upload", tag, str(path), "--clobber"],
        cwd=ROOT,
        env=env,
        check=True,
    )
    print(f"uploaded {path.name} to release {tag}")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Merge platform fragments into latest.json. With --upload, merge happens against the GitHub release asset.",
    )
    parser.add_argument(
        "fragments",
        nargs="*",
        type=Path,
        help="Fragment JSON files (default: dist/latest-*.json)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="Write here (default: dist/latest.json)",
    )
    parser.add_argument(
        "-b",
        "--base",
        type=Path,
        help="Existing latest.json to merge under the fragments",
    )
    parser.add_argument(
        "--upload",
        metavar="TAG",
        help="Merge into TAG's latest.json and upload it (--clobber)",
    )
    parser.add_argument(
        "--version",
        help="Override version (else Packager.toml / fragments)",
    )
    parser.add_argument("--notes", default="", help="Release notes")
    parser.add_argument(
        "--pub-date",
        help="Publication date (default: keep existing, else now)",
    )
    parser.add_argument(
        "--repo",
        help="GitHub repo owner/name (default: $GITHUB_REPOSITORY or origin)",
    )
    return parser.parse_args(argv)


def platform_keys(paths: list[Path]) -> set[str]:
    keys: set[str] = set()
    for path in paths:
        keys.update(load_json(path).get("platforms") or {})
    return keys


def apply_cli_overrides(
    manifest: dict, version: str, notes: str, pub_date: str | None
) -> None:
    if version:
        manifest["version"] = version
    if notes:
        manifest["notes"] = notes
    if pub_date:
        manifest["pub_date"] = pub_date
    if not manifest["pub_date"]:
        manifest["pub_date"] = datetime.now(timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"
        )


def write_manifest(path: Path, manifest: dict) -> None:
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    n = len(manifest["platforms"])
    print(f"wrote {path} ({n} platform(s), version {manifest['version']})")


def merge_sources(
    fragments: list[Path],
    base: Path | None,
    version: str,
    notes: str,
    pub_date: str | None,
) -> dict:
    manifest = empty_manifest()
    if base is not None and base.is_file():
        merge_into(manifest, load_json(base))
    for path in fragments:
        merge_into(manifest, load_json(path))
    apply_cli_overrides(manifest, version, notes, pub_date)
    return manifest


def upload_merged(
    tag: str,
    fragments: list[Path],
    output: Path,
    version: str,
    notes: str,
    pub_date: str | None,
    repo: str | None,
    attempts: int = 8,
) -> None:
    wanted = platform_keys(fragments)
    last_error: Exception | None = None
    for attempt in range(1, attempts + 1):
        with tempfile.TemporaryDirectory() as tmp:
            downloaded = download_release_latest(tag, Path(tmp), repo)
            manifest = merge_sources(fragments, downloaded, version, notes, pub_date)
            if not manifest["platforms"] and not fragments:
                raise SystemExit("no latest.json base or fragments to merge")
            write_manifest(output, manifest)
            upload_release_latest(tag, output, repo)
            verify_dir = Path(tmp) / "verify"
            verify_dir.mkdir()
            verified = download_release_latest(tag, verify_dir, repo)
            if verified is None:
                last_error = RuntimeError("latest.json missing after upload")
            else:
                have = set((load_json(verified).get("platforms") or {}).keys())
                if wanted <= have:
                    return
                last_error = RuntimeError(
                    f"release latest.json missing {sorted(wanted - have)} after upload"
                )
        delay = min(2 ** (attempt - 1), 8)
        print(
            f"retry {attempt}/{attempts} merging latest.json in {delay}s: {last_error}",
            file=sys.stderr,
        )
        time.sleep(delay)
    raise SystemExit(f"failed to merge latest.json onto {tag}: {last_error}")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = github_repo(args.repo)
    version = args.version or packager_version()
    output: Path = args.output
    output.parent.mkdir(parents=True, exist_ok=True)

    fragments = [path.resolve() for path in args.fragments]
    if not fragments:
        fragments = discover_fragments(ROOT / "dist")

    if args.upload:
        if not fragments:
            print("no fragments to merge into latest.json", file=sys.stderr)
            return 1
        upload_merged(
            args.upload,
            fragments,
            output,
            version,
            args.notes,
            args.pub_date,
            repo,
        )
        return 0

    base = args.base if args.base is not None else (output if output.is_file() else None)
    if not fragments and (base is None or not base.is_file()):
        print("no latest.json base or fragments to merge", file=sys.stderr)
        return 1
    manifest = merge_sources(fragments, base, version, args.notes, args.pub_date)
    write_manifest(output, manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
