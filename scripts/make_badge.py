#!/usr/bin/env python3
"""Draws the Højte badge: site/hojt.svg (the badge) and site/favicon.svg (the badge centred on a
square). The letters are Bebas Neue's own outlines at 22 px, placed by rules, not by eye, and
scripts/make-icon.py takes the same geometry (`from make_badge import ...`). Run it again after a
change to the rules below; the two svg files are committed. `--png` also makes site/og-hojt.png,
apple-touch-icon.png and favicon-32.png from them, with headless Chrome. Needs fontTools.

The rules, all in badge units (the plate is 22 px of letter on 30.8 px of plate):
- H, J, T and E stand one `TEXT_GAP` apart, ink edge to ink edge (J's edge is its hook).
- The Ø is a ring, the height of the capitals, drawn as a knob. A circle touches its neighbour at
  one point, so its white space is wider than a stem's: it stands `RING_GAP` (twice `TEXT_GAP`)
  from H's stem and from J's hook.
- The whole line is centred on the plate, so both margins are equal.
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.pens.recordingPen import DecomposingRecordingPen

ROOT = Path(__file__).resolve().parent.parent
FONT = ROOT / "crates/suite_common/assets/fonts/BebasNeue-Regular.ttf"

SIZE = 22.0  # px of letter: the capitals are 0.7 of it high
PLATE_W, PLATE_H, DROP = 65.5, 30.8, 3.0
CAP_TOP, CAP_BOTTOM = 7.7, 23.1
TEXT_GAP = 1.1
RING_GAP = 2 * TEXT_GAP
RING_R, RING_STROKE = 6.6, 2.31
SLASH_STROKE = 1.87
SLASH_ANGLE = -55.0  # degrees, the pointer
SLASH_LEN = 22.0

PLATE = "#1C1A18"
SILK = "#EADFC8"
ORANGE = "#E2581E"


def outline(font, ch):
    """The contours of a capital as lists of points in badge units, x from the glyph's origin and y
    down from the top of the line, curves flattened to 8 segments each."""
    gs = font.getGlyphSet()
    name = font.getBestCmap()[ord(ch)]
    pen = DecomposingRecordingPen(gs)
    gs[name].draw(pen)
    k = SIZE / font["head"].unitsPerEm
    # Font y runs up from the baseline; the baseline is CAP_BOTTOM
    pt = lambda p: (p[0] * k, CAP_BOTTOM - p[1] * k)
    contours, cur = [], []
    for op, args in pen.value:
        if op == "moveTo":
            cur = [pt(args[0])]
        elif op == "lineTo":
            cur.append(pt(args[0]))
        elif op == "qCurveTo":
            # TrueType implied on-curve points between consecutive off-curve ones
            offs, end = list(args[:-1]), args[-1]
            start = cur[-1]
            pts = []
            for i, o in enumerate(offs):
                nxt = end if i == len(offs) - 1 else ((o[0] + offs[i + 1][0]) / 2, (o[1] + offs[i + 1][1]) / 2)
                pts.append((o, nxt))
            p0 = (start[0] / k, (CAP_BOTTOM - start[1]) / k)
            for o, nxt in pts:
                for s in range(1, 9):
                    t = s / 8
                    x = (1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * o[0] + t * t * nxt[0]
                    y = (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * o[1] + t * t * nxt[1]
                    cur.append(pt((x, y)))
                p0 = nxt
        elif op in ("closePath", "endPath"):
            contours.append(cur)
            cur = []
    ink_left = min(x for c in contours for x, _ in c)
    ink_right = max(x for c in contours for x, _ in c)
    return contours, ink_left, ink_right


def geometry():
    """Where everything stands: the contours of each letter already moved into place, and the ring"""
    font = TTFont(FONT)
    letters = {ch: outline(font, ch) for ch in "HJTE"}
    ring_outer = RING_R * 2 + RING_STROKE
    widths = {ch: letters[ch][2] - letters[ch][1] for ch in letters}
    total = (
        widths["H"] + RING_GAP + ring_outer + RING_GAP + widths["J"]
        + TEXT_GAP + widths["T"] + TEXT_GAP + widths["E"]
    )
    x = (PLATE_W - total) / 2
    placed = {}

    def put(ch):
        nonlocal x
        cs, left, right = letters[ch]
        placed[ch] = [[(px - left + x, py) for px, py in c] for c in cs]
        x += right - left

    put("H")
    x += RING_GAP
    ring_cx = x + ring_outer / 2
    x += ring_outer + RING_GAP
    put("J")
    x += TEXT_GAP
    put("T")
    x += TEXT_GAP
    put("E")
    return placed, ring_cx, (PLATE_W - total) / 2


def slash(cx, cy=(CAP_TOP + CAP_BOTTOM) / 2):
    import math

    a = math.radians(SLASH_ANGLE)
    dx, dy = math.cos(a) * SLASH_LEN / 2, math.sin(a) * SLASH_LEN / 2
    return cx - dx, cy - dy, cx + dx, cy + dy


def path_d(placed):
    out = []
    for ch in "HJTE":
        for c in placed[ch]:
            out.append("M" + " ".join(f"{x:.3f},{y:.3f}" for x, y in c) + "z")
    return " ".join(out)


def plate_svg(placed, ring_cx, indent=""):
    cy = (CAP_TOP + CAP_BOTTOM) / 2
    x1, y1, x2, y2 = slash(ring_cx)
    inner = (PLATE_W - 5.5, PLATE_H - 5.5)
    lines = [
        f'<rect x="0" y="{DROP}" width="{PLATE_W}" height="{PLATE_H}" rx="6" ry="6" fill="#000000" fill-opacity="0.314"/>',
        f'<rect x="0" y="0" width="{PLATE_W}" height="{PLATE_H}" rx="6" ry="6" fill="{PLATE}"/>',
        f'<rect x="2.75" y="2.75" width="{inner[0]}" height="{inner[1]}" rx="3.5" ry="3.5" fill="none" stroke="{SILK}" stroke-width="1.5"/>',
        f'<path fill="{SILK}" d="{path_d(placed)}"/>',
        f'<circle cx="{ring_cx:.3f}" cy="{cy}" r="{RING_R}" fill="none" stroke="{SILK}" stroke-width="{RING_STROKE}"/>',
        f'<line x1="{x1:.3f}" y1="{y1:.3f}" x2="{x2:.3f}" y2="{y2:.3f}" stroke="{ORANGE}" stroke-width="{SLASH_STROKE}" stroke-linecap="butt"/>',
    ]
    return "\n".join(indent + l for l in lines)


def render_png(html, out, width, height, transparent=False):
    """A screenshot of `html` by headless Chrome, `width` x `height` px"""
    chrome = shutil.which("google-chrome") or shutil.which("chromium") or shutil.which("chromium-browser")
    if not chrome:
        sys.exit("--png needs google-chrome or chromium")
    with tempfile.TemporaryDirectory() as tmp:
        page = Path(tmp) / "p.html"
        page.write_text(html, encoding="utf-8")
        cmd = [chrome, "--headless", "--no-sandbox", "--disable-gpu", "--hide-scrollbars",
               f"--screenshot={out}", f"--window-size={width},{height}"]
        if transparent:
            cmd.append("--default-background-color=00000000")
        subprocess.run(cmd + [page.as_uri()], check=True, capture_output=True)


def pngs(site):
    """site/og-hojt.png (the social image), apple-touch-icon.png and favicon-32.png from the svgs"""
    bg = "#121110"
    badge, fav = (site / "hojt.svg").as_uri(), (site / "favicon.svg").as_uri()

    def page(src, w, h, margin_w, color):
        bh = margin_w * (PLATE_H + DROP) / PLATE_W
        return (f'<body style="margin:0;background:{color};width:{w}px;height:{h}px;overflow:hidden">'
                f'<img src="{src}" style="position:absolute;left:{(w - margin_w) / 2}px;top:{(h - bh) / 2}px" width="{margin_w}"></body>')

    render_png(page(badge, 1200, 630, 750, bg), site / "og-hojt.png", 1200, 630)
    render_png(page(badge, 180, 180, 156, bg), site / "apple-touch-icon.png", 180, 180)
    render_png(
        f'<body style="margin:0;background:transparent"><img src="{fav}" width="32" height="32" style="display:block"></body>',
        site / "favicon-32.png", 32, 32, transparent=True)


def main():
    placed, ring_cx, margin = geometry()
    h = PLATE_H + DROP
    badge = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {PLATE_W} {h}" role="img" aria-label="Højte">\n'
        "<!-- Made by scripts/make_badge.py: Bebas Neue's H, J, T and E, the Ø as a knob with its pointer -->\n"
        + plate_svg(placed, ring_cx)
        + "\n</svg>\n"
    )
    side = PLATE_W
    fav = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {side} {side}">\n'
        "<!-- The whole badge (hojt.svg) centred on a square. Made by scripts/make_badge.py -->\n"
        f'<g transform="translate(0 {(side - h) / 2:.2f})">\n'
        + plate_svg(placed, ring_cx)
        + "\n</g>\n</svg>\n"
    )
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = Path(args[0]) if args else ROOT / "site"
    (out / "hojt.svg").write_text(badge, encoding="utf-8")
    (out / "favicon.svg").write_text(fav, encoding="utf-8")
    if "--png" in sys.argv:
        pngs(out)
    print(f"wrote hojt.svg and favicon.svg to {out}; ring at x={ring_cx:.3f}, margins {margin:.3f}")


if __name__ == "__main__":
    main()
