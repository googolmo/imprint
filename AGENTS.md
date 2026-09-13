# AGENTS.md

Guide for coding agents working on Imprint. Humans: start with `README.md`.

## What this is

Cross-platform USB/SD image writer (Etcher-class). GPUI desktop app + CLI. Writing a block device destroys its contents; never target a disk with `system: true`.

## Layout

```
Cargo.toml                 workspace versions only
crates/imprint-build/      build.rs helpers (Packager.toml identity)
crates/imprint-core/       types + errors (no IO, no GPUI)
crates/imprint-image/      inspect + payload reader
crates/imprint-device/     list / unmount / eject (OS-specific under src/platform/)
crates/imprint-rpi/        Raspberry Pi catalog, download, first-boot config
crates/imprint-flash/      write + verify pipeline (+ FAT boot overlay)
crates/imprint-ui/         GPUI views (theme, cards, overlays)
crates/imprint-app/        binary: gpui_kit::platform::application()
crates/imprint-cli/        binary: clap
.grok/skills/gpui/              short pointer; full GPUI skill is ~/.grok/skills/gpui
.grok/skills/imprint-release/   GitHub Release / packaging pitfalls
```

Dependency graph:

```
core
  ↑
image, device, rpi
  ↑
flash
  ↑
ui → app
flash → cli
```

Do not depend on `imprint-ui` from flash/device/image. Do not put block-device IO in the UI crate.

## Cargo rules

- Versions: root `[workspace.dependencies]`
- Features: member `Cargo.toml` only, except `gpui-kit` which is declared once at the workspace (`features = ["component"]`)
- GPUI: **[gpui-kit](https://crates.io/crates/gpui-kit)** only. Do not add `gpui`, `gpui_platform`, `gpui-component`, or `gpui-unofficial` as direct dependencies. Import through `gpui_kit` (`gpui_kit::gpui`, `gpui_kit::platform`, `gpui_kit::component`, `gpui_kit::assets`).

App identity: `Packager.toml` `identifier` / `product-name`, baked in at compile time as `IMPRINT_APP_IDENTIFIER` and `IMPRINT_APP_PRODUCT_NAME` (`env!` in `imprint-app` and `imprint-ui`).

## Commands

```bash
cargo check -p imprint-cli
cargo check -p imprint-ui
cargo test --workspace --exclude imprint-app --exclude imprint-ui
cargo run -p imprint-cli -- devices
cargo run -p imprint-app         # macOS: full Xcode (metal), not just CLT
cargo fmt
```

macOS GUI needs **Xcode.app** (`xcrun metal`). Command Line Tools alone cannot compile `gpui_apple` shaders. The CLI does not need Metal.

## GPUI (via gpui-kit)

Read `~/.grok/skills/gpui/SKILL.md` and [gpui-kit](https://github.com/longbridge/gpui-kit) docs.

Bootstrap is in `crates/imprint-app/src/main.rs`:

```rust
use gpui_kit::{
  assets as gpui_component_assets, component as gpui_component, gpui, platform as gpui_platform,
};

gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed)
    .with_assets(gpui_component_assets::Assets)
    .run(|cx: &mut App| { … cx.open_window(…, |window, cx| cx.new(|cx| ImprintApp::new(window, cx))) });
```

View state: `crates/imprint-ui/src/app.rs` (`Render` for `ImprintApp`). Styling: Tailwind-like `div()` in `widgets.rs` + `theme.rs`. Widgets come from `gpui_kit::component`.

Patterns already in the app:

- `.id("…").on_click(cx.listener(…))` — clicks need an id
- `cx.prompt_for_paths(PathPromptOptions { … })` + `cx.spawn` + `WeakEntity::update`
- `.on_drop(cx.listener(|this, paths: &ExternalPaths, …| …))`
- `actions!` in `imprint-ui`, menus/keybindings in `imprint-app`
- Flash work runs on a **std thread**; UI pumps `crossbeam-channel` via `cx.spawn` + `background_executor().timer`

Do not call `Application::new()` — that is the pre-split API. Do not depend on Zed’s git tree directly.

## Flash pipeline

1. `imprint-image::inspect` → `ImageRef`
2. `imprint-device::list_targets` → hide `system` disks
3. `imprint-flash::validate_request` → size + system guard
4. unmount → write 1 MiB blocks → `sync_all` → optional verify → eject

Cancel with the `AtomicBool` passed into `flash()`.

## Adding an OS

New disk enumeration goes in `crates/imprint-device/src/platform/<os>.rs` and a `cfg` arm in `platform.rs`. Keep `TargetDisk.path` as the raw writable node (`/dev/rdiskN`, `/dev/sdX`, `\\.\PhysicalDriveN`).

## Tests

- Pure tests in `imprint-image` (magic bytes, names)
- Do **not** write integration tests that open real disks
- GPUI tests use `#[gpui::test]` / `gpui_kit::test` if you add them; they need gpui-kit `test-support`

## Style

`rustfmt.toml`: 2-space indent, 100-wide. Match neighboring files. No drive-by refactors.
