"""S0.2 — verify a Garmin IMG container (spike, prototype of pipeline Stage 6).

Parses the IMG "fake FAT" to list subfiles, so we can assert that a build actually
produced map content (TRE/RGN/LBL) rather than an empty shell (SPEC.md §7.7).
"""
from __future__ import annotations

import struct
import sys
from pathlib import Path


def load(path: Path) -> bytes:
    data = bytearray(path.read_bytes())
    xor = data[0]
    if xor:
        data = bytearray(b ^ xor for b in data)
    return bytes(data)


def check(path: Path) -> int:
    buf = load(path)
    # Verified against a real mkgmap gmapsupp.img:
    #   0x10 "DSKIMG"   0x41 "GARMIN"   0x49 description (20 chars)
    #   0x61 E1, 0x62 E2  ->  block size = 1 << (E1 + E2)
    sig = buf[0x10:0x16].decode("ascii", "replace")
    ident = buf[0x41:0x47].decode("ascii", "replace")
    e1, e2 = buf[0x61], buf[0x62]
    block = 1 << (e1 + e2)
    desc = buf[0x49:0x5D].decode("ascii", "replace").strip()

    print(f"file        : {path.name}  ({path.stat().st_size:,} bytes)")
    print(f"signature   : {sig!r}  identifier {ident!r}")
    print(f"block size  : {block:,} bytes (E1={e1} E2={e2})")
    print(f"description : {desc!r}")
    if sig != "DSKIMG":
        print("  !! not a Garmin IMG container")
        return 1

    print(f"\n{'subfile':<14} {'type':<5} {'bytes':>12}")
    print("-" * 34)
    pos, total, kinds = 0x600, 0, {}
    while pos + 512 <= len(buf):
        entry = buf[pos : pos + 512]
        if entry[0] != 0x01:
            break
        name = entry[1:9].decode("ascii", "replace").strip()
        ftype = entry[9:12].decode("ascii", "replace").strip()
        size = struct.unpack_from("<I", entry, 12)[0]
        part = struct.unpack_from("<H", entry, 16)[0]
        if part == 0:
            print(f"{name:<14} {ftype:<5} {size:>12,}")
            kinds[ftype] = kinds.get(ftype, 0) + 1
            total += size
        pos += 512

    print("-" * 34)
    print(f"{'total':<20} {total:>12,}")
    print(f"\nsubfile types: {kinds}")

    problems = []
    for required in ("TRE", "RGN", "LBL"):
        if required not in kinds:
            problems.append(f"missing {required} subfile -- map has no {'geometry' if required=='RGN' else 'content'}")
    if "TYP" not in kinds:
        problems.append("no TYP subfile -- custom styling will not be applied")
    if total == 0:
        problems.append("all subfiles empty")

    print()
    if problems:
        for p in problems:
            print(f"  !! {p}")
        return 1
    print("  OK: container valid, map content present")
    return 0


if __name__ == "__main__":
    sys.exit(check(Path(sys.argv[1])))
