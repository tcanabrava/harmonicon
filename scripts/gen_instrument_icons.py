#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Draw the Select Instrument screen's icons.

Writes `assets/images/instruments/<slug>.png`, one per
`harmonicon_core::instrument::InstrumentKind`. Each is a white silhouette on
transparency; the game tints it with the theme's text colour
(`ImageNode::color`), so a single file serves the light and dark themes.

Shapes are drawn at 4x and downsampled for antialiasing. Re-run after
changing a drawing:

    python3 scripts/gen_instrument_icons.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 256
SCALE = 4
S = SIZE * SCALE
INK = (255, 255, 255, 255)
CLEAR = (0, 0, 0, 0)
OUT = Path(__file__).resolve().parent.parent / "assets" / "images" / "instruments"


def canvas():
    image = Image.new("RGBA", (S, S), CLEAR)
    return image, ImageDraw.Draw(image)


def finish(image, angle=0.0):
    if angle:
        image = image.rotate(angle, resample=Image.BICUBIC)
    return image.resize((SIZE, SIZE), Image.LANCZOS)


def harmonica():
    image, d = canvas()
    d.rounded_rectangle((90, 380, 934, 644), radius=60, fill=INK)
    # The comb between the cover plates, and its ten holes.
    d.rectangle((90, 480, 934, 544), fill=CLEAR)
    d.rectangle((120, 492, 904, 532), fill=INK)
    for i in range(10):
        x = 140 + i * 76
        d.rounded_rectangle((x, 497, x + 50, 527), radius=8, fill=CLEAR)
    # Screws on the cover plates.
    for x in (150, 874):
        for y in (430, 594):
            d.ellipse((x - 14, y - 14, x + 14, y + 14), fill=CLEAR)
    return finish(image)


def fretted(body_w, body_h, neck_len, strings, sound_hole, solid=False):
    image, d = canvas()
    cx = S // 2
    bottom = 960
    top = bottom - body_h
    if solid:
        # A solid body: rounder lower bout, two short horns flanking the neck.
        d.ellipse((cx - body_w // 2, top + body_h // 3, cx + body_w // 2, bottom), fill=INK)
        d.ellipse((cx - body_w * 2 // 5, top + body_h // 6, cx + body_w * 2 // 5, top + body_h * 3 // 4), fill=INK)
        for side in (-1, 1):
            hx = cx + side * body_w // 4
            # Offset horns, the upper one longer, as on a solid-body bass.
            horn_top = top - 60 if side > 0 else top + 50
            d.ellipse((hx - 50, horn_top, hx + 50, top + body_h // 3), fill=INK)
    else:
        lower = body_w
        upper = int(body_w * 0.78)
        d.ellipse((cx - lower // 2, top + body_h * 2 // 5, cx + lower // 2, bottom), fill=INK)
        d.ellipse((cx - upper // 2, top, cx + upper // 2, top + body_h * 3 // 5), fill=INK)
        waist = int(body_w * 0.62)
        d.rectangle((cx - waist // 2, top + body_h * 2 // 5, cx + waist // 2, top + body_h * 3 // 5), fill=INK)
    neck_w = 46 + strings * 6
    neck_top = top - neck_len
    d.rectangle((cx - neck_w // 2, neck_top, cx + neck_w // 2, top + 40), fill=INK)
    # Headstock with its tuning pegs.
    head_h = 150
    d.rounded_rectangle(
        (cx - neck_w // 2 - 20, neck_top - head_h, cx + neck_w // 2 + 20, neck_top + 10),
        radius=24,
        fill=INK,
    )
    per_side = (strings + 1) // 2
    for i in range(per_side):
        y = neck_top - head_h + 30 + i * (head_h - 50) // max(per_side - 1, 1)
        d.ellipse((cx - neck_w // 2 - 60, y - 16, cx - neck_w // 2 - 20, y + 16), fill=INK)
        if i < strings - per_side:
            d.ellipse((cx + neck_w // 2 + 20, y - 16, cx + neck_w // 2 + 60, y + 16), fill=INK)
    if sound_hole:
        r = body_w // 7
        hy = top + body_h * 2 // 5
        d.ellipse((cx - r, hy - r, cx + r, hy + r), fill=CLEAR)
    else:
        for y in (top + body_h // 2, top + body_h * 2 // 3):
            d.rounded_rectangle((cx - neck_w, y - 18, cx + neck_w, y + 18), radius=10, fill=CLEAR)
    # Bridge.
    by = bottom - body_h // 5
    d.rectangle((cx - neck_w, by - 12, cx + neck_w, by + 12), fill=CLEAR)
    # Strings: thin gaps running down the neck to the bridge.
    for i in range(strings):
        x = cx - neck_w // 2 + 12 + i * (neck_w - 24) // max(strings - 1, 1)
        d.line((x, neck_top, x, by), fill=CLEAR, width=5)
    return finish(image, angle=-30)


def guitar():
    return fretted(body_w=420, body_h=520, neck_len=300, strings=6, sound_hole=True)


def bass():
    return fretted(body_w=380, body_h=430, neck_len=430, strings=4, sound_hole=False, solid=True)


def cavaquinho():
    return fretted(body_w=300, body_h=380, neck_len=260, strings=4, sound_hole=True)


def piano():
    image, d = canvas()
    left, right, top, bottom = 100, 924, 300, 760
    whites = 10
    w = (right - left) / whites
    stroke = 16
    d.rectangle((left, top, right, bottom), fill=INK)
    for i in range(whites):
        x0 = left + i * w + stroke
        x1 = left + (i + 1) * w - stroke / 2
        d.rectangle((x0, top + stroke, x1, bottom - stroke), fill=CLEAR)
    # Black keys after white keys 1, 2, 4, 5, 6 of each octave (C D _ F G A _).
    for i in range(whites - 1):
        if i % 7 in (0, 1, 3, 4, 5):
            x = left + (i + 1) * w
            d.rectangle((x - w * 0.3, top, x + w * 0.3, top + (bottom - top) * 0.6), fill=INK)
    return finish(image)


def flute():
    image, d = canvas()
    y = S // 2
    d.rounded_rectangle((60, y - 34, 964, y + 34), radius=34, fill=INK)
    # Joints between head, body and foot.
    for x in (300, 800):
        d.rectangle((x - 6, y - 34, x + 6, y + 34), fill=CLEAR)
    # Lip plate and embouchure hole.
    d.rounded_rectangle((150, y - 60, 250, y + 10), radius=24, fill=INK)
    d.ellipse((178, y - 40, 222, y - 10), fill=CLEAR)
    # Keys: rings on the body and foot.
    for x in range(360, 920, 70):
        d.ellipse((x - 26, y - 26, x + 26, y + 26), fill=CLEAR)
        d.ellipse((x - 18, y - 18, x + 18, y + 18), fill=INK)
    return finish(image, angle=25)


def clarinet():
    image, d = canvas()
    cx = S // 2
    # Mouthpiece, barrel, body, bell.
    d.polygon([(cx - 18, 60), (cx + 18, 60), (cx + 32, 170), (cx - 32, 170)], fill=INK)
    d.rounded_rectangle((cx - 44, 170, cx + 44, 260), radius=14, fill=INK)
    d.rectangle((cx - 36, 260, cx + 36, 820), fill=INK)
    d.polygon([(cx - 36, 800), (cx + 36, 800), (cx + 130, 970), (cx - 130, 970)], fill=INK)
    d.ellipse((cx - 130, 940, cx + 130, 1000), fill=INK)
    # Ligature and joint rings.
    for y in (150, 262, 520):
        d.rectangle((cx - 48, y - 6, cx + 48, y + 6), fill=CLEAR)
    # Tone holes.
    for y in list(range(320, 500, 55)) + list(range(570, 790, 55)):
        d.ellipse((cx - 14, y - 14, cx + 14, y + 14), fill=CLEAR)
    return finish(image, angle=-20)


def bowed(scale, with_bow):
    image, d = canvas()
    cx = S // 2

    def sx(v):
        return int(v * scale)

    bottom = 960
    lower_h = sx(330)
    upper_h = sx(270)
    lower_top = bottom - lower_h
    upper_top = lower_top - upper_h + sx(70)
    d.ellipse((cx - sx(200), lower_top, cx + sx(200), bottom), fill=INK)
    d.ellipse((cx - sx(165), upper_top, cx + sx(165), upper_top + upper_h), fill=INK)
    d.rectangle((cx - sx(120), upper_top + upper_h // 2, cx + sx(120), lower_top + lower_h // 2), fill=INK)
    # The C-bouts: the waist cut into each side.
    for side in (-1, 1):
        ex = cx + side * sx(190)
        ey = lower_top + sx(20)
        d.ellipse((ex - sx(80), ey - sx(70), ex + sx(80), ey + sx(70)), fill=CLEAR)
    # f-holes.
    for side in (-1, 1):
        fx = cx + side * sx(85)
        d.line((fx, lower_top - sx(10), fx, lower_top + sx(150)), fill=CLEAR, width=sx(16))
        d.ellipse((fx - sx(14), lower_top - sx(24), fx + sx(14), lower_top + sx(4)), fill=CLEAR)
        d.ellipse((fx - sx(14), lower_top + sx(136), fx + sx(14), lower_top + sx(164)), fill=CLEAR)
    # Neck, pegbox and scroll.
    neck_top = upper_top - sx(300)
    d.rectangle((cx - sx(30), neck_top, cx + sx(30), upper_top + sx(40)), fill=INK)
    d.rectangle((cx - sx(36), neck_top - sx(110), cx + sx(36), neck_top), fill=INK)
    d.ellipse((cx - sx(50), neck_top - sx(190), cx + sx(50), neck_top - sx(90)), fill=INK)
    d.ellipse((cx - sx(20), neck_top - sx(160), cx + sx(20), neck_top - sx(120)), fill=CLEAR)
    for i in range(2):
        y = neck_top - sx(80) + i * sx(50)
        d.ellipse((cx - sx(80), y - sx(14), cx - sx(36), y + sx(14)), fill=INK)
        d.ellipse((cx + sx(36), y - sx(14), cx + sx(80), y + sx(14)), fill=INK)
    # Bridge, tailpiece and strings.
    by = lower_top + sx(80)
    d.rectangle((cx - sx(60), by - sx(10), cx + sx(60), by + sx(10)), fill=CLEAR)
    for i in range(4):
        x = cx - sx(21) + i * sx(14)
        d.line((x, neck_top, x, bottom - sx(60)), fill=CLEAR, width=max(3, sx(5)))
    image = image.rotate(-25, resample=Image.BICUBIC)
    if with_bow:
        d = ImageDraw.Draw(image)
        d.line((150, 880, 900, 160), fill=INK, width=18)
        d.line((170, 920, 920, 200), fill=INK, width=8)
        d.rectangle((120, 860, 190, 930), fill=INK)
    return finish(image)


def violin():
    return bowed(0.82, with_bow=True)


def viola():
    return bowed(0.95, with_bow=False)


DRAWINGS = {
    "harmonica": harmonica,
    "guitar": guitar,
    "bass": bass,
    "cavaquinho": cavaquinho,
    "piano": piano,
    "flute": flute,
    "clarinet": clarinet,
    "violin": violin,
    "viola": viola,
}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for slug, draw in DRAWINGS.items():
        path = OUT / f"{slug}.png"
        draw().save(path, optimize=True)
        print(path.relative_to(OUT.parent.parent.parent))


if __name__ == "__main__":
    main()
