#!/usr/bin/env python3
"""Write an Arch Linux AUR PKGBUILD (imprint-bin) for both x86_64 and aarch64.

Expects pacman usr/ tarballs in dist/ (after .github/scripts/tag-release-assets.py):
  imprint_<version>_archlinux_x86_64.tar.gz
  imprint_<version>_archlinux_arm64.tar.gz

PKGBUILD `arch=` / `source_aarch64=` keep Arch's aarch64 name.

`--upload TAG` also uploads PKGBUILD and .SRCINFO to that GitHub Release.

Usage:
  .github/scripts/prepare-aur-pkgbuild.py
  .github/scripts/prepare-aur-pkgbuild.py --version 0.1.1 --upload v0.1.1
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
GITHUB_REMOTE = re.compile(r"github\.com[:/](?P<repo>[^/]+/[^/.]+)(?:\.git)?$")

AUR_DEPENDS = (
    "alsa-lib",
    "fontconfig",
    "freetype2",
    "libx11",
    "libxkbcommon",
    "libxkbcommon-x11",
    "mesa",
    "vulkan-icd-loader",
    "wayland",
)

ARCHES = ("x86_64", "aarch64")


def packager_version() -> str:
    for line in (ROOT / "Packager.toml").read_text().splitlines():
        if line.startswith("version = "):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("could not read version from Packager.toml")


def packager_field(key: str) -> str:
    prefix = f"{key} = "
    for line in (ROOT / "Packager.toml").read_text().splitlines():
        if line.startswith(prefix):
            return line.split("=", 1)[1].strip().strip('"')
    return ""


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
    homepage = packager_field("homepage")
    repo = repo_from_text(homepage) if homepage else None
    if repo:
        return repo
    raise SystemExit(
        "could not determine GitHub repository; set GITHUB_REPOSITORY or configure git origin"
    )


def sha512_file(path: Path) -> str:
    digest = hashlib.sha512()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_cpu(arch: str) -> str:
    if arch in {"aarch64", "arm64"}:
        return "arm64"
    return arch


def archive_candidates(version: str, arch: str) -> tuple[str, ...]:
    cpu = file_cpu(arch)
    names = [
        f"imprint_{version}_archlinux_{cpu}.tar.gz",
        f"imprint_{version}_{cpu}.tar.gz",
        f"imprint_{version}_archlinux_{arch}.tar.gz",
        f"imprint_{version}_{arch}.tar.gz",
    ]
    seen: list[str] = []
    for name in names:
        if name not in seen:
            seen.append(name)
    return tuple(seen)


def archive_name(version: str, arch: str) -> str:
    return archive_candidates(version, arch)[0]


def find_archive(version: str, arch: str) -> Path | None:
    for name in archive_candidates(version, arch):
        path = DIST / name
        if path.is_file():
            return path
    return None


def require_archives(version: str) -> dict[str, Path]:
    found: dict[str, Path] = {}
    missing: list[str] = []
    for arch in ARCHES:
        path = find_archive(version, arch)
        if path is not None:
            found[arch] = path
        else:
            missing.append(archive_name(version, arch))
    if missing:
        raise SystemExit(
            "missing pacman archives in dist/: " + ", ".join(missing)
        )
    return found


def bash_array(values: tuple[str, ...] | list[str]) -> str:
    return " ".join(f"'{v}'" for v in values)


def render_pkgbuild(
    *,
    version: str,
    repo: str,
    archives: dict[str, Path],
) -> str:
    homepage = packager_field("homepage") or f"https://github.com/{repo}"
    description = packager_field("description") or "Flash OS images onto USB drives and SD cards"
    source_lines = []
    sum_lines = []
    for arch in ARCHES:
        name = archives[arch].name
        url = f"https://github.com/{repo}/releases/download/v{version}/{name}"
        source_lines.append(f"source_{arch}=('{url}')")
        sum_lines.append(f"sha512sums_{arch}=('{sha512_file(archives[arch])}')")
    return f"""# Maintainer: Imprint Contributors
# Binary package for https://github.com/{repo}
# Prebuilt pacman packages (imprint-<pkgver>-1-<arch>.pkg.tar.zst) are also
# attached to the GitHub Release for `pacman -U`.
pkgname=imprint-bin
pkgver={version}
pkgrel=1
pkgdesc={description!r}
url={homepage!r}
arch=({bash_array(list(ARCHES))})
license=('Apache-2.0')
depends=({bash_array(AUR_DEPENDS)})
provides=('imprint')
conflicts=('imprint')
{chr(10).join(source_lines)}
{chr(10).join(sum_lines)}

package() {{
  cp -a "${{srcdir}}/usr" "${{pkgdir}}/usr"
}}
"""


def render_srcinfo(pkgbuild: str) -> str:
    """Minimal .SRCINFO so the AUR tarball can be submitted without makepkg."""
    fields: dict[str, list[str]] = {}
    for raw in pkgbuild.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line.endswith("{") or line == "}":
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip("'\"")
        if value.startswith("(") and value.endswith(")"):
            inner = value[1:-1].strip()
            parts = [p.strip().strip("'\"") for p in inner.split() if p.strip()]
            fields.setdefault(key, []).extend(parts)
        else:
            fields.setdefault(key, []).append(value)

    pkgname = fields.get("pkgname", ["imprint-bin"])[0]
    lines = [f"pkgbase = {pkgname}", f"pkgname = {pkgname}"]
    for key in (
        "pkgver",
        "pkgrel",
        "pkgdesc",
        "url",
        "arch",
        "license",
        "depends",
        "provides",
        "conflicts",
    ):
        for value in fields.get(key, []):
            lines.append(f"\t{key} = {value}")
    for arch in ARCHES:
        for value in fields.get(f"source_{arch}", []):
            lines.append(f"\tsource_{arch} = {value}")
        for value in fields.get(f"sha512sums_{arch}", []):
            lines.append(f"\tsha512sums_{arch} = {value}")
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Write an AUR imprint-bin PKGBUILD from pacman tarballs in dist/.",
    )
    parser.add_argument("--version", help="Package version (default: Packager.toml)")
    parser.add_argument(
        "--upload",
        metavar="TAG",
        help="Upload PKGBUILD and .SRCINFO to this GitHub Release",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=DIST / "aur",
        help="Directory for PKGBUILD and .SRCINFO (default: dist/aur)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    version = args.version or packager_version()
    repo = github_repo()
    DIST.mkdir(parents=True, exist_ok=True)
    archives = require_archives(version)
    out_dir: Path = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    pkgbuild = render_pkgbuild(version=version, repo=repo, archives=archives)
    srcinfo = render_srcinfo(pkgbuild)
    pkgbuild_path = out_dir / "PKGBUILD"
    srcinfo_path = out_dir / ".SRCINFO"
    pkgbuild_path.write_text(pkgbuild)
    srcinfo_path.write_text(srcinfo)
    print(f"wrote {pkgbuild_path}")
    print(f"wrote {srcinfo_path}")

    if args.upload:
        subprocess.run(
            [
                "gh",
                "release",
                "upload",
                args.upload,
                str(pkgbuild_path),
                str(srcinfo_path),
                "--clobber",
            ],
            cwd=ROOT,
            check=True,
        )
        print(f"uploaded PKGBUILD and .SRCINFO to {args.upload}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
