# Milestone 0 spikes — throwaway code

Purpose is knowledge, not reusable code. See [PLAN.md](../../PLAN.md) §2.

Python is used deliberately: these are exploratory scripts. The production
implementation is Rust (SPEC.md §10).

## Cache

Everything lands in `$S2G_CACHE` (default `~/.cache/swisstopo2garmin`). Nothing is
written into the repo.

## Scripts

| Script | Task | Purpose |
|---|---|---|
| `stac.py` | — | swisstopo STAC client: resolve latest release, list assets, verify multihash |
| `fetch_tlm3d.py` | S0.1 | Streaming download + on-the-fly inflate of the national swissTLM3D GeoPackage |
| `schema_dump.py` | S0.1 | Generate `docs/tlm3d-schema.md` from the real GeoPackage |

## Disk requirements

The swissTLM3D 2026-02 release is a 4.80 GB DEFLATE archive containing a single
**10.78 GB** GeoPackage. `fetch_tlm3d.py` inflates while downloading, so peak disk usage is
the 10.78 GB output only — the compressed archive is never stored.
