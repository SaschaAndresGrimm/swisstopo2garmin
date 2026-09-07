# Device verification procedure

Fills in `devices/*.json` with **measured** rather than **community** limits (SPEC.md
FR-DEV1…FR-DEV4), and gates Milestone 0 (PLAN.md §2, S0.4).

Garmin publishes none of these numbers. Forum figures are inconsistent and often stale —
the widely repeated "fēnix maps must be under 20 MB" is fēnix 3-era and does not apply to a
fēnix 7/8 that ships multi-gigabyte TopoActive maps. Measure, don't inherit.

---

## A. Smoke test (this is the Milestone 0 go/no-go gate)

Test maps in `out/`. **Install one at a time** — delete the previous
`gmapsupp.img` first — although they use different family ids (6324 / 6325) and
different overview map numbers, so they can coexist if you want to compare.

| File | For | Size | Cartography | DEM |
|---|---|---:|---|:--:|
| `gmapsupp-Grindelwald-edge840.img` | Edge 840 | 2.63 MB | handlebar | yes |
| `gmapsupp-Grindelwald-fenix.img` | fenix | 2.14 MB | wrist (reduced) | yes |

Both cover Grindelwald at an 8 km radius (256 km²) with 20 m contours, blue over ice.
The wrist build carries 20% less geometry and 73% fewer labels, and drops buildings,
parking and orchards.

### What specifically needs your eyes

| # | Check | Why it matters |
|---|---|---|
| 1 | **Shaded relief on the Edge 840** | The single largest step toward the raster Landeskarte look (FR-CART8). The map carries a 230 KB DEM subfile per tile. Unverified — DEM support varies by model, and if it does not work, porting DEM generation to Rust is wasted effort. |
| 2 | **Shaded relief on the fenix** | Same, and even less certain on a watch. |
| 3 | **Wrist legibility** | Is the reduced style readable, or still too dense? Line weights are 40% thinner and detail appears one zoom level later. This is a judgement call I cannot make from a desktop preview. |
| 4 | **Rock and wetland texture** | These are TYP bitmap pattern fills. The desktop preview cannot draw them at all — it approximates them as a flat tint — so hardware is the only way to see whether the hachures read as rock. |
| 5 | **Trail classes on a wrist screen** | Yellow / red-white / blue-white must stay distinguishable at 1.3 inch. |
| 6 | **Both maps installed together** | Confirms the family id and overview map number allocation avoids collisions (§4.6). |

Then the cartography sweep in section B, and the limits in section C — the latter is
what promotes `edge-840` from `community` to `measured` confidence.

### If the map lists but nothing draws

Work through these in order — the first two were real defects found on the first Edge 840 test:

1. **Are you actually looking at the covered area?** The map only has data inside its
   bounds. Search for `Grindelwald` and pan *to it*; a route-preview screen centred on your
   current position will legitimately show nothing.
2. **Overview map missing** (fixed in v2 — see m0-findings §4.5). Verify with
   `python3 spikes/s0/imgtre.py <file>`: the FAT must list a `…0000` overview map
   *and* the detail tiles.
3. **Other maps drawing on top.** Temporarily disable Trailforks and Base maps in
   Map Settings, leaving only swissTLM3D enabled, and re-check.
4. **Map not enabled.** Listed under "Other Maps" is not the same as enabled.
5. **Map ID collision.** Two installed maps sharing a family or overview map number
   (m0-findings §4.6). Install only one of ours at a time.

1. **Back up** any existing map. Connect the device by USB; it mounts as mass storage.
   Copy `/Garmin/gmapsupp.img` off the device first if one exists.
2. Copy the test map to the device as **`/Garmin/gmapsupp.img`**.
   On Windows with hidden file extensions, confirm you did not create `gmapsupp.img.img`
   (FR-84). On a device with an SD slot, `/Garmin/` on the card also works.
3. Eject safely, then power-cycle the device.
4. Record, **with photographs**:

   | Check | What to look for |
   |---|---|
   | Map appears | Listed in the device's map/activity map settings, and can be enabled |
   | Renders | Geometry visible around Grindelwald at several zoom levels |
   | **Trail classes** | Wanderweg (yellow), Bergwanderweg (red/white), Alpinwanderweg (blue/white) visually distinguishable — this is FR-CART3, the flagship feature |
   | Contours | Present, legible, not overwhelming; index contours labelled |
   | **Characters** | Names with umlauts render correctly — look for `Bäregg`, `Zäsenberg`, `Nällehüsli`, `Ägerteweid`. Mojibake or blanks here means the code page is wrong (FR-P10) |
   | Land cover | Glacier, rock and forest distinguishable |
   | Responsiveness | Panning and zooming not sluggish |
   | Coexistence | The factory map still works; no crash, no blank screen |
   | Search | Place search finds `Grindelwald` |

**The gate passes when at least one Edge and one fēnix/epix render the map and the trail
classes are distinguishable.** Anything less and the cartography or encoding needs work
before product code starts.

---

## B. Cartography sweep (added after Milestone 6)

Everything in this section has been built, compiled, style-checked and rendered on a
desktop — and none of it has been on a device. The desktop renderer is not Garmin's: it
draws the compiled map's own geometry with the same TYP colours, but it approximates
line casing and pattern fills and cannot show night mode at all. That is the gap these
files close.

Build them with:

```
sh tools/device_test_set.sh Grindelwald 6
```

Grindelwald at 6 km covers, in one 144 km² extent: the Eiger north face (slope classes
to over 50°), the Unterer Grindelwaldgletscher (blue contours over ice), SAC huts
(Gleckstein, Bäregg), two cableways and the Wengernalpbahn, bus stops through the
valley, and the town itself.

| File | Device | Content | Scheme | Relief | Slope |
|---|---|---|---|:--:|:--:|
| `edge-1-hiking-summer.img` | Edge 840 | hiking, 20 m | summer | gentle | – |
| `edge-2-slope-no-relief.img` | Edge 840 | hiking, 20 m | summer | – | yes |
| `edge-3-skimo-winter-slope.img` | Edge 840 | ski touring, 20 m | **winter** | gentle | yes |
| `edge-4-full-10m.img` | Edge 840 | full topo, 10 m | summer | gentle | – |
| `fenix-1-hiking-summer.img` | fēnix 5 Plus | hiking, 20 m | summer | gentle | – |
| `fenix-2-skimo-winter-slope.img` | fēnix 5 Plus | ski touring, 20 m | **winter** | – | yes |

### Installing them

The **Edge 840** holds several map sets, so all four `edge-*.img` files can sit in
`/Garmin/` at once under their own names, and be switched in the map settings. Each has
its own family id, so they will not collide.

The **fēnix 5 Plus** requires the exact name `gmapsupp.img` and holds one map set, so
copy one file at a time to `/Garmin/gmapsupp.img`, renaming it. Delete the previous one
first.

If either device is in MTP mode it will not mount as a drive — switch USB mode to
Garmin, or use the app's "Save to a folder" and copy with a transfer tool.

### What needs your eyes

| # | File | Check | Why hardware is the only way |
|---|---|---|---|
| 1 | `edge-3` | **Winter scheme.** Does the base map step back far enough for the violet SAC ski routes to read on top of it? | The colours are measured from swisstopo's Winter national map, but "the base steps back" is a judgement about legibility on a 2.6" transflective screen in daylight. |
| 2 | `edge-2` | **Slope hatch.** Are the five classes distinguishable, and does the map underneath stay readable through the hatch? | A Garmin TYP polygon has no alpha, so the classes are a 50% diagonal dither. Whether that reads as a tint or as visual noise cannot be judged from a PNG. |
| 3 | `edge-2` vs `edge-3` | **Slope with and without relief.** Is slope hatching legible over shaded relief, or does relief-plus-hatch become mud? | Both are rendered by the device, compositing in a way nothing on the desktop reproduces. |
| 4 | **all** | **Night mode.** Switch the device to night colours on each file. | Night colours are in the same `.img` — the device swaps to them, so there is no separate file. This palette is the only one in the project that is *derived* rather than measured (swisstopo publishes no night map), so it is the most likely to be wrong. Check especially: is the ground dark enough, do the pale road classes stay distinguishable, and do contours stay visible without glaring? |
| 5 | `edge-1` | **SAC huts and bus stops.** Are the hut and stop symbols distinguishable from each other and from settlements, and are their labels legible? | These POI types are new. The hut symbol is a 7×7 gable, the stop a hollow square; at device scale they may simply look like dots. This extent has 2 named SAC huts and 117 stops. |
| 6 | `edge-4` | **Density at 10 m contours with everything on.** Is full topo usable, or a wall of ink? | This is the largest content setting the app offers, and nobody has looked at it on a screen. |
| 7 | `fenix-1` | **Wrist cartography.** Trail classes still distinguishable at 1.3"? Labels readable? | The wrist variant thins lines to 60% and pushes detail a zoom level later; whether that is enough is a judgement. |
| 8 | `fenix-2` | **Winter and slope on a watch.** Does the winter scheme survive at wrist size, and is the slope hatch anything but noise at 1.3"? | The hatch is a fixed 32×32 pattern regardless of screen size, so this is where it is most likely to fail. |
| 9 | `edge-1` + `edge-3` | **Two schemes installed together.** Both in `/Garmin/`, switch between them on the device. | Confirms the colour scheme really is part of the map identity and the two do not overwrite each other. |
| 10 | **any** | **Blue contours over ice.** On the Unterer Grindelwaldgletscher, are the contours blue rather than bistre? | Verified in the data, never seen rendered. |

### Report back

For each numbered check: works, or does not, and if not what it looked like. Anything in
1–4 that fails is a cartography change rather than a bug; 5, 6 and 10 would be data or
style faults.

The sizes and feature counts are in the `.manifest.json` beside each file, along with
the dataset releases and tool versions that produced it.

---

## C. Measuring the real limits

Only after A passes. Each measurement promotes a profile field from `community`/`assumed`
to `measured`.

### C.1 Maximum single `.img` size
Binary search. Build progressively larger areas (`spikes/s0/build.sh <Place> <radius>`)
and record the largest that still loads and renders. Suspected ceilings are 4 GB (FAT32)
and, on some models, 2 GB. Test across the suspected boundary, not just below it.

### C.2 Maximum tiles per mapset
Rebuild one area with decreasing `--max-nodes` in `splitter` to raise the tile count at
constant content. Record the count at which the device stops loading the map. Community
reports suggest ~4096 for Edge; unverified.

### C.3 Install paths and filenames
Test each of: `/Garmin/gmapsupp.img`, `/Garmin/Maps/<name>.img`, `/Garmin/<name>.img`, and
the SD-card equivalents. Record which the device accepts, and whether the literal filename
`gmapsupp.img` is required (`mapFile.requiresExactFilename`).

### C.4 Multiple mapsets
Install two maps with **different** family IDs and confirm both appear and can be toggled
independently. Then repeat with the **same** family ID to confirm the collision behaviour
the app must avoid.

### C.5 Storage budget
Record total and free space as the device reports it, and how much can actually be consumed
by maps before behaviour degrades.

### C.6 Unplug during an install

The one row of SPEC.md §12 that no test can reach honestly: the code is tested against an
injected copy failure, which is a guess at what a real device does when the cable leaves
its socket. macOS, Windows and Linux each report it differently, and the interesting
question is whether the app's cleanup can still run at all once the volume is gone.

Install a map and **pull the cable while the copy bar is moving** — the window is a couple
of seconds on a 1 MB map, so use the largest map you have. Then reconnect and record:

1. What the app said. It should name the failure, and either say the device is clean and
   retrying is safe, or name the exact leftover file.
2. What is actually in `/Garmin/` on the device. A `gmapsupp.img.part` (or
   `<name>.img.part`) is the expected leftover; a truncated `gmapsupp.img` is a **defect**
   — the copy is written to `.part` and renamed precisely so that cannot happen.
3. Whether a previous map that was being replaced is still there, under its own name or
   as `.bak`. Losing an existing map for one that never arrived is the worst outcome.
4. Whether retrying the install then works without any manual cleanup.

Repeat once with "back up the existing map" ticked and once without.

### C.7 Routing

Routing is off by default and labelled unverified in the app because of this section.
Build a map with **Turn-by-turn routing** ticked ([docs/routing.md](routing.md)) and check,
in this order — the first item is the one that matters:

1. **Direction on a dual carriageway.** Ask the device to route along a stretch of
   motorway or a divided main road and confirm it goes *with* the traffic. This is the one
   assumption that can hurt somebody: `richtungsgetrennt=Wahr` becomes `oneway=yes`, and
   that is only right if swisstopo digitises in the direction of travel. It is verified
   against the data — motorway ramps agree 100 % of the time in the cases clear enough to
   judge — and **not** verified on a device. If it is wrong, turn routing off and say so
   immediately.
2. **A route is offered at all.** Pick two points on the road network and ask for a route.
   Nothing at all means NET/NOD did not reach the device, or the device does not accept a
   third-party routable map.
3. **Trails are usable on foot.** A hiking route along a `2m Weg` should be offered in
   pedestrian mode and refused for a car.
4. **Restricted roads are not used as shortcuts.** A forest track with
   `Allgemeine Verkehrsbeschraenkung` should be reachable as a destination but not appear
   in a through route.
5. **A via ferrata is never routed along.** `Klettersteig` carries no road class at all, so
   the device should refuse to route on it even in pedestrian mode.
6. **Steps and fords** are offered on foot and not to a bicycle.
7. **Coexistence.** A routable map alongside the factory map: no crash, no double
   guidance.

Record what the device did, not what it should have done.

### C.8 Address search

Build a map with **Address search** ticked ([docs/addresses.md](addresses.md)) and check:

1. **An address resolves.** Search a street and house number that exists in the area —
   the manifest lists how many addresses went in, and the register is the authority on
   what they are. "Dorfstrasse 1" style searches are the point.
2. **It lands in the right place.** Compare against the coordinates in the register.
3. **The postcode and locality show.** They are indexed as `mkgmap:postal_code` and
   `mkgmap:city`; if they are missing, a street name shared between villages is
   ambiguous on the device.
4. **A street with no house number is still findable** by name alone.

If nothing is searchable at all, the likely cause is that `addr:street` and the road's
`mkgmap:street` did not match — which is invisible in the map and would mean the 150 m
association never happened.

### C.9 Raster overlay (paper map)

**This is the section the whole feature rests on.** Everything else about the overlay is
verified — the KMZ is structurally valid, the georeferencing is checked against known
coordinates, the tile budget is respected — but *whether a device draws it at all* is
unknown, and SPEC.md §1.4 spent the project believing it would not. Both profiles
advertise a `Garmin/CustomMaps` directory; that is evidence, not proof.

Build a small map with **Paper-map overlay** ticked ([docs/raster.md](raster.md)) — a
2 km radius is 16 tiles and downloads in about 15 s — install it, and check in order:

1. **Does the device list it?** Look for a custom map entry in the map manager. If the
   device does not acknowledge the file, nothing below matters and the answer to §8.1's
   open question is "no". Record that; it is a real result and it retires the feature.
2. **Does it draw?** Pan to the area and zoom in. Custom Maps are reported to render
   only in a narrow zoom band, so sweep the whole range before concluding it is blank.
3. **Is it in the right place?** This is the one that would be invisible in testing and
   dangerous in use. Compare a distinctive feature — a road junction, a building corner
   — against the vector map underneath. A systematic offset means the `LatLonBox`
   georeferencing is wrong; the likely cause would be the axis-order or projection
   assumptions in FR-R3, and the offset's direction says which.
4. **Do the tiles line up?** Look along the seams between tiles for gaps or overlaps.
5. **Is it legible?** 1.25 m/px is the source's native resolution. If the device
   re-samples it to mush, record the tile size at which it stops doing so — that is a
   `measured` value for `maxCustomMapPixelsPerTile`.
6. **How many tiles before it stops?** The 100-tile figure is community-sourced and
   applies across all custom maps on the device. Install overlays until one fails to
   appear, and record the count. This is the single most valuable number to measure,
   because it sets the largest area the feature can cover.
7. **Does it slow the device down?** Pan and zoom with the overlay on and off. An
   overlay that makes the map unusable is worse than no overlay.

Whatever the outcome, update `maxCustomMapTiles` and `maxCustomMapPixelsPerTile` in the
profile and set `confidence.level` accordingly — including setting them to `null` if the
device turns out not to support overlays at all, which is what FR-R4 requires so that
the app then offers no overlay rather than a broken one.

---

## D. Recording results

Update the device's JSON profile with the measured values, set
`confidence.level = "measured"`, set `confidence.lastVerified`, and note the method in
`confidence.notes`. Append a dated entry below and open a pull request — community
measurements are accepted this way (FR-DEV4).

## E. Verification log

Transcribed from the `confidence.notes` fields of `devices/*.json` and the Milestone 0
findings, where these results were recorded at the time. VAL-5 asks for them here, and
having them in two places and not this one meant the log read as though no hardware had
ever been touched.

| Date | Device | Firmware | Result | Measured limits | Source |
|---|---|---|---|---|---|
| 2026-09-06 | Garmin Edge 840 | 3133 (part 006-B4062-00) | Map lists and renders; mixed-case Swiss labels correct; DEM shaded relief **does** render, but is too dark at 1 arc-second in alpine terrain. `GarminDevice.xml` advertises `Garmin/CustomMaps` and `Garmin/BirdsEye`. | none — `maxImgBytes` and `maxTilesPerMapset` still `community` | `devices/edge-840.json`, m0-findings §4.5, §4.14, §4.16 |
| 2026-09-06 | Garmin fēnix 5 Plus | 1930 (part 006-B3110-00) | Wrist cartography renders correctly and legibly; coexists with the factory map (VAL-3). `GarminDevice.xml` advertises `Garmin/CustomMaps` and `Garmin/BirdsEye`. | none — and whether DEM relief renders on this generation is still unknown | `devices/fenix-5-plus.json` |

### Findings from hardware, 2026-09-07 (Edge 840, firmware 3133)

**Every map this project has ever built was listed as "OSM street map".** Photographed
in the Edge's Other Maps manager: three test maps, all named `OSM street map`, with only
their sizes to tell them apart.

mkgmap needs the names on **both** of its passes and nothing carries over from the first.
Pass 2 — the `--gmapsupp` one — was given `--family-id` and `--product-id` and no names
at all, so mkgmap wrote its own defaults. Confirmed by reading the files: they contained
`OSM map`, `OSM map set` and `OSM street map`, and none of ours.

**Which field a device shows**, established from the photograph rather than guessed: the
Edge displayed "OSM street map", and that string is mkgmap's default for `--description`
and for no other option (found in `CommandArgsReader.class`, beside the option name). So
the description is what an Edge lists.

That settled the design. The description is now the map's **name and nothing else**, and
the attribution moved to `--copyright-message`, the field Garmin has for it — appending
the copyright to the description made every row read "Grindelwald ski touring (c)
swisstopo", and at 27 characters of suffix it also left too little room for the name.

Three format facts came out of it, all measured rather than assumed:

* **The description is capped at 50 characters.** mkgmap refuses the build outright:
  `IllegalArgumentException: Description is too long (max 50)` from
  `ImgHeader.setDescription`. One of the six test maps would not compile until the name
  was clamped.
* **The header stores it in two chunks** — 20 bytes at offset `0x49` and 30 more at
  `0x65`, space-padded. So `strings` on a finished map shows an alarming 20-character run
  and the rest of the name a few bytes later. Nothing is truncated at 20; 20 + 30 is the
  50. `the_img_header_holds_its_description_in_two_chunks` pins the layout, because the
  obvious reaction to that 20-character run is to "fix" a truncation that is not there.
* **The gmapsupp's map-set block has its own name**, set by `--x-mapset-name`. That
  option is in neither `mkgmap --help=options` nor the bundled help, and mkgmap validates
  option names against its own documentation and so rejects `--mapset-name` outright. It
  was found as a string inside `GmapsuppBuilder.class`; `x-` is mkgmap's escape hatch for
  undocumented options.

`builds_a_verified_gmapsupp_from_the_fixture` now asserts the description and the family
name are both in the compiled file, that the copyright is, and that none of mkgmap's three
defaults is.

### Still outstanding

Both profiles remain at `community` confidence, because a smoke test is not a
measurement. Nothing in section C has been done on either device, so every size and tile
limit in both profiles is inherited from forum reports with a safety factor applied.

Neither device has seen anything added after Milestone 6: the winter palette, slope
classes, SAC hut details, transit stops, label language, or the night palette — which is
the one most likely to be wrong, being the only palette in this project derived rather
than measured from a swisstopo product. Section B exists for exactly that sweep, and the
six files it refers to are built and waiting in `out/device-test/`.

VAL-4 — the whole-Switzerland multi-map-set output on a real device — has not been
attempted at all.

The raster overlay (C.9) is the largest single unknown in the project. It is fully built
and its output is verified as a *file*, but the question it exists to answer — do these
devices render Custom Maps? — has never been put to a device. Until it is, the feature
should be described as built and unproven, and it stays off by default.
