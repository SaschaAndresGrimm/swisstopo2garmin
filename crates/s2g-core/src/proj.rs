//! CH1903+/LV95 (EPSG:2056) <-> WGS84 (EPSG:4326).
//!
//! Uses swisstopo's published approximate formulas. Their stated accuracy is about
//! 1 m, which is below the ~2.4 m coordinate resolution of the Garmin IMG format
//! (24-bit coordinates over 360 degrees), so the error is not representable in the
//! output at all (SPEC.md FR-P1).
//!
//! A rigorous Hotine Oblique Mercator inverse plus the CHENyx06 grid shift would be
//! needed for survey work. It is not needed here, and would pull in a projection
//! library and its grid files for no visible benefit.

/// Garmin stores coordinates as 24-bit units of 360/2^24 degrees.
pub const GARMIN_DEGREES_PER_UNIT: f64 = 360.0 / (1u32 << 24) as f64;

/// LV95 easting/northing of the projection origin (Bern).
pub const LV95_ORIGIN: (f64, f64) = (2_600_000.0, 1_200_000.0);

/// Approximate bounds of the swissTLM3D coverage area in LV95, used to sanity-check
/// input before it reaches a database query.
pub const LV95_BOUNDS: (f64, f64, f64, f64) = (2_480_000.0, 1_070_000.0, 2_840_000.0, 1_300_000.0);

/// LV95 -> WGS84. Returns `(lon, lat)` in degrees.
pub fn lv95_to_wgs84(e: f64, n: f64) -> (f64, f64) {
    let y = (e - LV95_ORIGIN.0) / 1_000_000.0;
    let x = (n - LV95_ORIGIN.1) / 1_000_000.0;

    let lon =
        2.677_909_4 + 4.728_982 * y + 0.791_484 * y * x + 0.130_6 * y * x * x - 0.043_6 * y * y * y;
    let lat = 16.902_389_2 + 3.238_272 * x
        - 0.270_978 * y * y
        - 0.002_528 * x * x
        - 0.044_7 * y * y * x
        - 0.014_0 * x * x * x;

    (lon * 100.0 / 36.0, lat * 100.0 / 36.0)
}

/// WGS84 -> LV95. Takes `(lon, lat)` in degrees, returns `(easting, northing)`.
pub fn wgs84_to_lv95(lon: f64, lat: f64) -> (f64, f64) {
    // swisstopo's formulas work in units of 10000 seconds of arc from the origin.
    let phi = (lat * 3600.0 - 169_028.66) / 10_000.0;
    let lam = (lon * 3600.0 - 26_782.5) / 10_000.0;

    let e = 2_600_072.37 + 211_455.93 * lam
        - 10_938.51 * lam * phi
        - 0.36 * lam * phi * phi
        - 44.54 * lam * lam * lam;
    let n = 1_200_147.07 + 308_807.95 * phi + 3.7452_5e3 * lam * lam + 76.63 * phi * phi
        - 194.56 * lam * lam * phi
        + 119.79 * phi * phi * phi;

    (e, n)
}

/// Axis-aligned bounding box in LV95.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub min_e: f64,
    pub min_n: f64,
    pub max_e: f64,
    pub max_n: f64,
}

impl BBox {
    pub fn new(min_e: f64, min_n: f64, max_e: f64, max_n: f64) -> Self {
        Self {
            min_e: min_e.min(max_e),
            min_n: min_n.min(max_n),
            max_e: min_e.max(max_e),
            max_n: min_n.max(max_n),
        }
    }

    pub fn from_center(e: f64, n: f64, radius_m: f64) -> Self {
        Self::new(e - radius_m, n - radius_m, e + radius_m, n + radius_m)
    }

    pub fn width(&self) -> f64 {
        self.max_e - self.min_e
    }
    pub fn height(&self) -> f64 {
        self.max_n - self.min_n
    }
    pub fn area_km2(&self) -> f64 {
        self.width() * self.height() / 1e6
    }

    pub fn contains(&self, e: f64, n: f64) -> bool {
        e >= self.min_e && e <= self.max_e && n >= self.min_n && n <= self.max_n
    }

    pub fn intersects(&self, other: &BBox) -> bool {
        self.min_e <= other.max_e
            && self.max_e >= other.min_e
            && self.min_n <= other.max_n
            && self.max_n >= other.min_n
    }

    pub fn expand(&self, m: f64) -> Self {
        Self::new(
            self.min_e - m,
            self.min_n - m,
            self.max_e + m,
            self.max_n + m,
        )
    }

    pub fn union(&self, other: &BBox) -> Self {
        Self::new(
            self.min_e.min(other.min_e),
            self.min_n.min(other.min_n),
            self.max_e.max(other.max_e),
            self.max_n.max(other.max_n),
        )
    }

    /// WGS84 bounding box `[west, south, east, north]`, as STAC expects.
    ///
    /// All four corners are projected, not just two: the LV95 grid is rotated
    /// relative to the graticule, so a corner-pair conversion would clip content.
    pub fn to_wgs84(&self) -> [f64; 4] {
        let corners = [
            lv95_to_wgs84(self.min_e, self.min_n),
            lv95_to_wgs84(self.min_e, self.max_n),
            lv95_to_wgs84(self.max_e, self.min_n),
            lv95_to_wgs84(self.max_e, self.max_n),
        ];
        let lons = corners.map(|c| c.0);
        let lats = corners.map(|c| c.1);
        [
            lons.iter().cloned().fold(f64::INFINITY, f64::min),
            lats.iter().cloned().fold(f64::INFINITY, f64::min),
            lons.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            lats.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ]
    }

    /// True if the box overlaps the swissTLM3D coverage area at all.
    pub fn within_switzerland(&self) -> bool {
        let ch = BBox::new(LV95_BOUNDS.0, LV95_BOUNDS.1, LV95_BOUNDS.2, LV95_BOUNDS.3);
        self.intersects(&ch)
    }
}
