<p align="center">
  <img src="assets/icon/AppIcon-preview.png" width="128" alt="Imprint">
</p>

# Imprint

[English](README.md) · [简体中文](README.zh-Hans.md) · [繁體中文](README.zh-Hant.md) · [日本語](README.ja.md)

Flash OS images onto USB drives and SD cards. Same job as [balenaEtcher](https://github.com/balena-io/etcher): pick an image, pick a removable disk, write, verify.

Native desktop UI built with **[GPUI](https://gpui.rs)** (Zed’s GPU UI), plus a CLI. Runs on **macOS, Linux, and Windows**.

**Writing a disk erases it.** Imprint hides internal / system drives. The CLI will not write without `--yes`.

[How to use](docs/HOW_TO_USE.md) ([简体中文](docs/HOW_TO_USE.zh-Hans.md) · [繁體中文](docs/HOW_TO_USE.zh-Hant.md) · [日本語](docs/HOW_TO_USE.ja.md)) · [Releases](https://github.com/googolmo/imprint/releases)

## Features

- Flash `.iso`, `.img`, `.dmg`, and compressed variants (`.gz`, `.bz2`, `.xz`, `.zst`, `.zip`)
- Raspberry Pi mode: download official images and set hostname, user, Wi-Fi, and SSH
- Auto-detect removable USB / SD targets; **system disks stay hidden**
- Write + optional byte-for-byte validation
- Drag-and-drop an image onto the window
- Flash the same image again, or pick a new one
- In-app updates on packaged builds
- UI in English, 简体中文, 繁體中文, 日本語, 한국어, Deutsch, Español, Français, Português
- `imprint-cli` for scripts and recovery shells

## How to use

Full walkthrough: **[docs/HOW_TO_USE.md](docs/HOW_TO_USE.md)**.

### Desktop

1. Open **Imprint**.
2. **Image** — click **Select**, use **File → Open Image…**, or drop an `.iso` / `.img` / `.dmg` (or a compressed archive) onto the window.
3. **Target** — plug in the USB stick or SD card, click **Select**, choose the removable disk. Internal disks are not listed.
4. Click **Write**. Approve the administrator prompt (Touch ID / password, polkit, or UAC).
5. Wait for unmount → flash → optional validation. When it finishes, the drive is ready to boot.

**Raspberry Pi:** click the Raspberry Pi bar (or **File → Raspberry Pi…**). Pick the model, an official image or a local file, first-boot options (hostname, user, Wi-Fi, SSH), then the SD card.

Settings (gear, or `⌘,` / `Ctrl+,`): appearance, language, validate write, eject on success, hide system drives.

### Command line

```bash
# List removable disks
imprint-cli devices

# Flash — --yes is required; prompts for administrator / root access
imprint-cli flash ubuntu.iso --device /dev/rdisk4 --yes
```

Device paths: macOS `/dev/rdiskN`, Linux `/dev/sdX` or `/dev/nvmeXn1`, Windows `\\.\PhysicalDriveN`.

## Install

### macOS (Homebrew)

```bash
brew tap googolmo/imprint https://github.com/googolmo/imprint
brew install --cask imprint
```

This installs **Imprint** in `/Applications` and puts `imprint-cli` on your `PATH`. The tap URL is required because the cask lives in this repository, not `homebrew/cask`.

Or download the `.dmg` from [GitHub Releases](https://github.com/googolmo/imprint/releases).

### Linux and Windows

Packaged builds are attached to [GitHub Releases](https://github.com/googolmo/imprint/releases): Windows x86-64 (`.msi`) and arm64 (NSIS); Linux x86-64 and arm64 (`.deb` for Ubuntu 22.04/Debian and Ubuntu 24.04, AppImage, Arch `.pkg.tar.zst`, and an AUR `PKGBUILD`).

## Quick start (from source)

```bash
# List removable disks
cargo run -p imprint-cli -- devices

# Flash — prompts for administrator / root access
cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes

# GUI — macOS needs full Xcode (Metal shader compiler), not just CLT
cargo run -p imprint-app
```

Linux GUI builds need Fontconfig, FreeType, Wayland/X11, and Vulkan **development** packages (`*.pc` files). Runtime libs alone are not enough:

```bash
sudo apt-get install -y --no-install-recommends \
  pkg-config \
  libfontconfig-dev \
  libfreetype6-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libwayland-dev \
  libx11-dev \
  libx11-xcb-dev \
  libasound2-dev \
  libvulkan-dev \
  libgl1-mesa-dev
```

## Privileges

Opening a raw disk requires elevated rights. Imprint asks for them when you write:

- macOS: system authorization dialog (Touch ID / Apple Watch when the Mac allows it, otherwise password)
- Linux: polkit (`pkexec`) or `sudo`
- Windows: UAC prompt

You can still launch with `sudo` / “Run as administrator” if you prefer. The GUI does not need to be started as root.

## Workspace

| Crate | Role |
|-------|------|
| `imprint-core` | Shared types (`ImageRef`, `TargetDisk`, `FlashProgress`, errors) |
| `imprint-image` | Sniff / decompress / stream the payload |
| `imprint-device` | Enumerate disks, hide system drives, unmount / eject |
| `imprint-rpi` | Raspberry Pi catalog, download, first-boot config |
| `imprint-flash` | Block write + verify |
| `imprint-ui` | GPUI views and theme |
| `imprint-app` | Desktop binary (`imprint`) |
| `imprint-cli` | CLI binary (`imprint-cli`) |

Dependency **versions** live in the root `Cargo.toml` `[workspace.dependencies]`. **Features** (GPUI platform backends, clap derive, …) live on the member crates.

GPUI comes from the **official Zed tree** (`github.com/zed-industries/zed`) as a git dependency in the root `Cargo.toml`. Do not use `gpui-unofficial`. See `AGENTS.md` to change the UI.

## License

Imprint source is **Apache-2.0**. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

- **CLI** (`imprint-cli`): Apache-2.0. It does not link GPUI.
- **Desktop app** (`imprint`): GPUI from the official Zed git tree currently pulls `ztracing` / `zlog` / `ztracing_macro` (**GPL-3.0-or-later**). Distributing that binary is a combined work that must also comply with GPL-3.0 until [zed#55470](https://github.com/zed-industries/zed/issues/55470) is fixed.
