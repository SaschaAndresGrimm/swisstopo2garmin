//! The official directory of building addresses, for address search on the device
//! (SPEC.md §16 v2, "Address & POI search").
//!
//! swissTLM3D has 173,860 street *names* and no house numbers — the whole schema was
//! checked. House numbers live in a separate free dataset,
//! `ch.swisstopo.amtliches-gebaeudeadressverzeichnis`, which is the binding register for
//! Swiss public authorities: 3.3 million addresses, published as a semicolon-separated
//! CSV in LV95 under the same OGD terms as everything else here.
//!
//! Read a line at a time and discarded unless it falls inside the build area. The
//! uncompressed CSV is 468 MB, so holding it would breach NFR-2 on its own — and the
//! scan is linear and constant-memory, which is the same shape as the swissNAMES3D
//! reader next door.

use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::{Error, Result};
use crate::proj::BBox;

/// One official building address.
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    /// `STN_LABEL`, the official street name.
    pub street: String,
    /// `ADR_NUMBER`. Not always an integer — "31.1" and "12a" both occur.
    pub number: String,
    /// Postcode alone, split out of `ZIP_LABEL` ("4052 Basel").
    pub postcode: String,
    /// The locality from `ZIP_LABEL`, which is the postal town rather than the commune.
    /// A search for an address wants the name on the envelope.
    pub locality: String,
    pub easting: f64,
    pub northing: f64,
}

/// How many rows were read and how many survived the clip.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AddressStats {
    pub scanned: u64,
    pub kept: u64,
    /// Rows skipped because the register marks them as not official or not real.
    pub unofficial: u64,
    /// Rows whose coordinates or street were unusable.
    pub malformed: u64,
}

/// The columns this reader needs, by header name.
///
/// Looked up by name rather than by position: the register is republished daily and a
/// column order is not a promise anybody made.
struct Columns {
    street: usize,
    number: usize,
    zip_label: usize,
    status: usize,
    official: usize,
    easting: usize,
    northing: usize,
}

impl Columns {
    fn from_header(header: &str) -> Result<Self> {
        let names: Vec<&str> = header
            .trim_start_matches('\u{feff}')
            .trim_end()
            .split(';')
            .collect();
        let at = |want: &str| -> Result<usize> {
            names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(want))
                .ok_or_else(|| {
                    Error::NotFound(format!(
                        "the address register has no {want} column; it has {names:?}"
                    ))
                })
        };
        Ok(Self {
            street: at("STN_LABEL")?,
            number: at("ADR_NUMBER")?,
            zip_label: at("ZIP_LABEL")?,
            status: at("ADR_STATUS")?,
            official: at("ADR_OFFICIAL")?,
            easting: at("ADR_EASTING")?,
            northing: at("ADR_NORTHING")?,
        })
    }
}

/// Split `"4052 Basel"` into its postcode and its locality.
///
/// Kept separate because mkgmap indexes them separately: the postcode narrows a search
/// and the locality is what a user recognises.
fn split_zip(label: &str) -> (String, String) {
    let label = label.trim();
    match label.split_once(' ') {
        Some((zip, rest)) if zip.chars().all(|c| c.is_ascii_digit()) => {
            (zip.to_string(), rest.trim().to_string())
        }
        // No leading postcode: keep the whole thing as the locality rather than
        // inventing a postcode from part of a name.
        _ => (String::new(), label.to_string()),
    }
}

/// Read the addresses inside `bbox`, handing each to `sink`.
///
/// `sink` returning false stops the scan, which is how cancellation gets in.
pub fn read_in_bbox<F>(csv: &Path, bbox: &BBox, mut sink: F) -> Result<AddressStats>
where
    F: FnMut(Address) -> bool,
{
    let file = std::fs::File::open(csv).map_err(|e| Error::io(csv, e))?;
    // A large buffer: this is a 468 MB sequential read and the default 8 KB one turns it
    // into sixty thousand syscalls.
    let mut reader = BufReader::with_capacity(1 << 20, file);

    let mut header = String::new();
    if reader
        .read_line(&mut header)
        .map_err(|e| Error::io(csv, e))?
        == 0
    {
        return Err(Error::NotFound(format!("{} is empty", csv.display())));
    }
    let cols = Columns::from_header(&header)?;

    let mut stats = AddressStats::default();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| Error::io(csv, e))? == 0 {
            break;
        }
        stats.scanned += 1;
        let f: Vec<&str> = line.trim_end().split(';').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or_default();

        // The register carries planned and retired addresses too. Only the real,
        // official ones belong on a map somebody navigates by.
        if !get(cols.status).eq_ignore_ascii_case("real") {
            stats.unofficial += 1;
            continue;
        }
        // ADR_OFFICIAL marks the one official address of a building where several
        // exist; the others are alternates pointing at the same door.
        if !get(cols.official).eq_ignore_ascii_case("true") {
            stats.unofficial += 1;
            continue;
        }

        let (Ok(e), Ok(n)) = (
            get(cols.easting).parse::<f64>(),
            get(cols.northing).parse::<f64>(),
        ) else {
            stats.malformed += 1;
            continue;
        };
        // The cheap test first: most of the country is outside any one build.
        if e < bbox.min_e || e > bbox.max_e || n < bbox.min_n || n > bbox.max_n {
            continue;
        }

        let street = get(cols.street).trim();
        let number = get(cols.number).trim();
        if street.is_empty() || number.is_empty() {
            stats.malformed += 1;
            continue;
        }

        let (postcode, locality) = split_zip(get(cols.zip_label));
        stats.kept += 1;
        if !sink(Address {
            street: street.to_string(),
            number: number.to_string(),
            postcode,
            locality,
            easting: e,
            northing: n,
        }) {
            break;
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real header, from the published file on 2026-09-07.
    const HEADER: &str = "\u{feff}ADR_EGAID;STR_ESID;BDG_EGID;ADR_EDID;STN_LABEL;ADR_NUMBER;\
BDG_CATEGORY;BDG_NAME;ZIP_LABEL;COM_FOSNR;COM_NAME;COM_CANTON;ADR_STATUS;ADR_OFFICIAL;\
ADR_MODIFIED;ADR_EASTING;ADR_NORTHING";

    fn write(rows: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("addresses.csv");
        let mut text = String::from(HEADER);
        text.push('\n');
        for r in rows {
            text.push_str(r);
            text.push('\n');
        }
        std::fs::write(&p, text).unwrap();
        (dir, p)
    }

    /// Two real rows, copied from the published file.
    const BASEL: &str = "102435015;10025912;243056020;0;Grellingerstrasse;31.1;non_residential;;\
4052 Basel;2701;Basel;BS;real;false;23.07.2024;2612677.919;1266615.227";
    const LOH: &str = "100265200;10063531;450862;0;Im langen Loh;19;residential;;4054 Basel;2701;\
Basel;BS;real;true;15.11.2024;2609441.493;1267258.694";

    fn basel_bbox() -> BBox {
        BBox::new(2_605_000.0, 1_263_000.0, 2_615_000.0, 1_270_000.0)
    }

    #[test]
    fn reads_the_published_columns_by_name() {
        let (_d, p) = write(&[LOH]);
        let mut got = Vec::new();
        let stats = read_in_bbox(&p, &basel_bbox(), |a| {
            got.push(a);
            true
        })
        .unwrap();

        assert_eq!(stats.kept, 1);
        assert_eq!(got.len(), 1);
        let a = &got[0];
        assert_eq!(a.street, "Im langen Loh");
        assert_eq!(a.number, "19");
        assert_eq!(a.postcode, "4054");
        assert_eq!(a.locality, "Basel");
        assert!((a.easting - 2_609_441.493).abs() < 0.001);
        assert!((a.northing - 1_267_258.694).abs() < 0.001);
    }

    /// A building with several addresses lists one as official and the rest as
    /// alternates for the same door. Indexing all of them would offer the user a choice
    /// between duplicates.
    #[test]
    fn only_official_real_addresses_are_kept() {
        let (_d, p) = write(&[BASEL, LOH]);
        let mut kept = Vec::new();
        let stats = read_in_bbox(&p, &basel_bbox(), |a| {
            kept.push(a.street);
            true
        })
        .unwrap();
        assert_eq!(stats.scanned, 2);
        assert_eq!(
            stats.kept, 1,
            "the ADR_OFFICIAL=false row should be skipped"
        );
        assert_eq!(stats.unofficial, 1);
        assert_eq!(kept, vec!["Im langen Loh"]);
    }

    /// The register is national and a build is not. Discarding by coordinate before
    /// touching the strings is what keeps a 468 MB scan cheap.
    #[test]
    fn addresses_outside_the_area_are_discarded() {
        let (_d, p) = write(&[LOH]);
        let zurich = BBox::new(2_680_000.0, 1_245_000.0, 2_690_000.0, 1_252_000.0);
        let mut n = 0;
        let stats = read_in_bbox(&p, &zurich, |_| {
            n += 1;
            true
        })
        .unwrap();
        assert_eq!(stats.scanned, 1);
        assert_eq!(stats.kept, 0);
        assert_eq!(n, 0);
    }

    #[test]
    fn a_row_with_unusable_coordinates_is_counted_not_fatal() {
        let broken = LOH.replace("2609441.493", "not-a-number");
        let (_d, p) = write(&[&broken, LOH]);
        let stats = read_in_bbox(&p, &basel_bbox(), |_| true).unwrap();
        assert_eq!(stats.malformed, 1);
        assert_eq!(stats.kept, 1);
    }

    #[test]
    fn a_row_with_no_street_or_number_is_not_an_address() {
        let no_street = LOH.replace("Im langen Loh", "");
        let no_number = LOH.replacen(";19;", ";;", 1);
        let (_d, p) = write(&[&no_street, &no_number]);
        let stats = read_in_bbox(&p, &basel_bbox(), |_| true).unwrap();
        assert_eq!(stats.kept, 0);
        assert_eq!(stats.malformed, 2);
    }

    #[test]
    fn the_postcode_is_split_from_the_locality() {
        assert_eq!(split_zip("4052 Basel"), ("4052".into(), "Basel".into()));
        assert_eq!(
            split_zip("1000 Lausanne 25"),
            ("1000".into(), "Lausanne 25".into())
        );
        // No leading postcode: keep the label rather than invent one.
        assert_eq!(split_zip("Vaduz"), (String::new(), "Vaduz".into()));
        assert_eq!(split_zip(""), (String::new(), String::new()));
    }

    /// Column order is not a promise: the register is republished daily. A reader that
    /// counted positions would read coordinates out of the wrong fields, silently.
    #[test]
    fn columns_are_found_by_name_not_by_position() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("reordered.csv");
        std::fs::write(
            &p,
            "ADR_NORTHING;ADR_EASTING;ADR_OFFICIAL;ADR_STATUS;ZIP_LABEL;ADR_NUMBER;STN_LABEL\n\
             1267258.694;2609441.493;true;real;4054 Basel;19;Im langen Loh\n",
        )
        .unwrap();
        let mut got = None;
        read_in_bbox(&p, &basel_bbox(), |a| {
            got = Some(a);
            true
        })
        .unwrap();
        let a = got.expect("the reordered row should still be read");
        assert_eq!(a.street, "Im langen Loh");
        assert!((a.easting - 2_609_441.493).abs() < 0.001);
    }

    #[test]
    fn a_missing_column_says_which_one() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("short.csv");
        std::fs::write(&p, "STN_LABEL;ADR_NUMBER\nSomewhere;1\n").unwrap();
        let err = read_in_bbox(&p, &basel_bbox(), |_| true).unwrap_err();
        assert!(err.to_string().contains("ZIP_LABEL"), "{err}");
    }

    /// Cancellation: the sink says stop and the scan stops rather than reading 3.3
    /// million more rows.
    #[test]
    fn the_sink_can_stop_the_scan() {
        let rows: Vec<String> = (0..50).map(|_| LOH.to_string()).collect();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let (_d, p) = write(&refs);
        let mut n = 0;
        let stats = read_in_bbox(&p, &basel_bbox(), |_| {
            n += 1;
            n < 3
        })
        .unwrap();
        assert_eq!(n, 3);
        assert!(stats.scanned < 50, "the scan kept going: {}", stats.scanned);
    }
}
