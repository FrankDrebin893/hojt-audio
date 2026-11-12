#!/usr/bin/env python3
"""Makes plugins/amp/assets/amp.ico: the HØJT badge, drawn once per size from the geometry of
site/hojt.svg (the badge) and site/favicon.svg (the badge centred on a square), not scaled from
one picture. Run it again after the badge changes; the .ico is committed.

Each size is drawn on a canvas 4x as large and reduced, so the edges are clean, and is RGBA with a
transparent surround. At 32 px and up it is the whole badge with its drop shadow. Below that (16
and 24) the small print is dropped, the frame widened and the H made larger, since four letters
and a hairline frame turn to mud in so few pixels: a dark plate, a cream frame and the H.

The file is written by hand (a header and PNG images), so that running the script twice gives the
same bytes. Needs Pillow.
"""
import io
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent.parent / "plugins/amp/assets/amp.ico"
SIZES = [16, 24, 32, 48, 64, 128, 256]
SS = 4  # supersampling

PLATE = (0x1C, 0x1A, 0x18, 255)
SILK = (0xEA, 0xDF, 0xC8, 255)
ORANGE = (0xE2, 0x58, 0x1E, 255)
SHADOW = (0, 0, 0, round(0.314 * 255))

# From site/hojt.svg. The plate is 63 x 30.8 in a viewBox of 63 x 33.8 (the shadow is 3 below).
W, H_PLATE, DROP = 63.0, 30.8, 3.0
GLYPH_H = "M10.142,7.7L12.562,7.7 12.562,13.97 15.158,13.97 15.158,7.7 17.578,7.7 17.578,23.1 15.158,23.1 15.158,16.17 12.562,16.17 12.562,23.1 10.142,23.1 10.142,7.7z"
GLYPH_J = "M38.774,23.188L38.163,23.166 37.652,23.1 37.652,20.9 38.026,20.966 38.444,20.988 38.784,20.965 39.076,20.897 39.323,20.784 39.522,20.625 39.676,20.419 39.786,20.166 39.852,19.864 39.874,19.514 39.874,7.7 42.294,7.7 42.294,19.448 42.28,19.906 42.24,20.333 42.172,20.731 42.077,21.098 41.955,21.435 41.805,21.742 41.629,22.018 41.425,22.264 40.933,22.668 40.328,22.957 39.608,23.13 38.774,23.188z"
GLYPH_T = "M48.794,9.9L46.264,9.9 46.264,7.7 53.744,7.7 53.744,9.9 51.214,9.9 51.214,23.1 48.794,23.1 48.794,9.9z"


def polygon(d):
    """The points of a path made only of M, L and z with absolute coordinates"""
    pts = []
    for tok in d.replace("M", " ").replace("L", " ").replace("z", " ").replace(",", " ").split():
        pts.append(float(tok))
    return list(zip(pts[0::2], pts[1::2]))


def draw_size(px):
    small = px < 32
    n = px * SS
    k = n / W  # one badge unit in pixels
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))

    def pt(p, oy):
        return (p[0] * k, (p[1] + oy) * k)

    def rrect(layer, x0, y0, x1, y1, r, fill, oy):
        ImageDraw.Draw(layer).rounded_rectangle(
            [x0 * k, (y0 + oy) * k, x1 * k - 1, (y1 + oy) * k - 1], radius=r * k, fill=fill
        )

    # Vertical centring: the whole badge with its shadow, or the plate alone when small
    oy = (W - H_PLATE) / 2 if small else (W - (H_PLATE + DROP)) / 2

    if not small:
        shadow = Image.new("RGBA", (n, n), (0, 0, 0, 0))
        rrect(shadow, 0, DROP, W, H_PLATE + DROP, 6, SHADOW, oy)
        img = Image.alpha_composite(img, shadow)

    def layer(fn):
        nonlocal img
        lay = Image.new("RGBA", (n, n), (0, 0, 0, 0))
        fn(lay, ImageDraw.Draw(lay))
        img = Image.alpha_composite(img, lay)

    layer(lambda lay, d: rrect(lay, 0, 0, W, H_PLATE, 6, PLATE, oy))

    # The silk frame: a stroke centred on a rect inset 2.75 (1.5 wide), as an outer minus an inner
    if small:
        sw = W / px  # a frame one pixel wide, at the edge of the plate
        inset, rad = sw / 2, 5.0
    else:
        sw, inset, rad = 1.5, 2.75, 3.5
    half = sw / 2

    def frame(lay, d):
        rrect(lay, inset - half, inset - half, W - inset + half, H_PLATE - inset + half, rad + half, SILK, oy)
        inner = Image.new("RGBA", (n, n), (0, 0, 0, 0))
        rrect(inner, inset + half, inset + half, W - inset - half, H_PLATE - inset - half, max(rad - half, 0.5), PLATE, oy)
        # cut the inner out of the frame layer (replace, not blend)
        mask = inner.getchannel("A")
        lay.paste((0, 0, 0, 0), mask=mask)
        lay.paste(inner, mask=mask)

    layer(frame)

    if small:
        # The H alone, larger, in the middle of the plate
        pts = polygon(GLYPH_H)
        cx = (10.142 + 17.578) / 2
        cy = (7.7 + 23.1) / 2
        sx, sy = W / px * 1.0, 1.15  # stems about 2.4 px wide, whatever the size
        pts = [((x - cx) * sx + W / 2, (y - cy) * sy + H_PLATE / 2) for x, y in pts]
        layer(lambda lay, d: d.polygon([pt(p, oy) for p in pts], fill=SILK))
    else:
        for g in (GLYPH_H, GLYPH_J, GLYPH_T):
            layer(lambda lay, d, g=g: d.polygon([pt(p, oy) for p in polygon(g)], fill=SILK))

        # The O: a ring at (27.6, 15.4), radius 6.6, stroke 2.31
        def ring(lay, d):
            cx, cy, r, w = 27.6, 15.4, 6.6, 2.31
            ro, ri = r + w / 2, r - w / 2
            d.ellipse([(cx - ro) * k, (cy - ro + oy) * k, (cx + ro) * k, (cy + ro + oy) * k], fill=SILK)
            d.ellipse([(cx - ri) * k, (cy - ri + oy) * k, (cx + ri) * k, (cy + ri + oy) * k], fill=(0, 0, 0, 0))

        layer(lambda lay, d: ring(lay, d))

        # The pointer: (21.29, 24.41) to (33.91, 6.39), stroke 1.87, butt caps
        def pointer(lay, d):
            x1, y1, x2, y2, w = 21.29, 24.41, 33.91, 6.39, 1.87
            dx, dy = x2 - x1, y2 - y1
            ln = (dx * dx + dy * dy) ** 0.5
            nx, ny = -dy / ln * w / 2, dx / ln * w / 2
            quad = [(x1 + nx, y1 + ny), (x2 + nx, y2 + ny), (x2 - nx, y2 - ny), (x1 - nx, y1 - ny)]
            d.polygon([pt(p, oy) for p in quad], fill=ORANGE)

        layer(pointer)

    return img.resize((px, px), Image.LANCZOS)


def write_ico(images):
    blobs = []
    for im in images:
        buf = io.BytesIO()
        im.save(buf, "PNG", optimize=False, compress_level=9)
        blobs.append(buf.getvalue())
    head = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    for im, blob in zip(images, blobs):
        w = im.width if im.width < 256 else 0
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    return head + entries + b"".join(blobs)


def main():
    images = [draw_size(s) for s in SIZES]
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else OUT
    out.write_bytes(write_ico(images))
    # Renders for looking at: make-icon.py <out.ico> also writes <out>-<size>.png
    if len(sys.argv) > 1:
        for s, im in zip(SIZES, images):
            im.save(out.with_name(f"{out.stem}-{s}.png"))
    print(f"wrote {out} ({out.stat().st_size} bytes, sizes {SIZES})")


if __name__ == "__main__":
    main()
