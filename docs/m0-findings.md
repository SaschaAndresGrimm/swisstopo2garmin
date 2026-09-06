# Milestone 0 — findings report

**Date:** 2026-09-06 · **Status:** ALL SPIKES COMPLETE — **S0.4 go/no-go gate PASSED**

Answers the questions PLAN.md §2 said had to be settled before product code.
Every number here was measured on this machine (M-series Mac, ~40 MB/s downlink),
not estimated.

---

## Verdict

The architecture holds. Three of the four unknowns are resolved favourably, one
assumption was wrong in our favour, and one performance target in SPEC.md needs revising.

**The go/no-go gate (S0.4) is PASSED.** A map built by this pipeline renders on a Garmin
Edge 840: swissTLM3D land cover, the hiking network, contours and mixed-case Swiss place
names all display correctly. Milestone 1 may begin.

Getting there required fixing four defects that each presented identically as "the map
lists on the device but draws nothing" — see Findings 4.5 to 4.11. The most instructive is
4.9: the map was of the wrong place entirely.

---

## 1. Data acquisition

| Fact | Value |
|---|---|
| Latest release | `swisstlm3d_2026-02` (2026-02-24) |
| Archive | 4,795,840,706 B (4.80 GB), ZIP64, **one** DEFLATE member |
| Member | `SWISSTLM3D_2026_LV95_LN02.gpkg`, **10,777,276,416 B (10.78 GB)** |
| Compression ratio | 44.5 % |
| Download + inflate | **1.9 min at 39.6 MB/s**, sha256 verified |

**Finding 1.1 — the archive is DEFLATE, not STORED.** There is no random access into the
GeoPackage inside the zip, so it must be materialised on disk. Download-then-unzip peaks at
15.6 GB.

**Finding 1.2 — streaming inflate cuts peak disk to 10.78 GB.** `spikes/s0/fetch_tlm3d.py`
streams the archive once, feeds the member's byte range into a live `zlib` decompressor, and
writes the GeoPackage directly. The compressed archive is never stored. Full-file sha256 is
computed in the same pass. **Production should do this too** (it is also what makes the
10.78 GB fit on a machine with ~21 GB free).

**Finding 1.3 — resume has a caveat.** Raw DEFLATE state cannot be serialised, so a *process*
restart must re-download. Network blips are handled by reconnecting with `Range` into the
same live decompressor. To survive process restarts, production needs incremental
inflate-and-discard with periodic checkpoints, or it must accept re-downloading.

**Finding 1.4 — `file:checksum` is a multihash.** `1220…` = sha2-256 (0x12), 32 bytes.
Strip the two-byte prefix and compare hex. FR-D1 is directly implementable.

---

## 2. swissTLM3D schema

Full detail in [tlm3d-schema.md](tlm3d-schema.md) (generated). **41 tables — 32 spatial,
9 attribute/join — 21,730,349 features.**

**Finding 2.1 — the prior-art layer names are correct.** `tlm_strassen_strasse`,
`tlm_bb_bodenbedeckung`, `tlm_gewaesser_*` all exist as guessed. SPEC.md §3.1's warning was
appropriate but the risk did not materialise. Note the inconsistency `tlm_name_gelaendename`
(singular) vs `tlm_namen_*` (plural) — a real trap.

**Finding 2.2 — the hiking classification is an attribute, not a layer.** This is the single
most important discovery for FR-CART3. `tlm_strassen_strasse.wanderwege`:

| Value | Features | Swiss signage |
|---|---:|---|
| `Wanderweg` | 318,572 | yellow |
| `Bergwanderweg` | 87,997 | red/white |
| `Alpinwanderweg` | 2,695 | blue/white |

No separate hiking layer is needed for v1; `ch.swisstopo.swisstlm3d-wanderwege` is a
refinement, not a prerequisite.

**Finding 2.3 — `k_W` is a no-data sentinel**, not a value. It appears across many columns
(`belagsart`, `verkehrsbedeutung`, `befahrbarkeit`, …). Style rules must never match it, and
the converter must filter it alongside `NULL` and `Keine Angabe`.

**Finding 2.4 — `tlm_bb_einzelbaum` is 11,494,720 individual trees**, 53 % of all features.
It is meaningless at Garmin zoom levels and must be excluded by default. Anything that
naively iterates "all spatial layers" doubles its work for nothing.

**Finding 2.5 — attributes needed for cartography are all present and clean:**
`objektart` (22 road classes, 15 land-cover classes), `kunstbaute` (bridge / tunnel / stairs /
gallery / ford), `belagsart` (Hart / Natur), `stufe` (level −5…4, for bridge/tunnel layering),
`verkehrsbeschraenkung` (incl. `Panzerpiste`, `Gesicherte Kletterpartie`), and
`strassenname` directly on the feature.

**Finding 2.6 — R-tree spatial indexes DO exist** for all 32 spatial layers, declared in
`gpkg_extensions` as `gpkg_rtree_index`. Do **not** try to infer them by splitting
`rtree_*` object names on `_` — the layer names contain underscores, which produced a
false negative in the first version of the dump script.

---

## 3. Performance — and one spec correction

| Operation | Measured | Extrapolated to Switzerland (41,285 km²) |
|---|---|---|
| Acquire + inflate | 1.9 min (one-off) | — |
| Schema profile, all 21.7 M features | 2.0 min | — |
| **Vector clip, 144 km², R-tree** | **2.1 s** | ~10 min |
| Contours, 144 km², 20 m | 106 s | **~8.5 h** |
| splitter + mkgmap | 4 s | — |

**Finding 3.1 — the tilestore intermediate is probably unnecessary.** SPEC.md §4.2 and §7.2
assumed a per-release tiled intermediate was needed to avoid re-scanning 4.6 GB per build.
With the R-tree present, a 144 km² clip takes **2.1 s**, extrapolating to ~10 min nationally.
Milestone 2 task 4 should be **skipped** unless canton-scale measurement contradicts this.
The one thing the tilestore also provided — per-cell feature counts for the size estimator
(FR-61) — can be obtained instead from cheap R-tree range counts.

**Finding 3.2 — contour generation breaks the NFR-1 canton budget.** At 0.74 s/km² a
2,000 km² canton needs **~25 min** for contours alone, against NFR-1's 10-minute target for a
whole canton build. The work is network-bound on 181 COG reads per 144 km², single-threaded
in `gdal_contour`. Mitigations, in order of value: parallel tile fetch (production Rust
should fetch 8–16 tiles concurrently), a persistent elevation tile cache, and generating
contours once per region rather than per build. **NFR-1 must be restated** — see §6.

**Finding 3.3 — `gdalbuildvrt` over `/vsicurl` is unusable.** It opens every remote file to
read its geotransform; 362 tiles had not finished after several minutes. Because swissALTI3D
is a strict 1 km grid (cell `EEEE-NNNN` → origin `(EEEE*1000, (NNNN+1)*1000)`, 500×500 px at
2 m, Float32, nodata −9999), the VRT can be **derived with zero remote reads**: build time
went from minutes to **0.00 s**. Production must construct the mosaic descriptor from the
grid, never by probing.

**Finding 3.4 — swissALTI3D serves multiple acquisition years per cell.** The Grindelwald
bbox returned 362 assets for 181 cells: every cell exists for both 2019 and 2022. Feeding
all of them to a mosaic doubles bytes read and makes elevation depend on source ordering.
**Deduplicate by cell, keeping the newest year.**

---

## 4. Garmin toolchain

`mkgmap r4924` + `splitter r654` + Temurin JRE 17.0.20.1, all vendored by
`vendor/fetch_tools.py`, no system install. Validates FR-P11/FR-P12 and M1 task 9.

**Finding 4.1 — splitter requires ASCENDING node ids.** Negative descending ids (the OSM
convention for unsaved data) fail with *"Node ids are not sorted"*. This constrains FR-P4:
deterministic ids derived from TLM UUIDs must be **sorted before splitting**, or emitted
via a writer that sorts.

**Finding 4.2 — `--name-tag-list` does NOT populate labels from arbitrary tags.** This cost
real debugging time and is the kind of thing that silently ships a map with no names.
Passing `--name-tag-list=tlm:name,…` changed the LBL subfile by **zero bytes**. Labels
require an explicit style action:

```
tlm:strassenname=* { name '${tlm:strassenname}' }
tlm:name=*         { name '${tlm:name}' }
```

A rule with actions and no `[type]` performs the action and falls through. With it,
LBL went 4,003 → 33,348 B and the searchable MDR index 575 → 37,741 B.

**Finding 4.3 — cp1252 preserves Swiss characters.** `Ä` (0xC4) and `Ü` (0xDC) appear
correctly in the MDR index. Labels are stored uppercased in 5-character index chunks, so
plain `strings` on the IMG is a poor test. **This is not yet proof of correct rendering —
VAL-2 on hardware remains required.**

**Finding 4.5 — `--gmapsupp` does NOT include the overview map (Edge 840 rendering bug).**
The first device test (Edge 840) listed the map correctly at 1 MB but drew nothing.
Decoding the container showed the cause: a single-pass
`mkgmap --gmapsupp --overview-mapname=…` writes the overview map as a *separate*
`s2govm.img` and ships a gmapsupp containing **only** the detail tile. Garmin devices use
the overview map to locate a supplementary map's coverage.

The fix is a **two-pass build**:

```
# pass 1: tiles + overview + tdb  (NOT --gmapsupp)
mkgmap --tdbfile --index --overview-mapname=ovm --overview-mapnumber=<unique> …  tiles/*.pbf typ.txt
# pass 2: combine into the supplementary map
mkgmap --gmapsupp --index --family-id=… <tiles>.img ovm.img *.typ
```

After the fix the gmapsupp contains both the overview map and the detail tiles, and the
MPS subfile grows to describe them all. Production (Milestone 4) must do the two passes.

**Finding 4.6 — overview map numbers must be unique per mapset.** `--overview-mapname`
alone leaves the overview at mkgmap's default map number (`63240000` in both of our
builds), so installing two of our maps side by side collides. Always set
`--overview-mapnumber` explicitly, derived from the same allocation as the family id
(SPEC.md §5.1).

**Finding 4.7 — other enabled maps can hide ours.** The test device had Trailforks (199 MB)
and Base maps (47 MB) enabled over the same area. Garmin composites enabled maps, so a
custom map can be drawn under them. `--draw-priority=30` (default 25) raises ours above.
This remains a hypothesis until confirmed on hardware.

**Finding 4.8 — a TYP without `[_drawOrder]` hides every polygon.** mkgmap's
`doc/typ-compiler.txt` states: *"If a polygon type is not listed in this section, then
it will not be displayed at all."* Our first TYP had no `[_drawOrder]` at all, so
forest, rock, glacier, water and buildings were all discarded at render time while
lines still drew. Critically, **this does not change the IMG content** — the polygons
are in the RGN either way — so container inspection cannot detect it, and it presents
as a rendering failure rather than a styling bug. `spikes/s0/checkstyle.py` now fails
the build when the style emits a polygon type the TYP's drawOrder omits.

**Finding 4.9 — place names are NOT unique, and this invalidated every early build.**
`tlm_namen_siedlungsname_zentrum` contains **two** features named `Grindelwald`: the
2,000-9,999 inhabitant village at LV95 E2645921/N1163748 (46.6234N 8.0382E) and a
**<20 inhabitant hamlet** at E2649992/N1209309 (47.0329N 8.0964E), 45 km north near
Sursee. A `SELECT ... LIMIT 1` with no ordering picked the hamlet, so every map built
before this fix covered the wrong valley. The first Edge 840 test therefore showed
nothing at Grindelwald because the map genuinely had no data there.

Lesson for production: **a place-name lookup must disambiguate**, not pick arbitrarily.
The spike now ranks candidates by `einwohnerkategorie` and prints every match. The GUI
(FR-33) must present the choice to the user rather than guessing.

**Finding 4.10 — labels default to UPPERCASE and ASCII.** Without `--lower-case`,
mkgmap writes labels in the 6-bit uppercase format: `Rüedihus` became `RUEDIHUS`.
With `--code-page=1252 --lower-case` both case and Swiss characters survive --
464 labels in the Grindelwald tile contain non-ASCII (`Ahorezüün`, `Beesibärgli`,
`Bleuerflüö`). This is much stronger evidence for FR-P10 than the earlier MDR-index
inspection, though on-screen rendering still needs VAL-2.

**Finding 4.11 — mkgmap does emit the background polygon.** The dump shows one
`0x4b` shape per tile covering the full tile bounds, so the style does not need to
generate it -- but the TYP must still list `0x4a`/`0x4b` in `[_drawOrder]` and define
their colour, or the map has no base sheet.

**Finding 4.12 — multilingual place names are pipe-separated in one field.**
`tlm_namen_siedlungsname_zentrum.name` holds every language variant joined by ` | `:
`Bern | Berna | Berna | Berne`, `Genève | Genevra | Genf | Ginevra`,
`Zermatt | Praborgne`. Two consequences, both invisible while testing on monolingual
Grindelwald:

* A label emitted verbatim renders on the device as `Bern | Berna | Berna | Berne`.
* An exact-match place search finds neither `Bern` nor `Berne`, so major cities are
  simply unfindable.

The first variant is the local name — German-speaking Bern leads with `Bern`,
French-speaking Genève with `Genève` — so that is what gets rendered; the remainder are
kept under `alt_name` for search. Also affects `tlm_strassen_strasse.strassenname`.

**Finding 4.13 — mkgmap rewrites the TYP's FID to `--family-id`.** The `FID=` line in
a TYP source is therefore not a landmine: a build with a different family id still gets
its styling, and the compiled TYP carries the build's family id at offset 0x2f. Verified
by compiling a TYP declaring 6324 into a `--family-id=6399` build and finding 6399, not
6324, in the output.

**Finding 4.14 — DEM shaded relief works on the Edge 840, but 1 arc-second is too
dark.** Verified on hardware (firmware 3133): the device renders shaded relief from the
embedded DEM, which confirms FR-CART8 was worth building. At 1 arc-second (~30 m) over
alpine terrain the shading is heavy enough to darken the whole map and mute the
Landeskarte palette. mkgmap exposes no shading-intensity control — the device computes
it — so the only lever is DEM resolution. A 3 arc-second (~90 m) build is 445 KB → 71 KB
of DEM and should shade far more gently.

**Finding 4.15 — default POI icons clutter the map.** swissTLM3D contributes thousands
of Flurnamen; with Garmin's default icon for the point type they render as a field of
circles, which the Landeskarte does not do — it sets those names as text alone. Fixed by
defining the point types in the TYP with a fully transparent 1x1 icon, which keeps the
label and drops the symbol.

**Finding 4.16 — the Edge 840 advertises CustomMaps and BirdsEye directories.**
`GarminDevice.xml` lists `Garmin/CustomMaps` and `Garmin/BirdsEye` among its supported
paths. This contradicts the common claim, repeated in SPEC §1.4, that Edge devices have
no raster map support at all. The directory existing does not prove KMZ overlays render,
but given that visual fidelity to the swisstopo raster is a headline requirement, it is
worth testing before accepting the vector-only conclusion.

**Finding 4.17 — text-only attribute access silently dropped every numeric tag.**
`Value::as_meaningful_str` returned only `Value::Text`, so INTEGER and REAL columns
never became tags. `ski_network.access` (0 skiable / 1 carrying / 2 caution),
`ski_routes.difficulty` and every altitude field are INTEGER, so the style rules keyed
on them could not match and the map lost the distinction **with no error anywhere** —
the rules were correct, the data was present, and the output was simply poorer.
`Feature::tag` now renders numbers, and formats whole REALs without a `.0` so a rule
written against the integer form still matches.

**Finding 4.18 — winter route datasets are all GeoPackage; cycle datasets are not.**
`ch.swisstopo-karto.skitouren` (10,789 SAC tours plus a 19,915-segment network),
`ch.astra.schneeschuhwanderwege` (280) and `ch.astra.winterwanderwege` (507) are
GeoPackage, so the existing reader handles them. `ch.astra.veloland`,
`ch.astra.mountainbikeland` and `ch.astra.wanderland` publish **shapefile and File
Geodatabase only** — no GeoPackage — so cycle routes need a shapefile reader first.
The generic `veloland.zip` asset contains only PDFs.

**Finding 4.19 — swissTLM3D has no cycle route data at all.**
`tlm_strassen_strassenroute` looks like a route network but is motorway numbering
(`Nationalstrasse`, `Hauptstrasse A/B/C`, `HLS`). The road layer carries `wanderwege`
for hiking but nothing equivalent for cycling, so bike routes cannot come from
swissTLM3D.

**Finding 4.20 — layer names can carry the release year.** `ski_routes_2056` and
`ski_network_2056` embed the coordinate-system code, and other swisstopo layers embed
dates, so an exact-match layer lookup breaks on the next release. Layers are resolved
by exact name first, then by prefix.

**Finding 4.21 — all three ASTRA datasets ship a file named `Route.shp`.**
`veloland`, `mountainbikeland` and `wanderland` each contain `Route.shp`, `Etappe.shp`
and a network file. Tagged by file stem alone, a mountain-bike route is
indistinguishable from a cycle route and renders in the wrong colour. The layer tag is
therefore qualified with the dataset directory. A useful side effect: `wanderland_Route`
supplies the **official hiking route numbers**, which swissTLM3D does not carry.

**Finding 4.22 — `Netzhier` is empty in the real route data.** Both `VeloWeg` (88,939
records) and `MTBWeg` (52,316) declare a 254-character `Netzhier` field that looks like
a network hierarchy — exactly what a cycle map wants for grading route importance — and
it is empty in every record. The cycle network is therefore drawn as a single class.
`MTBWeg.IsSTrail` does carry signal: 4,802 segments are singletrail.

**Finding 4.4 — IMG header offsets** (for the Stage 6 verifier): `DSKIMG` at **0x10**,
`GARMIN` at 0x41, description at 0x49, block-size exponents E1/E2 at 0x61/0x62, FAT at
0x600. Several online references place `DSKIMG` elsewhere; the above is measured.

---

## 5. Output size — the estimator's first calibration point

Grindelwald, 6 km radius, 144 km², 31,227 vector features + 3,016 contour ways:

| Build | `gmapsupp.img` |
|---|---:|
| Vector only, no labels | 928,768 B |
| Vector + labels | 996,433 B |
| **Vector + labels + 20 m contours** | **1,174,528 B** |

Contours add only **17 %**, because Douglas–Peucker at 8 m cut 1,993,436 → 55,908 points
(**97 % reduction**) with no visible loss at Garmin's ~2 m coordinate resolution.

**Finding 5.1 — Switzerland extrapolates to ~336 MB.** Comfortably inside every modern
Edge and fēnix budget, and far below the 4 GB single-file ceiling. The whole-Switzerland
preset (FR-35) likely does **not** need multi-map-set splitting on current devices —
FR-36 becomes a safety net rather than the common path. Contour interval is the main
size lever: 10 m roughly doubles the contour contribution.

---

## 5b. Dataset acquisition — three bugs the spikes hid

The Milestone 5 spikes fetched the winter and route datasets with Python, into flat
`winter/` and `routes/` directories. The app writes the content-addressed layout
`<cache>/<collection>/<item>/<file>`. Everything worked on a machine that had run the
spikes and would have failed on a user's.

**Finding 5.2 — the pipeline searched only the spike layout.** Winter and cycle data
acquired through the app was downloaded, verified, recorded, and then never found by a
build. The content step told users to fetch data that the build could not read. Both
layouts are now searched (`crates/s2g-core/src/datasets.rs`).

**Finding 5.3 — the SAC skitouren archive holds two GeoPackages.**
`skitouren_2056.gpkg.zip` contains `ski_routes_2056.gpkg` (21.8 MB) *and*
`ski_network_2056.gpkg` (13.5 MB). The streaming inflater resolves the archive's *first*
member, so acquiring this collection through the app would have silently dropped the ski
network — the layer carrying the skiable / carrying / caution classification, which is
the distinction that matters in the field. Only swissTLM3D is streamed now; it is the
one archive where peak disk actually matters (4.5 GB compressed against 10.0 GB
inflated) and the one that really holds a single member.

**Finding 5.4 — the ASTRA archives are written as streams.** Local file headers have bit
3 of the flags set and zero sizes, with the real values in a trailing data descriptor.
A reader that walks local headers fails on `Etappe.cpg`, the first zero-length member.
`zip::extract_all` reads the central directory instead. `veloland_2056.shp.zip` is
54,431,691 B and expands to 15 files, the largest being `VeloWeg.dbf` at 283.5 MB.

**Finding 5.5 — both layouts on one machine means every route drawn twice.** Discovery
now keeps one file per dataset — for routes, per dataset *and* layer, because all three
ASTRA datasets ship a `Route.shp` — preferring the app layout and the newest release.

---

## 5c. Corridor masking

The extractor clips to a bounding box. That is exactly right for a rectangle or a place
radius and wrong for anything else: the bounding box of a transalpine route is most of
Switzerland.

Jungfrau region route, 20 km, 3 km buffer, Edge 840, 20 m contours, gentle relief:

| Build | Features | `gmapsupp.img` |
|---|---:|---:|
| Corridor | 23,636 | 1,070,080 B |
| Same extent as a plain rectangle | 35,094 | 1,325,568 B |

**Finding 5.6 — masking contours is what makes a corridor small.** At a 3 km buffer both
builds produced 9,195 contour ways: in that terrain every contour line touches the
corridor, because contours follow the valley walls the route follows. At a 0.5 km buffer
the count falls to 3,192. Contours are the dominant size term, so a mask that filtered
only vector features would leave most of the saving on the table.

---

## 6. Changes required to SPEC.md

1. **NFR-1** — canton budget is not achievable with serial contour generation.
   Restate as: canton vector build ≤ 2 min; canton contours ≤ 10 min **with parallel tile
   fetch and a warm elevation cache**; cold contour builds may take longer and must show
   an honest estimate.
2. **§4.2 / §7.2** — demote the tilestore from required to conditional (Finding 3.1).
3. **§7.4** — add per-cell year deduplication for swissALTI3D (Finding 3.4) and grid-derived
   mosaic construction (Finding 3.3).
4. **FR-P4** — add the ascending-id-ordering constraint (Finding 4.1).
5. **§8** — record that labels need explicit `{ name … }` actions (Finding 4.2).
6. **FR-36** — downgrade expected frequency; national output fits one file (Finding 5.1).

---

## 7. What is still unknown

- **S0.4, the go/no-go gate.** No map has been on a device. Rendering, legibility at
  wrist size, label encoding on screen, coexistence with the factory map, panning
  performance, and the real size/tile limits are all unverified.
- **Cartography quality.** The TYP file compiles and the three trail classes have distinct
  colours, but nobody has looked at the result on a screen.
- **Contour seam continuity.** The single derived-grid mosaic should make seams a non-issue,
  but this has not been visually confirmed.
- **Canton-scale behaviour.** All timings extrapolate from 144 km².
