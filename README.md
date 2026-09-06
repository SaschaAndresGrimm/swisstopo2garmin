# swisstopo2garmin

Build Garmin Edge and fēnix maps from free swisstopo geodata, with a desktop GUI.

Map data © swisstopo. Licensed GPL-3.0-or-later.

- **[SPEC.md](SPEC.md)** — what it does and why
- **[PLAN.md](PLAN.md)** — implementation plan and milestone status
- **[docs/](docs/)** — discovered swissTLM3D schema, cartography, device verification,
  Milestone 0 findings

## Status

Milestone 1 (foundation) complete. The map pipeline currently runs as Milestone 0 spike
scripts (`spikes/s0/`), which have produced maps verified on a Garmin Edge 840. Porting
them into `crates/s2g-core` is Milestones 2–4.

## Prerequisites

- Rust stable
- **Node ≥ 20.17** — note that a Node built without ICU (Anaconda ships one) breaks
  eslint; `.nvmrc` pins the version
- Java is *not* required: `vendor/fetch_tools.py` downloads a private JRE plus
  mkgmap and splitter, without touching the system

```bash
python3 vendor/fetch_tools.py        # mkgmap, splitter, JRE -> vendor/
cd frontend && npm ci && cd ..
```

## Develop

```bash
cargo test --workspace                       # 24 tests, offline
cargo test -p s2g-core --test live -- --ignored   # hits data.geo.admin.ch
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

cd frontend && npm run typecheck && npm run lint && npm run build

```

### Running the app

A **debug** build loads the frontend from the Vite dev server (`devUrl`); only a
**release** build embeds `frontend/dist`. `cargo build && ./target/debug/...` on its own
therefore opens an empty window.

```bash
# dev: two processes, with hot reload on frontend edits
cd frontend && npm run dev &          # serves http://localhost:1420
cargo build -p swisstopo2garmin && ./target/debug/swisstopo2garmin

# or, with the Tauri CLI, one command that starts both:
cargo install tauri-cli --version "^2"
cargo tauri dev

# standalone: embeds the frontend, no dev server needed (slow build: lto + codegen-units=1)
cargo build --release -p swisstopo2garmin && ./target/release/swisstopo2garmin
```

The IPC types in `frontend/src/state/bindings.ts` are **generated** from the Rust
definitions by `cargo test -p swisstopo2garmin`. CI fails if the checked-in file is
stale, so the two sides cannot drift.

## Build a map today (spike pipeline)

```bash
spikes/s0/build.sh Grindelwald 8 20      # place, radius km, contour interval m
```

Produces `out/gmapsupp-<place>.img` plus a preview PNG. Copy the `.img` to
`/Garmin/gmapsupp.img` on the device — see [docs/device-verification.md](docs/device-verification.md).

The cartography TYP is **generated** from the measured swisstopo palette:

```bash
python3 tools/make_typ.py && python3 spikes/s0/checkstyle.py
```

`checkstyle.py` is not optional. A polygon type emitted by the style but missing from the
TYP's `[_drawOrder]` renders as nothing on the device *and* is invisible in the `.img` —
see [docs/m0-findings.md](docs/m0-findings.md) §4.8.

## Data cache

Datasets live in `~/.cache/swisstopo2garmin` (override with `S2G_CACHE`). swissTLM3D is a
4.80 GB download that inflates to a **10.78 GB** GeoPackage; it is inflated during
download so the archive is never stored.
