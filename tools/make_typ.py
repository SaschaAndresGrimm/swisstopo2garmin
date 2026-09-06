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


def vineyard_rows(x: int, y: int) -> bool:
    if x % 8 != 0:
        return False
    return ((y + (x // 8) * 4) % 12) < 7


PATTERNS = {
    "rock":     (rock_hachure, "paper", "rock_ink"),
    "scree":    (scree_stipple, "paper", "scree_ink"),
    "wetland":  (wetland_ticks, "paper", "wetland_ink"),
    "vineyard": (vineyard_rows, "green_mid", "wetland_ink"),
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
    ("0x1a",    "Faehre",         "water_line",     2, 0),
    ("0x18",    "Bach",           "water_line",     2, 0),
    ("0x1f",    "Fluss",          "water_line",     4, 0),
    ("0x20",    "Hoehenkurve",    "contour_minor",  1, 0),
    ("0x21",    "Hoehenkurve",    "contour_medium", 1, 0),
    ("0x22",    "Hoehenkurve",    "contour_major",  2, 0),
    ("0x23",    "Hoehenkurve Eis","contour_ice",    1, 0),
]


def build(wrist: bool) -> str:
    L: list[str] = []
    w = L.append
    w(";-----------------------------------------------------------------------------")
    w("; swisstopo2garmin TYP -- GENERATED FILE, DO NOT EDIT.")
    if wrist:
        w("; WRIST VARIANT: thinner linework for a ~1.3 inch screen (FR-CART6).")
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

    for t, _o, lab, key in SOLID_POLYGONS:
        w("[_polygon]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        w('Xpm="0 0 1 0"')
        w(f'"1 c {c(key)}"')
        w("[end]")
        w("")

    for t, _o, lab, pkey in PATTERN_POLYGONS:
        fn, bg, ink = PATTERNS[pkey]
        rows = pattern(fn)
        w("[_polygon]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        w('Xpm="32 32 2 1"')
        w(f'". c {c(bg)}"')
        w(f'"# c {c(ink)}"')
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
            w(f'DayXpm="{size} {size} 2 1"')
            w('"  c none"')
            w(f'"# c {c("building")}"')
            for row in range(size):
                edge = row == 0 or row == size - 1
                w('"' + ("".join(" " if edge else "#" for _ in range(size))) + '"')
        w("FontStyle=SmallFont" if wrist else "FontStyle=NormalFont")
        w("[end]")
        w("")

    for t, lab, key, width, border in LINES:
        entry = PALETTE[key]
        if wrist:
            width = max(WRIST_MIN_WIDTH, round(width * WRIST_LINE_SCALE))
            border = min(border, 1)
        w("[_line]")
        w(f"Type={t}")
        w(f"String1=0x04,{lab}")
        if border and "casing" in entry:
            w('Xpm="0 0 2 0"')
            w(f'"1 c {entry["day"]}"')
            w(f'"2 c {entry["casing"]}"')
            w(f"LineWidth={width}")
            w(f"BorderWidth={border}")
        else:
            w('Xpm="0 0 1 0"')
            w(f'"1 c {entry["day"]}"')
            w(f"LineWidth={width}")
        w("[end]")
        w("")

    return "\n".join(L)


def main() -> None:
    for wrist, name in ((False, "swisstopo.txt"), (True, "swisstopo-wrist.txt")):
        out = REPO / "typ" / name
        out.write_text(build(wrist))
        print(f"wrote {out}")
    print(
        f"  {len(SOLID_POLYGONS)} solid polygons, {len(PATTERN_POLYGONS)} pattern fills, "
        f"{len(LINES)} lines"
    )


if __name__ == "__main__":
    main()
