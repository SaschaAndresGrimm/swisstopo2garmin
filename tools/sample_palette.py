"""Sample swisstopo's own raster maps to measure the Landeskarte palette (FR-CART9).

Colours in cartography/palette.json must come from swisstopo's published cartography,
not from someone's eye. This fetches WMTS tiles over locations chosen for a single
dominant land cover, discards paper-white and ink-black, and reports the dominant tints.

Prefer ch.swisstopo.swisstlm3d-karte-farbe -- it is served as lossless PNG.
ch.swisstopo.pixelkarte-farbe (true LK25) is JPEG, so its colours are smeared by
compression: use it to read symbology, not exact values.
"""
from __future__ import annotations

import argparse
import collections
import io
import json
import math
import urllib.request

from PIL import Image

WMTS = ("https://wmts.geo.admin.ch/1.0.0/{layer}/default/current/3857/"
        "{z}/{x}/{y}.{fmt}")

# locations chosen so one cover type dominates the tile
SPOTS = {
    "forest":      (46.6100, 8.0100),
    "glacier":     (46.5450, 8.0900),
    "rock":        (46.5775, 8.0053),
    "water":       (46.7300, 7.9700),
    "farmland":    (46.9480, 7.4474),
    "village":     (46.6234, 8.0382),
    "vineyard":    (46.4900, 6.7800),
    "wetland":     (46.9900, 7.0500),
}


def fetch(lat: float, lon: float, z: int, layer: str, fmt: str) -> Image.Image:
    n = 2 ** z
    x = int((lon + 180) / 360 * n)
    lr = math.radians(lat)
    y = int((1 - math.log(math.tan(lr) + 1 / math.cos(lr)) / math.pi) / 2 * n)
    url = WMTS.format(layer=layer, z=z, x=x, y=y, fmt=fmt)
    req = urllib.request.Request(url, headers={"User-Agent": "swisstopo2garmin/0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return Image.open(io.BytesIO(r.read())).convert("RGB")


def is_tint(c: tuple[int, int, int]) -> bool:
    r, g, b = c
    if max(c) > 246 and min(c) > 240:
        return False                                   # paper white
    if max(c) < 60:
        return False                                   # ink black
    if abs(r - g) < 4 and abs(g - b) < 4 and abs(r - b) < 4:
        return False                                   # neutral grey (text, hachure)
    return True


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--layer", default="ch.swisstopo.swisstlm3d-karte-farbe")
    ap.add_argument("--fmt", default="png")
    ap.add_argument("--zoom", type=int, default=15)
    ap.add_argument("--json", help="write the measurements to this file")
    args = ap.parse_args()

    print(f"layer {args.layer} ({args.fmt}) at z{args.zoom}\n")
    out = {}
    for name, (la, lo) in SPOTS.items():
        try:
            im = fetch(la, lo, args.zoom, args.layer, args.fmt)
        except Exception as e:
            print(f"{name:<10} ERR {e}")
            continue
        c = collections.Counter(p for p in im.getdata() if is_tint(p))
        total = sum(c.values()) or 1
        top = [("#%02X%02X%02X" % rgb, round(100 * n / total, 1))
               for rgb, n in c.most_common(3)]
        out[name] = {"lat": la, "lon": lo, "dominant": top, "tinted_px": total}
        print(f"{name:<10} tinted={total:>6,}  " +
              "  ".join(f"{h} {p:4.1f}%" for h, p in top))

    if args.json:
        with open(args.json, "w") as fh:
            json.dump({"layer": args.layer, "zoom": args.zoom, "spots": out}, fh, indent=2)
        print(f"\nwrote {args.json}")


if __name__ == "__main__":
    main()
