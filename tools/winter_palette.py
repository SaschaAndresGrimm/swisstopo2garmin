"""Measure swisstopo's Winter national map and add its colours to the palette (FR-CART2).

swisstopo publishes the Winter national map (`ch.swisstopo.pixelkarte-farbe-winter`)
alongside the summer one, so the winter scheme can be *measured* from swisstopo's own
cartography rather than invented -- the same discipline the summer palette follows
(FR-CART9).

What is measured is the *shift* from summer to winter, not the winter colour itself.
The two sheets are sampled with one estimator over the same tiles, and the resulting
per-channel delta is applied to the palette's existing summer value. Substituting the
winter reading directly would darken the whole map, because these tiles are JPEG and the
summer palette was measured from the lossless swissTLM3D raster -- the two numbers are
not comparable, but their difference is.

Three kinds of entry, and the palette records which is which:

  measured  an area fill that can be isolated at a location where it dominates.
  derived   a pale variant, taken from its measured parent by the same relationship it
            has in summer, because two shades of forest cannot be separated by sampling.
  kept      printed on top of the sheet at full strength: routes, ink, buildings, rail,
            and the road hierarchy, which the winter sheet prints as summer does.

A global per-channel fit was tried first and rejected: R^2 was 0.71 on blue, and it
turned the bistre contours pink. The winter sheet recolours selectively, so it has to be
measured selectively.

    python3 tools/winter_palette.py --write
"""
from __future__ import annotations

import argparse
import io
import json
import math
import statistics
import urllib.request
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
PALETTE_PATH = REPO / "cartography" / "palette.json"

WMTS = ("https://wmts.geo.admin.ch/1.0.0/{layer}/default/current/3857/"
        "{z}/{x}/{y}.jpeg")
WINTER = "ch.swisstopo.pixelkarte-farbe-winter"
SUMMER = "ch.swisstopo.pixelkarte-farbe"

# Several locations per fill, so one atypical tile cannot set a colour. Chosen where a
# single cover type dominates the tile.
MEASURE: dict[str, list[tuple[float, float]]] = {
    "forest":     [(46.6100, 8.0100), (46.7050, 7.7350), (47.0600, 8.9200)],
    "glacier":    [(46.5450, 8.0900), (46.0100, 7.8600), (46.4400, 8.1200)],
    "water":      [(46.7300, 7.9700), (46.4200, 6.5300), (47.2200, 8.7600)],
    "green_open": [(46.9480, 7.4474), (47.4900, 8.2000), (46.8200, 7.1200)],
    "builtup":    [(47.3769, 8.5417), (46.9480, 7.4474), (46.2044, 6.1432)],
    "rock_ink":   [(46.5775, 8.0053), (46.0207, 7.7491), (46.5600, 7.9600)],
}

# Pale variants: no way to isolate them by sampling, so they keep the relationship they
# have to their parent in the summer palette.
DERIVED = {
    "forest_open": "forest",
    "copse": "forest",
    "scrub": "forest",
    "firn": "glacier",
    "green_mid": "green_open",
    "leisure_area": "green_open",
    "transport_area": "builtup",
    "scree_ink": "rock_ink",
    "water_line": "water",
    "wetland_ink": "forest",
    "contour_ice": "water",
}

# Printed on the sheet at full strength, or printed the same way in winter.
KEEP = {
    "paper",
    "building", "rail", "path",
    "road_motorway", "road_trunk", "road_primary", "road_secondary",
    "road_tertiary", "road_minor", "road_access", "track",
    "contour_minor", "contour_medium", "contour_major",
    "hike_yellow", "hike_red", "hike_blue", "via_ferrata",
    "lift_aerial", "lift_surface",
    "ski_tour", "ski_carry", "ski_caution", "snowshoe", "winter_hiking",
    "cycle_route", "mtb_route", "mtb_singletrail",
}


def tile_xy(lat: float, lon: float, z: int) -> tuple[int, int]:
    n = 2 ** z
    lr = math.radians(lat)
    return (
        int((lon + 180) / 360 * n),
        int((1 - math.log(math.tan(lr) + 1 / math.cos(lr)) / math.pi) / 2 * n),
    )


def fetch(layer: str, lat: float, lon: float, z: int) -> Image.Image:
    x, y = tile_xy(lat, lon, z)
    url = WMTS.format(layer=layer, z=z, x=x, y=y)
    req = urllib.request.Request(url, headers={"User-Agent": "swisstopo2garmin/0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return Image.open(io.BytesIO(r.read())).convert("RGB")


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb_to_hex(rgb) -> str:
    return "#%02X%02X%02X" % tuple(max(0, min(255, round(v))) for v in rgb)


def dominant(im: Image.Image, want_ink: bool) -> tuple[int, int, int] | None:
    """Channel-wise median of the pixels that carry the fill.

    Median rather than mode: the tiles are JPEG, which smears every flat area into a
    cloud of near-identical values, so the most common single value is noise while the
    middle of the cloud is stable.
    """
    px = im.load()
    keep = []
    for y in range(0, im.size[1], 2):
        for x in range(0, im.size[0], 2):
            c = px[x, y]
            if want_ink:
                # Hachure and stipple: mid greys, not paper and not black text.
                if 90 < max(c) < 210:
                    keep.append(c)
            else:
                # Area fills: tinted, not paper white and not ink.
                if max(c) > 235 and min(c) > 228:
                    continue
                if max(c) < 90:
                    continue
                # Neutral greys are linework, labels and hachure, not fill. The summer
                # sampler excludes them too; without this, an urban or farmland tile's
                # median is dragged dark by its roads and place names.
                r, g, b = c
                if abs(r - g) < 5 and abs(g - b) < 5 and abs(r - b) < 5:
                    continue
                keep.append(c)
    if len(keep) < 500:
        return None
    return tuple(round(statistics.median(c[i] for c in keep)) for i in range(3))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--zoom", type=int, default=15)
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()

    palette = json.loads(PALETTE_PATH.read_text())
    measured: dict[str, str] = {}

    print(f"winter {WINTER}\nsummer {SUMMER}\nzoom   {args.zoom}\n")
    print(f"{'fill':<12} {'summer':>9} {'winter':>9} {'delta':>16}   palette -> winter")
    deltas: dict[str, tuple[int, int, int]] = {}
    for key, spots in MEASURE.items():
        ink = key.endswith("_ink")
        pairs = []
        for lat, lon in spots:
            try:
                w = dominant(fetch(WINTER, lat, lon, args.zoom), ink)
                su = dominant(fetch(SUMMER, lat, lon, args.zoom), ink)
            except Exception as e:
                print(f"  {key:<12} {lat},{lon}: skipped ({e})")
                continue
            if w and su:
                pairs.append((su, w))
        if not pairs:
            print(f"  {key:<12} NO READING -- keeping the summer colour")
            continue

        # Median across locations, so one atypical tile cannot set the shift.
        su_med = [statistics.median(p[0][i] for p in pairs) for i in range(3)]
        w_med = [statistics.median(p[1][i] for p in pairs) for i in range(3)]
        delta = tuple(round(w_med[i] - su_med[i]) for i in range(3))
        deltas[key] = delta

        day = hex_to_rgb(palette[key]["day"])
        winter = rgb_to_hex([day[i] + delta[i] for i in range(3)])
        measured[key] = winter
        print(
            f"  {key:<12} {rgb_to_hex(su_med):>9} {rgb_to_hex(w_med):>9} "
            f"{str(delta):>16}   {palette[key]['day']} -> {winter}"
        )

    # Apply.
    for key, colour in measured.items():
        palette[key]["winter"] = colour
        palette[key]["winter_measured"] = True
        palette[key]["winter_shift"] = list(deltas[key])

    for key, parent in DERIVED.items():
        if key not in palette or parent not in palette:
            continue
        if parent not in measured:
            palette[key]["winter"] = palette[key]["day"]
            continue
        # Keep the summer relationship: child = parent + delta.
        pd = hex_to_rgb(palette[parent]["day"])
        cd = hex_to_rgb(palette[key]["day"])
        pw = hex_to_rgb(measured[parent])
        palette[key]["winter"] = rgb_to_hex([pw[i] + (cd[i] - pd[i]) for i in range(3)])
        palette[key]["winter_derived_from"] = parent

    for key in KEEP:
        if key in palette and "day" in palette[key]:
            palette[key]["winter"] = palette[key]["day"]
            palette[key]["winter_kept"] = True

    # Anything not covered above keeps its summer colour rather than being guessed.
    missing = [
        k for k, v in palette.items()
        if not k.startswith("_") and "day" in v and "winter" not in v
    ]
    for k in missing:
        palette[k]["winter"] = palette[k]["day"]
        palette[k]["winter_kept"] = True
    if missing:
        print(f"kept the summer colour for: {', '.join(sorted(missing))}")

    palette["_winter"] = {
        "source": f"{WINTER} against {SUMMER}",
        "zoom": args.zoom,
        "method": (
            "The per-channel shift from the summer sheet to the Winter national map, "
            "measured as the channel-wise median of fill pixels over the same tiles in "
            "both, then applied to the palette's summer value. The shift is what is "
            "comparable: these tiles are JPEG while the summer palette was measured "
            "from the lossless swissTLM3D raster. Pale variants keep the relationship "
            "they have to their measured parent. Overlay colours, ink, the road "
            "hierarchy and contours are printed as in summer and are unchanged."
        ),
        "shifts": {k: list(v) for k, v in deltas.items()},
        "measured": sorted(measured),
        "derived": {k: v for k, v in DERIVED.items() if v in measured},
        "rejected_approach": (
            "A single per-channel linear fit of winter against summer over paired "
            "pixels reached only R^2 0.71 on blue and turned the bistre contours pink: "
            "the winter sheet recolours selectively, so it must be measured selectively."
        ),
    }

    if args.write:
        PALETTE_PATH.write_text(json.dumps(palette, indent=2) + "\n")
        print(f"\nwrote {PALETTE_PATH}")
    else:
        print("\n(dry run; pass --write to update the palette)")


if __name__ == "__main__":
    main()
