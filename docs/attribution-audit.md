# Attribution audit — FR-L1 … FR-L4

Checked 2026-09-07. Each requirement, where it is satisfied, and the test that keeps it
satisfied. Two findings are recorded at the end: one fixed, one open.

## FR-L1 — swisstopo attribution visible in the app, in the build report, and embedded in the map

| Required in | Where | Verified by |
|---|---|---|
| About screen | `frontend/src/steps/AboutScreen.tsx`, reachable from the header on every screen | `tools/check_i18n.py` (the strings exist in all four languages) |
| Build report | `manifest.attribution`, written beside every `.img` | `a_manifest_round_trips_through_disk` |
| Map metadata | `MapIdentity::description` → mkgmap `--description`, so it is inside the file | `builds_a_verified_gmapsupp_from_the_fixture` |

The third row is the one that matters, and it is now actually checked: the test reads the
compiled `gmapsupp.img` back as bytes and asserts the copyright string is present. A
footer in the app does not satisfy FR-L1, because the file leaves the app — it goes onto a
device and gets passed to other people, and the attribution has to travel with it.

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
| Eclipse Temurin JRE/JDK | **not bundled** | yes, with its licence named |

The About screen lists all three with the path to the shipped text, and says plainly when
a text is *not* bundled rather than offering a path to a file that is not there. Temurin's
licence text is the one gap: the JRE is fetched by `vendor/fetch_tools.py` and its licence
is not copied alongside it. Worth fixing before a release that ships the JRE.

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

**Fixed: NOTICE and SPEC.md asserted splitter is GPL-2.0.** Nothing available supports
that. The bundled r654 distribution ships `doc/LICENSE-gpl-3.0.txt` and no GPL v2 text;
its jar contains no licence file; the project's download and documentation pages state no
version; the source browser at `mkgmap.org.uk/websvn` returns 401. Both claims are now
reduced to what is checkable — splitter is GPL, and the text shipped with it is GPL v3 —
with the discrepancy written down rather than silently swapped for a different guess.

This changes nothing about compliance. Both tools are invoked as separate processes and
never linked, so no combined work is formed and their licence does not reach this code
under any version. The process boundary is what makes the arrangement clean, and SPEC.md
§14 now says so explicitly instead of resting on the version.

**Open: the licence version should be confirmed from the source.** A checkout of
splitter's subversion trunk would settle it from the file headers. Until then NOTICE says
what is shipped and marks the version unresolved.

**Open: Temurin's licence text is not bundled with the JRE.** `vendor/fetch_tools.py`
should save it beside the download, and the About screen will then show its path like the
other two.
