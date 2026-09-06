"""Generate docs/winter-schema.md from the real winter-sport GeoPackages.

Same rule as swissTLM3D: no layer name, attribute or value in the style rules may be
guessed (CLAUDE.md rule 2).
"""
from __future__ import annotations

import sqlite3
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir  # noqa: E402

CAP = 40
SKIP = {"fid", "geom", "uuid"}
SOURCES = {
    "skitouren_2056.gpkg.zip": "ch.swisstopo-karto.skitouren",
    "ski_routes_2056.gpkg": "ch.swisstopo-karto.skitouren",
    "ski_network_2056.gpkg": "ch.swisstopo-karto.skitouren",
    "schneeschuhwanderwege_2056.gpkg": "ch.astra.schneeschuhwanderwege",
    "winterwanderwege_2056.gpkg": "ch.astra.winterwanderwege",
}


def main() -> None:
    root = cache_dir() / "winter"
    files = sorted(root.glob("*.gpkg"))
    if not files:
        raise SystemExit(f"no GeoPackages under {root}")

    L: list[str] = []
    w = L.append
    w("# Winter sport routes — discovered schema\n")
    w("> **Generated, do not edit by hand.** Produced by `spikes/s0/winter_schema.py`\n"
      "> from the real GeoPackages. Every name and value is read out of the files.\n")
    w(f"- **Generated:** {time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime())}")
    w(f"- Columns with more than {CAP} distinct values are not enumerated.\n")

    for f in files:
        con = sqlite3.connect(f"file:{f}?mode=ro", uri=True)
        con.text_factory = lambda b: b.decode("utf-8", "replace")
        cur = con.cursor()
        w(f"## `{f.name}`\n")
        src = SOURCES.get(f.name)
        if src:
            w(f"Source collection: `{src}`\n")

        geom = {
            r[0]: (r[1], r[2])
            for r in cur.execute(
                "SELECT table_name, column_name, geometry_type_name "
                "FROM gpkg_geometry_columns"
            )
        }
        try:
            rtree = {
                r[0]
                for r in cur.execute(
                    "SELECT table_name FROM gpkg_extensions "
                    "WHERE extension_name = 'gpkg_rtree_index'"
                )
            }
        except sqlite3.Error:
            rtree = set()

        for (table,) in cur.execute("SELECT table_name FROM gpkg_contents ORDER BY table_name"):
            n = cur.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0]
            g = geom.get(table)
            pk = [r[1] for r in cur.execute(f'PRAGMA table_info("{table}")') if r[5]]
            w(f"### `{table}`\n")
            w(f"**{n:,}** rows · geometry `{g[1] if g else 'none'}` · "
              f"primary key `{pk[0] if pk else '?'}` · "
              f"{'R-tree index' if table in rtree else 'no R-tree index'}\n")

            w("| Column | Type | Distinct | Values |")
            w("|---|---|---:|---|")
            for r in cur.execute(f'PRAGMA table_info("{table}")'):
                col, typ = r[1], r[2]
                if col.lower() in SKIP:
                    w(f"| `{col}` | {typ} | | |")
                    continue
                rows = list(
                    cur.execute(
                        f'SELECT "{col}", COUNT(*) FROM "{table}" '
                        f"GROUP BY 1 ORDER BY 2 DESC LIMIT {CAP + 1}"
                    )
                )
                if len(rows) > CAP:
                    w(f"| `{col}` | {typ} | >{CAP} | _not enumerated_ |")
                else:
                    vals = " · ".join(f"`{v}` ({c:,})" for v, c in rows)
                    w(f"| `{col}` | {typ} | {len(rows)} | {vals} |")
            w("")

    out = Path(__file__).resolve().parents[2] / "docs" / "winter-schema.md"
    out.write_text("\n".join(L))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
