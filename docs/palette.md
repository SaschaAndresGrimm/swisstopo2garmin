# swisstopo palette — measured, not chosen

Reference colours for the TYP file (SPEC.md FR-CART9). Every value is **sampled from
swisstopo's own raster products** via WMTS, so the map matches the Landeskarte rather than
someone's recollection of it.

## Method

`tools/sample_palette.py` fetches WMTS tiles over locations chosen for a single dominant
cover type, discards paper-white and ink-black pixels, and reports the dominant tints.

- Prefer **`ch.swisstopo.swisstlm3d-karte-farbe`** — served as lossless **PNG**.
- `ch.swisstopo.pixelkarte-farbe` (true LK25) is **JPEG**, so its colours are smeared by
  compression; use it to read *symbology*, not exact values.
- Endpoint: `https://wmts.geo.admin.ch/1.0.0/{layer}/default/current/3857/{z}/{x}/{y}.{fmt}`

## Measured values (z15, 2026-09-06)

| Cover | Sample location | swisstopo | Previously used | Note |
|---|---|---|---|---|
| Water | Brienzersee | **`#D3EEFF`** | `#AAD6EF` | ours was far too saturated |
| Forest | Grindelwald slope | **`#CAECC1`** | `#BCDFA6` | ours too dark and olive |
| Open green / farmland | Bern | **`#CEE3CE`**, `#B4D6AB` | — | not previously modelled |
| Glacier / firn | Fiescher | **`#CCD3D3`** | `#EDF7FB` | swisstopo is grey-blue, not white-blue |
| Rock | Eiger north face | **`#706E6C`** ink on white | `#E0DCD3` flat grey | see below |
| Buildings | Gydisdorf | **`#000000`** solid black | `#8C837A` grey | LK draws buildings solid black |
| Paper | everywhere | `#FFFFFF` | `#FFFFFF` | correct |

## What the samples reveal about symbology

- **Rock is not a grey area fill.** The Landeskarte draws rock as fine dark hachures on
  white paper; sampling the Eiger returns mostly `#FFFFFF` with `#706E6C` linework. A flat
  grey polygon is the wrong idiom — this needs a TYP XPM pattern fill (FR-CART10).
- **Buildings are solid black**, not grey, at 1:25 000.
- **Individual trees are drawn.** `tlm_bb_einzelbaum` (11.5 M points) is currently excluded
  as noise, but the Landeskarte does show tree symbols. Reconsider for the highest zoom
  level only.
- **The map is mostly white.** Tints are pale and used sparingly; the ink does the work.
  Our current palette is uniformly too saturated.

## Applied

These values are now live in [`cartography/palette.json`](../cartography/palette.json),
from which `tools/make_typ.py` generates the TYP. Rock and scree became **pattern fills**
rather than flat tints, and buildings became solid black.

## Still to sample

Roads by class, railways, contour bistre, wetland, vineyard/orchard, scree, spot heights,
and the night palette. Sampling should move to a scripted, committed dataset so palette
changes are reviewable (FR-CART7).

---

# The winter scheme

swisstopo publishes a **Winter national map** (`ch.swisstopo.pixelkarte-farbe-winter`)
alongside the summer one, so the winter colours are measured from swisstopo's own
cartography like every other value here rather than being invented (SPEC.md FR-CART11).

Regenerate with:

```
python3 tools/winter_palette.py            # dry run, prints the shifts
python3 tools/winter_palette.py --write    # updates cartography/palette.json
python3 tools/make_typ.py                  # regenerates all four TYP variants
python3 spikes/s0/checkstyle.py            # guards every variant's [_drawOrder]
```

## What is measured is the shift, not the colour

Both sheets are sampled with one estimator over the *same tiles*, and the per-channel
difference is applied to the palette's summer value.

The winter readings cannot be used directly: those tiles are JPEG, while the summer
palette was measured from the lossless swissTLM3D raster. The two absolute numbers are
not comparable — substituting the winter reading would darken the whole map — but their
difference is.

Sampling uses the channel-wise **median** of fill pixels, not the mode: JPEG smears every
flat area into a cloud of near-identical values, so the most common single value is noise
while the middle of the cloud is stable. Neutral greys are excluded as linework, labels
and hachure, matching `sample_palette.py`; without that, an urban or farmland tile's
median is dragged dark by its roads and place names.

## Measured shifts (z15, three tiles per fill)

| Fill | Summer | Winter | Shift (R, G, B) | Palette day → winter |
|---|---|---|---|---|
| forest | `#BCD0A9` | `#B5D7D6` | (−7, +7, +45) | `#CAECC1` → `#C3F3EE` |
| glacier | `#C5CCD2` | `#BEDBEF` | (−7, +15, +29) | `#CCD3D3` → `#C5E2F0` |
| water | `#D3EEFF` | `#D3EDFC` | (0, −1, −3) | `#D3EEFF` → `#D3EDFC` |
| open land | `#BFC7B0` | `#B8D1D4` | (−7, +10, +36) | `#CEE3CE` → `#C7EDF2` |
| built-up | `#C3C6B3` | `#BCCDD3` | (−7, +7, +32) | `#EFECE6` → `#E8F3FF` |
| rock ink | `#9E9B9C` | `#8E969A` | (−16, −5, −2) | `#706E6C` → `#60696A` |

The pattern is consistent: green goes to teal, blue rises, red falls slightly. Water is
already blue and barely moves.

## What is not shifted, and why

Pale variants (`forest_open`, `copse`, `scrub`, `firn`, `green_mid`, `leisure_area`,
`transport_area`, `scree_ink`, `water_line`, `wetland_ink`, `contour_ice`) cannot be
isolated by sampling, so they keep the relationship they have to their measured parent in
summer.

Overlay colours are printed on the sheet at full strength and are left alone: routes
(ski, snowshoe, winter hiking, cycle, MTB, hiking), ink, buildings, rail, the road
hierarchy and the contours. The point of a winter sheet is that the base map steps back
so the routes on it read; washing out the routes too would defeat it.

Line **casings** also keep their summer colour. Their job is separation, not tint.

## A rejected approach

A single per-channel linear fit of the whole winter sheet against the summer one, over
59,292 paired pixels:

| Channel | Fit | R² |
|---|---|---:|
| R | `0.9614 × summer + 4.58` | 0.857 |
| G | `0.8975 × summer + 27.40` | 0.853 |
| B | `0.7331 × summer + 73.05` | 0.710 |

Rejected. R² of 0.71 on blue says a global recolour does not explain the winter sheet,
and applying it turned the bistre contours (`#C9A277`) pink (`#C6ADA0`) and the roads
salmon. The winter sheet recolours *selectively*, so it has to be measured selectively.
