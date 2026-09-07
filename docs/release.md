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

### The icons

`src-tauri/icons/` holds only what the four desktop bundles need: `icon.ico` for the
Windows resource file, `icon.icns` for the macOS bundle, and PNGs for Linux. `tauri icon`
also generates Android and iOS sets, which are deleted — this ships to four desktop
platforms and unused assets in a repository get mistaken for intent.

Two things about them are worth knowing. Windows **fails to build at all** without
`icon.ico` — that is what the release pipeline's first Windows run reported, after five
years of nobody building for Windows. And the source is 256×256 where `tauri icon` wants
1024×1024, so anything larger than 256 is upscaled: the `.icns` will look soft at large
sizes on macOS. Cosmetic, and worth replacing the source with a real 1024px artwork
before a release anybody sees.

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
end-to-end evidence that the distributable is self-contained; the Windows and Linux
bundles have not been built or opened by anybody.

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
   draft release.
7. **Install one bundle and check About → Licences shows real versions** rather than "not
   installed". That one line is the cheapest end-to-end proof that the packaged app found
   its own toolchain.
8. Build one real map from the installed app and put it on a device.
9. `sh tools/device_test_set.sh && sh tools/publish_samples.sh <tag>`.
10. Changelog, with before/after images for cartography changes. Publish the draft.
