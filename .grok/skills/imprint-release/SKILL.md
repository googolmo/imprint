---
name: imprint-release
description: >
  Imprint GitHub Release packaging overlay: product identity, Homebrew tap,
  linux-repo, GPUI apt, Ubuntu 26.04 preview. Use when changing
  .github/workflows/release.yml, release-package.yml, ci.yml packaging jobs,
  .github/scripts/*, Packager.toml, or when the user mentions release assets,
  Homebrew cask, pacman, AppImage, latest.json, or /imprint-release.
---

# Imprint release overlay

Generic pipeline rules live in the user skill **rust-app-release**
(`~/.grok/skills/rust-app-release`). Load that first. This file is
Imprint-only identity and extra jobs.

Canonical files: `.github/workflows/release.yml` (caller),
`release-package.yml` (pack), `.github/scripts/*`, `Packager.toml`.

## Identity

| Key | Value |
|-----|--------|
| slug / `Packager.toml` `name` | `imprint` |
| `product-name` | `Imprint` |
| `identifier` | `imprint.cdxtheme.com` |
| crates packed | `-p imprint-app -p imprint-cli` |
| sidecar | `imprint-cli` next to `imprint` in `Contents/MacOS` (codesign wrapper required) |
| DMG volume | `/Volumes/Imprint` |
| `.app` | `Imprint.app` |

Asset prefix: `imprint_{version}_{system}_{cpu}{suffix}`.

## Other-repo jobs

- Homebrew: generate `Casks/imprint.rb` (gitignored here), push to
  `googolmo/homebrew-tap` with `WORKFLOW_GH_TOKEN` via `http.extraheader`.
  Cask is not a GitHub Release asset. Tap is shared; this workflow only
  touches `Casks/imprint.rb`.
- linux-repo: `.github/scripts/dispatch-linux-repo.sh` → `googolmo/repo`
  `update-index.yml`. Inputs: `tag`, `version`, `github_repo`. That repo
  signs APT InRelease with its own `GPG_PRIVATE_KEY`. Ubuntu 26.04 is
  `package-linux-preview` → `linux-repo-preview` so a preview failure does
  not skip the 24.04+Arch index.

## GPUI / Linux

Pack Linux apt must stay in lockstep with CI GUI (`libx11-xcb-dev`
included). GUI CI Linux runners = primary pack runners (`ubuntu-24.04` /
`ubuntu-24.04-arm`).

Do not reintroduce Ubuntu 22.04 or `ubuntu22.04` asset tags.

## Local pack

`scripts/packager.sh` loads cert/key files, injects signing, puts
`.github/scripts/codesign` and `hdiutil` on `PATH`.
