# Raster overlays — the swisstopo paper map on the device

*Requirements: SPEC.md §8.5, FR-R1…FR-R6. Off by default. Built and structurally
verified; **never yet drawn by a device** — see [device-verification.md](device-verification.md) §C.9.*

## Why this exists

The vector map is the better map. It can be searched, it can be routed along, it scales
across zoom levels, and it costs a fraction of the space. What it cannot do is look like
the printed Landeskarte: the rock drawing (*Felszeichnung*) is a hand-crafted raster
product with no vector equivalent, and the TYP model allows no label halos, no rotated
text and no custom fonts (SPEC.md §8.1).

A Garmin Custom Map sidesteps the model entirely, because it *is* the paper map — JPEG
tiles cut from `ch.swisstopo.pixelkarte-farbe` and georeferenced in a KMZ.

So this is an **addition** to the vector map, never a replacement. The overlay cannot be
searched, cannot be routed along, and renders only in a narrow zoom band.

## What the numbers turned out to be

The project deferred this feature on the reasoning that "Custom Maps are capped near 100
tiles of 1 MP … so the vector map is better in every respect except literal appearance"
(SPEC.md §16, before this was built). The caps are real. What the arithmetic showed is
that they are less restrictive than they sound:

| | |
|---|---|
| Tiles per device | 100, across *all* custom maps on it |
| Pixels per tile | 1 048 576 (a 1024 × 1024 square) |
| Total | 105 megapixels |
| At the source's native 1.25 m/px | **about 12 × 12 km** |

12 km square is a day's walking. That is the headline: within one day's range the device
can show the paper map at the resolution swisstopo drew it.

Beyond that the planner coarsens rather than crops (FR-R5), and says so:

| Area (radius) | Tiles | Resolution | Verdict |
|---|---|---|---|
| 0.5 km | 1 | 1.25 m/px | native |
| 3 km | 25 | 1.25 m/px | native |
| 6 km | 100 | 1.25 m/px | native, exactly at the budget |
| 50 km | 100 | 9.9 m/px | coarsened; the user is told |
| 120 km | 100 | 24.9 m/px | coarser than the vector map underneath; the user is told that too |

### Size is not the constraint

Measured with `examples/probe_raster.rs` at 1.25 m/px over four 9 km² areas chosen to
span Swiss terrain:

| Area | JPEG bytes per pixel |
|---|---|
| Plateau farmland | 0.201 |
| Grindelwald, alpine valley | 0.380 |
| Jungfrau, rock and ice | 0.417 |
| Basel, dense town | 0.528 |

A 2.6× spread, which is why `RasterPlan::approx_bytes` is documented as a rule of thumb
and not an estimate with the standing of the size model. But the useful conclusion is
that a *full* 100-tile overlay is about 55 MB even at the dense-town rate, against a map
budget measured in gigabytes. **The tile count binds; the byte size does not.** Nothing
in this feature needs a size estimator.

Download time is about 0.8 s per tile against the live WMS, so a full overlay takes
roughly 80 s. That is the cost the user is quoted before committing.

## Two things that fail silently

Both were checked against the live service rather than assumed, because neither
announces itself.

**Axis order.** WMS 1.3.0 takes `BBOX` in the order the CRS *declares*, which is not
always x,y:

| CRS | Order | Example |
|---|---|---|
| EPSG:4326 | latitude first | `BBOX=46.60,8.00,46.65,8.08` |
| EPSG:2056 (LV95) | easting first | `BBOX=2643000,1163000,2645000,1165000` |

Get it backwards and the service returns **HTTP 200 with a valid JPEG** — a blank tile
from outside the data's extent. During development a 6.4 kB blank was briefly mistaken
for a working request on the strength of its status code and content type. The tell is
the size: a real 1 MP map tile is a few hundred kilobytes.

**Error responses.** A rejected `GetMap` also comes back as HTTP 200, with an XML
`ServiceExceptionReport` body. So the fetcher checks the JPEG magic bytes (`FF D8`) and
quotes the body in the error. Without that check the error page would be stored in the
KMZ as a "tile" and render as an empty rectangle on the device.

## Why the grid is planned in degrees (FR-R3)

A KMZ `GroundOverlay` is georeferenced by a `LatLonBox` — north, south, east, west: an
**axis-aligned box in WGS84**.

LV95 is an oblique Mercator projection. A rectangle in LV95 is *not* a rectangle in
lat/lon — its northern edge is not a line of constant latitude. Planning the grid in
LV95 and declaring each tile's projected corners as a `LatLonBox` would therefore
misplace the imagery, by more at the edges of the country than in the middle.

So the grid is uniform in degrees and the WMS is asked for EPSG:4326, which makes every
tile's box exact by construction. For the same reason, converting the selected area to
degrees projects **all four** LV95 corners and takes the extremes; using two would clip
a sliver off the area.

The consequence is that tiles are plate carrée, which is what the Custom Maps format is
— the device resamples into its own projection from the box. Pixels are allocated in
proportion to each tile's *ground* extent so resolution is equal in both directions,
which at Swiss latitudes means a full tile spans about 1.46× as much longitude as
latitude (1/cos 46.6°).

## The KMZ

A KMZ is a ZIP holding `doc.kml` and the tiles. `zip.rs` is the read side — it locates a
member in a *remote* archive with range requests — so `zip_write.rs` is a minimal
STORE-only writer.

STORE rather than DEFLATE is correct, not lazy: JPEG payloads are already entropy-coded,
so deflating them costs CPU to save fractions of a percent. No new dependency was needed;
`flate2` was already present and exposes CRC-32.

```
doc.kml                 one <GroundOverlay> per tile, with its <LatLonBox>
tiles/000_000.jpg       row 000 is the northern edge
tiles/000_001.jpg
...
```

Verified two ways: round-tripped through the project's own range-request reader in a
unit test, and the real output checked with Python's `zipfile` — all CRCs match, the KML
parses as XML, every `<href>` resolves to a member, and no member is an orphan.

The archive is written a tile at a time and renamed from `.partial` on success, so peak
memory is one tile rather than the whole archive (NFR-2) and a cancelled build leaves
nothing a device would try to read.

## Using it

Tick **Paper-map overlay** on the content step. The app shows the tile count, the
resolution it can achieve, the approximate size and the download time before you commit,
along with any warning about coarsening.

The build produces a second file beside `gmapsupp.img`, and the install step writes it to
`Garmin/CustomMaps/` — both targets are shown before either is written (FR-R1). A device
whose profile does not state its Custom Map limits is offered no overlay at all rather
than a guessed one (FR-R4); the checkbox is disabled with the reason shown.

A failure to build or install the overlay is reported as a warning and never fails the
build: the vector map is the deliverable and it is already correct.

## Reproducing the measurements

```
cargo run --release -p s2g-core --example probe_raster -- \
    --place-e 2645921 --place-n 1163748 --radius-km 2 --out out/raster
```

Prints the plan, fetches every tile, and reports the measured bytes per pixel against
the predicted figure. The four rows in the table above are this command at
`--radius-km 1.5` over Basel (2611000, 1267000), the Jungfrau (2643000, 1147000), the
plateau (2580000, 1210000) and Grindelwald (2645921, 1163748).

## Device limits and where they come from

Garmin publishes none of this. Its own documentation was unreachable while this was
built, so the figures in `devices/*.json` are `community`, with sources recorded
(working agreement rule 3):

- The QGIS *GarminCustomMap* plugin, which encodes the limits it has to respect:
  "Each jpg-file is limited to 1 megapixel (e.g. 1024 x 1024 pixel or 2048 x 512 pixel)"
  and "The number of Custom Map jpgs … is usually limited to max. 100 jpgs (across all
  Custom Maps on the unit)."
  <https://github.com/NINAnor/GarminCustomMaps>
- Garmin's own forums, where the 100-tile cap is discussed as current behaviour on recent
  hardware.
  <https://forums.garmin.com/outdoor-recreation/outdoor-recreation/f/fenix-7-series/348498/why-are-the-kmz-custom-maps-still-limited-to-100-tiles-on-the-new-devices>

The 500-tile allowance reported for the Montana, Oregon 6x0 and GPSMAP 64 is *not*
applied to either shipped profile, neither of which is in that list.

`Garmin/CustomMaps` as the install directory is not a guess either: both shipped
profiles' `GarminDevice.xml` advertise it, which is how the project learned Edge devices
have raster support at all (docs/m0-findings.md §4.16).

## What is still unknown

Everything about how a device behaves. The file is correct; the rendering is unproven.
[device-verification.md](device-verification.md) §C.9 is the sweep that would settle it,
and its first question is the one that matters: does the device list the custom map at
all? A "no" is a real result and retires the feature.

Until that is done, this is a feature that is built and unproven, and it stays off by
default.

---

Map data © swisstopo. The overlay carries the same attribution inside the KMZ (FR-R6).
