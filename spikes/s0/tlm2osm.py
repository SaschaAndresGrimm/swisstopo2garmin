"""S0.2 — clip swissTLM3D to a bbox and emit OSM XML for mkgmap (spike).

Reads the GeoPackage as plain SQLite (no GDAL), uses the GPKG R-tree indexes for the
bbox query, parses GPKG geometry blobs -> WKB, reprojects LV95 -> WGS84, and writes an
.osm file whose tags carry the raw TLM attributes under a `tlm:` prefix. The mkgmap style
rules key off those tags directly.

Layer/attribute names come from docs/tlm3d-schema.md (CLAUDE.md rule 2).
"""
from __future__ import annotations

import argparse
import math
import sqlite3
import struct
import sys
import time
from pathlib import Path
from xml.sax.saxutils import escape

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir  # noqa: E402

# ---------------------------------------------------------------------------
# Projection: CH1903+/LV95 (EPSG:2056) -> WGS84 (EPSG:4326)
#
# swisstopo's published approximate formulas. Accuracy is ~1 m, which is below the
# ~2 m coordinate resolution of the Garmin IMG format, so the error is not
# representable in the output (SPEC.md FR-P1).
# Production (Rust) should use a rigorous Hotine Oblique Mercator inverse plus the
# CHENyx06 grid shift -- see docs/m0-findings.md.
# ---------------------------------------------------------------------------
def lv95_to_wgs84(e: float, n: float) -> tuple[float, float]:
    y = (e - 2_600_000.0) / 1_000_000.0
    x = (n - 1_200_000.0) / 1_000_000.0
    lon = (2.6779094
           + 4.728982 * y
           + 0.791484 * y * x
           + 0.1306 * y * x * x
           - 0.0436 * y * y * y) * 100.0 / 36.0
    lat = (16.9023892
           + 3.238272 * x
           - 0.270978 * y * y
           - 0.002528 * x * x
           - 0.0447 * y * y * x
           - 0.0140 * x * x * x) * 100.0 / 36.0
    return lon, lat


# ---------------------------------------------------------------------------
# GPKG geometry blob -> coordinate rings
# ---------------------------------------------------------------------------
def parse_gpkg_geom(blob: bytes):
    """Return (geom_type_name, list_of_rings) where each ring is a list of (E, N)."""
    if blob is None or len(blob) < 8 or blob[:2] != b"GP":
        return None, []
    flags = blob[3]
    env_ind = (flags >> 1) & 0x07
    env_doubles = {0: 0, 1: 4, 2: 6, 3: 6, 4: 8}[env_ind]
    off = 8 + env_doubles * 8
    if flags & 0x10:            # empty geometry
        return None, []
    return parse_wkb(blob, off)


def parse_wkb(buf: bytes, off: int):
    byte_order = buf[off]
    end = "<" if byte_order == 1 else ">"
    gtype = struct.unpack_from(end + "I", buf, off + 1)[0]
    off += 5
    # ISO WKB: +1000 = Z, +2000 = M, +3000 = ZM
    has_z = (gtype // 1000) in (1, 3)
    has_m = (gtype // 1000) in (2, 3)
    base = gtype % 1000
    ndim = 2 + (1 if has_z else 0) + (1 if has_m else 0)

    def read_points(o: int, count: int):
        pts = [None] * count
        fmt = end + "d" * ndim
        size = 8 * ndim
        for i in range(count):
            vals = struct.unpack_from(fmt, buf, o)
            pts[i] = (vals[0], vals[1])   # drop Z/M (SPEC.md FR-P2)
            o += size
        return pts, o

    name = {1: "Point", 2: "LineString", 3: "Polygon",
            4: "MultiPoint", 5: "MultiLineString", 6: "MultiPolygon"}.get(base, f"type{base}")

    if base == 1:
        pts, off = read_points(off, 1)
        return name, [pts]
    if base == 2:
        n = struct.unpack_from(end + "I", buf, off)[0]; off += 4
        pts, off = read_points(off, n)
        return name, [pts]
    if base == 3:
        nring = struct.unpack_from(end + "I", buf, off)[0]; off += 4
        rings = []
        for _ in range(nring):
            n = struct.unpack_from(end + "I", buf, off)[0]; off += 4
            pts, off = read_points(off, n)
            rings.append(pts)
        return name, rings
    if base in (4, 5, 6):
        nsub = struct.unpack_from(end + "I", buf, off)[0]; off += 4
        rings = []
        for _ in range(nsub):
            _, sub = parse_wkb_at(buf, off)
            rings.extend(sub[0])
            off = sub[1]
        return name, rings
    raise ValueError(f"unsupported WKB base type {base}")


def parse_wkb_at(buf: bytes, off: int):
    """parse_wkb variant that also reports the end offset, for multi-geometries."""
    start = off
    byte_order = buf[off]
    end = "<" if byte_order == 1 else ">"
    gtype = struct.unpack_from(end + "I", buf, off + 1)[0]
    has_z = (gtype // 1000) in (1, 3)
    has_m = (gtype // 1000) in (2, 3)
    base = gtype % 1000
    ndim = 2 + (1 if has_z else 0) + (1 if has_m else 0)
    off += 5
    rings = []
    if base == 1:
        rings.append([struct.unpack_from(end + "dd", buf, off)])
        off += 8 * ndim
    elif base == 2:
        n = struct.unpack_from(end + "I", buf, off)[0]; off += 4
        pts = []
        for _ in range(n):
            v = struct.unpack_from(end + "d" * ndim, buf, off)
            pts.append((v[0], v[1])); off += 8 * ndim
        rings.append(pts)
    elif base == 3:
        nring = struct.unpack_from(end + "I", buf, off)[0]; off += 4
        for _ in range(nring):
            n = struct.unpack_from(end + "I", buf, off)[0]; off += 4
            pts = []
            for _ in range(n):
                v = struct.unpack_from(end + "d" * ndim, buf, off)
                pts.append((v[0], v[1])); off += 8 * ndim
            rings.append(pts)
    else:
        raise ValueError(f"nested multi-geometry base {base} unsupported")
    del start
    return None, (rings, off)


# ---------------------------------------------------------------------------
# Layers to export. Names verified against docs/tlm3d-schema.md.
# tlm_bb_einzelbaum (11.5 M single trees) is deliberately excluded -- it is half the
# dataset and meaningless at Garmin zoom levels.
# ---------------------------------------------------------------------------
LAYERS = {
    "tlm_strassen_strasse": ["objektart", "wanderwege", "belagsart", "kunstbaute", "stufe",
                             "verkehrsbedeutung", "verkehrsbeschraenkung", "befahrbarkeit",
                             "richtungsgetrennt", "strassenname"],
    "tlm_bb_bodenbedeckung": ["objektart"],
    # Land use ("Areale") -- these carry the lived-in texture of a Landeskarte:
    # vineyards, orchards, cemeteries, parks, school and hospital grounds, parking.
    "tlm_areale_nutzungsareal": ["objektart", "name"],
    "tlm_areale_freizeitareal": ["objektart", "name"],
    "tlm_areale_verkehrsareal": ["objektart", "name"],
    "tlm_gewaesser_fliessgewaesser": ["objektart", "name", "verlauf"],
    "tlm_gewaesser_stehendes_gewaesser": ["objektart", "name"],
    "tlm_bauten_gebaeude_footprint": ["objektart"],
    "tlm_oev_eisenbahn": ["objektart", "name"],
    "tlm_namen_flurname": ["objektart", "name"],
    "tlm_namen_siedlungsname_zentrum": ["objektart", "name", "einwohnerkategorie"],
    "tlm_eo_einzelobjekt": ["objektart", "name"],
}

NULLISH = {None, "", "k_W", "Keine Angabe"}

# Ordering for tlm_namen_siedlungsname_zentrum.einwohnerkategorie, used to pick the
# most significant place when a name is ambiguous. Values verified against
# docs/tlm3d-schema.md.
POP_RANK = {
    "> 100'000": 8,
    "50'000 bis 100'000": 7,
    "10'000 bis 49'999": 6,
    "2'000 bis 9'999": 5,
    "1'000 bis 1'999": 4,
    "100 bis 999": 3,
    "50 bis 99": 2,
    "20 bis 49": 1,
    "< 20": 0,
}


def table_columns(cur, table: str) -> set[str]:
    return {r[1] for r in cur.execute(f'PRAGMA table_info("{table}")')}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bbox", nargs=4, type=float, metavar=("MINE", "MINN", "MAXE", "MAXN"),
                    help="LV95 bounding box")
    ap.add_argument("--place", help="centre the bbox on a settlement name")
    ap.add_argument("--radius-km", type=float, default=6.0)
    ap.add_argument("-o", "--out", default="out.osm")
    args = ap.parse_args()

    root = cache_dir() / "ch.swisstopo.swisstlm3d"
    gpkg = sorted(root.glob("*/*.gpkg"))[-1]
    con = sqlite3.connect(f"file:{gpkg}?mode=ro", uri=True)
    con.text_factory = lambda b: b.decode("utf-8", "replace")
    cur = con.cursor()

    if args.place:
        # Place names are NOT unique. "Grindelwald" matches both the well-known
        # 2,000-9,999 inhabitant village AND a <20 inhabitant hamlet 45 km north --
        # a bare LIMIT 1 silently built maps of the wrong place. Rank by population
        # category and always report the alternatives.
        rows = cur.execute(
            "SELECT name, einwohnerkategorie, geom FROM tlm_namen_siedlungsname_zentrum "
            "WHERE name = ?", (args.place,)).fetchall()
        if not rows:
            raise SystemExit(f"place {args.place!r} not found")

        cands = []
        for name, ew, geom in rows:
            _, rings = parse_gpkg_geom(geom)
            if not rings:
                continue
            e, n = rings[0][0]
            cands.append((POP_RANK.get(ew, -1), ew, e, n))
        if not cands:
            raise SystemExit(f"place {args.place!r} has no usable geometry")
        cands.sort(reverse=True)

        if len(cands) > 1:
            print(f"note: {len(cands)} places named {args.place!r}:")
            for rank, ew, e, n in cands:
                lon, lat = lv95_to_wgs84(e, n)
                print(f"      {str(ew):<18} E={e:.0f} N={n:.0f}  ({lat:.4f}N {lon:.4f}E)")
            print("      -> using the most populous; pass --bbox to override")

        _, ew, e, n = cands[0]
        r = args.radius_km * 1000
        bbox = (e - r, n - r, e + r, n + r)
        lon, lat = lv95_to_wgs84(e, n)
        print(f"place {args.place!r} [{ew}] at E={e:.0f} N={n:.0f} "
              f"({lat:.4f}N {lon:.4f}E), radius {args.radius_km} km")
    elif args.bbox:
        bbox = tuple(args.bbox)
    else:
        raise SystemExit("need --bbox or --place")

    mine, minn, maxe, maxn = bbox
    lon0, lat0 = lv95_to_wgs84(mine, minn)
    lon1, lat1 = lv95_to_wgs84(maxe, maxn)
    area = (maxe - mine) * (maxn - minn) / 1e6
    print(f"bbox LV95  : {mine:.0f} {minn:.0f} {maxe:.0f} {maxn:.0f}  ({area:.0f} km2)")
    print(f"bbox WGS84 : {lon0:.5f} {lat0:.5f} {lon1:.5f} {lat1:.5f}")

    nodes: dict[tuple[int, int], int] = {}
    node_lines: list[str] = []
    way_lines: list[str] = []
    # splitter requires node ids in ASCENDING order, so ids must increment and
    # elements must be written in creation order. (Production emits deterministic ids
    # derived from TLM uuids -- those must then be sorted before splitting. FR-P4.)
    next_node = [1]
    next_way = [1]
    stats: dict[str, int] = {}
    t0 = time.time()

    def node_id(e: float, n: float) -> int:
        lon, lat = lv95_to_wgs84(e, n)
        # ~1e-7 deg grid: an order of magnitude finer than Garmin resolution, so
        # deduplication never merges distinguishable points (SPEC.md FR-P3)
        key = (int(round(lon * 1e7)), int(round(lat * 1e7)))
        nid = nodes.get(key)
        if nid is None:
            nid = next_node[0]
            next_node[0] += 1
            nodes[key] = nid
            node_lines.append(
                f'  <node id="{nid}" lat="{key[1]/1e7:.7f}" lon="{key[0]/1e7:.7f}" version="1"/>')
        return nid

    def emit_way(refs: list[int], tags: dict[str, str], closed: bool) -> None:
        if len(refs) < 2:
            return
        if closed and refs[0] != refs[-1]:
            refs = refs + [refs[0]]
        wid = next_way[0]
        next_way[0] += 1
        parts = [f'  <way id="{wid}" version="1">']
        parts += [f'    <nd ref="{r}"/>' for r in refs]
        parts += [f'    <tag k="{escape(k, {chr(34): "&quot;"})}" '
                  f'v="{escape(str(v), {chr(34): "&quot;"})}"/>' for k, v in tags.items()]
        parts.append("  </way>")
        way_lines.append("\n".join(parts))

    for table, attrs in LAYERS.items():
        have = table_columns(cur, table)
        attrs = [a for a in attrs if a in have]
        missing = [a for a in LAYERS[table] if a not in have]
        if missing:
            print(f"  ! {table}: columns not present, skipped: {missing}")

        sql = (f'SELECT t.geom, {", ".join(f"t.{a}" for a in attrs)} FROM "{table}" t '
               f'JOIN "rtree_{table}_geom" r ON t.id = r.id '
               f"WHERE r.maxx >= ? AND r.minx <= ? AND r.maxy >= ? AND r.miny <= ?")
        count = 0
        for row in cur.execute(sql, (mine, maxe, minn, maxn)):
            gname, rings = parse_gpkg_geom(row[0])
            if not rings:
                continue
            tags = {"tlm:layer": table}
            for a, v in zip(attrs, row[1:]):
                if v not in NULLISH:
                    tags[f"tlm:{a}"] = v
            if gname == "Point":
                # standalone tagged POI node -- deliberately not shared via node_id(),
                # so a POI never merges with a way vertex
                e, n = rings[0][0]
                lon, lat = lv95_to_wgs84(e, n)
                nid = next_node[0]
                next_node[0] += 1
                node_lines.append(
                    f'  <node id="{nid}" lat="{lat:.7f}" lon="{lon:.7f}" version="1">\n'
                    + "\n".join(f'    <tag k="{escape(k)}" v="{escape(str(v))}"/>'
                                 for k, v in tags.items())
                    + "\n  </node>")
            else:
                closed = gname in ("Polygon", "MultiPolygon")
                for ring in rings:
                    emit_way([node_id(e, n) for e, n in ring], tags, closed)
            count += 1
        stats[table] = count
        print(f"  {table:<38} {count:>9,} features")

    out = Path(args.out)
    with out.open("w", encoding="utf-8") as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?>\n')
        f.write('<osm version="0.6" generator="swisstopo2garmin-spike">\n')
        f.write(f'  <bounds minlat="{min(lat0,lat1):.7f}" minlon="{min(lon0,lon1):.7f}" '
                f'maxlat="{max(lat0,lat1):.7f}" maxlon="{max(lon0,lon1):.7f}"/>\n')
        f.write("\n".join(node_lines) + "\n")
        f.write("\n".join(way_lines) + "\n")
        f.write("</osm>\n")

    el = time.time() - t0
    print(f"\n{sum(stats.values()):,} features -> {len(nodes):,} nodes, "
          f"{next_way[0]-1:,} ways in {el:.1f}s")
    print(f"wrote {out} ({out.stat().st_size/1e6:.1f} MB)")


if __name__ == "__main__":
    main()
