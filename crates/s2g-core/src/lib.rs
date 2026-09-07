//! swisstopo2garmin core: data acquisition and the map build pipeline.
//!
//! The GUI (`src-tauri`) is a thin shell over this crate, so everything here is
//! testable with plain `cargo test` (PLAN.md working agreement, rule 5).
//!
//! Design rules that are load-bearing here:
//! - **Stream, never slurp** (SPEC.md NFR-2). The national swissTLM3D GeoPackage is
//!   10.78 GB uncompressed; nothing may hold a dataset in memory.
//! - Downloads are resumable, verified against the STAC `file:checksum` multihash, and
//!   only become visible in the cache after an atomic rename (FR-D1..D4).

pub mod addresses;
pub mod area_edit;
pub mod boundaries;
pub mod cache;
pub mod clock;
pub mod contour;
pub mod datasets;
pub mod dem;
pub mod devices;
pub mod diagnose;
pub mod download;
pub mod elevation;
pub mod error;
pub mod estimate;
pub mod extract;
pub mod fit;
pub mod garmin;
pub mod geojson;
pub mod geom;
pub mod gpkg;
pub mod gpx;
pub mod http;
pub mod img;
pub mod install;
pub mod library;
pub mod manifest;
pub mod mask;
pub mod names;
pub mod partition;
pub mod pbf;
pub mod perf;
pub mod pipeline;
pub mod proj;
pub mod recipe;
pub mod recovery;
pub mod settings;
pub mod shapefile;
pub mod slope;
pub mod stac;
pub mod stage_cache;
pub mod zip;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::{Error, Result};
