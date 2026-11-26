#!/usr/bin/env python3
"""Compose the Isolith README banner from the committed brand assets.

The mark geometry is never rebuilt: the committed lockup SVG
(assets/brand/logo/isolith-lockup.svg) is inlined verbatim and only transformed into
place. Outputs, next to this script: banner.svg (editable source) and banner.png
(2x raster, the file the README loads).

Brand rules applied (assets/brand/USAGE.md): paper #F6F4EF background, ink #0F2A43
text, slate #546981 secondary, copper #C8792B as the single accent, no gradients,
no shadows, clearspace wider than 0.5 x mark height around the lockup.

Requires Space Grotesk and Inter installed on the rendering machine, plus
google-chrome. Without the fonts Chrome substitutes a fallback and the wordmark
stops matching the brand.
"""

import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LOCKUP = ROOT / "assets/brand/logo/isolith-lockup.svg"

W, H = 1280, 330
GRID = 32
LOCKUP_W = 440.0
LOCKUP_VB_W, LOCKUP_VB_H = 340.0, 96.0
SCALE = LOCKUP_W / LOCKUP_VB_W
LOCKUP_H = LOCKUP_VB_H * SCALE
LX = (W - LOCKUP_W) / 2
LY = 54.0

TAGLINE = "the open engineering platform"
CAPS = "SELF-HOSTED  ·  AIR-GAP READY  ·  NO PER-SEAT FEES"


def lockup_inner() -> str:
    svg = LOCKUP.read_text(encoding="utf-8")
    inner = svg.split(">", 1)[1].rsplit("</svg>", 1)[0]
    inner = re.sub(r"<title.*?</title>|<desc.*?</desc>", "", inner, flags=re.S)
    return inner.strip()


def banner_svg() -> str:
    grid = [f'<line x1="{x}" y1="0" x2="{x}" y2="{H}"/>' for x in range(0, W + 1, GRID)]
    grid += [f'<line x1="0" y1="{y}" x2="{W}" y2="{y}"/>' for y in range(0, H + 1, GRID)]
    grid_svg = "\n    ".join(grid)

    rule_y = LY + LOCKUP_H + 22
    rule_w = 64
    tag_y = rule_y + 42
    caps_y = tag_y + 38

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" aria-labelledby="bt bd">
  <title id="bt">Isolith: the open engineering platform</title>
  <desc id="bd">The Isolith lockup on blueprint paper, with the tagline "the open engineering platform" and the line self-hosted, air-gap ready, no per-seat fees.</desc>
  <rect width="{W}" height="{H}" fill="#F6F4EF"/>
  <g stroke="#7C94AD" stroke-width="1" stroke-opacity="0.13">
    {grid_svg}
  </g>
  <g transform="translate({LX:.2f},{LY:.2f}) scale({SCALE:.5f})">
    {lockup_inner()}
  </g>
  <rect x="{(W - rule_w) / 2:.1f}" y="{rule_y}" width="{rule_w}" height="3" fill="#C8792B"/>
  <text x="{W / 2}" y="{tag_y}" fill="#0F2A43" text-anchor="middle"
        font-family="'Space Grotesk','IBM Plex Sans',system-ui,sans-serif"
        font-size="30" font-weight="500" letter-spacing="1.2">{TAGLINE}</text>
  <text x="{W / 2}" y="{caps_y}" fill="#546981" text-anchor="middle"
        font-family="'Inter','IBM Plex Sans',system-ui,sans-serif"
        font-size="13" font-weight="600" letter-spacing="2.4">{CAPS}</text>
</svg>
"""


def main() -> int:
    svg_path = HERE / "banner.svg"
    svg_path.write_text(banner_svg(), encoding="utf-8")

    html_path = HERE / ".banner-render.html"
    html_path.write_text(
        f"""<!DOCTYPE html><html><head><meta charset="utf-8">
<style>html,body{{margin:0;padding:0;background:#F6F4EF}}
img{{display:block;width:{W * 2}px;height:{H * 2}px}}</style></head>
<body><img src="file://{svg_path}" width="{W * 2}" height="{H * 2}"></body></html>""",
        encoding="utf-8",
    )

    png_path = HERE / "banner.png"
    subprocess.run(
        [
            "google-chrome", "--headless", "--disable-gpu", "--hide-scrollbars",
            "--default-background-color=F6F4EFFF",
            f"--screenshot={png_path}",
            f"--window-size={W * 2},{H * 2}",
            "--force-device-scale-factor=1",
            f"file://{html_path}",
        ],
        check=True, capture_output=True, text=True,
    )
    html_path.unlink()
    print(f"wrote {svg_path}")
    print(f"wrote {png_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
