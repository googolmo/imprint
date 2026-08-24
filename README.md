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
# Official GPUI sources (shallow Zed checkout, once)
./scripts/vendor-zed.sh

# List removable disks
cargo run -p imprint-cli -- devices

# Flash (requires root / Administrator)
sudo cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes

# GUI — macOS needs full Xcode (Metal shader compiler), not just CLT
cargo run -p imprint-app
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

GPUI comes from the **official Zed tree** (`github.com/zed-industries/zed`), vendored by `scripts/vendor-zed.sh` into `~/.cache/imprint/zed` so it is not a nested workspace. Do not use `gpui-unofficial`. See `AGENTS.md` to change the UI.

## Privileges

Opening a raw disk requires elevated rights:

- macOS / Linux: `sudo`
- Windows: “Run as administrator”

The GUI will error with a privileges message if it cannot open the device.

## License

Apache-2.0
