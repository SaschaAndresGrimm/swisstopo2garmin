# swisstopo2garmin — Software Specification

**Status:** Draft v1.0 · **Date:** 2026-09-06 · **Owner:** Sascha Grimm

A cross-platform desktop application that turns freely available swisstopo geodata into
Garmin-compatible vector maps for Garmin Edge cycling computers and Garmin fēnix / epix
watches, with a graphical interface for choosing the target device, choosing the area,
and installing the result.

---

## 1. Purpose and scope

### 1.1 Problem

swisstopo publishes the best topographic data in the world for Switzerland, free of charge,
since 1 March 2021. Garmin devices ship with generic global basemaps that lack Swiss trail
classifications, Swiss land-cover detail, accurate contours, and the visual language of the
Landeskarte that Swiss outdoor users navigate by.

Bridging the two today requires a hand-rolled toolchain (GDAL + `ogr2osm` + `splitter` +
`mkgmap` + hand-written style and TYP files), several gigabytes of manual downloads,
and knowledge of Garmin's undocumented device limits. Existing attempts
([`martinzellner/swisstopo-garmin`](https://github.com/martinzellner/swisstopo-garmin),
[`wirhabenzeit/sac-skimo-garmin`](https://github.com/wirhabenzeit/sac-skimo-garmin))
are useful proofs of concept but are command-line scripts, incomplete, and not
device-aware.

### 1.2 Goal

One application. Pick your device, pick your area, press Build, get a file that works.

### 1.3 In scope for v1

- macOS (Apple Silicon + Intel), Windows x64, Linux x64 desktop GUI.
- Acquisition and caching of swisstopo open geodata via the official STAC API.
- Conversion of swissTLM3D vector data into Garmin `.img` vector maps.
- Contour line generation from swissALTI3D.
- A custom TYP file giving the map a Landeskarte-like appearance.
- A device profile database that constrains output to what the target device can hold.
- Four area selection modes: draw on map, administrative units, whole Switzerland, GPX corridor.
- Guided install to a connected device, plus plain file export.

### 1.4 Explicitly out of scope for v1 (see §16 for the roadmap)

- **On-device routing / turn-by-turn.** v1 maps are non-routable. This is a deliberate
  scoping decision: deriving a clean routing graph from swissTLM3D topology is the single
  largest risk in the project and is deferred to v2. Devices can still follow a loaded
  course and display off-course warnings against a non-routable map.
- Raster maps (BirdsEye `.jnx`, Custom Maps `.kmz`). Garmin removed BirdsEye support on
  fēnix 7 / epix, and `.kmz` Custom Maps are not supported on Edge or fēnix. Vector `.img`
  is the only format that works across the whole target device range.
- Address search / geocoding.
- Countries other than Switzerland and Liechtenstein (the swissTLM3D coverage area).
- Mobile apps, web service, or hosted build farm.
- Garmin Connect / Garmin Express integration (we write to the USB mass-storage volume).

### 1.5 Non-goals (permanent)

- We do not redistribute prebuilt maps. The app builds locally from official sources.
- We do not attempt to reverse-engineer or replace `mkgmap`. We orchestrate it.
- We do not scrape `map.geo.admin.ch`. We use documented APIs and bulk downloads only.

---

## 2. Users and use cases

| # | User | Use case | Success looks like |
|---|------|----------|--------------------|
| U1 | Road cyclist with an Edge 840 | Wants Swiss detail for weekend rides across a few cantons | Selects Bern + Fribourg + Vaud, builds a ~250 MB map, drags it to the Edge, sees Swiss trails and contours |
| U2 | Alpinist with a fēnix 8 | Wants the Landeskarte look on the wrist for a hut-to-hut tour | Imports the SAC route GPX, builds a 10 km corridor, gets a ~15 MB map that renders fast on the watch |
| U3 | Bikepacker with an Edge 1050 | Wants all of Switzerland, best quality the device can take | Uses the "Whole Switzerland" preset, app splits into device-legal map sets automatically |
| U4 | Power user | Wants to tune what goes into the map | Opens the layer panel, disables buildings, sets contour interval to 20 m, saves the recipe as a preset |
| U5 | Contributor | Wants to improve the cartography | Edits the mkgmap style rules and TYP source in the repo, rebuilds, sees the change |

**Assumed user skill level:** comfortable plugging in a USB device and copying a file.
Not assumed: any knowledge of GIS, projections, Garmin internals, or the command line.

---

## 3. Data sources

All sources are swisstopo Open Government Data, free for any purpose, requiring source
attribution (§15). All are served from `data.geo.admin.ch` without registration or API keys.

### 3.1 Primary: swissTLM3D — the topographic landscape model

- **STAC collection:** `ch.swisstopo.swisstlm3d`
- **API:** `https://data.geo.admin.ch/api/stac/v1/collections/ch.swisstopo.swisstlm3d/items`
- **Releases:** one item per annual release. Verified present: `swisstlm3d_2020-03` …
  `swisstlm3d_2026-02`. The app must resolve "latest" dynamically, never hard-code a release.
- **Assets per release:** `.gpkg.zip` (preferred), `.gdb.zip`, `.shp.zip`, `.xtf.zip` (INTERLIS).
  Note that GeoPackage only appears from `swisstlm3d_2023-03` onward.
- **Size:** the 2024-03 GeoPackage archive is **4,576,290,652 bytes (~4.6 GB)**; expect
  roughly 12–20 GB unpacked.
- **CRS:** EPSG:2056 (CH1903+ / LV95), heights LN02 (EPSG:5728).
- **Critical property: the dataset is NOT tiled.** It is a single national file. This is the
  defining constraint on the whole application design — see §4.1.
- **Content:** eight topics — Roads and tracks, Public transport, Buildings, Areas,
  Land cover, Hydrography, Single point objects, Names.

> **Implementation note.** The exact GeoPackage layer names and attribute domains are *not*
> reliably documented in public sources. Prior art suggests names of the form
> `tlm_strassen_strasse`, `tlm_bb_bodenbedeckung`, `tlm_gewaesser_*`. **These must be treated
> as unverified.** Milestone 1 of the plan includes an explicit schema-discovery task that
> dumps the real layer names, geometry types, feature counts, and attribute value domains
> from the downloaded GeoPackage into a committed `docs/tlm3d-schema.md`. All style rules
> are written against that discovered schema, not against assumptions.

### 3.2 Elevation: swissALTI3D — for contour lines and optional DEM

- **STAC collection:** `ch.swisstopo.swissalti3d`
- **Tiling:** 1 km × 1 km tiles, named `swissalti3d_<year>_<E>-<N>` where `E`/`N` are LV95
  coordinates in km (e.g. `swissalti3d_2019_2485-1109`).
- **Assets per tile:** GeoTIFF at 0.5 m and 2 m resolution, plus ASCII XYZ.
- **Verified:** the 2 m GeoTIFF is a **cloud-optimized GeoTIFF**, 500 × 500 px, Float32,
  EPSG:2056. Confirmed readable over HTTP range requests without downloading the full file.
- **Consequence:** contours can be generated **on demand per selected region** by streaming
  only the tiles that intersect the area of interest. No national elevation download is
  needed. For a typical canton this is a few hundred to a few thousand tiles.
- **Resolution choice:** always use the **2 m** asset. 0.5 m is 16× the data for no benefit
  at Garmin's ~2 m coordinate resolution.

### 3.3 Supporting datasets

| Dataset | STAC collection | Size | Use |
|---|---|---|---|
| swissTLM3D Hiking trails | `ch.swisstopo.swisstlm3d-wanderwege` | 198 MB (GPKG) | Official hiking network with trail difficulty classes (yellow / red-white / blue-white). Small, high-value, ships as a single national file. |
| swissTLMRegio | `ch.swisstopo.swisstlmregio` | 163 MB (GPKG) | Generalized 1:200 000 model. Used to populate the **low zoom levels** of the map so zoomed-out views stay fast and legible instead of decimating TLM3D. |
| swissNAMES3D | `ch.swisstopo.swissnames3d` | — | 400 000+ geographic names, if TLM3D's own name topic proves insufficient for search and labels. |
| swissBOUNDARIES3D | `ch.swisstopo.swissboundaries3d` | — | Canton / district / commune polygons, used by the administrative-unit area picker (§6.4.2). |

### 3.4 Basemap for the in-app map picker

- **swisstopo WMTS:** `https://wmts.geo.admin.ch/1.0.0/{layer}/default/current/3857/{z}/{x}/{y}.{fmt}`
- Layers: `ch.swisstopo.pixelkarte-farbe` (national map colour),
  `ch.swisstopo.swisstlm3d-karte-farbe`, `ch.swisstopo.swissimage`.
- Used for display only, in Web Mercator (EPSG:3857), so it drops straight into MapLibre GL.
- Tiles are cached locally with a bounded LRU so the picker works offline after first use.

### 3.5 Source integrity requirements

- **FR-D1** Every download must be verified. Use the `file:checksum` / multihash fields from
  the STAC asset metadata where present; otherwise verify `Content-Length` and store the
  server's `Last-Modified` and `ETag`.
- **FR-D2** All downloads must be resumable via HTTP `Range`. A 4.6 GB download over a
  domestic connection will be interrupted; the app must never restart from zero.
- **FR-D3** Downloads land in a temp file and are atomically renamed on successful verification.
  A partially written cache entry must never be usable.
- **FR-D4** The app records, per cache entry, the STAC item id, asset name, release date,
  checksum, and fetch timestamp, so a build is fully reproducible from its recipe.

---

## 4. The central design constraint

### 4.1 National-only vector source vs. regional output

swissTLM3D is a 4.6 GB national download; the user wants a 40 MB map of Valais. The app
therefore has a **two-phase data model** that must be surfaced honestly in the UI:

1. **Acquire (once, ~4.6 GB, slow).** Download the national swissTLM3D GeoPackage. This
   happens once per annual release, with a clear one-time-cost explanation, a progress
   estimate, and the ability to pause and resume across app restarts.
2. **Build (per map, fast).** Clip the cached national dataset to the selected area, convert,
   compile. Target: a canton-sized map in under 10 minutes on a 2023 laptop.

**FR-C1** The first run must not present the 4.6 GB download as a surprise. It is an explicit,
explained, cancellable step with a disk-space precheck.

**FR-C2** Once acquired, all subsequent builds must work **fully offline** except for
swissALTI3D contour tiles, which are fetched per region. The app must offer to pre-cache
elevation tiles for a chosen area so that even that step can be made offline.

**FR-C3** The app must never require the user to download the national dataset in order to
just *look around* the area picker (which uses WMTS, §3.4) or to see size estimates.

**FR-C4** The data location must be **choosable before the first download**, not after.
swissTLM3D alone inflates to 10.0 GB, plus elevation tiles and build working files, so a
machine with a small internal disk needs to point this at an external volume up front.
The chosen directory is shown with its free space and the space already used, is checked
for writability when chosen rather than four gigabytes into a transfer, and the app states
plainly that changing it copies nothing — datasets already downloaded stay where they are.
`S2G_CACHE` overrides the setting, and the UI says so instead of appearing not to work.

**FR-C5** Two locations, one setting: the build working directory lives under the same
root as the dataset cache. A build's intermediates are as large as the datasets they come
from, so splitting them across volumes would defeat the purpose of choosing one.

### 4.2 Optimization: build a national intermediate once

> **Status after Milestone 0: CONDITIONAL — probably not needed.** swissTLM3D ships with
> GeoPackage R-tree indexes on all 32 spatial layers, and a 144 km² clip measured **2.1 s**,
> extrapolating to ~10 min for all of Switzerland. Build the tilestore only if canton-scale
> measurement contradicts this. See docs/m0-findings.md §3.1.

The concern was that clipping a 10.8 GB GeoPackage per build would be too expensive, so the
pipeline would convert the national GPKG **once** into a compact, spatially indexed
intermediate after acquisition. Measurement shows the source R-trees already provide this,
so the intermediate is retained here only as a fallback. The one capability it also offered —
per-cell feature counts for the size estimator (FR-61) — is instead obtained from cheap
R-tree range counts.

---

## 5. Output format and target devices

### 5.1 Output format

Garmin proprietary vector map images:

- **`gmapsupp.img`** — the single-file "supplementary map" understood by every device in
  scope. This is the primary deliverable.
- **`.gmap` directory bundle** — optional secondary output for Garmin BaseCamp / MapInstall
  on the desktop, useful for previewing before flashing a device.
- Each build gets a **unique Family ID (FID) and Product ID** so it never collides with
  Garmin's own preinstalled maps or with another build from this app. FIDs are allocated
  deterministically from a hash of the build recipe within a reserved range, and recorded
  in the build manifest.

### 5.2 Target devices

The device list must cover, at minimum:

- **Edge:** 130/130 Plus, 520 Plus, 530, 540, 830, 840, 1030, 1030 Plus, 1040, 1050,
  Explore, Explore 2, Touring.
- **fēnix:** 5 Plus, 6 series, 7 series, 8 series, and the epix / epix Pro line.
- **Related wearables** that use the same map stack: Forerunner 945/955/965, MARQ,
  Enduro, tactix, Instinct with mapping.

### 5.3 Device profile schema

Device capabilities are **data, not code** — a versioned JSON file shipped with the app and
updatable independently of a release.

```jsonc
// devices/edge-840.json
{
  "id": "edge-840",
  "displayName": "Garmin Edge 840 / 840 Solar",
  "family": "edge",
  "generation": 2023,
  "storage": {
    "internalBytes": 32000000000,
    "recommendedMapBudgetBytes": 4000000000,
    "hasRemovableStorage": false
  },
  "mapFile": {
    "maxImgBytes": 4294967295,        // FAT32 single-file ceiling
    "maxTilesPerMapset": 4096,        // COMMUNITY-DERIVED, see confidence
    "installPaths": ["/Garmin/", "/Garmin/Maps/"],
    "requiresExactFilename": false,    // true => must be literally gmapsupp.img
    "supportsMultipleMapsets": true
  },
  "rendering": {
    "supportsTypFile": true,
    "supportsRoutableMaps": true,      // relevant from v2 onward
    "displaysStreetNames": true,
    "screenClass": "handlebar",        // handlebar | wrist
    "recommendedMaxDetailLevel": "full"
  },
  "confidence": {
    "level": "community",              // vendor | measured | community | assumed
    "sources": [
      "https://www.dcrainmaker.com/2019/08/how-to-install-free-maps-on-your-garmin-edge.html",
      "https://community.openstreetmap.org/t/splitting-img-files-into-multiple-smaller-files/95508"
    ],
    "lastVerified": "2026-09-06",
    "notes": "maxTilesPerMapset is inferred from Edge 530 reports; not vendor-confirmed."
  }
}
```

### 5.4 Honesty requirement about device limits

Garmin does not publish `.img` size limits, tile-count limits, or install-path rules. The
values circulating in forums are inconsistent and often stale — e.g. the widely repeated
"fēnix maps must be under 20 MB" originates from fēnix 3-era hardware and does not apply to
fēnix 7/8, which ship multi-gigabyte TopoActive maps from the factory.

**FR-DEV1** Every numeric limit carries a `confidence` level and source list. The UI must
visually distinguish `vendor`/`measured` from `community`/`assumed`.

**FR-DEV2** For any profile below `measured` confidence, the app applies a **conservative
safety margin** (default 20 % below the stated budget) and says so in the build summary.

**FR-DEV3** Every limit is user-overridable, with the override stored per device instance
and never silently reset by an app update.

**FR-DEV4** The repo carries `docs/device-verification.md`: a reproducible checklist for
measuring the real limits of a device (binary search on map size, tile count, filename
variants, install paths), and profiles get promoted to `measured` only via that procedure.
Community-contributed measurements are accepted as pull requests against the profile files.

**FR-DEV5** Device auto-detection: when a Garmin USB mass-storage volume is mounted, read
`/Garmin/GarminDevice.xml` to identify the model and preselect the matching profile. Never
write to the device without explicit user confirmation.

---

## 6. Functional requirements

### 6.1 Application shell

- **FR-1** Single-window application with a linear primary flow —
  **Device → Area → Content → Build → Install** — presented as a step indicator that also
  allows jumping back to any completed step.
- **FR-2** Any long-running operation (download, convert, compile) runs in the background
  with per-stage progress, a live log pane, elapsed/remaining estimates, and a Cancel that
  actually terminates child processes and cleans up partial output.
- **FR-3** The app is fully usable by keyboard, respects OS light/dark mode, and honours
  reduced-motion and OS text-scaling settings.
- **FR-4** UI languages: German, French, Italian, English. English is the fallback. All
  strings live in resource files; no string literals in components.
- **FR-5** Crash and error reports are local-only by default. No telemetry without an
  explicit, off-by-default opt-in.

### 6.2 Data management

- **FR-10** A "Data" screen listing every cached dataset: name, release, size on disk,
  fetch date, and whether a newer release exists upstream.
- **FR-11** Per-dataset actions: download, resume, verify, delete, and "switch to release X"
  (older releases stay usable so a past build can be reproduced).
- **FR-12** A configurable cache location, defaulting to the OS app-data directory, with a
  clear total-size readout and a disk-space precheck before any download.
- **FR-13** On startup, check the STAC API for newer releases (fast, cacheable, with a
  timeout) and show a non-modal notice. Never auto-download.

### 6.3 Device selection (step 1)

- **FR-20** Searchable device list grouped by family, with the auto-detected device
  (FR-DEV5) surfaced at the top.
- **FR-21** A "Generic / other device" option with manual limit entry, so an unlisted or
  brand-new model is never a dead end.
- **FR-22** The selected device's budget, limits, and confidence level are shown inline and
  carried into the size estimator (§6.6).

### 6.4 Area selection (step 2)

All four modes are in v1 scope, and they **compose**: the selection is a set of geometries
that are unioned, and modes can be mixed (e.g. two cantons plus a route corridor).

#### 6.4.1 Draw on a swisstopo map
- **FR-30** An embedded MapLibre GL map with swisstopo WMTS basemap layers (§3.4), a layer
  switcher, scale bar, and coordinate readout in both WGS84 and LV95.
- **FR-31** Tools: rectangle, freehand polygon, and circle-with-radius. Existing shapes can
  be edited, moved, and deleted.
- **FR-32** Live readout as the shape changes: area in km², and the estimated output size
  against the device budget, shown as a progress-style budget bar.

#### 6.4.2 Administrative units
- **FR-33** Multi-select cantons, districts, and communes by name (searchable, with
  DE/FR/IT names), sourced from swissBOUNDARIES3D. Selected units highlight on the map.
- **FR-34** Selection is unioned and optionally buffered by a user-set distance so that
  border areas are not cut off mid-trail.

#### 6.4.3 Whole Switzerland preset
- **FR-35** A single action selecting the full swissTLM3D coverage area (Switzerland +
  Liechtenstein, plus the border overlap the dataset includes).
- **FR-36** If the result exceeds the device's `maxImgBytes` or `maxTilesPerMapset`, the app
  **automatically partitions** into multiple device-legal map sets, each with a unique
  family ID, along a sensible geographic split (default: by canton groups / language region),
  and clearly explains the resulting set of files and where each goes.
  *(Milestone 0 measurement extrapolates a national build to ~336 MB, far below the 4 GB
  single-file ceiling, so this is a safety net rather than the common path. Keep it, but do
  not let it drive the architecture.)*
- **FR-37** The national build must be resumable at map-set granularity: if set 3 of 5 fails,
  sets 1–2 are kept.

#### 6.4.4 GPX / FIT corridor
- **FR-38** Import `.gpx` (tracks and routes) and `.fit` (courses). Multiple files at once.
- **FR-39** Buffer the imported geometry by a user-set corridor width (default 5 km,
  range 0.5–50 km) and use the buffer as the area.
- **FR-40** Show the route on the map with its length, elevation gain, and the resulting
  corridor area and size estimate.

#### 6.4.5 Common
- **FR-41** Named, saveable selections, reusable across builds.
- **FR-42** Import/export selections as GeoJSON.

### 6.5 Content configuration (step 3)

- **FR-50** **Presets** are the primary control. Each selects a *content set*, a
  *cartography variant* and *contour defaults*; the layer panel (FR-51) then refines them.
  The preset list is fixed — it is not a free-form layer builder, because the value of a
  preset is that it is already correct for a purpose.

  | Preset | Content beyond the base topo | Contours | Cartography |
  |---|---|---|---|
  | **Hiking** *(default)* | hiking network by Swiss trail class (Wanderweg / Bergwanderweg / Alpinwanderweg), lifts and cableways | 20 m, index 100 m | per device class |
  | **Cycling** | cycle and mountain-bike route networks, road hierarchy emphasised, buildings simplified | 20 m, index 100 m | handlebar |
  | **Ski touring (skimo)** | SAC ski routes split into **skiable / carrying / caution**, snowshoe trails, winter hiking trails, lifts and cableways | 20 m, index 100 m | per device class |
  | **Full topo** | every available layer | 10 m, index 50 m | handlebar |

  Notes that matter for the UI:
  - **Base topo is always present** in every preset: land cover, hydrography, buildings,
    place names, contours and relief. A preset adds a themed network on top; it never
    strips the map down to just that network.
  - **Lifts and cableways** (`tlm_oev_uebrige_bahn`) appear in Hiking, Ski touring and
    Full topo. They are mountain context, not a winter-only feature.
  - The **Ski touring** preset requires the three winter datasets (~38 MB, all
    GeoPackage) and the **Cycling** preset requires the ASTRA route networks
    (~110 MB, shapefile). Both are separate downloads from swissTLM3D, so a preset that
    needs data the user does not have must offer to fetch it rather than silently
    producing an empty theme (see FR-13).
  - Presets are **not** device-locked. The earlier draft tied Cycling to Edge and Hiking
    to fēnix; that was wrong — a fēnix is used for ski touring and an Edge for
    bikepacking on hiking trails. The device chooses the *cartography variant*
    (FR-CART6), the preset chooses the *content*.

- **FR-51** An expandable **layer panel** for per-layer control: include/exclude. Layers
  are grouped so the panel stays legible. The groups are the *source layers* a build
  extracts, because that is the granularity at which a layer can actually be left out:

  | Group | Source layers |
  |---|---|
  | Land cover | ground cover, land use areas, recreation areas |
  | Water | rivers and streams, lakes and ponds |
  | Transport | roads/tracks/paths, railways, **lifts and cableways**, transport areas |
  | Built | buildings, individual objects |
  | Names | settlement names, local and field names |
  | **Winter** | SAC ski network (skiable / carrying / caution), SAC named tours, snowshoe trails, winter hiking trails |
  | **Cycling** | cycle paths, mountain-bike trails, signposted routes |

  There is deliberately **no Hiking group and no Terrain group**. Swiss trail classes are
  an *attribute* of the road layer (`tlm:wanderwege`), so a hiking toggle would either do
  nothing or take the roads with it; the panel says so instead of offering one. Rock,
  scree and glacier are attribute values of ground cover, and contours and relief have
  their own controls (FR-52, FR-CART3), so neither is a layer to switch.

  A minimum zoom level per layer is **not** implemented: level assignment lives in the
  mkgmap style, where it is expressed per feature class rather than per source layer.

  A group whose source data is not downloaded is shown **disabled with the reason**,
  never hidden — a user looking for ski routes must be able to see that the feature
  exists and what it needs.
- **FR-52** Contour settings: interval (5/10/20/50/100 m), whether to label, and which
  intervals count as minor/medium/major.
- **FR-53** Label language: German, French, Italian, or "local" (whatever the source
  feature carries — the correct default for Switzerland).
- **FR-54** A live size estimate that updates as content options change.
- **FR-55** Save the full configuration as a named **recipe** (§11.2) and load it later.

### 6.6 Size estimation

- **FR-60** Before building, estimate output size within **±25 %** for areas above 100 km².
- **FR-61** The estimator is a calibrated model, not a guess: feature counts per layer are
  read from the spatial index for the selected area, multiplied by per-layer bytes-per-feature
  coefficients that were **fitted from real builds** and shipped as a data file.
- **FR-62** After every successful build the app records (feature counts → actual size) into
  a local calibration log and refines its coefficients, improving accuracy with use.
- **FR-63** The UI shows estimate, device budget, and safety margin together, and blocks
  (with an override) any build predicted to exceed a hard device limit.

### 6.7 Build execution (step 4)

- **FR-70** Build stages are explicit and individually reported: *clip → convert → contours →
  split → compile → assemble → verify*. See §7.
- **FR-70a** Progress is weighted by how long each stage usually takes, not by stage
  count. Counting stages makes the bar lurch: on a warm elevation cache the contour stage
  alone is over 80 % of a build. Weights are measured from real builds and refitted from
  the user's own builds as the calibration log fills (see FR-62).
- **FR-70b** A **time remaining** figure is shown alongside elapsed time, derived by
  extrapolating from elapsed time and the weighted fraction complete, so it converges
  even when the shipped weights are wrong for this machine. No figure is shown at all
  until the extrapolation would mean something — a "four hours remaining" flashed in the
  first second is worse than no number — and the actual duration is reported on
  completion, which is what makes the next estimate credible.
- **FR-70c** **Cancellation must stop the work, not just the reporting.** The build spawns
  `splitter` and `mkgmap` as child processes; a cancelled build kills them rather than
  waiting for them to finish. Both are minutes long on a large area, so waiting means the
  UI says "cancelled" while the machine keeps working.
- **FR-71** Every build produces a **manifest** (§11.3) recording input dataset releases and
  checksums, the recipe, tool versions, timestamps, and output file hashes. A build must be
  bit-reproducible given the same manifest, up to timestamps.
- **FR-72** Stage-level caching: re-running a build after changing only the TYP file must
  not re-clip or re-convert. Cache keys derive from the recipe subtree each stage depends on.
- **FR-73** On failure, show the failing stage, the relevant tail of the tool's stderr, a
  plain-language interpretation of the most common failure modes, and a one-click
  "copy diagnostics" for bug reports.
- **FR-74** Builds run at below-normal process priority by default, with a configurable
  worker/thread count, so the machine stays usable.

### 6.8 Install and export (step 5)

- **FR-80** If a Garmin device is mounted: show its identity, free space, and existing maps;
  offer to copy the built map to the correct path for that profile, with the correct filename.
- **FR-81** Never overwrite an existing map file without explicit confirmation, and offer to
  back up any file being replaced.
- **FR-82** Verify the copy by re-hashing the file on the device, and warn that the device
  must be safely ejected.
- **FR-83** Always offer plain "Save to folder" export as an alternative, plus
  per-device textual install instructions (which folder, which filename, whether an SD card
  is an option).
- **FR-84** Detect and warn about the classic `gmapsupp.img.img` double-extension mistake
  when writing to a device from a system with hidden file extensions.

---

## 7. Conversion pipeline

The pipeline is the technical core. Stages are pure functions over files with explicit
inputs and outputs, so each is independently testable and cacheable.

```
                     ┌─────────────────────────────────────────┐
  swisstopo STAC ───► │ 0  ACQUIRE   national GPKG (~4.6 GB)   │  once per release
                     └──────────────────┬──────────────────────┘
                                        ▼
                     ┌─────────────────────────────────────────┐
                     │ 1  INDEX     national → tiled store     │  once per release
                     └──────────────────┬──────────────────────┘
                                        ▼
  area selection ──► ┌─────────────────────────────────────────┐
                     │ 2  CLIP      area of interest features  │
                     └──────────────────┬──────────────────────┘
                                        ▼
                     ┌─────────────────────────────────────────┐
  swissALTI3D COGs ► │ 3  CONVERT   TLM features  → OSM PBF    │
     (streamed)      │    CONTOUR   DEM → contour ways → PBF   │
                     └──────────────────┬──────────────────────┘
                                        ▼
                     ┌─────────────────────────────────────────┐
                     │ 4  SPLIT     splitter.jar → map tiles   │
                     └──────────────────┬──────────────────────┘
                                        ▼
  style + TYP ─────► ┌─────────────────────────────────────────┐
                     │ 5  COMPILE   mkgmap.jar → gmapsupp.img  │
                     └──────────────────┬──────────────────────┘
                                        ▼
                     ┌─────────────────────────────────────────┐
                     │ 6  VERIFY    structure, size, limits    │
                     └─────────────────────────────────────────┘
```

### 7.1 Stage 0 — Acquire

Resolve the latest (or user-pinned) STAC item, pick the `.gpkg.zip` asset, download with
resume and checksum verification (§3.5), unpack to the cache.

### 7.2 Stage 1 — Index

Read the national GeoPackage and write a compact tiled intermediate store keyed by a
1 km LV95 grid — the same grid swissALTI3D uses, which makes elevation and vector tiles
align for free.

- GeoPackage is SQLite; geometry columns hold a GPKG binary header (`GP` magic, flags,
  optional envelope) followed by standard WKB. It can be read directly with SQLite + a WKB
  parser, **with no GDAL dependency**.
- Rationale: this stage exists so the 4.6 GB scan happens once rather than per build (§4.2).
- The store must record, per grid cell and per layer, the feature count — this is what feeds
  the size estimator (FR-61) without any I/O over feature geometry.

### 7.3 Stage 2 — Clip

Select grid cells intersecting the area of interest, then clip geometries precisely to the
area boundary. Lines and polygons crossing the boundary are cut, not dropped. Polygons are
closed after clipping. Features are deduplicated across cells.

### 7.4 Stage 3 — Convert

**Vector.** Map each TLM3D feature to an OSM-model element with tags carrying the TLM
attributes, and write OSM PBF (mkgmap accepts `.osm`, `.o5m`, `.osm.pbf`; PBF is the fastest
and smallest). Requirements:

- **FR-P1** Reproject EPSG:2056 → EPSG:4326 using swisstopo's published approximate
  formulas. **Measured against PROJ** (`crates/s2g-core/tests/proj.rs`): about **1 m in the
  interior**, degrading to **4.2 m at the corners of the coverage rectangle**, which lie
  tens of kilometres outside Swiss territory where no swissTLM3D data exists. Garmin's
  `.img` grid is ~2.4 m, so interior error is not representable in the output; the corner
  case exceeds it but is both invisible at map scale and far smaller than GPS error.
  *(An earlier draft of this requirement claimed sub-metre accuracy everywhere. That was
  wrong and is corrected here.)* If survey-grade accuracy is ever needed, the rigorous
  Hotine Oblique Mercator inverse plus the CHENyx06 grid shift is the upgrade path.
  Note that swisstopo's stated coordinates for the projection origin are in the
  **Bessel/CH1903 datum**, not WGS84 — treating them as WGS84 truth makes a correct
  implementation look 164 m wrong.
- **FR-P2** Discard the Z coordinate for map geometry (TLM3D is 3D; Garmin maps are 2D).
  Retain Z only where it is genuinely useful, e.g. as a peak elevation tag.
- **FR-P3** Deduplicate coordinates into shared nodes so that connected features share
  geometry — required for clean rendering, and prerequisite groundwork for v2 routing.
- **FR-P4** Assign stable synthetic OSM ids, deterministic from the TLM feature UUID, so
  rebuilds are reproducible and diffable. **Elements must be written in ascending id order**:
  `splitter` rejects unsorted input with *"Node ids are not sorted"*, so deterministic ids
  must be sorted before splitting.
- **FR-P5** Preserve multilingual names. Emit `name` from the local-language attribute and
  `name:de` / `name:fr` / `name:it` where the source provides them.

**Contours.** For each 1 km cell in the area, stream the swissALTI3D 2 m COG over HTTP range
requests, mosaic, optionally smooth, and run marching squares at the configured interval.

- **FR-P13** swissALTI3D publishes one STAC item per acquisition campaign, so the **same
  1 km cell is returned for several years** (measured: every cell in the Grindelwald bbox
  exists for both 2019 and 2022). Deduplicate by cell, keeping the most recent year.
  Failing to do so doubles bytes read and makes elevation depend on mosaic ordering.
- **FR-P14** Construct the mosaic descriptor **from the tile grid, never by probing the
  tiles**. Cell `EEEE-NNNN` has origin `(EEEE*1000, (NNNN+1)*1000)`, 500 × 500 px at 2 m,
  Float32, nodata −9999. Probing each remote tile for its geotransform is prohibitively
  slow (minutes for 362 tiles vs. 0.00 s when derived).
- **FR-P15** Fetch elevation tiles **concurrently** (8–16 in flight) and cache them locally.
  Contour generation is network-bound and is the dominant cost of a build (NFR-1).

- **FR-P6** Emit contours using the tagging convention that mkgmap styles already expect:
  `contour=elevation`, `ele=<metres>`, and
  `contour_ext=elevation_minor|elevation_medium|elevation_major`. This keeps our contours
  compatible with the wider mkgmap style ecosystem.
- **FR-P7** Contour lines must be continuous across tile boundaries. Mosaic with a one-pixel
  overlap and stitch, rather than contouring tiles independently.
- **FR-P8** Simplify contour geometry (Douglas–Peucker at a tolerance below Garmin's
  coordinate resolution) — contours are the single largest contributor to output size and
  naive output will blow the device budget.
- **FR-P9** Elevations are LN02 in the source; note the datum in the manifest. Do not
  silently mix LN02 and LHN95.

### 7.5 Stage 4 — Split

Invoke `splitter.jar` to partition the PBF into Garmin map tiles.

- `--max-nodes` is tuned per device class (smaller tiles for wrist devices).
- Tile count must be checked against the device's `maxTilesPerMapset` before compiling; if
  exceeded, either raise `--max-nodes` or partition into multiple map sets (FR-36).

### 7.6 Stage 5 — Compile

Invoke `mkgmap.jar` with the project's style directory and compiled TYP file, producing
`gmapsupp.img` (and optionally the `.gmap` bundle).

- Key options: `--gmapsupp`, `--family-id`, `--product-id`, `--family-name`, `--series-name`,
  `--overview-mapname`, `--style-file`, `--levels`, `--code-page`, `--index`,
  `--draw-priority`. v1 does **not** pass `--route`.
- **FR-P10** Character encoding must be chosen so that Swiss names render correctly on
  device — umlauts and accented French names are a first-class correctness requirement, not
  a nicety. The chosen code page must be validated on real hardware.
- **FR-P11** mkgmap and splitter are invoked as **child processes**, never embedded. This
  keeps their GPLv2 licensing cleanly separated and lets users substitute their own JAR
  version.

### 7.7 Stage 6 — Verify

- Output parses as a valid Garmin IMG container; expected subfiles present.
- Size is within the device's hard limits and stated budget.
- Tile count within limits; family/product IDs as intended.
- Emit a human-readable build report and the manifest (§11.3).

### 7.8 Java runtime

- A trimmed JRE (Java 17 LTS via `jlink`, ~40–50 MB per platform) is bundled so the user
  never installs Java. `mkgmap` requires Java 8 or newer.
- **FR-P12** If a suitable system Java exists, prefer it and skip the bundled runtime;
  the bundled JRE is a fallback, and the choice is visible in settings.

---

## 8. Cartography specification

This is what determines whether the product is good. The pipeline is plumbing; the
cartography is the value.

### 8.1 Design intent

**Primary requirement: the map must look as close as possible to the swisstopo raster
national maps** (`ch.swisstopo.pixelkarte-farbe` / Landeskarte 1:25 000). This is a
headline product requirement, not a nice-to-have: it is the main reason a Swiss user would
choose this over Garmin's own maps.

**What "exactly" can and cannot mean.** Garmin vector maps cannot reproduce the raster
Landeskarte pixel-for-pixel, and the spec should not pretend otherwise. The TYP rendering
model allows solid fills, 32x32 bitmap pattern fills, and lines with a width plus a casing
colour. It does not allow label halos, curved or rotated text, arbitrary dash arrays,
custom fonts, or free-form relief shading — those are fixed by the device firmware.
swisstopo's signature rock drawing (*Felszeichnung*) is a hand-crafted raster product that
has no vector equivalent.

Using the raster maps directly was assumed not to be an option, on the basis that Edge
and fēnix devices support neither BirdsEye `.jnx` nor Custom Map `.kmz` (§1.4).
**That assumption is now in doubt:** the Edge 840's `GarminDevice.xml` advertises both
`Garmin/CustomMaps` and `Garmin/BirdsEye` directories (docs/m0-findings.md §4.16). The
directory existing does not prove KMZ overlays render or are usable at the required
zoom levels, but it should be tested rather than assumed away.

So the requirement is operationalised as: **match swisstopo's palette exactly, match its
symbol language as closely as the TYP model allows, and use every device capability that
moves toward the raster look** — above all DEM relief shading (FR-CART8).

- **FR-CART9** Colours are **sampled from swisstopo's own raster products**, not chosen by
  eye. See `docs/palette.md`. Measured reference values include forest `#CAECC1`,
  water `#D3EEFF`, glacier `#CCD3D3`, rock hachure ink `#706E6C` on a white ground.
- **FR-CART10** Where the Landeskarte uses a texture rather than a flat tint (rock
  hachures, wetland ticks, vineyard rows, scree stipple), use TYP XPM pattern fills rather
  than a solid colour approximation.
- **FR-CART11** A **winter colour scheme**, selectable independently of the content
  preset. swisstopo publishes a Winter national map
  (`ch.swisstopo.pixelkarte-farbe-winter`) alongside the summer one, so the winter
  scheme is *measured* from it like every other colour (FR-CART9) rather than invented:
  the per-channel shift from summer to winter is measured over the same tiles in both
  sheets and applied to the summer palette. Measured shifts, as (R, G, B):
  forest `(-7, +7, +45)`, glacier `(-7, +15, +29)`, open land `(-7, +10, +36)`,
  built-up `(-7, +7, +32)`, rock ink `(-16, -5, -2)`, water unchanged. Overlay colours —
  routes, ink, buildings, rail, the road hierarchy and contours — are printed on the
  sheet at full strength and are **not** shifted; the point of the winter sheet is that
  the base map steps back so the routes on it read.

  The scheme is **not** tied to the ski touring preset. A winter sheet is useful for a
  snowshoe outing on a hiking map, and a ski tourer may prefer the summer sheet; the
  preset chooses content, the scheme chooses colour. It changes the TYP only, since the
  geometry rules are identical, and it is part of the map's identity so a winter map does
  not overwrite the summer one on the device. See `tools/winter_palette.py`.

  *A single per-channel linear fit of the whole winter sheet against the summer one was
  tried first and rejected: R² reached only 0.71 on blue and it turned the bistre
  contours pink. The winter sheet recolours selectively, so it has to be measured
  selectively.*

- **FR-CART12** **Slope classes over 30°**, optional and off by default. swisstopo
  publishes `ch.swisstopo.hangneigung-ueber_30` as a rendered WMTS layer only, with no
  dataset behind it, so the classes are *computed* from the same swissALTI3D data a build
  already fetches for contours, and only the colours come from swisstopo (FR-CART9).
  Classes are 30–35°, 35–40°, 40–45°, 45–50° and over 50°.

  Two decisions the implementation rests on:
  - **Computed over a 10 m baseline, not the grid's native 2 m.** At 2 m the result is
    dominated by boulders, road cuttings and canopy artefacts: a single wild sample reads
    as an 80°+ cliff. Ski-touring practice and swisstopo's own product work at about
    10 m, where the number means what a skier reads it to mean.
  - **Drawn as a hatch over a transparent ground**, because a Garmin TYP polygon fill has
    no alpha and a solid fill would bury the map the classes are meant to inform.

  Cost, measured from three paired builds in alpine terrain: about **3,568 bytes per
  km²**, which the size estimate accounts for.

- **FR-CART13** Named **SAC huts** from `ch.swisstopo.unterkuenfte-winter` (506 of them).
  swissTLM3D marks hut *buildings* (`nutzung=Schutzhuette`) but carries no name for them,
  so a hut was an unlabelled rectangle. Contact details and opening hours are **not**
  available in any free dataset — the SAC portal holds them, and this dataset only links
  to it — so the map carries the name and position, not the phone number.

### 8.2 Deliverables

1. **`style/` — an mkgmap style directory** containing `points`, `lines`, `polygons`,
   `relations`, `options`, and `version`. Rules map the tags emitted in Stage 3 to Garmin
   type codes with explicit `[0x… resolution N]` assignments.
   Labels require an **explicit `{ name '${tag}' }` action** in the rules: mkgmap's
   `--name-tag-list` does *not* populate labels from arbitrary source tags (verified in
   Milestone 0 — it changed the LBL subfile by zero bytes). A rule with actions and no
   `[type]` performs the action and falls through to later rules.
2. **`typ/swisstopo.typ.txt` — TYP source**, compiled by mkgmap itself into the binary TYP.
   Defines colour, line width, pattern, fill, and bitmap for every custom type used, in both
   day and night palettes.
3. **`docs/cartography.md`** — the mapping table from TLM3D class → Garmin type → visual
   appearance → zoom levels, maintained as the human-readable source of truth.

### 8.3 Requirements

- **FR-CART1** Define an explicit **zoom-level plan** (`--levels`) with per-layer minimum
  levels, so that a zoomed-out view shows only major features. Feed low zoom levels from
  **swissTLMRegio** (§3.3) rather than by decimating TLM3D — this is both faster and
  cartographically better.
- **FR-CART2** Two palettes: day and night, both defined in the TYP file.
- **FR-CART3** **Hiking trails are rendered by class.** The Swiss trail classification
  (Wanderweg / Bergwanderweg / Alpinwanderweg — yellow / red-white / blue-white) is the
  single most valuable thing this map can offer over Garmin's own, and must be visually
  distinct at all relevant zoom levels.
- **FR-CART4** Land cover follows Landeskarte conventions: forest, rock/scree, glacier and
  firn, water, wetland, orchard/vineyard, built-up.
- **FR-CART5** Contours: minor / medium / major weights, index contours labelled, colour
  differentiated over rock and glacier the way the Landeskarte does.
- **FR-CART6** **A separate, reduced style variant for wrist devices.** A fēnix screen is
  ~1.3 in with far less rendering headroom than an Edge 1050; the same style at the same
  density is unreadable and slow. Line weights, label density, and minimum zoom levels are
  tuned per `screenClass`.
- **FR-CART8** **Ship DEM data in the map** so devices render shaded relief. mkgmap's
  `--dem` accepts `.hgt` elevation tiles, which can be generated from swissALTI3D. Relief
  shading is the single largest visual step toward the raster Landeskarte look, and is
  supported by the Edge and fēnix devices in scope. `--dem-dists` must supply one
  resolution per entry in `--levels`.
- **FR-CART7** A visual regression harness: render a fixed set of reference extents to
  images and diff against committed golden images, so cartography changes are reviewable
  (§13.4).

### 8.4 Prior art to draw on

`martinzellner/swisstopo-garmin` already contains a first-cut mkgmap style mapping TLM3D
tags to Garmin types, and `wirhabenzeit/sac-skimo-garmin` contains a working TYP file for
Swiss ski routes. Both are worth reading before writing from scratch — but check each
project's license before copying code, and record any reuse in `NOTICE`.

---

## 9. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-1 | **Performance.** Canton-sized (≈2000 km²) *vector* build completes in ≤ 2 min. Canton contours complete in ≤ 10 min **with parallel tile fetch and a warm elevation cache**; a cold contour build may take longer and must show an honest up-front estimate. National build completes overnight unattended. *(Revised after Milestone 0: serial contour generation measured 0.74 s/km², i.e. ~25 min per canton — see docs/m0-findings.md §3.2.)* |
| NFR-2 | **Memory.** Peak RSS stays under 4 GB regardless of area size. All large-data stages stream; no stage loads the national dataset into memory. |
| NFR-3 | **Disk.** The app reports its total footprint and never exceeds the user's configured cache cap without asking. Temp files are cleaned on success, cancel, and crash-recovery. |
| NFR-4 | **Robustness.** Killing the app mid-build leaves no corrupt cache entries and no orphaned Java processes. |
| NFR-5 | **Offline.** After acquisition, building works with no network except contour tiles, which can be pre-cached. |
| NFR-6 | **Startup.** Cold start to interactive UI in under 2 s. |
| NFR-7 | **Installers.** Signed and notarized `.dmg` for macOS (arm64 + x64), signed `.msi`/`.exe` for Windows, `.AppImage` + `.deb` for Linux. |
| NFR-8 | **Privacy.** No account, no telemetry by default, no data leaves the machine except requests to `data.geo.admin.ch` and `wmts.geo.admin.ch`. |
| NFR-9 | **Accessibility.** Keyboard-navigable, screen-reader labels on all controls, WCAG AA contrast in both themes, respects OS text scaling. |
| NFR-10 | **Supply chain.** Dependencies pinned and audited; reproducible builds; SBOM published per release. |

---

## 10. Architecture

### 10.1 Stack

**Tauri 2** — React + TypeScript frontend, Rust backend.

Rationale: the heavy work is streaming multi-gigabyte geodata, which Rust does well and
which needs no GDAL dependency (GeoPackage is SQLite + WKB; swissALTI3D is a plain COG).
Tauri produces a small, signable, dependency-free installer per platform, which is the
main packaging risk in a project like this. The only bundled runtime is the trimmed JRE
required by mkgmap.

### 10.2 Module layout

> **Revised in Milestone 1.** The pipeline modules live in a separate `s2g-core`
> library crate rather than inside `src-tauri`, so they can be tested with plain
> `cargo test` without Tauri in the loop — which is what "the GUI is a thin shell over
> tested Rust" actually requires (PLAN.md working agreement, rule 5).

```
swisstopo2garmin/
├── SPEC.md                     this document
├── PLAN.md                     implementation plan
├── Cargo.toml                  workspace
├── frontend/
│   ├── src/
│   │   ├── steps/              Device, Area, Content, Build, Install
│   │   ├── map/                MapLibre GL + swisstopo WMTS, draw tools
│   │   ├── components/
│   │   ├── state/              api.ts + bindings.ts (generated from Rust)
│   │   └── i18n/               de, fr, it, en
├── crates/s2g-core/            all pipeline logic, GUI-free and unit tested
│   ├── src/
│   │   ├── http.rs             Http trait + reqwest impl (trait enables offline tests)
│   │   ├── stac.rs             catalog client, multihash checksums
│   │   ├── zip.rs              ZIP64 member location by range request
│   │   ├── download.rs         resumable download + streaming inflate
│   │   ├── cache.rs            dataset cache, provenance, quarantine, disk checks
│   │   ├── testing.rs          in-memory Http fake with fault injection
│   │   ├── gpkg.rs             SQLite + WKB reader                     (M2)
│   │   ├── proj.rs             LV95 <-> WGS84                          (M2)
│   │   ├── geom.rs             clip, buffer, simplify, union           (M2)
│   │   ├── contour.rs          COG reader, mosaic, marching squares    (M3)
│   │   ├── osmpbf.rs           OSM PBF writer                          (M2)
│   │   ├── garmin.rs           splitter/mkgmap invocation, IMG verify  (M4)
│   │   ├── devices.rs          profile loading, USB detection, install (M6)
│   │   ├── estimate.rs         size model + calibration                (M6)
│   │   └── pipeline.rs         stage orchestration, caching, cancel    (M2)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   └── ipc.rs              Tauri commands + typed events (ts-rs export)
├── style/                      mkgmap style directory
├── typ/                        TYP source (day + night, edge + wrist)
├── devices/                    device profile JSON
├── vendor/                     mkgmap.jar, splitter.jar, jlink JRE recipe
├── docs/
│   ├── tlm3d-schema.md         DISCOVERED schema (generated, committed)
│   ├── cartography.md          TLM class -> Garmin type mapping table
│   └── device-verification.md  procedure for measuring real device limits
└── tests/
    ├── fixtures/               small clipped GPKG + COG fixtures
    └── golden/                 reference renders + reference IMG hashes
```

### 10.3 Process and concurrency model

- The UI thread never blocks. All pipeline work runs on a Tokio runtime in the Rust backend.
- Progress and log lines stream to the frontend as typed Tauri events, throttled to a
  sensible refresh rate.
- Java tools run as tracked child processes in a process group so cancellation reliably
  kills the whole tree, including on Windows.
- One build at a time. Queueing multiple builds is a v2 feature.

### 10.4 IPC contract

Typed commands, with TypeScript types generated from the Rust definitions (e.g. `ts-rs`)
so the two sides cannot drift:

```
list_datasets() -> DatasetStatus[]
download_dataset(collection, release?) -> TaskId
list_devices() -> DeviceProfile[]
detect_devices() -> DetectedDevice[]
resolve_admin_units(query) -> AdminUnit[]
import_track(path) -> TrackGeometry
estimate_size(recipe) -> SizeEstimate
start_build(recipe) -> TaskId
cancel_task(TaskId) -> ()
install_map(buildId, deviceId) -> InstallResult
```

Events: `task:progress`, `task:log`, `task:stage`, `task:done`, `task:error`,
`device:attached`, `device:detached`.

---

## 11. Data schemas

### 11.1 Device profile
See §5.3.

### 11.2 Build recipe

The complete, serializable description of a build — the unit of save/load/share, and the
cache key source.

As implemented (`crates/s2g-core/src/recipe.rs`):

```jsonc
{
  "schemaVersion": 1,
  "name": "Grindelwald 8 km",
  "deviceId": "edge-840",           // selects the cartography variant and the budget
  "area": {                          // "bbox" or "place"
    "kind": "place",
    "name": "Grindelwald",
    "radiusKm": 8.0,
    "easting": 2645921.0,            // the resolved coordinate is stored, because
    "northing": 1163748.0            // re-resolving could pick a different settlement
  },
  "preset": "hiking",                // hiking | cycling | skimo | full
  "contours": { "intervalM": 20, "indexM": 100, "simplifyM": 8.0 },
  "relief": "gentle",                // off | gentle (3") | detailed (1")
  "excludedLayers": ["tlm_bauten_gebaeude_footprint"]
}
```

Differences from the original sketch above, all deliberate:

- **No `sources` block.** Releases are resolved at build time and recorded in the build
  manifest (§11.3) instead. Pinning them in the recipe would make a saved recipe fail
  once swisstopo publishes a new release.
- **No `cartography` block.** The device id selects the style and TYP variant
  (FR-CART6); there is one cartography per device class, not a free choice.
- **No `output` block.** Family id is derived from the recipe (see `cache_key`), so the
  same recipe keeps its identity on the device across rebuilds.
- **Layer control is a list of exclusions, not per-layer objects.** A recipe saved today
  therefore picks up layers added by a later release, rather than silently missing them.
- **Composite areas** (union of an admin unit and a track corridor) are specified in
  FR-33 and FR-38 but not yet implemented; `area` currently holds one selection.
- `labelLanguage` is not yet implemented (FR-53); labels carry the source feature's own
  name, which is the "local" option and the correct Swiss default.

### 11.3 Build manifest

Written next to every output. Records recipe, resolved source releases with checksums,
tool versions (`mkgmap`, `splitter`, app, JRE), stage timings, feature counts per layer,
output file names with hashes, and the attribution string embedded in the map.

---

## 12. Error handling

Every error must state **what failed, why, and what the user can do**. Specified cases:

| Situation | Required behaviour |
|---|---|
| Insufficient disk space | Precheck before download and before build; report required vs. available |
| Download interrupted | Auto-retry with backoff, then resume on next attempt; never restart from zero |
| Checksum mismatch | Discard, warn, offer retry; never use unverified data |
| STAC API unreachable | Fall back to cached catalog; state that release info may be stale |
| swissALTI3D tile missing | Skip with a warning; contours for that cell are absent, and the build report says which cells |
| Estimate exceeds device limit | Block with explanation; offer smaller area, coarser contours, fewer layers, or split into map sets |
| Tile count exceeds device limit | Auto-retune `--max-nodes`, else split into map sets |
| Java tool non-zero exit | Show stage, command line, stderr tail, and a plain-language cause where recognized |
| Device unplugged mid-copy | Abort, report incomplete file on device, offer retry |
| Target file already exists | Confirm, offer backup, never silently overwrite |
| Corrupt cache detected | Quarantine, offer re-download, never crash |
| App killed mid-build | On next start, detect and clean orphaned temp state; offer to resume |

---

## 13. Testing and validation

### 13.1 Unit tests
Projection round-trips against known LV95↔WGS84 reference points from swisstopo;
WKB parsing against handcrafted blobs; clipping edge cases (line exactly on boundary,
polygon fully containing the area, degenerate rings); marching squares against analytic
surfaces; PBF writer round-tripped through an independent reader.

### 13.2 Fixture-based integration tests
A small committed fixture (one commune-sized GPKG extract plus a handful of COG tiles) runs
the whole pipeline in CI in under a minute, asserting on feature counts, output size bands,
and IMG structure.

### 13.3 Golden-file tests
Reference recipes produce reference outputs. IMG hashes are compared where deterministic;
otherwise structural properties (subfile inventory, type-code histogram, tile count).

### 13.4 Visual regression
Render fixed extents from the built map through an offline Garmin-IMG renderer and diff
against committed golden PNGs, at both Edge and wrist style variants, in day and night
palettes. This is what makes cartography changes reviewable in a pull request.

### 13.5 Real-device validation — mandatory, non-negotiable

CI cannot tell you whether a map works on a fēnix. Before any release:

- **VAL-1** Build and install on at least one Edge and one fēnix/epix device.
- **VAL-2** Verify: map appears in the device's map list; renders at all zoom levels;
  labels show correct Swiss characters (umlauts, accents); trail classes are distinguishable;
  panning and zooming are responsive; battery drain is not pathological.
- **VAL-3** Verify coexistence with the device's factory map — no crashes, no blank map, no
  ID collision.
- **VAL-4** Verify the whole-Switzerland multi-map-set output on a real device.
- **VAL-5** Record results in `docs/device-verification.md` and promote profile confidence
  levels accordingly (FR-DEV4).

### 13.6 Performance tests
Timed and memory-profiled builds at three scales (commune, canton, national), tracked
across releases to catch regressions.

---

## 14. Distribution

- **License:** GPLv3 (or GPLv2-or-later). Chosen deliberately: bundling and invoking
  `mkgmap` and `splitter` (both GPLv2) is then unambiguously clean, and existing mkgmap
  style files and prior art can be reused.
- **Public repository** with issues, and device profiles accepted as community pull requests.
- **Releases:** signed installers per platform (NFR-7), a published SBOM, and a changelog
  that calls out cartography changes with before/after images.

---

## 15. Attribution and legal

- **FR-L1** swisstopo attribution — `© swisstopo` / "Source: Federal Office of Topography
  swisstopo" — must be visible in the app's About screen, in the build report, and embedded
  in the generated map's metadata (family name / copyright string) so it travels with the file.
- **FR-L2** The `mkgmap` and `splitter` licenses ship with the app and are listed in `NOTICE`,
  along with the bundled JRE's license and any reused prior-art code.
- **FR-L3** The app must state plainly that generated maps are for personal use, that
  swisstopo terms of use apply to redistribution, and that Garmin is not affiliated with or
  endorsing this project.
- **FR-L4** "Garmin", "Edge", "fēnix", and "epix" are Garmin trademarks, used only
  descriptively to identify compatible devices.

---

## 16. Roadmap beyond v1

| Version | Theme | Contents |
|---|---|---|
| v2 | **Routing** | Build a routable graph from swissTLM3D: node connectivity, oneway, access classes, bridge/tunnel levels, turn restrictions where derivable, `mkgmap --route`. Highest-value follow-up, and the reason v1's converter must already deduplicate shared nodes (FR-P3). |
| v2 | Address & POI search | swissNAMES3D-driven searchable index on device |
| **v1** | **Ski touring (skimo) preset** | **Implemented.** SAC ski routes with the skiable / carrying / caution distinction, snowshoe and winter hiking trails, and swissTLM3D lifts and cableways. |
| v2 | Cycle routes | `ch.astra.veloland` and `ch.astra.mountainbikeland`, plus `ch.astra.wanderland` for official hiking route numbers. Needs a shapefile reader: these publish no GeoPackage, and swissTLM3D contains no cycle data at all (docs/m0-findings.md §4.19). |
| v2 | Raster overlay experiment | Both the Edge 840 and fēnix 5 Plus advertise `Garmin/CustomMaps`. Deferred past v1 by decision: Custom Maps are capped near 100 tiles of 1 MP, render only in a narrow zoom band, and support neither search nor routing, so the vector map is better in every respect except literal appearance. |
| v3 | Ski touring, extended | Slope-angle classification derived from swissALTI3D, avalanche terrain shading |
| v3 | Hillshade | Shaded-relief-derived features within Garmin's vector constraints |
| v3 | CLI | Headless build from a recipe file, for scripting and CI |
| v4 | Other countries | Generalize the source abstraction to other national OGD vector models (Austria, France IGN, Germany) |

---

## 17. Risks

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| Garmin device limits are undocumented and forum lore is wrong | Maps that silently fail on device | **High** | Conservative defaults, confidence levels, user overrides, real-device verification procedure (§5.4, §13.5) |
| swissTLM3D schema differs from prior-art assumptions | Style rules mismap or drop features | **High** | Explicit schema-discovery milestone; all rules written against the discovered schema (§3.1) |
| 4.6 GB acquisition is a poor first-run experience | Users abandon before first build | High | Explicit explanation, resumable download, WMTS-based picker usable before acquisition (§4.1) |
| Output size blows the device budget, contours being the main culprit | Unusable output | Medium | Calibrated estimator, aggressive contour simplification, auto map-set splitting (FR-P8, FR-36, FR-60) |
| mkgmap style/TYP work is more art than engineering | Map looks bad, effort underestimated | Medium | Treat cartography as its own workstream with visual regression tests; start from prior art (§8.4) |
| Cartography looks fine on desktop, illegible on a watch | Wrist devices unusable | Medium | Separate wrist style variant from the start (FR-CART6), validate on hardware early |
| swisstopo changes STAC layout or release cadence | Acquisition breaks | Low | Dynamic release resolution, no hard-coded URLs, cached catalog fallback |
| Character encoding mangles French/German names on device | Embarrassing correctness bug | Medium | Encoding is an explicit requirement (FR-P10) validated on real hardware |
| Bundled JRE inflates installer and complicates notarization | Distribution friction | Low | `jlink` minimal image; prefer system Java when present (FR-P12) |

---

## 18. Acceptance criteria for v1

The product ships when all of the following hold:

1. A first-time user on macOS installs the app, is guided through acquisition, selects an
   Edge device and one canton, and gets a working `gmapsupp.img` **without reading any
   documentation**.
2. The same flow works on Windows and Linux.
3. All four area selection modes work and compose.
4. The whole-Switzerland preset produces device-legal output, splitting into multiple map
   sets when required.
5. Generated maps are verified on real Edge and fēnix hardware per §13.5, including correct
   rendering of Swiss place names with umlauts and accents.
6. Hiking trails are visually distinguishable by Swiss trail class on both an Edge and a
   fēnix screen.
7. Size estimates land within ±25 % on a suite of at least ten reference areas.
8. Cancelling any stage leaves no corrupt cache and no orphaned processes.
9. Every build emits a manifest sufficient to reproduce it.
10. Attribution requirements (§15) are met in-app and in the output file.
