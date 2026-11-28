#!/usr/bin/env python3
"""Makes plugins/<name>/assets/<name>.ico for the six standalone programs: the Højte badge, drawn once per size from the geometry of
scripts/make_badge.py (which writes site/hojt.svg and site/favicon.svg), not scaled from
one picture. Run it again after the badge changes; the .ico files are committed. Every plugin's is the
same badge, so the six files are the same bytes (xtask/tests/standalones.rs holds it).

Each size is drawn on a canvas 4x as large and reduced, so the edges are clean, and is RGBA with a
transparent surround. At 32 px and up it is the whole badge with its drop shadow. Below that (16
and 24) the small print is dropped, the frame widened and the H made larger, since five letters
and a hairline frame turn to mud in so few pixels: a dark plate, a cream frame and the H.

The file is written by hand (a header and PNG images), so that running the script twice gives the
same bytes. Needs Pillow.
"""
import io
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import make_badge as mb  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PLUGINS = ["pitch_shifter", "drums", "amp", "bass", "pad", "bells"]
SIZES = [16, 24, 32, 48, 64, 128, 256]
SS = 4  # supersampling

PLATE = (0x1C, 0x1A, 0x18, 255)
SILK = (0xEA, 0xDF, 0xC8, 255)
ORANGE = (0xE2, 0x58, 0x1E, 255)
SHADOW = (0, 0, 0, round(0.314 * 255))

# The badge's geometry is scripts/make_badge.py's: the same letters, the same ring, the same pointer
W, H_PLATE, DROP = mb.PLATE_W, mb.PLATE_H, mb.DROP
PLACED, RING_CX, _MARGIN = mb.geometry()
RING_CY = (mb.CAP_TOP + mb.CAP_BOTTOM) / 2
SLASH = mb.slash(RING_CX)


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
        pts = PLACED["H"][0]
        cx = (min(x for x, _ in pts) + max(x for x, _ in pts)) / 2
        cy = (mb.CAP_TOP + mb.CAP_BOTTOM) / 2
        sx, sy = W / px * 1.0, 1.15  # stems about 2.4 px wide, whatever the size
        pts = [((x - cx) * sx + W / 2, (y - cy) * sy + H_PLATE / 2) for x, y in pts]
        layer(lambda lay, d: d.polygon([pt(p, oy) for p in pts], fill=SILK))
    else:
        for ch in "HJTE":
            for contour in PLACED[ch]:
                layer(lambda lay, d, c=contour: d.polygon([pt(p, oy) for p in c], fill=SILK))

        # The Ø: a ring at the ring centre, radius 6.6, stroke 2.31
        def ring(lay, d):
            cx, cy, r, w = RING_CX, RING_CY, mb.RING_R, mb.RING_STROKE
            ro, ri = r + w / 2, r - w / 2
            d.ellipse([(cx - ro) * k, (cy - ro + oy) * k, (cx + ro) * k, (cy + ro + oy) * k], fill=SILK)
            d.ellipse([(cx - ri) * k, (cy - ri + oy) * k, (cx + ri) * k, (cy + ri + oy) * k], fill=(0, 0, 0, 0))

        layer(lambda lay, d: ring(lay, d))

        # The pointer: a butt-capped line across the ring
        def pointer(lay, d):
            (x1, y1, x2, y2), w = SLASH, mb.SLASH_STROKE
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
    data = write_ico(images)
    if len(sys.argv) > 1:
        out = Path(sys.argv[1])
        out.write_bytes(data)
    else:
        for name in PLUGINS:
            (ROOT / f"plugins/{name}/assets/{name}.ico").parent.mkdir(exist_ok=True)
            (ROOT / f"plugins/{name}/assets/{name}.ico").write_bytes(data)
    # Renders for looking at: make-icon.py <out.ico> also writes <out>-<size>.png
    if len(sys.argv) > 1:
        for s, im in zip(SIZES, images):
            im.save(out.with_name(f"{out.stem}-{s}.png"))
    print(f"wrote {len(data)} bytes, sizes {SIZES}, to " + (str(out) if len(sys.argv) > 1 else "the six plugins' assets"))


if __name__ == "__main__":
    main()
