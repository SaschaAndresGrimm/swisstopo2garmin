//! swissTLM3D -> OSM PBF extraction (SPEC.md §7.3, §7.4).
//!
//! Reads a region out of the national GeoPackage, clips it, and writes OSM PBF whose
//! tags carry the raw TLM attributes under a `tlm:` prefix. The mkgmap style rules key
//! off those tags directly, so cartography stays editable without touching Rust.
//!
//! Layer and attribute names below all appear in `docs/tlm3d-schema.md`, which is
//! generated from the real file (CLAUDE.md rule 2).

use std::path::Path;

use crate::download::Cancel;
use crate::error::Result;
use crate::geom::{clip, simplify, Geometry};
use crate::gpkg::{Feature, Gpkg};
use crate::pbf::PbfWriter;
use crate::proj::{lv95_to_wgs84, BBox};

/// Which panel group a layer belongs to (SPEC.md FR-51).
///
/// Groups exist so the layer panel is navigable; they carry no cartographic meaning.
/// Note that hiking trails are **not** a group: they are an attribute of the road
/// layer (`tlm:wanderwege`), so they cannot be toggled independently of roads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayerGroup {
    LandCover,
    Water,
    Transport,
    Built,
    Names,
    Winter,
    Cycling,
}

impl LayerGroup {
    /// In panel order.
    pub fn all() -> &'static [LayerGroup] {
        &[
            LayerGroup::LandCover,
            LayerGroup::Water,
            LayerGroup::Transport,
            LayerGroup::Built,
            LayerGroup::Names,
            LayerGroup::Winter,
            LayerGroup::Cycling,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            LayerGroup::LandCover => "landCover",
            LayerGroup::Water => "water",
            LayerGroup::Transport => "transport",
            LayerGroup::Built => "built",
            LayerGroup::Names => "names",
            LayerGroup::Winter => "winter",
            LayerGroup::Cycling => "cycling",
        }
    }
}

/// A source layer and the attributes worth carrying into the map.
pub struct LayerSpec {
    /// Layer name, or a prefix of one.
    ///
    /// Some swisstopo layers carry the release year in their name
    /// (`ski_routes_2056`), so an exact match would break on the next release. The
    /// name is resolved exactly first, then by prefix.
    pub layer: &'static str,
    pub attributes: &'static [&'static str],
    /// Simplification tolerance in metres; 0 disables it.
    pub simplify_m: f64,
    /// Tag namespace. swissTLM3D uses `tlm`; other sources use their own so a style
    /// rule cannot accidentally match the wrong dataset's attribute.
    pub prefix: &'static str,
    /// Panel group, for the layer list in the GUI.
    pub group: LayerGroup,
}

/// Default extraction set.
///
/// `tlm_bb_einzelbaum` is deliberately absent: 11.5 M individual trees, 53% of all
/// features in the dataset, and meaningless at Garmin zoom levels
/// (docs/m0-findings.md §2.4).
pub const DEFAULT_LAYERS: &[LayerSpec] = &[
    LayerSpec {
        layer: "tlm_strassen_strasse",
        attributes: &[
            "objektart",
            "wanderwege",
            "belagsart",
            "kunstbaute",
            "stufe",
            "verkehrsbedeutung",
            "verkehrsbeschraenkung",
            "befahrbarkeit",
            "richtungsgetrennt",
            "strassenname",
        ],
        simplify_m: 1.0,
        prefix: "tlm",
        group: LayerGroup::Transport,
    },
    LayerSpec {
        layer: "tlm_bb_bodenbedeckung",
        attributes: &["objektart"],
        simplify_m: 2.0,
        prefix: "tlm",
        group: LayerGroup::LandCover,
    },
    LayerSpec {
        layer: "tlm_gewaesser_fliessgewaesser",
        attributes: &["objektart", "name", "verlauf"],
        simplify_m: 1.0,
        prefix: "tlm",
        group: LayerGroup::Water,
    },
    LayerSpec {
        layer: "tlm_gewaesser_stehendes_gewaesser",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
        prefix: "tlm",
        group: LayerGroup::Water,
    },
    LayerSpec {
        layer: "tlm_bauten_gebaeude_footprint",
        attributes: &["objektart"],
        simplify_m: 0.5,
        prefix: "tlm",
        group: LayerGroup::Built,
    },
    LayerSpec {
        layer: "tlm_oev_eisenbahn",
        attributes: &["objektart", "name"],
        simplify_m: 1.0,
        prefix: "tlm",
        group: LayerGroup::Transport,
    },
    // Lifts and cableways: 2,903 features nationally, and essential context on a
    // Swiss hiking or ski map. Present in swissTLM3D all along but never extracted.
    LayerSpec {
        layer: "tlm_oev_uebrige_bahn",
        attributes: &["objektart", "name"],
        simplify_m: 1.0,
        prefix: "tlm",
        group: LayerGroup::Transport,
    },
    LayerSpec {
        layer: "tlm_areale_nutzungsareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
        prefix: "tlm",
        group: LayerGroup::LandCover,
    },
    LayerSpec {
        layer: "tlm_areale_freizeitareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
        prefix: "tlm",
        group: LayerGroup::LandCover,
    },
    LayerSpec {
        layer: "tlm_areale_verkehrsareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
        prefix: "tlm",
        group: LayerGroup::Transport,
    },
    LayerSpec {
        layer: "tlm_namen_flurname",
        attributes: &["objektart", "name"],
        simplify_m: 0.0,
        prefix: "tlm",
        group: LayerGroup::Names,
    },
    LayerSpec {
        layer: "tlm_namen_siedlungsname_zentrum",
        attributes: &["objektart", "name", "einwohnerkategorie"],
        simplify_m: 0.0,
        prefix: "tlm",
        group: LayerGroup::Names,
    },
    LayerSpec {
        layer: "tlm_eo_einzelobjekt",
        attributes: &["objektart", "name"],
        simplify_m: 0.0,
        prefix: "tlm",
        group: LayerGroup::Built,
    },
];

/// Winter sport routes, from three separate swisstopo/ASTRA GeoPackages.
///
/// Layer names, attributes and value domains all verified against the real files:
/// `ski_routes` carries 10,789 SAC tours, `ski_network` a 19,915-segment connectivity
/// graph with an access classification, and the two ASTRA sets 280 snowshoe and 507
/// winter hiking routes.
///
/// `Datenstand_Kantone` is deliberately absent from every set: it records which cantons
/// have supplied current data and is metadata, not map content.
pub const WINTER_LAYERS: &[LayerSpec] = &[
    // The SAC ski tour network. `access` distinguishes skiable from carrying and
    // caution sections, which is the distinction that matters in the field.
    LayerSpec {
        layer: "ski_network",
        attributes: &["discipline", "access", "direction", "route_info"],
        simplify_m: 2.0,
        prefix: "sac",
        group: LayerGroup::Winter,
    },
    // Named tours with SAC difficulty, ascent and target.
    LayerSpec {
        layer: "ski_routes",
        attributes: &[
            "discipline",
            "name",
            "difficulty",
            "ascent_altitude",
            "ascent_time_label",
            "target_name",
            "target_altitude",
            "route_nr",
        ],
        simplify_m: 2.0,
        prefix: "sac",
        group: LayerGroup::Winter,
    },
    // ASTRA / SchweizMobil signposted snowshoe trails.
    LayerSpec {
        layer: "Schneeschuhwanderwege",
        attributes: &["NameR", "NrR", "TechnikR", "KonditionR", "Routenart"],
        simplify_m: 2.0,
        prefix: "swm",
        group: LayerGroup::Winter,
    },
    // ASTRA / SchweizMobil signposted winter hiking trails.
    LayerSpec {
        layer: "Winterwanderwege",
        attributes: &["NameR", "NrR", "KonditionR", "Routenart"],
        simplify_m: 2.0,
        prefix: "swm",
        group: LayerGroup::Winter,
    },
];

/// Cycle and mountain-bike networks, from the ASTRA shapefiles.
///
/// swissTLM3D carries no cycle data at all (docs/m0-findings.md §4.19) and these
/// datasets publish no GeoPackage, so they arrive through [`crate::shapefile`].
///
/// `Etappe` (stages) is deliberately absent: it duplicates the route geometry with
/// stage metadata that adds nothing on a device screen.
pub const CYCLE_LAYERS: &[LayerSpec] = &[
    // The cycle path network. Netzhier looks like a hierarchy but is empty in the
    // real data, so there is nothing to grade the network by.
    LayerSpec {
        layer: "VeloWeg",
        attributes: &["ObjektArt", "BelagTLM", "VerkehrM"],
        simplify_m: 2.0,
        prefix: "astra",
        group: LayerGroup::Cycling,
    },
    // The MTB network. IsSTrail marks singletrail, which is the distinction a rider
    // cares about.
    LayerSpec {
        layer: "MTBWeg",
        attributes: &["IsSTrail", "Technik", "InfraMtb", "BelagTLM"],
        simplify_m: 2.0,
        prefix: "astra",
        group: LayerGroup::Cycling,
    },
    // Named routes, for the route number label.
    LayerSpec {
        layer: "Route",
        attributes: &["NameR", "NrR", "Routenart", "TechnikR", "KonditionR"],
        simplify_m: 2.0,
        prefix: "astra",
        group: LayerGroup::Cycling,
    },
];

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExtractStats {
    pub features: u64,
    pub nodes: u64,
    pub ways: u64,
    pub points_before_simplify: u64,
    pub points_after_simplify: u64,
    /// Layers requested but absent from this GeoPackage.
    pub missing_layers: Vec<String>,
    pub per_layer: Vec<(String, u64)>,
}

impl ExtractStats {
    pub fn simplification_ratio(&self) -> f64 {
        if self.points_before_simplify == 0 {
            return 0.0;
        }
        1.0 - self.points_after_simplify as f64 / self.points_before_simplify as f64
    }
}

/// Extract `bbox` from `gpkg` into an OSM PBF at `dest`.
///
/// Convenience wrapper for vectors only; use [`RegionBuilder`] to add contours to the
/// same file.
pub fn extract_to_pbf(
    gpkg: &Gpkg,
    bbox: &BBox,
    layers: &[LayerSpec],
    dest: &Path,
    cancel: &Cancel,
    on_layer: impl FnMut(&str, u64),
) -> Result<ExtractStats> {
    let mut b = RegionBuilder::create(dest, bbox)?;
    b.add_vectors(gpkg, layers, cancel, on_layer)?;
    b.finish()
}

/// Builds one OSM PBF for a region from several sources.
///
/// Vectors and contours must share a single writer: element ids have to form one
/// ascending sequence or `splitter` rejects the file, and two writers would each
/// start from 1.
pub struct RegionBuilder {
    writer: PbfWriter<std::io::BufWriter<std::fs::File>>,
    bbox: BBox,
    stats: ExtractStats,
}

impl RegionBuilder {
    pub fn create(dest: &Path, bbox: &BBox) -> Result<Self> {
        Ok(Self {
            writer: PbfWriter::create(dest, bbox.to_wgs84())?,
            bbox: *bbox,
            stats: ExtractStats::default(),
        })
    }

    pub fn stats(&self) -> &ExtractStats {
        &self.stats
    }

    pub fn finish(self) -> Result<ExtractStats> {
        let mut stats = self.stats;
        let (nodes, ways) = self.writer.finish()?;
        stats.nodes = nodes;
        stats.ways = ways;
        Ok(stats)
    }

    /// Add contour lines, tagged the way mkgmap styles expect (FR-P6).
    ///
    /// `on_ice` decides whether a contour is drawn in the ice palette; the Landeskarte
    /// draws contours blue over glacier and firn rather than bistre.
    pub fn add_contours(
        &mut self,
        contours: &[crate::contour::Contour],
        mut on_ice: impl FnMut(crate::geom::Coord) -> bool,
        cancel: &Cancel,
    ) -> Result<u64> {
        let mut n = 0u64;
        for c in contours {
            if cancel.is_cancelled() {
                return Err(crate::Error::Cancelled);
            }
            let mut tags = vec![
                ("contour".to_string(), "elevation".to_string()),
                ("ele".to_string(), c.elevation.to_string()),
                ("contour_ext".to_string(), c.tier.contour_ext().to_string()),
            ];
            // Classified by the midpoint. A contour that only partly crosses ice takes
            // one colour throughout, which is acceptable at Garmin resolution and far
            // cheaper than splitting the line.
            let mid = c.points[c.points.len() / 2];
            if on_ice(mid) {
                tags.push(("contour_surface".to_string(), "ice".to_string()));
            }

            let refs: Vec<i64> = c
                .points
                .iter()
                .map(|p| {
                    let (lon, lat) = lv95_to_wgs84(p.e, p.n);
                    self.writer.node_at(lon, lat)
                })
                .collect();
            if self.writer.way(refs, tags).is_some() {
                n += 1;
            }
        }
        self.stats.features += n;
        self.stats.per_layer.push(("contours".to_string(), n));
        Ok(n)
    }

    /// Add features from a shapefile.
    ///
    /// Shapefiles hold one layer per file with no layer name inside, so the spec's
    /// `layer` names the file stem and becomes the `layer` tag.
    pub fn add_shapefile(
        &mut self,
        shp: &crate::shapefile::Shapefile,
        spec: &LayerSpec,
        cancel: &Cancel,
    ) -> Result<u64> {
        self.add_shapefile_as(shp, spec, spec.layer, cancel)
    }

    /// As [`RegionBuilder::add_shapefile`], but with an explicit layer tag.
    ///
    /// Needed because all three ASTRA datasets ship a file called `Route.shp`. Tagged
    /// by file stem alone, a mountain-bike route would be indistinguishable from a
    /// cycle route and would render in the wrong colour.
    pub fn add_shapefile_as(
        &mut self,
        shp: &crate::shapefile::Shapefile,
        spec: &LayerSpec,
        layer_tag: &str,
        cancel: &Cancel,
    ) -> Result<u64> {
        let bbox = &self.bbox;
        let stats = &mut self.stats;
        let writer = &mut self.writer;
        let mut count = 0u64;

        shp.for_each_in_bbox(bbox, spec.attributes, |f| {
            if cancel.is_cancelled() {
                return false;
            }
            let Some(clipped) = clip(&f.geometry, bbox) else {
                return true;
            };
            let mut tags = build_tags(spec, &f);
            // Replace the layer tag with the qualified name.
            if let Some(slot) = tags.iter_mut().find(|(k, _)| k.ends_with(":layer")) {
                slot.1 = layer_tag.to_string();
            }
            emit(writer, &clipped, &tags, spec.simplify_m, stats);
            count += 1;
            true
        })?;

        if cancel.is_cancelled() {
            return Err(crate::Error::Cancelled);
        }
        stats.features += count;
        stats.per_layer.push((layer_tag.to_string(), count));
        Ok(count)
    }

    pub fn add_vectors(
        &mut self,
        gpkg: &Gpkg,
        layers: &[LayerSpec],
        cancel: &Cancel,
        mut on_layer: impl FnMut(&str, u64),
    ) -> Result<()> {
        let available: Vec<String> = gpkg.layers()?.into_iter().map(|l| l.name).collect();
        let bbox = &self.bbox;
        let stats = &mut self.stats;
        let writer = &mut self.writer;

        for spec in layers {
            if cancel.is_cancelled() {
                return Err(crate::Error::Cancelled);
            }
            // Resolved by prefix as well as exact name, because some layers carry
            // the release year (ski_routes_2056).
            let Some(layer_name) = resolve_layer(&available, spec.layer) else {
                stats.missing_layers.push(spec.layer.to_string());
                continue;
            };

            let mut count = 0u64;
            gpkg.for_each_in_bbox(&layer_name, bbox, spec.attributes, |f| {
                if cancel.is_cancelled() {
                    return false;
                }
                let Some(clipped) = clip(&f.geometry, bbox) else {
                    return true;
                };
                let tags = build_tags(spec, &f);
                emit(writer, &clipped, &tags, spec.simplify_m, stats);
                count += 1;
                true
            })?;

            stats.features += count;
            stats.per_layer.push((spec.layer.to_string(), count));
            on_layer(spec.layer, count);
        }

        if cancel.is_cancelled() {
            return Err(crate::Error::Cancelled);
        }
        Ok(())
    }
}

/// Exact layer name, or the first one starting with `wanted`.
fn resolve_layer(available: &[String], wanted: &str) -> Option<String> {
    available
        .iter()
        .find(|a| a.as_str() == wanted)
        .or_else(|| available.iter().find(|a| a.starts_with(wanted)))
        .cloned()
}

fn build_tags(spec: &LayerSpec, f: &Feature) -> Vec<(String, String)> {
    let mut tags = Vec::with_capacity(spec.attributes.len() + 1);
    let p = spec.prefix;
    tags.push((format!("{p}:layer"), spec.layer.to_string()));
    for a in spec.attributes {
        // `attr` filters NULL and the k_W / "Keine Angabe" sentinels, so a style rule
        // can never match a no-data value.
        // `tag` rather than `attr`: numeric attributes must not be dropped.
        if let Some(v) = f.tag(a) {
            if *a == "name" || *a == "strassenname" || *a == "NameR" {
                // Multilingual names arrive pipe-separated. Rendered verbatim the
                // device would show "Bern | Berna | Berna | Berne", so only the
                // primary variant becomes the label; the rest are kept searchable
                // under alt_name (FR-P5).
                let variants = crate::gpkg::split_names(&v);
                if let Some(primary) = variants.first() {
                    tags.push((format!("{p}:{a}"), (*primary).to_string()));
                }
                if variants.len() > 1 {
                    tags.push(("alt_name".to_string(), variants[1..].join(";")));
                }
            } else {
                tags.push((format!("{p}:{a}"), v.to_string()));
            }
        }
    }
    tags
}

fn emit<W: std::io::Write>(
    w: &mut PbfWriter<W>,
    geom: &Geometry,
    tags: &[(String, String)],
    simplify_m: f64,
    stats: &mut ExtractStats,
) {
    match geom {
        Geometry::Point(c) => {
            let (lon, lat) = lv95_to_wgs84(c.e, c.n);
            w.poi(lon, lat, tags.to_vec());
        }
        Geometry::MultiPoint(v) => {
            for c in v {
                let (lon, lat) = lv95_to_wgs84(c.e, c.n);
                w.poi(lon, lat, tags.to_vec());
            }
        }
        Geometry::LineString(v) => emit_line(w, v, tags, simplify_m, stats, false),
        Geometry::MultiLineString(vs) => {
            for v in vs {
                emit_line(w, v, tags, simplify_m, stats, false);
            }
        }
        Geometry::Polygon(rings) => {
            // Each ring becomes a closed way. Holes are emitted as their own closed
            // ways; representing them properly needs multipolygon relations, which
            // matters for large water bodies and is deferred with the routing work.
            for r in rings {
                emit_line(w, r, tags, simplify_m, stats, true);
            }
        }
        Geometry::MultiPolygon(polys) => {
            for rings in polys {
                for r in rings {
                    emit_line(w, r, tags, simplify_m, stats, true);
                }
            }
        }
    }
}

fn emit_line<W: std::io::Write>(
    w: &mut PbfWriter<W>,
    points: &[crate::geom::Coord],
    tags: &[(String, String)],
    simplify_m: f64,
    stats: &mut ExtractStats,
    closed: bool,
) {
    stats.points_before_simplify += points.len() as u64;
    let simplified = simplify(points, simplify_m);
    stats.points_after_simplify += simplified.len() as u64;

    let mut refs: Vec<i64> = simplified
        .iter()
        .map(|c| {
            let (lon, lat) = lv95_to_wgs84(c.e, c.n);
            w.node_at(lon, lat)
        })
        .collect();
    refs.dedup();

    if closed {
        if refs.len() < 3 {
            return;
        }
        if refs.first() != refs.last() {
            refs.push(refs[0]);
        }
    }
    w.way(refs, tags.to_vec());
}
