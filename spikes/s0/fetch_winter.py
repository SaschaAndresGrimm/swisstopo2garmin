"""Download the winter-sport GeoPackages for the skimo preset.

All three are GeoPackage, so no shapefile parser is needed. Together they are about
38 MB, small enough to fetch whole rather than clipping server-side.
"""
from __future__ import annotations

import sys
import urllib.request
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import cache_dir, latest_item  # noqa: E402

COLLECTIONS = [
    ("ch.swisstopo-karto.skitouren", ".gpkg.zip"),
    ("ch.astra.schneeschuhwanderwege", ".gpkg"),
    ("ch.astra.winterwanderwege", ".gpkg"),
]


def main() -> None:
    out = cache_dir() / "winter"
    out.mkdir(parents=True, exist_ok=True)

    for collection, suffix in COLLECTIONS:
        item = latest_item(collection)
        asset = item.asset_by_suffix(suffix)
        dest = out / asset.name
        if dest.exists():
            print(f"  {asset.name}: present")
        else:
            print(f"  {asset.name}: downloading ...", flush=True)
            with urllib.request.urlopen(asset.href, timeout=300) as r:
                dest.write_bytes(r.read())
            print(f"     {dest.stat().st_size/1e6:.1f} MB")

        if dest.suffix == ".zip":
            with zipfile.ZipFile(dest) as z:
                for name in z.namelist():
                    if name.endswith(".gpkg") and not (out / Path(name).name).exists():
                        (out / Path(name).name).write_bytes(z.read(name))
                        print(f"     extracted {Path(name).name}")

    print(f"\nwinter GeoPackages in {out}:")
    for f in sorted(out.glob("*.gpkg")):
        print(f"  {f.name:<40} {f.stat().st_size/1e6:6.1f} MB")


if __name__ == "__main__":
    main()
