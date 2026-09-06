"""S0.1 — discover the real swissTLM3D schema (spike).

Generates docs/tlm3d-schema.md and docs/tlm3d-schema.json from the actual GeoPackage.
Nothing here is guessed: every name, type and value is read out of the file
(CLAUDE.md rule 2).

A GeoPackage is a plain SQLite database, so no GDAL is required -- this also validates
the approach the production Rust reader will take (SPEC.md §7.2).
"""
from __future__ import annotations

import json
import sqlite3
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
DOCS = REPO / "docs"

# Above this many distinct values a column is reported as high-cardinality rather than
# being enumerated -- keeps memory bounded and the document readable.
CARDINALITY_CAP = 300
# Columns never worth enumerating as classification domains.
SKIP_VALUE_SCAN = {"fid", "geom", "geometry", "shape"}


def human(n: float) -> str:
    for u in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024:
            return f"{n:.1f} {u}"
        n /= 1024
    return f"{n:.1f} PB"


def find_gpkg() -> Path:
    root = cache_dir() / "ch.swisstopo.swisstlm3d"
    cands = sorted(root.glob("*/*.gpkg"))
    if not cands:
        raise SystemExit(f"no GeoPackage found under {root} -- run fetch_tlm3d.py first")
    return cands[-1]


def main() -> None:
    gpkg = find_gpkg()
    print(f"reading {gpkg}  ({human(gpkg.stat().st_size)})")
    con = sqlite3.connect(f"file:{gpkg}?mode=ro", uri=True)
    con.text_factory = lambda b: b.decode("utf-8", "replace")
    cur = con.cursor()

    app_id, user_ver = cur.execute("PRAGMA application_id").fetchone()[0], \
                       cur.execute("PRAGMA user_version").fetchone()[0]

    # --- inventory -------------------------------------------------------------
    contents = {
        r[0]: {"data_type": r[1], "identifier": r[2], "description": r[3],
               "srs_id": r[4], "bbox": [r[5], r[6], r[7], r[8]]}
        for r in cur.execute(
            "SELECT table_name, data_type, identifier, description, srs_id,"
            " min_x, min_y, max_x, max_y FROM gpkg_contents")
    }
    geom_cols = {
        r[0]: {"column": r[1], "geometry_type": r[2], "srs_id": r[3], "z": r[4], "m": r[5]}
        for r in cur.execute(
            "SELECT table_name, column_name, geometry_type_name, srs_id, z, m"
            " FROM gpkg_geometry_columns")
    }
    all_tables = [r[0] for r in cur.execute(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
    # Spatial indexes are declared in gpkg_extensions -- this is the authoritative
    # source. Do NOT try to infer table names by splitting rtree_* object names:
    # the layer names themselves contain underscores.
    try:
        rtree_tables = {r[0] for r in cur.execute(
            "SELECT table_name FROM gpkg_extensions "
            "WHERE extension_name = 'gpkg_rtree_index'")}
    except sqlite3.Error:
        rtree_tables = set()

    spatial = sorted(t for t in contents if t in geom_cols)
    attribute_tables = sorted(t for t in contents if t not in geom_cols)
    internal = sorted(t for t in all_tables
                      if t not in contents and (t.startswith("gpkg_") or t.startswith("rtree_")
                                                or t.startswith("sqlite_")))
    other = sorted(t for t in all_tables
                   if t not in contents and t not in internal)

    print(f"  spatial layers   : {len(spatial)}")
    print(f"  attribute tables : {len(attribute_tables)}")
    print(f"  other tables     : {len(other)}")

    # --- per-table detail ------------------------------------------------------
    report: dict[str, dict] = {}
    targets = spatial + attribute_tables + other
    t0 = time.time()

    for n, table in enumerate(targets, 1):
        cols = [
            {"name": r[1], "type": r[2], "notnull": bool(r[3]), "pk": bool(r[5])}
            for r in cur.execute(f'PRAGMA table_info("{table}")')
        ]
        count = cur.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0]
        gc = geom_cols.get(table, {})
        gcol = gc.get("column")

        scan = [c["name"] for c in cols
                if c["name"].lower() not in SKIP_VALUE_SCAN
                and c["name"] != gcol
                and not c["pk"]]

        print(f"  [{n}/{len(targets)}] {table}: {count:,} rows, "
              f"{len(scan)} columns to profile", flush=True)

        counters = {c: Counter() for c in scan}
        overflow: set[str] = set()
        nulls = Counter()

        if scan and count:
            quoted = ", ".join(f'"{c}"' for c in scan)
            for row in cur.execute(f'SELECT {quoted} FROM "{table}"'):
                for name, val in zip(scan, row):
                    if val is None:
                        nulls[name] += 1
                        continue
                    if name in overflow:
                        continue
                    ctr = counters[name]
                    if len(ctr) >= CARDINALITY_CAP and val not in ctr:
                        overflow.add(name)
                        continue
                    ctr[val] += 1

        for c in cols:
            name = c["name"]
            c["nulls"] = nulls.get(name, 0)
            if name in overflow:
                c["cardinality"] = f">{CARDINALITY_CAP}"
                c["values"] = None
            elif name in counters:
                c["cardinality"] = len(counters[name])
                c["values"] = counters[name].most_common()
            else:
                c["cardinality"] = None
                c["values"] = None

        report[table] = {
            "kind": "spatial" if table in geom_cols else (
                "attribute" if table in contents else "other"),
            "count": count,
            "contents": contents.get(table),
            "geometry": gc or None,
            "has_rtree": table in rtree_tables,
            "columns": cols,
        }

    elapsed = time.time() - t0
    print(f"\nprofiled {len(targets)} tables in {elapsed/60:.1f} min")

    # --- write outputs ---------------------------------------------------------
    DOCS.mkdir(exist_ok=True)
    prov_path = gpkg.parent / "provenance.json"
    prov = json.loads(prov_path.read_text()) if prov_path.exists() else {}

    (DOCS / "tlm3d-schema.json").write_text(json.dumps({
        "source": {"file": gpkg.name, "bytes": gpkg.stat().st_size,
                   "application_id": app_id, "user_version": user_ver, **prov},
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "cardinality_cap": CARDINALITY_CAP,
        "tables": report,
    }, indent=2, ensure_ascii=False))

    md: list[str] = []
    w = md.append
    w("# swissTLM3D — discovered schema\n")
    w("> **Generated, do not edit by hand.** Produced by `spikes/s0/schema_dump.py` from the\n"
      "> actual GeoPackage. Every name and value below is read out of the file — nothing is\n"
      "> inferred (CLAUDE.md rule 2). Regenerate after every swissTLM3D release.\n")
    w(f"- **Release:** `{prov.get('item', '?')}` ({prov.get('datetime', '?')})")
    w(f"- **File:** `{gpkg.name}` — {human(gpkg.stat().st_size)} ({gpkg.stat().st_size:,} bytes)")
    w(f"- **Checksum (STAC multihash):** `{prov.get('checksum_multihash', '?')}`")
    w(f"- **Generated:** {time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime())}")
    w(f"- **Spatial layers:** {len(spatial)} · **Attribute tables:** {len(attribute_tables)}"
      f" · **Other:** {len(other)}")
    w(f"- Columns with more than {CARDINALITY_CAP} distinct values are reported as"
      f" high-cardinality rather than enumerated.\n")

    total_features = sum(report[t]["count"] for t in spatial)
    w(f"## Spatial layers\n\nTotal features across all spatial layers: **{total_features:,}**\n")
    w("| Layer | Geometry | SRID | Features | R-tree | Identifier |")
    w("|---|---|---:|---:|:---:|---|")
    for t in sorted(spatial, key=lambda x: -report[x]["count"]):
        r = report[t]
        g = r["geometry"] or {}
        ident = (r["contents"] or {}).get("identifier") or ""
        w(f"| `{t}` | {g.get('geometry_type','?')} | {g.get('srs_id','?')} | "
          f"{r['count']:,} | {'yes' if r['has_rtree'] else 'no'} | {ident} |")
    w("")

    if attribute_tables or other:
        w("## Non-spatial tables\n")
        w("| Table | Kind | Rows |")
        w("|---|---|---:|")
        for t in attribute_tables + other:
            w(f"| `{t}` | {report[t]['kind']} | {report[t]['count']:,} |")
        w("")

    w("## Layer detail\n")
    for t in sorted(targets, key=lambda x: (report[x]["kind"] != "spatial", -report[x]["count"])):
        r = report[t]
        g = r["geometry"] or {}
        w(f"### `{t}`\n")
        c = r["contents"] or {}
        if c.get("identifier"):
            w(f"*{c['identifier']}*\n")
        if c.get("description"):
            w(f"{c['description']}\n")
        bits = [f"**{r['count']:,}** rows"]
        if g:
            bits.append(f"geometry `{g['column']}` ({g['geometry_type']}, SRID {g['srs_id']}"
                        f"{', has Z' if g.get('z') else ''})")
        bits.append("R-tree index" if r["has_rtree"] else "no R-tree index")
        w(" · ".join(bits) + "\n")

        w("| Column | Type | Null | Distinct | Values |")
        w("|---|---|---:|---:|---|")
        for col in r["columns"]:
            vals = ""
            if col["values"] is not None and col["cardinality"]:
                shown = col["values"][:40]
                vals = " · ".join(
                    f"`{v}` ({n:,})" for v, n in shown
                    if not isinstance(v, bytes)
                )
                if len(col["values"]) > len(shown):
                    vals += f" · … +{len(col['values']) - len(shown)} more"
            elif col["cardinality"]:
                vals = "_high cardinality — not enumerated_"
            card = col["cardinality"] if col["cardinality"] is not None else ""
            w(f"| `{col['name']}` | {col['type']} | {col['nulls']:,} | {card} | {vals} |")
        w("")

    (DOCS / "tlm3d-schema.md").write_text("\n".join(md))
    print(f"wrote {DOCS/'tlm3d-schema.md'}")
    print(f"wrote {DOCS/'tlm3d-schema.json'}")


if __name__ == "__main__":
    main()
