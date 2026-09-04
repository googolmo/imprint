#!/usr/bin/env python3
"""Rename cargo-packager output so each asset includes OS + CPU.

Release CI writes packages into dist/<pack_dir>/, where pack_dir is
`{system}[-{version}]-{arch}` and arch is amd64 | arm64:

  macos-arm64, macos-amd64
  ubuntu-24.04-amd64, ubuntu-24.04-arm64
  ubuntu-26.04-amd64, ubuntu-26.04-arm64
  windows-amd64, windows-arm64
  archlinux-amd64, archlinux-arm64

cargo-packager names mix Debian (amd64/arm64), Windows (x64/arm64), and rustc
(x86_64/aarch64) arches. Files inside the pack dir become:

  imprint_{version}_{system}_{cpu}{suffix}

  system  ubuntu24.04 | ubuntu26.04 | macos | windows | archlinux
  cpu     amd64 | arm64

Examples:
  dist/ubuntu-24.04-amd64/imprint_0.1.3_amd64.deb
      → imprint_0.1.3_ubuntu24.04_amd64.deb
  dist/ubuntu-24.04-amd64/imprint_0.1.3_x86_64.AppImage
      → imprint_0.1.3_ubuntu24.04_amd64.AppImage
  dist/macos-arm64/Imprint_0.1.3_aarch64.dmg
      → imprint_0.1.3_macos_arm64.dmg
  dist/windows-amd64/imprint_0.1.3_x64_en-US.msi
      → imprint_0.1.3_windows_amd64.msi
  dist/windows-arm64/imprint_0.1.3_arm64-setup.exe
      → imprint_0.1.3_windows_arm64-setup.exe
  dist/archlinux-amd64/imprint-0.1.3-1-x86_64.pkg.tar.zst
      → imprint_0.1.3_archlinux_amd64.pkg.tar.zst

Sibling `.sig` files move with the asset. latest-*.json, PKGBUILD, and
directories are left alone.

A pack dir name supplies system + cpu, so `--arch` / `--deb-tag` are optional
when `--dist` is (or contains) those directories. Flat `dist/` still needs
`--arch` (and `--deb-tag` for .deb / AppImage).

Usage:
  .github/scripts/tag-release-assets.py --version 0.1.3 --dist dist/ubuntu-24.04-amd64
  .github/scripts/tag-release-assets.py --version 0.1.3 --arch amd64 --deb-tag ubuntu24.04
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
    "amd64": "amd64",
    "x86_64": "amd64",
    "x64": "amd64",
    "AMD64": "amd64",
    "arm64": "arm64",
    "aarch64": "arm64",
    "ARM64": "arm64",
}

SKIP_EXACT = {"latest.json", "PKGBUILD", ".SRCINFO"}
SKIP_PREFIXES = ("latest-", "PKGBUILD.")


def try_normalize_cpu(arch: str) -> str | None:
    key = arch.strip()
    return CPU_ALIASES.get(key) or CPU_ALIASES.get(key.lower())


def normalize_cpu(arch: str) -> str:
    mapped = try_normalize_cpu(arch)
    if mapped is None:
        raise SystemExit(
            f"unsupported arch {arch!r}; imprint packages amd64 and arm64 only"
        )
    return mapped


def parse_pack_dir(name: str) -> tuple[str, str] | None:
    """Return (system, cpu) for a pack directory name, or None if it is not one.

    `{system}[-{version}]-{arch}`:
      macos-arm64         → macos, arm64
      ubuntu-24.04-amd64  → ubuntu24.04, amd64
      windows-amd64       → windows, amd64
      archlinux-arm64     → archlinux, arm64
    rustc suffixes (x86_64 / aarch64) are accepted as aliases.
    """
    slug = Path(name).name
    system_raw, sep, arch_raw = slug.rpartition("-")
    if not sep or not system_raw or not arch_raw:
        return None
    cpu = try_normalize_cpu(arch_raw)
    if cpu is None:
        return None
    if system_raw in {"macos", "windows", "archlinux"}:
        return system_raw, cpu
    if system_raw.startswith("ubuntu-"):
        version = system_raw.removeprefix("ubuntu-")
        if not version or not any(ch.isdigit() for ch in version):
            return None
        return f"ubuntu{version}", cpu
    return None


def pack_dirs_to_process(dist: Path) -> list[Path]:
    """Prefer named pack dirs; fall back to treating dist/ as a flat folder."""
    if parse_pack_dir(dist.name):
        return [dist]
    children = sorted(
        path for path in dist.iterdir() if path.is_dir() and parse_pack_dir(path.name)
    )
    if children:
        return children
    return [dist]


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
        # AppImage is built on the same runner as the primary .deb (Ubuntu 24.04).
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
            "(ubuntu24.04 or ubuntu26.04); "
            "filenames would collide across distros"
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


def tag_one_dir(
    dist: Path,
    *,
    version: str,
    cpu: str,
    pack_system: str | None,
    deb_tag: str | None,
    dry_run: bool,
) -> list[tuple[Path, Path]]:
    planned: list[tuple[Path, Path]] = []
    dests: dict[Path, Path] = {}
    for path in sorted(dist.iterdir()):
        kind = classify(path)
        if kind is None:
            continue
        if pack_system is not None:
            system = pack_system
        else:
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


def tag_assets(
    dist: Path,
    *,
    version: str,
    arch: str | None = None,
    deb_tag: str | None = None,
    dry_run: bool = False,
) -> list[tuple[Path, Path]]:
    planned: list[tuple[Path, Path]] = []
    for pack in pack_dirs_to_process(dist):
        inferred = parse_pack_dir(pack.name)
        pack_system = inferred[0] if inferred else None
        inferred_cpu = inferred[1] if inferred else None
        if arch:
            cpu = normalize_cpu(arch)
            if inferred_cpu is not None and cpu != inferred_cpu:
                raise SystemExit(
                    f"--arch {arch} does not match pack dir {pack.name} ({inferred_cpu})"
                )
        elif inferred_cpu is not None:
            cpu = inferred_cpu
        else:
            raise SystemExit(
                "--arch is required when --dist is not a pack dir named like "
                "macos-arm64 or ubuntu-24.04-amd64"
            )
        if pack_system and deb_tag and pack_system != deb_tag:
            raise SystemExit(
                f"--deb-tag {deb_tag} does not match pack dir {pack.name} ({pack_system})"
            )
        planned.extend(
            tag_one_dir(
                pack,
                version=version,
                cpu=cpu,
                pack_system=pack_system,
                deb_tag=deb_tag,
                dry_run=dry_run,
            )
        )
    return planned


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rename pack-dir packages to imprint_{version}_{system}_{cpu}.*",
    )
    parser.add_argument("--version", help="Package version (required unless --self-test)")
    parser.add_argument(
        "--arch",
        help="CPU architecture of this job (amd64 or arm64; x86_64/aarch64/x64 aliases ok). "
        "Optional when --dist is a pack dir like macos-arm64 / ubuntu-24.04-amd64.",
    )
    parser.add_argument(
        "--deb-tag",
        help="OS tag for .deb files (ubuntu24.04 or ubuntu26.04). "
        "Optional when --dist is a ubuntu-<version>-<arch> pack dir.",
    )
    parser.add_argument(
        "--dist",
        type=Path,
        default=DEFAULT_DIST,
        help="Package directory (default: dist/). "
        "A pack dir (macos-arm64, ubuntu-24.04-amd64, ...) or a parent of those.",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print planned renames only")
    parser.add_argument("--self-test", action="store_true", help="Run built-in checks and exit")
    return parser.parse_args(argv)


def _touch(path: Path, body: str = "x") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)
    return path


def _expect_exit(fn) -> None:
    try:
        fn()
    except SystemExit:
        return
    raise SystemExit("expected SystemExit")


def _self_test() -> None:
    parsed = {
        "macos-arm64": ("macos", "arm64"),
        "macos-amd64": ("macos", "amd64"),
        "macos-aarch64": ("macos", "arm64"),
        "macos-x86_64": ("macos", "amd64"),
        "ubuntu-24.04-amd64": ("ubuntu24.04", "amd64"),
        "ubuntu-24.04-arm64": ("ubuntu24.04", "arm64"),
        "ubuntu-26.04-amd64": ("ubuntu26.04", "amd64"),
        "ubuntu-26.04-aarch64": ("ubuntu26.04", "arm64"),
        "windows-amd64": ("windows", "amd64"),
        "windows-arm64": ("windows", "arm64"),
        "windows-x86_64": ("windows", "amd64"),
        "windows-aarch64": ("windows", "arm64"),
        "archlinux-amd64": ("archlinux", "amd64"),
        "archlinux-arm64": ("archlinux", "arm64"),
        "archlinux-x86_64": ("archlinux", "amd64"),
        "archlinux-aarch64": ("archlinux", "arm64"),
    }
    for slug, want in parsed.items():
        got = parse_pack_dir(slug)
        if got != want:
            raise SystemExit(f"parse_pack_dir({slug!r}) -> {got}, want {want}")
    for slug in ("dist", "latest.json", "ubuntu", "macos-", "-amd64"):
        if parse_pack_dir(slug) is not None:
            raise SystemExit(f"parse_pack_dir({slug!r}) should be None")

    cases: list[tuple[str, str | None, list[str], list[str]]] = [
        (
            "x86_64",
            "ubuntu24.04",
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
                "imprint_0.1.3_ubuntu24.04_amd64.deb",
                "imprint_0.1.3_ubuntu24.04_amd64.deb.sig",
                "imprint_0.1.3_ubuntu24.04_amd64.AppImage",
                "imprint_0.1.3_archlinux_amd64.tar.gz",
                "imprint_0.1.3_archlinux_amd64.pkg.tar.zst",
                "PKGBUILD",
                "latest-linux-x86_64.json",
            ],
        ),
        (
            "aarch64",
            "ubuntu24.04",
            [
                "imprint_0.1.3_arm64.deb",
                "imprint_0.1.3_aarch64.AppImage",
                "imprint_0.1.3_aarch64.tar.gz",
                "imprint-0.1.3-1-aarch64.pkg.tar.zst",
            ],
            [
                "imprint_0.1.3_ubuntu24.04_arm64.deb",
                "imprint_0.1.3_ubuntu24.04_arm64.AppImage",
                "imprint_0.1.3_archlinux_arm64.tar.gz",
                "imprint_0.1.3_archlinux_arm64.pkg.tar.zst",
            ],
        ),
        (
            "amd64",
            "ubuntu26.04",
            ["imprint_0.1.3_amd64.deb"],
            ["imprint_0.1.3_ubuntu26.04_amd64.deb"],
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
            ["imprint_0.1.3_windows_amd64.msi"],
        ),
        (
            "arm64",
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
                raise SystemExit(
                    f"self-test failed arch={arch} deb_tag={deb_tag}\n  got:  {got}\n  want: {want}"
                )
    # Previous canonical x86_64 names migrate to amd64.
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_ubuntu24.04_x86_64.deb")
        tag_assets(dist, version="0.1.3", arch="x86_64", deb_tag="ubuntu24.04")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_ubuntu24.04_amd64.deb"]:
            raise SystemExit(f"x86_64 → amd64 migrate failed: {names}")
    # Idempotent when names are already canonical.
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_ubuntu24.04_amd64.deb")
        tag_assets(dist, version="0.1.3", arch="amd64", deb_tag="ubuntu24.04")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_ubuntu24.04_amd64.deb"]:
            raise SystemExit(f"idempotent rename failed: {names}")
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_ubuntu24.04_arm64.deb")
        tag_assets(dist, version="0.1.3", arch="arm64", deb_tag="ubuntu24.04")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_ubuntu24.04_arm64.deb"]:
            raise SystemExit(f"idempotent arm64 rename failed: {names}")
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / "imprint_0.1.3_amd64.deb")
        _expect_exit(lambda: tag_assets(dist, version="0.1.3", arch="x86_64"))

    # Pack dir infers system + cpu (no --arch / --deb-tag).
    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp) / "ubuntu-24.04-amd64"
        _touch(dist / "imprint_0.1.3_amd64.deb")
        _touch(dist / "imprint_0.1.3_x86_64.AppImage")
        tag_assets(dist, version="0.1.3")
        got = sorted(p.name for p in dist.iterdir())
        want = sorted(
            [
                "imprint_0.1.3_ubuntu24.04_amd64.deb",
                "imprint_0.1.3_ubuntu24.04_amd64.AppImage",
            ]
        )
        if got != want:
            raise SystemExit(f"pack-dir ubuntu infer failed\n  got:  {got}\n  want: {want}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp) / "macos-arm64"
        _touch(dist / "Imprint_0.1.3_aarch64.dmg")
        _touch(dist / "Imprint_0.1.3_aarch64.dmg.sig")
        tag_assets(dist, version="0.1.3")
        got = sorted(p.name for p in dist.iterdir())
        want = sorted(
            ["imprint_0.1.3_macos_arm64.dmg", "imprint_0.1.3_macos_arm64.dmg.sig"]
        )
        if got != want:
            raise SystemExit(f"pack-dir macos infer failed\n  got:  {got}\n  want: {want}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp) / "archlinux-amd64"
        _touch(dist / "imprint-0.1.3-1-x86_64.pkg.tar.zst")
        tag_assets(dist, version="0.1.3", arch="x86_64")
        names = [p.name for p in dist.iterdir()]
        if names != ["imprint_0.1.3_archlinux_amd64.pkg.tar.zst"]:
            raise SystemExit(f"pack-dir arch infer failed: {names}")

    # Parent dist/ with several pack dirs, no --arch.
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        _touch(root / "macos-arm64" / "Imprint_0.1.3_aarch64.dmg")
        _touch(root / "ubuntu-24.04-amd64" / "imprint_0.1.3_amd64.deb")
        _touch(root / "windows-amd64" / "imprint_0.1.3_x64_en-US.msi")
        tag_assets(root, version="0.1.3")
        got = sorted(
            str(p.relative_to(root))
            for p in root.rglob("*")
            if p.is_file()
        )
        want = sorted(
            [
                "macos-arm64/imprint_0.1.3_macos_arm64.dmg",
                "ubuntu-24.04-amd64/imprint_0.1.3_ubuntu24.04_amd64.deb",
                "windows-amd64/imprint_0.1.3_windows_amd64.msi",
            ]
        )
        if got != want:
            raise SystemExit(f"parent pack-dir walk failed\n  got:  {got}\n  want: {want}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp) / "macos-arm64"
        _touch(dist / "Imprint_0.1.3_aarch64.dmg")
        _expect_exit(lambda: tag_assets(dist, version="0.1.3", arch="x86_64"))

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp) / "ubuntu-24.04-amd64"
        _touch(dist / "imprint_0.1.3_amd64.deb")
        _expect_exit(
            lambda: tag_assets(dist, version="0.1.3", arch="x86_64", deb_tag="ubuntu26.04")
        )

    print("self-test ok")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        _self_test()
        return 0
    if not args.version:
        print("--version is required", file=sys.stderr)
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
