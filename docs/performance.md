# Performance and memory

What is measured, where the thresholds come from, and what is **not** covered
(SPEC.md NFR-1, NFR-2, §13.6).

## What CI checks

`cargo test --release -p s2g-core --test perf` runs on every push, on Linux, in release.
It measures rates on the committed 1 km² Grindelwald fixture and checks the resident set
while processing progressively larger grids.

| Measurement | Measured | Ceiling | Why that ceiling |
|---|---|---|---|
| `contour.per_km2` | 0.0079 s/km² | 0.040 | 5× measured; absorbs the 2–3× spread across CI runners |
| `contour.canton_extrapolated` | 15.7 s | 600 s | NFR-1's canton contour budget, not a calibration |
| `contour.national_extrapolated` | 324 s | 36,000 s | NFR-1's "overnight, unattended" |
| `slope.per_km2` | 0.0006 s/km² | 0.005 | 8× measured |
| `slope.canton_extrapolated` | 1.2 s | 150 s | A quarter of the contour budget: slope is optional |
| `proj.per_point` | 2.4 ns | 10 ns | 5× measured |
| peak RSS at 16× input | ~37 MB | grows less than 8× the largest grid, and under 4 GB | NFR-2 |

Measured on an Apple M-series laptop, release, 2026-09-07. CI runners are slower; the
headroom is for them.

## Why these numbers and not a real canton build

CI is offline and deterministic by design (PLAN.md M1 task 3). A canton build needs the
10.0 GB swissTLM3D GeoPackage and a few thousand swissALTI3D tiles from the network, so it
cannot run here at all — which is exactly why the requirement is checked by extrapolation
and why that is stated rather than glossed.

Extrapolation is a weaker claim than a measurement. It assumes the per-km² cost is flat
with area, which is true for contour tracing and slope classification (both are local
window operations over the grid) and is **not** true for anything that sorts, indexes or
joins across the whole extent. If such a stage is added, extrapolating it would be wrong
and this file should say so.

What extrapolation does catch reliably is the failure that actually occurs: an inner loop
becoming quadratic, or an allocation moving inside it. Those show up as a 10–100× change
in the rate, which no runner-to-runner variation can hide.

## Two traps this harness has already fallen into

**A benchmark of nothing.** The first version of the projection test discarded its result
and reported 0.03 ns per point — about a tenth of a cycle. The optimiser had deleted the
loop. Every rate here now accumulates its result and asserts the accumulator is non-zero.

**A rate measured over an empty stage.** The slope test at the shipped 30–50° bands
produced nothing on this fixture: the tile is the Grindelwald valley floor, 945 m to
1105 m across a kilometre, and the shipped bands find only two specks there. It was
therefore measuring `SlopeField::from_grid` and five empty traces. The test now uses
20–40° bands, where the fixture yields eight areas and drops fifty-eight small ones, so
tracing and simplification are both in the measurement. The comment in the test says so,
because the numbers are otherwise not comparable with a real build's.

## Not covered

* **Real builds at commune, canton and national scale.** Timed by hand during Milestone 5
  calibration (16 builds, recorded in `estimator/training-samples.jsonl`) and not tracked
  across releases. This is the gap that matters most, and closing it needs a machine with
  the datasets on it — a self-hosted runner or a scheduled job on the developer's machine.
* **Peak RSS during a real build**, including the Java children. `mkgmap`'s heap is capped
  by `-Xmx` at the call site, but this program's own peak during a national build has
  never been measured.
* **Startup time (NFR-6, under 2 s).** Needs the packaged app, not a test binary.
* **macOS and Windows peaks.** Both expose the high-water mark only through APIs that need
  `unsafe`, which the workspace forbids; the test falls back to current RSS there, which
  can miss a spike between samples.
