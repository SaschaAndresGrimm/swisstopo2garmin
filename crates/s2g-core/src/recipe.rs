//! Build recipes: the serializable description of one map (SPEC.md §11.2, FR-55).
//!
//! A recipe is the unit the GUI edits, saves and shares, and the key the build stages
//! are cached against. Everything needed to reproduce a map is here, so a build is
//! reproducible from its recipe plus the dataset releases recorded in its manifest.

use serde::{Deserialize, Serialize};

use crate::dem::Resolution;
use crate::proj::BBox;

/// Content preset (SPEC.md FR-50).
///
/// Presets are **not** device-locked: a fēnix is used for ski touring and an Edge for
/// bikepacking on hiking trails. The device selects the cartography variant; the preset
/// selects the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Hiking,
    Cycling,
    /// Ski touring: SAC routes split skiable / carrying / caution, snowshoe, winter hiking.
    Skimo,
    Full,
}

impl Preset {
    pub fn all() -> &'static [Preset] {
        &[Preset::Hiking, Preset::Cycling, Preset::Skimo, Preset::Full]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Preset::Hiking => "hiking",
            Preset::Cycling => "cycling",
            Preset::Skimo => "skimo",
            Preset::Full => "full",
        }
    }

    /// Winter route datasets, needed by skimo and full.
    pub fn needs_winter(&self) -> bool {
        matches!(self, Preset::Skimo | Preset::Full)
    }

    /// ASTRA route shapefiles, needed by cycling and full.
    pub fn needs_cycle(&self) -> bool {
        matches!(self, Preset::Cycling | Preset::Full)
    }

    /// Contour interval the preset defaults to.
    pub fn default_contour_m(&self) -> i32 {
        match self {
            Preset::Full => 10,
            _ => 20,
        }
    }

    pub fn default_index_contour_m(&self) -> i32 {
        match self {
            Preset::Full => 50,
            _ => 100,
        }
    }
}

/// Which area to build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AreaSelection {
    /// An explicit LV95 rectangle, as drawn on the map.
    ///
    /// The tag is spelled out because `rename_all = "camelCase"` turns `BBox` into
    /// `bBox`, which is neither what the frontend sends nor what SPEC §11.2 documents.
    /// The alias keeps recipes saved before this was noticed loadable.
    #[serde(rename = "bbox", alias = "bBox", rename_all = "camelCase")]
    BBox {
        min_e: f64,
        min_n: f64,
        max_e: f64,
        max_n: f64,
    },
    /// A named place plus a radius.
    ///
    /// Place names are not unique, so the resolved coordinate is stored alongside the
    /// name: re-resolving later could silently pick a different settlement.
    #[serde(rename_all = "camelCase")]
    Place {
        name: String,
        radius_km: f64,
        easting: f64,
        northing: f64,
    },

    /// One or more administrative units, optionally buffered (SPEC.md FR-33, FR-34).
    ///
    /// Stores the unit *numbers*, not their geometry: `bfs_nummer` and `kantonsnummer`
    /// are stable across releases while names are not, and a canton boundary is 14,000
    /// points that would bloat every saved recipe. The geometry is resolved from
    /// swissBOUNDARIES3D at build time. The extent is stored, though, so the area and
    /// size estimate need no file access while the wizard is open.
    #[serde(rename_all = "camelCase")]
    AdminUnits {
        level: crate::boundaries::AdminLevel,
        numbers: Vec<i64>,
        /// For display only; the numbers are the identity.
        names: Vec<String>,
        buffer_km: f64,
        min_e: f64,
        min_n: f64,
        max_e: f64,
        max_n: f64,
    },

    /// A freehand polygon drawn on the map (SPEC.md FR-31).
    #[serde(rename_all = "camelCase")]
    Polygon {
        /// LV95 `[easting, northing]`, in order. Closed implicitly.
        points: Vec<[f64; 2]>,
    },

    /// A circle around a point (SPEC.md FR-31).
    ///
    /// Distinct from `Place` even though both are a radius: a circle is somewhere the
    /// user pointed at, with no name to re-resolve and no settlement behind it.
    #[serde(rename_all = "camelCase")]
    Circle {
        easting: f64,
        northing: f64,
        radius_km: f64,
    },

    /// Several selections combined (SPEC.md FR-41).
    ///
    /// The union of its parts: a canton plus the valley next door, or two ends of a
    /// traverse. Nested composites are allowed but pointless, and flatten to the same
    /// thing.
    #[serde(rename_all = "camelCase")]
    Composite { parts: Vec<AreaSelection> },

    /// A corridor around an imported GPX or FIT track (SPEC.md FR-38, FR-39).
    ///
    /// Points are LV95 and already simplified: a corridor is kilometres wide, so metre
    /// detail in its centreline changes nothing and would bloat every saved recipe.
    #[serde(rename_all = "camelCase")]
    Corridor {
        name: String,
        buffer_km: f64,
        /// `[easting, northing]` pairs, in order.
        points: Vec<[f64; 2]>,
    },
}

/// A ring that closes, since a drawn polygon does not.
fn closed_ring(points: &[[f64; 2]]) -> Vec<crate::geom::Coord> {
    let mut ring: Vec<crate::geom::Coord> = points
        .iter()
        .map(|p| crate::geom::Coord::new(p[0], p[1]))
        .collect();
    if ring.first() != ring.last() {
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
    }
    ring
}

/// Bounding box of a point set, grown by `margin`.
fn bbox_of(points: &[[f64; 2]], margin: f64) -> BBox {
    let (mut e0, mut n0, mut e1, mut n1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in points {
        e0 = e0.min(p[0]);
        n0 = n0.min(p[1]);
        e1 = e1.max(p[0]);
        n1 = n1.max(p[1]);
    }
    if points.is_empty() {
        BBox::new(0.0, 0.0, 0.0, 0.0)
    } else {
        BBox::new(e0 - margin, n0 - margin, e1 + margin, n1 + margin)
    }
}

/// FNV-1a over coordinate bits. Not cryptographic; it separates two shapes.
fn digest_points(points: &[[f64; 2]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in points {
        for v in [p[0], p[1]] {
            for b in v.to_bits().to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x1000_0000_01b3);
            }
        }
    }
    h
}

impl AreaSelection {
    /// A stable tag for this kind of selection, for anything that needs to name it.
    ///
    /// Matches the serde tag, so the frontend can key translations off it without a
    /// second mapping to keep in step.
    pub fn kind(&self) -> &'static str {
        match self {
            AreaSelection::BBox { .. } => "bbox",
            AreaSelection::Place { .. } => "place",
            AreaSelection::Polygon { .. } => "polygon",
            AreaSelection::Circle { .. } => "circle",
            AreaSelection::Composite { .. } => "composite",
            AreaSelection::AdminUnits { .. } => "adminUnits",
            AreaSelection::Corridor { .. } => "corridor",
        }
    }

    /// The distinguishing part of this selection: its name, its size, its count.
    ///
    /// Names, numbers and units only, with no words to translate — so a caller can put
    /// it inside a localised sentence keyed on [`AreaSelection::kind`]. Extracted from
    /// `library::list`, which had this match written inline and was the only place that
    /// could describe a selection; the area step needed the same thing to say what is
    /// currently selected.
    pub fn detail(&self) -> String {
        let b = self.bbox();
        match self {
            AreaSelection::Place {
                name, radius_km, ..
            } => format!("{name} · {radius_km:.0} km"),
            AreaSelection::BBox { .. } => format!(
                "{:.0} × {:.0} km",
                (b.max_e - b.min_e) / 1000.0,
                (b.max_n - b.min_n) / 1000.0
            ),
            AreaSelection::Corridor {
                name, buffer_km, ..
            } => format!("{name} · ±{buffer_km:.1} km"),
            AreaSelection::Polygon { points } => {
                format!("{} · {:.0} km²", points.len(), b.area_km2())
            }
            AreaSelection::Circle { radius_km, .. } => format!("{radius_km:.0} km"),
            AreaSelection::Composite { parts } => {
                format!("{} · {:.0} km²", parts.len(), b.area_km2())
            }
            AreaSelection::AdminUnits {
                names, buffer_km, ..
            } => {
                let listed = match names.len() {
                    0 => "—".to_string(),
                    1..=2 => names.join(", "),
                    n => format!("{}, +{}", names[0], n - 1),
                };
                if *buffer_km > 0.0 {
                    format!("{listed} · +{buffer_km:.1} km")
                } else {
                    listed
                }
            }
        }
    }

    /// The shape features must intersect, when the selection is not a rectangle.
    ///
    /// Takes the cache root because an administrative selection stores unit numbers
    /// rather than geometry, and the geometry lives in swissBOUNDARIES3D.
    pub fn mask(
        &self,
        cache_root: &std::path::Path,
    ) -> crate::error::Result<Option<crate::mask::Mask>> {
        Ok(match self {
            AreaSelection::Corridor {
                buffer_km, points, ..
            } => Some(crate::mask::Mask::corridor(
                vec![points
                    .iter()
                    .map(|p| crate::geom::Coord::new(p[0], p[1]))
                    .collect()],
                buffer_km * 1000.0,
            )),
            AreaSelection::Polygon { points } => {
                Some(crate::mask::Mask::polygons(vec![vec![closed_ring(points)]]))
            }
            AreaSelection::Circle {
                easting,
                northing,
                radius_km,
            } => Some(crate::mask::Mask::corridor(
                vec![vec![crate::geom::Coord::new(*easting, *northing)]],
                radius_km * 1000.0,
            )),
            AreaSelection::Composite { parts } => {
                // A part with no mask covers its whole bounding box, and a union with
                // an unmasked part is that box: masking the rest would be a lie about
                // what the build contains.
                let mut masks = Vec::new();
                for part in parts {
                    match part.mask(cache_root)? {
                        Some(m) => masks.push(m),
                        None => return Ok(None),
                    }
                }
                Some(crate::mask::Mask::union(masks))
            }
            AreaSelection::AdminUnits {
                level,
                numbers,
                buffer_km,
                ..
            } => {
                let polys = crate::boundaries::load_geometry(cache_root, *level, numbers)?;
                Some(crate::mask::Mask::polygons_buffered(
                    polys,
                    buffer_km * 1000.0,
                ))
            }
            _ => None,
        })
    }

    pub fn bbox(&self) -> BBox {
        match self {
            AreaSelection::BBox {
                min_e,
                min_n,
                max_e,
                max_n,
            } => BBox::new(*min_e, *min_n, *max_e, *max_n),
            AreaSelection::Place {
                radius_km,
                easting,
                northing,
                ..
            } => BBox::from_center(*easting, *northing, radius_km * 1000.0),
            // The corridor's own extent already includes the buffer.
            AreaSelection::Corridor {
                points, buffer_km, ..
            } => {
                let m = buffer_km * 1000.0;
                let (mut e0, mut n0, mut e1, mut n1) = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for p in points {
                    e0 = e0.min(p[0]);
                    n0 = n0.min(p[1]);
                    e1 = e1.max(p[0]);
                    n1 = n1.max(p[1]);
                }
                if points.is_empty() {
                    BBox::new(0.0, 0.0, 0.0, 0.0)
                } else {
                    BBox::new(e0 - m, n0 - m, e1 + m, n1 + m)
                }
            }
            AreaSelection::Polygon { points } => bbox_of(points, 0.0),
            AreaSelection::Circle {
                easting,
                northing,
                radius_km,
            } => BBox::from_center(*easting, *northing, radius_km * 1000.0),
            AreaSelection::Composite { parts } => {
                let mut it = parts.iter().map(|p| p.bbox());
                match it.next() {
                    None => BBox::new(0.0, 0.0, 0.0, 0.0),
                    Some(first) => it.fold(first, |acc, b| {
                        BBox::new(
                            acc.min_e.min(b.min_e),
                            acc.min_n.min(b.min_n),
                            acc.max_e.max(b.max_e),
                            acc.max_n.max(b.max_n),
                        )
                    }),
                }
            }
            // Resolved when the units were chosen, so this needs no file access.
            AreaSelection::AdminUnits {
                min_e,
                min_n,
                max_e,
                max_n,
                ..
            } => BBox::new(*min_e, *min_n, *max_e, *max_n),
        }
    }

    /// A short stable digest of the selection's shape, for the cache key.
    ///
    /// Two different tracks can share a bounding box, so the bbox alone would let one
    /// corridor build be served from another's cached stages.
    pub fn shape_digest(&self) -> u64 {
        match self {
            AreaSelection::AdminUnits {
                level,
                numbers,
                buffer_km,
                ..
            } => {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for b in level.id().bytes().chain(
                    numbers
                        .iter()
                        .flat_map(|n| n.to_le_bytes())
                        .chain(buffer_km.to_bits().to_le_bytes()),
                ) {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x1000_0000_01b3);
                }
                h
            }
            AreaSelection::Polygon { points } => digest_points(points),
            AreaSelection::Circle {
                easting,
                northing,
                radius_km,
            } => digest_points(&[[*easting, *northing], [*radius_km, 0.0]]),
            AreaSelection::Composite { parts } => {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for p in parts {
                    for b in p.shape_digest().to_le_bytes() {
                        h ^= b as u64;
                        h = h.wrapping_mul(0x1000_0000_01b3);
                    }
                    // The bounding box too, so two parts differing only in extent are
                    // not treated as one.
                    let bb = p.bbox();
                    for v in [bb.min_e, bb.min_n, bb.max_e, bb.max_n] {
                        for b in v.to_bits().to_le_bytes() {
                            h ^= b as u64;
                            h = h.wrapping_mul(0x1000_0000_01b3);
                        }
                    }
                }
                h
            }
            AreaSelection::Corridor { points, .. } => {
                // FNV-1a over the coordinate bits. Not cryptographic; it only has to
                // separate two tracks a user might build on the same day.
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for p in points {
                    for v in [p[0], p[1]] {
                        for b in v.to_bits().to_le_bytes() {
                            h ^= b as u64;
                            h = h.wrapping_mul(0x1000_0000_01b3);
                        }
                    }
                }
                h
            }
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContourSettings {
    pub interval_m: i32,
    pub index_m: i32,
    pub simplify_m: f64,
}

impl Default for ContourSettings {
    fn default() -> Self {
        Self {
            interval_m: 20,
            index_m: 100,
            simplify_m: 8.0,
        }
    }
}

/// Which measured colour scheme the map is printed in (SPEC.md FR-CART11).
///
/// swisstopo publishes a Winter national map alongside the summer one, and its
/// recolouring is measured rather than invented (see `tools/winter_palette.py`). It
/// exists so ski and snowshoe routes read against a base map that has stepped back;
/// it is a cartography choice, not a content one, so it is independent of the preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    #[default]
    Summer,
    Winter,
}

impl Palette {
    pub fn id(&self) -> &'static str {
        match self {
            Palette::Summer => "summer",
            Palette::Winter => "winter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReliefDetail {
    /// No DEM: the device cannot render shaded relief.
    #[default]
    Off,
    /// 3 arc-second, gentler. Hardware showed 1 arc-second is very dark in the Alps.
    Gentle,
    /// 1 arc-second.
    Detailed,
}

impl ReliefDetail {
    pub fn resolution(&self) -> Option<Resolution> {
        match self {
            ReliefDetail::Off => None,
            ReliefDetail::Gentle => Some(Resolution::ArcSecond3),
            ReliefDetail::Detailed => Some(Resolution::ArcSecond1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub schema_version: u32,
    pub name: String,
    /// Device profile id; selects the cartography variant.
    pub device_id: String,
    pub area: AreaSelection,
    pub preset: Preset,
    pub contours: ContourSettings,
    pub relief: ReliefDetail,
    /// Colour scheme. Defaults to summer, and defaults rather than being required so
    /// recipes saved before winter existed still load.
    #[serde(default)]
    pub palette: Palette,
    /// Which language to label in (SPEC.md FR-53).
    ///
    /// Defaults to the local name, which is the correct default for Switzerland and the
    /// only option needing no extra dataset.
    #[serde(default)]
    pub label_language: crate::names::LabelLanguage,
    /// Draw slope classes over 30° (SPEC.md FR-CART12).
    ///
    /// Off by default: the classes cover whole mountainsides, and on a summer hiking
    /// map they are noise. Defaults rather than being required, so older recipes load.
    #[serde(default)]
    pub slope_classes: bool,
    /// Build a routable map: a road graph the device can navigate along (SPEC.md §16).
    ///
    /// Costs size — the NET and NOD subfiles are a substantial share of the output —
    /// and needs the device to support routable maps, which the profile records.
    ///
    /// **Off by default, and deliberately so.** Nothing about routing has been checked
    /// on hardware: not the road classes, not the access rules, and not the assumption
    /// that a directionally-separated carriageway is digitised the way traffic moves.
    /// That last one is the reason for caution rather than optimism — if it is wrong,
    /// a device sends a cyclist the wrong way down a dual carriageway. It is *verified
    /// against the data* (docs/routing.md) and unverified on a device, and those are
    /// different things.
    #[serde(default)]
    pub routing: bool,
    /// Include official building addresses, so the device can search for one
    /// (SPEC.md §16 v2).
    ///
    /// Needs `ch.swisstopo.amtliches-gebaeudeadressverzeichnis` downloaded — swissTLM3D
    /// has street names and no house numbers. Off by default: it is a separate 137 MB
    /// download and it adds a searchable point per building, which is a great many
    /// points in a town.
    #[serde(default)]
    pub addresses: bool,
    /// Also build a raster overlay of the swisstopo paper map (SPEC.md §8.5, FR-R1).
    ///
    /// A separate KMZ file rather than part of the `.img`, installed to
    /// `Garmin/CustomMaps`. Off by default, and it does not affect the vector map at
    /// all — so like `routing` and `addresses` it stays out of [`Recipe::cache_key`]
    /// and out of the region key: turning it on must not invalidate a cached clip.
    ///
    /// Needs a device profile that states its Custom Map limits
    /// ([`crate::devices::DeviceProfile::raster_limits`]). Unverified on hardware.
    #[serde(default)]
    pub raster: bool,
    /// Layer ids explicitly switched off in the layer panel (FR-51).
    #[serde(default)]
    pub excluded_layers: Vec<String>,
}

impl Recipe {
    pub fn new(name: impl Into<String>, device_id: impl Into<String>, area: AreaSelection) -> Self {
        Self {
            schema_version: 1,
            name: name.into(),
            device_id: device_id.into(),
            area,
            preset: Preset::Hiking,
            contours: ContourSettings::default(),
            relief: ReliefDetail::Gentle,
            palette: Palette::Summer,
            slope_classes: false,
            routing: false,
            addresses: false,
            raster: false,
            label_language: crate::names::LabelLanguage::Local,
            excluded_layers: Vec::new(),
        }
    }

    pub fn with_preset(mut self, preset: Preset) -> Self {
        self.contours.interval_m = preset.default_contour_m();
        self.contours.index_m = preset.default_index_contour_m();
        self.preset = preset;
        self
    }

    /// Stable key for map identity and stage caching.
    ///
    /// Deliberately excludes `name`: renaming a map must not change its identity on the
    /// device or invalidate cached stages.
    pub fn cache_key(&self) -> String {
        let b = self.area.bbox();
        format!(
            "{}|{:.0},{:.0},{:.0},{:.0}|{:x}|{}|{}|{}|{:?}|{}|{}|{}|{}",
            self.device_id,
            b.min_e,
            b.min_n,
            b.max_e,
            b.max_n,
            self.area.shape_digest(),
            self.preset.id(),
            self.contours.interval_m,
            self.contours.index_m,
            self.relief,
            self.palette.id(),
            self.slope_classes,
            self.label_language.id(),
            self.excluded_layers.join(",")
        )
    }
}
