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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl AreaSelection {
    /// The shape features must intersect, when the selection is not a rectangle.
    pub fn mask(&self) -> Option<crate::mask::Mask> {
        match self {
            AreaSelection::Corridor {
                buffer_km, points, ..
            } => Some(crate::mask::Mask::corridor(
                vec![points
                    .iter()
                    .map(|p| crate::geom::Coord::new(p[0], p[1]))
                    .collect()],
                buffer_km * 1000.0,
            )),
            _ => None,
        }
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
            AreaSelection::Corridor { .. } => self
                .mask()
                .map(|m| m.bbox())
                .unwrap_or_else(|| BBox::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    /// A short stable digest of the selection's shape, for the cache key.
    ///
    /// Two different tracks can share a bounding box, so the bbox alone would let one
    /// corridor build be served from another's cached stages.
    pub fn shape_digest(&self) -> u64 {
        match self {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            "{}|{:.0},{:.0},{:.0},{:.0}|{:x}|{}|{}|{}|{:?}|{}",
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
            self.excluded_layers.join(",")
        )
    }
}
