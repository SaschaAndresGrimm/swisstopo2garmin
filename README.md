# swisstopo2garmin

Build Garmin Edge and fēnix maps from free swisstopo geodata, with a desktop GUI.

Map data © swisstopo. Licensed GPL-3.0-or-later.

- **[SPEC.md](SPEC.md)** — what it does and why
- **[PLAN.md](PLAN.md)** — implementation plan and milestone status
- **[docs/getting-started.md](docs/getting-started.md)** — first map, start to finish
- **[docs/sample-maps.md](docs/sample-maps.md)** — six ready-made maps attached to each
  release, to try on a device before building anything
- **[docs/device-verification.md](docs/device-verification.md)** — what to check on real
  hardware, and why CI cannot
- **[docs/cartography.md](docs/cartography.md)** — how swissTLM3D becomes Garmin types,
  and how to change it
- **[docs/error-matrix.md](docs/error-matrix.md)** — every specified failure, its
  behaviour, and its test
- **[docs/performance.md](docs/performance.md)** — what is measured, and what is not
- **[docs/release.md](docs/release.md)** — how a release is built, signed and documented
- **[docs/](docs/)** — the discovered swissTLM3D schema, palettes, accessibility,
  attribution audit, Milestone 0 findings

## Status

The whole path works, end to end and from the GUI: choose a device, choose an area,
choose content, build, install. Maps have been verified on a Garmin Edge 840 (firmware
3133) and a fēnix 5 Plus (firmware 1930).

The pipeline is pure Rust — no GDAL, no Python at build time. `spikes/s0/` is kept for
schema discovery and the cartography guards, not for building.

What works:

- **Data**: swissTLM3D, hiking trails, SAC ski routes, snowshoe and winter hiking trails,
  and the three ASTRA route networks, each acquired by its own packaging (zipped
  GeoPackage, bare GeoPackage, or zipped shapefiles).
- **Areas**: a rectangle drawn on the swisstopo basemap, a radius around a searched
  place, a corridor around an imported GPX track or FIT course, or all of Switzerland —
  and once drawn, editable: drag a corner to reshape it, the middle to move it, a
  midpoint to add a corner.
- **Content**: four presets (hiking, cycling, ski touring, full topo), a per-layer panel,
  contour interval, shaded relief, and a summer or winter colour scheme measured from
  swisstopo's own sheets.
- **Estimation**: output size from a model fitted on real builds, and remaining build
  time weighted by measured stage durations.
- **Recipes**: save a configuration and rebuild it later.

- **Areas, continued**: cantons, districts and communes by name; polygons and circles
  drawn on the map; several selections combined; and selections exchanged as GeoJSON.
- **Labels**: place names in German, French, Italian or Romansh where swissNAMES3D has
  them, rather than only the local form.
- **Slope classes**: swisstopo's own 30–50° bands, computed from the elevation data.
- **Recovery**: an interrupted build is found on the next start, along with any map
  compiler still running for it, and can be resumed or discarded.

**Not yet**, and deliberately so: on-device routing, address search, and a raster
overlay — all deferred past v1 (SPEC.md §16). **Not yet, and outstanding**: a
screen-reader and keyboard-only pass ([docs/accessibility.md](docs/accessibility.md)
lists the gaps), signed installers ([docs/release.md](docs/release.md) says what is
missing), and a device sweep of everything added since the last one. See
[PLAN.md](PLAN.md) for milestone status.

## Prerequisites

- Rust stable
- **Node ≥ 20.17** — note that a Node built without ICU (Anaconda ships one) breaks
  eslint; `.nvmrc` pins the version
- Java is *not* required: `vendor/fetch_tools.py` downloads a private JRE plus
  mkgmap and splitter, without touching the system

```bash
python3 vendor/fetch_tools.py        # mkgmap, splitter, JRE -> vendor/
cd frontend && npm ci && cd ..
```

### Disk space

swissTLM3D inflates to **10.0 GB**, and every area built caches its own swissALTI3D
tiles at roughly 1.2 MB per square kilometre. Allow 20 GB. The app's Data screen shows
where the space went, broken down, and offers to delete the two re-derivable parts
(elevation tiles and build intermediates); the data directory itself can be pointed at
another volume before the first download.

## Develop

```bash
cargo test --workspace                       # 230 tests, offline
cargo test -p s2g-core --test live -- --ignored   # hits data.geo.admin.ch
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

cd frontend && npm run typecheck && npm run lint && npm run build

```

### Running the app

A **debug** build loads the frontend from the Vite dev server (`devUrl`); only a
**release** build embeds `frontend/dist`. `cargo build && ./target/debug/...` on its own
therefore opens an empty window.

```bash
# dev: two processes, with hot reload on frontend edits
cd frontend && npm run dev &          # serves http://localhost:1420
cargo build -p swisstopo2garmin && ./target/debug/swisstopo2garmin

# or, with the Tauri CLI, one command that starts both:
cargo install tauri-cli --version "^2"
cargo tauri dev

# standalone: embeds the frontend, no dev server needed (slow build: lto + codegen-units=1)
cargo build --release -p swisstopo2garmin && ./target/release/swisstopo2garmin
```

The IPC types in `frontend/src/state/bindings.ts` are **generated** from the Rust
definitions by `cargo test -p swisstopo2garmin`. CI fails if the checked-in file is
stale, so the two sides cannot drift.

## Build a map today (spike pipeline)

```bash
spikes/s0/build.sh Grindelwald 8 20      # place, radius km, contour interval m

# content presets (SPEC.md FR-50)
S2G_WINTER=1 spikes/s0/build.sh Grindelwald 8 20   # ski touring, snowshoe, winter hiking
S2G_CYCLE=1  spikes/s0/build.sh Grindelwald 8 20   # cycle and mountain-bike routes
S2G_STYLE=swisstopo-wrist spikes/s0/build.sh Grindelwald 8 20   # fenix cartography
S2G_ARCSEC=3 spikes/s0/build.sh Grindelwald 8 20   # gentler relief shading
```

The winter and cycling presets need their own data:

```bash
python3 spikes/s0/fetch_winter.py   # ~38 MB, GeoPackage
python3 spikes/s0/fetch_routes.py   # ~170 MB, shapefile
```

Produces `out/gmapsupp-<place>.img` plus a preview PNG. Copy the `.img` to
`/Garmin/gmapsupp.img` on the device — see [docs/device-verification.md](docs/device-verification.md).

The cartography TYP is **generated** from the measured swisstopo palette:

```bash
python3 tools/make_typ.py && python3 spikes/s0/checkstyle.py
```

`checkstyle.py` is not optional. A polygon type emitted by the style but missing from the
TYP's `[_drawOrder]` renders as nothing on the device *and* is invisible in the `.img` —
see [docs/m0-findings.md](docs/m0-findings.md) §4.8.

## Data cache

Datasets live in `~/.cache/swisstopo2garmin` (override with `S2G_CACHE`). swissTLM3D is a
4.80 GB download that inflates to a **10.78 GB** GeoPackage; it is inflated during
download so the archive is never stored.
