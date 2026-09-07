#!/bin/sh
# Attach the sample maps to a GitHub release (docs/sample-maps.md).
#
# The maps cannot be built in CI: that needs the 10 GB swissTLM3D GeoPackage, which no
# runner is going to download. So they are built locally and uploaded here, which is also
# why they are versioned by the release they hang off rather than committed --
# .gitignore excludes *.img deliberately, and 7 MB of binaries per release would
# accumulate in the history forever.
#
#   sh tools/device_test_set.sh          # build them first
#   sh tools/publish_samples.sh v0.1.0   # then attach them
set -e

TAG=$1
if [ -z "$TAG" ]; then
  echo "usage: sh tools/publish_samples.sh <tag>" >&2
  exit 2
fi

REPO=$(cd "$(dirname "$0")/.." && pwd)
DIR="$REPO/out/device-test"

if [ ! -f "$DIR/edge-1-hiking-summer.img" ]; then
  echo "no sample maps in $DIR -- run: sh tools/device_test_set.sh" >&2
  exit 1
fi

# Checksums, so somebody can tell a truncated download from a broken map.
( cd "$DIR" && shasum -a 256 ./*.img > SHA256SUMS-samples )

echo "attaching to $TAG:"
ls -l "$DIR"/*.img "$DIR"/*.manifest.json "$DIR/SHA256SUMS-samples" | awk '{print "  " $9, $5}'

# --clobber so re-running after a rebuild replaces the assets rather than failing.
gh release upload "$TAG" \
  "$DIR"/*.img "$DIR"/*.manifest.json "$DIR/SHA256SUMS-samples" \
  --clobber

echo
echo "done. The release notes should link docs/sample-maps.md and say these files"
echo "have not been verified on hardware."
