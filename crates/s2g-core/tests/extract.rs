//! Extraction: GeoPackage region -> OSM PBF, and then through the real Garmin
//! toolchain when it is vendored.
//!
//! The PBF container is checked structurally here, but the authoritative test is
//! whether `splitter` and `mkgmap` accept the file — a hand-written protobuf encoder
//! that only its own reader validates proves nothing.

use std::path::{Path, PathBuf};

use s2g_core::download::Cancel;
use s2g_core::extract::{extract_to_pbf, DEFAULT_LAYERS};
use s2g_core::gpkg::Gpkg;
use s2g_core::proj::BBox;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/grindelwald.gpkg"
);

fn fixture_bbox() -> BBox {
    BBox::new(2_645_000.0, 1_163_000.0, 2_647_000.0, 1_165_000.0)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// Read `vendor/toolchain.env`, if the toolchain has been fetched.
fn toolchain() -> Option<(String, String, String)> {
    let text = std::fs::read_to_string(repo_root().join("vendor/toolchain.env")).ok()?;
    let get = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(str::to_string)
    };
    Some((get("JAVA_BIN")?, get("SPLITTER_JAR")?, get("MKGMAP_JAR")?))
}

/// Walk the PBF blob container: a big-endian int32 header length, a BlobHeader, then
/// a Blob. Returns the sequence of blob type strings.
fn blob_types(path: &Path) -> Vec<String> {
    let data = std::fs::read(path).expect("read pbf");
    let mut types = Vec::new();
    let mut pos = 0usize;

    while pos + 4 <= data.len() {
        let hlen = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        assert!(
            hlen > 0 && hlen < 64 * 1024,
            "implausible header length {hlen}"
        );
        assert!(pos + hlen <= data.len(), "truncated blob header");
        let header = &data[pos..pos + hlen];
        pos += hlen;

        // BlobHeader: field 1 = type (string), field 3 = datasize (varint)
        let mut i = 0usize;
        let mut kind = String::new();
        let mut datasize = 0usize;
        while i < header.len() {
            let key = header[i];
            i += 1;
            let (field, wire) = (key >> 3, key & 0x07);
            match (field, wire) {
                (1, 2) => {
                    let (len, used) = varint(&header[i..]);
                    i += used;
                    kind = String::from_utf8_lossy(&header[i..i + len as usize]).into_owned();
                    i += len as usize;
                }
                (3, 0) => {
                    let (v, used) = varint(&header[i..]);
                    i += used;
                    datasize = v as usize;
                }
                (_, 2) => {
                    let (len, used) = varint(&header[i..]);
                    i += used + len as usize;
                }
                (_, 0) => {
                    let (_, used) = varint(&header[i..]);
                    i += used;
                }
                _ => panic!("unexpected wire type {wire} in BlobHeader"),
            }
        }
        assert!(datasize > 0, "blob {kind} declares zero size");
        assert!(pos + datasize <= data.len(), "truncated blob body");
        pos += datasize;
        types.push(kind);
    }
    assert_eq!(pos, data.len(), "trailing bytes after the last blob");
    types
}

fn varint(buf: &[u8]) -> (u64, usize) {
    let mut v = 0u64;
    let mut shift = 0;
    for (i, b) in buf.iter().enumerate() {
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return (v, i + 1);
        }
        shift += 7;
    }
    panic!("unterminated varint");
}

#[test]
fn extracts_the_fixture_into_a_well_formed_pbf() {
    let g = Gpkg::open(FIXTURE).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pbf = dir.path().join("out.osm.pbf");

    let mut layers_seen = Vec::new();
    let stats = extract_to_pbf(
        &g,
        &fixture_bbox(),
        DEFAULT_LAYERS,
        &pbf,
        &Cancel::new(),
        |name, n| layers_seen.push((name.to_string(), n)),
    )
    .unwrap();

    // The fixture only carries five of the twelve default layers; the rest must be
    // reported as missing rather than failing the extraction.
    assert_eq!(
        stats.missing_layers.len(),
        DEFAULT_LAYERS.len() - 5,
        "{:?}",
        stats.missing_layers
    );
    assert_eq!(stats.features, 716 + 99 + 54 + 104 + 2180);
    assert!(stats.nodes > 0 && stats.ways > 0);
    assert!(
        stats.simplification_ratio() > 0.0,
        "simplification should remove something"
    );

    let types = blob_types(&pbf);
    assert_eq!(types.first().map(String::as_str), Some("OSMHeader"));
    assert!(types.len() > 1, "expected data blobs after the header");
    assert!(
        types[1..].iter().all(|t| t == "OSMData"),
        "unexpected blob types: {types:?}"
    );
    println!(
        "{} features -> {} nodes, {} ways, {} blobs, {:.1} KB ({:.0}% points removed)",
        stats.features,
        stats.nodes,
        stats.ways,
        types.len(),
        std::fs::metadata(&pbf).unwrap().len() as f64 / 1024.0,
        stats.simplification_ratio() * 100.0
    );
}

#[test]
fn extraction_is_cancellable() {
    let g = Gpkg::open(FIXTURE).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pbf = dir.path().join("out.osm.pbf");
    let cancel = Cancel::new();
    cancel.cancel();

    let err = extract_to_pbf(
        &g,
        &fixture_bbox(),
        DEFAULT_LAYERS,
        &pbf,
        &cancel,
        |_, _| {},
    )
    .unwrap_err();
    assert!(matches!(err, s2g_core::Error::Cancelled), "{err}");
}

#[test]
fn an_empty_region_yields_a_header_only_pbf() {
    let g = Gpkg::open(FIXTURE).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pbf = dir.path().join("empty.osm.pbf");
    let far = BBox::new(2_700_000.0, 1_200_000.0, 2_701_000.0, 1_201_000.0);

    let stats = extract_to_pbf(&g, &far, DEFAULT_LAYERS, &pbf, &Cancel::new(), |_, _| {}).unwrap();
    assert_eq!(stats.features, 0);
    assert_eq!(blob_types(&pbf), vec!["OSMHeader"]);
}

/// The real test of the PBF encoder: does the Garmin toolchain read it?
#[test]
fn the_garmin_toolchain_accepts_our_pbf() {
    let Some((java, splitter, mkgmap)) = toolchain() else {
        eprintln!("skipping: vendor/toolchain.env absent (run vendor/fetch_tools.py)");
        return;
    };

    let g = Gpkg::open(FIXTURE).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pbf = dir.path().join("region.osm.pbf");
    extract_to_pbf(
        &g,
        &fixture_bbox(),
        DEFAULT_LAYERS,
        &pbf,
        &Cancel::new(),
        |_, _| {},
    )
    .unwrap();

    let tiles = dir.path().join("tiles");
    std::fs::create_dir_all(&tiles).unwrap();
    let split = std::process::Command::new(&java)
        .args(["-Xmx2g", "-jar", &splitter])
        .arg(format!("--output-dir={}", tiles.display()))
        .args(["--max-nodes=200000", "--mapid=63990001"])
        .arg(&pbf)
        .output()
        .expect("run splitter");
    assert!(
        split.status.success(),
        "splitter failed:\n{}\n{}",
        String::from_utf8_lossy(&split.stdout),
        String::from_utf8_lossy(&split.stderr)
    );

    let produced: Vec<PathBuf> = std::fs::read_dir(&tiles)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".osm.pbf"))
        .collect();
    assert!(!produced.is_empty(), "splitter produced no tiles");

    let img_dir = dir.path().join("img");
    std::fs::create_dir_all(&img_dir).unwrap();
    let mut cmd = std::process::Command::new(&java);
    cmd.current_dir(&img_dir)
        .args(["-Xmx2g", "-jar", &mkgmap])
        .arg(format!(
            "--style-file={}",
            repo_root().join("style/swisstopo").display()
        ))
        .args([
            "--code-page=1252",
            "--lower-case",
            "--family-id=6399",
            "--product-id=1",
            "--overview-mapname=ovm",
            "--overview-mapnumber=63990000",
        ]);
    for p in &produced {
        cmd.arg(p);
    }
    cmd.arg(repo_root().join("typ/swisstopo.txt"));
    let build = cmd.output().expect("run mkgmap");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(build.status.success(), "mkgmap failed:\n{log}");
    assert!(
        !log.contains("Exception:"),
        "mkgmap reported an exception:\n{log}"
    );

    // A tile .img must exist and carry real geometry, not just a container.
    let imgs: Vec<PathBuf> = std::fs::read_dir(&img_dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().map(|x| x == "img").unwrap_or(false)
                && p.file_name()
                    .map(|n| n.to_string_lossy().starts_with("6399"))
                    .unwrap_or(false)
        })
        .collect();
    assert!(!imgs.is_empty(), "mkgmap produced no tile .img");
    let biggest = imgs
        .iter()
        .map(|p| std::fs::metadata(p).unwrap().len())
        .max()
        .unwrap();
    assert!(
        biggest > 20_000,
        "largest .img is only {biggest} bytes, which suggests an empty map"
    );
    println!("mkgmap accepted the PBF; largest tile .img is {biggest} bytes");
}
