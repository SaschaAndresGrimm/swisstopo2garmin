//! Content-addressed dataset cache (SPEC.md FR-10..FR-13, FR-D3/D4).
//!
//! Layout, under the cache root:
//!   <collection>/<item id>/<file>            the dataset itself
//!   <collection>/<item id>/provenance.json   what it is and where it came from
//!
//! A dataset only becomes visible after an atomic rename, so a partial entry is never
//! usable. Provenance makes a build reproducible from its manifest (FR-71).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::stac::Digest;

/// Accepts a checksum written either as a `Digest` object or as a bare multihash
/// string, which is what the Milestone 0 Python spike wrote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ChecksumField {
    Digest(Digest),
    Multihash(String),
}

fn de_checksum<'de, D>(d: D) -> std::result::Result<Option<Digest>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<ChecksumField>::deserialize(d)? {
        None => None,
        Some(ChecksumField::Digest(x)) => Some(x),
        Some(ChecksumField::Multihash(s)) => Digest::from_multihash(&s).ok(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub collection: String,
    pub item: String,
    #[serde(default)]
    pub datetime: Option<String>,
    #[serde(default)]
    pub asset: String,
    #[serde(default)]
    pub href: String,
    // `checksum_multihash` is the Milestone 0 spike's spelling.
    #[serde(
        default,
        alias = "checksum_multihash",
        deserialize_with = "de_checksum"
    )]
    pub checksum: Option<Digest>,
    /// Name of the file on disk (the zip member name when inflated).
    // `member` is the Milestone 0 spike's spelling.
    #[serde(default, alias = "member")]
    pub file: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub fetched_at: String,
    /// True when the archive was inflated during download rather than stored.
    #[serde(default)]
    pub inflated: bool,
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub collection: String,
    pub item: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub provenance: Option<Provenance>,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default location: the OS app-data directory, overridable with `S2G_CACHE`.
    /// The data root in force: `S2G_CACHE`, else the saved setting, else the
    /// platform default. Resolved in one place so a build and the Data screen cannot
    /// disagree about where the data is (see [`crate::settings`]).
    pub fn default_root() -> PathBuf {
        crate::settings::data_root()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn item_dir(&self, collection: &str, item: &str) -> PathBuf {
        self.root.join(collection).join(item)
    }

    /// Where the last known STAC answer for each collection is kept, so release
    /// information survives the API being unreachable (SPEC.md §12).
    ///
    /// A dot-directory, so [`Cache::list`] does not report it as a dataset.
    pub fn catalog_dir(&self) -> PathBuf {
        self.root.join(".catalog")
    }

    pub async fn ensure_dir(&self, collection: &str, item: &str) -> Result<PathBuf> {
        let dir = self.item_dir(collection, item);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| Error::io(&dir, e))?;
        Ok(dir)
    }

    pub async fn write_provenance(&self, p: &Provenance) -> Result<()> {
        let dir = self.item_dir(&p.collection, &p.item);
        let path = dir.join("provenance.json");
        let json = serde_json::to_vec_pretty(p)?;
        // temp + rename so a torn write never leaves unreadable provenance
        let tmp = path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &json)
            .await
            .map_err(|e| Error::io(&tmp, e))?;
        tokio::fs::rename(&tmp, &path)
            .await
            .map_err(|e| Error::io(&path, e))?;
        Ok(())
    }

    pub async fn read_provenance(&self, collection: &str, item: &str) -> Option<Provenance> {
        let path = self.item_dir(collection, item).join("provenance.json");
        let bytes = tokio::fs::read(&path).await.ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Every cached dataset file, with its provenance when present.
    pub async fn list(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        let mut collections = match tokio::fs::read_dir(&self.root).await {
            Ok(rd) => rd,
            Err(_) => return Ok(out), // no cache yet is not an error
        };
        while let Some(c) = collections
            .next_entry()
            .await
            .map_err(|e| Error::io(&self.root, e))?
        {
            if !c.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let collection = c.file_name().to_string_lossy().to_string();
            // Skip internal directories -- notably `.quarantine`, which lives under the
            // cache root. Without this, quarantined data is listed as a usable dataset,
            // defeating the point of quarantining it.
            if collection.starts_with('.') {
                continue;
            }
            let mut items = match tokio::fs::read_dir(c.path()).await {
                Ok(rd) => rd,
                Err(_) => continue,
            };
            while let Some(i) = items
                .next_entry()
                .await
                .map_err(|e| Error::io(c.path(), e))?
            {
                if !i.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let item = i.file_name().to_string_lossy().to_string();
                let provenance = self.read_provenance(&collection, &item).await;
                let mut files = match tokio::fs::read_dir(i.path()).await {
                    Ok(rd) => rd,
                    Err(_) => continue,
                };
                while let Some(f) = files
                    .next_entry()
                    .await
                    .map_err(|e| Error::io(i.path(), e))?
                {
                    let name = f.file_name().to_string_lossy().to_string();
                    if !is_dataset_file(&name) {
                        continue;
                    }
                    let bytes = f.metadata().await.map(|m| m.len()).unwrap_or(0);
                    out.push(Entry {
                        collection: collection.clone(),
                        item: item.clone(),
                        path: f.path(),
                        bytes,
                        provenance: provenance.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| (&a.collection, &a.item).cmp(&(&b.collection, &b.item)));
        Ok(out)
    }

    /// Quarantine whatever dataset a file belongs to, given a path inside the cache.
    ///
    /// The corruption is discovered by whoever tries to read the file — the build
    /// opening a GeoPackage, say — and that code has a path, not a
    /// `(collection, item)` pair. Deriving one from the other here is what makes the
    /// quarantine mechanism reachable from where corruption is actually noticed;
    /// before this it existed and was never called (SPEC.md §12).
    ///
    /// Returns `None` when the path is not inside this cache, which is not an error:
    /// a user pointing the app at a GeoPackage elsewhere on disk is entitled to do so,
    /// and moving their file would be worse than leaving it.
    pub async fn quarantine_containing(&self, path: &Path) -> Result<Option<PathBuf>> {
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return Ok(None);
        };
        let mut parts = rel.components();
        let (Some(collection), Some(item)) = (parts.next(), parts.next()) else {
            return Ok(None);
        };
        let (collection, item) = (
            collection.as_os_str().to_string_lossy().to_string(),
            item.as_os_str().to_string_lossy().to_string(),
        );
        // A loose file directly under the root belongs to no dataset.
        if !self.item_dir(&collection, &item).is_dir() {
            return Ok(None);
        }
        Ok(Some(self.quarantine(&collection, &item).await?))
    }

    pub async fn total_bytes(&self) -> Result<u64> {
        Ok(self.list().await?.iter().map(|e| e.bytes).sum())
    }

    /// Move a corrupt entry aside instead of deleting it, so it can be inspected and
    /// never silently reused (FR: corrupt cache detected -> quarantine).
    pub async fn quarantine(&self, collection: &str, item: &str) -> Result<PathBuf> {
        let src = self.item_dir(collection, item);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dst = self
            .root
            .join(".quarantine")
            .join(format!("{collection}__{item}__{stamp}"));
        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::io(parent, e))?;
        }
        tokio::fs::rename(&src, &dst)
            .await
            .map_err(|e| Error::io(&src, e))?;
        Ok(dst)
    }

    /// Delete orphaned `.part` files across the cache and report what was reclaimed.
    ///
    /// [`crate::download`] cleans up its own partial file on every in-process exit path,
    /// but a kill -9 or a power loss cannot be caught from inside. Since an inflating
    /// download cannot be resumed anyway (raw DEFLATE state is not serialisable), a
    /// leftover `.part` is pure waste — a killed 4.8 GB acquisition strands gigabytes.
    /// Call this at startup (SPEC.md §12, crash recovery).
    pub async fn sweep_partials(&self) -> Result<(u64, Vec<PathBuf>)> {
        let mut freed = 0u64;
        let mut removed = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let mut rd = match tokio::fs::read_dir(&dir).await {
                Ok(rd) => rd,
                Err(_) => continue,
            };
            while let Some(e) = rd.next_entry().await.map_err(|er| Error::io(&dir, er))? {
                let path = e.path();
                if e.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(path);
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".part") || name.ends_with(".json.tmp") {
                    let bytes = e.metadata().await.map(|m| m.len()).unwrap_or(0);
                    if tokio::fs::remove_file(&path).await.is_ok() {
                        freed += bytes;
                        removed.push(path);
                    }
                }
            }
        }
        Ok((freed, removed))
    }

    pub async fn remove(&self, collection: &str, item: &str) -> Result<()> {
        let dir = self.item_dir(collection, item);
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(|e| Error::io(&dir, e))
    }
}

/// Free space at `path`, for the prechecks in FR-12 and the disk-space error case.
pub fn available_bytes(path: &Path) -> Option<u64> {
    // Walk up to the nearest existing ancestor: the target dir may not exist yet.
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        probe = probe.parent()?.to_path_buf();
    }
    fs_free_space(&probe)
}

/// Sidecar and in-progress files that live beside a dataset but are not one.
/// Without this, the Milestone 0 spike's `status.json` shows up as a 264-byte dataset.
fn is_dataset_file(name: &str) -> bool {
    const SIDECARS: [&str; 2] = ["provenance.json", "status.json"];
    if name.starts_with('.') || SIDECARS.contains(&name) {
        return false;
    }
    !(name.ends_with(".part") || name.ends_with(".tmp") || name.ends_with(".json.tmp"))
}

/// Free space on the filesystem containing `path`.
///
/// std has no API for this and this crate forbids `unsafe_code`, so `fs4` provides the
/// platform calls (statvfs / GetDiskFreeSpaceEx). It is a thin, maintained wrapper with
/// no transitive weight, which beats shelling out to `df` on every check.
fn fs_free_space(path: &Path) -> Option<u64> {
    fs4::available_space(path).ok()
}

/// Disk a download needs, given the wire size and the inflated size of its member.
///
/// The two differ enormously and the difference is the whole point: swissTLM3D is
/// 4.5 GB over the wire and 10.0 GB on disk, so checking the download size would pass a
/// machine that the download then fills.
///
/// `stream_inflate` distinguishes the two acquisition shapes. A streamed archive is
/// inflated as it arrives, so only the result is ever on disk. Everything else is
/// downloaded whole and then extracted, so both exist at the same moment and the peak
/// is their sum.
///
/// `None` when the server reported nothing: a precheck cannot be invented from no
/// information, and refusing a download because a `HEAD` failed would be worse.
pub fn download_need(
    archive_bytes: Option<u64>,
    member_bytes: Option<u64>,
    stream_inflate: bool,
) -> Option<u64> {
    if stream_inflate {
        return member_bytes.or(archive_bytes);
    }
    match (archive_bytes, member_bytes) {
        (Some(a), Some(m)) => Some(a.saturating_add(m)),
        (a, m) => a.or(m),
    }
}

pub fn precheck_space(path: &Path, need: u64) -> Result<()> {
    if let Some(available) = available_bytes(path) {
        if available < need {
            return Err(Error::InsufficientSpace {
                path: path.to_path_buf(),
                need,
                available,
            });
        }
    }
    Ok(())
}

/// SHA-256 of a file, streamed rather than read whole.
///
/// Used to verify a map after copying it to a device (SPEC.md FR-82). Streamed because
/// the same function should stay usable for a national build, which is gigabytes.
pub fn sha256_of(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Where the space in the data directory has gone.
///
/// [`Cache::list`] only sees `<collection>/<item>/` directories, which is every acquired
/// dataset and nothing else. The elevation tile cache is loose `.tif` files one level
/// down, and build working files are under `builds/`, so both were invisible — and both
/// grow without bound as areas are built. Eighteen calibration builds filled a disk
/// this way, with the UI reporting only the datasets.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Acquired datasets: `<collection>/<item>/…`.
    pub datasets: u64,
    /// Cached swissALTI3D tiles. Re-downloadable, and the fastest-growing part.
    pub elevation: u64,
    /// Build intermediates under `builds/`. Re-derivable.
    pub builds: u64,
    /// Saved recipes. Tiny, but not re-derivable, so never offered for deletion.
    pub recipes: u64,
    /// Quarantined corrupt downloads, kept for inspection.
    pub quarantine: u64,
    /// Anything else, including stranded `.part` files.
    pub other: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.datasets + self.elevation + self.builds + self.recipes + self.quarantine + self.other
    }
}

/// Total bytes of a directory tree, following no symlinks.
fn tree_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    total += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
                _ => {}
            }
        }
    }
    total
}

impl Cache {
    /// Account for every byte under the root, bucketed by what it is.
    ///
    /// Synchronous and walking the whole tree: it is called when a screen opens, not in
    /// a build loop, and being exhaustive is the entire point — a figure that quietly
    /// omits the largest directory is worse than no figure.
    pub fn usage(&self) -> Usage {
        let mut u = Usage::default();
        let Ok(rd) = std::fs::read_dir(&self.root) else {
            return u;
        };
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);

            if !is_dir {
                // Loose files at the root: the calibration log, stranded `.part` files.
                u.other += e.metadata().map(|m| m.len()).unwrap_or(0);
                continue;
            }
            match name.as_str() {
                crate::stac::ALTI3D | crate::stac::ALTIREGIO => u.elevation += tree_bytes(&path),
                "builds" => u.builds += tree_bytes(&path),
                "recipes" => u.recipes += tree_bytes(&path),
                ".quarantine" => u.quarantine += tree_bytes(&path),
                // The spike-era flat directories hold real datasets.
                "winter" | "routes" => u.datasets += tree_bytes(&path),
                _ if name.starts_with('.') => u.other += tree_bytes(&path),
                _ => u.datasets += tree_bytes(&path),
            }
        }
        u
    }

    /// Delete the cached elevation tiles. Returns the bytes freed.
    ///
    /// Safe because they are re-downloadable, and worth offering because they are the
    /// part that grows: every new area fetches its own 1 km tiles at about 1.2 MB each.
    pub fn clear_elevation(&self) -> Result<u64> {
        let mut freed = 0u64;
        for dir in [
            self.root.join(crate::stac::ALTI3D),
            self.root.join(crate::stac::ALTIREGIO),
        ] {
            if !dir.exists() {
                continue;
            }
            freed += tree_bytes(&dir);
            std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(freed)
    }

    /// Delete build working files. Returns the bytes freed.
    ///
    /// Finished maps are copied to the device or exported, so what is under `builds/`
    /// is intermediates: region PBFs, split tiles, DEM data.
    pub fn clear_builds(&self) -> Result<u64> {
        let dir = self.root.join("builds");
        if !dir.exists() {
            return Ok(0);
        }
        let freed = tree_bytes(&dir);
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        Ok(freed)
    }
}
