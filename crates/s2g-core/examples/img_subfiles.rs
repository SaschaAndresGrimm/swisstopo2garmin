//! Subfile inventory of one or more `.img` files.
//!
//!   cargo run --release -q -p s2g-core --example img_subfiles -- a.img b.img
//!
//! What a Garmin map *contains* is not visible in a screenshot: routing is NOD and NET,
//! search is MDR and LBL, relief is DEM. This prints them side by side so a claim about
//! a feature can be checked against the bytes.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        return Err("usage: img_subfiles <file.img>...".into());
    }
    const KINDS: [&str; 9] = [
        "TRE", "RGN", "LBL", "NET", "NOD", "DEM", "TYP", "MDR", "SRT",
    ];
    print!("{:<34}", "file");
    for k in KINDS {
        print!("{k:>9}");
    }
    println!("{:>11}", "total");

    for f in &files {
        let info = s2g_core::img::read(std::path::Path::new(f))?;
        let name = std::path::Path::new(f)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| f.clone());
        print!("{:<34}", &name[..name.len().min(33)]);
        for k in KINDS {
            print!("{:>9}", info.bytes_of_kind(k));
        }
        println!("{:>11}", std::fs::metadata(f)?.len());
    }
    Ok(())
}
