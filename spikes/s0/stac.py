"""swisstopo STAC client (spike).

Resolves dataset releases and assets from data.geo.admin.ch without hard-coding URLs
(SPEC.md FR-D1..D4). Standard library only.
"""
from __future__ import annotations

import binascii
import hashlib
import json
import os
import urllib.request
from dataclasses import dataclass
from pathlib import Path

STAC_ROOT = "https://data.geo.admin.ch/api/stac/v1"

# Multihash code -> (name, hashlib constructor). swisstopo currently emits sha2-256 (0x12).
MULTIHASH = {0x12: ("sha2-256", hashlib.sha256)}


def cache_dir() -> Path:
    p = Path(os.environ.get("S2G_CACHE", Path.home() / ".cache" / "swisstopo2garmin"))
    p.mkdir(parents=True, exist_ok=True)
    return p


def _get_json(url: str) -> dict:
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


@dataclass
class Asset:
    name: str
    href: str
    media_type: str
    checksum: str | None  # multihash, hex

    def expected_digest(self) -> tuple[str, "hashlib._Hash"] | None:
        """Decode the STAC `file:checksum` multihash into (algo_name, fresh hasher, hex)."""
        if not self.checksum:
            return None
        raw = binascii.unhexlify(self.checksum)
        code, length = raw[0], raw[1]
        if code not in MULTIHASH:
            raise ValueError(f"unsupported multihash code 0x{code:02x} for {self.name}")
        name, ctor = MULTIHASH[code]
        digest = raw[2 : 2 + length]
        return name, ctor(), digest.hex()


@dataclass
class Item:
    id: str
    datetime: str | None
    assets: dict[str, Asset]

    def asset_by_suffix(self, suffix: str) -> Asset:
        for a in self.assets.values():
            if a.name.endswith(suffix):
                return a
        raise KeyError(f"no asset ending in {suffix!r} on item {self.id}; "
                       f"have: {sorted(self.assets)}")


def items(collection: str, limit: int = 100) -> list[Item]:
    url = f"{STAC_ROOT}/collections/{collection}/items?limit={limit}"
    out: list[Item] = []
    while url:
        doc = _get_json(url)
        for f in doc.get("features", []):
            assets = {
                k: Asset(
                    name=k,
                    href=v["href"],
                    media_type=v.get("type", ""),
                    checksum=v.get("file:checksum"),
                )
                for k, v in f.get("assets", {}).items()
            }
            out.append(Item(id=f["id"], datetime=f.get("properties", {}).get("datetime"),
                            assets=assets))
        url = next((l["href"] for l in doc.get("links", []) if l.get("rel") == "next"), None)
    return out


def latest_item(collection: str) -> Item:
    """Newest release. Sorts by id, which is chronological for swisstopo releases
    (`swisstlm3d_2026-02`), falling back to the datetime property."""
    all_items = items(collection)
    if not all_items:
        raise RuntimeError(f"collection {collection} has no items")
    return sorted(all_items, key=lambda i: (i.datetime or "", i.id))[-1]


if __name__ == "__main__":
    import sys

    coll = sys.argv[1] if len(sys.argv) > 1 else "ch.swisstopo.swisstlm3d"
    it = latest_item(coll)
    print(f"collection : {coll}")
    print(f"latest item: {it.id}  ({it.datetime})")
    for a in it.assets.values():
        d = a.expected_digest()
        algo = f"{d[0]}:{d[2][:16]}..." if d else "no checksum"
        print(f"  {a.name}\n      {a.media_type}\n      {algo}")
