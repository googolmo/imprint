#!/usr/bin/env bash
# Build a real Arch Linux package (.pkg.tar.zst) with makepkg in Docker.
#
# cargo-packager's `pacman` format emits a usr/ tarball plus a PKGBUILD, but
# that tarball is not a pacman package. This script wraps the tarball with
# makepkg inside an Arch container (official image on x86_64; Arch Linux ARM
# image on aarch64 — the official image is amd64-only).
#
# Usage:
#   .github/scripts/build-arch-package.sh [--version 0.1.1] [--arch x86_64|aarch64]
#
# Expects dist/imprint_<version>_<arch>.tar.gz from `cargo packager --formats pacman`.
# Writes dist/imprint-<version>-1-<arch>.pkg.tar.zst

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

version=""
arch=""

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      version="${2:?--version requires a value}"
      shift 2
      ;;
    --arch)
      arch="${2:?--arch requires a value}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

packager_field() {
  local key="$1"
  sed -n "s/^${key} = \"\\(.*\\)\"/\\1/p" Packager.toml | head -n1
}

normalize_arch() {
  case "$1" in
    x86_64 | amd64) printf 'x86_64\n' ;;
    aarch64 | arm64) printf 'aarch64\n' ;;
    x86 | i386 | i686)
      printf 'unsupported arch: %s (32-bit x86 is not packaged)\n' "$1" >&2
      exit 1
      ;;
    *)
      printf 'unsupported arch: %s (only aarch64 and x86_64)\n' "$1" >&2
      exit 1
      ;;
  esac
}

if [[ -z "$version" ]]; then
  version="$(packager_field version)"
fi
if [[ -z "$version" ]]; then
  printf 'could not read version from Packager.toml\n' >&2
  exit 1
fi

if [[ -z "$arch" ]]; then
  arch="$(uname -m)"
fi
arch="$(normalize_arch "$arch")"

if ! command -v docker >/dev/null 2>&1; then
  printf 'docker is required to build the Arch Linux package\n' >&2
  exit 1
fi

archive="dist/imprint_${version}_${arch}.tar.gz"
if [[ ! -f "$archive" ]]; then
  printf 'missing %s; run cargo packager --formats pacman first\n' "$archive" >&2
  exit 1
fi

description="$(packager_field description)"
homepage="$(packager_field homepage)"
if [[ -z "$description" ]]; then
  description="Flash OS images onto USB drives and SD cards"
fi
if [[ -z "$homepage" ]]; then
  homepage="https://github.com/googolmo/imprint"
fi
# PKGBUILD is bash; keep pkgdesc/url as single-quoted strings.
quote_pkgbuild() {
  local value="$1"
  value="${value//\'/\'\\\'\'}"
  printf "'%s'" "$value"
}

# Keep in sync with Packager.toml [pacman].depends
depends=(
  alsa-lib
  fontconfig
  freetype2
  libx11
  libxkbcommon
  libxkbcommon-x11
  mesa
  vulkan-icd-loader
  wayland
)
depends_quoted=""
for dep in "${depends[@]}"; do
  depends_quoted+="'${dep}' "
done

sum="$(sha256sum -- "$archive" | awk '{print $1}')"
archive_name="$(basename "$archive")"

case "$arch" in
  x86_64)
    # Official Arch image (amd64 only).
    image="${ARCH_DOCKER_IMAGE_X86_64:-archlinux:base-devel}"
    ;;
  aarch64)
    # Official archlinux image has no arm64 tag.
    image="${ARCH_DOCKER_IMAGE_AARCH64:-menci/archlinuxarm:base-devel}"
    ;;
esac

# Bind-mount under the repo so Docker Desktop always shares it (`/tmp` can be
# a separate VM dir). mktemp -d is 0700; if the container leaves it owned by
# another uid the host cannot list, copy, or delete it.
mkdir -p "$root/dist"
work="$(mktemp -d "$root/dist/.arch-pkg.XXXXXX")"
cleanup() {
  if [[ ! -e "$work" ]]; then
    return 0
  fi
  rm -rf "$work" 2>/dev/null && return 0
  docker run --rm --user 0 --volume "$work:/pkg" "$image" \
    bash -lc 'chmod -R a+rwX /pkg' 2>/dev/null || true
  rm -rf "$work" 2>/dev/null || true
}
trap cleanup EXIT

cp -- "$archive" "$work/$archive_name"

# Unquoted heredoc: version/checksums expand here. Keep $srcdir/$pkgdir literal.
cat >"$work/PKGBUILD" <<EOF
# Maintainer: Imprint Contributors
pkgname=imprint
pkgver=${version}
pkgrel=1
pkgdesc=$(quote_pkgbuild "$description")
url=$(quote_pkgbuild "$homepage")
arch=('${arch}')
license=('Apache-2.0')
depends=(${depends_quoted})
provides=('imprint')
conflicts=('imprint')
options=('!debug' '!lto' '!strip')
source=('${archive_name}')
sha256sums=('${sum}')

package() {
  cp -a "\${srcdir}/usr" "\${pkgdir}/usr"
}
EOF

printf 'building Arch package in %s (%s)\n' "$image" "$arch"

docker pull "$image"
# makepkg refuses root. Use the host uid so the bind mount stays readable
# after the container exits, then chown back in case useradd could not match.
docker run --rm \
  --volume "$work:/pkg" \
  --workdir /pkg \
  --env HOST_UID="$(id -u)" \
  --env HOST_GID="$(id -g)" \
  "$image" \
  bash -lc '
set -euo pipefail
if ! command -v makepkg >/dev/null 2>&1; then
  if command -v pacman-key >/dev/null 2>&1; then
    pacman-key --init
    pacman-key --populate archlinux 2>/dev/null || \
      pacman-key --populate archlinuxarm 2>/dev/null || true
  fi
  pacman -Sy --noconfirm --needed base-devel
fi
if [[ "$(id -u)" -eq 0 ]]; then
  if [[ "${HOST_UID}" -ne 0 ]]; then
    if ! getent group "${HOST_GID}" >/dev/null 2>&1; then
      groupadd --gid "${HOST_GID}" builder
    fi
    if ! getent passwd "${HOST_UID}" >/dev/null 2>&1; then
      useradd --create-home --uid "${HOST_UID}" --gid "${HOST_GID}" \
        --shell /bin/bash builder
    fi
    run_as="$(getent passwd "${HOST_UID}" | cut -d: -f1)"
  else
    if ! id builder >/dev/null 2>&1; then
      useradd --create-home --user-group --shell /bin/bash builder
    fi
    run_as=builder
  fi
  chown -R "${run_as}" /pkg
  su -s /bin/bash "${run_as}" -c \
    "cd /pkg && PKGDEST=/pkg makepkg -f --nodeps --noconfirm"
  chown -R "${HOST_UID}:${HOST_GID}" /pkg
else
  PKGDEST=/pkg makepkg -f --nodeps --noconfirm
fi
'

shopt -s nullglob
packages=("$work"/imprint-*.pkg.tar.*)
if [[ ${#packages[@]} -eq 0 ]]; then
  printf 'makepkg produced no imprint-*.pkg.tar.* in %s\n' "$work" >&2
  ls -la "$work" >&2 || true
  exit 1
fi

mkdir -p dist
for pkg in "${packages[@]}"; do
  case "$pkg" in
    *.sig) continue ;;
  esac
  dest="dist/$(basename "$pkg")"
  cp -- "$pkg" "$dest"
  printf 'wrote %s\n' "$dest"
  if [[ -n "${CARGO_PACKAGER_SIGN_PRIVATE_KEY:-}" ]] && command -v cargo >/dev/null 2>&1; then
    cargo packager signer sign "$dest" || \
      printf 'warning: failed to sign %s\n' "$dest" >&2
  fi
done
