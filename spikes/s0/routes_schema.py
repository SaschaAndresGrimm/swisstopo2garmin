"""Generate docs/routes-schema.md from the real ASTRA route shapefiles."""
from __future__ import annotations

import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir  # noqa: E402

CAP = 25
SHAPE_TYPES = {0: "Null", 1: "Point", 3: "PolyLine", 5: "Polygon", 8: "MultiPoint",
               11: "PointZ", 13: "PolyLineZ", 15: "PolygonZ", 18: "MultiPointZ"}


def dbf_fields(path: Path):
    b = path.read_bytes()
    count = struct.unpack_from("<I", b, 4)[0]
    header_len = struct.unpack_from("<H", b, 8)[0]
    record_len = struct.unpack_from("<H", b, 10)[0]
    fields, off, p = [], 1, 32
    while p + 32 <= header_len and b[p] != 0x0D:
        name = bytes(b[p:p + 11]).split(b"\0")[0].decode("latin-1")
        fields.append((name, chr(b[p + 11]), b[p + 16], off))
        off += b[p + 16]
        p += 32
    return b, count, header_len, record_len, fields


def main() -> None:
    root = cache_dir() / "routes"
    shapes = sorted(root.glob("*/*/*.shp"))
    if not shapes:
        raise SystemExit(f"no shapefiles under {root}; run spikes/s0/fetch_routes.py")

    L: list[str] = []
    w = L.append
    w("# ASTRA route networks — discovered schema\n")
    w("> **Generated, do not edit by hand.** Produced by `spikes/s0/routes_schema.py`\n"
      "> from the real shapefiles.\n")
    w(f"- **Generated:** {time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime())}")
    w("- These datasets publish **shapefile and File Geodatabase only**; there is no")
    w("  GeoPackage, which is why `crates/s2g-core/src/shapefile.rs` exists.")
    w(f"- Columns with more than {CAP} distinct values are not enumerated.\n")

    for shp in shapes:
        dataset = shp.parent.parent.name
        raw = shp.read_bytes()
        stype = struct.unpack_from("<i", raw, 32)[0]
        pos, records = 100, 0
        while pos + 8 <= len(raw):
            cl = struct.unpack_from(">i", raw, pos + 4)[0] * 2
            if cl <= 0:
                break
            pos += 8 + cl
            records += 1

        w(f"## `{dataset}` / `{shp.name}`\n")
        w(f"**{records:,}** records · shape type {stype} "
          f"({SHAPE_TYPES.get(stype, 'unknown')})\n")

        dbf = shp.with_suffix(".dbf")
        if not dbf.exists():
            w("_no .dbf_\n")
            continue
        b, count, hl, rl, fields = dbf_fields(dbf)
        w("| Field | Type | Len | Distinct | Values |")
        w("|---|---|---:|---:|---|")
        for name, kind, length, off in fields:
            seen: dict[str, int] = {}
            overflow = False
            for i in range(count):
                s = hl + i * rl + off
                v = bytes(b[s:s + length]).decode("latin-1").strip()
                if len(seen) > CAP:
                    overflow = True
                    break
                seen[v] = seen.get(v, 0) + 1
            if overflow:
                w(f"| `{name}` | {kind} | {length} | >{CAP} | _not enumerated_ |")
            else:
                items = sorted(seen.items(), key=lambda kv: -kv[1])[:12]
                vals = " · ".join(
                    (f"`{k}` ({v:,})" if k else f"_empty_ ({v:,})") for k, v in items
                )
                w(f"| `{name}` | {kind} | {length} | {len(seen)} | {vals} |")
        w("")

    out = Path(__file__).resolve().parents[2] / "docs" / "routes-schema.md"
    out.write_text("\n".join(L))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
