#!/usr/bin/env python3
"""Render Mac tray images from the brand mark and shared accent tokens."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "apps/desktop/src-tauri/icons"


def images() -> dict[str, bytes]:
    tokens = json.loads((ROOT / "assets/brand/tokens.json").read_text())
    sources = {"macos-menubar.png": (ROOT / "assets/brand/menubar.svg").read_bytes()}
    for appearance in ("light", "dark"):
        accent = tokens["colors"]["accent"][appearance]
        sources[f"macos-recording-{appearance}.png"] = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 18 18">'
            f'<circle cx="9" cy="9" r="6.75" fill="none" stroke="{accent}" stroke-width="1.5"/>'
            f'<circle cx="9" cy="9" r="3.5" fill="{accent}"/>'
            '</svg>'
        ).encode()
    return {
        name: subprocess.run(
            ["rsvg-convert", "--width", "36", "--height", "36"],
            input=source,
            capture_output=True,
            check=True,
        ).stdout
        for name, source in sources.items()
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for name, content in images().items():
        path = OUTPUT / name
        if args.check:
            if not path.exists() or path.read_bytes() != content:
                raise SystemExit(f"Stale Mac tray image: {path.relative_to(ROOT)}")
        else:
            path.write_bytes(content)


if __name__ == "__main__":
    main()
