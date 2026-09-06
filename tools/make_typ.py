"""Generate typ/swisstopo.txt from cartography/palette.json.

The TYP is generated rather than hand-written because (a) colours must come from the
measured swisstopo palette (SPEC.md FR-CART9) and (b) pattern fills are 32-line XPM
blocks that are miserable to maintain by hand (FR-CART10).

Do not edit typ/swisstopo.txt directly -- edit the palette or the patterns here.
"""
from __future__ import annotations

import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PALETTE = json.loads((REPO / "cartography" / "palette.json").read_text())

# mkgmap rewrites this to whatever --family-id the build uses (verified: the compiled
# TYP carries the build's family id at offset 0x2f, not this one). It is kept only so
# the file is valid standalone.
FID = 6324

# A wrist screen is ~1.3 inch. The same line weights that read well on an Edge turn
# into a solid mass, so the wrist variant thins everything (SPEC.md FR-CART6).
WRIST_LINE_SCALE = 0.6
WRIST_MIN_WIDTH = 1


def c(name: str, key: str = "day") -> str:
    return PALETTE[name][key]


# --- 32x32 pattern generators -------------------------------------------------
def pattern(fn) -> list[str]:
    """fn(x, y) -> True where ink should be drawn."""
    return ["".join("#" if fn(x, y) else "." for x in range(32)) for y in range(32)]


def rock_hachure(x: int, y: int) -> bool:
    # broken diagonal strokes, loosely evoking the Landeskarte Felszeichnung
    d = (x + y) % 8
    return d == 0 and ((x * 7 + y * 3) % 11) > 2


def scree_stipple(x: int, y: int) -> bool:
    return ((x * 13 + y * 29) % 47) < 2


def wetland_ticks(x: int, y: int) -> bool:
    # short horizontal dashes in offset rows
    if y % 8 != 0:
        return False
    return ((x + (y // 8) * 4) % 16) < 6


def slope_hatch(x: int, y: int) -> bool:
    """Diagonal hatch at 50% coverage.

    Slope classes cover whole mountainsides, so a solid fill would bury the map they are
    meant to inform. A hatch over a transparent ground reads as a tint at a glance and
    still lets contours, trails and place names through -- which a Garmin TYP cannot
    achieve with alpha, since polygon fills have none.
    """
    return (x + y) % 4 < 2


def vineyard_rows(x: int, y: int) -> bool:
    if x % 8 != 0:
        return False
    return ((y + (x // 8) * 4) % 12) < 7


PATTERNS = {
    "rock":     (rock_hachure, "paper", "rock_ink"),
    "scree":    (scree_stipple, "paper", "scree_ink"),
    "wetland":  (wetland_ticks, "paper", "wetland_ink"),
    "vineyard": (vineyard_rows, "green_mid", "wetland_ink"),
    # Slope classes: hatched over a transparent ground (FR-CART12).
    "slope_30": (slope_hatch, None, "slope_30"),
    "slope_35": (slope_hatch, None, "slope_35"),
    "slope_40": (slope_hatch, None, "slope_40"),
    "slope_45": (slope_hatch, None, "slope_45"),
    "slope_50": (slope_hatch, None, "slope_50"),
}


# --- element tables -----------------------------------------------------------
# (type, drawOrder, label, fill-colour key)  -- solid polygons
SOLID_POLYGONS = [
    ("0x4a", 1, "Hintergrund",    "paper"),
    ("0x4b", 1, "Hintergrund",    "paper"),
    ("0x10203", 3, "Gletscher",   "glacier"),
    ("0x10204", 3, "Schneefeld",  "firn"),
    ("0x10205", 4, "Wald",        "forest"),
    ("0x10206", 4, "Gehoelz",     "copse"),
    ("0x10207", 4, "Gebueschwald","scrub"),
    ("0x10208", 4, "Wald offen",  "forest_open"),
    ("0x1020b", 5, "Obstanlage",  "green_mid"),
    ("0x1020c", 6, "Nutzungsareal","builtup"),
    ("0x1020d", 6, "Verkehrsareal","transport_area"),
    ("0x1020e", 6, "Freizeitareal","leisure_area"),
    ("0x1020f", 7, "See",          "water"),
    ("0x10210", 7, "Fliessgewaesser","water"),
    ("0x10211", 8, "Gebaeude",     "building"),
]

# (type, drawOrder, label, pattern key)
PATTERN_POLYGONS = [
    ("0x10201", 2, "Fels",          "rock"),
    ("0x10202", 2, "Lockergestein", "scree"),
    ("0x10209", 5, "Feuchtgebiet",  "wetland"),
    ("0x1020a", 5, "Reben",         "vineyard"),
    # Slope classes sit above the land cover and below the linework, and steeper draws
    # over shallower so an overlapping pair reads as the steeper of the two.
    #
    # 0x1021x, not 0x1022x: a Garmin extended type carries its subtype in five bits, so
    # a polygon subtype above 0x1f does not exist. mkgmap rejects 0x10220 outright --
    # "invalid type 0x10220 for POLYGON" -- which is at least a loud failure.
    ("0x10212", 9, "Hangneigung 30", "slope_30"),
    ("0x10213", 10, "Hangneigung 35", "slope_35"),
    ("0x10214", 11, "Hangneigung 40", "slope_40"),
    ("0x10215", 12, "Hangneigung 45", "slope_45"),
    ("0x10216", 13, "Hangneigung 50", "slope_50"),
]

# (type, label, icon) where icon None means "label only, no symbol".
#
# swissTLM3D contributes thousands of Flurnamen and single objects. Left with Garmin's
# default POI icon they render as a field of circles, which the Landeskarte does not do
# -- it sets those names as text alone.
POINTS = [
    ("0x6400", "Flurname", None),
    ("0x0400", "Ort", "dot"),
    ("0x0600", "Weiler", "dot"),
    # Destinations and access, which a paper Landeskarte also marks.
    ("0x2f08", "Berghuette", "hut"),
    ("0x2a01", "Bergrestaurant", "hut"),
    ("0x2f15", "Bushaltestelle", "stop"),
    ("0x2f17", "Bahnhof", "stop"),
    ("0x2f16", "Schiffstation", "stop"),
]

# (type, label, colour key, width, border width)
LINES = [
    ("0x10101", "Wanderweg",      "hike_yellow", 4, 1),
    ("0x10102", "Bergwanderweg",  "hike_red",    4, 2),
    ("0x10103", "Alpinwanderweg", "hike_blue",   4, 2),
    ("0x10104", "Klettersteig",   "via_ferrata", 3, 1),
    ("0x01",    "Autobahn",       "road_motorway",  7, 1),
    ("0x02",    "Autostrasse",    "road_trunk",     6, 1),
    ("0x03",    "Hauptstrasse",   "road_primary",   6, 1),
    ("0x04",    "Verbindungsstrasse", "road_secondary", 5, 1),
    ("0x05",    "Strasse 6m",     "road_tertiary",  5, 1),
    ("0x06",    "Strasse",        "road_minor",     4, 1),
    ("0x07",    "Zufahrt",        "road_access",    3, 1),
    ("0x08",    "Ein-/Ausfahrt",  "road_primary",   4, 1),
    ("0x0a",    "Fahrweg",        "track",          3, 1),
    ("0x16",    "Fussweg",        "path",           2, 0),
    ("0x14",    "Eisenbahn",      "rail",           4, 1),
    # Aerial and surface lifts. Already present in swissTLM3D as
    # tlm_oev_uebrige_bahn (2,903 features: 821 ski lifts, 366 chairlifts,
    # 322 cable cars, 144 gondolas) but previously not extracted at all -- a real
    # omission for a Swiss hiking or ski map.
    ("0x10105", "Luftseilbahn",   "lift_aerial",    2, 0),
    ("0x10106", "Skilift",        "lift_surface",   2, 0),
    # Winter routes. Only drawn when the skimo content preset is selected, but the
    # types are always defined so one TYP serves every preset.
    ("0x10107", "Skitour",         "ski_tour",       4, 1),
    ("0x10108", "Skitour tragen",  "ski_carry",      3, 1),
    ("0x10109", "Skitour Vorsicht", "ski_caution",   4, 1),
    ("0x1010a", "Schneeschuhtrail", "snowshoe",      4, 1),
    ("0x1010b", "Winterwanderweg", "winter_hiking",  4, 1),
    # Cycling preset.
    ("0x1010c", "Veloroute",       "cycle_route",     4, 1),
    ("0x1010d", "Mountainbikeroute", "mtb_route",     4, 1),
    ("0x1010e", "Singletrail",     "mtb_singletrail", 3, 1),
    ("0x1a",    "Faehre",         "water_line",     2, 0),
    ("0x18",    "Bach",           "water_line",     2, 0),
    ("0x1f",    "Fluss",          "water_line",     4, 0),
    ("0x20",    "Hoehenkurve",    "contour_minor",  1, 0),
    ("0x21",    "Hoehenkurve",    "contour_medium", 1, 0),
    ("0x22",    "Hoehenkurve",    "contour_major",  2, 0),
    ("0x23",    "Hoehenkurve Eis","contour_ice",    1, 0),
]


def build(wrist: bool, winter: bool = False) -> str:
    """Render the TYP. `winter` selects the measured winter colours (FR-CART11).

    Colour-only: the winter sheet recolours the base map so the routes printed on it
    read, and the geometry rules are the same either way. See tools/winter_palette.py.
    """
    key = "winter" if winter else "day"
    L: list[str] = []
    w = L.append
    w(";-----------------------------------------------------------------------------")
    w("; swisstopo2garmin TYP -- GENERATED FILE, DO NOT EDIT.")
    if wrist:
        w("; WRIST VARIANT: thinner linework for a ~1.3 inch screen (FR-CART6).")
    if winter:
        w("; WINTER VARIANT: colours measured from swisstopo's Winter national map")
        w(";   (FR-CART11); regenerate the measurements with tools/winter_palette.py.")
    w(";   source: cartography/palette.json + tools/make_typ.py")
    w(";   regenerate: python3 tools/make_typ.py")
    w(";")
    w("; Colours are sampled from swisstopo raster products (SPEC.md FR-CART9).")
    w("; Every polygon type MUST appear in [_drawOrder] or it is not drawn at all.")
    w(";-----------------------------------------------------------------------------")
    w("[_id]")
    w("ProductCode=1")
    w(f"FID={FID}")
    w("CodePage=1252")
    w("[end]")
    w("")
    w("[_drawOrder]")
    for t, order, _lab, _k in sorted(SOLID_POLYGONS + PATTERN_POLYGONS,
                                     key=lambda r: (r[1], r[0])):
        w(f"Type={t},{order}")
    w("[end]")
    w("")

    for t, _o, lab, fill in SOLID_POLYGONS:
        w("[_polygon]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        # Two colours: the device switches to the second in night mode (FR-CART2).
        w('Xpm="0 0 2 0"')
        w(f'"1 c {c(fill, key)}"')
        w(f'"2 c {c(fill, "night")}"')
        w("[end]")
        w("")

    for t, _o, lab, pkey in PATTERN_POLYGONS:
        fn, bg, ink = PATTERNS[pkey]
        rows = pattern(fn)
        w("[_polygon]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        # Four colours: day ink, day ground, night ink, night ground. mkgmap's
        # typ-compiler documents this order, and the device swaps to the night pair
        # by itself -- the pixmap is drawn only in the day colours.
        w('Xpm="32 32 4 1"')
        w(f'"# c {c(ink, key)}"')
        # A None background is transparent, so the map underneath still shows through.
        w('". c none"' if bg is None else f'". c {c(bg, key)}"')
        w(f'"3 c {c(ink, "night")}"')
        w('"4 c none"' if bg is None else f'"4 c {c(bg, "night")}"')
        for r in rows:
            w(f'"{r}"')
        w("[end]")
        w("")

    for t, lab, icon in POINTS:
        w("[_point]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        if icon is None:
            # A fully transparent 1x1 icon: the label still draws, the symbol does not.
            w('DayXpm="1 1 1 1"')
            w('"  c none"')
            w('" "')
        else:
            size = 5 if wrist else 7
            ink = {"dot": "building", "hut": "hike_red", "stop": "rail"}[icon]
            w(f'DayXpm="{size} {size} 2 1"')
            w('"  c none"')
            w(f'"# c {c(ink, key)}"')
            for row in range(size):
                if icon == "hut":
                    # A gable: wide at the base, narrowing to a peak.
                    half = size // 2
                    span = row  # widens downwards from the apex
                    on = [abs(col - half) <= span for col in range(size)]
                elif icon == "stop":
                    # A filled square with a gap, so it reads as a marker not a building.
                    on = [row in (0, size - 1) or col in (0, size - 1) for col in range(size)]
                else:
                    edge = row == 0 or row == size - 1
                    on = [not edge for _ in range(size)]
                w('"' + "".join("#" if v else " " for v in on) + '"')
        if icon is not None:
            # Same size, night ink: the compiler requires matching dimensions.
            size = 5 if wrist else 7
            ink = {"dot": "building", "hut": "hike_red", "stop": "rail"}[icon]
            w(f'NightXpm="{size} {size} 2 1"')
            w('"  c none"')
            w(f'"# c {c(ink, "night")}"')
            for row in range(size):
                if icon == "hut":
                    half = size // 2
                    on = [abs(col - half) <= row for col in range(size)]
                elif icon == "stop":
                    on = [row in (0, size - 1) or col in (0, size - 1) for col in range(size)]
                else:
                    edge = row == 0 or row == size - 1
                    on = [not edge for _ in range(size)]
                w('"' + "".join("#" if v else " " for v in on) + '"')
        w("FontStyle=SmallFont" if wrist else "FontStyle=NormalFont")
        w("[end]")
        w("")

    for t, lab, line_key, width, border in LINES:
        entry = PALETTE[line_key]
        if wrist:
            width = max(WRIST_MIN_WIDTH, round(width * WRIST_LINE_SCALE))
            border = min(border, 1)
        w("[_line]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        if border and "casing" in entry:
            # Day line, day casing, night line, night casing.
            w('Xpm="0 0 4 0"')
            w(f'"1 c {entry[key]}"')
            # Casings are dark outlines whose job is separation, not tint, so the
            # winter sheet keeps them.
            w(f'"2 c {entry["casing"]}"')
            w(f'"3 c {entry["night"]}"')
            w(f'"4 c {entry.get("casing_night", entry["casing"])}"')
            w(f"LineWidth={width}")
            w(f"BorderWidth={border}")
        else:
            w('Xpm="0 0 2 0"')
            w(f'"1 c {entry[key]}"')
            w(f'"2 c {entry["night"]}"')
            w(f"LineWidth={width}")
        w("[end]")
        w("")

    return "\n".join(L)


def main() -> None:
    variants = (
        (False, False, "swisstopo.txt"),
        (True, False, "swisstopo-wrist.txt"),
        (False, True, "swisstopo-winter.txt"),
        (True, True, "swisstopo-wrist-winter.txt"),
    )
    for wrist, winter, name in variants:
        out = REPO / "typ" / name
        out.write_text(build(wrist, winter))
        print(f"wrote {out}")
    print(
        f"  {len(SOLID_POLYGONS)} solid polygons, {len(PATTERN_POLYGONS)} pattern fills, "
        f"{len(LINES)} lines"
    )


if __name__ == "__main__":
    main()
