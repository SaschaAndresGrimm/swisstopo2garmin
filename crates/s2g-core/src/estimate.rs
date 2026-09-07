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

/// Bytes per km² added by the slope classes.
///
/// Not a fitted coefficient. It is measured from three paired builds — the same recipe
/// with and without the classes — because refitting the whole model would have meant
/// rebuilding the training set:
///
/// | area | km² | with | without | added | B/km² |
/// |---|---:|---:|---:|---:|---:|
/// | Grindelwald | 144 | 1,559,040 | 1,089,024 | 470,016 | 3,264 |
/// | Zermatt | 100 | 1,184,256 | 822,784 | 361,472 | 3,615 |
/// | Andermatt | 100 | 1,153,024 | 770,560 | 382,464 | 3,825 |
///
/// All three are steep alpine terrain, which is the worst case: slope classes only
/// exist above 30°, so a Mittelland build adds far less. The error is therefore on the
/// side of over-estimating, which is the safe side for a device budget.
pub const SLOPE_BYTES_PER_KM2: f64 = 3_568.0;

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
    /// Slope classes over 30°, which add a large amount of polygon (FR-CART12).
    /// Defaults so logs written before slope classes existed still load.
    #[serde(default)]
    pub slope_classes: bool,
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
    /// Seconds per stage, keyed by the lowercase stage name. Absent in logs written
    /// before build timing existed, which is why it defaults rather than being required.
    #[serde(default)]
    pub stage_seconds: Vec<(String, f64)>,
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

/// What the slope classes add to a build of this area.
///
/// Kept outside the fitted model deliberately: it is added here and subtracted before
/// fitting, so a user who builds with slope classes on does not have their extra bytes
/// attributed to the feature-count coefficients.
fn slope_adjustment(p: &Predictors) -> f64 {
    if p.slope_classes {
        p.area_km2 * SLOPE_BYTES_PER_KM2
    } else {
        0.0
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
        (y + slope_adjustment(p)).max(0.0) as u64
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
            // The slope term is not fitted, so its contribution is removed before the
            // rest of the model is asked to explain the bytes.
            let y = (s.actual_bytes as f64 - slope_adjustment(&s.predictors)).max(0.0);
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

    /// Leave-one-out cross-validated error for a given ridge strength.
    ///
    /// In-sample error always improves as the ridge weakens, so it cannot choose one:
    /// the 11-term model has enough freedom to fit 16 builds closely while predicting
    /// an unseen area badly. This refits without each sample and scores it on the one
    /// left out, which is what "±25 % on an area you have not built" actually means.
    pub fn loocv_mape(samples: &[Sample], prior: &SizeModel, lambda: f64) -> f64 {
        if samples.len() < 3 {
            return f64::INFINITY;
        }
        let mut sum = 0.0;
        for i in 0..samples.len() {
            let rest: Vec<Sample> = samples
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, s)| s.clone())
                .collect();
            let m = SizeModel::fit(&rest, prior, lambda);
            let actual = samples[i].actual_bytes as f64;
            if actual > 0.0 {
                sum += (m.predict(&samples[i].predictors) as f64 - actual).abs() / actual;
            }
        }
        sum / samples.len() as f64
    }

    /// Fit with the ridge strength that cross-validates best.
    ///
    /// Returns the model and the chosen lambda. The grid spans "essentially free" to
    /// "essentially the prior", so the data decides how much it is trusted.
    pub fn fit_cv(samples: &[Sample], prior: &SizeModel) -> (SizeModel, f64) {
        const GRID: [f64; 11] = [0.0, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11];
        let best = GRID
            .iter()
            .map(|l| (*l, SizeModel::loocv_mape(samples, prior, *l)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(l, _)| l)
            .unwrap_or(1e6);
        (SizeModel::fit(samples, prior, best), best)
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

/// Fraction of a build's time spent in each stage, in [`crate::pipeline::Stage`] order.
///
/// Measured from the 16 builds in `estimator/training-samples.jsonl` — the same set the
/// size model is fitted on — with a **cold** elevation cache, which is the case worth
/// seeding: on a warm cache contours dominate instead, and that is the case the estimate
/// corrects itself into within seconds.
///
/// Builds ranged 21 s to 126 s, median 61 s. The spread per stage is wide, which is the
/// point of refitting from the local log:
///
/// | stage | mean | min | max |
/// |---|---:|---:|---:|
/// | extract | 0.037 | 0.006 | 0.111 |
/// | elevation | 0.325 | 0.056 | 0.666 |
/// | contours | 0.434 | 0.083 | 0.746 |
/// | relief | 0.007 | 0.000 | 0.030 |
/// | split | 0.056 | 0.017 | 0.164 |
/// | compile | 0.141 | 0.058 | 0.338 |
/// | verify | 0.000 | 0.000 | 0.001 |
///
/// Two things correct for a seed that does not match a given machine: the estimate
/// re-extrapolates from elapsed time as the build proceeds, and [`stage_weights`]
/// replaces these entirely once the user's calibration log has samples of its own.
///
/// Averaged as fractions of each build, exactly as [`stage_weights`] averages the log,
/// so a measured seed and a refitted one mean the same thing.
/// A way of getting a too-large map under the device budget.
///
/// An enum rather than a sentence, because the UI translates into four languages and a
/// remedy that does not apply to the recipe at hand is worse than no advice: telling
/// somebody to coarsen contours that are already off reads as the app not knowing what
/// it is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Remedy {
    /// Select less ground. Always available.
    SmallerArea,
    /// Widen the contour interval. Contours are the largest single contributor.
    CoarserContours,
    /// Turn off layers that are currently on.
    FewerLayers,
    /// Drop the shaded-relief DEM, which is a fixed cost per square kilometre.
    NoRelief,
    /// Drop the slope classes.
    NoSlopeClasses,
    /// Build several map sets and let the device switch between them (FR-36).
    SplitIntoMapSets,
}

/// Whether a predicted size fits the device, and what would help if it does not
/// (SPEC.md §12, "Estimate exceeds device limit — block with explanation; offer smaller
/// area, coarser contours, fewer layers, or split into map sets").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetVerdict {
    pub over_budget: bool,
    /// How much has to go, zero when it fits.
    pub overshoot_bytes: u64,
    /// Only the remedies that would actually change this recipe, most effective first.
    pub remedies: Vec<Remedy>,
}

/// Judge a predicted size against a device budget.
///
/// Ordered by how much each one saves on a typical build, from the measured stage
/// weights: contours dominate, then the area itself, then relief, then the rest.
/// `SplitIntoMapSets` comes last because it is the only one that keeps every bit of the
/// map the user asked for, and so is the fallback rather than the first suggestion.
pub fn budget_verdict(estimated_bytes: u64, budget_bytes: u64, recipe: &Recipe) -> BudgetVerdict {
    if estimated_bytes <= budget_bytes {
        return BudgetVerdict {
            over_budget: false,
            overshoot_bytes: 0,
            remedies: Vec::new(),
        };
    }

    let mut remedies = Vec::new();
    // Contours first: they are 43% of build time and the largest part of the output.
    // Only worth suggesting when there is room to coarsen, which 100 m already lacks.
    if recipe.contours.interval_m > 0 && recipe.contours.interval_m < 100 {
        remedies.push(Remedy::CoarserContours);
    }
    remedies.push(Remedy::SmallerArea);
    if recipe.relief != ReliefDetail::Off {
        remedies.push(Remedy::NoRelief);
    }
    if recipe.slope_classes {
        remedies.push(Remedy::NoSlopeClasses);
    }
    // "Fewer layers" is only honest advice when some are still on. With everything
    // already excluded there is nothing left to turn off.
    if recipe.excluded_layers.len() < crate::extract::DEFAULT_LAYERS.len() {
        remedies.push(Remedy::FewerLayers);
    }
    remedies.push(Remedy::SplitIntoMapSets);

    BudgetVerdict {
        over_budget: true,
        overshoot_bytes: estimated_bytes - budget_bytes,
        remedies,
    }
}

pub const DEFAULT_STAGE_WEIGHTS: [f64; 7] = [
    0.0372, // extract
    0.3254, // elevation
    0.4336, // contours
    0.0068, // relief
    0.0558, // split
    0.1411, // compile
    0.0001, // verify
];

/// Stage names in the order [`DEFAULT_STAGE_WEIGHTS`] uses.
pub const STAGE_NAMES: [&str; 7] = [
    "extract",
    "elevation",
    "contours",
    "relief",
    "split",
    "compile",
    "verify",
];

/// Time weights per stage, averaged over the samples that recorded any.
///
/// Averaged as *fractions of each build* rather than as raw seconds, so one large
/// build does not drown out ten small ones — the weights describe shape, not duration.
pub fn stage_weights(samples: &[Sample]) -> [f64; 7] {
    let mut sums = [0.0f64; 7];
    let mut n = 0usize;
    for s in samples {
        let total: f64 = s.stage_seconds.iter().map(|(_, v)| *v).sum();
        if total <= 0.0 {
            continue;
        }
        for (name, secs) in &s.stage_seconds {
            if let Some(i) = STAGE_NAMES.iter().position(|x| x == name) {
                sums[i] += secs / total;
            }
        }
        n += 1;
    }
    if n == 0 {
        return DEFAULT_STAGE_WEIGHTS;
    }
    let mut out = [0.0; 7];
    let mut total = 0.0;
    for i in 0..7 {
        out[i] = sums[i] / n as f64;
        total += out[i];
    }
    if total <= 0.0 {
        return DEFAULT_STAGE_WEIGHTS;
    }
    for w in out.iter_mut() {
        *w /= total;
    }
    out
}

/// Fraction of a build complete, from the stage it is in and its progress within it.
pub fn build_fraction(weights: &[f64; 7], stage_index: usize, within: f64) -> f64 {
    let done: f64 = weights.iter().take(stage_index.min(7)).sum();
    let current = weights.get(stage_index).copied().unwrap_or(0.0);
    (done + current * within.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// Seconds remaining, from elapsed time and the fraction complete.
///
/// `None` until enough of the build has happened for the extrapolation to mean
/// anything: a "4 hours remaining" flashed in the first second is worse than no number.
pub fn eta_seconds(elapsed: f64, fraction: f64) -> Option<f64> {
    if fraction < 0.02 || elapsed < 2.0 || fraction >= 1.0 {
        return None;
    }
    Some(elapsed * (1.0 - fraction) / fraction)
}

/// Where the local calibration log lives, beside the dataset cache.
pub fn calibration_log_path() -> std::path::PathBuf {
    crate::cache::Cache::default_root().join("calibration.jsonl")
}

/// The model to use: the shipped prior, refit from whatever the local log holds.
pub fn current_model(shipped: &Path, log: &Path) -> SizeModel {
    let prior = SizeModel::load(shipped).unwrap_or_default();
    let samples = CalibrationLog::read(log);
    // Below three samples there is nothing to cross-validate against, and a fixed
    // strong ridge is the right answer: barely move the shipped model. λ is in the
    // units of XᵀX, whose entries are squared feature counts.
    if samples.len() < 3 {
        return SizeModel::fit(&samples, &prior, 1e9);
    }
    SizeModel::fit_cv(&samples, &prior).0
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
        for path in crate::datasets::winter_geopackages(root) {
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

    if recipe.preset.needs_cycle() {
        for path in crate::datasets::route_shapefiles(root) {
            let stem = path
                .file_stem()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default();
            let Some(spec) = crate::extract::CYCLE_LAYERS
                .iter()
                .find(|sp| sp.layer == stem)
            else {
                continue;
            };
            if !keep(spec.layer) {
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

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn predictors(
        counts: &[(&str, u64)],
        area: f64,
        interval: i32,
        relief: ReliefDetail,
    ) -> Predictors {
        Predictors {
            group_counts: counts.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            area_km2: area,
            contour_interval_m: interval,
            relief,
            slope_classes: false,
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
                stage_seconds: Vec::new(),
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
        let p = predictors(
            &[("transport", 20_000), ("built", 40_000)],
            144.0,
            20,
            ReliefDetail::Gentle,
        );
        let predicted = prior.predict(&p);
        let sample = Sample {
            predictors: p.clone(),
            actual_bytes: predicted * 2,
            stage_seconds: Vec::new(),
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
                stage_seconds: Vec::new(),
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
                stage_seconds: Vec::new(),
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
            stage_seconds: vec![("contours".into(), 12.5)],
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
    fn cross_validation_prefers_a_weak_ridge_when_the_data_is_clean() {
        // Noiseless samples from a known model: the data deserves to be trusted, so
        // the chosen lambda must be at the weak end and the fit near-exact.
        let truth = SizeModel {
            coefficients: vec![
                50_000.0, 10.0, 5.0, 20.0, 8.0, 30.0, 12.0, 15.0, 2_000.0, 800.0, 200.0,
            ],
            samples: 0,
        };
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut rng = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let samples: Vec<Sample> = (0..30)
            .map(|i| {
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
                Sample {
                    actual_bytes: truth.predict(&p),
                    predictors: p,
                    stage_seconds: Vec::new(),
                    at: 0,
                }
            })
            .collect();

        let (fitted, lambda) = SizeModel::fit_cv(&samples, &SizeModel::default());
        assert!(
            lambda <= 1e3,
            "clean data should not be regularised hard: {lambda}"
        );
        assert!(fitted.mape(&samples) < 1e-3);
    }

    #[test]
    fn fit_cv_picks_the_grid_minimum_and_stays_sane_on_noise() {
        // Sizes unrelated to the predictors. There is no "right" lambda here, so the
        // contract is only that the chosen one really is the cross-validated best and
        // that the resulting model is still usable.
        let prior = SizeModel::default();
        let mut seed = 12345u64;
        let mut rng = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let samples: Vec<Sample> = (0..12)
            .map(|_| Sample {
                predictors: predictors(
                    &[("landCover", rng() % 30_000), ("built", rng() % 50_000)],
                    50.0 + (rng() % 200) as f64,
                    20,
                    ReliefDetail::Gentle,
                ),
                actual_bytes: 500_000 + rng() % 3_000_000,
                stage_seconds: Vec::new(),
                at: 0,
            })
            .collect();

        let (fitted, lambda) = SizeModel::fit_cv(&samples, &prior);
        let chosen = SizeModel::loocv_mape(&samples, &prior, lambda);
        for other in [0.0, 1e3, 1e6, 1e9, 1e11] {
            let score = SizeModel::loocv_mape(&samples, &prior, other);
            assert!(
                chosen <= score + 1e-12,
                "lambda {lambda} scored {chosen} but {other} scored {score}"
            );
        }
        assert!(fitted
            .coefficients
            .iter()
            .all(|c| c.is_finite() && *c >= 0.0));
        assert_eq!(fitted.coefficients.len(), TERMS);
    }

    #[test]
    fn loocv_needs_at_least_three_samples() {
        let prior = SizeModel::default();
        let s = Sample {
            predictors: predictors(&[("water", 10)], 1.0, 20, ReliefDetail::Off),
            actual_bytes: 1_000,
            stage_seconds: Vec::new(),
            at: 0,
        };
        assert!(SizeModel::loocv_mape(&[s.clone(), s], &prior, 1e6).is_infinite());
    }

    fn timed(stages: &[(&str, f64)]) -> Sample {
        Sample {
            predictors: predictors(&[], 100.0, 20, ReliefDetail::Off),
            actual_bytes: 1_000_000,
            stage_seconds: stages.iter().map(|(n, v)| ((*n).to_string(), *v)).collect(),
            at: 0,
        }
    }

    /// The seed weights must be a distribution, or every estimate is scaled wrongly.
    #[test]
    fn the_default_stage_weights_sum_to_one() {
        let total: f64 = DEFAULT_STAGE_WEIGHTS.iter().sum();
        assert!((total - 1.0).abs() < 1e-3, "weights sum to {total}");
        assert_eq!(DEFAULT_STAGE_WEIGHTS.len(), STAGE_NAMES.len());
        assert!(DEFAULT_STAGE_WEIGHTS.iter().all(|w| *w >= 0.0));
    }

    /// The constant must be exactly what the shipped training data produces.
    ///
    /// It is a measurement, so it has to be reproducible from the data it was measured
    /// from — otherwise the doc comment above becomes a claim nobody can check.
    #[test]
    fn the_seed_weights_are_reproducible_from_the_shipped_training_data() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../estimator/training-samples.jsonl");
        let samples = CalibrationLog::read(&path);
        assert!(
            samples.len() >= 16,
            "expected the shipped training set, found {} samples at {}",
            samples.len(),
            path.display()
        );
        assert!(
            samples.iter().all(|s| !s.stage_seconds.is_empty()),
            "every training sample must carry stage timings"
        );

        let derived = stage_weights(&samples);
        for (i, (a, b)) in derived.iter().zip(DEFAULT_STAGE_WEIGHTS.iter()).enumerate() {
            assert!(
                (a - b).abs() < 5e-4,
                "stage {} ({}): documented {b}, recomputed {a}",
                i,
                STAGE_NAMES[i]
            );
        }
    }

    #[test]
    fn stage_weights_fall_back_to_the_shipped_defaults_without_data() {
        assert_eq!(stage_weights(&[]), DEFAULT_STAGE_WEIGHTS);
        // Samples from before timing existed carry no stages and must not count.
        assert_eq!(stage_weights(&[timed(&[])]), DEFAULT_STAGE_WEIGHTS);
    }

    #[test]
    fn stage_weights_are_shape_not_duration() {
        // Same shape, wildly different durations: one long build must not outvote the
        // short ones, so the weights must come out identical either way.
        let short = timed(&[("extract", 1.0), ("contours", 3.0)]);
        let long = timed(&[("extract", 100.0), ("contours", 300.0)]);
        let a = stage_weights(std::slice::from_ref(&short));
        let b = stage_weights(&[short, long]);
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 1e-9, "{a:?} vs {b:?}");
        }
        assert!((a[0] - 0.25).abs() < 1e-9, "{a:?}");
        assert!((a[2] - 0.75).abs() < 1e-9, "{a:?}");
        assert!((a.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_unknown_stage_name_is_ignored_rather_than_shifting_every_weight() {
        let w = stage_weights(&[timed(&[("extract", 1.0), ("teleport", 9.0)])]);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((w[0] - 1.0).abs() < 1e-9, "{w:?}");
    }

    #[test]
    fn build_fraction_accumulates_completed_stages() {
        let w = DEFAULT_STAGE_WEIGHTS;
        assert_eq!(build_fraction(&w, 0, 0.0), 0.0);
        assert!((build_fraction(&w, 1, 0.0) - w[0]).abs() < 1e-9);
        assert!((build_fraction(&w, 1, 0.5) - (w[0] + w[1] * 0.5)).abs() < 1e-9);
        // The last stage finishing means done, and out-of-range input cannot exceed 1.
        assert!((build_fraction(&w, 6, 1.0) - 1.0).abs() < 1e-9);
        assert_eq!(build_fraction(&w, 99, 1.0), 1.0);
    }

    #[test]
    fn no_eta_is_offered_until_it_would_mean_something() {
        // A "four hours remaining" flashed in the first second is worse than nothing.
        assert_eq!(eta_seconds(0.5, 0.5), None);
        assert_eq!(eta_seconds(10.0, 0.001), None);
        assert_eq!(eta_seconds(10.0, 1.0), None);

        // Half done after 10 s means about 10 s to go.
        let eta = eta_seconds(10.0, 0.5).unwrap();
        assert!((eta - 10.0).abs() < 1e-9, "{eta}");
        // A quarter done after 30 s means about 90 s to go.
        let eta = eta_seconds(30.0, 0.25).unwrap();
        assert!((eta - 90.0).abs() < 1e-9, "{eta}");
    }

    #[test]
    fn a_sample_written_before_timing_existed_still_loads() {
        let old = r#"{"predictors":{"groupCounts":{},"areaKm2":100.0,"contourIntervalM":20,"relief":"off"},"actualBytes":1000}"#;
        let s: Sample = serde_json::from_str(old).expect("old log lines must still parse");
        assert!(s.stage_seconds.is_empty());
        assert_eq!(s.at, 0);
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

#[cfg(test)]
mod slope_tests {
    use super::*;

    fn predictors(area_km2: f64, slope: bool) -> Predictors {
        Predictors {
            group_counts: Default::default(),
            area_km2,
            contour_interval_m: 20,
            relief: ReliefDetail::Off,
            slope_classes: slope,
        }
    }

    /// The measured cost must actually reach the estimate.
    #[test]
    fn slope_classes_add_their_measured_cost_per_square_kilometre() {
        let m = SizeModel::default();
        let plain = m.predict(&predictors(144.0, false));
        let with_slope = m.predict(&predictors(144.0, true));
        let added = with_slope - plain;
        let expected = (144.0 * SLOPE_BYTES_PER_KM2) as u64;
        assert_eq!(added, expected, "expected {expected} added, got {added}");

        // And it scales with area, since the classes cover terrain.
        let bigger = m.predict(&predictors(288.0, true)) - m.predict(&predictors(288.0, false));
        assert_eq!(bigger, added * 2);
    }

    /// The measurement it was taken from must still hold, within the observed spread.
    #[test]
    fn the_slope_estimate_matches_the_paired_builds_it_came_from() {
        // area km², measured with, measured without.
        let observed = [
            (144.0, 1_559_040u64, 1_089_024u64),
            (100.0, 1_184_256, 822_784),
            (100.0, 1_153_024, 770_560),
        ];
        for (km2, with_s, without) in observed {
            let predicted_extra = km2 * SLOPE_BYTES_PER_KM2;
            let actual_extra = (with_s - without) as f64;
            let err = (predicted_extra - actual_extra).abs() / actual_extra;
            assert!(
                err < 0.12,
                "{km2} km²: predicted {predicted_extra:.0} against {actual_extra:.0}, {:.0}% out",
                err * 100.0
            );
        }
    }

    /// Fitting must not attribute slope bytes to the feature coefficients.
    ///
    /// Without subtracting the term first, a user who builds with the classes on would
    /// teach the model that land cover costs several times what it does.
    #[test]
    fn fitting_ignores_the_slope_contribution() {
        let prior = SizeModel::default();
        let base = predictors(144.0, false);
        let truth = prior.predict(&base);

        // The same build, once without the classes and once with, both recorded truly.
        let without = Sample {
            predictors: base.clone(),
            actual_bytes: truth,
            stage_seconds: Vec::new(),
            at: 0,
        };
        let with_slope = Sample {
            predictors: predictors(144.0, true),
            actual_bytes: truth + (144.0 * SLOPE_BYTES_PER_KM2) as u64,
            stage_seconds: Vec::new(),
            at: 0,
        };

        // Both samples agree with the prior once the slope term is removed, so the fit
        // should stay put rather than being dragged by the slope build.
        let fitted = SizeModel::fit(&[without, with_slope], &prior, 1e7);
        for (a, b) in fitted.coefficients.iter().zip(prior.coefficients.iter()) {
            assert!(
                (a - b).abs() < 1.0,
                "fit moved from {b} to {a} on data that agrees with it"
            );
        }
    }

    #[test]
    fn a_log_written_before_slope_classes_existed_still_loads() {
        let old = r#"{"predictors":{"groupCounts":{},"areaKm2":100.0,"contourIntervalM":20,"relief":"off"},"actualBytes":1000}"#;
        let s: Sample = serde_json::from_str(old).expect("older log lines must parse");
        assert!(!s.predictors.slope_classes);
    }

    // ---- budget verdicts (SPEC.md §12) ----------------------------------

    fn budget_recipe() -> Recipe {
        let mut r = Recipe::new(
            "test",
            "fenix-5-plus",
            crate::recipe::AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_190_000.0,
                max_e: 2_650_000.0,
                max_n: 1_240_000.0,
            },
        );
        r.contours.interval_m = 20;
        r.relief = ReliefDetail::Gentle;
        r
    }

    #[test]
    fn a_map_that_fits_gets_no_advice() {
        let v = budget_verdict(50_000_000, 100_000_000, &budget_recipe());
        assert!(!v.over_budget);
        assert_eq!(v.overshoot_bytes, 0);
        assert!(
            v.remedies.is_empty(),
            "advice was offered for a map that fits"
        );
    }

    /// Exactly at the budget fits: the budget already carries a safety factor from the
    /// profile's confidence level, so subtracting a second margin here would compound it.
    #[test]
    fn a_map_exactly_at_the_budget_fits() {
        assert!(!budget_verdict(100_000_000, 100_000_000, &budget_recipe()).over_budget);
        assert!(budget_verdict(100_000_001, 100_000_000, &budget_recipe()).over_budget);
    }

    #[test]
    fn an_oversized_map_reports_how_much_has_to_go() {
        let v = budget_verdict(150_000_000, 100_000_000, &budget_recipe());
        assert!(v.over_budget);
        assert_eq!(v.overshoot_bytes, 50_000_000);
    }

    /// SPEC.md §12 names four remedies. Contours first, because they are the largest
    /// part of the output; splitting last, because it is the only one that keeps
    /// everything the user asked for.
    #[test]
    fn the_remedies_are_ordered_by_what_they_save() {
        let v = budget_verdict(150_000_000, 100_000_000, &budget_recipe());
        assert_eq!(v.remedies.first(), Some(&Remedy::CoarserContours));
        assert_eq!(v.remedies.last(), Some(&Remedy::SplitIntoMapSets));
        assert!(v.remedies.contains(&Remedy::SmallerArea));
        assert!(v.remedies.contains(&Remedy::FewerLayers));
    }

    /// Advice that cannot be followed reads as the app not understanding its own state.
    #[test]
    fn remedies_that_would_change_nothing_are_not_offered() {
        let mut r = budget_recipe();
        r.contours.interval_m = 0;
        r.relief = ReliefDetail::Off;
        r.slope_classes = false;
        r.excluded_layers = crate::extract::DEFAULT_LAYERS
            .iter()
            .map(|l| l.layer.to_string())
            .collect();

        let v = budget_verdict(150_000_000, 100_000_000, &r);
        assert!(
            !v.remedies.contains(&Remedy::CoarserContours),
            "contours are off"
        );
        assert!(!v.remedies.contains(&Remedy::NoRelief), "relief is off");
        assert!(
            !v.remedies.contains(&Remedy::NoSlopeClasses),
            "slopes are off"
        );
        assert!(
            !v.remedies.contains(&Remedy::FewerLayers),
            "every layer is excluded"
        );
        // Two always remain, and they are always true.
        assert_eq!(
            v.remedies,
            vec![Remedy::SmallerArea, Remedy::SplitIntoMapSets]
        );
    }

    /// 100 m is the coarsest interval the UI offers, so there is nothing to coarsen.
    #[test]
    fn contours_already_at_the_coarsest_interval_are_not_suggested() {
        let mut r = budget_recipe();
        r.contours.interval_m = 100;
        let v = budget_verdict(150_000_000, 100_000_000, &r);
        assert!(!v.remedies.contains(&Remedy::CoarserContours));
    }

    #[test]
    fn slope_classes_are_offered_as_a_saving_only_when_they_are_on() {
        let mut r = budget_recipe();
        r.slope_classes = true;
        assert!(budget_verdict(150_000_000, 100_000_000, &r)
            .remedies
            .contains(&Remedy::NoSlopeClasses));
    }
}
