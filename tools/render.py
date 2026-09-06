"""Render a compiled Garmin IMG to PNG using the project's own TYP palette.

Pipeline:  ImgDump (mkgmap's reader)  ->  TSV  ->  this  ->  PNG

This is the desktop preview and the basis of the visual-regression harness
(SPEC.md FR-CART7). It is faithful in the ways that matter for cartography review:
geometry, object types and zoom level come from the compiled map via mkgmap's own
reader, and colours come from the same typ/swisstopo.txt the device uses. It is not
a Garmin renderer -- line casing, label placement and pattern fills are approximated.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PolyCollection

REPO = Path(__file__).resolve().parents[1]


def parse_typ(path: Path):
    """-> (polygon colours, line colours+widths, draw order)."""
    text = "\n".join(l for l in path.read_text().splitlines()
                     if not l.lstrip().startswith(";"))
    order: dict[str, int] = {}
    sec = re.search(r"^\[_drawOrder\]\s*$(.*?)^\[end\]", text, re.S | re.M)
    if sec:
        for t, lv in re.findall(r"^Type=(0x[0-9a-fA-F]+)\s*,\s*(\d+)", sec.group(1), re.M):
            order[t.lower()] = int(lv)

    polys, lines = {}, {}
    for kind, body in re.findall(r"^\[_(polygon|line)\]\s*$(.*?)^\[end\]", text, re.S | re.M):
        m = re.search(r"^Type=(0x[0-9a-fA-F]+)", body, re.M)
        if not m:
            continue
        t = m.group(1).lower()
        cols = re.findall(r'^"\S+\s+c\s+(#[0-9A-Fa-f]{6})"', body, re.M)
        if not cols:
            continue
        if kind == "polygon":
            # A pattern fill lists background then ink plus 32 bitmap rows. This
            # renderer cannot draw the bitmap, so approximate the tint by blending
            # the two colours at the pattern's ink coverage -- otherwise rock and
            # wetland preview as flat white and look wrong.
            rows = re.findall(r'^"([.#]{32})"', body, re.M)
            if rows and len(cols) >= 2:
                ink = sum(r.count("#") for r in rows) / (len(rows) * 32)
                bg = tuple(int(cols[0][i:i+2], 16) for i in (1, 3, 5))
                fg = tuple(int(cols[1][i:i+2], 16) for i in (1, 3, 5))
                mix = tuple(round(b + (f - b) * ink) for b, f in zip(bg, fg))
                polys[t] = "#%02X%02X%02X" % mix
            else:
                polys[t] = cols[0]
        else:
            lw = re.search(r"^LineWidth=(\d+)", body, re.M)
            bw = re.search(r"^BorderWidth=(\d+)", body, re.M)
            lines[t] = {
                "color": cols[0],
                "casing": cols[1] if len(cols) > 1 and bw else None,
                "width": int(lw.group(1)) if lw else 2,
                "border": int(bw.group(1)) if bw else 0,
            }
    return polys, lines, order


def load_dump(path: Path, level: int):
    shapes, lns, pts = [], [], []
    bounds = None
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            f = line.rstrip("\n").split("\t")
            if f[0] == "BOUNDS":
                bounds = tuple(float(x) for x in f[1:5])
            elif f[0] in ("S", "L") and f[1] == str(level):
                coords = []
                for c in f[4:]:
                    if "," in c:
                        la, lo = c.split(",")
                        coords.append((float(lo), float(la)))
                if len(coords) >= 2:
                    (shapes if f[0] == "S" else lns).append((f[2].lower(), f[3], coords))
            elif f[0] == "P" and f[1] == str(level) and len(f) >= 5 and "," in f[4]:
                la, lo = f[4].split(",")
                pts.append((f[2].lower(), f[3], float(lo), float(la)))
    return bounds, shapes, lns, pts


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("dump", nargs="+")
    ap.add_argument("-o", "--out", default="preview.png")
    ap.add_argument("--level", type=int, default=0)
    ap.add_argument("--typ", default=str(REPO / "typ" / "swisstopo.txt"))
    ap.add_argument("--bbox", nargs=4, type=float, metavar=("W", "S", "E", "N"))
    ap.add_argument("--width", type=float, default=12.0)
    ap.add_argument("--dpi", type=int, default=140)
    ap.add_argument("--labels", action="store_true")
    args = ap.parse_args()

    polys, lines, order = parse_typ(Path(args.typ))
    shapes, lns, pts = [], [], []
    boxes = []
    for d in args.dump:
        b, sh, ln, pt = load_dump(Path(d), args.level)
        if b:
            boxes.append(b)
        shapes += sh; lns += ln; pts += pt
    if not boxes:
        sys.exit("no BOUNDS record in any dump")
    minlat = min(b[0] for b in boxes); maxlat = max(b[1] for b in boxes)
    minlon = min(b[2] for b in boxes); maxlon = max(b[3] for b in boxes)
    if args.bbox:
        minlon, minlat, maxlon, maxlat = args.bbox

    aspect = 1 / max(0.2, abs(__import__("math").cos(__import__("math").radians((minlat + maxlat) / 2))))
    h = args.width * (maxlat - minlat) / (maxlon - minlon) * aspect
    fig, ax = plt.subplots(figsize=(args.width, max(2.0, h)), dpi=args.dpi)
    ax.set_facecolor(polys.get("0x4b", "#FFFFFF"))

    # polygons, painted in TYP draw order
    by_order: dict[int, list] = {}
    for t, _lab, coords in shapes:
        by_order.setdefault(order.get(t, 99), []).append((t, coords))
    drawn = skipped = 0
    for lv in sorted(by_order):
        for t, coords in by_order[lv]:
            pass
        group = by_order[lv]
        for colour in {polys.get(t, None) for t, _ in group}:
            sel = [c for t, c in group if polys.get(t) == colour]
            if colour is None or not sel:
                skipped += len(sel)
                continue
            ax.add_collection(PolyCollection(sel, facecolors=colour,
                                             edgecolors="none", zorder=lv))
            drawn += len(sel)

    # lines
    for t in sorted({t for t, _, _ in lns}):
        segs = [c for tt, _, c in lns if tt == t]
        spec = lines.get(t)
        if not spec:
            continue
        if spec["casing"]:
            ax.add_collection(LineCollection(
                segs, colors=spec["casing"],
                linewidths=spec["width"] / 2.5 + spec["border"] * 0.8, zorder=20))
        ax.add_collection(LineCollection(
            segs, colors=spec["color"], linewidths=spec["width"] / 2.5, zorder=21))

    if args.labels:
        for t, lab, lo, la in pts:
            # clip to the drawn extent, otherwise labels from the whole tile
            # spill outside the axes
            if lab and minlon <= lo <= maxlon and minlat <= la <= maxlat:
                ax.text(lo, la, lab, fontsize=3.5, ha="center",
                        color="#333333", zorder=30, clip_on=True)

    ax.set_xlim(minlon, maxlon)
    ax.set_ylim(minlat, maxlat)
    ax.set_aspect(aspect)
    ax.set_xticks([]); ax.set_yticks([])
    for sp in ax.spines.values():
        sp.set_visible(False)
    fig.tight_layout(pad=0.1)
    fig.savefig(args.out, dpi=args.dpi, facecolor=ax.get_facecolor())
    print(f"level {args.level}: {drawn} polygons drawn, {skipped} unstyled, "
          f"{len(lns)} lines, {len(pts)} points")
    print(f"bbox {minlon:.4f},{minlat:.4f} -> {maxlon:.4f},{maxlat:.4f}")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
