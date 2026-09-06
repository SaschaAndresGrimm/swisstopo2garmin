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

Then the limits in section B, which is what promotes `edge-840` from `community` to
`measured` confidence.

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

## B. Measuring the real limits

Only after A passes. Each measurement promotes a profile field from `community`/`assumed`
to `measured`.

### B.1 Maximum single `.img` size
Binary search. Build progressively larger areas (`spikes/s0/build.sh <Place> <radius>`)
and record the largest that still loads and renders. Suspected ceilings are 4 GB (FAT32)
and, on some models, 2 GB. Test across the suspected boundary, not just below it.

### B.2 Maximum tiles per mapset
Rebuild one area with decreasing `--max-nodes` in `splitter` to raise the tile count at
constant content. Record the count at which the device stops loading the map. Community
reports suggest ~4096 for Edge; unverified.

### B.3 Install paths and filenames
Test each of: `/Garmin/gmapsupp.img`, `/Garmin/Maps/<name>.img`, `/Garmin/<name>.img`, and
the SD-card equivalents. Record which the device accepts, and whether the literal filename
`gmapsupp.img` is required (`mapFile.requiresExactFilename`).

### B.4 Multiple mapsets
Install two maps with **different** family IDs and confirm both appear and can be toggled
independently. Then repeat with the **same** family ID to confirm the collision behaviour
the app must avoid.

### B.5 Storage budget
Record total and free space as the device reports it, and how much can actually be consumed
by maps before behaviour degrades.

---

## C. Recording results

Update the device's JSON profile with the measured values, set
`confidence.level = "measured"`, set `confidence.lastVerified`, and note the method in
`confidence.notes`. Append a dated entry below and open a pull request — community
measurements are accepted this way (FR-DEV4).

## D. Verification log

| Date | Device | Firmware | Result | Measured limits | By |
|---|---|---|---|---|---|
| _(pending)_ | | | | | |
