//! swisstopo STAC client (SPEC.md §3, FR-D1/FR-D4).
//!
//! Release identifiers are always resolved from the API — never hard-coded — because
//! swisstopo publishes a new swissTLM3D each year and the collection layout is theirs
//! to change (SPEC.md risk table).

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::error::{Error, Result};
use crate::http::Http;

pub const ROOT: &str = "https://data.geo.admin.ch/api/stac/v1";

pub const TLM3D: &str = "ch.swisstopo.swisstlm3d";
pub const ALTI3D: &str = "ch.swisstopo.swissalti3d";
pub const ALTIREGIO: &str = "ch.swisstopo.swissaltiregio";
pub const WANDERWEGE: &str = "ch.swisstopo.swisstlm3d-wanderwege";
pub const TLMREGIO: &str = "ch.swisstopo.swisstlmregio";

/// Administrative boundaries: cantons, districts and communes (SPEC.md FR-33).
pub const BOUNDARIES: &str = "ch.swisstopo.swissboundaries3d";

// Winter sport routes, for the skimo content preset. All three are GeoPackage, so the
// existing reader handles them with no new parser.
pub const SKITOUREN: &str = "ch.swisstopo-karto.skitouren";
pub const SCHNEESCHUH: &str = "ch.astra.schneeschuhwanderwege";
pub const WINTERWANDERN: &str = "ch.astra.winterwanderwege";

/// Mountain huts and winter accommodation, with contact details (SPEC.md FR-50).
pub const UNTERKUENFTE: &str = "ch.swisstopo.unterkuenfte-winter";

/// Cycle and hiking route networks.
///
/// **Shapefile and File Geodatabase only** — no GeoPackage is published, so these need
/// a shapefile reader before they can be used (see PLAN.md).
pub const VELOLAND: &str = "ch.astra.veloland";
pub const MOUNTAINBIKELAND: &str = "ch.astra.mountainbikeland";
pub const WANDERLAND: &str = "ch.astra.wanderland";

/// Multihash codes we accept. swisstopo currently emits sha2-256 (0x12).
const MULTIHASH_SHA2_256: u8 = 0x12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Digest {
    pub algo: String,
    /// Lower-case hex of the raw digest.
    pub hex: String,
}

impl Digest {
    /// Decode a STAC `file:checksum` multihash: `<code><len><digest>` in hex.
    pub fn from_multihash(s: &str) -> Result<Self> {
        let raw =
            hex::decode(s.trim()).map_err(|e| Error::MalformedMultihash(format!("{s}: {e}")))?;
        if raw.len() < 2 {
            return Err(Error::MalformedMultihash(format!("too short: {s}")));
        }
        let (code, len) = (raw[0], raw[1] as usize);
        if code != MULTIHASH_SHA2_256 {
            return Err(Error::UnsupportedMultihash { code });
        }
        if raw.len() != 2 + len {
            return Err(Error::MalformedMultihash(format!(
                "declares {len} bytes but carries {}",
                raw.len() - 2
            )));
        }
        Ok(Digest {
            algo: "sha2-256".into(),
            hex: hex::encode(&raw[2..]),
        })
    }

    pub fn hasher(&self) -> Sha256 {
        Sha256::new()
    }

    pub fn verify(&self, hasher: Sha256) -> Result<()> {
        let actual = hex::encode(hasher.finalize());
        if actual != self.hex {
            return Err(Error::ChecksumMismatch {
                algo: self.algo.clone(),
                expected: self.hex.clone(),
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub href: String,
    pub media_type: Option<String>,
    pub checksum: Option<Digest>,
}

/// What kind of file a STAC asset is, and therefore how to acquire it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// A zip holding one or more GeoPackages.
    ZippedGeoPackage,
    /// A zip holding shapefile components.
    ZippedShapefiles,
    /// A GeoPackage published directly, with no archive around it.
    PlainGeoPackage,
}

impl AssetKind {
    pub fn is_archive(&self) -> bool {
        !matches!(self, AssetKind::PlainGeoPackage)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub datetime: Option<String>,
    pub bbox: Option<Vec<f64>>,
    pub assets: Vec<Asset>,
}

impl Item {
    /// How a collection's data is packaged, which decides how it is acquired.
    ///
    /// swisstopo and ASTRA publish the same kind of thing four different ways, and the
    /// difference is not visible from the collection id.
    pub fn primary_asset(&self) -> Result<(&Asset, AssetKind)> {
        // Order matters: a GeoPackage is preferred over a shapefile because it needs no
        // dBASE decoding, and the zipped form is listed first because most collections
        // publish only that.
        const PREFERENCE: [(&str, AssetKind); 3] = [
            (".gpkg.zip", AssetKind::ZippedGeoPackage),
            (".shp.zip", AssetKind::ZippedShapefiles),
            (".gpkg", AssetKind::PlainGeoPackage),
        ];
        for (suffix, kind) in PREFERENCE {
            if let Some(a) = self.assets.iter().find(|a| a.name.ends_with(suffix)) {
                return Ok((a, kind));
            }
        }
        Err(Error::NotFound(format!(
            "item {} publishes no GeoPackage or shapefile asset; available: {:?}",
            self.id,
            self.assets.iter().map(|a| &a.name).collect::<Vec<_>>()
        )))
    }

    /// The asset whose name ends with `suffix`, e.g. `.gpkg.zip`.
    pub fn asset_ending(&self, suffix: &str) -> Result<&Asset> {
        self.assets
            .iter()
            .find(|a| a.name.ends_with(suffix))
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "item {} has no asset ending in {suffix:?}; available: {:?}",
                    self.id,
                    self.assets.iter().map(|a| &a.name).collect::<Vec<_>>()
                ))
            })
    }
}

/// Item parsing, exposed so tests can build items from real catalog documents rather
/// than from hand-constructed structs.
#[doc(hidden)]
pub fn parse_items_for_test(doc: &serde_json::Value) -> Vec<Item> {
    parse_items(doc)
}

fn parse_items(doc: &serde_json::Value) -> Vec<Item> {
    doc.get("features")
        .and_then(|f| f.as_array())
        .map(|feats| {
            feats
                .iter()
                .map(|f| {
                    let assets = f
                        .get("assets")
                        .and_then(|a| a.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(name, v)| {
                                    Some(Asset {
                                        name: name.clone(),
                                        href: v.get("href")?.as_str()?.to_string(),
                                        media_type: v
                                            .get("type")
                                            .and_then(|t| t.as_str())
                                            .map(str::to_owned),
                                        checksum: v
                                            .get("file:checksum")
                                            .and_then(|c| c.as_str())
                                            .and_then(|c| Digest::from_multihash(c).ok()),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Item {
                        id: f
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        datetime: f
                            .pointer("/properties/datetime")
                            .and_then(|d| d.as_str())
                            .map(str::to_owned),
                        bbox: f
                            .get("bbox")
                            .and_then(|b| b.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_f64()).collect()),
                        assets,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn next_link(doc: &serde_json::Value) -> Option<String> {
    doc.get("links")?
        .as_array()?
        .iter()
        .find(|l| l.get("rel").and_then(|r| r.as_str()) == Some("next"))?
        .get("href")?
        .as_str()
        .map(str::to_owned)
}

pub struct Stac<'a> {
    http: &'a dyn Http,
    root: String,
}

impl<'a> Stac<'a> {
    pub fn new(http: &'a dyn Http) -> Self {
        Self {
            http,
            root: ROOT.to_string(),
        }
    }

    pub fn with_root(http: &'a dyn Http, root: impl Into<String>) -> Self {
        Self {
            http,
            root: root.into(),
        }
    }

    /// All items in a collection, following `rel=next` pagination.
    /// `max_pages` bounds the walk; swissALTI3D has tens of thousands of items.
    pub async fn items(&self, collection: &str, max_pages: usize) -> Result<Vec<Item>> {
        let mut url = format!("{}/collections/{collection}/items?limit=100", self.root);
        let mut out = Vec::new();
        for _ in 0..max_pages {
            let doc = self.http.get_json(&url).await?;
            out.extend(parse_items(&doc));
            match next_link(&doc) {
                Some(n) => url = n,
                None => break,
            }
        }
        Ok(out)
    }

    /// Items intersecting a WGS84 bbox, following pagination.
    pub async fn items_in_bbox(
        &self,
        collection: &str,
        bbox: [f64; 4],
        max_pages: usize,
    ) -> Result<Vec<Item>> {
        let mut url = format!(
            "{}/collections/{collection}/items?bbox={},{},{},{}&limit=100",
            self.root, bbox[0], bbox[1], bbox[2], bbox[3]
        );
        let mut out = Vec::new();
        for _ in 0..max_pages {
            let doc = self.http.get_json(&url).await?;
            out.extend(parse_items(&doc));
            match next_link(&doc) {
                Some(n) => url = n,
                None => break,
            }
        }
        Ok(out)
    }

    /// The newest release of a collection, by `datetime` then id.
    pub async fn latest(&self, collection: &str) -> Result<Item> {
        let mut items = self.items(collection, 50).await?;
        if items.is_empty() {
            return Err(Error::NotFound(format!(
                "collection {collection} has no items"
            )));
        }
        items.sort_by(|a, b| a.datetime.cmp(&b.datetime).then_with(|| a.id.cmp(&b.id)));
        Ok(items.pop().expect("non-empty"))
    }
}
