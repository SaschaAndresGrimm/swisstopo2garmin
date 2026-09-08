# Releasing

What a release consists of, what is automated, and — the part that matters — what is
**not yet possible** because it needs credentials or hardware nobody has yet supplied
(SPEC.md NFR-7, NFR-10, §14).

## Before anything else: the device sweep

`docs/device-verification.md`, all of it, on at least one Edge and one fēnix. CI cannot
tell you whether a map works on a Garmin, and §13.5 makes this non-negotiable. A release
that skips it is a release of untested software, whatever the test count says.

## What ships

| Platform | Artefact | Signing |
|---|---|---|
| macOS arm64 | `.dmg` | Developer ID Application certificate, then notarisation |
| macOS x64 | `.dmg` | same |
| Windows | `.msi` | Authenticode certificate |
| Linux | `.AppImage`, `.deb` | unsigned; `.deb` may be signed by a repository if one is set up |

Plus `sbom.json` from `tools/sbom.py`, and a changelog with before/after images for any
cartography change (§14).

## Bundled resources

`src-tauri/tauri.conf.json` declares everything a build needs at runtime: `style`, `typ`,
`devices`, `estimator`, `NOTICE`, `LICENSE`, the two Java tools, and the JRE. The bundle is
around 140 MB, almost all of it the JRE.

Two things here were broken until the Milestone 7 audit, and both would have shipped an
application that could not build a single map while working perfectly on the developer's
machine:

* **No resources were declared at all.** `resource_root()` finds its files by walking up
  from the executable looking for `devices/` and `style/`. In a development checkout that
  walk reaches the repository. In a `.app` it reaches nothing, and returned `"."`.
* **The toolchain was located through `vendor/toolchain.env`**, which `fetch_tools.py`
  writes with absolute paths into the developer's home directory. A bundle built on that
  machine carried those paths and looked for the map compiler somewhere that exists on
  exactly one computer.

A third followed from fixing them: declaring the JRE as a resource made every build
*after the first* fail. Tauri copies resources into `target/` preserving their mode, and
Temurin ships 430 read-only files — the CDS archives and every licence text — so the
second build could not overwrite what the first had written. `fetch_tools.py` now makes
the vendored tree owner-writable, which fixes the cause rather than the symptom.

All three are fixed and all three have tests. The strongest is
`the_real_bundle_is_self_sufficient`: when a macOS bundle exists under `target/release`, it
resolves the resource root from the bundle's own executable path, checks every needed
directory is inside it, discovers the toolchain from the layout, and **runs the bundled
Java against the bundled mkgmap** to read its version. It skips when no bundle has been
built, so build one before every release:

```sh
cargo tauri build --bundles app
cargo test -p swisstopo2garmin resource_root
```

Then install it and check **About → Licences** shows real versions rather than "not
installed". That one line is the cheapest end-to-end proof that the packaged app found its
toolchain.

## The pipeline

`.github/workflows/release.yml` builds all four platforms on a `v*` tag, and on demand via
**Actions → Release → Run workflow** so an unsigned build can be produced and tried at any
time. Each runner:

1. installs Rust and Node, and `npm ci` in `frontend/` — which is where the Tauri CLI is
   pinned, so CI does not spend five minutes compiling it;
2. runs `vendor/fetch_tools.py`, which downloads mkgmap, splitter and a **per-platform**
   JRE, because the bundled runtime has to match the runner's architecture;
3. builds the bundle;
4. on macOS, runs `cargo test -p swisstopo2garmin resource_root`, which resolves the
   resource root from the bundle's own executable path and runs the **bundled** Java
   against the **bundled** mkgmap. This is the check that would have caught the three
   packaging defects below;
5. uploads the installers as artefacts, and on a tag opens a **draft** release with
   `SHA256SUMS` and `sbom.json`.

A draft, never a published release: whether an unsigned build should be offered to the
public is a person's decision, not a workflow's.

Three details worth knowing if you edit it, or if you build a bundle by hand.

* **The CLI must run from the repository root.** It finds the project by searching below
  the working directory, so an `npm run` inside `frontend/` searches `frontend/` and finds
  nothing.
* **Ubuntu 22.04, not latest.** An AppImage built against a newer glibc will not start on
  an older distribution.
* **`CI=true` is required for the DMG**, and its absence fails silently. Tauri's DMG step
  drives Finder through AppleScript to position the icons, which simply hangs in a
  non-interactive shell — no error, no timeout, no output. GitHub Actions sets `CI`
  itself, so the workflow is fine; building one locally is not:

  ```sh
  CI=true npx --prefix frontend tauri build --bundles dmg
  ```

### What CI costs, and what it therefore tests

On 2026-09-07 the account's Actions budget ran out and every job on the next two pushes
failed before starting, with `recent account payments have failed or your spending limit
needs to be increased`. Nothing was wrong with the code; the runs simply never began.
The lesson is that this matters and is worth writing down.

Measured against the last green run, one push cost **84 billable minutes**:

| job | wall time | rate | billable |
|---|---|---|---|
| rust (macos-15-intel) | 199 s | ×10 | 40 min |
| rust (macos-14) | 90 s | ×10 | 20 min |
| rust (windows-latest) | 380 s | ×2 | 14 min |
| rust (ubuntu-latest) | 196 s | ×1 | 4 min |
| the five small Linux jobs | — | ×1 | 6 min |

Two of the nine jobs were 71 % of the bill, and one of those two bought almost nothing:
`macos-14` is arm64, which is the platform this project is developed on, so it re-ran at
ten times the price what the author's own machine had already run. `macos-15-intel` is
different — x86_64 macOS is covered by nothing else.

So per-push CI is **Linux and Windows**, x86_64 macOS runs **weekly and on
`workflow_dispatch`**, and arm64 macOS is not in CI at all — `release.yml` still builds
and verifies both macOS bundles on a tag, which is where the artefact actually matters.
That is about 24 billable minutes a push instead of 84.

Two smaller economies came with it. A superseded pull-request run is now cancelled,
while pushes to `main` are deliberately not — the working agreement is that every commit
on main passes (rule 4), and cancelling would leave commits with no verdict. And
markdown-only pushes skip CI, because no job reads a `.md` file; `docs/tlm3d-schema.json`
is pointedly excluded from that, since the code is held against it (rule 2).

Windows keeps its per-push slot despite costing ×2. It is the platform that has actually
broken — it could not build at all for want of `icon.ico` — and FR-74 makes it a
priority.

### The icons

The artwork is **generated, not hand-painted**, by `tools/make_icon.py`, so it is
reproducible and its colours come from the one place this project keeps colours:
`docs/palette.md`, where every value is measured off a swisstopo raster product. To
change or regenerate it:

```bash
python3 tools/make_icon.py --variant slate --out src-tauri/icons/icon.png
cargo tauri icon src-tauri/icons/icon.png
rm -rf src-tauri/icons/android src-tauri/icons/ios \
       src-tauri/icons/Square*Logo.png src-tauri/icons/StoreLogo.png
python3 tools/make_icon.py --variant slate --out src-tauri/icons/icon.png   # see below
```

The last line is not a typo. `tauri icon` **overwrites its own input** with a 512×512
copy, so without it the 1024×1024 master silently degrades every time the assets are
regenerated — and the next run would then build the `.icns` from 512.

`--variant` picks the ground: `slate` is what ships, `sky` is the same landform on the
Landeskarte's measured water blue, and `paper` is the most cartographically faithful.
`paper` was rejected on evidence rather than taste: rendered at 16 and 32 px against a
light desktop it is a pale smudge, because an icon that is mostly white paper has
nothing to be found by. `--all --preview DIR` writes every variant at 16…512 px on both
a light and a dark ground, which is what that was judged on.

`src-tauri/icons/` keeps only what the four desktop bundles need: `icon.ico` for the
Windows resource file, `icon.icns` for the macOS bundle, and PNGs for Linux. `tauri icon`
also emits Android, iOS and Windows Store sets, which are deleted — this ships to four
desktop platforms and unused assets in a repository get mistaken for intent.

Two other things are worth knowing. Windows **fails to build at all** without `icon.ico`
— that is what the release pipeline's first Windows run reported, after five years of
nobody building for Windows. And the previous source was 256×256 where `tauri icon`
wants 1024×1024, so every slice above 256 was upscaled and the `.icns` was soft at large
sizes on macOS. That is now fixed: the master is a true 1024×1024 and the `.icns` carries
a real `ic10` (1024 px) slice.

#### Why the icon is not a Swiss cross

The icon this replaced was a **red cross on a white ground**. That is not the Swiss flag
— the flag is a white cross on red, the inverse — it is the emblem of the **Red Cross**,
protected by the Geneva Conventions and, in Switzerland specifically, by the
*Bundesgesetz über den Schutz des Zeichens und des Namens des Roten Kreuzes*. Shipping
it would have been a real problem, not a stylistic one.

The Swiss coat of arms is separately restricted to official use under the
*Wappenschutzgesetz*, and the flag itself may not be used in a way suggesting official
endorsement. So no cross of any colour. The Swissness comes instead from the
cartography — the Landeskarte's measured tints, its red hiking route, its north-west
relief lighting and a reference to its rock hachure — which is also what the app
actually produces. Nothing imitates swisstopo's own mark, and nothing implies
endorsement (SPEC.md FR-L3).

### Verified locally, 2026-09-07

A macOS arm64 build produced `swisstopo2garmin_0.1.0_aarch64.dmg`, 51 MB. Mounted, the app
carries `style`, `typ`, `devices`, `estimator`, `NOTICE`, `LICENSE` and `vendor` in
`Contents/Resources`, and **the bundled Java runs the bundled tools from inside the
mounted image**:

```
Mkgmap version 4924
splitter 654 compiled 2025-04-13T22:39:53+01:00
```

`codesign` reports `adhoc, linker-signed`, which is what unsigned looks like. That is the
end-to-end evidence that the distributable is self-contained.

### What the first real release run found, 2026-09-08 (v0.2.0)

There were no tags before `v0.2.0`, so this workflow had **never executed**. Every
statement above about it working was inference from a local build, and three of the four
platforms failed on the first attempt. All three causes were the same shape: something
that is absent locally is *present but empty* in CI, or something the local build never
had to do.

**macOS, both architectures — signing.** `env: APPLE_SIGNING_IDENTITY: ${{ secrets.X }}`
puts the variable into the environment as an **empty string** when the secret does not
exist, and Tauri decides whether to sign on the variable's *presence*, not its value. So a
project with no certificate asked `codesign` to sign with the identity `""`:

```
error: The specified item could not be found in the keychain.
failed to bundle project: failed codesign application
```

The comment in the workflow asserted the opposite — "with none set it builds unsigned
rather than failing" — and cited the CLI binary's strings. The strings were read
correctly; the inference from them was wrong. This is also exactly why the local build
above succeeded: locally the variable did not exist at all. The Apple variables are now
exported through `GITHUB_ENV` only when non-empty, which is the only way to leave an
environment variable genuinely unset.

**Linux — the bundled JRE defeats linuxdeploy.** Reported by Tauri as nothing more than
`failed to run linuxdeploy`. Two guesses were wrong before `--verbose` produced the actual
message:

```
Deploying dependencies for ELF file .../vendor/jre/lib/librmi.so
ERROR: Could not find dependency: libjvm.so
ERROR: Failed to deploy dependencies for existing files
```

linuxdeploy resolves the dependencies of every ELF file in the AppDir, and this bundle
ships a whole JRE. The JRE's libraries link against `libjvm.so`, which sits in
`jre/lib/server` and is found at runtime through an rpath rather than the loader's search
path. `LD_LIBRARY_PATH` pointed at the JRE's own directories inside the AppDir fixes it.

The wrong guesses are worth recording so they are not made again: **missing FUSE was not
the cause** (`libfuse2` installed and `APPIMAGE_EXTRACT_AND_RUN=1` set, and it still
failed), and **neither was stripping** (`NO_STRIP=true` is kept regardless, since there is
no reason for linuxdeploy to strip Adoptium's shared objects).

Two process notes. `--verbose` is now permanent on the bundle step: a bundler that hides
its child process's error costs an hour per guess. And a platform-specific bundling bug is
best chased on a throwaway branch with the matrix cut to that one platform — eight minutes
a cycle instead of waiting on the 26-minute macOS x64 leg.

Windows built correctly on the first attempt and every attempt since.

### v0.2.0 verified from the published artefact, 2026-09-08

Checklist step 7, done against the file users actually get rather than a local build. The
arm64 `.dmg` was downloaded from the release, its digest checked against the published
`SHA256SUMS`, mounted, and:

```
CFBundleShortVersionString      0.2.0
Resources/                      LICENSE NOTICE devices estimator icon.icns style typ vendor
Resources/icon.icns             sha256 matches src-tauri/icons/icon.icns
codesign                        flags=0x20002(adhoc,linker-signed), Signature=adhoc
vendor/jre .../bin/java -jar mkgmap.jar --version    Mkgmap version 4924
vendor/jre .../bin/java -jar splitter.jar --version  splitter 654
```

The last two lines are the point: the bundled Java ran the bundled tools from inside the
mounted image, so the distributable found its own toolchain. `adhoc, linker-signed` is
what unsigned looks like, and is expected until a certificate exists.

All four platforms produced artefacts: `aarch64.dmg` 51.5 MB, `x64.dmg` 47.1 MB,
`x64_en-US.msi` 52.1 MB, `amd64.AppImage` 123.3 MB, `amd64.deb` 56.4 MB, plus `sbom.json`
and `SHA256SUMS`. The AppImage is much the largest because linuxdeploy bundles the WebKit
stack that the other platforms take from the OS.

**Still not done for this release:** checklist step 8 (a map built from the installed app
and put on a device) and step 9 (sample maps attached to the release). The sample maps on
disk were built before the version bump, so their manifests record `0.1.0`; they should be
rebuilt with `tools/device_test_set.sh` before being attached to a 0.2.0 release rather
than shipped with a version string that is now wrong.

## Signing: wired, and inert until the secrets exist

Every signing step is guarded on **its secret being present**, so today the workflow
produces working unsigned bundles, and the day certificates exist it produces signed ones
with no edit to the file. Nothing below has been executed end to end — **no signed
artefact has ever been produced.**

### macOS

Set these repository secrets:

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | the Developer ID Application `.p12`, base64-encoded |
| `APPLE_CERTIFICATE_PASSWORD` | its export password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: NAME (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarisation; `APPLE_PASSWORD` is an **app-specific** password, not the account one |

The workflow imports the certificate into a throwaway keychain that is discarded with the
job, so the runner's login keychain is never touched. Tauri signs when
`APPLE_SIGNING_IDENTITY` is set and notarises when the other three are too; with none set
it builds unsigned rather than failing. Those variable names were read out of the CLI
binary rather than assumed — an earlier draft of this file invented a Windows one that
does not exist.

Needs a paid Apple Developer account. **Not obtained.** Until it is, macOS shows
Gatekeeper's "cannot be opened because the developer cannot be verified", and the honest
thing is to say so in the release notes with the right-click → Open workaround, which the
draft notes do.

### Windows

Set `WINDOWS_CERT_THUMBPRINT`. There is no environment variable for Authenticode — the
thumbprint is a config field — so the workflow writes a one-line override that
`tauri build --config` merges over `tauri.conf.json`, and writes `{}` when the secret is
absent. One build step either way, rather than two that drift apart.

Needs an Authenticode certificate. **Not obtained.** Unsigned, SmartScreen warns until the
binary accumulates reputation.

### Linux

Unsigned by design. Signing a `.deb` is a repository's job, and there is no repository.

## Is it standalone?

Yes, with one caveat that matters more than the packaging.

The bundle carries the map compiler, the splitter, a private JRE, the cartography, the TYP
files, the device profiles, the size model, `NOTICE` and `LICENSE`. Nothing needs to be
installed — no Java, no GDAL, no Python. On macOS it is about **147 MB**, almost all of it
the JRE.

What it does **not** carry is the geodata. swissTLM3D is 4.5 GB over the wire and 10.0 GB
unpacked, and the app downloads it on first run. So the installer is standalone; the first
launch is not, and `docs/getting-started.md` opens by saying so.

## SBOM

```sh
python3 tools/sbom.py > sbom.json     # CycloneDX 1.5
python3 tools/sbom.py --check         # in CI
```

Covers four ecosystems, because no single tool does: Rust crates from `cargo metadata`
(the resolved graph, so it is what was built), shipped npm packages from the lock file,
the vendored Java tools and JRE — which are the largest components and the only GPL-2 ones
and are invisible to both package managers — and the swisstopo datasets as `data`
components, derived from the constants in `stac.rs` so a new dataset appears without
anybody remembering.

npm **dev** dependencies are excluded: they are build tools, nothing of them reaches the
bundle, and listing 240 of them would bury the four a redistributor has obligations about.
The count is recorded as a property so the omission is visible.

Splitter is recorded as `GPL-3.0-only`, settled from its own source headers rather than
from the GPL-2.0 this project's `NOTICE` used to assert. That matters beyond bookkeeping:
GPL-2.0 (mkgmap) and GPL-3.0-only (splitter) cannot be combined by linking, so the process
boundary is what makes shipping both of them lawful.

## NFR-10's other two clauses

* **Dependencies pinned.** `Cargo.lock` and `package-lock.json` are committed and CI builds
  with `--locked`.
* **Reproducible builds.** *Not achieved and not attempted.* The build embeds no
  timestamps that we add, but it has never been checked that two builds of the same commit
  produce identical bytes, and Tauri's bundling has not been examined for it. Claiming
  reproducibility without having compared two builds would be exactly the kind of unbacked
  claim rule 8 forbids.
* **Dependencies audited.** `cargo audit` and `npm audit` are not in CI. Worth adding;
  neither has been run as part of a release process, because there has not been one.

## Sample maps

Six ready-made `.img` files go up with each release so the cartography can be looked at on
a device without a 10 GB download first — `docs/sample-maps.md` describes them, and they
carry a clear warning that they have not been verified on hardware.

They cannot be built in CI, because that needs swissTLM3D and no runner is going to fetch
it. Build them locally and attach them:

```sh
sh tools/device_test_set.sh
sh tools/publish_samples.sh v0.1.0
```

## Release checklist

1. Device sweep, `docs/device-verification.md`, recorded in its log table.
2. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`.
3. `cargo test --release -p s2g-core --test perf -- --nocapture`, and record the numbers in
   `docs/performance.md` if they have moved.
4. `npm --prefix frontend run check:i18n`, `python3 spikes/s0/checkstyle.py`.
5. `python3 tools/sbom.py --check`, then generate `sbom.json`.
6. Tag, or run the Release workflow by hand. It builds all four platforms and opens a
   draft release. **Watch it.** Its first real execution failed on three of four
   platforms; see "What the first real release run found" above. A platform-specific
   failure is best chased on a throwaway branch with the matrix cut to that platform.
7. **Install one bundle and check About → Licences shows real versions** rather than "not
   installed". That one line is the cheapest end-to-end proof that the packaged app found
   its own toolchain.
8. Build one real map from the installed app and put it on a device.
9. `sh tools/device_test_set.sh && sh tools/publish_samples.sh <tag>`.
10. Changelog, with before/after images for cartography changes. Publish the draft.
