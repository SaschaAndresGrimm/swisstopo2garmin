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

## Signing: what is configured and what is missing

The configuration is in place; **the credentials are not, so no signed artefact has ever
been produced**. Nothing below has been executed end to end.

### macOS

```sh
export APPLE_SIGNING_IDENTITY="Developer ID Application: NAME (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="app-specific-password"   # not the account password
export APPLE_TEAM_ID="TEAMID"
cargo tauri build --target aarch64-apple-darwin
cargo tauri build --target x86_64-apple-darwin
```

Tauri signs and then submits for notarisation when all four are set, and skips signing
entirely when `APPLE_SIGNING_IDENTITY` is absent — which is why an unsigned local build
still works.

Requires a paid Apple Developer account. **Not obtained.** Until it is, macOS users get
Gatekeeper's "cannot be opened because the developer cannot be verified", and the only
honest thing to do is say so in the release notes with the right-click → Open workaround.

### Windows

```sh
export TAURI_WINDOWS_SIGNTOOL_PATH="C:/Program Files (x86)/Windows Kits/10/bin/x64/signtool.exe"
# and set bundle.windows.certificateThumbprint in tauri.conf.json
cargo tauri build
```

Requires an Authenticode certificate. **Not obtained.** Unsigned, Windows SmartScreen
warns until the binary accumulates reputation.

### Linux

Unsigned by design. The AppImage and `.deb` are reproducible from the same sources.

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

Splitter's licence is `GPL-2.0-or-later OR GPL-3.0-or-later`, deliberately. Its exact
version is unresolved — see the note in `NOTICE` — and an SBOM asserting one of them would
be stating something nobody has checked.

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

## Release checklist

1. Device sweep, `docs/device-verification.md`, recorded in its log table.
2. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`.
3. `cargo test --release -p s2g-core --test perf -- --nocapture`, and record the numbers in
   `docs/performance.md` if they have moved.
4. `python3 tools/check_i18n.py`, `python3 spikes/s0/checkstyle.py`.
5. `python3 tools/sbom.py --check`, then generate `sbom.json`.
6. Build each platform's bundle; **install one and check About → Licences shows versions**.
7. Build one real map from the installed app and put it on a device.
8. Changelog, with before/after images for cartography changes.
