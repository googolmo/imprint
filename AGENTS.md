# AGENTS.md

Guide for coding agents working on Imprint. Humans: start with `README.md`.

## What this is

Cross-platform USB/SD image writer (Etcher-class). GPUI desktop app + CLI. Writing a block device destroys its contents; never target a disk with `system: true`.

## Layout

```
Cargo.toml                 workspace versions only
crates/imprint-core/       types + errors (no IO, no GPUI)
crates/imprint-image/      inspect + payload reader
crates/imprint-device/     list / unmount / eject (OS-specific under src/platform/)
crates/imprint-flash/      write + verify pipeline
crates/imprint-ui/         GPUI views (theme, cards, overlays)
crates/imprint-app/        binary: gpui_platform::application()
crates/imprint-cli/        binary: clap
.grok/skills/gpui/         short pointer; full GPUI skill is ~/.grok/skills/gpui
```

Dependency graph:

```
core
  ↑
image, device
  ↑
flash
  ↑
ui → app
flash → cli
```

Do not depend on `imprint-ui` from flash/device/image. Do not put block-device IO in the UI crate.

## Cargo rules

- Versions and git revs: root `[workspace.dependencies]`
- Features: member `Cargo.toml` only
- GPUI: official Zed sources only, **never** `gpui-unofficial`
- Nested Cargo workspaces are invalid, so Zed is a **shallow checkout outside this repo**:
  `~/.cache/imprint/zed` (see `scripts/vendor-zed.sh`)
- Root `Cargo.toml` path-depends on that checkout. After bumping `ZED_REV`, run the script and keep both `gpui` / `gpui_platform` paths in sync.

`imprint-app` enables `gpui_platform` features `font-kit`, `wayland`, `x11` (Zed README cross-platform set).

App identity: `imprint.cdxtheme.com` (`cx.set_app_identity` in `imprint-app`).

## Commands

```bash
./scripts/vendor-zed.sh          # once: shallow-clone official Zed
cargo check -p imprint-cli
cargo check -p imprint-ui
cargo test --workspace --exclude imprint-app --exclude imprint-ui
cargo run -p imprint-cli -- devices
cargo run -p imprint-app         # macOS: full Xcode (metal), not just CLT
cargo fmt
```

macOS GUI needs **Xcode.app** (`xcrun metal`). Command Line Tools alone cannot compile `gpui_apple` shaders. The CLI does not need Metal.

## GPUI (official only)

Read `~/.grok/skills/gpui/SKILL.md` and Zed `crates/gpui/examples/`.

Bootstrap is in `crates/imprint-app/src/main.rs`:

```rust
gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed)
    .run(|cx: &mut App| { … cx.open_window(…, |window, cx| cx.new(|cx| ImprintApp::new(window, cx))) });
```

View state: `crates/imprint-ui/src/app.rs` (`Render` for `ImprintApp`). Styling: Tailwind-like `div()` in `widgets.rs` + `theme.rs`.

Patterns already in the app (copied from Zed examples / `div.rs`):

- `.id("…").on_click(cx.listener(…))` — clicks need an id
- `cx.prompt_for_paths(PathPromptOptions { … })` + `cx.spawn` + `WeakEntity::update`
- `.on_drop(cx.listener(|this, paths: &ExternalPaths, …| …))`
- `actions!` in `imprint-ui`, menus/keybindings in `imprint-app`
- Flash work runs on a **std thread**; UI pumps `crossbeam-channel` via `cx.spawn` + `background_executor().timer`

Do not call `Application::new()` — that is the pre-split API.

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
- GPUI tests use `#[gpui::test]` if you add them; they need `gpui` `test-support`

## Style

`rustfmt.toml`: 2-space indent, 100-wide. Match neighboring files. No drive-by refactors.
