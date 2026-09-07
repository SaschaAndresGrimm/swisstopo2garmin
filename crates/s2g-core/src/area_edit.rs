//! Making a drawn area editable (SPEC.md FR-31).
//!
//! Drawing a shape and then having to start over to nudge one corner is the wrong
//! interaction, and it was worse than that: the map drew every selection as its bounding
//! *rectangle*, so a polygon or a circle vanished the moment it was finished and was
//! replaced by a box that was not what the user drew.
//!
//! All of the geometry lives here rather than in the map component, for two reasons.
//! The authoritative coordinates are LV95 — the recipe's, and the ones a build reads —
//! so editing in the map's WGS84 and converting back would make the projection round
//! trip on every drag. And a corner-ordering mistake is invisible in a screenshot and
//! obvious in a test.
//!
//! The map's job is reduced to: ask for the [`handles`], notice which one the pointer
//! grabbed, and send back one [`AreaEdit`].

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::proj::lv95_to_wgs84;
use crate::recipe::AreaSelection;

/// How many segments a circle is drawn with. Enough that the outline reads as a circle
/// at any zoom the area step offers.
const CIRCLE_STEPS: usize = 64;

/// The smallest area the editor will produce, in metres on a side.
///
/// A rectangle dragged inside-out or collapsed onto a line would build nothing, and
/// "nothing" is a far more confusing result than a refused drag.
const MIN_SPAN_M: f64 = 200.0;

/// What a draggable point on the map does when it is dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HandleRole {
    /// Moves one corner or vertex, leaving the rest where they are.
    Vertex,
    /// Sits between two vertices; dragging it creates a new vertex there.
    Midpoint,
    /// Moves the whole shape.
    Centre,
    /// Changes a circle's radius.
    Radius,
}

/// A point the user can grab, in WGS84 because that is what the map draws in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handle {
    pub lon: f64,
    pub lat: f64,
    pub role: HandleRole,
    /// Which vertex this is, or which vertex a midpoint follows. Zero for centre and
    /// radius handles, which there is only ever one of.
    pub index: usize,
}

/// Everything the map needs to draw a selection and let it be edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outline {
    /// One closed ring per part, WGS84 `[lon, lat]`. A composite has several.
    pub rings: Vec<Vec<[f64; 2]>>,
    pub handles: Vec<Handle>,
    /// False when this selection cannot be edited by dragging, so the UI can say why
    /// instead of ignoring the drag.
    pub editable: bool,
    /// Present when `editable` is false: what to do instead.
    pub not_editable_because: Option<String>,
    /// Serde tag of the selection's kind, so the UI can name it in the user's language.
    pub kind: String,
    /// The distinguishing part: name, size or count. Numbers and names only, no words.
    pub detail: String,
}

/// One change to a selection, expressed in LV95 because that is what the recipe holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AreaEdit {
    /// Put vertex `index` at this position.
    #[serde(rename_all = "camelCase")]
    MoveVertex {
        index: usize,
        easting: f64,
        northing: f64,
    },
    /// Insert a new vertex after `after`, which is what dragging a midpoint does.
    #[serde(rename_all = "camelCase")]
    InsertVertex {
        after: usize,
        easting: f64,
        northing: f64,
    },
    /// Drop vertex `index`. Refused when it would leave fewer than three.
    #[serde(rename_all = "camelCase")]
    RemoveVertex { index: usize },
    /// Shift the whole selection.
    #[serde(rename_all = "camelCase")]
    Translate { d_easting: f64, d_northing: f64 },
    /// Set a circle's radius.
    #[serde(rename_all = "camelCase")]
    SetRadiusKm { radius_km: f64 },
}

/// The rings and handles for a selection.
pub fn outline(area: &AreaSelection) -> Outline {
    let mut rings = Vec::new();
    push_rings(area, &mut rings);
    let refusal = why_not_editable(area);
    Outline {
        rings,
        handles: if refusal.is_none() {
            handles(area)
        } else {
            Vec::new()
        },
        editable: refusal.is_none(),
        not_editable_because: refusal,
        // Carried here rather than fetched separately: the area step already asks for an
        // outline on every selection change, and it needs to say what is selected in the
        // same breath.
        kind: area.kind().to_string(),
        detail: area.detail(),
    }
}

/// Why this selection cannot be dragged, or `None` when it can.
///
/// The three that cannot are not arbitrary: their shape is *derived*, from a file or
/// from a track, so moving a point of it would either be a lie about what will be built
/// or would silently convert the selection into something else.
fn why_not_editable(area: &AreaSelection) -> Option<String> {
    match area {
        AreaSelection::BBox { .. }
        | AreaSelection::Polygon { .. }
        | AreaSelection::Circle { .. }
        | AreaSelection::Place { .. } => None,
        AreaSelection::AdminUnits { .. } => Some(
            "An administrative unit's boundary comes from swissBOUNDARIES3D. Change the \
             selection or the buffer instead."
                .into(),
        ),
        AreaSelection::Corridor { .. } => Some(
            "A corridor follows the imported track. Change the buffer width, or import \
             a different track."
                .into(),
        ),
        AreaSelection::Composite { .. } => Some(
            "A combined selection is edited one part at a time. Remove a part and draw \
             it again."
                .into(),
        ),
    }
}

/// The draggable points, in the order the map should draw them.
///
/// Rectangle corners are south-west, south-east, north-east, north-west — anticlockwise
/// from the bottom left, which is the order [`AreaEdit::MoveVertex`] indexes and the
/// order [`opposite_corner`] is written against. Getting this wrong makes a corner drag
/// resize from the wrong anchor, which looks like the map fighting the user.
pub fn handles(area: &AreaSelection) -> Vec<Handle> {
    let vertex = |i: usize, e: f64, n: f64| {
        let (lon, lat) = lv95_to_wgs84(e, n);
        Handle {
            lon,
            lat,
            role: HandleRole::Vertex,
            index: i,
        }
    };
    match area {
        AreaSelection::BBox {
            min_e,
            min_n,
            max_e,
            max_n,
        } => {
            let mut out = vec![
                vertex(0, *min_e, *min_n),
                vertex(1, *max_e, *min_n),
                vertex(2, *max_e, *max_n),
                vertex(3, *min_e, *max_n),
            ];
            out.push(centre_handle((min_e + max_e) / 2.0, (min_n + max_n) / 2.0));
            out
        }
        AreaSelection::Polygon { points } => {
            let mut out: Vec<Handle> = points
                .iter()
                .enumerate()
                .map(|(i, p)| vertex(i, p[0], p[1]))
                .collect();
            // A midpoint after every vertex, wrapping, so any edge can be subdivided --
            // including the closing one, which is the edge people most often want to
            // pull out and the one a non-wrapping loop forgets.
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                let (lon, lat) = lv95_to_wgs84((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
                out.push(Handle {
                    lon,
                    lat,
                    role: HandleRole::Midpoint,
                    index: i,
                });
            }
            if let Some((e, n)) = centroid(points) {
                out.push(centre_handle(e, n));
            }
            out
        }
        AreaSelection::Circle {
            easting,
            northing,
            radius_km,
        }
        | AreaSelection::Place {
            easting,
            northing,
            radius_km,
            ..
        } => {
            let (lon, lat) = lv95_to_wgs84(easting + radius_km * 1000.0, *northing);
            vec![
                centre_handle(*easting, *northing),
                Handle {
                    lon,
                    lat,
                    role: HandleRole::Radius,
                    index: 0,
                },
            ]
        }
        _ => Vec::new(),
    }
}

fn centre_handle(e: f64, n: f64) -> Handle {
    let (lon, lat) = lv95_to_wgs84(e, n);
    Handle {
        lon,
        lat,
        role: HandleRole::Centre,
        index: 0,
    }
}

/// Apply one edit, or say why it cannot be applied.
pub fn apply(area: &AreaSelection, edit: AreaEdit) -> Result<AreaSelection> {
    if let Some(reason) = why_not_editable(area) {
        return Err(Error::NotFound(reason));
    }
    match (area, edit) {
        // ---- rectangle -------------------------------------------------------
        (
            AreaSelection::BBox {
                min_e,
                min_n,
                max_e,
                max_n,
            },
            AreaEdit::MoveVertex {
                index,
                easting,
                northing,
            },
        ) => {
            let (ae, an) = opposite_corner(index, *min_e, *min_n, *max_e, *max_n)?;
            rect(ae, an, easting, northing)
        }
        (
            AreaSelection::BBox {
                min_e,
                min_n,
                max_e,
                max_n,
            },
            AreaEdit::Translate {
                d_easting,
                d_northing,
            },
        ) => Ok(AreaSelection::BBox {
            min_e: min_e + d_easting,
            min_n: min_n + d_northing,
            max_e: max_e + d_easting,
            max_n: max_n + d_northing,
        }),

        // ---- polygon ---------------------------------------------------------
        (
            AreaSelection::Polygon { points },
            AreaEdit::MoveVertex {
                index,
                easting,
                northing,
            },
        ) => {
            let mut p = points.clone();
            *p.get_mut(index)
                .ok_or_else(|| Error::NotFound(format!("no vertex {index}")))? =
                [easting, northing];
            Ok(AreaSelection::Polygon { points: p })
        }
        (
            AreaSelection::Polygon { points },
            AreaEdit::InsertVertex {
                after,
                easting,
                northing,
            },
        ) => {
            if after >= points.len() {
                return Err(Error::NotFound(format!("no vertex {after}")));
            }
            let mut p = points.clone();
            p.insert(after + 1, [easting, northing]);
            Ok(AreaSelection::Polygon { points: p })
        }
        (AreaSelection::Polygon { points }, AreaEdit::RemoveVertex { index }) => {
            if points.len() <= 3 {
                return Err(Error::NotFound(
                    "a polygon needs at least three corners; move this one instead of \
                     removing it"
                        .into(),
                ));
            }
            if index >= points.len() {
                return Err(Error::NotFound(format!("no vertex {index}")));
            }
            let mut p = points.clone();
            p.remove(index);
            Ok(AreaSelection::Polygon { points: p })
        }
        (
            AreaSelection::Polygon { points },
            AreaEdit::Translate {
                d_easting,
                d_northing,
            },
        ) => Ok(AreaSelection::Polygon {
            points: points
                .iter()
                .map(|p| [p[0] + d_easting, p[1] + d_northing])
                .collect(),
        }),

        // ---- circle, and a named place, which becomes one ---------------------
        (
            AreaSelection::Circle {
                easting,
                northing,
                radius_km,
            },
            AreaEdit::Translate {
                d_easting,
                d_northing,
            },
        ) => Ok(AreaSelection::Circle {
            easting: easting + d_easting,
            northing: northing + d_northing,
            radius_km: *radius_km,
        }),
        (
            AreaSelection::Circle {
                easting, northing, ..
            },
            AreaEdit::SetRadiusKm { radius_km },
        ) => circle(*easting, *northing, radius_km),

        // A named place that is moved or resized is no longer that place. Turning it
        // into a circle is the honest outcome: keeping the name would leave the recipe
        // claiming an area centred somewhere it is not, and refusing the drag would
        // make the handles a lie.
        (
            AreaSelection::Place {
                easting,
                northing,
                radius_km,
                ..
            },
            AreaEdit::Translate {
                d_easting,
                d_northing,
            },
        ) => Ok(AreaSelection::Circle {
            easting: easting + d_easting,
            northing: northing + d_northing,
            radius_km: *radius_km,
        }),
        (
            AreaSelection::Place {
                easting,
                northing,
                radius_km,
                ..
            },
            AreaEdit::SetRadiusKm { radius_km: r },
        ) => {
            // A radius change alone keeps the centre, so the place is still the place --
            // but the radius is the user's now, not the one the place step offered.
            let _ = radius_km;
            circle(*easting, *northing, r)
        }

        (a, e) => Err(Error::NotFound(format!(
            "{} cannot be edited that way ({e:?})",
            kind_of(a)
        ))),
    }
}

/// The corner that stays put when corner `index` is dragged.
///
/// Indices follow [`handles`]: 0 south-west, 1 south-east, 2 north-east, 3 north-west.
fn opposite_corner(
    index: usize,
    min_e: f64,
    min_n: f64,
    max_e: f64,
    max_n: f64,
) -> Result<(f64, f64)> {
    Ok(match index {
        0 => (max_e, max_n),
        1 => (min_e, max_n),
        2 => (min_e, min_n),
        3 => (max_e, min_n),
        _ => {
            return Err(Error::NotFound(format!(
                "a rectangle has four corners, not a corner {index}"
            )))
        }
    })
}

/// A rectangle from two opposite corners, normalised and checked.
fn rect(ae: f64, an: f64, be: f64, bn: f64) -> Result<AreaSelection> {
    let (min_e, max_e) = (ae.min(be), ae.max(be));
    let (min_n, max_n) = (an.min(bn), an.max(bn));
    // Normalising means a corner dragged past its opposite flips the rectangle rather
    // than inverting it, which is what every drawing program does and what a user
    // dragging quickly across the shape expects.
    if max_e - min_e < MIN_SPAN_M || max_n - min_n < MIN_SPAN_M {
        return Err(Error::NotFound(format!(
            "that would leave an area under {MIN_SPAN_M:.0} m across, which would build \
             nothing"
        )));
    }
    Ok(AreaSelection::BBox {
        min_e,
        min_n,
        max_e,
        max_n,
    })
}

fn circle(easting: f64, northing: f64, radius_km: f64) -> Result<AreaSelection> {
    if radius_km * 1000.0 < MIN_SPAN_M / 2.0 {
        return Err(Error::NotFound(format!(
            "that radius is under {:.0} m, which would build nothing",
            MIN_SPAN_M / 2.0
        )));
    }
    Ok(AreaSelection::Circle {
        easting,
        northing,
        radius_km,
    })
}

fn kind_of(area: &AreaSelection) -> &'static str {
    match area {
        AreaSelection::BBox { .. } => "a rectangle",
        AreaSelection::Place { .. } => "a place",
        AreaSelection::Polygon { .. } => "a polygon",
        AreaSelection::Circle { .. } => "a circle",
        AreaSelection::Composite { .. } => "a combined selection",
        AreaSelection::AdminUnits { .. } => "an administrative selection",
        AreaSelection::Corridor { .. } => "a corridor",
    }
}

/// Area-weighted centroid of a ring, for the move handle.
///
/// The vertex mean would sit wherever the vertices are densest, which on a polygon
/// traced along one detailed edge puts the move handle off the shape entirely.
fn centroid(points: &[[f64; 2]]) -> Option<(f64, f64)> {
    if points.len() < 3 {
        return None;
    }
    let (mut a2, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..points.len() {
        let p = points[i];
        let q = points[(i + 1) % points.len()];
        let cross = p[0] * q[1] - q[0] * p[1];
        a2 += cross;
        cx += (p[0] + q[0]) * cross;
        cy += (p[1] + q[1]) * cross;
    }
    if a2.abs() < f64::EPSILON {
        // Degenerate: fall back to the vertex mean rather than dividing by zero.
        let n = points.len() as f64;
        return Some((
            points.iter().map(|p| p[0]).sum::<f64>() / n,
            points.iter().map(|p| p[1]).sum::<f64>() / n,
        ));
    }
    Some((cx / (3.0 * a2), cy / (3.0 * a2)))
}

fn push_rings(area: &AreaSelection, out: &mut Vec<Vec<[f64; 2]>>) {
    let to_wgs = |pts: &[[f64; 2]]| -> Vec<[f64; 2]> {
        let mut ring: Vec<[f64; 2]> = pts
            .iter()
            .map(|p| {
                let (lon, lat) = lv95_to_wgs84(p[0], p[1]);
                [lon, lat]
            })
            .collect();
        if ring.first() != ring.last() {
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
        }
        ring
    };
    match area {
        AreaSelection::Polygon { points } => out.push(to_wgs(points)),
        AreaSelection::Circle {
            easting,
            northing,
            radius_km,
        }
        | AreaSelection::Place {
            easting,
            northing,
            radius_km,
            ..
        } => out.push(to_wgs(&circle_points(*easting, *northing, *radius_km))),
        AreaSelection::Composite { parts } => {
            for p in parts {
                push_rings(p, out);
            }
        }
        // A rectangle is its own outline; a corridor and an administrative selection
        // are drawn as their extent here, because their real shape is a mask and their
        // centreline or boundary is already drawn on the map as a line.
        other => {
            let b = other.bbox();
            out.push(to_wgs(&[
                [b.min_e, b.min_n],
                [b.max_e, b.min_n],
                [b.max_e, b.max_n],
                [b.min_e, b.max_n],
            ]));
        }
    }
}

fn circle_points(easting: f64, northing: f64, radius_km: f64) -> Vec<[f64; 2]> {
    let r = radius_km * 1000.0;
    (0..CIRCLE_STEPS)
        .map(|i| {
            let a = i as f64 / CIRCLE_STEPS as f64 * std::f64::consts::TAU;
            [easting + r * a.cos(), northing + r * a.sin()]
        })
        .collect()
}
