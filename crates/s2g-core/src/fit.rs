//! Garmin FIT course import (SPEC.md FR-38).
//!
//! Reads only what a corridor needs: the ordered positions of a course. That is the
//! `record` messages (global message 20), with `course_point` (32) as a fallback for
//! files that carry turn points but no track, and the course name from `course` (31).
//!
//! The format is documented by Garmin as the FIT Protocol; the parts implemented here
//! are the file header, definition and data messages, both endiannesses, compressed
//! timestamp headers and developer fields (skipped, but their sizes respected — a
//! parser that ignores the developer field count desynchronises on any file written by
//! a modern head unit).
//!
//! Positions are `sint32` semicircles: degrees = value × 180 / 2^31.

use std::path::Path;

use crate::error::{Error, Result};

/// A position from a course, in WGS84 degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitPoint {
    pub lat: f64,
    pub lon: f64,
    pub ele_m: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Course {
    pub name: Option<String>,
    /// From `record` messages when present, otherwise from `course_point`.
    pub points: Vec<FitPoint>,
}

const MSG_COURSE: u16 = 31;
const MSG_RECORD: u16 = 20;
const MSG_COURSE_POINT: u16 = 32;

/// Semicircles to degrees.
fn semicircles(v: i64) -> f64 {
    v as f64 * (180.0 / 2147483648.0)
}

#[derive(Clone)]
struct FieldDef {
    number: u8,
    size: usize,
    base_type: u8,
}

#[derive(Clone)]
struct MessageDef {
    global: u16,
    big_endian: bool,
    fields: Vec<FieldDef>,
    /// Developer fields carry no course data but do occupy bytes.
    dev_size: usize,
}

impl MessageDef {
    fn record_size(&self) -> usize {
        self.fields.iter().map(|f| f.size).sum::<usize>() + self.dev_size
    }
}

/// Read an integer field, honouring the message's endianness and signedness.
///
/// `None` when the value is the base type's "invalid" sentinel, which FIT uses in place
/// of absent fields — treating those as data is how a course ends up with a point at
/// 21.47° N.
fn read_int(bytes: &[u8], base_type: u8, big_endian: bool) -> Option<i64> {
    // Low 5 bits are the base type number; bit 7 marks endian-dependent types.
    let signed = matches!(base_type & 0x1F, 0x01 | 0x03 | 0x05 | 0x0E);
    let width = bytes.len();
    let mut raw: u64 = 0;
    if big_endian {
        for b in bytes {
            raw = (raw << 8) | *b as u64;
        }
    } else {
        for b in bytes.iter().rev() {
            raw = (raw << 8) | *b as u64;
        }
    }
    // Invalid sentinels: all bits set for unsigned, max positive for signed.
    let all_ones = if width >= 8 {
        u64::MAX
    } else {
        (1u64 << (width * 8)) - 1
    };
    if signed {
        if raw == all_ones >> 1 {
            return None;
        }
        let shift = 64 - width * 8;
        Some(((raw << shift) as i64) >> shift)
    } else {
        if raw == all_ones {
            return None;
        }
        Some(raw as i64)
    }
}

pub fn parse(data: &[u8]) -> Result<Course> {
    if data.len() < 14 {
        return Err(Error::Zip("FIT file is too short to hold a header".into()));
    }
    let header_size = data[0] as usize;
    if !(12..=255).contains(&header_size) || header_size > data.len() {
        return Err(Error::Zip(format!(
            "FIT header size {header_size} is invalid"
        )));
    }
    if &data[8..12] != b".FIT" {
        return Err(Error::Zip(
            "not a FIT file: missing the .FIT signature".into(),
        ));
    }
    let data_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    // Trust the smaller of declared and actual: a truncated file should yield the
    // points it does have rather than an error.
    let end = (header_size + data_size).min(data.len());

    let mut defs: [Option<MessageDef>; 16] = Default::default();
    let mut course = Course::default();
    let mut course_points: Vec<FitPoint> = Vec::new();

    let mut i = header_size;
    while i < end {
        let header = data[i];
        i += 1;

        if header & 0x80 != 0 {
            // Compressed timestamp header: a data message, local type in bits 5-6.
            let local = ((header >> 5) & 0x03) as usize;
            let Some(def) = defs[local].clone() else {
                // Without its definition the record length is unknown, so nothing
                // after this point can be located. Stop rather than guess.
                break;
            };
            if i + def.record_size() > end {
                break;
            }
            read_data(
                &def,
                &data[i..i + def.record_size()],
                &mut course,
                &mut course_points,
            );
            i += def.record_size();
            continue;
        }

        let local = (header & 0x0F) as usize;
        if header & 0x40 != 0 {
            // Definition message.
            if i + 5 > end {
                break;
            }
            let big_endian = data[i + 1] == 1;
            let global = if big_endian {
                u16::from_be_bytes([data[i + 2], data[i + 3]])
            } else {
                u16::from_le_bytes([data[i + 2], data[i + 3]])
            };
            let n = data[i + 4] as usize;
            i += 5;
            if i + n * 3 > end {
                break;
            }
            let mut fields = Vec::with_capacity(n);
            for k in 0..n {
                fields.push(FieldDef {
                    number: data[i + k * 3],
                    size: data[i + k * 3 + 1] as usize,
                    base_type: data[i + k * 3 + 2],
                });
            }
            i += n * 3;

            // Developer fields, present only when the header's bit 5 is set.
            let mut dev_size = 0usize;
            if header & 0x20 != 0 {
                if i >= end {
                    break;
                }
                let dn = data[i] as usize;
                i += 1;
                if i + dn * 3 > end {
                    break;
                }
                for k in 0..dn {
                    dev_size += data[i + k * 3 + 1] as usize;
                }
                i += dn * 3;
            }

            defs[local] = Some(MessageDef {
                global,
                big_endian,
                fields,
                dev_size,
            });
        } else {
            // Data message.
            let Some(def) = defs[local].clone() else {
                break;
            };
            let size = def.record_size();
            if i + size > end {
                break;
            }
            read_data(&def, &data[i..i + size], &mut course, &mut course_points);
            i += size;
        }
    }

    if course.points.is_empty() {
        course.points = course_points;
    }
    if course.points.is_empty() {
        return Err(Error::NotFound(
            "the FIT file contains no course positions".into(),
        ));
    }
    Ok(course)
}

fn read_data(
    def: &MessageDef,
    body: &[u8],
    course: &mut Course,
    course_points: &mut Vec<FitPoint>,
) {
    let mut offset = 0usize;
    let mut lat: Option<f64> = None;
    let mut lon: Option<f64> = None;
    let mut ele: Option<f64> = None;
    let mut name: Option<String> = None;

    for f in &def.fields {
        let raw = &body[offset..offset + f.size];
        offset += f.size;
        match (def.global, f.number) {
            // record: position_lat, position_long, altitude, enhanced_altitude
            (MSG_RECORD, 0) | (MSG_COURSE_POINT, 1) => {
                lat = read_int(raw, f.base_type, def.big_endian).map(semicircles);
            }
            (MSG_RECORD, 1) | (MSG_COURSE_POINT, 2) => {
                lon = read_int(raw, f.base_type, def.big_endian).map(semicircles);
            }
            // Both altitude fields are scaled by 5 with a 500 m offset.
            (MSG_RECORD, 2) | (MSG_RECORD, 78) => {
                ele = read_int(raw, f.base_type, def.big_endian).map(|v| v as f64 / 5.0 - 500.0);
            }
            (MSG_COURSE, 5) => name = read_string(raw),
            _ => {}
        }
    }

    match def.global {
        MSG_COURSE => {
            if course.name.is_none() {
                course.name = name;
            }
        }
        MSG_RECORD => {
            if let (Some(lat), Some(lon)) = (lat, lon) {
                course.points.push(FitPoint {
                    lat,
                    lon,
                    ele_m: ele,
                });
            }
        }
        MSG_COURSE_POINT => {
            if let (Some(lat), Some(lon)) = (lat, lon) {
                course_points.push(FitPoint {
                    lat,
                    lon,
                    ele_m: None,
                });
            }
        }
        _ => {}
    }
}

/// FIT strings are null-terminated UTF-8, padded to the field size.
fn read_string(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    let s = String::from_utf8_lossy(&raw[..end]).trim().to_string();
    (!s.is_empty()).then_some(s)
}

pub fn parse_file(path: &Path) -> Result<Course> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    parse(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds FIT byte streams, so the parser is tested against the format rather than
    /// against one sample file. A real Garmin course file should still be checked once
    /// hardware is at hand — see docs/device-verification.md.
    struct Writer {
        body: Vec<u8>,
    }

    impl Writer {
        fn new() -> Self {
            Self { body: Vec::new() }
        }

        fn definition(
            &mut self,
            local: u8,
            global: u16,
            fields: &[(u8, u8, u8)],
            big_endian: bool,
        ) {
            self.body.push(0x40 | (local & 0x0F));
            self.body.push(0); // reserved
            self.body.push(big_endian as u8);
            if big_endian {
                self.body.extend_from_slice(&global.to_be_bytes());
            } else {
                self.body.extend_from_slice(&global.to_le_bytes());
            }
            self.body.push(fields.len() as u8);
            for (n, size, bt) in fields {
                self.body.extend_from_slice(&[*n, *size, *bt]);
            }
        }

        /// Definition with developer fields, whose bytes must be skipped by size.
        fn definition_with_dev(
            &mut self,
            local: u8,
            global: u16,
            fields: &[(u8, u8, u8)],
            dev: &[(u8, u8, u8)],
        ) {
            self.body.push(0x40 | 0x20 | (local & 0x0F));
            self.body.push(0);
            self.body.push(0);
            self.body.extend_from_slice(&global.to_le_bytes());
            self.body.push(fields.len() as u8);
            for (n, size, bt) in fields {
                self.body.extend_from_slice(&[*n, *size, *bt]);
            }
            self.body.push(dev.len() as u8);
            for (n, size, idx) in dev {
                self.body.extend_from_slice(&[*n, *size, *idx]);
            }
        }

        fn data(&mut self, local: u8, payload: &[u8]) {
            self.body.push(local & 0x0F);
            self.body.extend_from_slice(payload);
        }

        fn compressed(&mut self, local: u8, payload: &[u8]) {
            self.body.push(0x80 | ((local & 0x03) << 5));
            self.body.extend_from_slice(payload);
        }

        fn finish(self) -> Vec<u8> {
            let mut out = vec![12, 0x20, 0, 0];
            out.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
            out.extend_from_slice(b".FIT");
            out.extend_from_slice(&self.body);
            out
        }
    }

    /// 180 / 2^31 degrees per semicircle.
    fn to_semi(deg: f64) -> i32 {
        (deg / (180.0 / 2147483648.0)) as i32
    }

    const SINT32: u8 = 0x85;
    const UINT16: u8 = 0x84;
    const STRING: u8 = 0x07;

    fn record_bytes(lat: f64, lon: f64, alt_m: f64) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&to_semi(lat).to_le_bytes());
        v.extend_from_slice(&to_semi(lon).to_le_bytes());
        v.extend_from_slice(&(((alt_m + 500.0) * 5.0) as u16).to_le_bytes());
        v
    }

    #[test]
    fn reads_a_course_name_and_its_record_positions() {
        let mut w = Writer::new();
        w.definition(0, MSG_COURSE, &[(5, 16, STRING)], false);
        let mut name = b"Haute Route".to_vec();
        name.resize(16, 0);
        w.data(0, &name);
        w.definition(
            1,
            MSG_RECORD,
            &[(0, 4, SINT32), (1, 4, SINT32), (2, 2, UINT16)],
            false,
        );
        w.data(1, &record_bytes(46.0207, 7.7491, 1620.0));
        w.data(1, &record_bytes(46.0250, 7.7600, 1900.0));

        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.name.as_deref(), Some("Haute Route"));
        assert_eq!(c.points.len(), 2);
        assert!((c.points[0].lat - 46.0207).abs() < 1e-6);
        assert!((c.points[0].lon - 7.7491).abs() < 1e-6);
        assert!((c.points[0].ele_m.unwrap() - 1620.0).abs() < 0.2);
    }

    #[test]
    fn big_endian_files_are_read_correctly() {
        let mut w = Writer::new();
        w.definition(0, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], true);
        let mut payload = Vec::new();
        payload.extend_from_slice(&to_semi(46.5).to_be_bytes());
        payload.extend_from_slice(&to_semi(8.5).to_be_bytes());
        w.data(0, &payload);

        let c = parse(&w.finish()).unwrap();
        assert!((c.points[0].lat - 46.5).abs() < 1e-6);
        assert!((c.points[0].lon - 8.5).abs() < 1e-6);
    }

    #[test]
    fn developer_field_bytes_are_skipped_by_size() {
        // A parser that ignored the developer field count would desynchronise here and
        // read the next record's header as coordinate data.
        let mut w = Writer::new();
        w.definition_with_dev(
            0,
            MSG_RECORD,
            &[(0, 4, SINT32), (1, 4, SINT32)],
            &[(0, 4, 0)],
        );
        let mut p1 = Vec::new();
        p1.extend_from_slice(&to_semi(46.5).to_le_bytes());
        p1.extend_from_slice(&to_semi(8.5).to_le_bytes());
        p1.extend_from_slice(&[9, 9, 9, 9]); // developer payload
        w.data(0, &p1);
        let mut p2 = Vec::new();
        p2.extend_from_slice(&to_semi(46.6).to_le_bytes());
        p2.extend_from_slice(&to_semi(8.6).to_le_bytes());
        p2.extend_from_slice(&[9, 9, 9, 9]);
        w.data(0, &p2);

        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.points.len(), 2);
        assert!((c.points[1].lat - 46.6).abs() < 1e-6);
    }

    #[test]
    fn compressed_timestamp_records_are_data_messages() {
        let mut w = Writer::new();
        w.definition(2, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], false);
        let mut p = Vec::new();
        p.extend_from_slice(&to_semi(46.7).to_le_bytes());
        p.extend_from_slice(&to_semi(8.7).to_le_bytes());
        w.compressed(2, &p);

        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.points.len(), 1);
        assert!((c.points[0].lat - 46.7).abs() < 1e-6);
    }

    #[test]
    fn invalid_positions_are_skipped_not_taken_as_coordinates() {
        // 0x7FFFFFFF is the sint32 invalid sentinel, which is 180° if read as data.
        let mut w = Writer::new();
        w.definition(0, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], false);
        let mut bad = Vec::new();
        bad.extend_from_slice(&i32::MAX.to_le_bytes());
        bad.extend_from_slice(&i32::MAX.to_le_bytes());
        w.data(0, &bad);
        let mut good = Vec::new();
        good.extend_from_slice(&to_semi(46.5).to_le_bytes());
        good.extend_from_slice(&to_semi(8.5).to_le_bytes());
        w.data(0, &good);

        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.points.len(), 1);
        assert!((c.points[0].lat - 46.5).abs() < 1e-6);
    }

    #[test]
    fn negative_coordinates_survive_the_sign_extension() {
        let mut w = Writer::new();
        w.definition(0, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], false);
        let mut p = Vec::new();
        p.extend_from_slice(&to_semi(-33.9).to_le_bytes());
        p.extend_from_slice(&to_semi(-18.4).to_le_bytes());
        w.data(0, &p);

        let c = parse(&w.finish()).unwrap();
        assert!((c.points[0].lat + 33.9).abs() < 1e-6);
        assert!((c.points[0].lon + 18.4).abs() < 1e-6);
    }

    #[test]
    fn course_points_are_used_only_when_there_are_no_records() {
        let mut w = Writer::new();
        w.definition(
            0,
            MSG_COURSE_POINT,
            &[(1, 4, SINT32), (2, 4, SINT32)],
            false,
        );
        let mut p = Vec::new();
        p.extend_from_slice(&to_semi(46.1).to_le_bytes());
        p.extend_from_slice(&to_semi(8.1).to_le_bytes());
        w.data(0, &p);
        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.points.len(), 1);

        // With records present, those win: they are the actual path.
        let mut w = Writer::new();
        w.definition(
            0,
            MSG_COURSE_POINT,
            &[(1, 4, SINT32), (2, 4, SINT32)],
            false,
        );
        w.data(0, &p);
        w.definition(1, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], false);
        let mut r = Vec::new();
        r.extend_from_slice(&to_semi(46.9).to_le_bytes());
        r.extend_from_slice(&to_semi(8.9).to_le_bytes());
        w.data(1, &r);
        let c = parse(&w.finish()).unwrap();
        assert_eq!(c.points.len(), 1);
        assert!((c.points[0].lat - 46.9).abs() < 1e-6);
    }

    #[test]
    fn a_truncated_file_yields_the_records_it_does_contain() {
        let mut w = Writer::new();
        w.definition(0, MSG_RECORD, &[(0, 4, SINT32), (1, 4, SINT32)], false);
        let mut p = Vec::new();
        p.extend_from_slice(&to_semi(46.5).to_le_bytes());
        p.extend_from_slice(&to_semi(8.5).to_le_bytes());
        w.data(0, &p);
        let mut bytes = w.finish();
        // Chop mid-record after appending a partial second one.
        bytes.push(0);
        bytes.extend_from_slice(&[1, 2, 3]);

        let c = parse(&bytes).unwrap();
        assert_eq!(c.points.len(), 1);
    }

    #[test]
    fn a_file_without_the_signature_is_rejected() {
        let mut bytes = Writer::new().finish();
        bytes[8] = b'X';
        assert!(parse(&bytes).is_err());
        assert!(parse(&[0u8; 4]).is_err());
    }

    #[test]
    fn a_data_message_without_its_definition_stops_parsing_rather_than_guessing() {
        let mut w = Writer::new();
        w.data(5, &[1, 2, 3, 4]);
        assert!(parse(&w.finish()).is_err());
    }

    #[test]
    fn a_course_with_no_positions_is_an_error() {
        let mut w = Writer::new();
        w.definition(0, MSG_COURSE, &[(5, 8, STRING)], false);
        w.data(0, b"Empty\0\0\0");
        assert!(parse(&w.finish()).is_err());
    }
}
