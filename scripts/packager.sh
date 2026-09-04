#!/usr/bin/env bash
# Load signing secrets from the given files and run cargo-packager.
#
# --apple-certificate accepts a raw .p12 or a .p12.base64 / .base64 file
# (openssl/base64 text; wrapping whitespace is stripped). Extra args after
# -- are forwarded to cargo-packager.
#
# Usage:
#   scripts/packager.sh \
#     --apple-certificate /path/to/cert.p12.base64 \
#     --apple-certificate-password /path/to/cert.password \
#     --apple-signing-identity /path/to/identity \
#     --sign-private-key /path/to/minisign.key \
#     --sign-private-key-password /path/to/minisign.password \
#     -- --formats app,dmg

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

apple_certificate_file=""
apple_certificate_password_file=""
apple_signing_identity_file=""
sign_private_key_file=""
sign_private_key_password_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apple-certificate)
      apple_certificate_file="${2:?--apple-certificate requires a path}"
      shift 2
      ;;
    --apple-certificate-password)
      apple_certificate_password_file="${2:?--apple-certificate-password requires a path}"
      shift 2
      ;;
    --apple-signing-identity)
      apple_signing_identity_file="${2:?--apple-signing-identity requires a path}"
      shift 2
      ;;
    --sign-private-key)
      sign_private_key_file="${2:?--sign-private-key requires a path}"
      shift 2
      ;;
    --sign-private-key-password)
      sign_private_key_password_file="${2:?--sign-private-key-password requires a path}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_file() {
  local flag="$1"
  local file="$2"
  if [[ -z "$file" ]]; then
    printf 'missing %s\n' "$flag" >&2
    usage >&2
    exit 2
  fi
  if [[ ! -f "$file" ]]; then
    printf '%s is not a file: %s\n' "$flag" "$file" >&2
    exit 1
  fi
}

require_file --apple-certificate "$apple_certificate_file"
require_file --apple-certificate-password "$apple_certificate_password_file"
require_file --apple-signing-identity "$apple_signing_identity_file"
require_file --sign-private-key "$sign_private_key_file"
require_file --sign-private-key-password "$sign_private_key_password_file"

# Command substitution strips trailing newlines (wanted for these secrets).
read_secret_file() {
  cat -- "$1"
}

read_base64_file() {
  # cargo-packager's APPLE_CERTIFICATE is decoded as standard base64, which
  # rejects wrapped lines. Collapse openssl/base64 text to one line.
  tr -d '[:space:]' <"$1"
}

is_base64_text() {
  LC_ALL=C awk '
    BEGIN { nonempty = 0 }
    NF { nonempty = 1 }
    /[^A-Za-z0-9+/=\r\n \t]/ { exit 1 }
    END { exit !nonempty }
  ' "$1"
}

load_apple_certificate() {
  local file="$1"
  local lower
  lower="$(printf '%s' "$file" | tr '[:upper:]' '[:lower:]')"
  case "$lower" in
    *.p12.base64 | *.base64)
      APPLE_CERTIFICATE="$(read_base64_file "$file")"
      ;;
    *.p12 | *.pfx)
      APPLE_CERTIFICATE="$(openssl base64 -A -in "$file")"
      ;;
    *)
      if is_base64_text "$file"; then
        APPLE_CERTIFICATE="$(read_base64_file "$file")"
      else
        APPLE_CERTIFICATE="$(openssl base64 -A -in "$file")"
      fi
      ;;
  esac
  if [[ -z "$APPLE_CERTIFICATE" ]]; then
    printf 'APPLE_CERTIFICATE is empty after reading %s\n' "$file" >&2
    exit 1
  fi
  export APPLE_CERTIFICATE
}

load_apple_certificate "$apple_certificate_file"

export APPLE_CERTIFICATE_PASSWORD
APPLE_CERTIFICATE_PASSWORD="$(read_secret_file "$apple_certificate_password_file")"
export APPLE_SIGNING_IDENTITY
APPLE_SIGNING_IDENTITY="$(read_secret_file "$apple_signing_identity_file")"
export CARGO_PACKAGER_SIGN_PRIVATE_KEY
CARGO_PACKAGER_SIGN_PRIVATE_KEY="$(read_secret_file "$sign_private_key_file")"
export CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD
CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD="$(read_secret_file "$sign_private_key_password_file")"

packager_toml="$root/Packager.toml"
packager_backup=""
codesign_wrap=""
cleanup_packager() {
  if [[ -n "$packager_backup" && -f "$packager_backup" ]]; then
    mv "$packager_backup" "$packager_toml"
    packager_backup=""
  fi
  if [[ -n "$codesign_wrap" && -d "$codesign_wrap" ]]; then
    rm -rf "$codesign_wrap"
    codesign_wrap=""
  fi
}
trap cleanup_packager EXIT

packager_backup="$(mktemp)"
cp "$packager_toml" "$packager_backup"
python3 "$root/.github/scripts/inject-packager-signing.py" "$packager_toml"

# cargo-packager signs Contents/MacOS binaries by path depth only, so a
# sidecar (imprint-cli) can be signed after the main exe. Prepend our
# codesign wrapper (same as CI). Also wrap hdiutil so create-dmg can
# force-detach when Spotlight holds the RW image (Intel CI flake).
codesign_wrap="$(mktemp -d "${TMPDIR:-/tmp}/imprint-codesign.XXXXXX")"
cp "$root/.github/scripts/codesign" "$codesign_wrap/codesign"
cp "$root/.github/scripts/hdiutil" "$codesign_wrap/hdiutil"
chmod +x "$codesign_wrap/codesign" "$codesign_wrap/hdiutil"
export PATH="$codesign_wrap:$PATH"

# Packager.toml lives at the workspace root (name = "imprint").
# cargo-packager --packages matches that name, not a crate path, so we
# package from the repo root with --release (same as CI). imprint-app is
# built by before-packaging-command. Do not exec: the EXIT trap must
# restore Packager.toml after inject-packager-signing.py.
cargo packager -r \
  -k "${CARGO_PACKAGER_SIGN_PRIVATE_KEY}" \
  --password "${CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD}" \
  "$@"
