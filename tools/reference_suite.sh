#!/bin/sh
# Build the reference suite the size estimate is judged against (SPEC.md FR-60).
#
# FR-60 asks for ±25% on at least ten reference areas. These are chosen to be areas the
# model has NOT seen: none of them appears in estimator/training-samples.jsonl, and the
# four wrist builds are the case the training set has none of, since all sixteen
# training builds were handlebar maps.
#
# A manifest is enough to evaluate a build afterwards -- it records the recipe, the
# per-layer counts and the actual size -- so only the manifests are kept and the .img
# files and working directories are deleted. Ten builds of these sizes would otherwise
# want several gigabytes.
#
#   sh tools/reference_suite.sh
#   cargo run --release -q -p s2g-core --example check_estimator -- out/reference
set -e

REPO=$(cd "$(dirname "$0")/.." && pwd)
OUT="$REPO/out/reference"
mkdir -p "$OUT"

build() {
  name=$1; place=$2; radius=$3; device=$4; preset=$5; relief=$6; contour=$7; slope=$8
  printf '%-32s ' "$name"
  extra=""
  [ "$slope" = "yes" ] && extra="--slope"
  cargo run --release -q -p s2g-core --example build_recipe -- \
      --place "$place" --radius-km "$radius" \
      --device "$device" --preset "$preset" \
      --relief "$relief" --contour "$contour" $extra \
      --work-dir "reference/$name" --no-log > "$OUT/$name.log" 2>&1 || {
        echo "FAILED (see $OUT/$name.log)"; return 1; }

  cp "$REPO/out/reference/$name/img/gmapsupp.manifest.json" "$OUT/$name.manifest.json"
  size=$(wc -c < "$REPO/out/reference/$name/img/gmapsupp.img" | tr -d ' ')
  # Only the manifest is needed to evaluate the estimate; the rest is re-derivable.
  rm -rf "$REPO/out/reference/$name"
  printf '%10s B\n' "$size"
}

# Places and radii deliberately absent from the training plan, spread across terrain
# types: alpine, plateau, Jura, Ticino, and a city.
#      name                        place        km  device        preset  relief   c   slope
build "01-saas-fee-hiking"         "Saas-Fee"    5  edge-840      hiking  gentle   20  no
build "02-saas-fee-wrist"          "Saas-Fee"    5  fenix-5-plus  hiking  off      20  no
build "03-appenzell-hiking"        "Appenzell"   6  edge-840      hiking  gentle   20  no
build "04-appenzell-wrist-slope"   "Appenzell"   6  fenix-5-plus  hiking  off      20  yes
build "05-le-locle-full"           "Le Locle"    5  edge-840      full    off      50  no
build "06-bellinzona-hiking"       "Bellinzona"  7  edge-840      hiking  detailed 20  no
build "07-bellinzona-wrist"        "Bellinzona"  7  fenix-5-plus  hiking  off      50  no
build "08-arosa-skimo"             "Arosa"       6  edge-840      skimo   gentle   20  yes
build "09-arosa-wrist-skimo"       "Arosa"       6  fenix-5-plus  skimo   off      20  no
build "10-neuchatel-cycling"       "Neuchâtel"   8  edge-840      cycling off      50  no
