# swisstopo2garmin — working agreement

Read [SPEC.md](SPEC.md) and [PLAN.md](PLAN.md) before starting any milestone.

## Rules

1. **Requirement IDs are the contract.** `FR-30`, `NFR-2`, `FR-P6` etc. refer to SPEC.md.
   Reference them in commits and PR descriptions. State which FR a change satisfies.
2. **Never invent a data schema.** Every swissTLM3D layer name, attribute name, and
   attribute value used in code must appear in `docs/tlm3d-schema.md`, which is generated
   from the real file. If it is not in that document, it was invented — that is a defect.
3. **Never invent a device limit.** Every number in `devices/*.json` carries a `confidence`
   level (`vendor` | `measured` | `community` | `assumed`) and a source list. `assumed` is
   an acceptable value. A fabricated citation is not.
4. **One milestone per branch, small commits.** Each commit compiles and passes tests.
5. **Tests before UI.** Logic lives in tested Rust modules; the GUI is a thin shell.
6. **Stream, never slurp.** Any code path that could hold a national dataset in memory is a
   defect (NFR-2). The national GeoPackage is 10.78 GB uncompressed.
7. **No new dependency without justification**: what it does, why not std, license,
   maintenance status.
8. **Report honestly.** "Works" means a test asserts it. If acceptance criteria are not met,
   say which ones and why — do not narrow the milestone to fit what got built.
9. **Ask when the spec is ambiguous**; record the answer back into SPEC.md.

## Data cache

Geodata never goes in the repo. Default cache location is `~/.cache/swisstopo2garmin`,
overridable with the `S2G_CACHE` environment variable. See `spikes/s0/README.md`.

## Attribution

Generated maps and the app must carry `© swisstopo` (FR-L1).
