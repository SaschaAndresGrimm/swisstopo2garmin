//! Acquire one dataset the way the app's `acquire_dataset` command does.
//!
//!   cargo run --release -p s2g-core --example acquire -- ch.astra.veloland
//!
//! Exists so the acquisition path can be exercised without the GUI, in particular the
//! multi-file shapefile archives, which take a different route from the single huge
//! GeoPackage the streaming downloader was built for.

use s2g_core::cache::Cache;
use s2g_core::download::{download, download_zip_all, download_zip_member_inflated, Cancel};
use s2g_core::http::ReqwestHttp;
use s2g_core::stac::Stac;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let collection = std::env::args()
        .nth(1)
        .ok_or("usage: acquire <stac collection id>")?;

    let http = ReqwestHttp::new()?;
    let stac = Stac::new(&http);
    let item = stac.latest(&collection).await?;
    let (asset, kind) = item.primary_asset()?;
    let (asset, kind) = (asset.clone(), kind);
    let stream_inflate = collection == s2g_core::stac::TLM3D;

    println!("collection   : {collection}");
    println!(
        "release      : {}  ({})",
        item.id,
        item.datetime.as_deref().unwrap_or("no date")
    );
    println!("asset        : {}", asset.name);
    println!("packaging    : {kind:?}");

    let cache = Cache::new(Cache::default_root());
    let dir = cache.ensure_dir(&collection, &item.id).await?;
    let cancel = Cancel::new();
    let started = std::time::Instant::now();
    let mut last = std::time::Instant::now();
    let mut on_progress = |p: s2g_core::download::Progress| {
        if last.elapsed().as_millis() < 500 {
            return;
        }
        last = std::time::Instant::now();
        match p.total {
            Some(t) => println!("  {:>5.1}%  {} / {} B", p.read as f64 / t as f64 * 100.0, p.read, t),
            None => println!("  {} B", p.read),
        }
    };

    let written: std::path::PathBuf;
    if !kind.is_archive() {
        let dest = dir.join(&asset.name);
        let path = download(
            &http,
            &asset.href,
            &dest,
            asset.checksum.as_ref(),
            &cancel,
            &mut on_progress,
        )
        .await?;
        println!(
            "\nwrote {} ({} B)",
            path.display(),
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
        );
        written = path;
    } else if !stream_inflate {
        let files = download_zip_all(
            &http,
            &asset.href,
            &dir,
            asset.checksum.as_ref(),
            &cancel,
            &mut on_progress,
        )
        .await?;
        println!("\nextracted {} files into {}", files.len(), dir.display());
        written = files.first().cloned().unwrap_or_else(|| dir.clone());
        for f in files.iter().take(20) {
            let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            println!("  {:>12} B  {}", size, f.file_name().unwrap().to_string_lossy());
        }
    } else {
        let dest = dir.join(format!("{}.gpkg", item.id));
        let (path, member) = download_zip_member_inflated(
            &http,
            &asset.href,
            &dest,
            asset.checksum.as_ref(),
            &cancel,
            &mut on_progress,
        )
        .await?;
        println!(
            "\nwrote {} ({} B, member {})",
            path.display(),
            member.uncompressed_size,
            member.name
        );
        written = path;
    }
    // Record where the bytes came from, exactly as the app's acquire_dataset does.
    // Without this the cache looks complete but a build manifest cannot say which
    // release a map came from.
    let prov = s2g_core::cache::Provenance {
        collection: collection.clone(),
        item: item.id.clone(),
        datetime: item.datetime.clone(),
        asset: asset.name.clone(),
        href: asset.href.clone(),
        checksum: asset.checksum.clone(),
        file: written
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default(),
        bytes: std::fs::metadata(&written).map(|m| m.len()).unwrap_or(0),
        fetched_at: {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("@{secs}")
        },
        inflated: stream_inflate,
    };
    cache.write_provenance(&prov).await?;
    println!("provenance   : recorded for {}/{}", prov.collection, prov.item);

    println!("elapsed      : {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}
