"""Visual regression harness for the cartography (SPEC.md FR-CART7, §13.4).

Builds a fixed set of small reference maps, renders each one, and compares it against a
committed golden image. Cartography changes then show up as a diff a person can look at,
rather than as a surprise on a device.

    python3 tools/visual_regression.py            # compare against the goldens
    python3 tools/visual_regression.py --update   # accept the current output as golden
    python3 tools/visual_regression.py --only grindelwald-summer

**What this does not claim.** The renderer is not Garmin's: it draws the compiled map's
own geometry and types with the same TYP colours the device uses, but line casing, label
placement and pattern fills are approximations. A diff here means the *map data or its
colours* changed. It cannot tell you the device will look right — only hardware does that
(docs/device-verification.md).

Goldens also depend on the swisstopo release the map was built from. Each one records
its release, and a mismatch is reported separately from a pixel difference, because
"swisstopo published new data" and "we changed the cartography" are different events.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
GOLDEN_DIR = REPO / "cartography" / "golden"
WORK = REPO / "out" / "visual"

# Small on purpose: the harness has to be cheap enough to run before a commit.
REFERENCES = [
    # name, extra build_recipe arguments, renderer arguments
    ("grindelwald-summer", ["--place", "Grindelwald", "--radius-km", "2",
                            "--preset", "hiking", "--relief", "off"], []),
    ("zermatt-winter", ["--place", "Zermatt", "--radius-km", "2",
                        "--preset", "skimo", "--relief", "off", "--palette", "winter"],
     ["--typ", str(REPO / "typ" / "swisstopo-winter.txt")]),
    ("andermatt-slope", ["--place", "Andermatt", "--radius-km", "2",
                         "--preset", "skimo", "--relief", "off", "--slope"], []),
    ("bern-cycling", ["--place", "Bern", "--radius-km", "2",
                      "--preset", "cycling", "--relief", "off"], []),
]

# Below this the difference is rendering noise rather than a change worth looking at.
TOLERANCE_PERCENT = 0.2


def toolchain() -> dict[str, str]:
    env = {}
    for line in (REPO / "vendor" / "toolchain.env").read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            env[k] = v
    return env


def build(name: str, args: list[str]) -> tuple[Path, str]:
    """Build one reference map, returning its .img and the dataset release used."""
    work = f"visual/{name}"
    cmd = [
        "cargo", "run", "--release", "-q", "-p", "s2g-core", "--example", "build_recipe",
        "--", "--device", "edge-840", "--work-dir", work, "--no-log", *args,
    ]
    out = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"{name}: build failed\n{out.stdout}\n{out.stderr}")
    img_dir = WORK / name / "img"
    gmapsupp = img_dir / "gmapsupp.img"
    if not gmapsupp.is_file():
        raise SystemExit(f"{name}: no map at {gmapsupp}")

    manifest = gmapsupp.with_suffix(".manifest.json")
    release = "unknown"
    if manifest.is_file():
        sources = json.loads(manifest.read_text()).get("sources", [])
        tlm = [s for s in sources if s["collection"].endswith("swisstlm3d")]
        if tlm:
            release = tlm[0]["item"]
    return img_dir, release


def render(img_dir: Path, dest: Path, extra: list[str]) -> None:
    env = toolchain()
    # The detail tiles, not the container: gmapsupp.img holds them, and the overview
    # map is a separate generalised copy that would draw over the detail.
    # mkgmap also writes an index (ovm_mdr.img) and a TYP, neither of which is a map:
    # ImgDump rejects them with "No TRE entry in img file". Detail tiles are the
    # numeric ones.
    tiles = sorted(
        p for p in img_dir.glob("*.img")
        if p.stem.isdigit()
    )
    if not tiles:
        raise SystemExit(f"no tile images in {img_dir}")

    dumps = []
    for tile in tiles:
        dump = tile.with_suffix(".tsv")
        with dump.open("w") as fh:
            r = subprocess.run(
                [env["JAVA_BIN"], "-cp",
                 f"{env['MKGMAP_JAR']}:{REPO / 'tools' / 'imgdump'}", "ImgDump", str(tile)],
                cwd=REPO, stdout=fh, stderr=subprocess.PIPE, text=True,
            )
        if r.returncode != 0:
            raise SystemExit(f"ImgDump failed for {tile}:\n{r.stderr}")
        dumps.append(str(dump))

    r = subprocess.run(
        [sys.executable, str(REPO / "tools" / "render.py"), *dumps,
         "-o", str(dest), "--width", "6", "--dpi", "100", *extra],
        cwd=REPO, capture_output=True, text=True,
    )
    if r.returncode != 0:
        raise SystemExit(f"render failed for {img_dir}:\n{r.stdout}\n{r.stderr}")


def difference(a: Path, b: Path) -> tuple[float, Path | None]:
    """Percentage of differing pixels, and a diff image when they differ."""
    from PIL import Image, ImageChops

    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    if ia.size != ib.size:
        return 100.0, None
    diff = ImageChops.difference(ia, ib)
    changed = sum(1 for p in diff.getdata() if p != (0, 0, 0))
    pct = 100.0 * changed / (ia.size[0] * ia.size[1])
    if changed == 0:
        return 0.0, None
    # Amplify, so a reviewer can see where it moved.
    out = b.with_name(b.stem + "-diff.png")
    diff.point(lambda v: min(255, v * 8)).save(out)
    return pct, out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true", help="accept current output as golden")
    ap.add_argument("--only", help="run one reference by name")
    args = ap.parse_args()

    GOLDEN_DIR.mkdir(parents=True, exist_ok=True)
    index_path = GOLDEN_DIR / "index.json"
    index = json.loads(index_path.read_text()) if index_path.is_file() else {}

    failures: list[str] = []
    for name, build_args, render_args in REFERENCES:
        if args.only and args.only != name:
            continue
        print(f"--- {name}")
        img_dir, release = build(name, build_args)
        current = WORK / f"{name}.png"
        render(img_dir, current, render_args)

        golden = GOLDEN_DIR / f"{name}.png"
        if args.update or not golden.is_file():
            current.replace(golden)
            index[name] = {"release": release, "build": build_args, "render": render_args}
            print(f"    wrote golden from release {release}")
            continue

        pct, diff_img = difference(golden, current)
        known_release = index.get(name, {}).get("release")
        if known_release and known_release != release:
            print(f"    release changed: golden {known_release}, now {release}")
            print("    a difference here may be new swisstopo data rather than a "
                  "cartography change")
        if pct > TOLERANCE_PERCENT:
            failures.append(f"{name}: {pct:.2f}% of pixels differ")
            print(f"    DIFFERS: {pct:.2f}% of pixels" + (f", see {diff_img}" if diff_img else ""))
        else:
            print(f"    ok ({pct:.2f}% differ, tolerance {TOLERANCE_PERCENT}%)")

    if args.update or not (GOLDEN_DIR / "index.json").is_file():
        index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n")

    if failures:
        print("\n" + "\n".join(failures))
        raise SystemExit(
            "the cartography changed; review the diffs and re-run with --update to accept"
        )
    print("\nno visual change")


if __name__ == "__main__":
    main()
