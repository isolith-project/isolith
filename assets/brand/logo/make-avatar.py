#!/usr/bin/env python3
"""Build the Isolith organization avatar from the committed mark.

The avatar is a derived export: the committed raster mark
(logo/png/isolith-mark-1024.png) cropped to its ink bounds, scaled to 620 px tall
and centered on a 1024 x 1024 paper tile (#F6F4EF). Two reasons for each choice:

- Crop to ink: the committed mark carries uneven transparent margins, so placing
  it raw leaves the cube off center inside GitHub's circular crop.
- 620 of 1024: the cube stays inside the circle's inscribed square with room to
  spare, and the shape still reads at 32 px in a timeline.

GitHub has no API for an organization avatar; the file this writes is uploaded by
hand under Organization settings, Profile. Keep this script as the record of how
the live avatar was produced.

Requires Pillow.
"""

import pathlib
import sys

from PIL import Image

LANCZOS = Image.Resampling.LANCZOS

PAPER = (246, 244, 239, 255)
TILE = 1024
MARK_HEIGHT = 620

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE / "png/isolith-mark-1024.png"
OUTPUT = HERE / "png/isolith-avatar-1024.png"


def main() -> int:
    mark = Image.open(SOURCE).convert("RGBA")
    ink = mark.split()[3].getbbox()
    if ink is None:
        print(f"{SOURCE} has no visible pixels", file=sys.stderr)
        return 1
    mark = mark.crop(ink)

    scale = MARK_HEIGHT / mark.height
    mark = mark.resize((round(mark.width * scale), MARK_HEIGHT), LANCZOS)

    tile = Image.new("RGBA", (TILE, TILE), PAPER)
    tile.alpha_composite(mark, ((TILE - mark.width) // 2, (TILE - mark.height) // 2))
    tile.convert("RGB").save(OUTPUT, "PNG", optimize=True)
    print(f"wrote {OUTPUT} ({TILE}x{TILE}, mark {mark.width}x{mark.height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
