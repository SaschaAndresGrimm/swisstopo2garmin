# Your first map

Written for somebody who has just installed the app and wants a working map on their
Garmin. Acceptance criterion 1 says this should be possible **without reading any
documentation** — so if you find yourself needing a paragraph below to get unstuck, that
is a defect in the app worth reporting, not a failure on your part.

## What you will need

* **20 GB of free disk space.** swissTLM3D, the national dataset everything is built
  from, inflates to 10.0 GB, and each area you build caches its own elevation tiles at
  roughly 1.2 MB per square kilometre. The Data screen shows where the space went and
  offers to delete the two parts that can be re-downloaded.
* **A connection for the first download.** After that, building works offline apart from
  elevation tiles for ground you have not covered before.
* **Your Garmin and its cable**, when you get to the end.

Nothing else. No Java, no GDAL, no Python: the map compiler and a private Java runtime
are bundled.

## 1. Data

The **Data** screen lists every source. Only the first two are needed for a hiking map:

| Source | Size on disk | Needed for |
|---|---|---|
| swissTLM3D | 10.0 GB | everything |
| swissTLM3D Wanderwege | 0.4 GB | hiking-trail classes |
| SAC ski routes, snowshoe and winter trails | 0.05 GB | the ski touring preset |
| Veloland, Mountainbikeland, Wanderland | 1.0 GB | the cycling preset |
| swissNAMES3D | 0.1 GB | place names in a chosen language |
| swissBOUNDARIES3D | 0.07 GB | selecting a canton or commune by name |

The first download is the long one — 4.5 GB over the wire, unpacked as it arrives. It
resumes if it is interrupted, and it will not start if the disk cannot hold the result.

## 2. Device

Pick your model. If it is plugged in and mounted, it is marked as connected.

Each profile carries a **confidence level** for its size limits: `vendor`, `measured`,
`community`, or `assumed`. Anything below `measured` gets a safety margin applied to the
budget, because the limits Garmin devices actually enforce are undocumented and forum
lore about them is frequently wrong. If your device shows `assumed` and you are willing to
find out what it really does, [docs/device-verification.md](device-verification.md) is the
procedure, and measurements are accepted as pull requests.

If your device is plugged in but does not appear, it is probably in MTP mode — recent Edge
and fēnix models default to it, and macOS cannot mount MTP at all. The app will say so and
name the device. Change USB mode on the device itself, or use **Save to a folder** at the
end and copy the file across yourself.

## 3. Area

Four ways, and they combine:

* **Draw** a rectangle, polygon or circle on the map.
* **Search** a place and take a radius around it.
* **Import** a GPX track or FIT course and take a corridor around it — useful for a route
  you are actually going to ride or walk.
* **Choose** a canton, district or commune by name.

The estimated size updates as you go, against your device's budget. If it goes over, you
are told by how much and offered only the changes that would actually help *your* setup —
and if the area cannot fit on that device however it is divided, the build button is
disabled rather than letting you wait for a map the device will refuse.

**Start small.** A 10 km radius builds in a couple of minutes and tells you whether the
result looks right on your screen. A canton takes considerably longer, and the first thing
worth knowing is whether you like the map.

## 4. Content

The four presets set sensible defaults; the layer panel is there if you want to remove
something.

* **Contour interval.** 20 m is the Landeskarte's own spacing and the default. Contours
  are the largest part of a map by a wide margin, so 50 m is the first thing to change if
  you are short of space.
* **Shaded relief.** Available where the device supports a DEM.
* **Colour scheme.** Summer or winter, both measured from swisstopo's own printed sheets.
* **Slope classes.** swisstopo's 30–50° bands, drawn as a hatch so the map underneath
  stays readable. About 3.5 MB per 1,000 km² in alpine terrain.
* **Label language.** German, French, Italian or Romansh where swissNAMES3D has an
  exonym; otherwise the local name stays.

## 5. Build

Progress is reported per stage, weighted by how long each stage actually takes, so the bar
does not lurch. Elevation and contours are most of it.

Cancelling stops within a second or so and leaves nothing behind. If the app is killed
outright, the next start finds the interrupted build — and any map compiler still running
for it — and offers to resume, which reuses the expensive stages that survived.

A **manifest** is written beside every map, recording the recipe, the exact dataset
releases, the tool versions, and how long each stage took. It is enough to reproduce the
build later.

## 6. Install

The app copies the map to the device, then reads it back and compares digests. A flaky
cable produces a file of exactly the right length with wrong bytes in it, and only the
comparison catches that.

Two differences worth knowing:

* An **Edge** holds several maps at once under their own names, and you switch between
  them in the device's map settings.
* A **fēnix** takes one map at a time, named `gmapsupp.img`. Installing a second one
  replaces the first — the app offers to back up what it replaces.

## If something goes wrong

Every failure states what failed, why, and what to do about it. If you get one that does
not, that is a bug worth reporting: the recognised cases and their exact wording are in
[docs/error-matrix.md](error-matrix.md), and an unrecognised failure deliberately says so
rather than guessing at a cause.

The build log is shown on the build screen and can be copied in one click. It includes the
compiler's own output and the command line that produced it.
