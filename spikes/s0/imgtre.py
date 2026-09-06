"""Decode Garmin IMG subfile inventory + TRE headers (bounds, zoom levels).

Diagnostic for 'map recognised but does not render': tells us where the map thinks it
is and which resolutions actually carry data.
"""
from __future__ import annotations
import struct, sys
from pathlib import Path

def load(p: Path) -> bytes:
    d = bytearray(p.read_bytes())
    x = d[0]
    if x: d = bytearray(b ^ x for b in d)
    return bytes(d)

def s24(b: bytes) -> int:
    v = b[0] | (b[1] << 8) | (b[2] << 16)
    return v - (1 << 24) if v & 0x800000 else v

def deg(v: int) -> float:
    return v * 360.0 / (1 << 24)

def main(path: Path) -> None:
    buf = load(path)
    e1, e2 = buf[0x61], buf[0x62]
    block = 1 << (e1 + e2)
    print(f"{path.name}: {path.stat().st_size:,} B, block {block}")

    # ---- full FAT scan (do not stop at the first unused entry) ----
    files: dict[str, dict] = {}
    pos, scanned = 0x600, 0
    while pos + 512 <= len(buf) and scanned < 4000:
        e = buf[pos:pos+512]; pos += 512; scanned += 1
        if e[0] != 0x01:
            continue
        raw_name, raw_type = e[1:9], e[9:12]
        # The FAT is followed directly by data blocks; without this guard the scan
        # runs on and invents subfiles out of map geometry.
        if not all(32 <= c < 127 for c in raw_name + raw_type):
            break
        name = raw_name.decode("ascii").strip()
        typ  = raw_type.decode("ascii").strip()
        size = struct.unpack_from("<I", e, 12)[0]
        part = struct.unpack_from("<H", e, 16)[0]
        blocks = [struct.unpack_from("<H", e, 0x20+2*i)[0] for i in range(240)]
        key = f"{name}.{typ}"
        f = files.setdefault(key, {"name":name,"type":typ,"size":size,"blocks":[]})
        f["blocks"] += [b for b in blocks if b != 0xFFFF]
        if part == 0: f["size"] = size

    maps = sorted({f["name"] for f in files.values()})
    print(f"\nFAT entries: {len(files)} subfiles across maps: {maps}")
    for k, f in sorted(files.items()):
        print(f"  {k:<16} {f['size']:>10,} B")

    def extract(key: str) -> bytes:
        f = files[key]
        out = bytearray()
        for b in f["blocks"]:
            out += buf[b*block:(b+1)*block]
        return bytes(out[:f["size"]])

    for m in maps:
        key = f"{m}.TRE"
        if key not in files:
            continue
        tre = extract(key)
        hlen = struct.unpack_from("<H", tre, 0)[0]
        magic = tre[2:12].decode("ascii","replace")
        north, east = s24(tre[0x15:0x18]), s24(tre[0x18:0x1b])
        south, west = s24(tre[0x1b:0x1e]), s24(tre[0x1e:0x21])
        lv_off, lv_size = struct.unpack_from("<II", tre, 0x21)
        print(f"\n=== TRE {m}  (header {hlen} B, {magic!r})")
        print(f"  bounds  N {deg(north):9.5f}  S {deg(south):9.5f}"
              f"   W {deg(west):9.5f}  E {deg(east):9.5f}")
        print(f"  levels section: offset {lv_off}, size {lv_size} "
              f"({lv_size//4} entries)")
        print(f"  {'level':>5} {'resolution':>11} {'subdivs':>9} {'inherited':>10}")
        for i in range(lv_size // 4):
            z, sub = struct.unpack_from("<HH", tre, lv_off + i*4)
            zoom = z & 0x0F
            inherited = bool(z & 0x80)
            res = (z >> 8) & 0xFF
            print(f"  {zoom:>5} {res:>11} {sub:>9} {str(inherited):>10}")

if __name__ == "__main__":
    main(Path(sys.argv[1]))
