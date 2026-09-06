//! Acquire one dataset the way the app's `acquire_dataset` command does.
//!
//!   cargo run --release -p s2g-core --example acquire -- ch.astra.veloland
//!
//! Exists so the acquisition path can be exercised without the GUI, in particular the
//! multi-file shapefile archives, which take a different route from the single huge
//! GeoPackage the streaming downloader was built for.

use s2g_core::cache::Cache;
use s2g_core::download::{download_zip_all, download_zip_member_inflated, Cancel};
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
    let multi_file = item.asset_ending(".gpkg.zip").is_err();
    let asset = item
        .asset_ending(if multi_file { ".shp.zip" } else { ".gpkg.zip" })?
        .clone();

    println!("collection   : {collection}");
    println!(
        "release      : {}  ({})",
        item.id,
        item.datetime.as_deref().unwrap_or("no date")
    );
    println!("asset        : {}", asset.name);
    println!("multi-file   : {multi_file}");

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

    if multi_file {
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
    }
    println!("elapsed      : {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}
