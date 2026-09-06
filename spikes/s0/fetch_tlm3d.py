"""S0.1 — acquire the national swissTLM3D GeoPackage (spike).

The release ships as a ZIP64 archive holding one DEFLATE-compressed GeoPackage:
  4.80 GB compressed -> 10.78 GB uncompressed.

Downloading then unzipping would peak at 15.6 GB of disk. Instead we stream the archive
once and inflate on the fly, so peak usage is only the inflated output. The full-file
sha256 is computed over the same single pass and checked against the STAC `file:checksum`
multihash (SPEC.md FR-D1).

Resilience: a dropped connection reconnects with an HTTP Range request and keeps feeding
the *same* live decompressor, so network blips do not restart the transfer (FR-D2).
A process restart does have to start over -- raw DEFLATE state cannot be serialised.
See the findings note in docs/ for what production should do instead.
"""
from __future__ import annotations

import json
import shutil
import struct
import sys
import time
import urllib.error
import urllib.request
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from stac import Asset, cache_dir, latest_item  # noqa: E402

COLLECTION = "ch.swisstopo.swisstlm3d"
CHUNK = 1 << 20          # 1 MiB
MAX_RETRIES = 12
STATUS_EVERY = 2.0       # seconds


def _open_range(url: str, start: int):
    req = urllib.request.Request(url, headers={"Range": f"bytes={start}-"})
    return urllib.request.urlopen(req, timeout=120)


def _total_size(url: str) -> int:
    req = urllib.request.Request(url, method="HEAD")
    with urllib.request.urlopen(req, timeout=60) as r:
        return int(r.headers["Content-Length"])


def _zip_layout(url: str, total: int) -> tuple[int, int, int, str]:
    """Return (data_start, compressed_size, uncompressed_size, member_name).

    Reads the ZIP64 end-of-central-directory and the single central directory record,
    then the local file header, using only range requests.
    """
    tail_len = min(total, 65536 + 22)
    with _open_range(url, total - tail_len) as r:
        tail = r.read()

    i = tail.rfind(b"PK\x05\x06")
    if i < 0:
        raise RuntimeError("no end-of-central-directory record found")
    cd_size, cd_off = struct.unpack("<II", tail[i + 12 : i + 20])

    if cd_off == 0xFFFFFFFF or cd_size == 0xFFFFFFFF:
        j = tail.rfind(b"PK\x06\x07")
        if j < 0:
            raise RuntimeError("zip64 locator missing")
        z64_off = struct.unpack("<Q", tail[j + 8 : j + 16])[0]
        with _open_range(url, z64_off) as r:
            blk = r.read(56)
        if blk[:4] != b"PK\x06\x06":
            raise RuntimeError("bad zip64 end-of-central-directory signature")
        cd_size, cd_off = struct.unpack("<QQ", blk[40:56])

    with _open_range(url, cd_off) as r:
        cd = r.read(cd_size)
    if cd[:4] != b"PK\x01\x02":
        raise RuntimeError("bad central directory signature")

    method = struct.unpack("<H", cd[10:12])[0]
    if method != 8:
        raise RuntimeError(f"expected DEFLATE (8), got compression method {method}")
    csize, usize = struct.unpack("<II", cd[20:28])
    nlen, elen, clen = struct.unpack("<HHH", cd[28:34])
    name = cd[46 : 46 + nlen].decode("utf-8", "replace")
    lho = struct.unpack("<I", cd[42:46])[0]
    extra = cd[46 + nlen : 46 + nlen + elen]

    if 0xFFFFFFFF in (usize, csize, lho):
        q = 0
        while q + 4 <= len(extra):
            hid, hsz = struct.unpack("<HH", extra[q : q + 4])
            if hid == 0x0001:
                blob, off = extra[q + 4 : q + 4 + hsz], 0
                if usize == 0xFFFFFFFF:
                    usize = struct.unpack("<Q", blob[off : off + 8])[0]; off += 8
                if csize == 0xFFFFFFFF:
                    csize = struct.unpack("<Q", blob[off : off + 8])[0]; off += 8
                if lho == 0xFFFFFFFF:
                    lho = struct.unpack("<Q", blob[off : off + 8])[0]; off += 8
            q += 4 + hsz

    # local file header: name/extra lengths differ from the central directory
    with _open_range(url, lho) as r:
        lfh = r.read(30)
    if lfh[:4] != b"PK\x03\x04":
        raise RuntimeError("bad local file header signature")
    l_nlen, l_elen = struct.unpack("<HH", lfh[26:30])
    data_start = lho + 30 + l_nlen + l_elen
    return data_start, csize, usize, name


def _human(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024:
            return f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} PB"


def fetch(asset: Asset, dest: Path, status_path: Path) -> Path:
    url = asset.href
    total = _total_size(url)
    data_start, csize, usize, member = _zip_layout(url, total)

    print(f"archive      : {asset.name}")
    print(f"member       : {member}")
    print(f"compressed   : {total:,} B ({_human(total)})")
    print(f"uncompressed : {usize:,} B ({_human(usize)})")
    print(f"destination  : {dest}")

    free = shutil.disk_usage(dest.parent).free
    need = usize + (256 << 20)
    print(f"free disk    : {_human(free)}  (need ~{_human(need)})")
    if free < need:
        raise SystemExit(f"ERROR: insufficient disk space: need {_human(need)}, have {_human(free)}")

    algo, hasher, expected_hex = asset.expected_digest()
    decomp = zlib.decompressobj(-15)  # raw DEFLATE
    data_end = data_start + csize

    part = dest.with_suffix(dest.suffix + ".part")
    pos = 0            # absolute byte offset within the archive
    written = 0        # inflated bytes written
    retries = 0
    started = time.time()
    last_status = 0.0

    def emit(state: str, note: str = "") -> None:
        el = time.time() - started
        rate = pos / el if el > 0 else 0
        eta = (total - pos) / rate if rate > 0 else 0
        status_path.write_text(json.dumps({
            "state": state, "note": note,
            "compressed_read": pos, "compressed_total": total,
            "inflated_written": written, "inflated_total": usize,
            "pct": round(pos / total * 100, 2),
            "rate_bytes_s": round(rate), "elapsed_s": round(el), "eta_s": round(eta),
            "retries": retries,
        }, indent=2))

    with part.open("wb") as out:
        while pos < total:
            try:
                with _open_range(url, pos) as resp:
                    while pos < total:
                        buf = resp.read(CHUNK)
                        if not buf:
                            break
                        hasher.update(buf)

                        # route only the member's compressed payload into the inflater
                        lo, hi = pos, pos + len(buf)
                        s, e = max(lo, data_start), min(hi, data_end)
                        if s < e:
                            chunk = decomp.decompress(buf[s - lo : e - lo])
                            if chunk:
                                out.write(chunk)
                                written += len(chunk)
                        pos = hi

                        now = time.time()
                        if now - last_status >= STATUS_EVERY:
                            last_status = now
                            emit("downloading")
                            el = now - started
                            rate = pos / el if el else 0
                            eta = (total - pos) / rate if rate else 0
                            print(f"  {pos/total*100:5.1f}%  {_human(pos)}/{_human(total)}  "
                                  f"-> {_human(written)}  {_human(rate)}/s  "
                                  f"eta {eta/60:.0f}m  retries={retries}", flush=True)
            except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as exc:
                retries += 1
                if retries > MAX_RETRIES:
                    emit("failed", f"{type(exc).__name__}: {exc}")
                    raise
                backoff = min(60, 2 ** min(retries, 6))
                print(f"  ! {type(exc).__name__}: {exc} -- reconnecting at "
                      f"{pos:,} in {backoff}s (retry {retries}/{MAX_RETRIES})", flush=True)
                emit("retrying", str(exc))
                time.sleep(backoff)

        tail = decomp.flush()
        if tail:
            out.write(tail)
            written += len(tail)

    actual = hasher.hexdigest()
    if actual != expected_hex:
        part.unlink(missing_ok=True)
        emit("failed", "checksum mismatch")
        raise SystemExit(f"ERROR: {algo} mismatch\n  expected {expected_hex}\n  actual   {actual}")
    if written != usize:
        part.unlink(missing_ok=True)
        emit("failed", "size mismatch")
        raise SystemExit(f"ERROR: inflated {written:,} B, expected {usize:,} B")

    part.replace(dest)   # atomic (FR-D3)
    emit("done")
    el = time.time() - started
    print(f"\nOK  {algo} verified: {actual}")
    print(f"    {_human(written)} written in {el/60:.1f} min ({_human(total/el)}/s)")
    return dest


def main() -> None:
    item = latest_item(COLLECTION)
    asset = item.asset_by_suffix(".gpkg.zip")
    out_dir = cache_dir() / COLLECTION / item.id
    out_dir.mkdir(parents=True, exist_ok=True)

    _, _, _, member = _zip_layout(asset.href, _total_size(asset.href))
    dest = out_dir / member
    status = out_dir / "status.json"

    print(f"release      : {item.id}  ({item.datetime})\n")
    if dest.exists():
        print(f"already present: {dest} ({_human(dest.stat().st_size)})")
        return

    fetch(asset, dest, status)
    # record provenance for reproducibility (FR-D4)
    (out_dir / "provenance.json").write_text(json.dumps({
        "collection": COLLECTION, "item": item.id, "datetime": item.datetime,
        "asset": asset.name, "href": asset.href, "checksum_multihash": asset.checksum,
        "member": member, "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }, indent=2))


if __name__ == "__main__":
    main()
