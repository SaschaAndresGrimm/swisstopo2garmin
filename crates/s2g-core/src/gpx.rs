//! GPX track and route import (SPEC.md FR-38).
//!
//! Deliberately a small purpose-built scanner rather than a general XML parser. GPX
//! files reaching this app come from Garmin devices, Strava, SchweizMobil and
//! komoot; all that is needed from them is the ordered `lat`/`lon` of every track,
//! route and waypoint, plus elevations where present. Pulling in an XML stack to read
//! two attributes would be a poor trade.
//!
//! What it therefore does *not* do: validate the document, resolve namespaces beyond
//! ignoring prefixes, expand entities other than the five predefined ones, or read
//! extensions. A malformed file yields whatever points could be read, which is the
//! useful behaviour for an import.

use std::path::Path;

use crate::error::{Error, Result};

/// One point of a track, route or waypoint list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackPoint {
    pub lat: f64,
    pub lon: f64,
    /// Metres above sea level, when the file carries one.
    pub ele_m: Option<f64>,
}

/// An ordered run of points: one track segment, or one route.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: Option<String>,
    pub points: Vec<TrackPoint>,
}

impl Track {
    /// Cumulative ascent in metres, ignoring points without an elevation (FR-40).
    pub fn ascent_m(&self) -> f64 {
        let mut sum = 0.0;
        let mut last: Option<f64> = None;
        for p in &self.points {
            if let Some(e) = p.ele_m {
                if let Some(prev) = last {
                    if e > prev {
                        sum += e - prev;
                    }
                }
                last = Some(e);
            }
        }
        sum
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gpx {
    /// Track segments and routes, in file order. Segments are kept separate because a
    /// gap between them is real: joining them would invent a straight line.
    pub tracks: Vec<Track>,
    pub waypoints: Vec<TrackPoint>,
}

impl Gpx {
    pub fn parse_file(path: &Path) -> Result<Gpx> {
        let text = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        // GPX is UTF-8 in practice; a stray invalid byte should not fail an import.
        let text = String::from_utf8_lossy(&text);
        let gpx = parse(&text);
        if gpx.tracks.is_empty() && gpx.waypoints.is_empty() {
            return Err(Error::NotFound(format!(
                "{} contains no track, route or waypoint coordinates",
                path.display()
            )));
        }
        Ok(gpx)
    }
}

/// Local name of a tag, with any namespace prefix and attributes stripped.
fn tag_name(tag: &str) -> (&str, bool) {
    let closing = tag.starts_with('/');
    let body = tag.trim_start_matches('/').trim_end_matches('/');
    let name = body
        .split([' ', '\t', '\n', '\r'])
        .next()
        .unwrap_or("")
        .rsplit(':')
        .next()
        .unwrap_or("");
    (name, closing)
}

/// Value of an attribute in a start tag, unescaped.
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    while let Some(i) = rest.find(name) {
        let after = &rest[i + name.len()..];
        // Guard against matching "lat" inside another attribute name.
        let before_ok = i == 0
            || rest[..i]
                .chars()
                .next_back()
                .map(|c| c.is_whitespace())
                .unwrap_or(false);
        let after_trimmed = after.trim_start();
        if before_ok && after_trimmed.starts_with('=') {
            let v = after_trimmed[1..].trim_start();
            let quote = v.chars().next()?;
            if quote == '"' || quote == '\'' {
                let end = v[1..].find(quote)?;
                return Some(unescape(&v[1..1 + end]));
            }
        }
        rest = &rest[i + name.len()..];
    }
    None
}

/// The five predefined XML entities, plus numeric character references.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        let Some(end) = after.find(';') else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()));
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    // Not an entity this needs to understand: keep it verbatim rather
                    // than dropping characters from a name.
                    None => out.push_str(&after[..=end]),
                }
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn parse(text: &str) -> Gpx {
    let mut gpx = Gpx::default();

    // Parser state. `current` collects points for the track segment or route being
    // read; `pending` is the point whose child elements are being read.
    let mut current: Option<Track> = None;
    let mut pending: Option<TrackPoint> = None;
    let mut in_waypoint = false;
    let mut track_name: Option<String> = None;
    let mut text_target: Option<&'static str> = None; // "ele" or "name"
    let mut buffer = String::new();

    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let Some(open) = text[i..].find('<').map(|o| i + o) else {
            break;
        };
        // Character data since the last tag belongs to whatever element asked for it.
        if text_target.is_some() {
            buffer.push_str(&text[i..open]);
        }
        let Some(close) = text[open..].find('>').map(|c| open + c) else {
            break;
        };
        let tag = &text[open + 1..close];
        i = close + 1;

        // Comments, CDATA and processing instructions carry nothing needed here.
        if tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        let (name, closing) = tag_name(tag);
        let self_closing = tag.ends_with('/');

        match (name, closing) {
            ("trkpt" | "rtept" | "wpt", false) => {
                let lat = attr(tag, "lat").and_then(|v| v.trim().parse().ok());
                let lon = attr(tag, "lon").and_then(|v| v.trim().parse().ok());
                if let (Some(lat), Some(lon)) = (lat, lon) {
                    let p = TrackPoint {
                        lat,
                        lon,
                        ele_m: None,
                    };
                    in_waypoint = name == "wpt";
                    if self_closing {
                        push_point(&mut gpx, &mut current, in_waypoint, p);
                        in_waypoint = false;
                    } else {
                        pending = Some(p);
                    }
                }
            }
            ("trkpt" | "rtept" | "wpt", true) => {
                if let Some(p) = pending.take() {
                    push_point(&mut gpx, &mut current, in_waypoint, p);
                }
                in_waypoint = false;
            }
            // A new segment or route starts a new run of points.
            ("trkseg" | "rte", false) => {
                flush(&mut gpx, &mut current);
                current = Some(Track {
                    name: track_name.clone(),
                    points: Vec::new(),
                });
            }
            ("trkseg" | "rte", true) => flush(&mut gpx, &mut current),
            // A track's name precedes its segments; a route's precedes its points.
            ("trk", false) => track_name = None,
            ("trk", true) => {
                flush(&mut gpx, &mut current);
                track_name = None;
            }
            ("name" | "ele", false) if !self_closing => {
                text_target = Some(if name == "ele" { "ele" } else { "name" });
                buffer.clear();
            }
            ("ele", true) => {
                if let (Some(p), Ok(v)) = (pending.as_mut(), buffer.trim().parse::<f64>()) {
                    p.ele_m = Some(v);
                }
                text_target = None;
                buffer.clear();
            }
            ("name", true) => {
                let value = unescape(buffer.trim());
                if !value.is_empty() {
                    // Whichever is open takes the name; otherwise it is the track's,
                    // read before its first segment.
                    match current.as_mut() {
                        Some(t) if t.name.is_none() => t.name = Some(value),
                        Some(_) => {}
                        None => track_name = Some(value),
                    }
                }
                text_target = None;
                buffer.clear();
            }
            _ => {}
        }
    }
    // A truncated file can leave a point whose closing tag never arrived. Keeping it
    // is the useful behaviour for an import: the coordinates were readable.
    if let Some(p) = pending.take() {
        push_point(&mut gpx, &mut current, in_waypoint, p);
    }
    flush(&mut gpx, &mut current);
    gpx
}

fn push_point(gpx: &mut Gpx, current: &mut Option<Track>, waypoint: bool, p: TrackPoint) {
    if waypoint {
        gpx.waypoints.push(p);
    } else if let Some(t) = current.as_mut() {
        t.points.push(p);
    } else {
        // A `trkpt` outside any `trkseg` is malformed but common in hand-edited files.
        *current = Some(Track {
            name: None,
            points: vec![p],
        });
    }
}

fn flush(gpx: &mut Gpx, current: &mut Option<Track>) {
    if let Some(t) = current.take() {
        if !t.points.is_empty() {
            gpx.tracks.push(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="Garmin Connect" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata><name>ignored metadata name</name></metadata>
  <wpt lat="46.6237" lon="8.0345"><name>Grimsel</name><ele>2164</ele></wpt>
  <trk>
    <name>Haute Route &amp; back</name>
    <trkseg>
      <trkpt lat="46.0207" lon="7.7491"><ele>1620.5</ele></trkpt>
      <trkpt lat="46.0250" lon="7.7600"><ele>1900.0</ele></trkpt>
      <trkpt lat="46.0300" lon="7.7700"><ele>1750.0</ele></trkpt>
    </trkseg>
    <trkseg>
      <trkpt lat="46.1000" lon="7.8000"/>
    </trkseg>
  </trk>
  <rte>
    <name>Alternative</name>
    <rtept lat="46.2000" lon="7.9000"></rtept>
    <rtept lat="46.2100" lon="7.9100"></rtept>
  </rte>
</gpx>"#;

    #[test]
    fn reads_tracks_routes_and_waypoints() {
        let g = parse(SAMPLE);
        assert_eq!(g.tracks.len(), 3, "two segments and one route");
        assert_eq!(g.tracks[0].points.len(), 3);
        assert_eq!(g.tracks[1].points.len(), 1);
        assert_eq!(g.tracks[2].points.len(), 2);
        assert_eq!(g.waypoints.len(), 1);
        assert_eq!(g.waypoints[0].ele_m, Some(2164.0));
    }

    #[test]
    fn segments_are_not_joined() {
        // Joining them would invent a straight line across the gap.
        let g = parse(SAMPLE);
        assert_ne!(g.tracks[0].points.last(), g.tracks[1].points.first());
        assert_eq!(g.tracks[1].points[0].lat, 46.1);
    }

    #[test]
    fn entity_references_in_names_are_decoded() {
        let g = parse(SAMPLE);
        assert_eq!(g.tracks[0].name.as_deref(), Some("Haute Route & back"));
        assert_eq!(g.tracks[2].name.as_deref(), Some("Alternative"));
    }

    #[test]
    fn elevations_are_read_and_ascent_ignores_descents() {
        let g = parse(SAMPLE);
        assert_eq!(g.tracks[0].points[0].ele_m, Some(1620.5));
        // 1620.5 -> 1900 is +279.5; 1900 -> 1750 does not count.
        assert!((g.tracks[0].ascent_m() - 279.5).abs() < 1e-9);
        // A segment without elevations has no ascent rather than a wrong one.
        assert_eq!(g.tracks[1].ascent_m(), 0.0);
    }

    #[test]
    fn a_self_closing_point_is_read() {
        let g = parse(r#"<gpx><trk><trkseg><trkpt lat="46.5" lon="8.5"/></trkseg></trk></gpx>"#);
        assert_eq!(g.tracks[0].points.len(), 1);
        assert_eq!(g.tracks[0].points[0].lon, 8.5);
    }

    #[test]
    fn namespace_prefixes_are_ignored() {
        let g = parse(
            r#"<gpx:gpx xmlns:gpx="x"><gpx:trk><gpx:trkseg>
               <gpx:trkpt lat="46.5" lon="8.5"><gpx:ele>1000</gpx:ele></gpx:trkpt>
               </gpx:trkseg></gpx:trk></gpx:gpx>"#,
        );
        assert_eq!(g.tracks.len(), 1);
        assert_eq!(g.tracks[0].points[0].ele_m, Some(1000.0));
    }

    #[test]
    fn attribute_order_and_extra_attributes_do_not_matter() {
        let g = parse(
            r#"<gpx><trkseg><trkpt lon = '8.5'  lat='46.5' something="lat=1"/></trkseg></gpx>"#,
        );
        assert_eq!(g.tracks[0].points[0].lat, 46.5);
        assert_eq!(g.tracks[0].points[0].lon, 8.5);
    }

    #[test]
    fn a_point_without_coordinates_is_skipped_not_fatal() {
        let g = parse(
            r#"<gpx><trkseg><trkpt lat="46.5"/><trkpt lat="46.6" lon="8.6"/></trkseg></gpx>"#,
        );
        assert_eq!(g.tracks[0].points.len(), 1);
        assert_eq!(g.tracks[0].points[0].lat, 46.6);
    }

    #[test]
    fn truncated_input_yields_what_was_readable() {
        let g = parse(r#"<gpx><trkseg><trkpt lat="46.5" lon="8.5"><ele>900</e"#);
        assert_eq!(g.tracks[0].points.len(), 1);
        // The elevation element never closed, so it is absent rather than wrong.
        assert_eq!(g.tracks[0].points[0].ele_m, None);
    }

    #[test]
    fn a_file_with_no_coordinates_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.gpx");
        std::fs::write(
            &path,
            r#"<gpx><metadata><name>nothing</name></metadata></gpx>"#,
        )
        .unwrap();
        assert!(Gpx::parse_file(&path).is_err());
    }

    #[test]
    fn a_real_file_round_trips_through_parse_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.gpx");
        std::fs::write(&path, SAMPLE).unwrap();
        let g = Gpx::parse_file(&path).unwrap();
        assert_eq!(g.tracks.len(), 3);
    }

    #[test]
    fn numeric_character_references_are_decoded() {
        assert_eq!(unescape("Gr&#xE4;chen"), "Grächen");
        assert_eq!(unescape("Gr&#228;chen"), "Grächen");
        assert_eq!(unescape("a &unknown; b"), "a &unknown; b");
        assert_eq!(unescape("no entities"), "no entities");
    }
}
