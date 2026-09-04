---
name: imprint-release
description: >
  Imprint GitHub Release packaging: cargo-packager, Arch Docker, Homebrew tap,
  linux-repo index, asset names, and workflow job graph. Use when changing
  .github/workflows/release.yml, release-package.yml, ci.yml packaging jobs,
  .github/scripts/*, Packager.toml formats, or when the user mentions release
  assets, Homebrew cask, pacman, AppImage, latest.json, or /imprint-release.
---

# Imprint release packaging

Canonical files: `.github/workflows/release.yml` (caller), `release-package.yml`
(pack), `.github/scripts/*`, `Packager.toml`.

## Job graph

GitHub `needs` waits for **every matrix cell** of a job. `fail-fast: false`
does not change that. Do not put optional/preview packs in the same matrix as
a job that `linux-repo` or Homebrew `needs`.

Current split:

- `package-macos` → `homebrew` (tap googolmo/homebrew-tap, not this repo)
- `package-linux` = Ubuntu 24.04 (`.deb` + AppImage) + Arch Docker → `linux-repo`
- `package-linux-preview` = Ubuntu 26.04 `.deb` only → `linux-repo-preview`
- `package-windows` is independent

`linux-repo` must keep running when 26.04 fails. Preview may re-dispatch
`update-index` after both stable and preview packs succeed.

Release concurrency: `cancel-in-progress: false`. Cancelling mid-upload or
mid-`latest.json` merge leaves a partial release.

## What we ship

cargo-packager `--out-dir dist/<pack_dir>/`. `pack_dir` is
`{system}[-{version}]-{arch}`:

- `macos-arm64` / `macos-amd64`
- `ubuntu-24.04-amd64` / `ubuntu-24.04-arm64`
- `ubuntu-26.04-amd64` / `ubuntu-26.04-arm64`
- `windows-amd64` / `windows-arm64`
- `archlinux-amd64` / `archlinux-arm64` (makepkg, not cargo-packager)

`.github/scripts/tag-release-assets.py` then renames files in that dir to
`imprint_{version}_{system}_{cpu}{suffix}`. `cpu` is `amd64` or `arm64`.
`system` is `ubuntu24.04` | `ubuntu26.04` | `macos` | `windows` | `archlinux`.
GitHub Release assets are the basename (flat); the pack dir is local only.
rustc triples and cargo-packager-updater `platforms` keys stay `x86_64` /
`aarch64`. Arch PKGBUILD `arch=()` stays `x86_64` / `aarch64`.

Do not reintroduce Ubuntu 22.04 runners or `ubuntu22.04` asset tags. Stable
`.deb` is glibc 2.39 (Ubuntu 24.04 / Debian 13+).

AppImage is built on the Ubuntu 24.04 pack job (`formats: deb,appimage`,
`skip_updater` unset). 24.04+ needs `libfuse2t64` (not `libfuse2`) plus
`APPIMAGE_EXTRACT_AND_RUN=1`. Deb/pacman installs are not in-app-updatable.

Arch: `formats: pacman` skips host Rust/cargo-packager. Ubuntu is Docker host
only (`ubuntu-24.04` / `ubuntu-24.04-arm`). Compile + makepkg run in
`.github/scripts/build-arch-package.sh`. cargo-packager's `pacman` format is a
`usr/` tarball, not a pacman package; CI must not use it.

`menci/archlinuxarm` does not ship `DisableSandbox`. Pacman 7 Landlock + the
`alpm` download user fail in GitHub ARM Docker; `build-arch-package.sh` must
uncomment `DisableSandbox` and comment `DownloadUser` before any `pacman -Sy`.

## Packager.toml / cargo-packager

- `before-packaging-command` must use `--locked`.
- `binaries-dir = "./target/release"`. `--target <triple>` is for package
  **naming** only; it does not send `cargo build --target` or write
  `target/<triple>/release`. CI also passes `--binaries-dir ./target/release`
  next to `--out-dir dist/<pack_dir>` so overriding out-dir cannot steal
  the binary path.
- Each pack matrix cell has `out_dir` (required `workflow_call` input).
  Scripts take `--dist dist/${{ inputs.out_dir }}`.
- `skip_updater` is a `workflow_call` boolean. Pass
  `${{ matrix.skip_updater || false }}` — do not `if:` on the raw matrix
  string (`"false"` is truthy).

Linux pack apt must stay in lockstep with CI GUI (`libx11-xcb-dev` included).
GUI CI Linux runners must match the primary pack runners (`ubuntu-24.04` /
`ubuntu-24.04-arm`).

## Other-repo writes

- Homebrew: generate `Casks/imprint.rb` (gitignored here), push to
  `googolmo/homebrew-tap` with `WORKFLOW_GH_TOKEN`. Authenticate with
  `http.https://github.com/.extraheader`; never put a PAT in the git remote URL.
- linux-repo: `.github/scripts/dispatch-linux-repo.sh` with `LINUX_REPO_TOKEN`.
  The script checks that `update-index.yml` exists and lists workflows on
  failure (`googolmo/repo` may be private). Inputs: `tag`, `version`,
  `github_repo`.

## Scripts CI

`ci.yml` `packaging-scripts` must run `--self-test` on the Python packagers,
`python3 -m py_compile .github/scripts/*.py`, and `bash -n` on
`build-arch-package.sh`, `dispatch-linux-repo.sh`, `codesign`, and `hdiutil`.

## macOS Intel DMG (`macos-15-intel`)

cargo-packager vendors create-dmg 1.1.1, which retries `hdiutil detach` three
times **without** `-force`. On GitHub Intel runners Spotlight / XProtect hold
the RW image after `--volicon`, and pack fails with:

```
hdiutil: couldn't eject "disk4" - Resource busy
```

`release-package.yml` must keep all three mitigations:

1. `.github/scripts/hdiutil` on `PATH` (retry, then `detach -force`). Same
   wrapper is copied next to `.github/scripts/codesign` for local
   `scripts/packager.sh`.
2. Kill XProtect and `mdutil -a -i off` before `cargo packager`.
3. Retry `cargo packager` up to 3 times, force-unmounting `/Volumes/Imprint`
   between attempts. `before-packaging-command` is a no-op once
   `target/release` exists; do not delete the `.app`.
