#!/bin/sh
# Build the map files for a device validation sweep (SPEC.md §13.5, docs/device-verification.md).
#
# One small area, exercising every cartography path that has never been on hardware:
# the winter scheme, the night palette, slope classes, the new POI types, and the wrist
# variant. Each build's working files are deleted after its .img is copied out, because
# the whole set would otherwise want several gigabytes.
#
#   sh tools/device_test_set.sh [place] [radius_km]
set -e

REPO=$(cd "$(dirname "$0")/.." && pwd)
PLACE=${1:-Grindelwald}
RADIUS=${2:-6}
OUT="$REPO/out/device-test"
mkdir -p "$OUT"

build() {
  name=$1; device=$2; preset=$3; palette=$4; relief=$5; slope=$6; contour=$7; label=$8
  printf '%-28s ' "$name"
  extra=""
  [ "$slope" = "yes" ] && extra="--slope"
  cargo run --release -q -p s2g-core --example build_recipe -- \
      --place "$PLACE" --radius-km "$RADIUS" \
      --device "$device" --preset "$preset" --palette "$palette" \
      --relief "$relief" --contour "$contour" $extra \
      --name "$label" \
      --work-dir "device-test/$name" --no-log > "$OUT/$name.log" 2>&1 || {
        echo "FAILED (see $OUT/$name.log)"; return 1; }

  cp "$REPO/out/device-test/$name/img/gmapsupp.img" "$OUT/$name.img"
  cp "$REPO/out/device-test/$name/img/gmapsupp.manifest.json" "$OUT/$name.manifest.json" 2>/dev/null || true
  size=$(wc -c < "$OUT/$name.img" | tr -d ' ')
  # The working files are large and re-derivable; the region stays in the shared cache.
  rm -rf "$REPO/out/device-test/$name"
  printf '%10s B\n' "$size"
}

# The last column is the map's *name*, which is what the device's map manager lists.
# Six maps of one place need six names, or the list is no help in choosing between them.
echo "area: $PLACE ${RADIUS} km radius"
echo

#     name                       device         preset   palette relief  slope contour
build edge-1-hiking-summer       edge-840       hiking   summer  gentle  no    20 "Grindelwald hiking"
build edge-2-slope-no-relief     edge-840       hiking   summer  off     yes   20 "Grindelwald slope classes"
build edge-3-skimo-winter-slope  edge-840       skimo    winter  gentle  yes   20 "Grindelwald ski touring"
build edge-4-full-10m            edge-840       full     summer  gentle  no    10 "Grindelwald full topo 10 m"
build fenix-1-hiking-summer      fenix-5-plus   hiking   summer  gentle  no    20 "Grindelwald hiking, wrist"
build fenix-2-skimo-winter-slope fenix-5-plus   skimo    winter  off     yes   20 "Grindelwald ski touring, wrist"

echo
echo "written to $OUT"
ls -la "$OUT"/*.img | awk '{printf "  %10d B  %s\n", $5, $9}'
