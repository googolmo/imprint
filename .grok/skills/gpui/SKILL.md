---
name: gpui
description: >
  Official Zed GPUI notes for Imprint. Prefer the user skill at
  ~/.grok/skills/gpui when present. Use when changing imprint-ui / imprint-app
  GPUI code. Never depend on gpui-unofficial.
---

# GPUI in this repo

Imprint uses **official** GPUI from `github.com/zed-industries/zed` (see root `Cargo.toml` rev).

Bootstrap: `gpui_platform::application()` in `crates/imprint-app`. Features (`font-kit`, `wayland`, `x11`) are on `imprint-app` / `imprint-ui`, not in workspace versions.

Full agent guide: `~/.grok/skills/gpui/SKILL.md` and `AGENTS.md` in the repo root.
