#!/usr/bin/env python3
"""Render Imprint app icons.

Windows / Linux masters are square, full-bleed, unmasked 1024×1024 artwork.

macOS Dock / Finder / Cmd+Tab icons follow the traditional Mac grid: a smaller
rounded-rectangle (squircle) centered on a transparent 1024 canvas. `cargo run`
is not an `.app` bundle, so AppKit will not mask a full-bleed square; the
rounded shape and padding are baked into the macOS assets.
"""

from __future__ import annotations

import math
import shutil
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "icon"
LAYERS = OUT / "layers"

SIZE = 1024
SUPERSAMPLE = 4
WORK = SIZE * SUPERSAMPLE
# Apple's macOS icon grid: ~824px rounded rect on a 1024 canvas (~100px margin).
MACOS_BODY = 824

# Brand tokens from crates/imprint-ui/src/theme.rs
PRIMARY = np.array([0x0E, 0x4B, 0xEF], dtype=np.float64)
CYAN = np.array([0x58, 0xE2, 0xEE], dtype=np.float64)


def lerp(a: np.ndarray, b: np.ndarray, t: np.ndarray | float) -> np.ndarray:
  t = np.clip(t, 0.0, 1.0)
  return a + (b - a) * t[..., None] if isinstance(t, np.ndarray) else a + (b - a) * t


def smooth(t: np.ndarray) -> np.ndarray:
  return t * t * (3.0 - 2.0 * t)


def circle_mask(yy: np.ndarray, xx: np.ndarray, cx: float, cy: float, r: float) -> np.ndarray:
  """Coverage-aware disc, 0..1, using a 1px (work-space) aa band."""
  d = np.sqrt((xx - cx) ** 2 + (yy - cy) ** 2)
  return np.clip(r + 0.75 - d, 0.0, 1.0)


def overlay(dst: np.ndarray, src_rgb: np.ndarray, alpha: np.ndarray) -> None:
  a = alpha[..., None]
  dst *= 1.0 - a
  dst += src_rgb * a


def paint_background(yy: np.ndarray, xx: np.ndarray, dark: bool) -> np.ndarray:
  u = xx / (WORK - 1)
  v = yy / (WORK - 1)
  # Match the app atmosphere (~132–148° CSS): more down than right.
  angle = math.radians(140.0)
  t = u * math.sin(angle) + v * math.cos(angle)
  t = smooth((t - t.min()) / (t.max() - t.min()))

  if dark:
    c0 = np.array([0x12, 0x2A, 0x6A], dtype=np.float64)
    c1 = np.array([0x0A, 0x16, 0x3A], dtype=np.float64)
    c2 = np.array([0x18, 0x12, 0x48], dtype=np.float64)
  else:
    c0 = np.array([0x3D, 0x9D, 0xFF], dtype=np.float64)
    c1 = PRIMARY.copy()
    c2 = np.array([0x2A, 0x18, 0xC4], dtype=np.float64)

  mid = lerp(c0, c1, np.clip(t * 1.35, 0.0, 1.0))
  rgb = lerp(mid, c2, np.clip((t - 0.42) / 0.58, 0.0, 1.0))

  # Soft blooms, same language as the in-app atmosphere (not a glyph shadow).
  if dark:
    overlay(rgb, CYAN, 0.16 * np.exp(-(((u - 0.78) ** 2 + (v - 0.22) ** 2) / 0.18)))
    overlay(rgb, PRIMARY, 0.22 * np.exp(-(((u - 0.18) ** 2 + (v - 0.80) ** 2) / 0.22)))
  else:
    overlay(rgb, CYAN, 0.28 * np.exp(-(((u - 0.82) ** 2 + (v - 0.18) ** 2) / 0.16)))
    overlay(rgb, np.array([0x6A, 0x40, 0xE8], dtype=np.float64), 0.18 * np.exp(-(((u - 0.12) ** 2 + (v - 0.88) ** 2) / 0.20)))
    overlay(rgb, np.array([0xFF, 0xFF, 0xFF], dtype=np.float64), 0.12 * np.exp(-(((u - 0.28) ** 2 + (v - 0.12) ** 2) / 0.10)))
  return rgb


def glyph_geometry() -> tuple[float, float, float, float, float]:
  """Centered circular seal. Diameter ~66% so the squircle mask never clips it."""
  cx = (WORK - 1) / 2.0
  cy = (WORK - 1) / 2.0 - 0.008 * WORK  # optical center
  outer = 0.330 * WORK
  inner = 0.218 * WORK
  hub = 0.122 * WORK
  return cx, cy, outer, inner, hub


def paint_glyph(yy: np.ndarray, xx: np.ndarray, dark: bool) -> tuple[np.ndarray, np.ndarray]:
  """Foreground: filled overlapping circles (wax-seal / media imprint)."""
  cx, cy, outer, inner, hub = glyph_geometry()
  white = np.array([0xF7, 0xFB, 0xFF], dtype=np.float64)
  glass = np.array([0x4E, 0xD0, 0xE4] if not dark else [0x3A, 0xC0, 0xD8], dtype=np.float64)
  hub_a = PRIMARY if not dark else np.array([0x4A, 0xA0, 0xFF], dtype=np.float64)
  hub_b = CYAN if not dark else np.array([0x6A, 0xE8, 0xF2], dtype=np.float64)

  m_outer = circle_mask(yy, xx, cx, cy, outer)
  m_inner = circle_mask(yy, xx, cx, cy, inner)
  m_hub = circle_mask(yy, xx, cx, cy, hub)

  # Disc body: cool white with a top-left light, hard edge (HIG: defined edges).
  lx = (xx - (cx - 0.35 * outer)) / (outer * 2.2)
  ly = (yy - (cy - 0.45 * outer)) / (outer * 2.2)
  shade = smooth(np.clip(0.35 + 0.65 * (1.0 - np.sqrt(np.clip(lx * lx + ly * ly, 0.0, 1.0))), 0.0, 1.0))
  disc = lerp(np.array([0xD4, 0xE4, 0xF8], dtype=np.float64), white, shade)

  rgba = np.zeros((WORK, WORK, 4), dtype=np.float64)
  overlay(rgba[..., :3], disc, m_outer)
  rgba[..., 3] = np.maximum(rgba[..., 3], m_outer)

  overlay(rgba[..., :3], glass, m_inner * (0.92 if not dark else 0.88))
  rgba[..., 3] = np.maximum(rgba[..., 3], m_inner)

  # Hub: brand sapphire → cyan, same as the primary button.
  u = xx / (WORK - 1)
  v = yy / (WORK - 1)
  hub_t = smooth(np.clip((u - v + 0.55) / 1.1, 0.0, 1.0))
  hub_rgb = lerp(hub_a, hub_b, hub_t)
  overlay(rgba[..., :3], hub_rgb, m_hub)
  rgba[..., 3] = np.maximum(rgba[..., 3], m_hub)

  # Tiny specular coin-catch on the hub only — object material, not a canvas effect.
  spec = circle_mask(yy, xx, cx - 0.28 * hub, cy - 0.32 * hub, 0.28 * hub)
  overlay(rgba[..., :3], np.array([0xFF, 0xFF, 0xFF], dtype=np.float64), spec * m_hub * 0.55)
  return rgba, m_outer


def downscale(arr: np.ndarray) -> Image.Image:
  img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8))
  return img.resize((SIZE, SIZE), Image.Resampling.LANCZOS)


def compose(dark: bool) -> tuple[Image.Image, Image.Image, Image.Image]:
  yy, xx = np.mgrid[0:WORK, 0:WORK]
  bg = paint_background(yy, xx, dark)
  glyph, _ = paint_glyph(yy, xx, dark)
  master = bg.copy()
  overlay(master, glyph[..., :3], glyph[..., 3])
  return downscale(bg), downscale(glyph), downscale(master)


def squircle_mask(size: int) -> Image.Image:
  """Apple-style superellipse (n≈5)."""
  n = 5.0
  a = size / 2.0 - 0.5
  yy, xx = np.mgrid[0:size, 0:size]
  x = (xx + 0.5 - size / 2.0) / a
  y = (yy + 0.5 - size / 2.0) / a
  d = np.abs(x) ** n + np.abs(y) ** n
  cov = np.clip((1.02 - d) / 0.035, 0.0, 1.0)
  return Image.fromarray((cov * 255).astype(np.uint8))


def macos_icon(master: Image.Image) -> Image.Image:
  """Centered squircle with transparent padding for Dock / Cmd+Tab / .icns."""
  canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
  body = master.convert("RGBA").resize((MACOS_BODY, MACOS_BODY), Image.Resampling.LANCZOS)
  body.putalpha(squircle_mask(MACOS_BODY))
  origin = (SIZE - MACOS_BODY) // 2
  canvas.alpha_composite(body, (origin, origin))
  return canvas


def write_iconset(icon: Image.Image, iconset: Path) -> None:
  iconset.mkdir(parents=True, exist_ok=True)
  mapping = [
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
  ]
  rgba = icon.convert("RGBA")
  for px, name in mapping:
    rgba.resize((px, px), Image.Resampling.LANCZOS).save(iconset / name, "PNG")


def write_ico(master: Image.Image, path: Path) -> None:
  rgb = master.convert("RGBA")
  sizes = [(16, 16), (32, 32), (48, 48), (256, 256)]
  rgb.save(path, sizes=sizes)


def main() -> None:
  OUT.mkdir(parents=True, exist_ok=True)
  LAYERS.mkdir(parents=True, exist_ok=True)

  bg, fg, master = compose(dark=False)
  bg_d, fg_d, master_d = compose(dark=True)

  bg.convert("RGB").save(LAYERS / "background.png", "PNG")
  fg.save(LAYERS / "foreground.png", "PNG")
  bg_d.convert("RGB").save(LAYERS / "background-dark.png", "PNG")
  fg_d.save(LAYERS / "foreground-dark.png", "PNG")

  master.convert("RGB").save(OUT / "AppIcon.png", "PNG")
  master_d.convert("RGB").save(OUT / "AppIcon-dark.png", "PNG")

  mac = macos_icon(master)
  mac_d = macos_icon(master_d)
  mac.save(OUT / "AppIcon-macos.png", "PNG")
  mac_d.save(OUT / "AppIcon-macos-dark.png", "PNG")
  mac.save(OUT / "AppIcon-preview.png", "PNG")
  mac_d.save(OUT / "AppIcon-preview-dark.png", "PNG")
  write_ico(master, OUT / "AppIcon.ico")

  if shutil.which("iconutil"):
    with tempfile.TemporaryDirectory() as tmp:
      iconset = Path(tmp) / "AppIcon.iconset"
      write_iconset(mac, iconset)
      subprocess.run(
        ["iconutil", "-c", "icns", "-o", str(OUT / "AppIcon.icns"), str(iconset)],
        check=True,
      )
  else:
    print("iconutil not found; skipped AppIcon.icns")

  print(f"wrote icons in {OUT}")


if __name__ == "__main__":
  main()
