"""Guard against the blank-map failure mode.

mkgmap doc/typ-compiler.txt: "If a polygon type is not listed in [_drawOrder],
then it will not be displayed at all." A polygon type emitted by the style but
missing from the TYP's drawOrder silently disappears on the device. This checks
the two files agree.
"""
from __future__ import annotations
import re, sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

# Both cartography variants must be checked. The wrist style is derived from the main
# one, so a type added to the main style silently appears there too.
VARIANTS = [
    (REPO / "style" / "swisstopo", REPO / "typ" / "swisstopo.txt"),
    (REPO / "style" / "swisstopo-wrist", REPO / "typ" / "swisstopo-wrist.txt"),
    # The winter variants recolour the same rules, so they must cover the same types.
    (REPO / "style" / "swisstopo", REPO / "typ" / "swisstopo-winter.txt"),
    (REPO / "style" / "swisstopo-wrist", REPO / "typ" / "swisstopo-wrist-winter.txt"),
]

def types_in(path: Path) -> set[str]:
    out = set()
    for line in path.read_text().splitlines():
        line = line.split("#", 1)[0]
        for m in re.finditer(r"\[\s*(0x[0-9a-fA-F]+)", line):
            out.add(m.group(1).lower())
    return out

def check(style: Path, typ: Path) -> int:
  # Strip comments first, and anchor section headers to line start -- otherwise a
  # mention of "[_drawOrder]" inside a comment is matched as the section itself.
  t = "\n".join(l for l in typ.read_text().splitlines()
                 if not l.lstrip().startswith(";"))

  sec = re.search(r"^\[_drawOrder\]\s*$(.*?)^\[end\]", t, re.S | re.M)
  if not sec:
    sys.exit("no [_drawOrder] section in the TYP -- ALL polygons would be hidden")
  draw = {m.lower() for m in re.findall(r"^Type=(0x[0-9a-fA-F]+)\s*,\s*\d+",
                                      sec.group(1), re.M)}
  defined_poly = {m.lower() for m in re.findall(
    r"^\[_polygon\](?:(?!^\[_)[\s\S])*?^Type=(0x[0-9a-fA-F]+)", t, re.M)}
  defined_line = {m.lower() for m in re.findall(
    r"^\[_line\](?:(?!^\[_)[\s\S])*?^Type=(0x[0-9a-fA-F]+)", t, re.M)}

  used_poly = types_in(style / "polygons")
  used_line = types_in(style / "lines")

  fail = 0
  missing_draw = sorted(used_poly - draw)
  if missing_draw:
    fail = 1
    print("FAIL polygon types used by the style but absent from [_drawOrder]")
    print("     -> these render as NOTHING on the device:")
    for x in missing_draw: print(f"       {x}")

  undef_poly = sorted(used_poly - defined_poly)
  if undef_poly:
    print("WARN polygons in drawOrder but with no [_polygon] definition "
          "(Garmin default colours will be used):")
    for x in undef_poly: print(f"       {x}")

  undef_line = sorted(used_line - defined_line)
  if undef_line:
    print("WARN line types with no [_line] definition (Garmin defaults):")
    for x in undef_line: print(f"       {x}")

  orphan = sorted(draw - used_poly)
  if orphan:
    print(f"note {len(orphan)} drawOrder entries not emitted by the style "
          f"(harmless): {' '.join(orphan)}")

  print(f"\npolygons: {len(used_poly)} used, {len(draw)} in drawOrder, "
      f"{len(defined_poly)} styled")
  print(f"lines:    {len(used_line)} used, {len(defined_line)} styled")
  print("OK" if not fail else "FAILED")
  return fail


total = 0
for style_dir, typ_file in VARIANTS:
    if not style_dir.exists() or not typ_file.exists():
        print(f"skip {style_dir.name}: not generated yet")
        continue
    print(f"=== {style_dir.name} + {typ_file.name}")
    total += check(style_dir, typ_file)
    print()
sys.exit(total)