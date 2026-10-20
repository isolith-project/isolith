#!/usr/bin/env python3
"""P10-M5-T02 - render a `script -T` terminal typescript into an MP4.

Minimal ANSI terminal emulator (16-color + bold) rendering each distinct
screen to PNG frames with PIL, then piping frames to ffmpeg/libx264 to make a
playable MP4 of the recorded Isolith install walkthrough.

Usage:
  python3 p10m5t02-video-render.py <typescript> <timing> <out.mp4>

The renderer elides long idle spans: it emits a frame only when the terminal
screen content changes, and holds each distinct screen ~0.7s, so a 50-minute
release build compresses to a readable couple of seconds of video.
"""
import argparse
import re
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

FONT_PATH = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
COLS, ROWS = 112, 40
CELL_W, CELL_H = 12, 22  # ~ DejaVu Sans Mono at size 20

BASIC = [(0, 0, 0), (170, 0, 0), (0, 170, 0), (170, 85, 0),
         (0, 0, 170), (170, 0, 170), (0, 170, 170), (192, 192, 192)]
BRIGHT = [(85, 85, 85), (255, 85, 85), (85, 255, 85), (255, 255, 85),
          (85, 85, 255), (255, 85, 255), (85, 255, 255), (255, 255, 255)]


class Term:
    def __init__(self):
        self.screen = [[" " for _ in range(COLS)] for _ in range(ROWS)]
        self.fg = [[(220, 220, 220) for _ in range(COLS)] for _ in range(ROWS)]
        self.bg = [[(16, 16, 20) for _ in range(COLS)] for _ in range(ROWS)]
        self.bold = [[False for _ in range(COLS)] for _ in range(ROWS)]
        self.r, self.c = 0, 0
        self.fgcol, self.bgcol, self.isbold = (220, 220, 220), (16, 16, 20), False
        self.saved = (0, 0)

    def _put(self, ch):
        if self.c >= COLS:
            self.c = 0
            self.r = min(self.r + 1, ROWS - 1)
        if self.c < COLS and self.r < ROWS:
            self.screen[self.r][self.c] = ch
            self.fg[self.r][self.c] = self.fgcol
            self.bg[self.r][self.c] = self.bgcol
            self.bold[self.r][self.c] = self.isbold
            self.c += 1

    def feed(self, data):
        i, n = 0, len(data)
        while i < n:
            ch = data[i]
            if ch == "\x1b":
                m = re.match(r"\x1b\[([0-9;?]*)([A-Za-z])", data[i:])
                if m:
                    i += m.end()
                    self._csi(m.group(1), m.group(2))
                    continue
                m2 = re.match(r"\x1b\][^\x07]*(\x07|\x1b\\)", data[i:])
                i += m2.end() if m2 else 1
                continue
            elif ch == "\r":
                self.c = 0
            elif ch == "\n":
                self.r = min(self.r + 1, ROWS - 1)
            elif ch == "\b":
                self.c = max(self.c - 1, 0)
            elif ch == "\x07" or "\x0e" <= ch <= "\x0f" or ord(ch) < 32:
                pass
            else:
                self._put(ch)
            i += 1

    def _csi(self, params, final):
        if final == "A":
            self.r = max(self.r - int(params or 1), 0)
        elif final == "B":
            self.r = min(self.r + int(params or 1), ROWS - 1)
        elif final == "C":
            self.c = min(self.c + int(params or 1), COLS - 1)
        elif final == "D":
            self.c = max(self.c - int(params or 1), 0)
        elif final == "G":
            self.c = max(int(params or 1) - 1, 0)
        elif final == "d":
            self.r = max(int(params or 1) - 1, 0)
        elif final in ("H", "f"):
            parts = (params or "1;1").split(";")
            self.r = max(min(int(parts[0] or 1) - 1, ROWS - 1), 0)
            self.c = max(min(int(parts[1] if len(parts) > 1 else 1) - 1, COLS - 1), 0)
        elif final == "J":
            if params == "2":
                for r in range(ROWS):
                    for cc in range(COLS):
                        self.screen[r][cc] = " "
                        self.fg[r][cc] = self.fgcol
                        self.bg[r][cc] = self.bgcol
            elif params == "1":
                for cc in range(0, self.c):
                    self.screen[self.r][cc] = " "
            else:
                for cc in range(self.c, COLS):
                    self.screen[self.r][cc] = " "
        elif final == "K":
            if params == "1":
                for cc in range(0, self.c):
                    self.screen[self.r][cc] = " "
            elif params == "2":
                for cc in range(COLS):
                    self.screen[self.r][cc] = " "
            else:
                for cc in range(self.c, COLS):
                    self.screen[self.r][cc] = " "
        elif final == "m":
            if not params:
                self._reset()
            else:
                for p in params.split(";"):
                    self._sgr(int(p))
        elif final == "s":
            self.saved = (self.r, self.c)
        elif final == "u":
            self.r, self.c = self.saved

    def _reset(self):
        self.fgcol, self.bgcol, self.isbold = (220, 220, 220), (16, 16, 20), False

    def _sgr(self, code):
        if code == 0:
            self._reset()
        elif code == 1:
            self.isbold = True
        elif code == 7:
            self.fgcol, self.bgcol = self.bgcol, self.fgcol
        elif 30 <= code <= 37:
            self.fgcol = BASIC[code - 30]
        elif 90 <= code <= 97:
            self.fgcol = BRIGHT[code - 90]
        elif 40 <= code <= 47:
            self.bgcol = BASIC[code - 40]
        elif 100 <= code <= 107:
            self.bgcol = BRIGHT[code - 100]


def render_frame(term):
    font = ImageFont.truetype(FONT_PATH, 20)
    w, h = COLS * CELL_W, ROWS * CELL_H
    img = Image.new("RGB", (w, h), (10, 10, 14))
    d = ImageDraw.Draw(img)
    for r in range(ROWS):
        for cc in range(COLS):
            ch = term.screen[r][cc]
            if ch == " " and term.bg[r][cc] == (16, 16, 20):
                continue
            fg = term.fg[r][cc]
            if term.bold[r][cc]:
                fg = tuple(min(c + 60, 255) for c in fg)
            x0, y0 = cc * CELL_W, r * CELL_H
            d.rectangle([x0, y0, x0 + CELL_W - 1, y0 + CELL_H - 1], fill=term.bg[r][cc])
            d.text((x0 + 1, y0 + 1), ch, font=font, fill=fg)
    return img.resize((w * 2, h * 2), Image.NEAREST)


def read_timing(path):
    out = []
    with open(path) as fh:
        for line in fh:
            p = line.split()
            if len(p) >= 2:
                out.append((float(p[0]), int(p[1])))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("typescript")
    ap.add_argument("timing")
    ap.add_argument("out")
    args = ap.parse_args()

    raw = open(args.typescript, "rb").read().decode("utf-8", "replace")
    timing = read_timing(args.timing)
    se = 0
    term = Term()
    CAPTURE, HOLD = 0.3, 7  # candidate capture 0.3s; hold each screen 0.7s
    next_cap, elapsed, last_key, frames = 0.0, 0.0, None, 0
    ffmpeg = subprocess.Popen(
        ["ffmpeg", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
         "-s", f"{COLS * CELL_W * 2}x{ROWS * CELL_H * 2}", "-r", "10", "-i", "-",
         "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
         "-movflags", "+faststart", args.out],
        stdin=subprocess.PIPE)
    for delay, n in timing:
        elapsed += delay
        chunk = raw[se:se + n]
        se += n
        if chunk:
            term.feed(chunk)
        while elapsed >= next_cap:
            key = "".join("".join(row) for row in term.screen)
            if key != last_key:
                frame = render_frame(term).tobytes()
                for _ in range(HOLD):
                    ffmpeg.stdin.write(frame)
                frames += HOLD
                last_key = key
            next_cap += CAPTURE
    frame = render_frame(term).tobytes()
    for _ in range(HOLD):
        ffmpeg.stdin.write(frame)
    ffmpeg.stdin.close()
    ret = ffmpeg.wait(timeout=300)
    print(f"render: session {elapsed:.1f}s -> {args.out}, {frames} frames (rc={ret})")
    sys.exit(0 if ret == 0 else 1)


if __name__ == "__main__":
    main()
