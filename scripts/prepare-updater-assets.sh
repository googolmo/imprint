#!/usr/bin/env bash
# Build the cargo-packager-updater payload for this platform and a latest.json
# fragment that GitHub Releases can serve from IMPRINT_UPDATER_ENDPOINT:
#   https://github.com/<owner>/<repo>/releases/latest/download/latest.json
#
# Requires a packaged dist/ from `cargo packager --release`.
# Sign with CARGO_PACKAGER_SIGN_PRIVATE_KEY (and optional password).
#
# Usage:
#   scripts/prepare-updater-assets.sh [version] [notes]
#
# Then upload the signed asset, its .sig, and (after merging fragments from
# every OS job) dist/latest.json onto the GitHub release.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

version="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)}"
notes="${2:-}"
dist="${root}/dist"
mkdir -p "$dist"

arch="$(uname -m)"
case "$arch" in
  arm64 | aarch64) arch="aarch64" ;;
  x86_64 | amd64) arch="x86_64" ;;
esac

github_base="https://github.com/googolmo/imprint/releases/download/v${version}"
pub_date="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

sign() {
  local file="$1"
  if [[ -z "${CARGO_PACKAGER_SIGN_PRIVATE_KEY:-}" ]]; then
    echo "warning: CARGO_PACKAGER_SIGN_PRIVATE_KEY is unset; ${file}.sig will be missing" >&2
    return 0
  fi
  cargo packager signer sign "$file"
}

write_fragment() {
  local platform="$1"
  local format="$2"
  local filename="$3"
  local sigfile="${filename}.sig"
  local signature=""
  if [[ -f "${dist}/${sigfile}" ]]; then
    signature="$(cat "${dist}/${sigfile}")"
  fi
  python3 - "$dist" "$version" "$notes" "$pub_date" "$platform" "$format" "$github_base/$filename" "$signature" <<'PY'
import json, sys
from pathlib import Path

dist, version, notes, pub_date, platform, fmt, url, signature = sys.argv[1:]
fragment = {
    "version": version,
    "notes": notes,
    "pub_date": pub_date,
    "platforms": {
        platform: {
            "signature": signature,
            "url": url,
            "format": fmt,
        }
    },
}
path = Path(dist) / f"latest-{platform}.json"
path.write_text(json.dumps(fragment, indent=2) + "\n")
print(f"wrote {path}")
PY
}

merge_latest() {
  python3 - "$dist" "$version" "$notes" "$pub_date" <<'PY'
import json, sys
from pathlib import Path

dist = Path(sys.argv[1])
version, notes, pub_date = sys.argv[2:]
platforms = {}
for path in sorted(dist.glob("latest-*.json")):
    if path.name == "latest.json":
        continue
    data = json.loads(path.read_text())
    platforms.update(data.get("platforms") or {})
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": pub_date,
    "platforms": platforms,
}
out = dist / "latest.json"
out.write_text(json.dumps(manifest, indent=2) + "\n")
print(f"wrote {out} ({len(platforms)} platform(s))")
PY
}

os="$(uname -s)"
case "$os" in
  Darwin)
    app="${dist}/Imprint.app"
    if [[ ! -d "$app" ]]; then
      echo "missing ${app}; run cargo packager --release first" >&2
      exit 1
    fi
    asset="Imprint_${arch}.app.tar.gz"
    COPYFILE_DISABLE=1 tar -C "$dist" -czf "${dist}/${asset}" Imprint.app
    sign "${dist}/${asset}"
    write_fragment "macos-${arch}" "app" "$asset"
    ;;
  Linux)
    shopt -s nullglob
    images=("${dist}"/*.AppImage)
    if [[ ${#images[@]} -eq 0 ]]; then
      echo "missing AppImage in ${dist}; run cargo packager --release first" >&2
      exit 1
    fi
    asset="$(basename "${images[0]}")"
    sign "${dist}/${asset}"
    write_fragment "linux-${arch}" "appimage" "$asset"
    ;;
  MINGW* | MSYS* | CYGWIN* | Windows_NT)
    shopt -s nullglob
    msi=("${dist}"/*.msi)
    nsis=("${dist}"/*setup.exe "${dist}"/*.exe)
    if [[ ${#msi[@]} -gt 0 ]]; then
      asset="$(basename "${msi[0]}")"
      sign "${dist}/${asset}"
      write_fragment "windows-${arch}" "wix" "$asset"
    elif [[ ${#nsis[@]} -gt 0 ]]; then
      asset="$(basename "${nsis[0]}")"
      sign "${dist}/${asset}"
      write_fragment "windows-${arch}" "nsis" "$asset"
    else
      echo "missing WiX/NSIS installer in ${dist}" >&2
      exit 1
    fi
    ;;
  *)
    echo "unsupported OS: $os" >&2
    exit 1
    ;;
esac

merge_latest
