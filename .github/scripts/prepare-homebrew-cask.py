#!/usr/bin/env python3
"""Write a Homebrew cask (Casks/imprint.rb) for the macOS .dmg assets.

Expects disk images in dist/ (after .github/scripts/tag-release-assets.py):

  imprint_<version>_macos_arm64.dmg
  imprint_<version>_macos_x86_64.dmg

Falls back to cargo-packager names (Imprint_<version>_aarch64.dmg /
Imprint_<version>_x64.dmg) so an already-published release can still be
casked.

`--upload TAG` also uploads imprint.rb to that GitHub Release.

Usage:
  .github/scripts/prepare-homebrew-cask.py
  .github/scripts/prepare-homebrew-cask.py --version 0.1.3 --upload v0.1.3
  .github/scripts/prepare-homebrew-cask.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
CASK_PATH = ROOT / "Casks" / "imprint.rb"
GITHUB_REMOTE = re.compile(r"github\.com[:/](?P<repo>[^/]+/[^/.]+)(?:\.git)?$")

# Homebrew `arch` tokens (arm, intel) → filename CPU for the canonical scheme.
CANONICAL = {
    "arm": "arm64",
    "intel": "x86_64",
}
# cargo-packager macOS .dmg names before tag-release-assets.py.
LEGACY = {
    "arm": "aarch64",
    "intel": "x64",
}


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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def dmg_candidates(version: str, brew_arch: str) -> tuple[str, ...]:
    if brew_arch == "arm":
        names = [
            f"imprint_{version}_macos_arm64.dmg",
            f"Imprint_{version}_aarch64.dmg",
            f"Imprint_{version}_arm64.dmg",
            f"imprint_{version}_aarch64.dmg",
            f"imprint_{version}_arm64.dmg",
        ]
    else:
        names = [
            f"imprint_{version}_macos_x86_64.dmg",
            f"Imprint_{version}_x64.dmg",
            f"Imprint_{version}_x86_64.dmg",
            f"imprint_{version}_x64.dmg",
            f"imprint_{version}_x86_64.dmg",
        ]
    seen: list[str] = []
    for name in names:
        if name not in seen:
            seen.append(name)
    return tuple(seen)


def find_dmg(dist: Path, version: str, brew_arch: str) -> Path | None:
    for name in dmg_candidates(version, brew_arch):
        path = dist / name
        if path.is_file():
            return path
    return None


def require_dmgs(dist: Path, version: str) -> dict[str, Path]:
    found: dict[str, Path] = {}
    missing: list[str] = []
    for brew_arch in ("arm", "intel"):
        path = find_dmg(dist, version, brew_arch)
        if path is not None:
            found[brew_arch] = path
        else:
            missing.append(dmg_candidates(version, brew_arch)[0])
    if missing:
        raise SystemExit("missing macOS disk images in dist/: " + ", ".join(missing))
    return found


def ruby_string(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("#", "\\#")
    return f'"{escaped}"'


def url_scheme(dmgs: dict[str, Path], version: str) -> tuple[str, dict[str, str]] | None:
    """Return (`url` with `#{version}` / `#{arch}`, arch tokens) when both names match."""
    arm_name = dmgs["arm"].name
    intel_name = dmgs["intel"].name
    schemes = (
        (CANONICAL, f"imprint_{version}_macos_{{arch}}.dmg"),
        (LEGACY, f"Imprint_{version}_{{arch}}.dmg"),
    )
    for tokens, template in schemes:
        if arm_name == template.format(arch=tokens["arm"]) and intel_name == template.format(
            arch=tokens["intel"]
        ):
            url = template.replace(version, "#{version}").replace("{arch}", "#{arch}")
            return url, tokens
    return None


def render_cask(
    *,
    version: str,
    repo: str,
    dmgs: dict[str, Path],
) -> str:
    homepage = packager_field("homepage") or f"https://github.com/{repo}"
    description = packager_field("description") or "Flash OS images onto USB drives and SD cards"
    product = packager_field("product-name") or "Imprint"
    identifier = packager_field("identifier") or "imprint.cdxtheme.com"
    sha = {arch: sha256_file(path) for arch, path in dmgs.items()}
    scheme = url_scheme(dmgs, version)
    download = f"https://github.com/{repo}/releases/download/v#{{version}}"
    if scheme is not None:
        filename, tokens = scheme
        url_lines = f'  url "{download}/{filename}"'
        arch_lines = (
            f'  arch arm: {ruby_string(tokens["arm"])}, intel: {ruby_string(tokens["intel"])}'
        )
        version_sha = f"""  version "{version}"
  sha256 arm:   "{sha["arm"]}",
         intel: "{sha["intel"]}"
"""
        header = f"""cask "imprint" do
{arch_lines}

{version_sha}
{url_lines}
"""
    else:
        arm_file = dmgs["arm"].name.replace(version, "#{version}")
        intel_file = dmgs["intel"].name.replace(version, "#{version}")
        header = f"""cask "imprint" do
  on_arm do
    version "{version}"
    sha256 "{sha["arm"]}"

    url "{download}/{arm_file}"
  end
  on_intel do
    version "{version}"
    sha256 "{sha["intel"]}"

    url "{download}/{intel_file}"
  end
"""

    return f"""# Homebrew cask for https://github.com/{repo}
# Generated by .github/scripts/prepare-homebrew-cask.py
{header}  name {ruby_string(product)}
  desc {ruby_string(description)}
  homepage {ruby_string(homepage)}

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true

  app {ruby_string(product + ".app")}
  binary "#{{appdir}}/{product}.app/Contents/MacOS/imprint-cli", target: "imprint-cli"

  uninstall quit: {ruby_string(identifier)}

  zap trash: [
    "~/Library/Application Support/imprint",
    "~/Library/Caches/{identifier}",
    "~/Library/Logs/{identifier}",
    "~/Library/Preferences/{identifier}.plist",
    "~/Library/Saved Application State/{identifier}.savedState",
  ]
end
"""


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Write a Homebrew cask from macOS .dmg files in dist/.",
    )
    parser.add_argument("--version", help="Package version (default: Packager.toml)")
    parser.add_argument(
        "--upload",
        metavar="TAG",
        help="Upload imprint.rb to this GitHub Release",
    )
    parser.add_argument(
        "--dist",
        type=Path,
        default=DIST,
        help="Directory that contains the macOS .dmg files (default: dist/)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=CASK_PATH,
        help="Cask path (default: Casks/imprint.rb)",
    )
    parser.add_argument("--self-test", action="store_true", help="Run built-in checks and exit")
    return parser.parse_args(argv)


def _touch(path: Path, body: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)
    return path


def _self_test() -> None:
    repo = "googolmo/imprint"
    version = "0.1.3"
    arm_body = "arm-dmg"
    intel_body = "intel-dmg"
    arm_sha = hashlib.sha256(arm_body.encode()).hexdigest()
    intel_sha = hashlib.sha256(intel_body.encode()).hexdigest()

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / f"imprint_{version}_macos_arm64.dmg", arm_body)
        _touch(dist / f"imprint_{version}_macos_x86_64.dmg", intel_body)
        dmgs = require_dmgs(dist, version)
        cask = render_cask(version=version, repo=repo, dmgs=dmgs)
        for needle in (
            'cask "imprint" do',
            'arch arm: "arm64", intel: "x86_64"',
            f'version "{version}"',
            f'sha256 arm:   "{arm_sha}"',
            f'intel: "{intel_sha}"',
            "imprint_#{version}_macos_#{arch}.dmg",
            'app "Imprint.app"',
            'target: "imprint-cli"',
            'auto_updates true',
            'strategy :github_latest',
        ):
            if needle not in cask:
                raise SystemExit(f"canonical cask missing {needle!r}\n{cask}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / f"Imprint_{version}_aarch64.dmg", arm_body)
        _touch(dist / f"Imprint_{version}_x64.dmg", intel_body)
        dmgs = require_dmgs(dist, version)
        cask = render_cask(version=version, repo=repo, dmgs=dmgs)
        for needle in (
            'arch arm: "aarch64", intel: "x64"',
            "Imprint_#{version}_#{arch}.dmg",
            f'sha256 arm:   "{arm_sha}"',
        ):
            if needle not in cask:
                raise SystemExit(f"legacy cask missing {needle!r}\n{cask}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / f"Imprint_{version}_aarch64.dmg", arm_body)
        _touch(dist / f"imprint_{version}_macos_x86_64.dmg", intel_body)
        dmgs = require_dmgs(dist, version)
        cask = render_cask(version=version, repo=repo, dmgs=dmgs)
        if "on_arm do" not in cask or "on_intel do" not in cask:
            raise SystemExit(f"mixed names should use on_arm/on_intel\n{cask}")
        if "Imprint_#{version}_aarch64.dmg" not in cask:
            raise SystemExit(f"mixed cask missing arm filename\n{cask}")
        if "imprint_#{version}_macos_x86_64.dmg" not in cask:
            raise SystemExit(f"mixed cask missing intel filename\n{cask}")

    with tempfile.TemporaryDirectory() as tmp:
        dist = Path(tmp)
        _touch(dist / f"imprint_{version}_macos_arm64.dmg", arm_body)
        try:
            require_dmgs(dist, version)
        except SystemExit:
            pass
        else:
            raise SystemExit("expected SystemExit when the intel .dmg is missing")

    print("self-test ok")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        _self_test()
        return 0
    version = args.version or packager_version()
    repo = github_repo()
    dist: Path = args.dist
    if not dist.is_dir():
        print(f"missing package directory {dist}", file=sys.stderr)
        return 1
    dmgs = require_dmgs(dist, version)
    cask = render_cask(version=version, repo=repo, dmgs=dmgs)
    out: Path = args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(cask)
    print(f"wrote {out}")
    for brew_arch, path in dmgs.items():
        print(f"  {brew_arch}: {path.name} sha256:{sha256_file(path)}")

    if args.upload:
        subprocess.run(
            ["gh", "release", "upload", args.upload, str(out), "--clobber"],
            cwd=ROOT,
            check=True,
        )
        print(f"uploaded {out.name} to {args.upload}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
