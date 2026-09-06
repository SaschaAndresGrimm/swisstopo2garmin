//! Shapefile reader, for the ASTRA route networks (SPEC.md FR-50, Cycling preset).
//!
//! `ch.astra.veloland`, `ch.astra.mountainbikeland` and `ch.astra.wanderland` publish
//! **shapefile and File Geodatabase only** — no GeoPackage — and swissTLM3D contains no
//! cycle data at all (docs/m0-findings.md §4.19). So cycle routes require this reader.
//!
//! Only what those datasets actually need is implemented: 2D and Z/M polyline, polygon
//! and point shapes, plus dBASE III attributes. Anything else is reported rather than
//! guessed at.
//!
//! Shapefiles carry no spatial index, so a bbox query scans the record headers. Each
//! record stores its own bounding box, so only candidates are fully decoded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::geom::{Coord, Geometry};
use crate::gpkg::{Feature, Value};
use crate::proj::BBox;

const SHP_MAGIC: i32 = 9994;
const HEADER_LEN: usize = 100;

/// Shape types we handle. The Z and M variants carry extra trailing arrays that are
/// read past and discarded, since Garmin maps are 2D (FR-P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeType {
    Null,
    Point,
    PolyLine,
    Polygon,
    MultiPoint,
}

fn classify(raw: i32) -> Result<ShapeType> {
    Ok(match raw {
        0 => ShapeType::Null,
        1 | 11 | 21 => ShapeType::Point,
        3 | 13 | 23 => ShapeType::PolyLine,
        5 | 15 | 25 => ShapeType::Polygon,
        8 | 18 | 28 => ShapeType::MultiPoint,
        other => {
            return Err(Error::Zip(format!(
                "unsupported shapefile shape type {other}"
            )))
        }
    })
}

pub struct Shapefile {
    path: PathBuf,
    shp: Vec<u8>,
    /// Attribute rows, parallel to shape records.
    dbf: Dbf,
    pub shape_type: ShapeType,
    pub bbox: BBox,
}

impl Shapefile {
    /// Open a `.shp` and its sibling `.dbf`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let shp = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
        if shp.len() < HEADER_LEN {
            return Err(Error::Zip(format!("{} is too short", path.display())));
        }
        if i32::from_be_bytes(shp[0..4].try_into().unwrap()) != SHP_MAGIC {
            return Err(Error::Zip(format!(
                "{} is not a shapefile (bad magic)",
                path.display()
            )));
        }
        let shape_type = classify(i32::from_le_bytes(shp[32..36].try_into().unwrap()))?;
        let f = |o: usize| f64::from_le_bytes(shp[o..o + 8].try_into().unwrap());
        let bbox = BBox::new(f(36), f(44), f(52), f(60));

        let dbf_path = path.with_extension("dbf");
        let dbf = if dbf_path.exists() {
            Dbf::open(&dbf_path)?
        } else {
            Dbf::empty()
        };

        Ok(Self {
            path,
            shp,
            dbf,
            shape_type,
            bbox,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn field_names(&self) -> Vec<String> {
        self.dbf.fields.iter().map(|f| f.name.clone()).collect()
    }

    /// Stream features whose bounding box intersects `bbox`.
    pub fn for_each_in_bbox<F>(
        &self,
        bbox: &BBox,
        attributes: &[&str],
        mut sink: F,
    ) -> Result<usize>
    where
        F: FnMut(Feature) -> bool,
    {
        let mut pos = HEADER_LEN;
        let mut index = 0usize;
        let mut emitted = 0usize;

        while pos + 8 <= self.shp.len() {
            let content_words = i32::from_be_bytes(self.shp[pos + 4..pos + 8].try_into().unwrap());
            let content_len = (content_words as usize) * 2;
            let body = pos + 8;
            if content_len == 0 || body + content_len > self.shp.len() {
                break;
            }
            let record = &self.shp[body..body + content_len];
            pos = body + content_len;
            let record_index = index;
            index += 1;

            let raw_type = i32::from_le_bytes(record[0..4].try_into().unwrap());
            let kind = classify(raw_type)?;
            if kind == ShapeType::Null {
                continue;
            }

            // Every non-point record stores its own bbox, so filtering is cheap.
            if kind != ShapeType::Point {
                if record.len() < 36 {
                    continue;
                }
                let g = |o: usize| f64::from_le_bytes(record[o..o + 8].try_into().unwrap());
                let rec_box = BBox::new(g(4), g(12), g(20), g(28));
                if !rec_box.intersects(bbox) {
                    continue;
                }
            }

            let Some(geometry) = decode(kind, record)? else {
                continue;
            };
            if geometry.bbox().map(|b| !b.intersects(bbox)).unwrap_or(true) {
                continue;
            }

            let mut attrs = HashMap::new();
            for name in attributes {
                if let Some(v) = self.dbf.value(record_index, name) {
                    attrs.insert((*name).to_string(), v);
                }
            }

            emitted += 1;
            if !sink(Feature {
                id: record_index as i64,
                geometry,
                attributes: attrs,
            }) {
                break;
            }
        }
        Ok(emitted)
    }
}

fn decode(kind: ShapeType, r: &[u8]) -> Result<Option<Geometry>> {
    let f = |o: usize| f64::from_le_bytes(r[o..o + 8].try_into().unwrap());
    match kind {
        ShapeType::Point => {
            if r.len() < 20 {
                return Ok(None);
            }
            Ok(Some(Geometry::Point(Coord::new(f(4), f(12)))))
        }
        ShapeType::MultiPoint => {
            let n = i32::from_le_bytes(r[36..40].try_into().unwrap()) as usize;
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                let o = 40 + i * 16;
                if o + 16 > r.len() {
                    break;
                }
                v.push(Coord::new(f(o), f(o + 8)));
            }
            Ok((!v.is_empty()).then_some(Geometry::MultiPoint(v)))
        }
        ShapeType::PolyLine | ShapeType::Polygon => {
            if r.len() < 44 {
                return Ok(None);
            }
            let num_parts = i32::from_le_bytes(r[36..40].try_into().unwrap()) as usize;
            let num_points = i32::from_le_bytes(r[40..44].try_into().unwrap()) as usize;
            let parts_off = 44;
            let points_off = parts_off + num_parts * 4;
            if points_off + num_points * 16 > r.len() {
                return Ok(None);
            }
            let part_start = |i: usize| {
                i32::from_le_bytes(
                    r[parts_off + i * 4..parts_off + i * 4 + 4]
                        .try_into()
                        .unwrap(),
                ) as usize
            };

            let mut parts: Vec<Vec<Coord>> = Vec::with_capacity(num_parts);
            for i in 0..num_parts {
                let from = part_start(i);
                let to = if i + 1 < num_parts {
                    part_start(i + 1)
                } else {
                    num_points
                };
                let mut ring = Vec::with_capacity(to.saturating_sub(from));
                for p in from..to {
                    let o = points_off + p * 16;
                    ring.push(Coord::new(f(o), f(o + 8)));
                }
                if ring.len() >= 2 {
                    parts.push(ring);
                }
            }
            if parts.is_empty() {
                return Ok(None);
            }

            Ok(Some(if kind == ShapeType::Polygon {
                // Shapefile rings are ordered clockwise for exteriors and
                // counter-clockwise for holes, but distinguishing them properly needs
                // containment tests. Every ring becomes its own polygon, which matches
                // how the extractor emits closed ways anyway.
                Geometry::MultiPolygon(parts.into_iter().map(|r| vec![r]).collect())
            } else if parts.len() == 1 {
                Geometry::LineString(parts.into_iter().next().unwrap())
            } else {
                Geometry::MultiLineString(parts)
            }))
        }
        ShapeType::Null => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// dBASE III attributes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DbfField {
    name: String,
    kind: u8,
    length: usize,
    offset: usize,
}

struct Dbf {
    fields: Vec<DbfField>,
    records: Vec<u8>,
    record_len: usize,
    count: usize,
    /// True when the byte stream decodes as UTF-8; otherwise Latin-1 is assumed.
    utf8: bool,
}

impl Dbf {
    fn empty() -> Self {
        Self {
            fields: Vec::new(),
            records: Vec::new(),
            record_len: 0,
            count: 0,
            utf8: true,
        }
    }

    fn open(path: &Path) -> Result<Self> {
        let raw = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        if raw.len() < 32 {
            return Err(Error::Zip(format!("{} is too short", path.display())));
        }
        let count = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
        let header_len = u16::from_le_bytes(raw[8..10].try_into().unwrap()) as usize;
        let record_len = u16::from_le_bytes(raw[10..12].try_into().unwrap()) as usize;

        let mut fields = Vec::new();
        let mut off = 1usize; // byte 0 of each record is the deletion flag
        let mut p = 32usize;
        while p + 32 <= header_len && raw.get(p).copied().unwrap_or(0x0D) != 0x0D {
            let name_bytes: Vec<u8> = raw[p..p + 11]
                .iter()
                .copied()
                .take_while(|c| *c != 0)
                .collect();
            let length = raw[p + 16] as usize;
            fields.push(DbfField {
                name: String::from_utf8_lossy(&name_bytes).trim().to_string(),
                kind: raw[p + 11],
                length,
                offset: off,
            });
            off += length;
            p += 32;
        }

        let records = raw.get(header_len..).unwrap_or(&[]).to_vec();
        // A .cpg sidecar declares the encoding; ASTRA ships Latin-1 in practice, but
        // checking the bytes is more reliable than trusting the sidecar.
        let utf8 = std::str::from_utf8(&records).is_ok();
        Ok(Self {
            fields,
            records,
            record_len,
            count,
            utf8,
        })
    }

    fn decode(&self, bytes: &[u8]) -> String {
        if self.utf8 {
            String::from_utf8_lossy(bytes).trim().to_string()
        } else {
            // Latin-1: every byte is a code point, so this never fails.
            bytes
                .iter()
                .map(|b| *b as char)
                .collect::<String>()
                .trim()
                .to_string()
        }
    }

    fn value(&self, index: usize, name: &str) -> Option<Value> {
        if index >= self.count || self.record_len == 0 {
            return None;
        }
        let field = self.fields.iter().find(|f| f.name == name)?;
        let start = index * self.record_len + field.offset;
        let raw = self.records.get(start..start + field.length)?;
        let text = self.decode(raw);
        if text.is_empty() {
            return Some(Value::Null);
        }
        Some(match field.kind {
            // N and F are stored as text; parse so numeric style rules work.
            b'N' | b'F' => match text.parse::<i64>() {
                Ok(i) => Value::Int(i),
                Err(_) => match text.parse::<f64>() {
                    Ok(f) => Value::Real(f),
                    Err(_) => Value::Text(text),
                },
            },
            b'L' => Value::Text(text),
            _ => Value::Text(text),
        })
    }
}
