#!/usr/bin/env bash
# Build the committed test fixture: a small swissTLM3D extract that CI can run the
# pipeline against offline, without the 10.78 GB national GeoPackage.
#
# ogr2ogr is used rather than hand-rolled SQL because it recreates the GeoPackage
# R-tree indexes, and the R-tree path is exactly what the reader tests exercise.
set -euo pipefail

SRC="${1:-$HOME/.cache/swisstopo2garmin/ch.swisstopo.swisstlm3d/swisstlm3d_2026-02/swisstlm3d_2026-02.gpkg}"
OUT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/crates/s2g-core/tests/fixtures/grindelwald.gpkg"

# 2 x 2 km around the centre of Grindelwald village (LV95).
MINE=2645000; MINN=1163000; MAXE=2647000; MAXN=1165000

rm -f "$OUT"
for layer in tlm_strassen_strasse tlm_bb_bodenbedeckung \
             tlm_gewaesser_fliessgewaesser tlm_namen_flurname \
             tlm_bauten_gebaeude_footprint; do
  echo "  $layer"
  # -lco FID=id keeps the fixture's primary key identical to production's. GDAL
  # would otherwise name it `fid`, and a fixture whose schema differs from the real
  # data is worse than no fixture at all.
  ogr2ogr -f GPKG -update -append -lco FID=id \
    -spat $MINE $MINN $MAXE $MAXN \
    "$OUT" "$SRC" "$layer" 2>/dev/null || \
  ogr2ogr -f GPKG -lco FID=id -spat $MINE $MINN $MAXE $MAXN "$OUT" "$SRC" "$layer"
done

echo
ls -lh "$OUT"
echo "bbox LV95: $MINE $MINN $MAXE $MAXN"
