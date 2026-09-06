"""S0.3 — contour lines from swissALTI3D, streamed over HTTP (spike).

Enumerates the 1 km swissALTI3D tiles intersecting a bbox via STAC, builds a GDAL VRT
over /vsicurl/ URLs so the cloud-optimized GeoTIFFs are read by range request rather
than downloaded, runs gdal_contour, and emits OSM ways using the tagging convention
mkgmap styles already expect (SPEC.md FR-P6):
    contour=elevation, ele=<m>, contour_ext=elevation_minor|medium|major

The spike uses the GDAL CLI. Production is a Rust COG reader + marching squares
(SPEC.md §7.4 / Milestone 3).
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from tlm2osm import lv95_to_wgs84, parse_gpkg_geom  # noqa: E402
from stac import cache_dir as _cache_dir  # noqa: E402

COLLECTION = "ch.swisstopo.swissalti3d"
ASSET_SUFFIX = "_2_2056_5728.tif"      # 2 m resolution; 0.5 m is 16x data for no gain


def stac_tiles(bbox_wgs84: tuple[float, float, float, float]) -> list[str]:
    """Newest 2 m GeoTIFF href per 1 km cell intersecting the bbox.

    IMPORTANT: swissALTI3D publishes an item per acquisition campaign, so the same
    1 km cell is returned for several years (verified: every cell in the Grindelwald
    bbox exists for both 2019 and 2022). Feeding all of them to gdalbuildvrt creates
    overlapping rasters -- double the bytes read, and elevations that depend on VRT
    ordering. Deduplicate by cell, keeping the most recent year.
    """
    url = (f"https://data.geo.admin.ch/api/stac/v1/collections/{COLLECTION}/items"
           f"?bbox={bbox_wgs84[0]},{bbox_wgs84[1]},{bbox_wgs84[2]},{bbox_wgs84[3]}&limit=100")
    hrefs, pages = [], 0
    while url:
        with urllib.request.urlopen(url, timeout=120) as r:
            doc = json.load(r)
        pages += 1
        for f in doc.get("features", []):
            for name, a in f.get("assets", {}).items():
                if name.endswith(ASSET_SUFFIX):
                    hrefs.append(a["href"])
        url = next((l["href"] for l in doc.get("links", []) if l.get("rel") == "next"), None)
        print(f"  STAC page {pages}: {len(hrefs)} tile assets so far", flush=True)

    best: dict[str, tuple[int, str]] = {}
    for h in hrefs:
        m = re.search(r"swissalti3d_(\d{4})_(\d{4}-\d{4})_", h)
        if not m:
            continue
        year, cell = int(m.group(1)), m.group(2)
        if cell not in best or year > best[cell][0]:
            best[cell] = (year, h)
    dropped = len(hrefs) - len(best)
    years = sorted({y for y, _ in best.values()})
    print(f"  deduplicated to {len(best)} cells (dropped {dropped} older duplicates); "
          f"years kept: {years}")
    return [href for _cell, (_year, href) in sorted(best.items())]


TILE_PX = 500          # 500 x 500 pixels
TILE_RES = 2.0         # 2 m
TILE_M = 1000          # 1 km cell
NODATA = -9999.0


def write_grid_vrt(hrefs: list[str], vrt: Path) -> float:
    """Write a VRT mosaic derived purely from the tile grid -- no remote reads."""
    t0 = time.time()
    cells = []
    for h in hrefs:
        m = re.search(r"swissalti3d_\d{4}_(\d{4})-(\d{4})_", h)
        if m:
            cells.append((int(m.group(1)), int(m.group(2)), h))
    if not cells:
        raise SystemExit("no parseable tile names")

    min_e = min(c[0] for c in cells)
    max_e = max(c[0] for c in cells)
    min_n = min(c[1] for c in cells)
    max_n = max(c[1] for c in cells)
    cols = (max_e - min_e + 1)
    rows = (max_n - min_n + 1)
    width, height = cols * TILE_PX, rows * TILE_PX
    origin_x = min_e * TILE_M
    origin_y = (max_n + 1) * TILE_M      # top edge

    parts = [
        f'<VRTDataset rasterXSize="{width}" rasterYSize="{height}">',
        '  <SRS>EPSG:2056</SRS>',
        f'  <GeoTransform>{origin_x}, {TILE_RES}, 0.0, {origin_y}, 0.0, -{TILE_RES}</GeoTransform>',
        '  <VRTRasterBand dataType="Float32" band="1">',
        f'    <NoDataValue>{NODATA}</NoDataValue>',
    ]
    for e, n, h in cells:
        dx = (e - min_e) * TILE_PX
        dy = (max_n - n) * TILE_PX
        parts += [
            '    <SimpleSource>',
            f'      <SourceFilename relativeToVRT="0">/vsicurl/{h}</SourceFilename>',
            '      <SourceBand>1</SourceBand>',
            f'      <SrcRect xOff="0" yOff="0" xSize="{TILE_PX}" ySize="{TILE_PX}"/>',
            f'      <DstRect xOff="{dx}" yOff="{dy}" xSize="{TILE_PX}" ySize="{TILE_PX}"/>',
            f'      <NODATA>{NODATA}</NODATA>',
            '    </SimpleSource>',
        ]
    parts += ['  </VRTRasterBand>', '</VRTDataset>', '']
    vrt.write_text("\n".join(parts))
    print(f"  grid {cols} x {rows} cells -> {width} x {height} px, "
          f"origin E={origin_x} N={origin_y}")
    return time.time() - t0


# --- ice mask -----------------------------------------------------------------
# The Landeskarte draws contours blue over glaciers and firn rather than bistre
# (SPEC.md FR-CART9). Contours are classified by testing their midpoint against the
# swissTLM3D ice polygons. A contour that only partly crosses ice takes the class of
# its midpoint -- acceptable at Garmin resolution, and far cheaper than splitting.
ICE_CLASSES = ("Gletscher", "Schneefeld Toteis")


def load_ice(bbox_lv95) -> list:
    import sqlite3
    root = _cache_dir() / "ch.swisstopo.swisstlm3d"
    gpkgs = sorted(root.glob("*/*.gpkg"))
    if not gpkgs:
        print("  no swissTLM3D GeoPackage cached -- skipping ice classification")
        return []
    mine, minn, maxe, maxn = bbox_lv95
    con = sqlite3.connect(f"file:{gpkgs[-1]}?mode=ro", uri=True)
    q = ("SELECT t.geom FROM tlm_bb_bodenbedeckung t "
         "JOIN rtree_tlm_bb_bodenbedeckung_geom r ON t.id = r.id "
         "WHERE r.maxx>=? AND r.minx<=? AND r.maxy>=? AND r.miny<=? "
         f"AND t.objektart IN ({','.join('?' * len(ICE_CLASSES))})")
    polys = []
    for (blob,) in con.execute(q, (mine, maxe, minn, maxn, *ICE_CLASSES)):
        _, rings = parse_gpkg_geom(blob)
        for ring in rings:
            if len(ring) < 4:
                continue
            xs = [p[0] for p in ring]; ys = [p[1] for p in ring]
            polys.append((min(xs), min(ys), max(xs), max(ys), ring))
    print(f"  ice polygons in area: {len(polys)}")
    return polys


def in_ice(e: float, n: float, polys: list) -> bool:
    for x0, y0, x1, y1, ring in polys:
        if not (x0 <= e <= x1 and y0 <= n <= y1):
            continue
        inside = False
        j = len(ring) - 1
        for i in range(len(ring)):
            xi, yi = ring[i]; xj, yj = ring[j]
            if (yi > n) != (yj > n) and e < (xj - xi) * (n - yi) / (yj - yi + 1e-12) + xi:
                inside = not inside
            j = i
        if inside:
            return True
    return False


def classify(ele: float, major: int, medium: int) -> str:
    e = int(round(ele))
    if major and e % major == 0:
        return "elevation_major"
    if medium and e % medium == 0:
        return "elevation_medium"
    return "elevation_minor"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bbox-lv95", nargs=4, type=float, required=True,
                    metavar=("MINE", "MINN", "MAXE", "MAXN"))
    ap.add_argument("--interval", type=int, default=20)
    ap.add_argument("--medium", type=int, default=50)
    ap.add_argument("--major", type=int, default=100)
    ap.add_argument("--simplify", type=float, default=8.0,
                    help="Douglas-Peucker tolerance in metres (0 disables)")
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--id-offset", type=int, default=100_000_000,
                    help="OSM id base; must not collide with the vector extract")
    ap.add_argument("--no-ice", action="store_true",
                    help="skip blue-over-ice contour classification")
    ap.add_argument("--reuse", action="store_true",
                    help="reuse an existing contours.geojson instead of re-contouring")
    ap.add_argument("-o", "--out", required=True)
    args = ap.parse_args()

    work = Path(args.workdir); work.mkdir(parents=True, exist_ok=True)
    mine, minn, maxe, maxn = args.bbox_lv95
    lon0, lat0 = lv95_to_wgs84(mine, minn)
    lon1, lat1 = lv95_to_wgs84(maxe, maxn)
    area_km2 = (maxe - mine) * (maxn - minn) / 1e6

    print(f"area           : {area_km2:.0f} km2")
    print("enumerating swissALTI3D tiles...")
    t0 = time.time()
    hrefs = stac_tiles((lon0, lat0, lon1, lat1))
    t_stac = time.time() - t0
    print(f"  {len(hrefs)} tiles in {t_stac:.1f}s")
    if not hrefs:
        raise SystemExit("no elevation tiles found")

    # total remote size, for the bytes-per-km2 figure
    total_remote = 0
    for h in hrefs[: min(len(hrefs), 12)]:
        req = urllib.request.Request(h, method="HEAD")
        with urllib.request.urlopen(req, timeout=60) as r:
            total_remote += int(r.headers["Content-Length"])
    avg = total_remote / min(len(hrefs), 12)
    print(f"  mean tile size {avg/1e6:.2f} MB -> ~{avg*len(hrefs)/1e6:.0f} MB if fully read")

    listfile = work / "alti_tiles.txt"
    listfile.write_text("\n".join(f"/vsicurl/{h}" for h in hrefs) + "\n")
    vrt = work / "alti.vrt"

    # gdalbuildvrt would open all 181 remote files just to read their geotransforms,
    # which is minutes of HTTP round-trips. The tiling is a regular 1 km LV95 grid
    # (verified: cell EEEE-NNNN has origin (EEEE*1000, (NNNN+1)*1000), 500x500 px at
    # 2 m, Float32, nodata -9999), so the VRT is fully derivable with zero remote reads.
    print("\nbuilding VRT (derived from the tile grid, no remote reads)...")
    t0 = time.time()
    t_vrt = write_grid_vrt(hrefs, vrt)
    print(f"  VRT built in {t_vrt:.2f}s")

    gj = work / "contours.geojson"
    if args.reuse and gj.exists():
        print(f"\nreusing existing {gj.name} ({gj.stat().st_size/1e6:.1f} MB)")
        t_contour = 0.0
    else:
        gj.unlink(missing_ok=True)
        print(f"\nrunning gdal_contour at {args.interval} m ...")
        t0 = time.time()
        cmd = ["gdal_contour", "-a", "ELEV", "-i", str(args.interval),
               "-f", "GeoJSON", str(vrt), str(gj)]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            print(res.stdout[-2000:]); print(res.stderr[-2000:])
            raise SystemExit("gdal_contour failed")
        t_contour = time.time() - t0
        print(f"  contours generated in {t_contour:.1f}s "
              f"({gj.stat().st_size/1e6:.1f} MB GeoJSON)")

    # --- GeoJSON -> OSM ways, with simplification -------------------------
    print("\nconverting to OSM...")
    t0 = time.time()
    data = json.loads(gj.read_text())
    feats = data["features"]

    def simplify(pts: list[tuple[float, float]], tol: float) -> list[tuple[float, float]]:
        if tol <= 0 or len(pts) < 3:
            return pts
        keep = [False] * len(pts)
        keep[0] = keep[-1] = True
        stack = [(0, len(pts) - 1)]
        while stack:
            i, j = stack.pop()
            if j <= i + 1:
                continue
            ax, ay = pts[i]; bx, by = pts[j]
            dx, dy = bx - ax, by - ay
            den = dx * dx + dy * dy
            best, bidx = -1.0, -1
            for k in range(i + 1, j):
                px, py = pts[k]
                if den == 0:
                    d = ((px - ax) ** 2 + (py - ay) ** 2) ** 0.5
                else:
                    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / den))
                    cx, cy = ax + t * dx, ay + t * dy
                    d = ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5
                if d > best:
                    best, bidx = d, k
            if best > tol:
                keep[bidx] = True
                stack.append((i, bidx)); stack.append((bidx, j))
        return [p for p, k in zip(pts, keep) if k]

    ice = [] if args.no_ice else load_ice((mine, minn, maxe, maxn))

    nodes: dict[tuple[int, int], int] = {}
    node_lines: list[str] = []
    way_lines: list[str] = []
    nid = [args.id_offset]; wid = [args.id_offset]
    pts_before = pts_after = 0
    n_ice = 0

    def node_id(e: float, n: float) -> int:
        lon, lat = lv95_to_wgs84(e, n)
        key = (int(round(lon * 1e7)), int(round(lat * 1e7)))
        got = nodes.get(key)
        if got is None:
            got = nid[0]; nid[0] += 1
            nodes[key] = got
            node_lines.append(
                f'  <node id="{got}" lat="{key[1]/1e7:.7f}" lon="{key[0]/1e7:.7f}" version="1"/>')
        return got

    for f in feats:
        ele = f["properties"].get("ELEV")
        if ele is None:
            continue
        geom = f["geometry"]
        parts = [geom["coordinates"]] if geom["type"] == "LineString" else geom["coordinates"]
        for part in parts:
            pts = [(c[0], c[1]) for c in part]
            pts_before += len(pts)
            pts = simplify(pts, args.simplify)
            pts_after += len(pts)
            if len(pts) < 2:
                continue
            refs = [node_id(e, n) for e, n in pts]
            cls = classify(ele, args.major, args.medium)
            mx, my = pts[len(pts) // 2]
            on_ice = bool(ice) and in_ice(mx, my, ice)
            if on_ice:
                n_ice += 1
            w = [f'  <way id="{wid[0]}" version="1">']
            wid[0] += 1
            w += [f'    <nd ref="{r}"/>' for r in refs]
            w.append('    <tag k="contour" v="elevation"/>')
            w.append(f'    <tag k="ele" v="{int(round(ele))}"/>')
            w.append(f'    <tag k="contour_ext" v="{cls}"/>')
            if on_ice:
                w.append('    <tag k="contour_surface" v="ice"/>')
            w.append("  </way>")
            way_lines.append("\n".join(w))

    out = Path(args.out)
    with out.open("w", encoding="utf-8") as fh:
        fh.write('<?xml version="1.0" encoding="UTF-8"?>\n')
        fh.write('<osm version="0.6" generator="swisstopo2garmin-contours">\n')
        fh.write(f'  <bounds minlat="{min(lat0,lat1):.7f}" minlon="{min(lon0,lon1):.7f}" '
                 f'maxlat="{max(lat0,lat1):.7f}" maxlon="{max(lon0,lon1):.7f}"/>\n')
        fh.write("\n".join(node_lines) + "\n")
        fh.write("\n".join(way_lines) + "\n")
        fh.write("</osm>\n")
    t_osm = time.time() - t0

    print(f"  {len(feats):,} contour features -> {wid[0]-args.id_offset:,} ways, {len(nodes):,} nodes")
    if ice:
        print(f"  contours over ice: {n_ice:,} of {wid[0]-args.id_offset:,}")
    print(f"  simplification: {pts_before:,} -> {pts_after:,} points "
          f"({100*(1-pts_after/max(pts_before,1)):.0f}% reduction, tol={args.simplify} m)")
    print(f"  wrote {out} ({out.stat().st_size/1e6:.1f} MB) in {t_osm:.1f}s")
    print(f"\nTIMING  stac={t_stac:.1f}s vrt={t_vrt:.1f}s contour={t_contour:.1f}s osm={t_osm:.1f}s")
    print(f"        {area_km2:.0f} km2 -> {(t_stac+t_vrt+t_contour+t_osm)/area_km2:.2f} s/km2")


if __name__ == "__main__":
    main()
