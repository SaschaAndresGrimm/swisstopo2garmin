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
