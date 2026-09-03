# How to use Imprint

[English](HOW_TO_USE.md) · [简体中文](HOW_TO_USE.zh-Hans.md) · [繁體中文](HOW_TO_USE.zh-Hant.md) · [日本語](HOW_TO_USE.ja.md)

Imprint writes an OS image onto a USB drive or SD card. The same steps work on macOS, Linux, and Windows.

**Writing erases the selected drive.** Internal / system disks are hidden by default. Do not turn that protection off unless you know exactly which disk you are targeting.

## Desktop app

Launch **Imprint**. The main window is three stages: **Image → Target → Write**.

### 1. Choose an image

Pick a disk image in any of these ways:

- Click **Select** on the Image card
- **File → Open Image…** (`⌘O` on macOS, `Ctrl+O` on Linux/Windows)
- Drag an image file onto the window

Supported files:

| Kind | Extensions |
|------|------------|
| Disk images | `.iso`, `.img`, `.dmg`, `.raw`, `.bin` |
| Compressed | `.gz`, `.bz2`, `.xz`, `.zst`, `.zip` (and combinations such as `.img.xz`) |

The card shows the file name, kind, and size. Compressed images also show the uncompressed payload size when Imprint can read it.

### 2. Choose a drive

1. Plug in the USB stick or SD card (use a reader if needed).
2. Click **Select** on the Target card, or **File → Select Drive…**.
3. Choose the removable disk. Click a second disk to flash more than one at once. Click **Refresh** if a drive does not appear.
4. Click **Done**.

Drives that are too small for the image are marked and cannot be selected. Internal disks stay hidden unless you change that in Settings — leave that off.

On Linux the device is typically `/dev/sdX` or `/dev/nvmeXn1`. On macOS prefer `/dev/rdiskN`. On Windows it is `\\.\PhysicalDriveN`. You do not need those paths in the GUI; they matter for the [CLI](#command-line).

### 3. Write

1. Confirm the image name and the drive label.
2. Click **Write**. The selected drive will be erased.
3. Approve the administrator prompt:
   - **macOS:** Touch ID, Apple Watch, or password
   - **Linux:** polkit (`pkexec`) or `sudo`
   - **Windows:** UAC
4. Wait through unmount → flash → optional validation → sync.
5. Click **Cancel** if you need to stop. A cancelled write leaves the drive in an incomplete state; flash it again before using it.

When it finishes, the drive is ready to boot. Choose **Write another** for a new image, or **Keep image** to flash the same file to another disk.

### Raspberry Pi

Use this when you want an official Raspberry Pi OS image and first-boot options (hostname, user, Wi-Fi, SSH) without writing `userconf` files by hand.

1. Click the **Raspberry Pi** bar at the bottom of the main window, or **File → Raspberry Pi…** (`⇧⌘R` / `Ctrl+Shift+R`).
2. **Device** — pick the Pi model so the catalog can filter compatible images.
3. **OS** — choose an official image, or **Use custom** for a local `.img` / `.iso` / compressed file. Official downloads are cached; a second flash of the same image does not re-download.
4. **Options** — turn on only the sections you want applied after the write:
   - Hostname
   - Username and password
   - Wi-Fi (network name, password, country code)
   - SSH (optional public key)
   - Timezone and keyboard
5. **Storage** — pick the SD card or USB disk, then **Write**.

Imprint downloads the image if needed, writes it, then overlays first-boot files on the FAT boot partition (cloud-init or legacy systemd, depending on the image).

Click the back arrow at any time to return to the normal flash screen.

### Settings

Open **Settings** from the gear in the title bar, **Imprint → Settings…**, or `⌘,` / `Ctrl+,`.

| Setting | What it does | Default |
|---------|----------------|---------|
| Appearance | System, light, or dark | Follows the OS |
| Language | UI language | OS language when a translation exists, otherwise English |
| Validate write | Re-read every byte after flashing | On |
| Eject on success | Unmount the drive when the write finishes | On |
| Hide system drives | Never list internal disks | On |

Languages: English, 简体中文, 繁體中文, 日本語, 한국어, Deutsch, Español, Français, Português.

### Keyboard shortcuts

| Action | macOS | Linux / Windows |
|--------|-------|-----------------|
| Open image | `⌘O` | `Ctrl+O` |
| Raspberry Pi mode | `⇧⌘R` | `Ctrl+Shift+R` |
| Refresh drives | `⌘R` | `Ctrl+R` |
| Settings | `⌘,` | `Ctrl+,` |
| Quit | `⌘Q` | `Ctrl+Q` |

**Imprint → About Imprint** shows the version and **Check for Updates…** (packaged builds only).

## Command line

`imprint-cli` is the same write pipeline without a window. Use it in scripts, SSH sessions, or recovery shells. Raspberry Pi first-boot options (hostname, Wi-Fi, SSH) are GUI-only.

List removable disks:

```bash
imprint-cli devices
```

Include system disks (read-only listing; still refuse to flash them unless you change code-level guards):

```bash
imprint-cli devices --all
```

Flash. `--yes` is required so a typo cannot wipe a disk:

```bash
imprint-cli flash ubuntu.iso --device /dev/rdisk4 --yes
```

Skip byte-for-byte validation, or leave the volume mounted:

```bash
imprint-cli flash raspios.img.xz --device /dev/sdc --yes --no-verify --no-eject
```

Device path examples:

| OS | Typical path |
|----|----------------|
| macOS | `/dev/rdiskN` (`diskutil list`) |
| Linux | `/dev/sdX` or `/dev/nvmeXn1` |
| Windows | `\\.\PhysicalDriveN` |

The CLI still prompts for administrator / root access when it opens the raw disk. You can also run it under `sudo` / “Run as administrator”.

From a source checkout:

```bash
cargo run -p imprint-cli -- devices
cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes
```

## Privileges

Opening a raw disk needs elevated rights. Imprint asks at write time; you do not have to start the GUI as root.

If the prompt is cancelled, the write does not start. If elevation fails, check that you can approve admin dialogs on this machine (polkit on Linux, an admin account on macOS/Windows).

## If something goes wrong

| What you see | What to try |
|--------------|-------------|
| No removable drives | Unplug and replug the disk, then **Refresh**. Confirm the OS mounted it as removable, not an internal volume. |
| Drive is “too small” | The image (uncompressed) is larger than the disk. Pick a bigger card or a smaller image. |
| Unsupported image | Use `.iso` / `.img` / `.dmg` or a compressed variant listed above. |
| Need administrator privileges | Approve the prompt, or run the CLI with `sudo` / as Administrator. |
| Verification failed | The write did not match on read-back. Try another cable, port, or disk; SD cards and cheap USB sticks fail this more often than the host. |
| Raspberry Pi catalog failed | Check the network and click **Retry**. You can still **Use custom** with a local file. |
| Download / checksum failed | Retry the download. A checksum failure means the file was truncated or corrupted. |
| Flash cancelled | The disk is not bootable. Flash it again. |

Validation is on by default. Turn it off in Settings only if you understand you will not catch a bad write until the target fails to boot.

## Build from source

See the [README](../README.md#quick-start) for `cargo run`, Linux development packages, and the macOS Xcode requirement for the GUI.
