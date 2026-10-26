#!/usr/bin/env python3
"""Render the connect screen inside the obsidian frame, for the README.

Needs Pillow and cargo. Run from anywhere: python3 frames/make_screenshots.py
Writes docs/images/connect-*.png. The pictures come from the `renders_every_screen` test in
app/src/connect.rs (the order of its screens decides the indices below).
"""
import os
import subprocess
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SCREENS = {0: "connect-home", 4: "connect-wifi", 5: "connect-qr"}
HEIGHT = 1000  # output height in pixels


def main():
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(
            ["cargo", "test", "--manifest-path", str(ROOT / "app/Cargo.toml"), "renders_every_screen"],
            check=True,
            env=dict(os.environ, HANDSET_TEST_OUT=tmp),
        )
        frame = Image.open(ROOT / "app/assets/frame-obsidian.png").convert("RGBA")
        x, y, w, h = [int(v) for v in (ROOT / "app/assets/frame-obsidian.frame").read_text().split()]
        out = ROOT / "docs/images"
        out.mkdir(parents=True, exist_ok=True)
        for index, name in SCREENS.items():
            screen = Image.open(Path(tmp) / f"connect-{index}.png").convert("RGBA").resize((w, h), Image.LANCZOS)
            canvas = Image.new("RGBA", frame.size, (0, 0, 0, 0))
            canvas.paste(screen, (x, y))
            canvas.alpha_composite(frame)
            canvas = canvas.resize((round(frame.width * HEIGHT / frame.height), HEIGHT), Image.LANCZOS)
            canvas.save(out / f"{name}.png", optimize=True)
            print("wrote", out / f"{name}.png")


if __name__ == "__main__":
    main()
