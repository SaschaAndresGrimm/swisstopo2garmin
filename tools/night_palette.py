"""Derive the night palette from the measured day palette (SPEC.md FR-CART2).

Unlike the summer, winter and slope colours, this one is **not measured**: swisstopo
publishes no night map, and Garmin's night mode is a device rendering mode whose colours
the TYP author chooses. So this is a stated transform rather than a measurement, and the
palette records it as `derived` — the distinction matters, and every other colour in this
project can point at a swisstopo product.

The transform inverts lightness while preserving hue, which is what keeps a night map
recognisable as the same map: forest stays green, water stays blue, the ink-on-paper
relationship is simply reversed. Saturation is pulled down, because a fully saturated
colour on a dark ground glares at night. Lightness is clamped well short of both ends, so
nothing becomes pure black (invisible against the ground) or pure white (a glare source).

Route overlays are treated differently: they are printed on top and must stay findable,
so they keep their hue and saturation and are lightened rather than inverted.

    python3 tools/night_palette.py           # report
    python3 tools/night_palette.py --write   # update cartography/palette.json
"""
from __future__ import annotations

import argparse
import colorsys
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PALETTE_PATH = REPO / "cartography" / "palette.json"

# Ground and ink stay where they are: the night ground was chosen directly, and the
# building ink is already near-black.
GROUND = "paper"
NIGHT_GROUND = "#1A1A1A"

# Area fills. These are meant to be *subtle*: on the Landeskarte forest is a pale tint
# barely separable from the paper, and at night it should likewise be a dark tint barely
# separable from the ground. Holding them to a high contrast ratio would make a night map
# that glows.
FILLS = {
    "forest", "forest_open", "copse", "scrub", "glacier", "firn", "water",
    "green_open", "green_mid", "builtup", "transport_area", "leisure_area",
}

# Printed over the sheet: kept findable rather than inverted.
OVERLAYS = {
    "hike_yellow", "hike_red", "hike_blue", "via_ferrata",
    "ski_tour", "ski_carry", "ski_caution", "snowshoe", "winter_hiking",
    "cycle_route", "mtb_route", "mtb_singletrail",
    "slope_30", "slope_35", "slope_40", "slope_45", "slope_50",
}

# Lightness band for inverted colours. Neither end is reached: 0.10 is still visible
# against the 0.10-lightness ground, and 0.78 is bright without glaring.
L_MIN, L_MAX = 0.10, 0.78
# Overlays sit at least this light, so a route never sinks into the ground.
OVERLAY_L_MIN = 0.55

# Anything that draws as ink -- lines, symbols, labels, overlays -- must stand off the
# ground. 3:1 is the WCAG threshold for graphical objects.
INK_MIN_CONTRAST = 3.0
# A fill above this stops being a tint and starts competing with the ink on top of it.
FILL_MAX_CONTRAST = 3.0


def hex_to_rgb(h: str) -> tuple[float, float, float]:
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))


def rgb_to_hex(rgb) -> str:
    return "#%02X%02X%02X" % tuple(max(0, min(255, round(c * 255))) for c in rgb)


def relative_luminance(h: str) -> float:
    """WCAG relative luminance, for the contrast check."""
    def channel(c: float) -> float:
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (channel(c) for c in hex_to_rgb(h))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: str, b: str) -> float:
    la, lb = relative_luminance(a), relative_luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def lighten_to_contrast(colour: str, ground: str, target: float) -> str:
    """Raise a colour's lightness until it stands off the ground, keeping its hue.

    A pale day colour inverts to a dark night colour, which is right in spirit — a
    minor road should stay quiet — but a quiet road still has to be visible. Rather
    than lower the threshold, the colour is lightened just far enough to meet it, so
    the ordering between road classes survives and only the floor moves.
    """
    if contrast(colour, ground) >= target:
        return colour
    r, g, b = hex_to_rgb(colour)
    h, l, sat = colorsys.rgb_to_hls(r, g, b)
    lo, hi = l, 1.0
    for _ in range(40):
        mid = (lo + hi) / 2
        candidate = rgb_to_hex(colorsys.hls_to_rgb(h, mid, sat))
        if contrast(candidate, ground) >= target:
            hi = mid
        else:
            lo = mid
    return rgb_to_hex(colorsys.hls_to_rgb(h, hi, sat))


def night_of(day: str, overlay: bool) -> str:
    r, g, b = hex_to_rgb(day)
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    if overlay:
        # Keep the hue and saturation; only ensure it is light enough to find.
        l2 = max(l, OVERLAY_L_MIN)
        s2 = s
    else:
        # Invert lightness into a comfortable band, and calm the saturation.
        l2 = L_MIN + (1.0 - l) * (L_MAX - L_MIN)
        s2 = s * 0.7
    return rgb_to_hex(colorsys.hls_to_rgb(h, l2, s2))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()

    palette = json.loads(PALETTE_PATH.read_text())
    print(f"{'entry':22} {'day':>9} {'night':>9}  {'contrast':>13}  kind")
    worst = (999.0, "")
    failures: list[tuple[str, float]] = []
    lifted_entries: list[str] = []
    for key, entry in palette.items():
        if key.startswith("_") or "day" not in entry:
            continue
        if key == GROUND:
            entry["night"] = NIGHT_GROUND
            entry["night_derived"] = "chosen directly as the night ground"
            print(f"{key:22} {entry['day']:>9} {NIGHT_GROUND:>9}  (the ground itself)")
            continue

        overlay = key in OVERLAYS
        paper_like = entry["day"].upper() == palette[GROUND]["day"].upper()

        if paper_like:
            # A white road on white paper is the paper showing through; what makes it a
            # road is its casing. The night equivalent is the ground showing through, so
            # the fill becomes the ground and the casing does the work — inverting it to
            # a mid grey would draw a road the paper map does not have.
            night = NIGHT_GROUND
            why = "the ground showing through, as on paper; the casing carries it"
            if "casing" not in entry:
                raise SystemExit(
                    f"{key} is paper-coloured but has no casing, so it would be "
                    f"invisible at night"
                )
        else:
            night = night_of(entry["day"], overlay)
            if key not in FILLS:
                # Ink must stand off the ground; a fill must not.
                lifted = lighten_to_contrast(night, NIGHT_GROUND, INK_MIN_CONTRAST + 0.05)
                if lifted != night:
                    lifted_entries.append(key)
                night = lifted
            why = (
                "overlay: hue and saturation kept, lightened to stay findable"
                if overlay
                else "lightness inverted into a comfortable band, saturation reduced"
            )
        entry["night"] = night
        entry["night_derived"] = why

        # Casings invert too: a dark outline on light paper becomes a light one on the
        # dark ground.
        if "casing" in entry:
            entry["casing_night"] = night_of(entry["casing"], overlay=False)

        c = contrast(night, NIGHT_GROUND)
        if paper_like:
            # Deliberately equal to the ground; scored on its casing instead.
            c = contrast(entry["casing_night"], NIGHT_GROUND)

        kind = "fill" if key in FILLS else "ink"
        if kind == "ink":
            if c < worst[0]:
                worst = (c, key)
            if c < INK_MIN_CONTRAST:
                failures.append((key, c))
        elif c > FILL_MAX_CONTRAST:
            failures.append((key, c))
        print(f"{key:22} {entry['day']:>9} {night:>9}  {c:13.2f}  {kind}")

    palette["_night"] = {
        "measured": False,
        "method": (
            "Derived from the day palette, not measured: swisstopo publishes no night "
            "map and Garmin's night mode is a rendering mode whose colours the TYP "
            "author chooses. Lightness is inverted into a band clear of both extremes "
            "and saturation reduced, so hue is preserved and the map stays recognisable. "
            "Route overlays keep hue and saturation and are only lightened."
        ),
        "ground": NIGHT_GROUND,
        "lightness_band": [L_MIN, L_MAX],
        "overlay_min_lightness": OVERLAY_L_MIN,
        "overlays": sorted(OVERLAYS),
        "worst_contrast_against_ground": {"entry": worst[1], "ratio": round(worst[0], 2)},
        "ink_min_contrast": INK_MIN_CONTRAST,
        "fill_max_contrast": FILL_MAX_CONTRAST,
        "lightened_to_meet_the_floor": lifted_entries,
        "paper_like": (
            "Entries whose day colour is the paper colour become the night ground, and "
            "are scored on their casing: what makes a white road a road is its outline, "
            "and that relationship should invert rather than break."
        ),
    }

    print(f"\nlowest ink contrast against the night ground: {worst[1]} at {worst[0]:.2f}:1")
    if lifted_entries:
        print(
            f"lightened to meet the {INK_MIN_CONTRAST}:1 floor: "
            + ", ".join(lifted_entries)
        )
    if failures:
        print("\nout of range:")
        for key, c in failures:
            print(f"  {key:22} {c:.2f}:1")
        raise SystemExit(
            f"{len(failures)} entries are out of range; the palette was not written"
        )
    if args.write:
        PALETTE_PATH.write_text(json.dumps(palette, indent=2) + "\n")
        print(f"wrote {PALETTE_PATH}")
    else:
        print("(dry run; pass --write to update the palette)")


if __name__ == "__main__":
    main()
