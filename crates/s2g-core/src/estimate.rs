//! Calibrated output-size estimation (SPEC.md FR-60..FR-63).
//!
//! The estimate is a fitted linear model, not a bytes-per-km² guess. Its predictors are
//! things that can be obtained in milliseconds before a build: per-group feature counts
//! from the GeoPackage R-tree indexes, the area, the contour interval and the relief
//! setting. Coefficients ship fitted from real builds and are refit from the user's own
//! builds as they accumulate (FR-62).
//!
//! Known limitation: contour output is modelled from area and interval alone, because
//! nothing cheap reveals terrain roughness before the elevation tiles are fetched. An
//! Alpine square kilometre carries far more contour line than a Mittelland one, so the
//! contour term is the largest source of error until the local log has seen both.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::extract::LayerGroup;
use crate::recipe::{Recipe, ReliefDetail};

/// Number of fitted coefficients: intercept, one per layer group, contour, two relief.
const TERMS: usize = 1 + 7 + 1 + 2;

/// What a build's size is predicted from. Cheap to compute for a candidate area.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Predictors {
    /// Feature counts per layer group, keyed by [`LayerGroup::id`].
    pub group_counts: BTreeMap<String, u64>,
    pub area_km2: f64,
    /// 0 when contours are off.
    pub contour_interval_m: i32,
    pub relief: ReliefDetail,
}

impl Predictors {
    /// The design-matrix row, in the same order as [`SizeModel::coefficients`].
    fn row(&self) -> [f64; TERMS] {
        let mut x = [0.0; TERMS];
        x[0] = 1.0;
        for (i, g) in LayerGroup::all().iter().enumerate() {
            x[1 + i] = *self.group_counts.get(g.id()).unwrap_or(&0) as f64;
        }
        // Contour length scales with area and inversely with the interval. Normalised
        // at 20 m so the coefficient reads as "bytes per km² at a 20 m interval".
        x[8] = if self.contour_interval_m > 0 {
            self.area_km2 * (20.0 / self.contour_interval_m as f64)
        } else {
            0.0
        };
        x[9] = match self.relief {
            ReliefDetail::Detailed => self.area_km2,
            _ => 0.0,
        };
        x[10] = match self.relief {
            ReliefDetail::Gentle => self.area_km2,
            _ => 0.0,
        };
        x
    }
}

/// One observed build: what was predicted from, and what it actually produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub predictors: Predictors,
    pub actual_bytes: u64,
    /// Unix seconds, for pruning an old log.
    #[serde(default)]
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeModel {
    /// In design-matrix order: intercept, the seven layer groups in
    /// [`LayerGroup::all`] order, contour, relief-detailed, relief-gentle.
    pub coefficients: Vec<f64>,
    /// How many real builds this was fitted from. 0 means the shipped prior only.
    pub samples: usize,
}

impl Default for SizeModel {
    /// The shipped prior, fitted from the builds recorded in docs/size-model.md.
    ///
    /// It is a starting point, not a claim of accuracy on an unseen area: the local
    /// calibration log takes over as soon as the user has built anything.
    fn default() -> Self {
        Self {
            coefficients: vec![
                48_000.0, // intercept: the fixed cost of a one-tile map
                14.0,     // landCover
                12.0,     // water
                18.0,     // transport
                9.0,      // built
                26.0,     // names
                16.0,     // winter
                16.0,     // cycling
                1_900.0,  // contour bytes per km² at 20 m
                760.0,    // relief bytes per km², 1 arc-second
                150.0,    // relief bytes per km², 3 arc-second
            ],
            samples: 0,
        }
    }
}

impl SizeModel {
    pub fn predict(&self, p: &Predictors) -> u64 {
        let x = p.row();
        let y: f64 = x
            .iter()
            .zip(self.coefficients.iter())
            .map(|(a, b)| a * b)
            .sum();
        y.max(0.0) as u64
    }

    /// Refit from samples, pulled toward `prior` by `lambda` (ridge regression).
    ///
    /// Regularising toward the prior rather than toward zero is what makes this usable
    /// after a single build: with few samples the fit stays near the shipped model and
    /// moves only where the data actually disagrees. An unregularised fit on three
    /// samples and eleven terms would be arbitrary.
    pub fn fit(samples: &[Sample], prior: &SizeModel, lambda: f64) -> SizeModel {
        if samples.is_empty() {
            return prior.clone();
        }
        let b0 = &prior.coefficients;

        // Normal equations with a ridge pulling toward the prior:
        //   (XᵀX + λI) β = Xᵀy + λβ₀
        let mut a = [[0.0f64; TERMS]; TERMS];
        let mut rhs = [0.0f64; TERMS];
        for s in samples {
            let x = s.predictors.row();
            let y = s.actual_bytes as f64;
            for i in 0..TERMS {
                rhs[i] += x[i] * y;
                for j in 0..TERMS {
                    a[i][j] += x[i] * x[j];
                }
            }
        }
        for i in 0..TERMS {
            a[i][i] += lambda;
            rhs[i] += lambda * b0.get(i).copied().unwrap_or(0.0);
        }

        let beta = match solve(a, rhs) {
            Some(b) => b,
            // A singular system means the samples carry no information for some term.
            // Keeping the prior is the honest response.
            None => return prior.clone(),
        };

        SizeModel {
            // A negative coefficient would say "more of this makes the map smaller",
            // which is never true and would produce absurd estimates off the fitted range.
            coefficients: beta.iter().map(|b| b.max(0.0)).collect(),
            samples: samples.len(),
        }
    }

    /// Mean absolute percentage error over samples, for reporting honesty about the fit.
    pub fn mape(&self, samples: &[Sample]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = samples
            .iter()
            .map(|s| {
                let p = self.predict(&s.predictors) as f64;
                let a = s.actual_bytes as f64;
                if a > 0.0 {
                    (p - a).abs() / a
                } else {
                    0.0
                }
            })
            .sum();
        sum / samples.len() as f64
    }

    pub fn load(path: &Path) -> Result<SizeModel> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        let m: SizeModel = serde_json::from_slice(&bytes)?;
        if m.coefficients.len() != TERMS {
            return Err(Error::NotFound(format!(
                "size model at {} has {} coefficients, expected {TERMS}",
                path.display(),
                m.coefficients.len()
            )));
        }
        Ok(m)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?).map_err(|e| Error::io(path, e))
    }
}

/// Gaussian elimination with partial pivoting. `None` when the system is singular.
fn solve(mut a: [[f64; TERMS]; TERMS], mut b: [f64; TERMS]) -> Option<[f64; TERMS]> {
    for col in 0..TERMS {
        let (pivot, _) = (col..TERMS)
            .map(|r| (r, a[r][col].abs()))
            .max_by(|x, y| x.1.total_cmp(&y.1))?;
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for r in (col + 1)..TERMS {
            let f = a[r][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            let (top, rest) = a.split_at_mut(r);
            for (dst, src) in rest[0][col..].iter_mut().zip(top[col][col..].iter()) {
                *dst -= f * src;
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; TERMS];
    for r in (0..TERMS).rev() {
        let mut v = b[r];
        for c in (r + 1)..TERMS {
            v -= a[r][c] * x[c];
        }
        x[r] = v / a[r][r];
    }
    Some(x)
}

/// The append-only local calibration log (FR-62).
///
/// JSON Lines so a build only ever appends: a crash mid-write costs the last line,
/// never the history.
pub struct CalibrationLog;

impl CalibrationLog {
    pub fn append(path: &Path, sample: &Sample) -> Result<()> {
        use std::io::Write;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| Error::io(path, e))?;
        let mut line = serde_json::to_vec(sample)?;
        line.push(b'\n');
        f.write_all(&line).map_err(|e| Error::io(path, e))
    }

    /// Read the log, skipping lines that do not parse rather than failing.
    pub fn read(path: &Path) -> Vec<Sample> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}

/// Where the local calibration log lives, beside the dataset cache.
pub fn calibration_log_path() -> std::path::PathBuf {
    crate::cache::Cache::default_root().join("calibration.jsonl")
}

/// The model to use: the shipped prior, refit from whatever the local log holds.
pub fn current_model(shipped: &Path, log: &Path) -> SizeModel {
    let prior = SizeModel::load(shipped).unwrap_or_default();
    let samples = CalibrationLog::read(log);
    // λ is in the units of XᵀX, whose entries are squared feature counts — tens of
    // millions for a real area. 1e6 leaves a single build able to move the fit a
    // little and a dozen able to move it a lot.
    SizeModel::fit(&samples, &prior, 1e6)
}

/// Count features per layer group for an area.
///
/// swissTLM3D counts come from the R-tree indexes and are exact and instant. Winter
/// counts come from the winter GeoPackages' own R-trees. Cycle counts come from a
/// bbox scan of the ASTRA shapefiles, which have no index — those files are small
/// enough (tens of thousands of records) that a scan is still fast.
///
/// A missing source contributes zero rather than failing: a user may not have
/// downloaded it, and the estimate should still be given with that stated.
pub fn count_groups(
    gpkg: &crate::gpkg::Gpkg,
    recipe: &Recipe,
    cache_root: Option<&Path>,
) -> BTreeMap<String, u64> {
    let bbox = recipe.area.bbox();
    let mut out: BTreeMap<String, u64> = BTreeMap::new();
    let keep = |name: &str| !recipe.excluded_layers.iter().any(|x| x == name);

    for spec in crate::extract::DEFAULT_LAYERS {
        if !keep(spec.layer) {
            continue;
        }
        let n = gpkg.count_in_bbox(spec.layer, &bbox).unwrap_or(0).max(0) as u64;
        *out.entry(spec.group.id().to_string()).or_default() += n;
    }

    let Some(root) = cache_root else {
        return out;
    };

    if recipe.preset.needs_winter() {
        if let Ok(rd) = std::fs::read_dir(root.join("winter")) {
            for e in rd.flatten() {
                let path = e.path();
                if path.extension().map(|x| x != "gpkg").unwrap_or(true) {
                    continue;
                }
                let Ok(src) = crate::gpkg::Gpkg::open(&path) else {
                    continue;
                };
                let Ok(layers) = src.layers() else { continue };
                for layer in layers.iter().filter(|l| l.is_spatial()) {
                    // Layer names carry the release year, so match the spec by prefix.
                    let Some(spec) = crate::extract::WINTER_LAYERS
                        .iter()
                        .find(|sp| layer.name.starts_with(sp.layer))
                    else {
                        continue;
                    };
                    if !keep(spec.layer) {
                        continue;
                    }
                    let n = src.count_in_bbox(&layer.name, &bbox).unwrap_or(0).max(0) as u64;
                    *out.entry(spec.group.id().to_string()).or_default() += n;
                }
            }
        }
    }

    if recipe.preset.needs_cycle() {
        let mut stack = vec![root.join("routes")];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().map(|x| x != "shp").unwrap_or(true) {
                    continue;
                }
                let stem = path
                    .file_stem()
                    .map(|x| x.to_string_lossy().to_string())
                    .unwrap_or_default();
                let Some(spec) = crate::extract::CYCLE_LAYERS.iter().find(|sp| sp.layer == stem)
                else {
                    continue;
                };
                if !keep(spec.layer) {
                    continue;
                }
                // All three ASTRA datasets ship a Route.shp; only the cycle ones count.
                let dataset = path
                    .parent()
                    .and_then(|d| d.parent())
                    .and_then(|d| d.file_name())
                    .map(|x| x.to_string_lossy().to_string())
                    .unwrap_or_default();
                if stem == "Route" && dataset == "wanderland" {
                    continue;
                }
                let Ok(shp) = crate::shapefile::Shapefile::open(&path) else {
                    continue;
                };
                let mut n = 0u64;
                let _ = shp.for_each_in_bbox(&bbox, &[], |_| {
                    n += 1;
                    true
                });
                *out.entry(spec.group.id().to_string()).or_default() += n;
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn predictors(counts: &[(&str, u64)], area: f64, interval: i32, relief: ReliefDetail) -> Predictors {
        Predictors {
            group_counts: counts
                .iter()
                .map(|(k, v)| ((*k).to_string(), *v))
                .collect(),
            area_km2: area,
            contour_interval_m: interval,
            relief,
        }
    }

    /// A synthetic model must be recovered exactly from noiseless samples.
    #[test]
    fn fit_recovers_a_known_linear_model() {
        let truth = SizeModel {
            coefficients: vec![
                50_000.0, 10.0, 5.0, 20.0, 8.0, 30.0, 12.0, 15.0, 2_000.0, 800.0, 200.0,
            ],
            samples: 0,
        };
        // Rows must vary independently: counts that are all multiples of one index
        // leave the design matrix rank-deficient and nothing is recoverable.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut rng = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut samples = Vec::new();
        for i in 0..40u64 {
            let p = predictors(
                &[
                    ("landCover", rng() % 40_000),
                    ("water", rng() % 6_000),
                    ("transport", rng() % 30_000),
                    ("built", rng() % 90_000),
                    ("names", rng() % 4_000),
                    ("winter", rng() % 3_000),
                    ("cycling", rng() % 2_000),
                ],
                20.0 + (rng() % 4_000) as f64 / 4.0,
                [0, 10, 20, 50, 100][(rng() % 5) as usize],
                match i % 3 {
                    0 => ReliefDetail::Off,
                    1 => ReliefDetail::Gentle,
                    _ => ReliefDetail::Detailed,
                },
            );
            let actual = truth.predict(&p);
            samples.push(Sample {
                predictors: p,
                actual_bytes: actual,
                at: 0,
            });
        }
        // No ridge: the data fully determines the model.
        let fitted = SizeModel::fit(&samples, &SizeModel::default(), 0.0);
        for (a, b) in fitted.coefficients.iter().zip(truth.coefficients.iter()) {
            assert!((a - b).abs() < 1e-2, "coefficient {a} != {b}");
        }
        assert!(fitted.mape(&samples) < 1e-6);
        assert_eq!(fitted.samples, 40);
    }

    #[test]
    fn no_samples_keeps_the_prior() {
        let prior = SizeModel::default();
        assert_eq!(SizeModel::fit(&[], &prior, 1e6), prior);
    }

    /// One sample and eleven terms is underdetermined; the ridge must keep the fit sane.
    #[test]
    fn a_single_sample_nudges_rather_than_replaces() {
        let prior = SizeModel::default();
        let p = predictors(&[("transport", 20_000), ("built", 40_000)], 144.0, 20, ReliefDetail::Gentle);
        let predicted = prior.predict(&p);
        let sample = Sample {
            predictors: p.clone(),
            actual_bytes: predicted * 2,
            at: 0,
        };
        let fitted = SizeModel::fit(&[sample], &prior, 1e6);
        let after = fitted.predict(&p);
        assert!(after > predicted, "the fit ignored the observation");
        assert!(
            after < predicted * 2,
            "one observation overwrote the prior: {after} vs {}",
            predicted * 2
        );
        // Terms the sample says nothing about must be left near the prior.
        assert!((fitted.coefficients[6] - prior.coefficients[6]).abs() < 1.0);
    }

    #[test]
    fn coefficients_are_never_negative() {
        let prior = SizeModel::default();
        // An absurd observation that a plain least-squares fit would answer with a
        // negative coefficient.
        let p = predictors(&[("built", 500_000)], 10.0, 20, ReliefDetail::Off);
        let fitted = SizeModel::fit(
            &[Sample {
                predictors: p,
                actual_bytes: 1,
                at: 0,
            }],
            &prior,
            1.0,
        );
        assert!(fitted.coefficients.iter().all(|c| *c >= 0.0));
    }

    #[test]
    fn contours_off_removes_the_contour_term() {
        let m = SizeModel::default();
        let on = predictors(&[], 100.0, 20, ReliefDetail::Off);
        let off = predictors(&[], 100.0, 0, ReliefDetail::Off);
        assert!(m.predict(&on) > m.predict(&off));
        assert_eq!(m.predict(&off), m.coefficients[0] as u64);
    }

    #[test]
    fn a_finer_interval_predicts_more_bytes() {
        let m = SizeModel::default();
        let coarse = predictors(&[], 100.0, 50, ReliefDetail::Off);
        let fine = predictors(&[], 100.0, 10, ReliefDetail::Off);
        assert!(m.predict(&fine) > m.predict(&coarse));
    }

    #[test]
    fn detailed_relief_predicts_more_than_gentle() {
        let m = SizeModel::default();
        let gentle = predictors(&[], 100.0, 20, ReliefDetail::Gentle);
        let detailed = predictors(&[], 100.0, 20, ReliefDetail::Detailed);
        assert!(m.predict(&detailed) > m.predict(&gentle));
    }

    #[test]
    fn a_singular_system_keeps_the_prior_instead_of_producing_nonsense() {
        let prior = SizeModel::default();
        // Every row identical: XᵀX is rank 1 and no ridge is applied.
        let p = predictors(&[("water", 10)], 1.0, 20, ReliefDetail::Off);
        let samples: Vec<_> = (0..5)
            .map(|_| Sample {
                predictors: p.clone(),
                actual_bytes: 1_000,
                at: 0,
            })
            .collect();
        assert_eq!(SizeModel::fit(&samples, &prior, 0.0), prior);
    }

    #[test]
    fn the_log_round_trips_and_survives_a_corrupt_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("calibration.jsonl");
        let s = Sample {
            predictors: predictors(&[("names", 12)], 5.0, 20, ReliefDetail::Gentle),
            actual_bytes: 123_456,
            at: 1_700_000_000,
        };
        CalibrationLog::append(&path, &s).unwrap();
        std::fs::write(
            &path,
            format!(
                "{}\nnot json\n\n",
                String::from_utf8(serde_json::to_vec(&s).unwrap()).unwrap()
            ),
        )
        .unwrap();
        CalibrationLog::append(&path, &s).unwrap();

        let back = CalibrationLog::read(&path);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].actual_bytes, 123_456);
        assert_eq!(back[0].predictors, s.predictors);
    }

    #[test]
    fn a_model_file_with_the_wrong_shape_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.json");
        std::fs::write(&path, br#"{"coefficients": [1.0, 2.0], "samples": 3}"#).unwrap();
        assert!(SizeModel::load(&path).is_err());
        // current_model falls back rather than propagating the error.
        assert_eq!(
            current_model(&path, &dir.path().join("missing.jsonl")),
            SizeModel::default()
        );
    }
}
