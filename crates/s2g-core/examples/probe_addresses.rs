//! Read the real address register and report what a build would get from it.
//!
//!   cargo run --release -q -p s2g-core --example probe_addresses -- <csv> [place]
//!
//! The unit tests use rows copied out of the published file; this reads the whole 468 MB
//! of it, which is the only way to find out what the scan actually costs and whether the
//! header still looks the way the reader expects.

use s2g_core::addresses;
use s2g_core::proj::BBox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv = std::env::args()
        .nth(1)
        .ok_or("usage: probe_addresses <csv> [place]")?;
    // Grindelwald at 6 km, the area the device test set covers.
    let bbox = BBox::new(2_642_000.0, 1_160_000.0, 2_650_000.0, 1_168_000.0);

    let started = std::time::Instant::now();
    let mut sample = Vec::new();
    let stats = addresses::read_in_bbox(std::path::Path::new(&csv), &bbox, |a| {
        if sample.len() < 6 {
            sample.push(a);
        }
        true
    })?;
    let secs = started.elapsed().as_secs_f64();

    println!("scanned   : {} rows in {secs:.1}s", stats.scanned);
    println!("kept      : {}", stats.kept);
    println!("unofficial: {}", stats.unofficial);
    println!("malformed : {}", stats.malformed);
    println!("throughput: {:.0} MB/s of CSV", 468.0 / secs);
    println!("\nsample:");
    for a in &sample {
        println!(
            "   {} {}, {} {}  ({:.0} {:.0})",
            a.street, a.number, a.postcode, a.locality, a.easting, a.northing
        );
    }
    Ok(())
}
