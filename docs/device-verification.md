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

---

## D. Recording results

Update the device's JSON profile with the measured values, set
`confidence.level = "measured"`, set `confidence.lastVerified`, and note the method in
`confidence.notes`. Append a dated entry below and open a pull request — community
measurements are accepted this way (FR-DEV4).

## E. Verification log

| Date | Device | Firmware | Result | Measured limits | By |
|---|---|---|---|---|---|
| _(pending)_ | | | | | |
