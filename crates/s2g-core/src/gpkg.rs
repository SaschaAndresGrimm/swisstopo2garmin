//! GeoPackage reader (SPEC.md §7.2).
//!
//! A GeoPackage is a plain SQLite database, so this needs no GDAL: SQLite plus a WKB
//! parser is enough, which keeps the application a single dependency-free binary.
//!
//! Two facts from Milestone 0 shape this module:
//!
//! * **R-tree indexes exist** on all 32 swissTLM3D spatial layers, declared in
//!   `gpkg_extensions`. A 144 km² clip takes ~2 s, which is why the tiled intermediate
//!   the spec originally required was dropped. Do **not** infer index presence by
//!   splitting `rtree_*` object names — layer names contain underscores, and that
//!   produced a false negative during Milestone 0.
//! * **`k_W` is a no-data sentinel**, not a value, and appears across many attribute
//!   columns. [`Value::as_meaningful_str`] filters it alongside NULL and
//!   "Keine Angabe".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::{Error, Result};
use crate::geom::{Coord, Geometry, Ring};
use crate::proj::BBox;

/// Values that mean "no data" in swissTLM3D and must never reach a style rule.
pub const NO_DATA: &[&str] = &["", "k_W", "Keine Angabe"];

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub name: String,
    /// Integer primary key column. `id` in swissTLM3D, but `fid` in GeoPackages
    /// written by GDAL, so it must be discovered rather than assumed.
    pub id_column: String,
    pub geometry_column: Option<String>,
    pub geometry_type: Option<String>,
    pub srs_id: i64,
    pub identifier: Option<String>,
    pub has_rtree: bool,
    pub columns: Vec<String>,
}

impl LayerInfo {
    pub fn is_spatial(&self) -> bool {
        self.geometry_column.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl Value {
    /// The text value, or `None` if it is NULL or a swissTLM3D no-data sentinel.
    pub fn as_meaningful_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) if !NO_DATA.contains(&s.as_str()) => Some(s),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Feature {
    pub id: i64,
    pub geometry: Geometry,
    pub attributes: HashMap<String, Value>,
}

impl Feature {
    /// Attribute value, filtered through the no-data sentinels.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).and_then(Value::as_meaningful_str)
    }
}

pub struct Gpkg {
    conn: Connection,
    path: PathBuf,
}

fn sql_err(path: &Path, e: rusqlite::Error) -> Error {
    Error::io(path, std::io::Error::other(e.to_string()))
}

impl Gpkg {
    /// Open read-only. The file is never modified.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(|e| sql_err(&path, e))?;
        Ok(Self { conn, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Layers declared in `gpkg_contents`, with their geometry column and index status.
    pub fn layers(&self) -> Result<Vec<LayerInfo>> {
        // Authoritative source for spatial indexes; see the module note.
        let mut rtree: Vec<String> = Vec::new();
        if let Ok(mut st) = self.conn.prepare(
            "SELECT table_name FROM gpkg_extensions WHERE extension_name = 'gpkg_rtree_index'",
        ) {
            if let Ok(rows) = st.query_map([], |r| r.get::<_, String>(0)) {
                rtree = rows.flatten().collect();
            }
        }

        let mut geom: HashMap<String, (String, String, i64)> = HashMap::new();
        {
            let mut st = self
                .conn
                .prepare(
                    "SELECT table_name, column_name, geometry_type_name, srs_id \
                     FROM gpkg_geometry_columns",
                )
                .map_err(|e| sql_err(&self.path, e))?;
            let rows = st
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|e| sql_err(&self.path, e))?;
            for row in rows.flatten() {
                geom.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut st = self
            .conn
            .prepare("SELECT table_name, identifier, srs_id FROM gpkg_contents ORDER BY table_name")
            .map_err(|e| sql_err(&self.path, e))?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| sql_err(&self.path, e))?;

        let mut out = Vec::new();
        for (name, identifier, srs_id) in rows.flatten() {
            let g = geom.get(&name);
            out.push(LayerInfo {
                id_column: self.primary_key_of(&name).unwrap_or_else(|| "id".into()),
                columns: self.columns_of(&name).unwrap_or_default(),
                geometry_column: g.map(|x| x.0.clone()),
                geometry_type: g.map(|x| x.1.clone()),
                srs_id: g.map(|x| x.2).unwrap_or(srs_id),
                has_rtree: rtree.contains(&name),
                identifier,
                name,
            });
        }
        Ok(out)
    }

    /// The declared INTEGER PRIMARY KEY of a table, if it has one.
    pub fn primary_key_of(&self, table: &str) -> Option<String> {
        let mut st = self
            .conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", escape_ident(table)))
            .ok()?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(5)?)))
            .ok()?;
        // Collected rather than returned directly so the borrow of `st` ends inside
        // this statement.
        let pk: Vec<String> = rows
            .flatten()
            .filter(|(_, is_pk)| *is_pk == 1)
            .map(|(name, _)| name)
            .collect();
        pk.into_iter().next()
    }

    pub fn columns_of(&self, table: &str) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", escape_ident(table)))
            .map_err(|e| sql_err(&self.path, e))?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| sql_err(&self.path, e))?;
        Ok(rows.flatten().collect())
    }

    pub fn count(&self, table: &str) -> Result<i64> {
        self.conn
            .query_row(
                &format!("SELECT COUNT(*) FROM \"{}\"", escape_ident(table)),
                [],
                |r| r.get(0),
            )
            .map_err(|e| sql_err(&self.path, e))
    }

    /// Number of features whose bounding box intersects `bbox`.
    ///
    /// This is an R-tree-only query — it never touches feature geometry — which makes
    /// it cheap enough to drive the size estimator (SPEC.md FR-61).
    pub fn count_in_bbox(&self, table: &str, bbox: &BBox) -> Result<i64> {
        let t = escape_ident(table);
        let geom_col = self
            .layers()?
            .into_iter()
            .find(|l| l.name == table)
            .and_then(|l| l.geometry_column)
            .map(|c| escape_ident(&c))
            .ok_or_else(|| Error::NotFound(format!("layer {table} has no geometry")))?;
        self.conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM \"rtree_{t}_{geom_col}\" \
                     WHERE maxx >= ?1 AND minx <= ?2 AND maxy >= ?3 AND miny <= ?4"
                ),
                [bbox.min_e, bbox.max_e, bbox.min_n, bbox.max_n],
                |r| r.get(0),
            )
            .map_err(|e| sql_err(&self.path, e))
    }

    /// Stream features intersecting `bbox`, calling `sink` once per feature.
    ///
    /// Streaming rather than collecting is deliberate: swissTLM3D layers reach
    /// 11.5 M features and nothing may hold a layer in memory (SPEC.md NFR-2).
    /// Returning `false` from `sink` stops the scan.
    pub fn for_each_in_bbox<F>(
        &self,
        table: &str,
        bbox: &BBox,
        attributes: &[&str],
        mut sink: F,
    ) -> Result<usize>
    where
        F: FnMut(Feature) -> bool,
    {
        let t = escape_ident(table);
        let layer = self
            .layers()?
            .into_iter()
            .find(|l| l.name == table)
            .ok_or_else(|| Error::NotFound(format!("layer {table} not in gpkg_contents")))?;
        let id_col = escape_ident(&layer.id_column);
        let geom_col = escape_ident(
            layer
                .geometry_column
                .as_deref()
                .ok_or_else(|| Error::NotFound(format!("layer {table} has no geometry")))?,
        );
        let available = self.columns_of(table)?;
        let cols: Vec<&str> = attributes
            .iter()
            .copied()
            .filter(|a| available.iter().any(|c| c == a))
            .collect();

        let selected = cols
            .iter()
            .map(|c| format!(", f.\"{}\"", escape_ident(c)))
            .collect::<String>();

        // The R-tree join is what makes regional extraction fast; without it this is
        // a full table scan of up to 11.5 M rows.
        let sql = format!(
            "SELECT f.\"{id_col}\", f.\"{geom_col}\"{selected} FROM \"{t}\" f \
             JOIN \"rtree_{t}_{geom_col}\" r ON f.\"{id_col}\" = r.id \
             WHERE r.maxx >= ?1 AND r.minx <= ?2 AND r.maxy >= ?3 AND r.miny <= ?4"
        );

        let mut st = self
            .conn
            .prepare(&sql)
            .map_err(|e| sql_err(&self.path, e))?;
        let mut rows = st
            .query([bbox.min_e, bbox.max_e, bbox.min_n, bbox.max_n])
            .map_err(|e| sql_err(&self.path, e))?;

        let mut n = 0usize;
        while let Some(row) = rows.next().map_err(|e| sql_err(&self.path, e))? {
            let id: i64 = row.get(0).map_err(|e| sql_err(&self.path, e))?;
            let blob: Option<Vec<u8>> = row.get(1).map_err(|e| sql_err(&self.path, e))?;
            let Some(blob) = blob else { continue };
            let Some(geometry) = parse_gpkg_geometry(&blob)? else {
                continue;
            };

            let mut attrs = HashMap::with_capacity(cols.len());
            for (i, name) in cols.iter().enumerate() {
                let v = match row.get_ref(i + 2) {
                    Ok(rusqlite::types::ValueRef::Null) | Err(_) => Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(x)) => Value::Int(x),
                    Ok(rusqlite::types::ValueRef::Real(x)) => Value::Real(x),
                    Ok(rusqlite::types::ValueRef::Text(x)) => {
                        Value::Text(String::from_utf8_lossy(x).into_owned())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(_)) => Value::Null,
                };
                attrs.insert((*name).to_string(), v);
            }

            n += 1;
            if !sink(Feature {
                id,
                geometry,
                attributes: attrs,
            }) {
                break;
            }
        }
        Ok(n)
    }
}

/// SQLite has no parameter binding for identifiers, so doubled quotes are the escape.
fn escape_ident(s: &str) -> String {
    s.replace('"', "\"\"")
}

// ---------------------------------------------------------------------------
// GPKG geometry blob -> Geometry
// ---------------------------------------------------------------------------

/// Parse a GeoPackage geometry blob: `"GP"` magic, flags, optional envelope, then WKB.
pub fn parse_gpkg_geometry(blob: &[u8]) -> Result<Option<Geometry>> {
    if blob.len() < 8 || &blob[0..2] != b"GP" {
        return Err(Error::Zip("not a GeoPackage geometry blob".into()));
    }
    let flags = blob[3];
    if flags & 0x10 != 0 {
        return Ok(None); // empty geometry
    }
    let envelope_doubles = match (flags >> 1) & 0x07 {
        0 => 0,
        1 => 4,
        2 | 3 => 6,
        4 => 8,
        other => return Err(Error::Zip(format!("bad envelope indicator {other}"))),
    };
    let offset = 8 + envelope_doubles * 8;
    if blob.len() < offset + 5 {
        return Ok(None);
    }
    let (geom, _) = parse_wkb(blob, offset)?;
    Ok(Some(geom))
}

fn parse_wkb(buf: &[u8], mut off: usize) -> Result<(Geometry, usize)> {
    let little = buf[off] == 1;
    let gtype = read_u32(buf, off + 1, little)?;
    off += 5;

    // ISO WKB: +1000 = Z, +2000 = M, +3000 = ZM
    let has_z = matches!(gtype / 1000, 1 | 3);
    let has_m = matches!(gtype / 1000, 2 | 3);
    let ndim = 2 + usize::from(has_z) + usize::from(has_m);
    let base = gtype % 1000;

    let read_points = |o: &mut usize, count: usize| -> Result<Vec<Coord>> {
        let need = count * ndim * 8;
        if buf.len() < *o + need {
            return Err(Error::Zip("truncated WKB point array".into()));
        }
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            // Z and M are read past but discarded: Garmin maps are 2D (FR-P2).
            let e = read_f64(buf, *o, little)?;
            let n = read_f64(buf, *o + 8, little)?;
            v.push(Coord::new(e, n));
            *o += ndim * 8;
        }
        Ok(v)
    };

    let read_ring_list = |o: &mut usize| -> Result<Vec<Ring>> {
        let nrings = read_u32(buf, *o, little)? as usize;
        *o += 4;
        let mut rings = Vec::with_capacity(nrings);
        for _ in 0..nrings {
            let n = read_u32(buf, *o, little)? as usize;
            *o += 4;
            rings.push(read_points(o, n)?);
        }
        Ok(rings)
    };

    let geom = match base {
        1 => {
            let p = read_points(&mut off, 1)?;
            Geometry::Point(p[0])
        }
        2 => {
            let n = read_u32(buf, off, little)? as usize;
            off += 4;
            Geometry::LineString(read_points(&mut off, n)?)
        }
        3 => Geometry::Polygon(read_ring_list(&mut off)?),
        4..=6 => {
            let count = read_u32(buf, off, little)? as usize;
            off += 4;
            let mut points = Vec::new();
            let mut lines = Vec::new();
            let mut polys = Vec::new();
            for _ in 0..count {
                let (sub, next) = parse_wkb(buf, off)?;
                off = next;
                match sub {
                    Geometry::Point(c) => points.push(c),
                    Geometry::LineString(l) => lines.push(l),
                    Geometry::Polygon(r) => polys.push(r),
                    other => {
                        return Err(Error::Zip(format!(
                            "unexpected {} inside a multi-geometry",
                            other.kind()
                        )))
                    }
                }
            }
            match base {
                4 => Geometry::MultiPoint(points),
                5 => Geometry::MultiLineString(lines),
                _ => Geometry::MultiPolygon(polys),
            }
        }
        other => return Err(Error::Zip(format!("unsupported WKB geometry type {other}"))),
    };
    Ok((geom, off))
}

fn read_u32(buf: &[u8], off: usize, little: bool) -> Result<u32> {
    let b: [u8; 4] = buf
        .get(off..off + 4)
        .ok_or_else(|| Error::Zip("truncated WKB".into()))?
        .try_into()
        .map_err(|_| Error::Zip("truncated WKB".into()))?;
    Ok(if little {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    })
}

fn read_f64(buf: &[u8], off: usize, little: bool) -> Result<f64> {
    let b: [u8; 8] = buf
        .get(off..off + 8)
        .ok_or_else(|| Error::Zip("truncated WKB".into()))?
        .try_into()
        .map_err(|_| Error::Zip("truncated WKB".into()))?;
    Ok(if little {
        f64::from_le_bytes(b)
    } else {
        f64::from_be_bytes(b)
    })
}
