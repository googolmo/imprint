#!/usr/bin/env python3
"""Rename cargo-packager dist/ files so each asset includes OS + CPU.

cargo-packager names mix Debian (amd64/arm64), Windows (x64/arm64), and rustc
(x86_64/aarch64) arches, and only the Ubuntu 24.04 job used to inject a distro
tag. Release assets become:

  imprint_{version}_{system}_{cpu}{suffix}

  system  ubuntu22.04 | ubuntu24.04 | macos | windows | archlinux
  cpu     x86_64 | arm64

Examples:
  imprint_0.1.3_amd64.deb              → imprint_0.1.3_ubuntu22.04_x86_64.deb
  imprint_0.1.3_x86_64.AppImage        → imprint_0.1.3_ubuntu22.04_x86_64.AppImage
  Imprint_0.1.3_aarch64.dmg            → imprint_0.1.3_macos_arm64.dmg
  imprint_0.1.3_x64_en-US.msi          → imprint_0.1.3_windows_x86_64.msi
  imprint_0.1.3_arm64-setup.exe        → imprint_0.1.3_windows_arm64-setup.exe
  imprint_0.1.3_x86_64.tar.gz          → imprint_0.1.3_archlinux_x86_64.tar.gz
  imprint-0.1.3-1-x86_64.pkg.tar.zst   → imprint_0.1.3_archlinux_x86_64.pkg.tar.zst

Sibling `.sig` files move with the asset. latest-*.json, PKGBUILD, and
directories are left alone.

Usage:
  .github/scripts/tag-release-assets.py --version 0.1.3 --arch x86_64 --deb-tag ubuntu22.04
  .github/scripts/tag-release-assets.py --self-test
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DIST = ROOT / "dist"

CPU_ALIASES = {
    "x86_64": "x86_64",
    "amd64": "x86_64",
    "x64": "x86_64",
    "AMD64": "x86_64",
    "aarch64": "arm64",
    "arm64": "arm64",
    "ARM64": "arm64",
}

SKIP_EXACT = {"latest.json", "PKGBUILD", ".SRCINFO"}
SKIP_PREFIXES = ("latest-", "PKGBUILD.")


def normalize_cpu(arch: str) -> str:
    key = arch.strip()
    mapped = CPU_ALIASES.get(key) or CPU_ALIASES.get(key.lower())
    if mapped is None:
        raise SystemExit(
            f"unsupported arch {arch!r}; imprint packages x86_64 and arm64 only"
        )
    return mapped


def classify(path: Path) -> str | None:
    """Return a kind tag, or None if this file is not a release package."""
    name = path.name
    if not path.is_file():
        return None
    if name.endswith(".sig"):
        return None
    if name in SKIP_EXACT or name.startswith(SKIP_PREFIXES):
        return None
    if not name.lower().startswith("imprint"):
        return None
    if name.endswith(".deb"):
        return "deb"
    if name.endswith(".AppImage"):
        return "appimage"
    if name.endswith(".dmg"):
        return "dmg"
    if name.endswith(".msi"):
        return "msi"
    if name.endswith("-setup.exe"):
        return "nsis"
    if name.endswith(".pkg.tar.zst") or name.endswith(".pkg.tar.xz"):
        return "archpkg"
    if name.endswith(".app.tar.gz"):
        return "apptar"
    if name.endswith(".tar.gz"):
        return "pacman_tar"
    return None


def pkg_suffix(path: Path, kind: str) -> str:
    name = path.name
    if kind == "nsis":
        return "-setup.exe"
    if kind == "archpkg":
        if name.endswith(".pkg.tar.xz"):
            return ".pkg.tar.xz"
        return ".pkg.tar.zst"
    if kind == "apptar":
        return ".app.tar.gz"
    if kind == "pacman_tar":
        return ".tar.gz"
    return path.suffix  # .deb .AppImage .dmg .msi


def system_for(kind: str, deb_tag: str | None) -> str:
    mapping = {
        "deb": deb_tag or "linux",
        # AppImage is built on the same runner as the .deb (Ubuntu 22.04 today).
        "appimage": deb_tag or "linux",
        "dmg": "macos",
        "msi": "windows",
        "nsis": "windows",
        "archpkg": "archlinux",
        "pacman_tar": "archlinux",
        "apptar": "macos",
    }
    system = mapping[kind]
    if kind == "deb" and not deb_tag:
        raise SystemExit(
            f"refusing to tag {kind} without --deb-tag "
            "(ubuntu22.04 or ubuntu24.04); filenames would collide across distros"
        )
    return system


def canonical_name(version: str, system: str, cpu: str, suffix: str) -> str:
    return f"imprint_{version}_{system}_{cpu}{suffix}"


def move_with_sig(src: Path, dest: Path) -> None:
    if dest.resolve() == src.resolve():
        return
    if dest.exists():
        raise SystemExit(f"refusing to overwrite existing {dest}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(src), str(dest))
    src_sig = src.with_name(src.name + ".sig")
    if src_sig.is_file():
        dest_sig = dest.with_name(dest.name + ".sig")
        if dest_sig.exists() and dest_sig.resolve() != src_sig.resolve():
            raise SystemExit(f"refusing to overwrite existing {dest_sig}")
        shutil.move(str(src_sig), str(dest_sig))


def tag_assets(
    dist: Path,
    *,
    version: str,
    arch: str,
    deb_tag: str | None = None,
    dry_run: bool = False,
) -> list[tuple[Path, Path]]:
    cpu = normalize_cpu(arch)
    planned: list[tuple[Path, Path]] = []
    dests: dict[Path, Path] = {}
    for path in sorted(dist.iterdir()):
        kind = classify(path)
        if kind is None:
            continue
        system = system_for(kind, deb_tag)
        dest = dist / canonical_name(version, system, cpu, pkg_suffix(path, kind))
        planned.append((path, dest))
        if dest in dests and dests[dest].resolve() != path.resolve():
            raise SystemExit(
                f"two inputs map to {dest.name}: {dests[dest].name} and {path.name}"
            )
        dests[dest] = path

    if dry_run:
        return planned

    for src, dest in planned:
        if dest.resolve() == src.resolve():
            print(f"keep {src.name}")
            continue
        move_with_sig(src, dest)
        print(f"renamed {src.name} -> {dest.name}")
    return planned


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rename dist/ packages to imprint_{version}_{system}_{cpu}.*",
    )
    parser.add_argument("--version", help="Package version (required unless --self-test)")
    parser.add_argument(
        "--arch",
        help="CPU architecture of this job (x86_64 or arm64; aarch64/amd64/x64 aliases ok)",
    )
    parser.add_argument(
        "--deb-tag",
        help="OS tag for .deb files (ubuntu22.04 or ubuntu24.04). Required when dist/ has a .deb.",
    )
    parser.add_argument(
        "--dist",
        type=Path,
        default=DEFAULT_DIST,
        help="Package directory (default: dist/)",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print planned renames only")
    parser.add_argument("--self-test", action="store_true", help="Run built-in checks and exit")
    return parser.parse_args(argv)


def _touch(path: Path, body: str = "x") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)
    return path


def _self_test() -> None:
    cases: list[tuple[str, str | None, list[str], list[str]]] = [
        (
            "x86_64",
            "ubuntu22.04",
            [
                "imprint_0.1.3_amd64.deb",
                "imprint_0.1.3_amd64.deb.sig",
                "imprint_0.1.3_x86_64.AppImage",
                "imprint_0.1.3_x86_64.tar.gz",
                "imprint-0.1.3-1-x86_64.pkg.tar.zst",
                "PKGBUILD",
                "latest-linux-x86_64.json",
            ],
            [
                "imprint_0.1.3_ubuntu22.04_x86_64.deb",
                "imprint_0.1.3_ubuntu22.04_x86_64.deb.sig",
                "imprint_0.1.3_ubuntu22.04_x86_64.AppImage",
                "imprint_0.1.3_archlinux_x86_64.tar.gz",
                "imprint_0.1.3_archlinux_x86_64.pkg.tar.zst",
                "PKGBUILD",
                "latest-linux-x86_64.json",
            ],
        ),
        (
            "aarch64",
            "ubuntu22.04",
            [
                "imprint_0.1.3_arm64.deb",
                "imprint_0.1.3_aarch64.AppImage",
                "imprint_0.1.3_aarch64.tar.gz",
                "imprint-0.1.3-1-aarch64.pkg.tar.zst",
            ],
            [
                "imprint_0.1.3_ubuntu22.04_arm64.deb",
                "imprint_0.1.3_ubuntu22.04_arm64.AppImage",
                "imprint_0.1.3_archlinux_arm64.tar.gz",
                "imprint_0.1.3_archlinux_arm64.pkg.tar.zst",
            ],
        ),
        (
            "aarch64",
            "ubuntu24.04",
            ["imprint_0.1.3_arm64.deb", "imprint_0.1.3_arm64.deb.sig"],
            [
                "imprint_0.1.3_ubuntu24.04_arm64.deb",
                "imprint_0.1.3_ubuntu24.04_arm64.deb.sig",
            ],
        ),
        (
            "aarch64",
            None,
            ["Imprint_0.1.3_aarch64.dmg", "Imprint_0.1.3_aarch64.dmg.sig"],
            ["imprint_0.1.3_macos_arm64.dmg", "imprint_0.1.3_macos_arm64.dmg.sig"],
        ),
        (
            "x86_64",
            None,
            ["imprint_0.1.3_x64_en-US.msi"],
            ["imprint_0.1.3_windows_x86_64.msi"],
        ),
        (
            "aarch64",
            None,
            ["imprint_0.1.3_arm64-setup.exe"],
            ["imprint_0.1.3_windows_arm64-setup.exe"],
        ),
    ]
    for arch, deb_tag, inputs, expected in cases:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp)
            for name in inputs:
                _touch(dist / name, name)
            tag_assets(dist, version="0.1.3", arch=arch, deb_tag=deb_tag)
            got = sorted(p.name for p in dist.iterdir())
            want = sorted(expected)
            if got != want:
                raise SystemExit(f"self-test failed arch={arch} deb_tag={deb_tag}\n  got:  {got}\n  want: {want}")
    # Idempotent when names are already canonical.
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_ubuntu22.04_x86_64.deb")
        tag_assets(dist, version="0.1.3", arch="x86_64", deb_tag="ubuntu22.04")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_ubuntu22.04_x86_64.deb"]:
            raise SystemExit(f"idempotent rename failed: {names}")
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_ubuntu22.04_arm64.deb")
        tag_assets(dist, version="0.1.3", arch="arm64", deb_tag="ubuntu22.04")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_ubuntu22.04_arm64.deb"]:
            raise SystemExit(f"idempotent arm64 rename failed: {names}")
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_amd64.deb")
        try:
            tag_assets(dist, version="0.1.3", arch="x86_64")
        except SystemExit:
            pass
        else:
            raise SystemExit("expected SystemExit when tagging a .deb without --deb-tag")
    print("self-test ok")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        _self_test()
        return 0
    if not args.version or not args.arch:
        print("--version and --arch are required", file=sys.stderr)
        return 2
    dist: Path = args.dist
    if not dist.is_dir():
        print(f"missing package directory {dist}", file=sys.stderr)
        return 1
    tag_assets(
        dist,
        version=args.version,
        arch=args.arch,
        deb_tag=args.deb_tag,
        dry_run=args.dry_run,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
