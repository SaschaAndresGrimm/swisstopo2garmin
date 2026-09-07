# swisstopo2garmin

**Free Swiss topographic maps for your Garmin Edge or fēnix — built from swisstopo's own
open data, on your own computer.**

Choose a device, choose an area, press build. You get a `gmapsupp.img` with the
Landeskarte's colours, Swiss hiking-trail classes, 20 m contours and shaded relief, and
the app copies it onto the watch or bike computer for you.

Map data © swisstopo. The program is free software under GPL-3.0-or-later.

---

# For users

## Getting it

Installers for macOS, Windows and Linux are built for every release and attached to it:
**[Releases](https://github.com/SaschaAndresGrimm/swisstopo2garmin/releases)**.

| Platform | File |
|---|---|
| macOS (Apple silicon) | `swisstopo2garmin_*_aarch64.dmg` |
| macOS (Intel) | `swisstopo2garmin_*_x64.dmg` |
| Windows | `swisstopo2garmin_*_x64_en-US.msi` |
| Linux | `*.AppImage` or `*.deb` |

Verify your download against the `SHA256SUMS` file beside it.

> **The installers are not code-signed yet.** macOS will say "cannot be opened because
> the developer cannot be verified" — right-click the app and choose **Open**, once.
> Windows SmartScreen will warn. This is because signing certificates cost money and
> none have been bought, not because anything is wrong with the file;
> [docs/release.md](docs/release.md) explains exactly what is missing.

Nothing else needs installing. No Java, no GDAL, no Python — the map compiler, the
splitter and a private Java runtime are all inside the app. That is why the download is
around 150 MB: 130 MB of it is the Java runtime.

**Want to see the maps before installing anything?** Six ready-made maps of Grindelwald
are attached to each release —
[docs/sample-maps.md](docs/sample-maps.md) says what each one is and how to copy it
across. They have not been verified on hardware, which is exactly why they are the
interesting ones to try.

## Your first map

The app is meant to be usable without reading anything. If you get stuck,
[docs/getting-started.md](docs/getting-started.md) walks through it properly.

1. **Data** — download swissTLM3D. This is the long part: 4.5 GB over the wire, unpacked
   as it arrives, and it resumes if interrupted. Add "swissTLM3D Wanderwege" for Swiss
   hiking-trail classes.
2. **Device** — pick your model. There are tested profiles for the Edge 840 and the
   fēnix 5 Plus and generic ones for other Edge and fēnix models; each carries a size
   budget and says how confident that number is, because Garmin publishes none of them.
   If yours is not listed, the generic profile for its family is deliberately
   conservative.
3. **Area** — draw on the map, search a place, choose a canton, or take a corridor along
   a GPX track. Once drawn, drag a corner to reshape it or the middle to move it. Start
   small: a 10 km radius builds in a couple of minutes and tells you whether you like
   the result.
4. **Content** — hiking, cycling, ski touring or full topo; contour interval; shaded
   relief; summer or winter colours; slope-angle classes; label language.
5. **Build** — progress per stage. Cancelling is immediate and leaves nothing behind.
6. **Install** — plug the device in. The app copies the map and then reads it back to
   check it arrived intact.

### Disk space

Allow **20 GB**. swissTLM3D is 10.0 GB unpacked, and every new area caches its own
elevation tiles at roughly 1.2 MB per square kilometre. The Data screen shows where the
space went and can delete the two re-derivable parts; the data folder can be pointed at
another drive before the first download.

## What you get

- **The Landeskarte's look**, from colours measured off swisstopo's own printed sheets
  rather than guessed — summer and winter schemes, rock hachures, blue contours over ice.
- **Swiss hiking-trail classes** drawn distinguishably: hiking, mountain hiking, alpine
  hiking.
- **Contours** at 10, 20, 50 or 100 m, from swissALTI3D, with shaded relief where the
  device supports it.
- **Ski touring**: SAC ski routes with the skiable / carrying / caution distinction,
  snowshoe and winter hiking trails, lifts and cableways.
- **Slope-angle classes** in swisstopo's own bands, 30° to over 50°, drawn as a hatch so
  the map underneath stays readable.
- **Cycling**: Veloland, Mountainbikeland and the official Wanderland route numbers.
- **SAC huts and public transport stops**, named.
- **Optional turn-by-turn routing and address search**, both off by default because
  neither has been on a device yet.
- **Place names** in German, French, Italian or Romansh where swissNAMES3D has them.
- **A size estimate before you build**, fitted on real builds and refined by yours.
- **A manifest** beside every map: the recipe, the exact dataset releases, the tool
  versions and the timings. Enough to reproduce it.

## What it does not do yet

- **Routing is built but unverified.** Turn-by-turn navigation along the roads and paths
  can be switched on in the content step, and it is **off by default** because nothing
  about it has been checked on a device — including the assumption that a divided
  carriageway is digitised in the direction of travel. That assumption is verified against
  the data ([docs/routing.md](docs/routing.md)) and not on hardware, which are different
  things. Check any route it gives you against the road signs.
- **Address search is built but unverified**, and off by default. It needs a separate
  137 MB download — swissTLM3D has street names and no house numbers, so the house numbers
  come from the official address register ([docs/addresses.md](docs/addresses.md)).
- **No raster/paper-map view**, deferred past v1.
- **Not verified on hardware since Milestone 6.** Maps render correctly on an Edge 840
  (firmware 3133) and a fēnix 5 Plus (firmware 1930), but the winter colours, slope
  classes, hut symbols, transit stops and night palette have never been on a device.
  [docs/device-verification.md](docs/device-verification.md) records exactly what has and
  has not been checked.
- **No screen-reader or keyboard-only pass**, and the map's drawing and editing tools are
  pointer-only. Every other way of choosing an area works from the keyboard.
  [docs/accessibility.md](docs/accessibility.md) lists the gaps.

If something fails, the app says what failed, why and what to do about it — and says
plainly when it does not recognise a failure rather than inventing a cause.

## Using the maps you make

They are yours to use. If you pass one to somebody else, swisstopo's
[terms of use for free geodata](https://www.swisstopo.admin.ch/en/terms-of-use-free-geodata-and-geoservices)
come with it: the attribution has to stay, and it is embedded in the file so it does.

Garmin, Edge, fēnix and epix are trademarks of Garmin Ltd. or its subsidiaries, used here
only to say which devices this works with. This project is not affiliated with, endorsed
by or sponsored by Garmin.

---

# For developers

The pipeline is pure Rust — no GDAL, no Python at build time. `spikes/s0/` is kept for
schema discovery and the cartography guards, not for building.

## Prerequisites

- Rust stable
- **Node ≥ 20.17** — a Node built without ICU (Anaconda ships one) breaks eslint;
  `.nvmrc` pins the version
- Java is *not* required: `vendor/fetch_tools.py` downloads a private JRE plus mkgmap and
  splitter without touching the system

```bash
python3 vendor/fetch_tools.py     # mkgmap, splitter, JRE -> vendor/
npm --prefix frontend ci
```

`vendor/fetch_tools.py` is not optional: `src-tauri` declares the vendored toolchain as a
bundle resource, so it does not compile without it.

## Checks

Everything CI runs, in the order it runs it:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                  # offline and deterministic
cargo test -p s2g-core --test live -- --ignored          # hits data.geo.admin.ch

npm --prefix frontend run typecheck
npm --prefix frontend run lint
npm --prefix frontend run check:i18n     # all four bundles agree; no dead keys
npm --prefix frontend run check:a11y     # static: control names, labels, live regions

python3 tools/make_typ.py && python3 spikes/s0/checkstyle.py   # cartography consistency
python3 tools/sbom.py --check                                  # supply chain (NFR-10)
cargo test --release -p s2g-core --test perf -- --nocapture     # NFR-1, NFR-2
cargo test --release -p s2g-core --test estimator_reference     # FR-60, ±25%
```

## Running the app

A **debug** build loads the frontend from the Vite dev server (`devUrl`); only a
**release** build embeds `frontend/dist`. So `cargo build && ./target/debug/…` on its own
opens an empty window.

```bash
# dev, with hot reload on frontend edits
npx --prefix frontend tauri dev

# a real installer -- CI=true is required for the .dmg, or the step hangs
# on Finder AppleScript with no error
CI=true npx --prefix frontend tauri build
```

The Tauri CLI is pinned in `frontend/package-lock.json`, so it needs no separate install.
It must run from the **repository root**: it locates the project by searching below the
working directory, so `npm run` inside `frontend/` finds nothing.

## Generated files — do not edit

| File | Generated by | Guarded by |
|---|---|---|
| `frontend/src/state/bindings.ts` | `cargo test -p swisstopo2garmin` | CI fails if stale |
| `typ/*.txt` | `python3 tools/make_typ.py` | CI fails if stale |
| `estimator/size-model.json` | `cargo run --example fit_size_model` | `tests/estimator_reference.rs` |

## Where things live

- `crates/s2g-core/` — everything that reads data, projects, clips, contours, estimates
  and builds. All the logic and all the tests.
- `src-tauri/` — the IPC surface. A thin shell: it validates, spawns and reports.
- `frontend/` — React. No geometry and no projection; it asks the backend.
- `style/`, `typ/`, `cartography/` — the mkgmap style and the measured palettes.
- `devices/` — one JSON per device, every limit carrying a confidence level and sources.
- `vendor/` — the fetched toolchain. Gitignored.

## The rules that matter

From [CLAUDE.md](CLAUDE.md), and they are load-bearing:

1. **Never invent a schema.** Every swissTLM3D layer, attribute and value used in code
   must appear in [docs/tlm3d-schema.md](docs/tlm3d-schema.md), which is generated from
   the real file. If it is not there, it was invented — that is a defect.
2. **Never invent a device limit.** Every number in `devices/*.json` carries a
   `confidence` of `vendor`, `measured`, `community` or `assumed`, and sources.
   `assumed` is fine. A fabricated citation is not.
3. **Stream, never slurp.** Any path that could hold the national dataset in memory is a
   defect. The GeoPackage is 10.0 GB.
4. **Report honestly.** "Works" means a test asserts it.

## Documentation

- **[SPEC.md](SPEC.md)** — what it does and why, with the requirement IDs the code cites
- **[PLAN.md](PLAN.md)** — milestone status, including what is not met
- **[docs/getting-started.md](docs/getting-started.md)** — first map, for a user
- **[docs/sample-maps.md](docs/sample-maps.md)** — the ready-made maps on each release
- **[docs/device-verification.md](docs/device-verification.md)** — what to check on real
  hardware, and why CI cannot
- **[docs/cartography.md](docs/cartography.md)** — how swissTLM3D becomes Garmin types,
  and how to contribute a change
- **[docs/error-matrix.md](docs/error-matrix.md)** — every specified failure, its
  behaviour, its test, and what is not covered
- **[docs/size-model.md](docs/size-model.md)** — the size and time estimates, and their
  limits
- **[docs/routing.md](docs/routing.md)** — the road network: what is mapped, what it
  costs, and the one assumption that was checked
- **[docs/addresses.md](docs/addresses.md)** — where house numbers come from, and how
  they reach the device's search index
- **[docs/performance.md](docs/performance.md)** — what is measured, and what is not
- **[docs/release.md](docs/release.md)** — how a release is built, signed and documented
- **[docs/accessibility.md](docs/accessibility.md)**,
  **[docs/attribution-audit.md](docs/attribution-audit.md)**,
  **[docs/m0-findings.md](docs/m0-findings.md)** — the audits and the early findings

## Contributing

Device measurements are especially welcome: most limits in `devices/*.json` are still
`community` or `assumed`, and [docs/device-verification.md](docs/device-verification.md)
is the procedure for turning one into `measured`. Cartography changes should come with
before/after renders — [docs/cartography.md](docs/cartography.md) says how.

`NOTICE` lists every third-party component and its licence, including the bundled Java
tools and runtime.
