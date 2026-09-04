#!/usr/bin/env bash
# Compile Imprint and wrap it with makepkg inside Arch Linux Docker.
#
# The GitHub runner is Ubuntu (Docker host only). cargo build and makepkg
# both run in an Arch container so the binary links against Arch libraries,
# not Ubuntu glibc. Official image on x86_64; Arch Linux ARM image on
# aarch64 (the official image is amd64-only).
#
# Usage:
#   .github/scripts/build-arch-package.sh [--version 0.1.4] [--arch x86_64|aarch64]
#       [--out-dir dist/archlinux-amd64]
#
# Writes <out-dir>/imprint-<version>-1-<arch>.pkg.tar.zst
# (default out-dir: dist/archlinux-amd64 or dist/archlinux-arm64).
# PKGBUILD arch=() stays Arch's x86_64 / aarch64.
#
# Internal (invoked by this script inside the container):
#   --in-container   root: pacman, builder user, then --as-builder
#   --as-builder     rustup + cargo build --release + makepkg

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

version=""
arch=""
out_dir=""
in_container=0
as_builder=0

usage() {
  sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'
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
    --out-dir)
      out_dir="${2:?--out-dir requires a value}"
      shift 2
      ;;
    --in-container)
      in_container=1
      shift
      ;;
    --as-builder)
      as_builder=1
      shift
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

quote_pkgbuild() {
  local value="$1"
  value="${value//\'/\'\\\'\'}"
  printf "'%s'" "$value"
}

quote_desktop() {
  # Desktop Entry values: escape \ and newlines. Keep as a single line.
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//$'\n'/\\n}"
  printf '%s' "$value"
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
if [[ -z "$out_dir" ]]; then
  case "$arch" in
    x86_64) out_dir="dist/archlinux-amd64" ;;
    aarch64) out_dir="dist/archlinux-arm64" ;;
  esac
fi

arch_image() {
  case "$arch" in
    x86_64)
      printf '%s\n' "${ARCH_DOCKER_IMAGE_X86_64:-archlinux:base-devel}"
      ;;
    aarch64)
      printf '%s\n' "${ARCH_DOCKER_IMAGE_AARCH64:-menci/archlinuxarm:base-devel}"
      ;;
  esac
}

disable_pacman_sandbox() {
  # Pacman 7+ sandboxes downloads with Landlock and a drop to user `alpm`.
  # Official archlinux:base-devel already ships DisableSandbox
  # (archlinux-docker#103). menci/archlinuxarm does not, and GitHub ARM
  # Docker rejects Landlock ("Operation not permitted") plus the alpm switch.
  local conf=/etc/pacman.conf
  [[ -f "$conf" ]] || return 0
  sed -i -E \
    's/^[[:space:]]*#[[:space:]]*(DisableSandbox(Filesystem|Syscalls)?)\b/\1/' \
    "$conf"
  if ! grep -qE '^[[:space:]]*DisableSandbox' "$conf"; then
    sed -i '/^\[options\]/a DisableSandbox' "$conf"
  fi
  sed -i -E 's/^[[:space:]]*DownloadUser\b/#DownloadUser/' "$conf"
}

setup_pacman() {
  disable_pacman_sandbox
  if command -v pacman-key >/dev/null 2>&1; then
    pacman-key --init
    pacman-key --populate archlinux 2>/dev/null || \
      pacman-key --populate archlinuxarm 2>/dev/null || true
  fi
  pacman -Sy --noconfirm --needed archlinux-keyring 2>/dev/null || \
    pacman -Sy --noconfirm --needed archlinuxarm-keyring 2>/dev/null || true
}

install_build_packages() {
  local pkgs=(
    base-devel
    ca-certificates
    clang
    cmake
    curl
    git
    pkgconf
    alsa-lib
    fontconfig
    freetype2
    libx11
    libxcb
    libxkbcommon
    libxkbcommon-x11
    libxrandr
    libxi
    libxinerama
    libxcursor
    libxcomposite
    libxdamage
    libxext
    libxfixes
    mesa
    openssl
    vulkan-headers
    vulkan-icd-loader
    wayland
    zstd
  )
  pacman -Syu --noconfirm --needed "${pkgs[@]}"
}

resolve_builder() {
  # makepkg and rustup refuse root. Match the host uid so bind mounts stay
  # readable after the container exits.
  if [[ "$(id -u)" -ne 0 ]]; then
    printf '%s\n' "$(id -un)"
    return 0
  fi
  local host_uid="${HOST_UID:-0}"
  local host_gid="${HOST_GID:-0}"
  if [[ "$host_uid" -ne 0 ]]; then
    if ! getent group "${host_gid}" >/dev/null 2>&1; then
      groupadd --gid "${host_gid}" builder
    fi
    if ! getent passwd "${host_uid}" >/dev/null 2>&1; then
      useradd --create-home --uid "${host_uid}" --gid "${host_gid}" \
        --shell /bin/bash builder
    fi
    getent passwd "${host_uid}" | cut -d: -f1
    return 0
  fi
  if ! id builder >/dev/null 2>&1; then
    useradd --create-home --user-group --shell /bin/bash builder
  fi
  printf 'builder\n'
}

run_as_user() {
  local user="$1"
  shift
  if command -v runuser >/dev/null 2>&1; then
    runuser -u "$user" -- "$@"
    return
  fi
  local cmd=""
  printf -v cmd '%q ' "$@"
  su -s /bin/bash "$user" -c "$cmd"
}

as_builder_main() {
  if [[ "$(id -u)" -eq 0 ]]; then
    printf '--as-builder must not run as root\n' >&2
    exit 1
  fi

  export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
  export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
  mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"
  export PATH="${CARGO_HOME}/bin:${PATH}"

  if ! command -v rustup >/dev/null 2>&1; then
    printf 'installing rustup (toolchain 1.98.0)\n'
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
      sh -s -- -y --no-modify-path --profile minimal --default-toolchain 1.98.0
  fi
  if ! command -v rustup >/dev/null 2>&1; then
    printf 'rustup not on PATH after install (%s)\n' "$CARGO_HOME/bin" >&2
    exit 1
  fi
  rustup toolchain install 1.98.0 --profile minimal --no-self-update
  rustup show

  printf 'cargo build --release --locked (Arch %s)\n' "$arch"
  cargo build --release --locked -p imprint-app -p imprint-cli

  local bin_app="target/release/imprint"
  local bin_cli="target/release/imprint-cli"
  if [[ ! -x "$bin_app" || ! -x "$bin_cli" ]]; then
    printf 'missing release binaries: %s %s\n' "$bin_app" "$bin_cli" >&2
    ls -la target/release >&2 || true
    exit 1
  fi

  local description homepage
  description="$(packager_field description)"
  homepage="$(packager_field homepage)"
  if [[ -z "$description" ]]; then
    description="Flash OS images onto USB drives and SD cards"
  fi
  if [[ -z "$homepage" ]]; then
    homepage="https://github.com/googolmo/imprint"
  fi

  mkdir -p dist "$out_dir"
  local work
  work="$(mktemp -d "$root/dist/.arch-pkg.XXXXXX")"
  cp -- "$bin_app" "$work/imprint"
  cp -- "$bin_cli" "$work/imprint-cli"
  chmod 755 "$work/imprint" "$work/imprint-cli"
  cp -- assets/icon/AppIcon.png "$work/imprint.png"
  cp -- LICENSE "$work/LICENSE"

  cat >"$work/imprint.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Imprint
Comment=$(quote_desktop "$description")
Exec=imprint
Icon=imprint
Terminal=false
Categories=Utility;
StartupNotify=true
EOF

  local depends_quoted=""
  local dep
  for dep in "${depends[@]}"; do
    depends_quoted+="'${dep}' "
  done

  # Unquoted heredoc: version expands here. Keep $srcdir/$pkgdir literal.
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
source=(
  'imprint'
  'imprint-cli'
  'imprint.desktop'
  'imprint.png'
  'LICENSE'
)
sha256sums=('SKIP' 'SKIP' 'SKIP' 'SKIP' 'SKIP')

package() {
  install -Dm755 "\${srcdir}/imprint" "\${pkgdir}/usr/bin/imprint"
  install -Dm755 "\${srcdir}/imprint-cli" "\${pkgdir}/usr/bin/imprint-cli"
  install -Dm644 "\${srcdir}/imprint.desktop" \
    "\${pkgdir}/usr/share/applications/imprint.desktop"
  install -Dm644 "\${srcdir}/imprint.png" \
    "\${pkgdir}/usr/share/icons/hicolor/1024x1024/apps/imprint.png"
  install -Dm644 "\${srcdir}/LICENSE" \
    "\${pkgdir}/usr/share/licenses/\${pkgname}/LICENSE"
}
EOF

  printf 'makepkg in %s\n' "$work"
  (cd "$work" && PKGDEST="$work" makepkg -f --noconfirm)

  shopt -s nullglob
  local packages=("$work"/imprint-*.pkg.tar.*)
  if [[ ${#packages[@]} -eq 0 ]]; then
    printf 'makepkg produced no imprint-*.pkg.tar.* in %s\n' "$work" >&2
    ls -la "$work" >&2 || true
    exit 1
  fi

  local pkg dest
  for pkg in "${packages[@]}"; do
    case "$pkg" in
      *.sig) continue ;;
    esac
    dest="${out_dir}/$(basename "$pkg")"
    cp -- "$pkg" "$dest"
    printf 'wrote %s\n' "$dest"
  done
  rm -rf "$work"
}

in_container_main() {
  if [[ "$(id -u)" -ne 0 ]]; then
    printf '--in-container expected to start as root (pacman)\n' >&2
    exit 1
  fi

  local got
  got="$(normalize_arch "$(uname -m)")"
  if [[ "$got" != "$arch" ]]; then
    printf 'container uname -m %s (normalized %s) does not match --arch %s\n' \
      "$(uname -m)" "$got" "$arch" >&2
    exit 1
  fi

  export CARGO_HOME="${CARGO_HOME:-/cargo-home}"
  export RUSTUP_HOME="${RUSTUP_HOME:-/rustup-home}"

  setup_pacman
  install_build_packages

  git config --system --add safe.directory /src
  git config --system --add safe.directory '*'

  local builder
  builder="$(resolve_builder)"

  mkdir -p "$CARGO_HOME" "$RUSTUP_HOME" /src/target /src/dist "$out_dir"
  chown -R "$builder" "$CARGO_HOME" "$RUSTUP_HOME" /src/target /src/dist "$out_dir"

  export PATH="${CARGO_HOME}/bin:/usr/local/bin:/usr/bin:/bin"
  export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
  export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
  export CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-true}"
  export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

  printf 'building as %s (uid %s)\n' "$builder" "$(id -u "$builder")"
  run_as_user "$builder" env \
    CARGO_HOME="$CARGO_HOME" \
    RUSTUP_HOME="$RUSTUP_HOME" \
    PATH="$PATH" \
    CARGO_TERM_COLOR="$CARGO_TERM_COLOR" \
    CARGO_INCREMENTAL="$CARGO_INCREMENTAL" \
    CARGO_NET_GIT_FETCH_WITH_CLI="$CARGO_NET_GIT_FETCH_WITH_CLI" \
    CARGO_PACKAGER_UPDATER_PUBKEY="${CARGO_PACKAGER_UPDATER_PUBKEY:-}" \
    CARGO_PACKAGER_UPDATER_ENDPOINT="${CARGO_PACKAGER_UPDATER_ENDPOINT:-}" \
    RUST_BACKTRACE="$RUST_BACKTRACE" \
    "$0" --as-builder --version "$version" --arch "$arch" --out-dir "$out_dir"

  chown -R "${HOST_UID:-0}:${HOST_GID:-0}" /src/dist /src/target \
    "$CARGO_HOME" "$RUSTUP_HOME"
}

host_main() {
  if ! command -v docker >/dev/null 2>&1; then
    printf 'docker is required to build the Arch Linux package\n' >&2
    exit 1
  fi

  local image
  image="$(arch_image)"

  local host_cargo="${CARGO_HOME:-$HOME/.cargo}"
  local host_rustup="${RUSTUP_HOME:-$HOME/.rustup}"
  mkdir -p "$host_cargo" "$host_rustup" "$root/target" "$root/dist" "$out_dir"

  cleanup() {
    local leftover img
    img="$(arch_image)"
    shopt -s nullglob
    for leftover in "$root"/dist/.arch-pkg.*; do
      rm -rf "$leftover" 2>/dev/null && continue
      docker run --rm --user 0 --volume "$leftover:/pkg" "$img" \
        bash -lc 'chmod -R a+rwX /pkg' 2>/dev/null || true
      rm -rf "$leftover" 2>/dev/null || true
    done
  }
  trap cleanup EXIT

  printf 'building Arch package in %s (%s)\n' "$image" "$arch"
  docker pull "$image"
  docker run --rm \
    --user 0 \
    --volume "$root:/src" \
    --volume "$host_cargo:/cargo-home" \
    --volume "$host_rustup:/rustup-home" \
    --workdir /src \
    --env HOST_UID="$(id -u)" \
    --env HOST_GID="$(id -g)" \
    --env CARGO_HOME=/cargo-home \
    --env RUSTUP_HOME=/rustup-home \
    --env CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}" \
    --env CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}" \
    --env CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-true}" \
    --env CARGO_PACKAGER_UPDATER_PUBKEY="${CARGO_PACKAGER_UPDATER_PUBKEY:-}" \
    --env CARGO_PACKAGER_UPDATER_ENDPOINT="${CARGO_PACKAGER_UPDATER_ENDPOINT:-}" \
    --env RUST_BACKTRACE="${RUST_BACKTRACE:-1}" \
    "$image" \
    /src/.github/scripts/build-arch-package.sh \
      --in-container --version "$version" --arch "$arch" --out-dir "$out_dir"

  shopt -s nullglob
  local packages=("$out_dir"/imprint-*.pkg.tar.*)
  if [[ ${#packages[@]} -eq 0 ]]; then
    printf 'no imprint-*.pkg.tar.* in %s after Arch Docker build\n' "$out_dir" >&2
    ls -la "$out_dir" >&2 || true
    ls -la dist/ >&2 || true
    exit 1
  fi
  local pkg
  for pkg in "${packages[@]}"; do
    printf 'arch package: %s\n' "$pkg"
  done
}

if [[ "$as_builder" -eq 1 ]]; then
  as_builder_main
elif [[ "$in_container" -eq 1 ]]; then
  in_container_main
else
  host_main
fi
