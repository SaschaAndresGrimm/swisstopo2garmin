//! Splitting an oversized area into device-legal map sets (SPEC.md FR-36, FR-37).
//!
//! Milestone 0 measured a national build at roughly 336 MB, far inside the 4 GB
//! single-file ceiling, so this is a safety net rather than the common path — and it is
//! written to stay one. It does not restructure the build: each part is an ordinary
//! recipe with a smaller area, built by the ordinary pipeline, which is what makes
//! FR-37's "if set 3 of 5 fails, sets 1–2 are kept" fall out for free.
//!
//! The split is a grid rather than "by canton groups". Cantons would need
//! swissBOUNDARIES3D for a job that has nothing to do with administration, would produce
//! wildly uneven parts (Graubünden against Basel-Stadt), and would leave a user who
//! selected a rectangle wondering why their map came back in the shape of Valais. A grid
//! divides the area the user actually chose.

use crate::estimate::{Predictors, SizeModel};
use crate::proj::BBox;
use crate::recipe::{AreaSelection, Recipe};

/// One piece of a partitioned build.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// 1-based, for naming: "Valais 2 of 4".
    pub index: usize,
    pub total: usize,
    pub recipe: Recipe,
    /// What the estimator predicts for this part alone.
    pub estimated_bytes: u64,
}

/// How an area was divided, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub parts: Vec<Part>,
    pub columns: usize,
    pub rows: usize,
    /// The limit that forced the split, for explaining it.
    pub reason: String,
    /// False when even the largest allowed split leaves parts over budget.
    pub fits: bool,
}

impl Plan {
    pub fn is_split(&self) -> bool {
        self.parts.len() > 1
    }
}

/// Divide a recipe into parts that each fit the device.
///
/// Returns a single-part plan when the whole thing already fits, so a caller can treat
/// both cases the same way.
pub fn plan(
    recipe: &Recipe,
    model: &SizeModel,
    predictors: &Predictors,
    budget_bytes: u64,
    max_tiles: usize,
) -> Plan {
    let whole = model.predict(predictors);
    if whole <= budget_bytes {
        return Plan {
            parts: vec![Part {
                index: 1,
                total: 1,
                recipe: recipe.clone(),
                estimated_bytes: whole,
            }],
            columns: 1,
            rows: 1,
            reason: String::new(),
            fits: true,
        };
    }

    // A square-ish grid with enough cells to fit, plus one step of headroom: the
    // estimate is a model, and being one part short means the whole build fails at the
    // end rather than at the plan.
    // 32 x 32 is 1,024 map sets. Past that the answer is not "more parts", it is that
    // this area cannot be built for this device, and the plan says so rather than
    // producing a number nobody would act on.
    const MAX_SIDE: usize = 32;
    let needed = (whole as f64 / budget_bytes as f64).ceil() as usize;
    let mut side = ((needed as f64).sqrt().ceil() as usize).clamp(1, MAX_SIDE);
    let bbox = recipe.area.bbox();

    // Grow until every part is predicted to fit. Bounded, because a device budget can
    // in principle be smaller than a single part's fixed overhead.
    let mut columns;
    let mut rows;
    loop {
        columns = side;
        rows = side;
        let per_part = scale_predictors(predictors, columns * rows);
        if model.predict(&per_part) <= budget_bytes || side >= MAX_SIDE {
            break;
        }
        side += 1;
    }

    let mut parts = Vec::new();
    let total = columns * rows;
    let w = (bbox.max_e - bbox.min_e) / columns as f64;
    let h = (bbox.max_n - bbox.min_n) / rows as f64;
    for row in 0..rows {
        for col in 0..columns {
            let index = row * columns + col + 1;
            let mut part = recipe.clone();
            part.area = AreaSelection::BBox {
                min_e: bbox.min_e + col as f64 * w,
                min_n: bbox.min_n + row as f64 * h,
                max_e: bbox.min_e + (col + 1) as f64 * w,
                max_n: bbox.min_n + (row + 1) as f64 * h,
            };
            // The name is what distinguishes the files on the device, so it has to
            // carry the part number.
            part.name = format!("{} {index} of {total}", recipe.name);
            parts.push(Part {
                index,
                total,
                estimated_bytes: model.predict(&scale_predictors(predictors, total)),
                recipe: part,
            });
        }
    }

    // Being honest about a plan that does not solve the problem matters more than
    // producing one: a user told "17 files" who then hits the same limit on every one of
    // them has been misled.
    let fits = parts.iter().all(|p| p.estimated_bytes <= budget_bytes);
    let caveat = if fits {
        String::new()
    } else {
        format!(
            " -- and even split {columns} by {rows}, each part is still estimated over \
             the budget, so this area cannot be built for this device"
        )
    };

    Plan {
        parts,
        columns,
        rows,
        reason: format!(
            "the estimate for the whole area is {} against a device budget of {}, \
             and a map set may hold at most {max_tiles} tiles{caveat}",
            bytes(whole),
            bytes(budget_bytes)
        ),
        fits,
    }
}

/// The predictors for one part of an `n`-way split.
///
/// Feature counts and area divide; the contour and relief terms follow the area because
/// they are per-km² already. The fixed per-map overhead does *not* divide, which is why
/// splitting has a cost and why the grid grows until the parts genuinely fit.
fn scale_predictors(p: &Predictors, n: usize) -> Predictors {
    let f = 1.0 / n as f64;
    Predictors {
        group_counts: p
            .group_counts
            .iter()
            .map(|(k, v)| (k.clone(), (*v as f64 * f) as u64))
            .collect(),
        area_km2: p.area_km2 * f,
        contour_interval_m: p.contour_interval_m,
        relief: p.relief,
        slope_classes: p.slope_classes,
        // A split does not change which cartography the device gets.
        wrist: p.wrist,
    }
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", UNITS[i])
}

/// Where a part's own bounding box sits, for showing the split on a map.
pub fn part_bbox(part: &Part) -> BBox {
    part.recipe.area.bbox()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{AreaSelection, Recipe, ReliefDetail};

    fn recipe(area_km2: f64) -> Recipe {
        let side = area_km2.sqrt() * 1000.0;
        Recipe::new(
            "Test",
            "edge-840",
            AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_100_000.0,
                max_e: 2_600_000.0 + side,
                max_n: 1_100_000.0 + side,
            },
        )
    }

    fn predictors(area_km2: f64) -> Predictors {
        Predictors {
            group_counts: [("transport".to_string(), (area_km2 * 200.0) as u64)]
                .into_iter()
                .collect(),
            area_km2,
            contour_interval_m: 20,
            relief: ReliefDetail::Off,
            slope_classes: false,
            wrist: false,
        }
    }

    #[test]
    fn an_area_that_fits_is_not_split() {
        let model = SizeModel::default();
        let p = plan(
            &recipe(100.0),
            &model,
            &predictors(100.0),
            4_000_000_000,
            4096,
        );
        assert!(!p.is_split());
        assert_eq!(p.parts.len(), 1);
        assert_eq!(
            p.parts[0].recipe.name, "Test",
            "a whole build keeps its name"
        );
        assert!(p.reason.is_empty());
    }

    #[test]
    fn an_oversized_area_is_split_until_the_parts_fit() {
        let model = SizeModel::default();
        // A national-scale area against a small budget.
        let area = 41_000.0;
        let p = plan(&recipe(area), &model, &predictors(area), 100_000_000, 4096);

        assert!(p.is_split(), "{} parts", p.parts.len());
        assert_eq!(p.parts.len(), p.columns * p.rows);
        assert!(p.fits, "{}", p.reason);
        for part in &p.parts {
            assert!(
                part.estimated_bytes <= 100_000_000,
                "part {} still over budget at {}",
                part.index,
                part.estimated_bytes
            );
        }
        assert!(p.reason.contains("device budget"), "{}", p.reason);
    }

    /// The parts must tile the original area exactly: no gap, no overlap, no loss.
    #[test]
    fn the_parts_cover_the_original_area_exactly() {
        let model = SizeModel::default();
        let area = 20_000.0;
        let original = recipe(area);
        let p = plan(&original, &model, &predictors(area), 50_000_000, 4096);
        assert!(p.is_split());

        let whole = original.area.bbox();
        let summed: f64 = p.parts.iter().map(|x| part_bbox(x).area_km2()).sum();
        assert!(
            (summed - whole.area_km2()).abs() / whole.area_km2() < 1e-9,
            "parts cover {summed} km² of {} km²",
            whole.area_km2()
        );

        // Corners of the union match the original.
        let min_e = p
            .parts
            .iter()
            .map(|x| part_bbox(x).min_e)
            .fold(f64::INFINITY, f64::min);
        let max_n = p
            .parts
            .iter()
            .map(|x| part_bbox(x).max_n)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((min_e - whole.min_e).abs() < 1e-6);
        assert!((max_n - whole.max_n).abs() < 1e-6);
    }

    #[test]
    fn every_part_is_named_so_the_files_do_not_collide() {
        let model = SizeModel::default();
        let area = 20_000.0;
        let p = plan(&recipe(area), &model, &predictors(area), 50_000_000, 4096);

        let names: std::collections::BTreeSet<_> =
            p.parts.iter().map(|x| x.recipe.name.clone()).collect();
        assert_eq!(
            names.len(),
            p.parts.len(),
            "names must be unique: {names:?}"
        );
        assert!(names.iter().all(|n| n.starts_with("Test ")));

        // And the identities differ, so the device keeps them apart.
        let keys: std::collections::BTreeSet<_> =
            p.parts.iter().map(|x| x.recipe.cache_key()).collect();
        assert_eq!(
            keys.len(),
            p.parts.len(),
            "each part needs its own identity"
        );
    }

    /// Splitting has a cost: the fixed per-map overhead is paid once per part.
    #[test]
    fn splitting_does_not_pretend_the_overhead_divides() {
        let model = SizeModel::default();
        let area = 20_000.0;
        let whole = model.predict(&predictors(area));
        let p = plan(&recipe(area), &model, &predictors(area), 50_000_000, 4096);

        let summed: u64 = p.parts.iter().map(|x| x.estimated_bytes).sum();
        assert!(
            summed > whole,
            "the parts should total more than the whole: {summed} vs {whole}"
        );
    }

    #[test]
    fn a_budget_smaller_than_one_map_stops_rather_than_looping() {
        let model = SizeModel::default();
        // Below the model's fixed intercept: no split can ever fit.
        let p = plan(&recipe(1_000.0), &model, &predictors(1_000.0), 1_000, 4096);
        assert!(
            p.columns <= 32 && p.rows <= 32,
            "the search must be bounded"
        );
        assert!(p.is_split());
        // And it must say that the split does not actually solve the problem.
        assert!(!p.fits);
        assert!(p.reason.contains("cannot be built"), "{}", p.reason);
    }
}
