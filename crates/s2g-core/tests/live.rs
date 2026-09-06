//! Smoke tests against the real data.geo.admin.ch API.
//!
//! Ignored by default so CI stays offline and deterministic. Run explicitly:
//!   cargo test -p s2g-core --test live -- --ignored --nocapture

use s2g_core::http::ReqwestHttp;
use s2g_core::stac::{Stac, TLM3D};

#[tokio::test]
#[ignore = "requires network"]
async fn resolves_the_current_swisstlm3d_release() {
    let http = ReqwestHttp::new().unwrap();
    let stac = Stac::new(&http);
    let item = stac.latest(TLM3D).await.unwrap();
    println!("latest {} ({:?})", item.id, item.datetime);

    let asset = item.asset_ending(".gpkg.zip").unwrap();
    println!("asset  {} -> {}", asset.name, asset.href);
    let digest = asset
        .checksum
        .as_ref()
        .expect("swisstopo publishes checksums");
    assert_eq!(digest.algo, "sha2-256");
    assert_eq!(digest.hex.len(), 64);
    assert!(item.id.starts_with("swisstlm3d_"));
}

#[tokio::test]
#[ignore = "requires network"]
async fn locates_the_geopackage_inside_the_real_archive() {
    use s2g_core::http::Http;
    let http = ReqwestHttp::new().unwrap();
    let stac = Stac::new(&http);
    let item = stac.latest(TLM3D).await.unwrap();
    let asset = item.asset_ending(".gpkg.zip").unwrap();

    let head = http.head(&asset.href).await.unwrap();
    let total = head.len.expect("content-length");
    let member = s2g_core::zip::first_member(&http, &asset.href, total)
        .await
        .unwrap();

    println!(
        "archive {:.2} GB -> member {} {:.2} GB (deflate={})",
        total as f64 / 1e9,
        member.name,
        member.uncompressed_size as f64 / 1e9,
        member.is_deflate()
    );
    assert!(member.name.to_lowercase().ends_with(".gpkg"));
    assert!(
        member.is_deflate(),
        "acquisition depends on streaming inflate"
    );
    assert!(member.uncompressed_size > member.compressed_size);
}
