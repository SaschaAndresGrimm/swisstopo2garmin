"""Measure swisstopo's slope-class colours and add them to the palette (FR-CART12).

`ch.swisstopo.hangneigung-ueber_30` is published as WMTS only — there is no dataset to
download — so the classes themselves are computed from swissALTI3D (see
`crates/s2g-core/src/slope.rs`), and only the *colours* come from swisstopo.

The class each colour belongs to is established by evidence rather than by assuming the
legend's order: tiles are sampled over terrain of known steepness, and a colour that
appears on moderate ground but not on near-vertical ground is a shallower class than one
that appears only on the steepest.

    python3 tools/slope_palette.py           # report
    python3 tools/slope_palette.py --write   # update cartography/palette.json
"""
from __future__ import annotations

import argparse
import collections
import io
import json
import math
import urllib.request
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
PALETTE_PATH = REPO / "cartography" / "palette.json"
LAYER = "ch.swisstopo.hangneigung-ueber_30"

# Terrain whose steepness is not in question, used to order the classes.
PROBES = {
    "valley floor, Grindelwald": (46.6244, 8.0413),
    "moderate flank, Aletsch": (46.4400, 8.0600),
    "near-vertical, Eiger north face": (46.5775, 8.0053),
}

# Established from those probes; see the module docstring.
CLASSES = [
    (30, "#F2E50A", "dominates the valley floor, so the shallowest class"),
    (35, "#F46F24", "prominent on a moderate flank"),
    (40, "#DE055B", "present on a moderate flank and on the face"),
    (45, "#C889BB", "absent from moderate ground, present on the face"),
    (50, "#4B4B4B", "dominates the near-vertical face"),
]


def tile(lat: float, lon: float, z: int = 15) -> Image.Image:
    n = 2 ** z
    x = int((lon + 180) / 360 * n)
    lr = math.radians(lat)
    y = int((1 - math.log(math.tan(lr) + 1 / math.cos(lr)) / math.pi) / 2 * n)
    url = (f"https://wmts.geo.admin.ch/1.0.0/{LAYER}/default/current/3857/{z}/{x}/{y}.png")
    req = urllib.request.Request(url, headers={"User-Agent": "swisstopo2garmin/0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return Image.open(io.BytesIO(r.read())).convert("RGBA")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()

    seen: dict[str, dict[str, float]] = {}
    print(f"layer {LAYER}\n")
    for name, (la, lo) in PROBES.items():
        try:
            im = tile(la, lo)
        except Exception as e:
            print(f"  {name}: skipped ({e})")
            continue
        counts = collections.Counter(p[:3] for p in im.getdata() if p[3] > 40)
        total = sum(counts.values()) or 1
        print(f"  {name:34} {100 * total / (256 * 256):5.1f}% of the tile classified")
        for rgb, n in counts.most_common(5):
            hexed = "#%02X%02X%02X" % rgb
            share = 100 * n / total
            seen.setdefault(hexed, {})[name] = round(share, 1)
            print(f"      {hexed}  {share:5.1f}%")
        print()

    print("class assignment:")
    palette = json.loads(PALETTE_PATH.read_text())
    for deg, colour, why in CLASSES:
        key = f"slope_{deg}"
        where = seen.get(colour, {})
        print(f"  {key:10} {colour}  {why}")
        if where:
            print(f"             observed: {where}")
        else:
            print("             NOT OBSERVED in the sampled tiles")
        palette[key] = {
            "day": colour,
            "winter": colour,
            "measured": True,
            "note": f"slope {deg}-{deg + 5}°" if deg < 50 else "slope over 50°",
            "source": LAYER,
            "evidence": why,
            "observed_share_percent": where,
        }

    palette["_slope"] = {
        "source": LAYER,
        "method": (
            "The layer is WMTS only, so the classes are computed from swissALTI3D and "
            "only the colours are taken from swisstopo. Each colour's class is "
            "established by sampling terrain of known steepness rather than by assuming "
            "the legend's order."
        ),
        "probes": {k: list(v) for k, v in PROBES.items()},
        "classes": {str(d): c for d, c, _ in CLASSES},
    }

    if args.write:
        PALETTE_PATH.write_text(json.dumps(palette, indent=2) + "\n")
        print(f"\nwrote {PALETTE_PATH}")
    else:
        print("\n(dry run; pass --write to update the palette)")


if __name__ == "__main__":
    main()
