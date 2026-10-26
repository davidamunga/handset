#!/usr/bin/env python3
"""Generate the built-in device bezel PNGs (+ .frame metadata).

Each frame is an RGBA PNG whose screen area is fully transparent. The sidecar `<name>.frame`
holds "x y w h" of that area in PNG pixels. Everything is drawn from scratch with Pillow: the
shapes are generic device types (punch-hole, notch, island, tablet, foldable...), not copies of
any maker's artwork.

    python3 frames/make_frames.py app/assets            # every built-in frame
    python3 frames/make_frames.py out punch-hole        # just one device type
    python3 frames/make_frames.py out --sheet           # also write a contact sheet, sheet.png

To make your own, add a `Device` and its colourways below, or draw a PNG with a transparent
screen area and write the `x y w h` of that area next to it (see docs/frames.md).
"""
import sys
from dataclasses import dataclass, field
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

SS = 3  # supersampling factor
MARGIN = 70  # transparent room for buttons + shadow


@dataclass
class Device:
    name: str
    screen: tuple  # w, h in pixels
    radius: int  # screen corner radius
    bezel: tuple  # left, top, right, bottom glass border around the screen
    rail: int  # metal frame thickness
    cutout: str = "none"  # punch | notch | island | drop | home | tablet | fold | flip | rugged-top
    # side buttons: (side, offset from the top of the body, length)
    buttons: tuple = (("r", 520, 170), ("r", 800, 300))
    body_inset: int = 8  # how much tighter the outer corner is than screen radius + bezel + rail
    body_radius: int = 0  # overrides the above when set
    rugged: bool = False
    colours: dict = field(default_factory=dict)  # colourway: (rail light, rail dark, glass)


DEVICES = [
    Device(
        "classic",
        (1080, 2400),
        96,
        (28, 28, 28, 28),
        14,
        "punch",
        # the original three frames keep their plain names
        colours={
            "obsidian": ((92, 95, 102), (28, 29, 33), (6, 6, 8)),
            "porcelain": ((236, 236, 232), (170, 170, 166), (14, 14, 16)),
            "hazel": ((176, 168, 140), (96, 90, 70), (8, 8, 8)),
        },
    ),
    Device(
        "notch",
        (1080, 2340),
        88,
        (26, 26, 26, 26),
        14,
        "notch",
        buttons=(("l", 420, 110), ("l", 580, 170), ("l", 780, 170), ("r", 640, 250)),
        colours={
            "graphite": ((122, 124, 130), (40, 41, 46), (0, 0, 0)),
            "silver": ((228, 230, 232), (176, 178, 182), (8, 8, 10)),
        },
    ),
    Device(
        "island",
        (1080, 2340),
        120,
        (22, 22, 22, 22),
        14,
        "island",
        buttons=(("l", 360, 80), ("l", 480, 150), ("l", 670, 150), ("r", 560, 230)),
        colours={
            "titanium": ((164, 156, 144), (92, 86, 78), (6, 6, 8)),
            "black": ((86, 88, 92), (30, 31, 34), (0, 0, 0)),
        },
    ),
    Device(
        "drop",
        (1080, 2340),
        56,
        (30, 56, 30, 70),
        14,
        "drop",
        colours={
            "midnight": ((76, 92, 136), (26, 32, 56), (4, 4, 8)),
            "coral": ((238, 154, 138), (172, 94, 86), (8, 6, 6)),
        },
    ),
    Device(
        "home",
        (1080, 1920),
        0,
        (40, 190, 40, 210),
        14,
        "home",
        body_radius=150,
        buttons=(("l", 380, 120), ("l", 540, 120), ("r", 460, 200)),
        colours={
            "black": ((206, 206, 210), (120, 120, 126), (10, 10, 12)),
            "white": ((236, 236, 234), (178, 178, 176), (246, 246, 246)),
        },
    ),
    Device(
        "flat",
        (1080, 2340),
        34,
        (22, 22, 22, 22),
        12,
        "punch",
        body_radius=58,
        buttons=(("r", 520, 140), ("r", 720, 240)),
        colours={
            "slate": ((142, 148, 156), (60, 64, 72), (4, 4, 6)),
            "sage": ((178, 198, 172), (98, 116, 94), (6, 8, 6)),
        },
    ),
    Device(
        "tablet",
        (1600, 2560),
        34,
        (70, 70, 70, 70),
        16,
        "tablet",
        body_radius=84,
        buttons=(("r", 300, 120), ("r", 460, 120)),
        colours={
            "space": ((112, 114, 120), (42, 44, 48), (6, 6, 8)),
            "silver": ((226, 228, 230), (176, 178, 182), (12, 12, 14)),
        },
    ),
    Device(
        "fold",
        (1800, 2160),
        30,
        (28, 28, 28, 28),
        14,
        "fold",
        body_radius=70,
        buttons=(("r", 420, 130), ("r", 660, 300)),
        colours={
            "black": ((82, 82, 88), (28, 28, 32), (0, 0, 0)),
            "cream": ((232, 224, 208), (178, 168, 148), (8, 8, 8)),
        },
    ),
    Device(
        "flip",
        (1080, 2640),
        70,
        (24, 24, 24, 24),
        12,
        "flip",
        body_radius=96,
        buttons=(("r", 700, 130), ("r", 900, 260)),
        colours={
            "lilac": ((198, 178, 226), (132, 110, 166), (4, 4, 6)),
            "black": ((88, 88, 94), (30, 30, 34), (0, 0, 0)),
        },
    ),
    Device(
        "rugged",
        (1080, 2160),
        22,
        (46, 120, 46, 150),
        34,
        "rugged-top",
        body_radius=96,
        rugged=True,
        buttons=(("r", 420, 150), ("r", 640, 260)),
        colours={
            "orange": ((236, 126, 34), (150, 72, 12), (6, 6, 8)),
            "olive": ((126, 138, 90), (70, 80, 48), (6, 6, 8)),
        },
    ),
]


def rrect(draw, box, r, fill, corners=None):
    draw.rounded_rectangle(box, radius=r, fill=fill, corners=corners)


def vgrad(size, top, bottom):
    w, h = size
    col = Image.new("RGB", (1, h))
    for y in range(h):
        t = y / max(1, h - 1)
        col.putpixel((0, y), tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3)))
    return col.resize((w, h))


def luma(c):
    return 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]


def build(d, light, dark, glass_col):
    s = SS
    sw, sh = d.screen
    bl, bt, br, bb = d.bezel
    body_w = sw + bl + br + 2 * d.rail
    body_h = sh + bt + bb + 2 * d.rail
    W, H = body_w + 2 * MARGIN, body_h + 2 * MARGIN
    body_r = d.body_radius or (d.radius + min(d.bezel) + d.rail - d.body_inset)
    bx, by = MARGIN, MARGIN

    img = Image.new("RGBA", (W * s, H * s), (0, 0, 0, 0))

    def layer():
        return Image.new("RGBA", img.size, (0, 0, 0, 0))

    # --- soft drop shadow
    sh_mask = Image.new("L", img.size, 0)
    rrect(ImageDraw.Draw(sh_mask), [bx * s, (by + 14) * s, (bx + body_w) * s, (by + body_h + 14) * s], body_r * s, 150)
    sh_mask = sh_mask.filter(ImageFilter.GaussianBlur(26 * s))
    shadow = layer()
    shadow.putalpha(sh_mask)
    img = Image.alpha_composite(img, shadow)

    # --- side buttons, drawn under the rail
    btn = layer()
    bd = ImageDraw.Draw(btn)
    for side, top, length in d.buttons:
        if side == "r":
            x0, x1 = bx + body_w - 6, bx + body_w + 9
        else:
            x0, x1 = bx - 9, bx + 6
        rrect(bd, [x0 * s, (by + top) * s, x1 * s, (by + top + length) * s], 6 * s, (*dark, 255))
        inner = [x0 * s, (by + top) * s, (x1 - 5) * s, (by + top + length) * s] if side == "r" else [
            (x0 + 5) * s,
            (by + top) * s,
            x1 * s,
            (by + top + length) * s,
        ]
        rrect(bd, inner, 5 * s, (*light, 255))
    img = Image.alpha_composite(img, btn)

    # --- metal rail with vertical gradient + edge highlight
    mask = Image.new("L", img.size, 0)
    rrect(ImageDraw.Draw(mask), [bx * s, by * s, (bx + body_w) * s, (by + body_h) * s], body_r * s, 255)
    grad = vgrad((body_w * s, body_h * s), light, dark).convert("RGBA")
    rail = layer()
    rail.paste(grad, (bx * s, by * s))
    rail.putalpha(mask)
    img = Image.alpha_composite(img, rail)

    edge = Image.new("L", img.size, 0)
    ed = ImageDraw.Draw(edge)
    rrect(ed, [bx * s, by * s, (bx + body_w) * s, (by + body_h) * s], body_r * s, 255)
    rrect(ed, [(bx + 3) * s, (by + 3) * s, (bx + body_w - 3) * s, (by + body_h - 3) * s], (body_r - 3) * s, 0)
    hi = Image.new("RGBA", img.size, (255, 255, 255, 0))
    hi.putalpha(edge.point(lambda v: int(v * 0.35)))
    img = Image.alpha_composite(img, hi)

    # --- rugged: a rubber bumper over the rail, with corner guards and screws
    if d.rugged:
        rub = layer()
        rd = ImageDraw.Draw(rub)
        t = 22
        outer = [bx * s, by * s, (bx + body_w) * s, (by + body_h) * s]
        rrect(rd, outer, body_r * s, (30, 30, 33, 255))
        rrect(rd, [(bx + t) * s, (by + t) * s, (bx + body_w - t) * s, (by + body_h - t) * s], (body_r - t) * s, (0, 0, 0, 0))
        # the inner cut-out must erase, not paint: punch it out of the ring
        ring = Image.new("L", img.size, 0)
        rrect(ImageDraw.Draw(ring), outer, body_r * s, 255)
        rrect(ImageDraw.Draw(ring), [(bx + t) * s, (by + t) * s, (bx + body_w - t) * s, (by + body_h - t) * s], (body_r - t) * s, 0)
        rub = layer()
        rub.paste(Image.new("RGBA", img.size, (30, 30, 33, 255)), (0, 0), ring)
        img = Image.alpha_composite(img, rub)
        guards = layer()
        gd = ImageDraw.Draw(guards)
        g = 150
        for gx, gy, corners in (
            (bx, by, (True, False, False, False)),
            (bx + body_w - g, by, (False, True, False, False)),
            (bx, by + body_h - g, (False, False, False, True)),
            (bx + body_w - g, by + body_h - g, (False, False, True, False)),
        ):
            rrect(gd, [gx * s, gy * s, (gx + g) * s, (gy + g) * s], body_r * s, (44, 44, 48, 255), corners)
        gmask = Image.new("L", img.size, 0)
        gm = ImageDraw.Draw(gmask)
        gm.rectangle([0, 0, img.size[0], img.size[1]], fill=255)
        guards.putalpha(ImageChops.multiply(guards.getchannel("A"), ring))
        img = Image.alpha_composite(img, guards)
        scr = layer()
        sd = ImageDraw.Draw(scr)
        for cx, cy in (
            (bx + 52, by + 52),
            (bx + body_w - 52, by + 52),
            (bx + 52, by + body_h - 52),
            (bx + body_w - 52, by + body_h - 52),
        ):
            sd.ellipse([(cx - 11) * s, (cy - 11) * s, (cx + 11) * s, (cy + 11) * s], fill=(150, 152, 158, 255))
            sd.line([(cx - 7) * s, cy * s, (cx + 7) * s, cy * s], fill=(50, 50, 54, 255), width=3 * s)
            sd.line([cx * s, (cy - 7) * s, cx * s, (cy + 7) * s], fill=(50, 50, 54, 255), width=3 * s)
        img = Image.alpha_composite(img, scr)

    # --- glass bezel
    ix, iy = bx + d.rail, by + d.rail
    iw, ih = body_w - 2 * d.rail, body_h - 2 * d.rail
    glass = layer()
    rrect(
        ImageDraw.Draw(glass),
        [ix * s, iy * s, (ix + iw) * s, (iy + ih) * s],
        max(body_r - d.rail, 4) * s,
        (*glass_col, 255),
    )
    img = Image.alpha_composite(img, glass)

    # --- punch the screen hole
    sx, sy = ix + bl, iy + bt
    hole = Image.new("L", img.size, 255)
    rrect(ImageDraw.Draw(hole), [sx * s, sy * s, (sx + sw) * s, (sy + sh) * s], d.radius * s, 0)
    img.putalpha(ImageChops.multiply(img.getchannel("A"), hole))

    # --- front details: camera cutouts over the screen, speakers and buttons on the glass
    det = layer()
    dd = ImageDraw.Draw(det)
    cx = sx + sw // 2
    detail = (46, 46, 52, 255) if luma(glass_col) < 100 else (206, 206, 210, 255)

    def dot(x, y, r, fill):
        dd.ellipse([(x - r) * s, (y - r) * s, (x + r) * s, (y + r) * s], fill=fill)

    def lens(x, y, r):
        dot(x, y, r + 3, (0, 0, 0, 255))
        dot(x, y, r * 0.5, (18, 22, 40, 255))
        dot(x, y, r * 0.27, (8, 10, 22, 255))
        dot(x - r * 0.3, y - r * 0.35, r * 0.2, (90, 110, 170, 200))

    c = d.cutout
    if c == "punch":
        lens(cx, sy + 62, 24)
    elif c == "island":
        rrect(dd, [(cx - 125) * s, (sy + 34) * s, (cx + 125) * s, (sy + 106) * s], 36 * s, (0, 0, 0, 255))
        lens(cx + 82, sy + 70, 14)
    elif c == "notch":
        rrect(dd, [(cx - 230) * s, (sy - 6) * s, (cx + 230) * s, (sy + 80) * s], 40 * s, (0, 0, 0, 255), (False, False, True, True))
        rrect(dd, [(cx - 62) * s, (sy + 22) * s, (cx + 62) * s, (sy + 34) * s], 6 * s, (34, 34, 38, 255))
        lens(cx + 120, sy + 36, 12)
    elif c == "drop":
        rrect(dd, [(cx - 84) * s, (sy - 6) * s, (cx + 84) * s, (sy + 64) * s], 36 * s, (0, 0, 0, 255), (False, False, True, True))
        lens(cx, sy + 28, 11)
    elif c == "home":
        mid = iy + (bt + 0) / 2 - 0
        rrect(dd, [(cx - 80) * s, (mid - 7) * s, (cx + 80) * s, (mid + 7) * s], 7 * s, detail)
        dot(cx - 170, mid, 12, detail)
        dot(cx + 130, mid, 6, detail)
        bmid = sy + sh + bb / 2
        dot(cx, bmid, 62, detail)
        dot(cx, bmid, 56, (*glass_col, 255))
        rrect(dd, [(cx - 22) * s, (bmid - 22) * s, (cx + 22) * s, (bmid + 22) * s], 8 * s, detail)
        rrect(dd, [(cx - 18) * s, (bmid - 18) * s, (cx + 18) * s, (bmid + 18) * s], 6 * s, (*glass_col, 255))
    elif c == "tablet":
        dot(cx, iy + bt / 2, 10, (22, 24, 34, 255))
        dot(cx, iy + bt / 2, 5, (8, 10, 22, 255))
    elif c == "fold":
        lens(sx + sw - 70, sy + 56, 16)
    elif c == "flip":
        lens(cx, sy + 62, 22)
    elif c == "rugged-top":
        mid = iy + bt / 2
        rrect(dd, [(cx - 90) * s, (mid - 8) * s, (cx + 90) * s, (mid + 8) * s], 8 * s, detail)
        dot(cx - 190, mid, 13, detail)
        dot(cx + 160, mid, 6, detail)
        bmid = sy + sh + bb / 2
        for k, x in enumerate((-220, 0, 220)):
            if k == 0:
                dd.polygon([((cx + x + 20) * s, (bmid - 24) * s), ((cx + x + 20) * s, (bmid + 24) * s), ((cx + x - 22) * s, bmid * s)], outline=detail, width=4 * s)
            elif k == 1:
                dd.ellipse([(cx + x - 24) * s, (bmid - 24) * s, (cx + x + 24) * s, (bmid + 24) * s], outline=detail, width=4 * s)
            else:
                dd.rounded_rectangle([(cx + x - 22) * s, (bmid - 22) * s, (cx + x + 22) * s, (bmid + 22) * s], radius=5 * s, outline=detail, width=4 * s)
    img = Image.alpha_composite(img, det)

    # --- a fold or hinge crease: faint, so touches and the picture still go through it
    if c in ("fold", "flip"):
        crease = layer()
        cd = ImageDraw.Draw(crease)
        if c == "fold":
            xm = sx + sw // 2
            cd.rectangle([(xm - 22) * s, sy * s, (xm + 22) * s, (sy + sh) * s], fill=(0, 0, 0, 22))
            cd.rectangle([(xm - 2) * s, sy * s, (xm + 2) * s, (sy + sh) * s], fill=(0, 0, 0, 60))
            cd.rectangle([(xm + 3) * s, sy * s, (xm + 6) * s, (sy + sh) * s], fill=(255, 255, 255, 26))
            # the hinge shows on the rail at the top and bottom
            for yy in (by, by + body_h - 6):
                cd.rectangle([(xm - 40) * s, yy * s, (xm + 40) * s, (yy + 6) * s], fill=(*dark, 255))
        else:
            ym = sy + sh // 2
            cd.rectangle([sx * s, (ym - 20) * s, (sx + sw) * s, (ym + 20) * s], fill=(0, 0, 0, 20))
            cd.rectangle([sx * s, (ym - 2) * s, (sx + sw) * s, (ym + 2) * s], fill=(0, 0, 0, 56))
            cd.rectangle([sx * s, (ym + 3) * s, (sx + sw) * s, (ym + 6) * s], fill=(255, 255, 255, 24))
            for xx in (bx - 4, bx + body_w - 4):
                cd.rectangle([xx * s, (ym - 36) * s, (xx + 8) * s, (ym + 36) * s], fill=(*dark, 255))
        img = Image.alpha_composite(img, crease)

    img = img.resize((W, H), Image.LANCZOS)
    return img, (sx, sy, sw, sh)


def all_frames():
    """(frame name, device, colourway colours)."""
    for d in DEVICES:
        for colour, cols in d.colours.items():
            yield (colour if d.name == "classic" else f"{d.name}-{colour}"), d, cols


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    sheet = "--sheet" in sys.argv
    out = Path(args[0] if args else ".")
    only = set(args[1:])
    out.mkdir(parents=True, exist_ok=True)
    made = []
    for name, d, (light, dark, glass) in all_frames():
        if only and d.name not in only and name not in only:
            continue
        img, (x, y, w, h) = build(d, light, dark, glass)
        img.save(out / f"frame-{name}.png", optimize=True)
        (out / f"frame-{name}.frame").write_text(f"{x} {y} {w} {h}\n")
        print("wrote", out / f"frame-{name}.png", (x, y, w, h), img.size)
        made.append((name, img))
    if sheet and made:
        th = 520
        tiles = [(n, i.resize((round(i.width * th / i.height), th), Image.LANCZOS)) for n, i in made]
        cols = 6
        rows = [tiles[k : k + cols] for k in range(0, len(tiles), cols)]
        width = max(sum(t.width for _, t in r) + 20 * (len(r) + 1) for r in rows)
        sheet_img = Image.new("RGBA", (width, (th + 40) * len(rows) + 20), (236, 238, 240, 255))
        y = 20
        for r in rows:
            x = 20
            for n, t in r:
                sheet_img.alpha_composite(t, (x, y))
                ImageDraw.Draw(sheet_img).text((x, y + th + 6), n, fill=(40, 40, 44, 255))
                x += t.width + 20
            y += th + 40
        sheet_img.save(out / "sheet.png")


if __name__ == "__main__":
    main()
