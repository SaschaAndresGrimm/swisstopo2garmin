# Sample maps

Six ready-made `.img` files, attached to each GitHub release, so the map can be looked at
on a real device without downloading 10 GB of geodata and building anything first.

**They have not been verified on hardware.** They are the device validation set from
`docs/device-verification.md` §B — built to exercise the cartography paths that have never
been on a Garmin, which is exactly why they are the interesting files to try and why
nobody can yet tell you they work. If one of them fails to draw, that is a finding worth
reporting, not a broken download.

## What each one is

All six cover **Grindelwald at a 6 km radius** — 144 km², the Eiger north face, the
Unterer Grindelwaldgletscher, two named SAC huts, two cableways, the Wengernalpbahn, 117
transit stops and the town. One extent was chosen deliberately: it holds every feature
class worth judging, so the six files differ only in the cartography under test.

| File | Device | Content | Scheme | Relief | Slope | Size |
|---|---|---|---|:--:|:--:|---:|
| `edge-1-hiking-summer.img` | Edge 840 | hiking, 20 m contours | summer | gentle | – | 1.11 MB |
| `edge-2-slope-no-relief.img` | Edge 840 | hiking, 20 m | summer | – | yes | 1.54 MB |
| `edge-3-skimo-winter-slope.img` | Edge 840 | ski touring, 20 m | **winter** | gentle | yes | 1.59 MB |
| `edge-4-full-10m.img` | Edge 840 | full topo, 10 m | summer | gentle | – | 1.48 MB |
| `fenix-1-hiking-summer.img` | fēnix 5 Plus | hiking, 20 m | summer | gentle | – | 0.79 MB |
| `fenix-2-skimo-winter-slope.img` | fēnix 5 Plus | ski touring, 20 m | **winter** | – | yes | 1.19 MB |

The `edge-*` files use the handlebar cartography and the `fenix-*` files the reduced wrist
cartography — thinner lines, fewer labels, buildings and parking dropped. They are not
interchangeable in any technical sense, but nothing stops you trying an Edge file on a
fēnix; it will just be denser than that screen can carry.

A `.manifest.json` accompanies each one, recording the exact recipe, the swisstopo dataset
releases, the tool versions, the per-layer feature counts and the output's SHA-256. That
is enough to reproduce the file, and it is how the size estimator's reference suite was
assembled without rebuilding anything.

## Installing

**Edge** — copy any or all of them into `/Garmin/` on the device under their own names and
pick between them in the map settings. Each has its own family id, so they coexist without
colliding, and each is named for what it is — "Grindelwald ski touring", "Grindelwald
hiking, wrist" — so the device's map manager is some help in choosing.

> Maps built before 2026-09-07 all appear as **"OSM street map"** in the Edge's map
> manager. That was a defect: mkgmap needs the names on both of its passes and only got
> them on the first. If you have older files, rebuild them.

**fēnix** — one at a time, renamed to `/Garmin/gmapsupp.img`. Delete or rename the previous
one first. Back up whatever is already there if you care about it.

If the device does not appear as a drive, it is in MTP mode: change USB mode on the device
itself. macOS cannot mount MTP at all.

Night colours are in the same file — the device switches to them, there is no separate
download. Please look at them. Of everything in these six files that is the likeliest to
be wrong, because the night palette is the only one in this project *derived* rather than
measured from a swisstopo product.

## Rebuilding them

```sh
sh tools/device_test_set.sh                 # Grindelwald, 6 km
sh tools/device_test_set.sh Zermatt 8       # or anywhere else
```

Needs swissTLM3D and the winter route data downloaded, and about ten minutes.

## Attribution

Map data © swisstopo. These files carry that string in their metadata, so it travels with
them; `docs/attribution-audit.md` records how that is verified. swisstopo's
[terms of use for free geodata](https://www.swisstopo.admin.ch/en/terms-of-use-free-geodata-and-geoservices)
apply if you pass them on.
