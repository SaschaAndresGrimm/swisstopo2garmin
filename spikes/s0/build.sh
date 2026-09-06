#!/usr/bin/env bash
# S0.2 + S0.3 end-to-end: swissTLM3D + swissALTI3D -> gmapsupp.img
# Usage: spikes/s0/build.sh <Place> [radius_km] [contour_interval_m]
set -euo pipefail

PLACE="${1:-Grindelwald}"
RADIUS="${2:-6}"
INTERVAL="${3:-20}"

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Cartography variant. The wrist build uses the reduced style and thinner linework
# (FR-CART6); a handlebar-density map is unreadable on a ~1.3 inch screen.
STYLE_NAME="${S2G_STYLE:-swisstopo}"
STYLE_DIR="$REPO/style/$STYLE_NAME"
TYP_FILE="$REPO/typ/${STYLE_NAME}.txt"
SUFFIX="${S2G_SUFFIX:-}"

# mkgmap needs exactly one --dem-dist per style level, and the wrist style has fewer
# levels than the handlebar one. Derive the list rather than hardcoding it.
LEVELS=$(grep -E "^levels" "$REPO/style/$STYLE_NAME/options" | tr ',' '\n' | grep -c ':')
DEM_DISTS=$(python3 -c "print(','.join(str(3314 << i) for i in range($LEVELS)))")
source "$REPO/vendor/toolchain.env"

WORK="${S2G_WORK:-$REPO/work}/$PLACE"
OUT="$REPO/out"
mkdir -p "$WORK" "$OUT"

# The TYP is generated from the measured swisstopo palette; never edit it by hand.
python3 "$REPO/tools/make_typ.py"
python3 "$REPO/tools/make_wrist_style.py"
python3 "$REPO/spikes/s0/checkstyle.py" >/dev/null || {
  echo "style/TYP mismatch -- polygons would be invisible on the device"; exit 1; }

echo "=== 1/6  clip swissTLM3D around $PLACE (${RADIUS} km)"
python3 "$REPO/spikes/s0/tlm2osm.py" --place "$PLACE" --radius-km "$RADIUS" \
        -o "$WORK/vector.osm" --bbox-out "$WORK/bbox.txt"

BBOX="$(cat "$WORK/bbox.txt")"

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
  --style-file="$STYLE_DIR" \
  --tdbfile --index --code-page=1252 --lower-case \
  --family-id="$FID" --product-id=1 \
  --family-name="swisstopo2garmin" \
  --series-name="swissTLM3D $PLACE" \
  --description="swissTLM3D (c) swisstopo" \
  --draw-priority=30 \
  --overview-mapname=ovm --overview-mapnumber="$OVNUM" \
  --dem="$WORK/dem" --dem-dists="$DEM_DISTS" \
  "$WORK/tiles"/*.osm.pbf "$TYP_FILE" | grep -iE "^ *(error|.*Exception:)" || true

echo "=== 6/6  mkgmap: combine into gmapsupp"
"$JAVA_BIN" -Xmx4g -jar "$MKGMAP_JAR" --gmapsupp --index \
  --family-id="$FID" --product-id=1 \
  ${FID}*.img ovm.img *.typ | grep -iE "^ *(error|.*Exception:)" || true

cp gmapsupp.img "$OUT/gmapsupp-${PLACE}${SUFFIX}.img"

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
      -o "$OUT/preview-${PLACE}${SUFFIX}.png" --width 13 --typ "$TYP_FILE" || true
fi
echo
python3 "$REPO/spikes/s0/imgcheck.py" "$OUT/gmapsupp-${PLACE}${SUFFIX}.img"
echo
echo "-> $OUT/gmapsupp-${PLACE}${SUFFIX}.img"
echo "   Copy to the device as /Garmin/gmapsupp.img (see docs/device-verification.md)"
