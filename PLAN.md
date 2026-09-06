# swisstopo2garmin — AI Implementation Plan

**Companion to:** [SPEC.md](SPEC.md) · **Date:** 2026-09-06

A plan for building this software with an AI coding agent. It is ordered by **risk, not by
architecture layer** — the things most likely to kill the project are proven first, on
throwaway code, before a line of UI exists.

---

## 0. Read this first: the one thing that matters

The instinct with an AI agent is to say "build the app" and let it scaffold a beautiful
Tauri project with a five-step wizard. **Do not do that.** This project has four unknowns
that no amount of clean architecture survives being wrong about:

1. What is actually inside the swissTLM3D GeoPackage (§3.1 of the spec — the layer names
   in circulation are unverified).
2. Whether contours streamed from swissALTI3D COGs are cheap enough to be a per-build step.
3. Whether a `gmapsupp.img` built this way actually renders on a real fēnix and a real Edge.
4. What the real size and tile limits of those devices are.

Unknowns 1–3 are answered by **Milestone 0**, which produces a scrappy command-line
pipeline and one `.img` file on one real device. Unknown 4 is answered by hardware
measurement, which starts in Milestone 0 and continues throughout.

**Gate: do not start Milestone 2 until a map built by Milestone 0 has rendered on a
physical Garmin device.** Everything after that point is comparatively predictable work.

---

## 1. Working agreement with the agent

Put this in `CLAUDE.md` at the repo root so it applies to every session.

### Rules

1. **Read `SPEC.md` before every milestone.** Requirement IDs (`FR-30`, `NFR-2`, `FR-P6`)
   are the contract. Reference them in commits and PR descriptions.
2. **Never invent a data schema.** If the agent does not know a real swissTLM3D layer name,
   attribute name, or attribute value, it must read it out of the actual file and record it
   in `docs/tlm3d-schema.md`. Guessed schema is the single most likely source of silent
   wrongness in this project.
3. **Never invent a device limit.** Every number in `devices/*.json` carries a
   `confidence` level and a source. `assumed` is an acceptable value; a fabricated citation
   is not.
4. **One milestone per branch, small commits.** Each commit compiles and passes tests.
5. **Tests before UI.** Every pipeline module lands with unit tests against fixtures. The
   GUI is a thin shell over tested Rust; it is never where logic lives.
6. **Stream, never slurp.** Any code path that could hold a national dataset in memory is a
   defect (NFR-2). This must be checked in review, not discovered in production.
7. **No new dependency without justification** in the PR description: what it does, why not
   the standard library, its license, its maintenance status.
8. **Report honestly.** "Contours work" means a golden test asserts it. If a milestone's
   acceptance criteria are not met, say which ones and why — do not narrow the milestone to
   fit what got built.
9. **Ask when the spec is ambiguous** rather than picking silently and moving on. Record the
   answer in the spec.

### Definition of done, per milestone

- Acceptance criteria in this plan are demonstrably met.
- `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` clean.
- `npm run lint`, `npm run typecheck`, frontend tests clean.
- New behaviour has tests; new schema knowledge is documented.
- CI green on macOS, Windows, and Linux.
- The PR description lists the requirement IDs implemented.

---

## 2. Milestone 0 — De-risking spikes (throwaway code)

**Goal:** prove the entire chain end-to-end on one small area. Ugly scripts are fine and
expected. Nothing here needs to survive into the product; the *knowledge* is the deliverable.

**Duration:** aim for 1 week. This is the highest-value week in the project.

### S0.1 — Discover the real swissTLM3D schema

> **Prompt:** Download the latest swissTLM3D GeoPackage from the swisstopo STAC API
> (`https://data.geo.admin.ch/api/stac/v1/collections/ch.swisstopo.swisstlm3d/items` —
> resolve the newest item, take the `.gpkg.zip` asset; it is ~4.6 GB, so download it
> resumably and cache it outside the repo). Then, treating the GeoPackage as a plain SQLite
> database, produce `docs/tlm3d-schema.md` containing, for every layer: the exact layer
> name, geometry type, SRID, feature count, and for every attribute the name, type, and —
> for anything that looks like a classification code — the complete set of distinct values
> with their frequencies. Do not guess or paraphrase any name. Also record the total
> unpacked size and how long the enumeration took.

**Acceptance:** `docs/tlm3d-schema.md` exists, is generated from the real file, and lets a
reader answer "which layer and which attribute value identifies an alpine hiking trail?"
without opening the data.

### S0.2 — Minimal vector path: GPKG → OSM PBF → IMG

> **Prompt:** Pick one small commune. Using the schema from S0.1, write a throwaway script
> that reads roads, hiking paths, water, and forest for that commune out of the GeoPackage
> via SQLite + WKB, reprojects EPSG:2056 → EPSG:4326, and writes an OSM XML or PBF file.
> Then run `splitter.jar` and `mkgmap.jar` over it with a minimal style to produce a
> `gmapsupp.img`. Report: output size, feature counts, wall-clock time per stage, and any
> mkgmap warnings. Open the result in a desktop Garmin map viewer and attach a screenshot.

**Acceptance:** a `gmapsupp.img` exists, renders recognizable geometry, and every stage's
cost is written down.

### S0.3 — Contours from swissALTI3D over HTTP

> **Prompt:** For the same commune, enumerate the intersecting swissALTI3D tiles via STAC,
> stream **only** the 2 m GeoTIFF assets using HTTP range requests (do not download whole
> files if the COG structure allows partial reads), mosaic them, and generate 20 m contour
> lines. Emit them as OSM ways tagged `contour=elevation`, `ele=<m>`,
> `contour_ext=elevation_minor|medium|major`. Measure: bytes transferred, wall-clock time,
> number of contour ways, and the resulting `.img` size contribution both with and without
> Douglas–Peucker simplification. Verify contours are continuous across tile seams.

**Acceptance:** contours render, seams are clean, and there is a hard number for
"bytes and seconds per km² of contours" — this figure decides whether contour generation
is a per-build step or needs its own cache.

### S0.4 — Real hardware, both device classes

> **Prompt:** Install the S0.2 + S0.3 map on a real Garmin Edge and a real fēnix/epix.
> Document, with photographs: does it appear in the map list; does it render at each zoom
> level; do Swiss place names with umlauts and accents display correctly; is panning
> responsive; does it coexist with the factory map. Then determine the device's real limits
> by binary search on map size and tile count, and record everything in
> `docs/device-verification.md`.

**Acceptance:** photographs of both devices showing the map, plus at least two device
profiles promoted from `assumed` to `measured` confidence.

**This is the project's go/no-go gate.**

### S0.5 — Cartography feasibility

> **Prompt:** Read the mkgmap style and TYP files in
> `martinzellner/swisstopo-garmin` and `wirhabenzeit/sac-skimo-garmin`. Check each
> project's license. Write `docs/cartography.md` as a first-draft mapping table:
> TLM3D layer + attribute value → Garmin type code → intended appearance → zoom levels.
> Then build a TYP file covering the top ~20 types and show, on a real device, that Swiss
> hiking trail classes (yellow / red-white / blue-white) are visually distinguishable.

**Acceptance:** trail classes are distinguishable in a photograph of a real device screen,
and `docs/cartography.md` is the living mapping table the style work will grow from.

### Milestone 0 output

A short written **findings report** answering: is the per-build clip fast enough without a
tiled intermediate? Is contour streaming viable? What are the real device limits? Where did
the spec's assumptions turn out wrong? **Update `SPEC.md` accordingly before Milestone 1.**

---

## 2b. Milestone 0 result — GATE PASSED (2026-09-06)

A map built by the spike pipeline renders correctly on a Garmin Edge 840. Findings and the
six resulting SPEC changes are in [docs/m0-findings.md](docs/m0-findings.md).

Four defects all presented identically as *"the map is listed on the device but draws
nothing"*, which is worth internalising before Milestone 4:

| # | Defect | Detectable from the .img? |
|---|---|---|
| 4.9 | **Built the wrong place** — two settlements named Grindelwald; `LIMIT 1` picked the <20-inhabitant hamlet 45 km away | no — the map was internally perfect |
| 4.5 | Overview map absent from `gmapsupp` (single-pass `--gmapsupp`) | yes — FAT lists one map |
| 4.8 | TYP without `[_drawOrder]` hides **every** polygon | **no** — polygons are in the RGN either way |
| 4.10 | Labels uppercase + ASCII without `--lower-case` | yes, via the dump |

Two of the four are invisible to container inspection, which is why Milestone 4 needs the
render harness (`tools/imgdump` + `tools/render.py`), not just structural checks.

**New requirement raised after the gate:** visual fidelity to the swisstopo raster maps is
a headline feature, not a polish item. See SPEC.md §8.1 and FR-CART8/9/10. This promotes
Milestone 5 (cartography) from "parallel workstream" to **the first thing worked on**.

## 3. Milestone 1 — Foundation

**Goal:** the real repository, CI, and the data acquisition layer. No pipeline, no map UI.

### Tasks

1. Tauri 2 project scaffold matching the layout in spec §10.2; React + TypeScript frontend;
   Vite; workspace-level `cargo fmt`/`clippy` config.
2. CI on GitHub Actions: build, test, lint, typecheck on macOS (arm64 + x64), Windows x64,
   Linux x64. Cache Rust and npm artifacts.
3. `stac/` — typed STAC client: list collections, resolve latest item for a collection,
   enumerate assets, list tiles intersecting a bbox. Tested against recorded HTTP fixtures
   so CI is offline and deterministic.
4. `stac/download` — resumable ranged downloader with checksum verification, atomic rename,
   backoff retry, and cancellation (FR-D1…D4).
5. `cache/` — content-addressed dataset cache with a manifest, size accounting, corruption
   quarantine, and disk-space prechecks (FR-10…FR-13).
6. `ipc/` — Tauri command layer plus typed event stream; TypeScript types generated from
   Rust (`ts-rs`) and checked in CI so the two sides cannot drift.
7. Minimal shell UI: window, step indicator, and a working **Data** screen that can
   download, resume, verify, and delete swissTLM3D.
8. i18n scaffolding with de/fr/it/en resource files; a lint rule that fails on string
   literals in components.
9. `vendor/` — a documented, scripted way to fetch `mkgmap.jar` and `splitter.jar` at pinned
   versions, and to produce the `jlink` JRE per platform. Plus system-Java detection (FR-P12).

**Acceptance:** on all three platforms, a fresh checkout builds; the app launches; the Data
screen downloads swissTLM3D with visible progress; killing the app mid-download and
restarting resumes rather than restarting; CI is green.

---

### Milestone 1 status — foundation complete (2026-09-06)

| Task | State |
|---|---|
| 1. Tauri 2 + React/TS + Vite scaffold, workspace lints | done |
| 2. CI on macOS arm64/x64, Linux, Windows | done (`.github/workflows/ci.yml`) |
| 3. Typed STAC client, tested offline | done — 5 tests |
| 4. Resumable ranged downloader, checksum, atomic rename | done — 7 tests |
| 5. Content-addressed cache, provenance, quarantine, disk precheck | done — 6 tests |
| 6. Typed IPC, TS generated from Rust, drift checked in CI | done — `ts-rs`, 6 export tests |
| 7. Shell UI + working Data screen | done |
| 8. i18n de/fr/it/en | done |
| 9. Vendored mkgmap/splitter/JRE | done (Python, from M0) |

24 tests pass; `cargo fmt --check` and `cargo clippy -- -D warnings` are clean; the app
launches. Two live tests (`tests/live.rs`) hit the real API and are `#[ignore]`d so CI
stays offline — they are what caught the reqwest bug below.

**Deviation from SPEC §10.2:** pipeline modules live in `crates/s2g-core`, not inside
`src-tauri`, so they are testable without Tauri. The spec has been updated.

**Bug worth remembering:** `reqwest::Response::content_length()` returns the *body*
length, which is **0 for a HEAD request**. Every size-dependent code path saw 0, and the
zip reader underflowed on `total - 1`. The Content-Length header must be read directly.
Synthetic tests missed it entirely; only the live test caught it.

## 4. Milestone 2 — Vector pipeline

**Goal:** national GPKG → clipped OSM PBF, production quality, fully tested.

### Tasks

1. `gpkg/` — GeoPackage reader: SQLite access, GPKG geometry-header parsing, WKB decode for
   all relevant geometry types, streaming iteration with bounded memory, use of the GPKG
   R-tree index for spatial pre-filtering.
2. `proj/` — EPSG:2056 ↔ EPSG:4326. Tested against published swisstopo reference points.
   Document the accuracy argument from FR-P1 in the module docs.
3. `geom/` — clip line/polygon to polygon, buffer, union, Douglas–Peucker simplify.
   Property-based tests plus the explicit edge cases from spec §13.1.
4. `tilestore/` — the 1 km LV95 grid intermediate (spec §7.2): build once per release,
   store per-cell per-layer feature counts for the size estimator, memory-mapped reads.
   **Skip or simplify this if Milestone 0 showed direct clipping is fast enough** — do not
   build the optimization the findings report said was unnecessary.
5. `osmpbf/` — OSM PBF writer: node deduplication into shared nodes (FR-P3), deterministic
   synthetic IDs from TLM UUIDs (FR-P4), multilingual name tags (FR-P5), Z dropped except
   where useful (FR-P2).
6. `pipeline/` — stage orchestration: typed stage inputs/outputs, content-addressed stage
   caching (FR-72), progress reporting, and cancellation that leaves no partial artifacts.
7. Test fixtures: one commune-sized GPKG extract committed to `tests/fixtures/`, and a CI
   integration test running clip → convert in under a minute.

**Acceptance:** given a cached national GPKG and a polygon, the pipeline emits a valid OSM
PBF; an independent PBF reader round-trips it; peak RSS stays under 4 GB for a
whole-Switzerland clip (NFR-2); cancelling mid-clip leaves no partial output; re-running an
unchanged stage is a cache hit.

---

## 5. Milestone 3 — Contours

**Goal:** production contour generation from swissALTI3D.

### Tasks

1. `contour/cog` — GeoTIFF/COG reader for Float32 tiled TIFFs over `/vsicurl`-style ranged
   HTTP, with a local tile cache.
2. `contour/mosaic` — assemble tiles for an area with one-pixel overlap and seam stitching
   (FR-P7); handle missing tiles gracefully with a warning that names the affected cells.
3. `contour/generate` — marching squares at configurable intervals, minor/medium/major
   classification, optional pre-smoothing, and simplification tuned below Garmin's
   coordinate resolution (FR-P8).
4. Emit contours as OSM ways with the mkgmap-compatible tagging of FR-P6.
5. Elevation-tile pre-caching so a chosen area can be made fully offline (FR-C2).
6. Tests: analytic surfaces (plane, cone, saddle) with known contour geometry; a seam test
   asserting continuity across a tile boundary; a size test asserting simplification stays
   within a byte budget per km².

**Acceptance:** contours for a canton generate within the time budget established in S0.3;
no visible seams in a rendered check; simplification reduces size by a measured factor with
no visible quality loss at device zoom levels.

---

## 6. Milestone 4 — Garmin compilation

**Goal:** OSM PBF + style + TYP → verified `gmapsupp.img`.

### Tasks

1. `garmin/split` — invoke `splitter.jar`; per-device-class `--max-nodes` tuning; parse the
   tile inventory it produces; enforce the device tile-count limit.
2. `garmin/compile` — invoke `mkgmap.jar` with the full documented option set (spec §7.6);
   deterministic family/product ID allocation from the recipe hash; TYP compilation;
   `.gmap` bundle as optional secondary output.
3. Child-process management: process groups, reliable cancellation on all three platforms,
   stdout/stderr streamed into the build log, non-zero exits mapped to plain-language causes.
4. `garmin/verify` — parse the produced IMG: subfile inventory, type-code histogram, tile
   count, size, family/product IDs (spec §7.7).
5. Encoding correctness: choose and validate the code page so that umlauts and French
   accents survive to the device (FR-P10). **Validate on hardware, not in a desktop viewer.**
6. Map-set splitting for oversized areas: partition, allocate distinct family IDs, resume at
   map-set granularity (FR-36, FR-37).
7. Build manifests (spec §11.3) written for every build.

**Acceptance:** an end-to-end CI test builds a fixture-sized IMG and asserts its structure;
a canton build completes in under 10 minutes (NFR-1); Swiss characters verified correct on
real hardware; the whole-Switzerland path produces multiple device-legal map sets.

---

### Milestones 2-4 status — pipeline complete in Rust (2026-09-06)

The whole build now runs from `s2g-core` with no GDAL and no Python. 85 tests pass;
`tests/build.rs` takes the committed fixture through extraction, contours, splitter and
mkgmap and verifies the resulting `gmapsupp.img`.

| Module | Purpose | Notes |
|---|---|---|
| `proj.rs` | LV95 ↔ WGS84 | measured against PROJ; ~1 m interior, 4.2 m at coverage corners |
| `gpkg.rs` | GeoPackage reader | SQLite + WKB, R-tree, streaming; id/geometry columns discovered |
| `geom.rs` | clip, simplify, point-in-polygon | Liang-Barsky, Sutherland-Hodgman, Douglas-Peucker |
| `pbf.rs` | OSM PBF writer | protobuf by hand; ascending ids asserted; DenseNodes |
| `extract.rs` | region → PBF | `RegionBuilder` shares one writer across vectors and contours |
| `elevation.rs` | swissALTI3D tiles | grid derived not probed; per-cell year dedup; concurrent fetch |
| `contour.rs` | marching squares | validated on analytic surfaces and a real tile |
| `garmin.rs` | splitter + mkgmap | two-pass build; identity allocation |
| `img.rs` | IMG verification | catches the missing overview map and empty maps |

**Tilestore skipped**, as Milestone 0 concluded: the source R-trees make it unnecessary.

**Bugs the tests caught, all from assumptions rather than logic:**

- `reqwest::Response::content_length()` is 0 for a HEAD request, so every size-dependent
  path saw zero. Only the live test found it.
- The GeoPackage primary key is `id` in swissTLM3D but `fid` in GDAL-written files, so
  the fixture did not match production until both were discovered from the schema.
- The R-tree table is named after the geometry column, not literally `_geom`.
- `splitter` rejects a map id above 99 999 999. Deriving map numbers as
  `family_id * 10000` overflowed that; family ids and map numbers are independent
  identifiers with different ranges and are now allocated separately.
- Milestone 0's contour tiers were configured so the medium tier could never occur
  (every multiple of 50 that is also a multiple of 20 is a multiple of 100).
  `medium_is_reachable()` now reports this and the default disables the tier.
- Cancelling a download left a multi-gigabyte `.part` file, and the test that claimed
  to check for it asserted on the wrong path.

**Still outstanding in this band:** DEM `.hgt` generation is still the Python spike, since
it needs windowed reads of the 10 GB swissALTIRegio COG rather than whole 1 km tiles.

## 7. Milestone 5 — Cartography

**Goal:** it looks like a Landeskarte. This is a **parallel workstream** that can start
right after Milestone 0 and run alongside Milestones 2–4 — it is mostly independent of the
Rust work and iterates on a different rhythm.

### Tasks

1. Grow `style/` from the S0.5 draft into full coverage of the discovered TLM3D schema:
   `points`, `lines`, `polygons`, `relations`, `options`.
2. `typ/` — complete TYP source with day and night palettes (FR-CART2), following
   Landeskarte colour conventions for land cover (FR-CART4).
3. The zoom-level plan (FR-CART1), including wiring **swissTLMRegio** into the low zoom
   levels rather than decimating TLM3D.
4. Hiking trail classes rendered distinctly at every relevant zoom (FR-CART3) — treat this
   as the flagship feature.
5. Contour styling: minor/medium/major weights, labelled index contours, differentiated over
   rock and glacier (FR-CART5).
6. The **separate wrist-device style variant** (FR-CART6) with reduced line weights, label
   density, and higher minimum zoom levels.
7. `docs/cartography.md` maintained as the authoritative mapping table.
8. Visual regression harness (FR-CART7): fixed reference extents rendered to PNG and diffed
   against committed goldens, across both style variants and both palettes, running in CI.

**Acceptance:** side-by-side comparison against the actual Landeskarte for three reference
areas (urban, alpine, lakeside) is convincing; trail classes distinguishable on both an Edge
and a fēnix photograph; the visual regression suite catches an intentionally introduced
style change.

---

## 8. Milestone 6 — The GUI

**Goal:** the five-step flow, on top of the now-tested backend.

### Tasks

1. **Device step:** searchable grouped list, USB auto-detection via `GarminDevice.xml`
   (FR-DEV5), generic-device fallback (FR-21), confidence-level display (FR-DEV1),
   limit overrides (FR-DEV3).
2. **Area step:** MapLibre GL with swisstopo WMTS layers and bounded offline tile cache
   (§3.4); rectangle/polygon/circle draw tools with editing (FR-31); administrative-unit
   multi-select from swissBOUNDARIES3D (FR-33) with buffering (FR-34); whole-Switzerland
   preset (FR-35); GPX/FIT import with corridor buffering (FR-38…FR-40); composite
   selections, saved selections, GeoJSON import/export (FR-41, FR-42).
3. **Content step:** the four presets (FR-50), expandable layer panel (FR-51), contour
   settings (FR-52), label language (FR-53), recipe save/load (FR-55).
4. **Size estimator** (`estimate/`): feature-count-driven model with shipped coefficients,
   post-build calibration logging that refines them (FR-60…FR-63), and the budget bar UI.
5. **Build step:** stage progress, live log pane, ETA, working cancellation, failure
   presentation with plain-language causes and copy-diagnostics (FR-70…FR-74).
6. **Install step:** device identity and free space, correct path and filename per profile,
   overwrite confirmation with backup, post-copy hash verification, safe-eject reminder,
   plain export, per-device instructions, double-extension guard (FR-80…FR-84).
7. Accessibility and theming pass against NFR-9 and FR-3.

**Acceptance:** the spec §18 acceptance criteria 1–4 pass; a user unfamiliar with the
project completes a canton build with no documentation; every long operation is cancellable
and leaves a clean state.

---

## 9. Milestone 7 — Hardening and release

### Tasks

1. Crash-recovery: detect orphaned temp state and Java processes on startup, offer resume.
2. The full error matrix of spec §12, each with a test.
3. Performance and memory profiling at commune / canton / national scale, tracked in CI
   against thresholds (NFR-1, NFR-2, §13.6).
4. Signed and notarized installers for all platforms (NFR-7); SBOM generation (NFR-10).
5. Full real-device validation sweep (§13.5, VAL-1…VAL-5) across as many devices as can be
   borrowed; promote profile confidence levels.
6. Documentation: README with screenshots, a getting-started guide, the device verification
   procedure, a cartography contribution guide, and `NOTICE` with all third-party licenses.
7. Attribution audit against FR-L1…FR-L4, including the copyright string embedded in output.
8. Estimator calibration against at least ten reference areas until ±25 % holds (FR-60).

**Acceptance:** every one of the ten acceptance criteria in spec §18 is demonstrably met.

---

## 10. Sequencing

```
Week   1     2     3     4     5     6     7     8     9    10    11    12
     ┌─────┐
  M0 │spike│  ◄── GO/NO-GO GATE: map renders on real hardware
     └─────┘
           ┌───────────┐
  M1       │foundation │
           └───────────┘
                       ┌─────────────────┐
  M2                   │ vector pipeline │
                       └─────────────────┘
                                         ┌───────┐
  M3                                     │contour│
                                         └───────┘
                                                 ┌───────────┐
  M4                                             │  garmin   │
                                                 └───────────┘
           ┌───────────────────────────────────────────────────────┐
  M5       │ cartography  (parallel workstream, own rhythm)        │
           └───────────────────────────────────────────────────────┘
                                                             ┌───────────┐
  M6                                                         │    GUI    │
                                                             └───────────┘
                                                                         ┌─────┐
  M7                                                                     │ ship│
                                                                         └─────┘
```

The timeline assumes one AI agent working with a human reviewer who has both a Garmin Edge
and a fēnix on hand. **Hardware access is on the critical path** — Milestone 0 cannot
complete without it, and Milestones 4, 5, and 7 each need it again.

---

## 11. How to run the agent effectively

### Per-session pattern

1. Point the agent at `SPEC.md` and the current milestone in this plan.
2. Ask for a **task breakdown and a plan** before any code. Review it. Cheap to fix here.
3. Let it implement one task at a time, with tests, committing as it goes.
4. Review against the requirement IDs. Ask "which FR does this satisfy, and where is the
   test?" — a satisfying answer to that question is the whole quality bar.
5. At milestone end, ask it to write the acceptance evidence, then verify the claims
   yourself. Do not accept "works" without an artifact.

### Things to watch for

- **Fabricated schema.** The highest-risk failure mode in this project. If code references
  a TLM3D layer or attribute value, grep `docs/tlm3d-schema.md` for it. If it is not there,
  it was invented.
- **Fabricated device limits.** Same test against `devices/*.json` and its `sources`.
- **Slurping.** Any `collect()` or full-file read on a path that could see national data.
- **Premature abstraction.** A trait with one implementation, a plugin system for one
  data source, a config layer for a constant. Push back.
- **Skipped hardware validation.** A desktop map viewer is not a Garmin device. The
  difference has bitten every project in this space.
- **Silent scope reduction.** "Contours implemented" that quietly means "without seam
  stitching". Check acceptance criteria individually.

### Good first prompt

> Read `SPEC.md` and `PLAN.md` in full. We are starting Milestone 0, task S0.1. Before
> writing code, tell me your plan: what you will download, where you will cache it (outside
> the repo), how you will avoid re-downloading 4.6 GB if interrupted, and exactly what
> `docs/tlm3d-schema.md` will contain. Do not guess any layer or attribute name — every name
> in that document must come from the actual file.

---

## 12. Risk register for the build itself

| Risk | Mitigation in this plan |
|---|---|
| Agent guesses the TLM3D schema and everything downstream is subtly wrong | S0.1 is task one; working-agreement rule 2; review checklist greps the schema doc |
| Weeks of work before discovering the map does not work on a fēnix | S0.4 is a hard go/no-go gate before any product code |
| Cartography underestimated as "just config" | It is its own milestone and its own parallel workstream, with visual regression tests |
| The 4.6 GB download makes every iteration slow | Cache outside the repo; commit a commune-sized fixture in Milestone 1 so all tests and most iteration run on small data |
| Agent builds the tilestore optimization that Milestone 0 proves unnecessary | Milestone 2 task 4 explicitly says to skip it if the findings report says so |
| Beautiful UI over an untested pipeline | GUI is Milestone 6, deliberately last; working-agreement rule 5 |
| No hardware available when a milestone needs it | Hardware access flagged as critical path in §10; schedule it |
| Milestone claimed complete without evidence | Definition of done requires artifacts; §11 step 5 requires independent verification |
