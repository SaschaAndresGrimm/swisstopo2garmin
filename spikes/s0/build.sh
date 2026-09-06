#!/usr/bin/env bash
# S0.2 + S0.3 end-to-end: swissTLM3D + swissALTI3D -> gmapsupp.img
# Usage: spikes/s0/build.sh <Place> [radius_km] [contour_interval_m]
set -euo pipefail

PLACE="${1:-Grindelwald}"
RADIUS="${2:-6}"
INTERVAL="${3:-20}"

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$REPO/vendor/toolchain.env"

WORK="${S2G_WORK:-$REPO/work}/$PLACE"
OUT="$REPO/out"
mkdir -p "$WORK" "$OUT"

# The TYP is generated from the measured swisstopo palette; never edit it by hand.
python3 "$REPO/tools/make_typ.py"
python3 "$REPO/spikes/s0/checkstyle.py" >/dev/null || {
  echo "style/TYP mismatch -- polygons would be invisible on the device"; exit 1; }

echo "=== 1/6  clip swissTLM3D around $PLACE (${RADIUS} km)"
python3 "$REPO/spikes/s0/tlm2osm.py" --place "$PLACE" --radius-km "$RADIUS" \
        -o "$WORK/vector.osm"

# derive the LV95 bbox the converter used, for the contour stage
BBOX=$(python3 - "$PLACE" "$RADIUS" <<'PY'
import sys, sqlite3, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
sys.path.insert(0, str(pathlib.Path("REPO_PLACEHOLDER")/"spikes"/"s0"))
from stac import cache_dir
from tlm2osm import parse_gpkg_geom
gpkg = sorted((cache_dir()/"ch.swisstopo.swisstlm3d").glob("*/*.gpkg"))[-1]
con = sqlite3.connect(f"file:{gpkg}?mode=ro", uri=True)
row = con.execute("SELECT geom FROM tlm_namen_siedlungsname_zentrum WHERE name=? LIMIT 1",
                  (sys.argv[1],)).fetchone()
_, rings = parse_gpkg_geom(row[0]); e, n = rings[0][0]
r = float(sys.argv[2])*1000
print(f"{e-r:.0f} {n-r:.0f} {e+r:.0f} {n+r:.0f}")
PY
)
BBOX=${BBOX//REPO_PLACEHOLDER/$REPO}

echo "=== 2/6  contours at ${INTERVAL} m from swissALTI3D"
GDAL_DISABLE_READDIR_ON_OPEN=EMPTY_DIR GDAL_HTTP_MULTIPLEX=YES VSI_CACHE=TRUE \
python3 "$REPO/spikes/s0/contours.py" --bbox-lv95 $BBOX --interval "$INTERVAL" \
        --simplify 8 --reuse --workdir "$WORK/alti" -o "$WORK/contours.osm"

echo "=== 3/6  DEM tiles for relief shading"
GDAL_DISABLE_READDIR_ON_OPEN=EMPTY_DIR VSI_CACHE=TRUE \
python3 "$REPO/spikes/s0/make_dem.py" --bbox-lv95 $BBOX --arcsec 1 \
        --out "$WORK/dem"

echo "=== 4/6  splitter"
rm -rf "$WORK/tiles"; mkdir -p "$WORK/tiles"
"$JAVA_BIN" -Xmx4g -jar "$SPLITTER_JAR" --output-dir="$WORK/tiles" \
   --max-nodes=700000 --mapid="${S2G_FID:-6324}0001" "$WORK/vector.osm" "$WORK/contours.osm" >/dev/null

# Two passes are required. A single `--gmapsupp` run writes the overview map as a
# separate file and ships a gmapsupp containing only the detail tiles, which a Garmin
# device lists but cannot draw (docs/m0-findings.md §4.5).
FID="${S2G_FID:-6324}"
OVNUM="${FID}0000"

echo "=== 5/6  mkgmap: tiles + overview map"
rm -rf "$WORK/img"; mkdir -p "$WORK/img"; cd "$WORK/img"
"$JAVA_BIN" -Xmx4g -jar "$MKGMAP_JAR" \
  --style-file="$REPO/style/swisstopo" \
  --tdbfile --index --code-page=1252 --lower-case \
  --family-id="$FID" --product-id=1 \
  --family-name="swisstopo2garmin" \
  --series-name="swissTLM3D $PLACE" \
  --description="swissTLM3D (c) swisstopo" \
  --draw-priority=30 \
  --overview-mapname=ovm --overview-mapnumber="$OVNUM" \
  --dem="$WORK/dem" --dem-dists=3314,6628,13256,26512,53024 \
  "$WORK/tiles"/*.osm.pbf "$REPO/typ/swisstopo.txt" | grep -iE "^ *(error|.*Exception:)" || true

echo "=== 6/6  mkgmap: combine into gmapsupp"
"$JAVA_BIN" -Xmx4g -jar "$MKGMAP_JAR" --gmapsupp --index \
  --family-id="$FID" --product-id=1 \
  ${FID}*.img ovm.img *.typ | grep -iE "^ *(error|.*Exception:)" || true

cp gmapsupp.img "$OUT/gmapsupp-${PLACE}.img"

# Desktop preview: dump the compiled map with mkgmap's own reader, then render
# it through the project TYP palette (SPEC.md FR-CART7).
if [ -f "$REPO/tools/imgdump/ImgDump.class" ]; then
  echo "=== preview"
  for t in ${FID}0*.img; do
    [ "$t" = "${OVNUM}.img" ] && continue
    "$JAVA_BIN" -cp "$MKGMAP_JAR:$REPO/tools/imgdump" ImgDump "$t" \
        > "${t%.img}.tsv" 2>/dev/null
  done
  python3 "$REPO/tools/render.py" ${FID}0*.tsv --level 0 \
      -o "$OUT/preview-${PLACE}.png" --width 13 || true
fi
echo
python3 "$REPO/spikes/s0/imgcheck.py" "$OUT/gmapsupp-${PLACE}.img"
echo
echo "-> $OUT/gmapsupp-${PLACE}.img"
echo "   Copy to the device as /Garmin/gmapsupp.img (see docs/device-verification.md)"
