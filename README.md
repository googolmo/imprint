# Imprint

Flash OS images onto USB drives and SD cards. Same job as [balenaEtcher](https://github.com/balena-io/etcher): pick an image, pick a removable disk, write, verify.

Native desktop UI built with **[GPUI](https://gpui.rs)** (Zed’s GPU UI), plus a CLI. Runs on **macOS, Linux, and Windows**.

## Features

- Flash `.iso`, `.img`, `.dmg`, and compressed variants (`.gz`, `.bz2`, `.xz`, `.zst`, `.zip`)
- Auto-detect removable USB / SD targets; **system disks stay hidden**
- Write + optional byte-for-byte validation
- Drag-and-drop an image onto the window
- Flash the same image again, or pick a new one
- `imprint-cli` for scripts and recovery shells

Writing a disk **erases it**. Imprint will not list internal / system drives unless you explicitly ask the CLI for `--all`.

## Quick start

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

On Linux you typically write to `/dev/sdX` or `/dev/nvmeXn1`. On macOS prefer `/dev/rdiskN` after `diskutil list`. On Windows use `\\.\PhysicalDriveN`.

## Workspace

| Crate | Role |
|-------|------|
| `imprint-core` | Shared types (`ImageRef`, `TargetDisk`, `FlashProgress`, errors) |
| `imprint-image` | Sniff / decompress / stream the payload |
| `imprint-device` | Enumerate disks, hide system drives, unmount / eject |
| `imprint-flash` | Block write + verify |
| `imprint-ui` | GPUI views and theme |
| `imprint-app` | Desktop binary (`imprint`) |
| `imprint-cli` | CLI binary (`imprint-cli`) |

Dependency **versions** live in the root `Cargo.toml` `[workspace.dependencies]`. **Features** (GPUI platform backends, clap derive, …) live on the member crates.

GPUI comes from the **official Zed tree** (`github.com/zed-industries/zed`) as a git dependency in the root `Cargo.toml`. Do not use `gpui-unofficial`. See `AGENTS.md` to change the UI.

## Privileges

Opening a raw disk requires elevated rights. Imprint asks for them when you write:

- macOS: system authorization dialog (Touch ID / Apple Watch when the Mac allows it, otherwise password)
- Linux: polkit (`pkexec`) or `sudo`
- Windows: UAC prompt

You can still launch with `sudo` / “Run as administrator” if you prefer. The GUI no longer needs to be started as root.

## License

Apache-2.0
