"""Generate SRTM .hgt tiles from swissALTIRegio for mkgmap --dem (SPEC.md FR-CART8).

Shipping DEM data inside the .img makes Garmin devices render shaded relief natively,
which is the single largest step toward the look of the swisstopo raster maps.

Source: ch.swisstopo.swissaltiregio -- ONE national cloud-optimized GeoTIFF, 10 m,
55000x43000 px, EPSG:2056, with 5 overview levels. Covering a full 1x1 degree .hgt cell
from the 2 m swissALTI3D tiles would mean reading ~10 GB of COGs; swissALTIRegio serves
the same cell from overviews in a few MB.

mkgmap expects classic SRTM naming and geometry:
  1 arc-second -> 3601x3601, 3 arc-second -> 1201x1201, big-endian int16, 1 degree cells.
"""
from __future__ import annotations

import argparse
import math
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from tlm2osm import lv95_to_wgs84  # noqa: E402

COG = ("/vsicurl/https://data.geo.admin.ch/ch.swisstopo.swissaltiregio/"
       "swissaltiregio/swissaltiregio_2056_5728.tif")
SRC_NODATA = "3.4e+38"
VOID = -32768          # SRTM void value; mkgmap understands it


def cell_name(lat: int, lon: int) -> str:
    return (f"{'N' if lat >= 0 else 'S'}{abs(lat):02d}"
            f"{'E' if lon >= 0 else 'W'}{abs(lon):03d}")


def run(cmd: list[str]) -> None:
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print(" ".join(cmd))
        print(res.stdout[-1500:])
        print(res.stderr[-1500:])
        raise SystemExit("command failed")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bbox-lv95", nargs=4, type=float, required=True,
                    metavar=("MINE", "MINN", "MAXE", "MAXN"))
    ap.add_argument("--arcsec", type=int, choices=(1, 3), default=1,
                    help="1 => 3601x3601 (~30 m), 3 => 1201x1201 (~90 m)")
    ap.add_argument("--out", required=True, help="directory for the .hgt files")
    args = ap.parse_args()

    size = 3601 if args.arcsec == 1 else 1201
    step = 1.0 / (size - 1)
    half = step / 2.0

    mine, minn, maxe, maxn = args.bbox_lv95
    lons, lats = [], []
    for e, n in ((mine, minn), (mine, maxn), (maxe, minn), (maxe, maxn)):
        lo, la = lv95_to_wgs84(e, n)
        lons.append(lo); lats.append(la)

    out = Path(args.out); out.mkdir(parents=True, exist_ok=True)
    cells = [(la, lo)
             for la in range(math.floor(min(lats)), math.floor(max(lats)) + 1)
             for lo in range(math.floor(min(lons)), math.floor(max(lons)) + 1)]
    print(f"area {min(lats):.4f}..{max(lats):.4f}N {min(lons):.4f}..{max(lons):.4f}E")
    print(f"{len(cells)} cell(s) at {args.arcsec} arc-second ({size}x{size})")

    t0 = time.time()
    for la, lo in cells:
        name = cell_name(la, lo)
        hgt = out / f"{name}.hgt"
        if hgt.exists():
            print(f"  {name}: present"); continue
        tmp = out / f".{name}.tif"
        print(f"  {name}: warping from swissALTIRegio ...", flush=True)
        # pixel-is-area cells centred on the arc-second grid, which is what the
        # SRTMHGT driver expects
        run(["gdalwarp", "-q", "-overwrite",
             "-t_srs", "EPSG:4326",
             "-te", f"{lo - half}", f"{la - half}", f"{lo + 1 + half}", f"{la + 1 + half}",
             "-ts", str(size), str(size),
             "-r", "bilinear",
             "-srcnodata", SRC_NODATA, "-dstnodata", str(VOID),
             "-ot", "Int16", "-of", "GTiff",
             COG, str(tmp)])
        run(["gdal_translate", "-q", "-of", "SRTMHGT", str(tmp), str(hgt)])
        tmp.unlink(missing_ok=True)
        print(f"     -> {hgt.name} ({hgt.stat().st_size:,} B)")

    print(f"\n{len(cells)} cell(s) in {time.time()-t0:.1f}s -> {out}")
    print(f"mkgmap: --dem={out}")


if __name__ == "__main__":
    main()
