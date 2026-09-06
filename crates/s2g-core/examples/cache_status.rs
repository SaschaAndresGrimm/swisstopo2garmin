//! Print what the app's Data screen would show for the real cache.
//!   cargo run -p s2g-core --example cache_status
use s2g_core::cache::{available_bytes, Cache};

#[tokio::main]
async fn main() {
    let c = Cache::new(Cache::default_root());
    println!("root : {}", c.root().display());
    println!("free : {:?} bytes", available_bytes(c.root()));
    match c.list().await {
        Ok(entries) if entries.is_empty() => println!("(no datasets cached)"),
        Ok(entries) => {
            for e in &entries {
                println!(
                    "\n{}/{}\n  file       {}\n  bytes      {} ({:.2} GB)\n  provenance {}",
                    e.collection,
                    e.item,
                    e.path.file_name().unwrap_or_default().to_string_lossy(),
                    e.bytes,
                    e.bytes as f64 / 1e9,
                    match &e.provenance {
                        Some(p) =>
                            format!("ok (inflated={}, fetched {})", p.inflated, p.fetched_at),
                        None => "MISSING OR UNREADABLE".into(),
                    }
                );
            }
            println!(
                "\ntotal {:.2} GB",
                c.total_bytes().await.unwrap_or(0) as f64 / 1e9
            );
        }
        Err(e) => println!("error: {e}"),
    }
}
