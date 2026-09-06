# Output size model

How `estimator/size-model.json` was produced, what it predicts, and what it does not.
Regenerate with:

```
cargo run --release -p s2g-core --example fit_size_model                  # rebuild and refit
cargo run --release -p s2g-core --example fit_size_model -- --samples estimator/training-samples.jsonl
```

## What it predicts

`gmapsupp.img` size in bytes, as a linear function of eleven terms:

| Term | Predictor | Source |
|---|---|---|
| intercept | 1 | the fixed cost of a one-tile map |
| landCover, water, transport, built, names | feature count in the area | GeoPackage R-tree range count, exact, milliseconds |
| winter | feature count | the winter GeoPackages' own R-trees |
| cycling | feature count | a bbox scan of the ASTRA shapefiles, which carry no index |
| contour | `area_km² × 20 / interval_m` | — |
| relief 1″ / 3″ | `area_km²` for the selected resolution | — |

All eleven are available before a build starts, which is the point: the estimate must be
live as the user drags a rectangle (FR-54).

## Fitted coefficients

From 16 real builds on an Edge 840, listed in `estimator/training-samples.jsonl`:

| Term | Prior | Fitted | Unit |
|---|---:|---:|---|
| intercept | 48 000 | 48 000 | bytes |
| landCover | 14 | 109.3 | bytes/feature |
| water | 12 | 96.2 | bytes/feature |
| transport | 18 | 22.2 | bytes/feature |
| built | 9 | 6.6 | bytes/feature |
| names | 26 | 0.0 | bytes/feature |
| winter | 16 | 13.3 | bytes/feature |
| cycling | 16 | 0.0 | bytes/feature |
| contour | 1 900 | 1 881.7 | bytes/km² at 20 m |
| relief 1″ | 760 | 765.7 | bytes/km² |
| relief 3″ | 150 | 159.3 | bytes/km² |

**In-sample error 5.8 %, leave-one-out error 7.3 %, worst single residual 22.6 %** —
inside the ±25 % FR-60 asks for, but only just at the worst point.

## Why ridge regression, and why cross-validation chooses its strength

Eleven terms fitted from sixteen builds whose predictors are strongly correlated (a
larger area has more of *everything*) is an underdetermined problem dressed up as a
determined one. Plain least squares on this data produced a model with 3″ relief costing
more per km² than 1″ relief and winter routes at 1.1 KB per feature — an excellent
in-sample fit and nonsense out of sample.

So the fit is regularised toward a prior, and the ridge strength λ is chosen by
leave-one-out cross-validation rather than by in-sample error, which always prefers no
regularisation:

| λ | leave-one-out error |
|---:|---:|
| 0 | 27.6 % |
| 10³ | 23.4 % |
| 10⁴ | 16.1 % |
| 10⁵ | 11.3 % |
| 10⁶ | 8.5 % |
| **10⁷** | **7.3 %** |
| 10⁸ | 9.7 % |
| 10⁹ | 19.3 % |
| 10¹¹ | 25.7 % |

At the selected λ the coefficients are also physically ordered: 1″ relief costs about
five times 3″ relief per km², which is what the data should say and what the
unregularised fit denied.

Two coefficients are clamped to zero. `names` and `cycling` counts are collinear with
other terms in this training set, so the fit has nothing to attribute to them. That is a
statement about the training set, not about labels being free; a user's own builds can
move them (see below). Coefficients are never allowed to go negative — "more of this
makes the map smaller" is never true and extrapolates absurdly.

## The local calibration log

Every successful build appends its predictors and actual size to
`~/.cache/swisstopo2garmin/calibration.jsonl` (FR-62). `estimate::current_model` refits
the shipped model against that log, again choosing λ by cross-validation once there are
at least three samples. A user who builds only Alpine areas therefore ends up with an
Alpine-calibrated model.

## Known limitations

- **Contour output is modelled from area and interval alone.** Nothing cheap reveals
  terrain roughness before the elevation tiles are fetched, and an Alpine square
  kilometre carries far more contour line than a Mittelland one. This is the single
  largest error source, and it is why the worst residuals in the training set are the
  flattest and the steepest areas.
- **The training set is one device.** Cartography differs between the Edge and wrist
  style variants (FR-CART6), so a fēnix build of the same area is not the same size.
  Wrist builds are not yet in the training set.
- **Linear.** Label deduplication and compression are not linear in feature count. Over
  the 64–576 km² range sampled this does not show; far outside it, expect worse.
- **R-tree counts are bbox-overlap counts**, slightly more than the features a build
  keeps. Since the same counts are used for training and for prediction, this biases
  the coefficients rather than the estimates.

---

# Build time

The same 16 builds are timed per stage, which seeds the "time remaining" figure
(FR-70a, FR-70b). Durations ranged **21 s to 126 s, median 61 s**.

| Stage | Mean share | Min | Max |
|---|---:|---:|---:|
| extract | 3.7 % | 0.6 % | 11.1 % |
| elevation | 32.5 % | 5.6 % | 66.6 % |
| contours | 43.4 % | 8.3 % | 74.6 % |
| relief | 0.7 % | 0.0 % | 3.0 % |
| split | 5.6 % | 1.7 % | 16.4 % |
| compile | 14.1 % | 5.8 % | 33.8 % |
| verify | 0.0 % | 0.0 % | 0.1 % |

These are **cold-cache** figures: the elevation cache had been cleared, so every area
downloaded its own tiles. That is the case worth seeding, because it is the slow one and
the one a first-time user meets. On a warm cache the shape is completely different —
three repeat builds of the same area measured contours at 82 % and elevation at 4 % —
which is why the spread above is so wide and why the estimate does two things rather
than trusting the seed:

1. It re-extrapolates from elapsed time and the weighted fraction as the build proceeds,
   so it converges even when the seed is wrong for this machine.
2. `stage_weights` replaces the seed entirely once the local calibration log has samples,
   so a user who rebuilds the same areas ends up with warm-cache weights.

The weights are averaged as fractions of each build rather than as raw seconds, so one
126 s build does not outvote five 25 s ones: they describe shape, not duration.

`the_seed_weights_are_reproducible_from_the_shipped_training_data` recomputes the shipped
constant from `estimator/training-samples.jsonl`, so the table above cannot drift from
the constant it documents.
