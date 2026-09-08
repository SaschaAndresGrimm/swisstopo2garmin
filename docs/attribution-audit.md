# Attribution audit — FR-L1 … FR-L4

Checked 2026-09-07. Each requirement, where it is satisfied, and the test that keeps it
satisfied. Two findings are recorded at the end: one fixed, one open.

## FR-L1 — swisstopo attribution visible in the app, in the build report, and embedded in the map

| Required in | Where | Verified by |
|---|---|---|
| About screen | `frontend/src/steps/AboutScreen.tsx`, reachable from the header on every screen | `frontend/scripts/check-i18n.mjs` (the strings exist in all four languages) |
| Build report | `manifest.attribution`, written beside every `.img` | `a_manifest_round_trips_through_disk` |
| Map metadata | `MapIdentity::description` → mkgmap `--description`, so it is inside the file | `builds_a_verified_gmapsupp_from_the_fixture` |
| Raster overlay | `raster::doc_kml` writes `© swisstopo` into the KMZ's own `doc.kml` (FR-R6) | `the_kml_has_one_ground_overlay_per_tile_and_carries_the_attribution` |

The last two rows are the ones that matter, and both are actually checked: the `.img` test
reads the compiled `gmapsupp.img` back as bytes and asserts the copyright string is
present. A footer in the app does not satisfy FR-L1, because the file leaves the app — it
goes onto a device and gets passed to other people, and the attribution has to travel with
it.

The overlay row was added in 0.2.0 and is a stronger case than the vector map: a KMZ
contains *redistributed swisstopo imagery* rather than a derived rendering, so its
attribution is the difference between passing on a licensed file and an unlicensed one.
`NOTICE` was corrected at the same time — it had listed `ch.swisstopo.pixelkarte-farbe` as
"palette reference only", which stopped being true when the overlay began fetching tiles
from it.

The About screen reads the string from the backend, from the same call the pipeline uses
(`MapIdentity::for_recipe`). A hand-written copy in the frontend could drift from what is
actually embedded, and then the app would be displaying a promise the files do not keep.

Also present, unchanged: the footer line on every screen, and the PBF writer's
`osmosis_replication_*`-adjacent producer string.

## FR-L2 — mkgmap and splitter licences ship with the app and are listed in NOTICE

| Component | Licence text shipped | Listed in NOTICE |
|---|---|---|
| mkgmap r4924 | `vendor/mkgmap-r4924/LICENCE` — GPL v2, verbatim | yes |
| splitter r654 | `vendor/splitter-r654/doc/LICENSE-gpl-3.0.txt` and three others | yes |
| Eclipse Temurin JRE/JDK | `vendor/jre/**/legal/java.base/` — GPL v2 text, `ASSEMBLY_EXCEPTION`, and `ADDITIONAL_LICENSE_INFO` carrying the Classpath Exception | yes |

The About screen lists all three with the path to the shipped text, and says plainly when
a text is *not* bundled rather than offering a path to a file that is not there.

Temurin's licences were initially recorded here as missing, on the assumption that
`fetch_tools.py` would have had to copy them. It does not have to: the Temurin
distribution ships them itself, per module, under `legal/`. Checking the directory rather
than reasoning about the download settled it, and all three components are covered.

## FR-L3 — personal use, swisstopo terms on redistribution, no Garmin affiliation

**Was missing entirely from the app.** NOTICE carried the trademark disclaimer and nothing
carried the other two. Now three sentences under "Using the maps you build" on the About
screen, in all four languages, each saying one thing:

* maps built here are for your own use;
* passing one to anyone else brings swisstopo's terms with it, and the attribution must
  stay;
* this project is not affiliated with, endorsed by, or sponsored by Garmin.

## FR-L4 — Garmin trademarks used descriptively

In NOTICE, and now on the About screen: *"Garmin, Edge, fēnix and epix are trademarks of
Garmin Ltd. or its subsidiaries, used here only to identify compatible devices."*

The device profiles in `devices/*.json` use the model names as identifiers, which is the
descriptive use this covers.

## Findings

**Fixed: NOTICE and SPEC.md asserted splitter is GPL-2.0, and it is GPL-3.0-only.**
Nothing in the distribution supported the GPL-2.0 claim — r654 ships
`doc/LICENSE-gpl-3.0.txt` and no GPL v2 text, its jar carries no licence file, and neither
the project's download nor its documentation pages name a version (the source browser at
`mkgmap.org.uk/websvn` returns 401).

Settled from splitter's own source headers, consistent across four files and two
independent mirrors of the subversion trunk:

> Copyright (c) 2009, Steve Ratcliffe
> This program is free software; you can redistribute it and/or modify it under the terms
> of the **GNU General Public License version 3** as published by the Free Software
> Foundation.

"version 3" with no "or later" — GPL-3.0-only, matching the licence text shipped beside
it.

Compliance is unaffected, and the reason is worth stating rather than assuming: both tools
are invoked as separate processes and never linked, so no combined work is formed and
neither licence reaches this code. The sharper point is that **GPL-2.0 and GPL-3.0-only
cannot be combined by linking at all** — so the process boundary is what makes bundling
mkgmap *and* splitter lawful in the first place, not merely what makes it tidy. SPEC.md
§14 now says that.

*(A third item was recorded here and withdrawn: Temurin's licence text is bundled, under
`legal/` inside the distribution, and the About screen now shows its path like the other
two.)*
