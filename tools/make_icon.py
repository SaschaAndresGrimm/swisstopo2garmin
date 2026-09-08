#!/usr/bin/env python3
"""Generate the application icon.

    python3 tools/make_icon.py --variant terrace --preview out/icon-preview
    python3 tools/make_icon.py --variant terrace --out src-tauri/icons/icon.png
    cargo tauri icon src-tauri/icons/icon.png     # fan out to every platform

Drawn rather than hand-painted so it is reproducible, and so its colours come from the
one place this project keeps colours: `docs/palette.md`, where every value is *measured*
off a swisstopo raster product rather than chosen by eye.

Design constraints, in the order they were decided:

1. **No cross.** The icon this replaces was a red cross on white, which is not the Swiss
   flag -- that is a white cross on red, the inverse -- but the emblem of the Red Cross,
   protected by the Geneva Conventions and by Swiss federal law. The Swiss coat of arms
   is separately restricted to official use under the Wappenschutzgesetz. So the
   Swissness here comes from the cartography, which is what the app actually produces.
2. **No imitation of swisstopo's mark, and nothing implying endorsement** (SPEC.md
   FR-L3). What is evoked is the *Landeskarte's* visual language -- bistre contours on
   white paper -- not a corporate logo.
3. **It must survive 16 px.** Real terrain was tried first and abandoned: the cached
   swissALTI3D around Grindelwald is a valley, and even an isolated summit turns to mush
   once its contours are a pixel apart. Plan-view contours were tried second and
   abandoned too -- concentric closed curves read as tree rings or a fingerprint,
   because they carry no silhouette, and a silhouette is all that survives at 16 px.
"""

import argparse
import math
import pathlib

from PIL import Image, ImageDraw, ImageFilter

# --- palette (docs/palette.md; measured off swisstopo products) -----------------------
PAPER = (255, 255, 255)
CONTOUR = (0xC9, 0xA2, 0x77)  # bistre, the Landeskarte's contour ink
GLACIER = (0xCC, 0xD3, 0xD3)  # firn: swisstopo's is grey, not blue-white
FOREST = (0xCA, 0xEC, 0xC1)
WATER = (0xD3, 0xEE, 0xFF)
ROCK = (0x70, 0x6E, 0x6C)  # rock hachure ink
TRAIL = (0xD4, 0x2B, 0x1E)  # the Landeskarte draws hiking trails red
SLATE = (0x2E, 0x3A, 0x42)  # not from the palette: a ground, not a map colour

SS = 4  # supersample factor; drawn at SS x and reduced, for antialiasing
CORNER = 0.205  # rounded-square radius, as a fraction of the canvas


def mix(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def darken(rgb, f=0.72):
    return tuple(int(c * f) for c in rgb)


def ring(t, sharpness=1.25, drift=0.10, wobble=1.0, n=720):
    """One contour ring in plan, as a closed polygon in unit coordinates.

    `t` runs 0 at the lowest contour to 1 at the summit. Rings share their harmonic
    phases so they read as one landform rather than a stack of unrelated blobs, and the
    centre drifts as the level rises so the peak is asymmetric -- steep on one flank,
    drawn out on the other, the way a real summit is.

    `sharpness` shapes the profile: 1 is a cone, above 1 concave and peaky, below 1 a
    dome. The dome was what made the first attempt read as a rock rather than a mountain.
    """
    cx = 0.500 + drift * t
    cy = 0.560 - 0.06 * t
    radius = 0.430 * (1.0 - t) ** sharpness + 0.052
    harmonics = ((3, 0.100, 0.7), (5, 0.055, 2.1), (7, 0.028, 4.0), (11, 0.013, 5.3))
    amp = (1.0 - 0.5 * t) * wobble
    pts = []
    for i in range(n):
        a = 2 * math.pi * i / n
        r = radius
        for k, h, ph in harmonics:
            r += radius * h * amp * math.sin(k * a + ph)
        pts.append((cx + r * math.cos(a), cy + r * 0.90 * math.sin(a)))
    return pts


def card(n, fill, border=None):
    """The rounded square every platform expects to be handed, plus its mask."""
    mask = Image.new("L", (n, n))
    inset = int(n * 0.030)
    ImageDraw.Draw(mask).rounded_rectangle(
        [inset, inset, n - inset, n - inset], radius=int(n * CORNER), fill=255
    )
    img = Image.new("RGBA", (n, n), fill + (255,))
    if border:
        d = ImageDraw.Draw(img)
        d.rounded_rectangle(
            [inset, inset, n - inset, n - inset],
            radius=int(n * CORNER),
            outline=border + (255,),
            width=max(1, int(n * 0.0085)),
        )
    return img, mask


def terraces(b, px, levels, step, base, tint, sharpness, index_every=4, wobble=1.0):
    """Draw the contour terraces, lowest first so each step overlaps the one below."""
    for lv in range(levels):
        t = lv / (levels - 1)
        poly = ring(t, sharpness=sharpness, wobble=wobble)
        dy = base - step * lv
        pts = px(poly, dy)
        b.polygon(pts, fill=tint(t) + (255,))
        heavy = lv % index_every == 0
        b.line(
            pts + [pts[0]],
            fill=(darken(CONTOUR, 0.60) if heavy else CONTOUR) + (255,),
            width=max(1, int(len(pts) and px.n * (0.0110 if heavy else 0.0068))),
            joint="curve",
        )


class Px:
    """Unit coordinates to pixels, with a vertical offset."""

    def __init__(self, n):
        self.n = n

    def __call__(self, p, dy=0.0):
        return [(x * self.n, (y + dy) * self.n) for x, y in p]


def route(b, px, n, pts, width=0.0235):
    b.line(px(pts), fill=TRAIL + (255,), width=int(n * width), joint="curve")


def spot(b, px, n, x, y, r=0.0165, col=ROCK):
    b.ellipse([x * n - n * r, y * n - n * r, x * n + n * r, y * n + n * r], fill=col + (255,))


# --- variants -------------------------------------------------------------------------

# The massif, in unit coordinates: left foot, two shoulders, the summit, a steep
# south-east face, a subpeak, and the right foot. Asymmetric on purpose -- a symmetric
# triangle is a logo of a mountain, not a drawing of one.
_RIDGE_RAW = [
    (0.048, 0.828), (0.128, 0.706), (0.198, 0.664), (0.272, 0.548),
    (0.330, 0.478), (0.394, 0.306), (0.440, 0.168),           # summit
    (0.498, 0.302), (0.548, 0.418), (0.598, 0.520), (0.640, 0.586),
    (0.700, 0.512), (0.752, 0.578), (0.836, 0.678), (0.968, 0.828),
]

# The boundary between the lit and shaded faces: a spur running off the summit. The
# first version ran nearly vertically and read as a fold in the paper rather than as a
# change of slope, so it now fans outward toward the base the way a real spur does.
_SPUR_RAW = [
    (0.440, 0.168), (0.448, 0.298), (0.428, 0.446), (0.386, 0.612), (0.320, 0.872),
]

FOOT = 0.872          # where the massif meets the forest
_SNOWLINE_RAW = 0.447

# Stretched vertically about the foot, because the first composition left a third of
# the card as empty sky. Applied as a transform rather than by re-typing coordinates so
# the landform's proportions cannot drift while it is being resized.
RELIEF = 1.075


def _lift(y):
    return FOOT - (FOOT - y) * RELIEF


RIDGE = [(x, _lift(y)) for x, y in _RIDGE_RAW]
SPUR = [(x, _lift(y)) for x, y in _SPUR_RAW]
SUMMIT = RIDGE[6]
SNOWLINE = _lift(_SNOWLINE_RAW)


def _poly_mask(n, px, poly):
    m = Image.new("L", (n, n), 0)
    ImageDraw.Draw(m).polygon(px(poly), fill=255)
    return m


def _massif(poly_extra=()):
    return RIDGE + [(0.968, FOOT), (0.048, FOOT)] + list(poly_extra)


def _snow_field(n, px):
    """Firn above the snowline, with tongues reaching down the gullies.

    A straight snowline looks printed on. The first attempt made it a regular zigzag,
    which looked like a crown; this is a smooth line with three descents, which is how
    snow actually lies.
    """
    edge = [(0.255, SNOWLINE + 0.012)]
    for i in range(49):
        t = i / 48
        x = 0.255 + 0.400 * t
        # Three tongues of different depth, so the line is not periodic.
        y = SNOWLINE - 0.014 * math.sin(math.pi * t)
        y += 0.034 * math.exp(-((t - 0.20) / 0.115) ** 2)
        y += 0.048 * math.exp(-((t - 0.54) / 0.100) ** 2)
        y += 0.026 * math.exp(-((t - 0.83) / 0.105) ** 2)
        edge.append((x, y))
    poly = [(-0.05, SNOWLINE)] + edge + [(1.05, SNOWLINE), (1.05, -0.05), (-0.05, -0.05)]
    return _poly_mask(n, px, poly)


def _hachures(layer, px, n, colour, width):
    """Rock drawing on the shaded face.

    swisstopo's *Felszeichnung* has no vector equivalent (SPEC.md 8.1) and is not being
    reproduced here; it is being *referred to*, with slope-following strokes in the
    measured hachure ink. They fade out below about 48 px, which is the right thing for
    a texture to do.
    """
    d = ImageDraw.Draw(layer)
    for i in range(34):
        t = i / 33
        x0 = 0.455 + 0.185 * t + 0.010 * math.sin(t * 11)
        y0 = 0.215 + 0.330 * t
        ln = 0.062 + 0.055 * math.sin(t * 2.9 + 0.4)
        d.line(px([(x0, y0), (x0 + ln * 0.55, y0 + ln)]), fill=colour + (255,), width=width)


def _draw_massif(img, px, n, sunlit, shaded, snow, forest_col, hachure):
    b = ImageDraw.Draw(img)

    # Forest at the foot: the flat tint that carries the icon at a small size. Its top
    # edge overlaps the massif's base so no sliver of ground shows between them.
    b.polygon(px([(-0.05, 1.05), (1.05, 1.05), (1.05, FOOT - 0.030)] + [
        (1.05 - 1.10 * i / 48, FOOT - 0.030 + 0.026 * math.sin(5.2 * i / 48 + 0.6))
        for i in range(49)
    ]), fill=forest_col + (255,))

    rock = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    rb = ImageDraw.Draw(rock)
    rb.polygon(px(_massif()), fill=sunlit + (255,))
    # The shaded face: everything right of the spur.
    rb.polygon(px(SPUR + [(0.968, FOOT)] + list(reversed(RIDGE[7:])) + [SUMMIT]),
               fill=shaded + (255,))
    _hachures(rock, px, n, hachure, max(1, int(n * 0.0052)))

    field = Image.new("RGBA", (n, n), snow + (255,))
    rock.paste(field, (0, 0), _snow_field(n, px))

    # Deliberately no stroke along the ridge. It was tried and removed: an outline dark
    # enough to read on paper is, on the slate ground, close enough to the sky that the
    # steep descent off the summit looked like a gap in the mountain. The two-tone faces
    # and the silhouette carry the form without it.

    img.paste(rock, (0, 0), _poly_mask(n, px, _massif()))

    # A hiking route across the lower slope, in the Landeskarte's red: the only saturated
    # colour, and what says this is a map to walk with rather than a picture of a hill.
    b.line(px([(0.070, 0.930), (0.205, 0.892), (0.330, 0.842), (0.430, 0.792),
               (0.545, 0.756), (0.700, 0.744), (0.855, 0.768), (0.945, 0.812)]),
           fill=TRAIL + (255,), width=int(n * 0.0165), joint="curve")


def v_paper(n):
    """The Landeskarte's own scheme: white paper, measured tints, bistre linework.

    The most faithful of the three, and the one that needs a hairline border: a white
    icon has no edge against a light desktop.
    """
    img, mask = card(n, PAPER, border=mix(CONTOUR, ROCK, 0.30))
    _draw_massif(
        img, Px(n), n,
        sunlit=mix(PAPER, GLACIER, 0.42),
        shaded=mix(GLACIER, ROCK, 0.40),
        # Not paper-white: on a paper ground that makes the summit vanish into the sky.
        snow=mix(PAPER, GLACIER, 0.30),
        forest_col=FOREST,
        hachure=darken(ROCK, 0.55),
    )
    return img, mask


def v_slate(n):
    """The same massif on a dark ground.

    Slate is not a map colour and is not pretending to be one. An icon has to be found
    in a dock, and white-on-white cannot be; this is the version that survives a dark
    desktop, a dark taskbar and a dark app grid.
    """
    img, mask = card(n, SLATE)
    _draw_massif(
        img, Px(n), n,
        sunlit=mix(GLACIER, PAPER, 0.12),
        shaded=mix(GLACIER, SLATE, 0.46),
        snow=PAPER,
        forest_col=mix(FOREST, SLATE, 0.50),
        hachure=mix(SLATE, GLACIER, 0.10),
    )
    return img, mask


def v_sky(n):
    """Paper massif on the measured water blue.

    The blue is `#D3EEFF`, the Landeskarte's own water colour, used here as sky. It gives
    the icon a defined edge without leaving the palette, which the slate version does.
    """
    img, mask = card(n, WATER)
    _draw_massif(
        img, Px(n), n,
        sunlit=mix(PAPER, GLACIER, 0.34),
        shaded=mix(GLACIER, ROCK, 0.44),
        snow=PAPER,
        forest_col=FOREST,
        hachure=darken(ROCK, 0.52),
    )
    return img, mask


VARIANTS = {"paper": v_paper, "slate": v_slate, "sky": v_sky}


def draw(variant="terrace", size=1024):
    n = size * SS
    img, mask = VARIANTS[variant](n)
    out = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    out.paste(img, (0, 0), mask)
    return out.resize((size, size), Image.LANCZOS)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--variant", default="terrace", choices=sorted(VARIANTS))
    ap.add_argument("--out", default=None)
    ap.add_argument("--preview", default=None, help="write 16..512 px samples here")
    ap.add_argument("--all", action="store_true", help="preview every variant")
    a = ap.parse_args()

    names = sorted(VARIANTS) if a.all else [a.variant]
    for name in names:
        icon = draw(name, 1024)
        if a.out and not a.all:
            out = pathlib.Path(a.out)
            out.parent.mkdir(parents=True, exist_ok=True)
            icon.save(out)
            print(f"{out} 1024x1024")
        if a.preview:
            p = pathlib.Path(a.preview)
            p.mkdir(parents=True, exist_ok=True)
            icon.save(p / f"{name}-1024.png")
            # A sheet on two grounds, because an icon that only works on one is not done.
            for bg, tag in (((0xF2, 0xF2, 0xF4), "light"), ((0x24, 0x26, 0x2B), "dark")):
                sheet = Image.new("RGBA", (760, 300), bg + (255,))
                x = 24
                for s in (256, 128, 64, 32, 16):
                    sheet.alpha_composite(icon.resize((s, s), Image.LANCZOS), (x, 24))
                    x += s + 28
                sheet.save(p / f"{name}-sizes-{tag}.png")
            print(f"{p}/{name}-sizes-light.png")


if __name__ == "__main__":
    main()
