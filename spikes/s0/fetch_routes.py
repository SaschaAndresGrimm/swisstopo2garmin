"""Download the ASTRA route networks for the cycling preset.

These publish shapefile and File Geodatabase only -- no GeoPackage -- which is why
crates/s2g-core/src/shapefile.rs exists. About 170 MB in total.
"""
from __future__ import annotations

import sys
import urllib.request
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir, latest_item  # noqa: E402

COLLECTIONS = ["ch.astra.veloland", "ch.astra.mountainbikeland", "ch.astra.wanderland"]


def main() -> None:
    out = cache_dir() / "routes"
    out.mkdir(parents=True, exist_ok=True)

    for collection in COLLECTIONS:
        item = latest_item(collection)
        asset = item.asset_by_suffix(".shp.zip")
        archive = out / asset.name
        target = out / collection.split(".")[-1]

        if not archive.exists():
            print(f"  {asset.name}: downloading ...", flush=True)
            with urllib.request.urlopen(asset.href, timeout=600) as r:
                archive.write_bytes(r.read())
            print(f"     {archive.stat().st_size/1e6:.0f} MB")
        else:
            print(f"  {asset.name}: present")

        if not target.exists():
            with zipfile.ZipFile(archive) as z:
                z.extractall(target)
            print(f"     unpacked into {target.name}/")

    shapes = sorted(out.glob("*/*/*.shp"))
    print(f"\n{len(shapes)} shapefiles in {out}:")
    for s in shapes:
        print(f"  {s.parent.parent.name}/{s.name}")


if __name__ == "__main__":
    main()
