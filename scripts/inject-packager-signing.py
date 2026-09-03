#!/usr/bin/env python3
"""Append CI signing fields to Packager.toml from the environment.

macOS:
  APPLE_SIGNING_IDENTITY -> [macos].signing-identity

Windows:
  WINDOWS_CERTIFICATE_THUMBPRINT -> [windows].certificate-thumbprint
  WINDOWS_TIMESTAMP_URL          -> [windows].timestamp-url
                                    (default http://timestamp.digicert.com)
  SIGNTOOL_PATH                  -> [windows].sign-command
                                    (cargo-packager's built-in lookup fails on
                                    ARM and some SDK 10.0.26100 layouts)
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGER = ROOT / "Packager.toml"


def toml_string(value: str) -> str:
    return repr(value)


def append_table(text: str, header: str, body: str) -> str:
    if f"[{header}]" in text:
        print(f"Packager.toml already has [{header}]; leaving as-is")
        return text
    return text.rstrip() + f"\n\n[{header}]\n{body}"


def inject_macos(text: str) -> str:
    ident = os.environ.get("APPLE_SIGNING_IDENTITY", "").strip()
    if not ident:
        print(
            "APPLE_SIGNING_IDENTITY unset; .app/.dmg will not be Developer ID signed"
        )
        return text
    body = f"signing-identity = {toml_string(ident)}\n"
    updated = append_table(text, "macos", body)
    if updated != text:
        print("wrote [macos].signing-identity")
    return updated


def inject_windows(text: str) -> str:
    thumb = os.environ.get("WINDOWS_CERTIFICATE_THUMBPRINT", "").strip().replace(" ", "")
    if not thumb:
        return text
    ts = (
        os.environ.get("WINDOWS_TIMESTAMP_URL", "").strip()
        or "http://timestamp.digicert.com"
    )
    body = (
        'digest-algorithm = "sha256"\n'
        f"certificate-thumbprint = {toml_string(thumb)}\n"
        f"timestamp-url = {toml_string(ts)}\n"
        "tsp = true\n"
    )
    # cargo-packager splits sign-command on spaces, so the binary path must
    # not contain any. CI copies signtool to RUNNER_TEMP (see locate-signtool.ps1).
    sign_bin = os.environ.get("SIGNTOOL_PATH", "").strip().replace("\\", "/")
    if sign_bin:
        if " " in sign_bin:
            print(
                f"SIGNTOOL_PATH contains a space ({sign_bin}); "
                "cargo-packager cannot parse it — unsigned MSI",
                file=sys.stderr,
            )
        else:
            # /tr+/td = RFC 3161; %1 is the file cargo-packager substitutes.
            cmd = (
                f"{sign_bin} sign /fd sha256 /sha1 {thumb} "
                f"/tr {ts} /td sha256 %1"
            )
            body += f"sign-command = {toml_string(cmd)}\n"
    updated = append_table(text, "windows", body)
    if updated != text:
        print("wrote [windows] Authenticode config")
    return updated


def main() -> int:
    target = sys.argv[1] if len(sys.argv) > 1 else str(PACKAGER)
    path = Path(target)
    text = path.read_text()
    system = sys.platform
    if system == "darwin":
        text = inject_macos(text)
    elif system == "win32":
        text = inject_windows(text)
    path.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
