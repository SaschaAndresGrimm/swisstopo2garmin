# Changelog

Versions follow [semantic versioning](https://semver.org/). Below 1.0 the recipe and
device-profile schemas may still change; both carry a `schemaVersion` so a saved recipe
keeps working.

## 0.2.0 — first public release

There is no released 0.1.0. That number was carried through development from Milestone 1
and never tagged, so this is the first version anybody outside the project can install.

### What it does

- Builds a Garmin `gmapsupp.img` for a chosen device and area from swisstopo open data,
  and copies it onto the device. No account, no Basecamp, no toolchain to install: the
  bundle carries mkgmap, the splitter and a private JRE.
- **Cartography measured, not guessed.** Every colour is sampled off swisstopo's own
  raster products (`docs/palette.md`) — summer and winter schemes, rock hachures, blue
  contours over ice. Swiss hiking-trail classes are drawn distinguishably.
- Contours at 10/20/50/100 m from swissALTI3D, with device-side shaded relief where the
  profile supports it. Slope-angle classes in swisstopo's own bands.
- Ski touring (SAC routes with the skiable / carrying / caution distinction, snowshoe and
  winter trails, lifts), cycling (Veloland, Mountainbikeland, Wanderland route numbers),
  named SAC huts and public transport stops.
- Area selection by place, radius, bounding box, polygon, circle, administrative unit
  (canton, district, commune), GPX/FIT corridor, or GeoJSON — combinable, and editable by
  dragging.
- Place names in German, French, Italian or Romansh from swissNAMES3D.
- A size estimate before building, fitted on real builds and refitted from yours.
- A manifest beside every map recording the recipe, the exact dataset releases, tool
  versions and timings — enough to reproduce it.

### New in this release

- **Turn-by-turn routing** (`--route`): NET and NOD subfiles, road classes and speeds,
  and access, oneway, bridge, tunnel and surface derived from swissTLM3D attributes. The
  one dangerous assumption — that a divided carriageway is digitised in the direction of
  travel — is verified against the data, where motorway ramps agree 100 % of the time in
  the cases clear enough to judge (`docs/routing.md`). **Off by default.**
- **Address search**: house numbers from `ch.swisstopo.amtliches-gebaeudeadressverzeichnis`,
  the official register, since swissTLM3D has street names and no numbers. 3.3 M rows
  streamed in 1.5 s at constant memory (`docs/addresses.md`). **Off by default.**
- **Paper-map overlay**: the printed Landeskarte as Garmin Custom Map KMZ tiles. Within
  the 100-tile limit it covers about 12 × 12 km at the source's native 1.25 m/px — a
  day's walking at full sharpness (`docs/raster.md`). **Off by default.**
- A rewritten area step: one selection method at a time, a search that is no longer
  case- or accent-sensitive, a Clear button, and drag/draw that no longer lags.
- Four device profiles, and a way to record your own measured limits so the app builds
  against your device rather than a guess.

### Fixed

- **Every map was listed as "OSM street map"** on an Edge 840. mkgmap's second pass got
  no name options, and `--description` — whose default is that string — is what the
  device's map manager actually displays. Found on hardware.
- Two layer labels rendered as their own key names (`layer.tlm_oev_haltestelle`) because
  a translation key built from a value cannot be checked by the frontend's own linter.
  The hole that hid them is closed by a test over the Rust-side list.
- Place search could not find `Genève` from `geneve`, and ranked four hamlets above
  Grindelwald for `grindel`.
- Windows could not build at all for want of `icon.ico`, and the vendoring script never
  worked on Windows (Adoptium serves a zip there, and `java.exe` was not recognised).
- **The app icon was the Red Cross emblem** — a red cross on white, which is not the
  Swiss flag but a mark protected by the Geneva Conventions and Swiss federal law.
  Replaced with generated Landeskarte-derived artwork; no cross of any colour.

### Known limitations — please read before trusting a map

- **Routing, address search and the paper-map overlay have never been on a device.** All
  three are off by default for that reason. Check any route against the road signs. For
  the overlay it is not even known whether Edge and fēnix devices render custom maps at
  all — both advertise the folder, which is evidence, not proof.
- **The hardware sweep is incomplete.** Maps render correctly on an Edge 840 (firmware
  3133) and a fēnix 5 Plus (firmware 1930), but the winter colours, slope classes, hut
  symbols, transit stops and night palette have never been on a device.
  `docs/device-verification.md` records exactly what has and has not been checked.
- **Only two devices have a profile of their own.** Everything else falls back to a
  generic profile whose limits are `assumed`, so you get a smaller map than your device
  could hold. Measurements are the most useful contribution anybody can make — see
  "Help wanted" in the README.
- **The bundles are unsigned.** No Apple Developer ID and no Authenticode certificate
  exists for this project yet. macOS will say "cannot be opened because the developer
  cannot be verified" — right-click → Open. Windows SmartScreen will warn until the
  binary accumulates reputation. Linux `.AppImage` and `.deb` are unsigned as usual.
- **No screen-reader or keyboard-only pass**, and the map's drawing and editing tools are
  pointer-only. Every other way of choosing an area works from the keyboard.
- Switzerland and Liechtenstein only — that is swissTLM3D's coverage.

### Requirements

About 12 GB of free disk for the national swissTLM3D download plus working space, and the
first run downloads roughly 5 GB. Maps themselves are tens of megabytes.
