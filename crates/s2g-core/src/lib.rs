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

pub mod cache;
pub mod contour;
pub mod dem;
pub mod devices;
pub mod download;
pub mod elevation;
pub mod error;
pub mod estimate;
pub mod extract;
pub mod garmin;
pub mod geom;
pub mod gpkg;
pub mod http;
pub mod img;
pub mod library;
pub mod pbf;
pub mod pipeline;
pub mod proj;
pub mod recipe;
pub mod shapefile;
pub mod stac;
pub mod zip;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::{Error, Result};
