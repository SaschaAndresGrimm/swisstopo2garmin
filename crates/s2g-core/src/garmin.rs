//! Garmin map compilation: `splitter`, `mkgmap`, and IMG verification (SPEC.md §7.5-7.7).
//!
//! Both Java tools are invoked as **child processes**, never embedded. That keeps their
//! GPLv2 licensing cleanly separated and lets a user substitute their own JAR (FR-P11).
//!
//! Three Milestone 0 findings are encoded here as behaviour, because each one produced
//! a map that a device lists but cannot draw:
//!
//! * A single `--gmapsupp` run omits the overview map. The build must be **two passes**
//!   (§4.5).
//! * `--overview-mapnumber` must be set explicitly, or two of our own maps collide
//!   (§4.6).
//! * Labels default to uppercase ASCII without `--lower-case` (§4.10).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::download::Cancel;
use crate::error::{Error, Result};

/// Located toolchain: a Java runtime plus the two JARs.
#[derive(Debug, Clone)]
pub struct Toolchain {
    pub java: PathBuf,
    pub splitter_jar: PathBuf,
    pub mkgmap_jar: PathBuf,
}

impl Toolchain {
    /// Read `vendor/toolchain.env`, written by `vendor/fetch_tools.py`.
    pub fn from_env_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        let get = |key: &str| -> Result<PathBuf> {
            text.lines()
                .find_map(|l| l.strip_prefix(&format!("{key}=")))
                .map(|v| PathBuf::from(v.trim()))
                .ok_or_else(|| Error::NotFound(format!("{key} missing from {}", path.display())))
        };
        let tc = Self {
            java: get("JAVA_BIN")?,
            splitter_jar: get("SPLITTER_JAR")?,
            mkgmap_jar: get("MKGMAP_JAR")?,
        };
        tc.check()?;
        Ok(tc)
    }

    /// Look for the vendored toolchain relative to a repository or install root.
    pub fn discover(root: &Path) -> Result<Self> {
        Self::from_env_file(&root.join("vendor").join("toolchain.env"))
    }

    fn check(&self) -> Result<()> {
        for p in [&self.java, &self.splitter_jar, &self.mkgmap_jar] {
            if !p.exists() {
                return Err(Error::NotFound(format!(
                    "{} is missing; run vendor/fetch_tools.py",
                    p.display()
                )));
            }
        }
        Ok(())
    }

    pub fn mkgmap_version(&self) -> Option<String> {
        let out = Command::new(&self.java)
            .arg("-jar")
            .arg(&self.mkgmap_jar)
            .arg("--version")
            .output()
            .ok()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        text.lines().next().map(|l| l.trim().to_string())
    }
}

/// Identity of a produced map. Family and overview numbers must be unique across all
/// maps installed on a device.
#[derive(Debug, Clone)]
pub struct MapIdentity {
    pub family_id: u16,
    pub product_id: u16,
    /// Base map number; the overview map takes this value and tiles take the next ones.
    pub map_base: u32,
    pub family_name: String,
    pub series_name: String,
    pub description: String,
}

impl MapIdentity {
    /// Deterministic identity from a recipe key, so rebuilding the same map keeps its
    /// identity and two different maps do not collide (SPEC.md §5.1).
    ///
    /// Family id and map numbers are **independent** identifiers in Garmin's model and
    /// have different ranges. Deriving map numbers as `family_id * 10000` overflows the
    /// 8-digit map-number limit that splitter enforces, so they are allocated separately.
    pub fn for_recipe(key: &str, name: &str) -> Self {
        let mut h: u32 = 2166136261;
        for b in key.as_bytes() {
            h ^= *b as u32;
            h = h.wrapping_mul(16777619);
        }
        // Family id is 16-bit; stay clear of the low numbers Garmin's own products use.
        let family_id = 6000 + (h % 55_000) as u16;
        // Map numbers must be at most 99_999_999. Reserve the low four digits for
        // tiles, giving 10_000 tiles per map set — well above any device's limit.
        let map_base = 10_000_000 + (h.rotate_left(13) % 8_900) * 10_000;
        Self {
            family_id,
            product_id: 1,
            map_base,
            family_name: "swisstopo2garmin".into(),
            series_name: name.into(),
            description: "swissTLM3D (c) swisstopo".into(),
        }
    }

    /// Largest map number splitter and mkgmap accept.
    pub const MAX_MAP_NUMBER: u32 = 99_999_999;

    /// Map number of the overview map. Ends in `0000`, which is how the IMG verifier
    /// recognises it.
    pub fn overview_mapnumber(&self) -> u32 {
        self.map_base
    }

    /// Map number of the nth detail tile.
    pub fn tile_mapnumber(&self, n: u32) -> u32 {
        self.map_base + n + 1
    }
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub identity: MapIdentity,
    /// Style directory passed to `--style-file`.
    pub style_dir: PathBuf,
    /// TYP source passed as an input file; mkgmap compiles it.
    pub typ_file: PathBuf,
    /// Directory of `.hgt` tiles for relief shading, if any (FR-CART8).
    pub dem_dir: Option<PathBuf>,
    /// DEM resolution per zoom level. **Must have exactly one entry per level in the
    /// style's `levels` option**, or mkgmap aborts with "More dem-dist values than
    /// levels". The handlebar style has 5 levels and the wrist style 4, so this cannot
    /// be a fixed list — use [`BuildOptions::with_dem`].
    pub dem_dists: Vec<u32>,
    pub max_nodes: u32,
    /// Draw above other enabled maps on the device.
    pub draw_priority: u8,
    pub code_page: u16,
    pub max_heap_mb: u32,
}

/// Number of zoom levels declared by a style's `options` file.
pub fn style_level_count(style_dir: &Path) -> Result<usize> {
    let path = style_dir.join("options");
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("levels") && l.contains('='))
        .ok_or_else(|| Error::NotFound(format!("no `levels` in {}", path.display())))?;
    let rhs = line.split_once('=').map(|(_, r)| r).unwrap_or("");
    let n = rhs.split(',').filter(|p| p.contains(':')).count();
    if n == 0 {
        return Err(Error::Zip(format!(
            "could not parse levels from {}: {line:?}",
            path.display()
        )));
    }
    Ok(n)
}

impl BuildOptions {
    /// Attach DEM data, deriving one resolution per style level.
    ///
    /// Each level is half the resolution of the previous one, starting from the
    /// 1 arc-second spacing mkgmap documents (3314).
    pub fn with_dem(mut self, dem_dir: PathBuf) -> Result<Self> {
        let levels = style_level_count(&self.style_dir)?;
        self.dem_dists = (0..levels).map(|i| 3314u32 << i).collect();
        self.dem_dir = Some(dem_dir);
        Ok(self)
    }

    pub fn new(identity: MapIdentity, style_dir: PathBuf, typ_file: PathBuf) -> Self {
        Self {
            identity,
            style_dir,
            typ_file,
            dem_dir: None,
            // Empty until with_dem() derives one value per style level.
            dem_dists: Vec::new(),
            max_nodes: 700_000,
            draw_priority: 30,
            // 1252 keeps mixed case and Swiss characters; validated on hardware.
            code_page: 1252,
            max_heap_mb: 4096,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub gmapsupp: PathBuf,
    pub tile_imgs: Vec<PathBuf>,
    pub overview_img: Option<PathBuf>,
    pub tile_count: usize,
    pub bytes: u64,
}

/// Run a child process, killing it if the build is cancelled.
///
/// `Command::output` blocks until the child exits, so a build cancelled during
/// splitting or compiling — the two longest stages, minutes on a large area — would
/// keep running to completion and only then notice. The child is polled instead, and
/// killed on cancellation.
///
/// Both pipes are drained on their own threads: mkgmap is talkative, and a child whose
/// stdout pipe fills up blocks forever, which would turn every large build into a hang.
fn run(mut cmd: Command, what: &str, cancel: &Cancel) -> Result<String> {
    use std::io::Read;

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::io(Path::new(what), e))?;

    fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut buf);
            }
            buf
        })
    }
    let out_thread = drain(child.stdout.take());
    let err_thread = drain(child.stderr.take());

    let status = loop {
        match child.try_wait().map_err(|e| Error::io(Path::new(what), e))? {
            Some(status) => break status,
            None => {
                if cancel.is_cancelled() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    };

    let log = format!(
        "{}{}",
        out_thread.join().unwrap_or_default(),
        err_thread.join().unwrap_or_default()
    );
    if !status.success() {
        // A killed child looks like a failure; report it as the cancellation it is.
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let tail: String = log.lines().rev().take(25).collect::<Vec<_>>().join("\n");
        return Err(Error::Zip(format!("{what} failed:\n{tail}")));
    }
    // mkgmap exits 0 even after throwing, so the log has to be inspected.
    if log.contains("Exception:")
        || log.contains("MapFailedException") && !log.contains("Number of MapFailedExceptions: 0")
    {
        let tail: String = log
            .lines()
            .filter(|l| l.contains("Exception") || l.contains("ERROR"))
            .take(10)
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Error::Zip(format!("{what} reported errors:\n{tail}")));
    }
    Ok(log)
}

/// Split an OSM PBF into Garmin map tiles.
pub fn split(
    tc: &Toolchain,
    pbf: &Path,
    out_dir: &Path,
    identity: &MapIdentity,
    max_nodes: u32,
    max_heap_mb: u32,
    cancel: &Cancel,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io(out_dir, e))?;
    let mut cmd = Command::new(&tc.java);
    cmd.arg(format!("-Xmx{max_heap_mb}m"))
        .arg("-jar")
        .arg(&tc.splitter_jar)
        .arg(format!("--output-dir={}", out_dir.display()))
        .arg(format!("--max-nodes={max_nodes}"))
        .arg(format!("--mapid={}", identity.tile_mapnumber(0)))
        .arg(pbf);
    run(cmd, "splitter", cancel)?;

    let mut tiles: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .map_err(|e| Error::io(out_dir, e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".osm.pbf"))
        .collect();
    tiles.sort();
    if tiles.is_empty() {
        return Err(Error::Zip(
            "splitter produced no tiles; the input may be empty".into(),
        ));
    }
    Ok(tiles)
}

/// Compile tiles into a `gmapsupp.img`, in two passes.
///
/// Pass 1 writes the detail tiles plus the overview map; pass 2 combines them. A single
/// `--gmapsupp` run leaves the overview map out, and the device then lists the map but
/// draws nothing (docs/m0-findings.md §4.5).
pub fn compile(
    tc: &Toolchain,
    tiles: &[PathBuf],
    out_dir: &Path,
    opts: &BuildOptions,
    cancel: &Cancel,
) -> Result<BuildOutput> {
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io(out_dir, e))?;
    let id = &opts.identity;

    // ---- pass 1: tiles + overview map ----
    let mut cmd = Command::new(&tc.java);
    cmd.current_dir(out_dir)
        .arg(format!("-Xmx{}m", opts.max_heap_mb))
        .arg("-jar")
        .arg(&tc.mkgmap_jar)
        .arg(format!("--style-file={}", opts.style_dir.display()))
        .arg("--tdbfile")
        .arg("--index")
        .arg(format!("--code-page={}", opts.code_page))
        // Without this, labels are uppercased and transliterated to ASCII.
        .arg("--lower-case")
        .arg(format!("--family-id={}", id.family_id))
        .arg(format!("--product-id={}", id.product_id))
        .arg(format!("--family-name={}", id.family_name))
        .arg(format!("--series-name={}", id.series_name))
        .arg(format!("--description={}", id.description))
        .arg(format!("--draw-priority={}", opts.draw_priority))
        .arg("--overview-mapname=ovm")
        // Explicit, or two of our own maps share mkgmap's default and collide.
        .arg(format!("--overview-mapnumber={}", id.overview_mapnumber()));

    if let Some(dem) = &opts.dem_dir {
        let levels = style_level_count(&opts.style_dir)?;
        if opts.dem_dists.len() != levels {
            return Err(Error::Zip(format!(
                "--dem-dists has {} values but the style at {} declares {levels} levels; \
                 mkgmap requires exactly one per level",
                opts.dem_dists.len(),
                opts.style_dir.display()
            )));
        }
        cmd.arg(format!("--dem={}", dem.display()));
        let dists: Vec<String> = opts.dem_dists.iter().map(|d| d.to_string()).collect();
        cmd.arg(format!("--dem-dists={}", dists.join(",")));
    }
    for t in tiles {
        cmd.arg(t);
    }
    cmd.arg(&opts.typ_file);
    run(cmd, "mkgmap (tiles and overview)", cancel)?;

    let imgs = |dir: &Path, prefix: &str| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.extension().map(|x| x == "img").unwrap_or(false)
                            && p.file_name()
                                .map(|n| n.to_string_lossy().starts_with(prefix))
                                .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    };

    // mkgmap names tile images after the map number, not the family id.
    let family_prefix = (id.map_base / 10_000).to_string();
    let tile_imgs: Vec<PathBuf> = imgs(out_dir, &family_prefix)
        .into_iter()
        .filter(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy() != id.overview_mapnumber().to_string())
                .unwrap_or(true)
        })
        .collect();
    let overview_img = out_dir.join("ovm.img");
    let overview = overview_img.exists().then_some(overview_img.clone());
    if overview.is_none() {
        return Err(Error::Zip(
            "mkgmap did not write an overview map; the result would not draw on a device".into(),
        ));
    }

    // ---- pass 2: combine into gmapsupp ----
    let mut cmd = Command::new(&tc.java);
    cmd.current_dir(out_dir)
        .arg(format!("-Xmx{}m", opts.max_heap_mb))
        .arg("-jar")
        .arg(&tc.mkgmap_jar)
        .arg("--gmapsupp")
        .arg("--index")
        .arg(format!("--family-id={}", id.family_id))
        .arg(format!("--product-id={}", id.product_id));
    for t in &tile_imgs {
        cmd.arg(t);
    }
    cmd.arg(&overview_img);
    // The compiled TYP written by pass 1.
    for e in std::fs::read_dir(out_dir).map_err(|e| Error::io(out_dir, e))? {
        let p = e.map_err(|e| Error::io(out_dir, e))?.path();
        if p.extension().map(|x| x == "typ").unwrap_or(false) {
            cmd.arg(p);
        }
    }
    run(cmd, "mkgmap (gmapsupp)", cancel)?;

    let gmapsupp = out_dir.join("gmapsupp.img");
    if !gmapsupp.exists() {
        return Err(Error::Zip("mkgmap did not write gmapsupp.img".into()));
    }
    let bytes = std::fs::metadata(&gmapsupp)
        .map(|m| m.len())
        .map_err(|e| Error::io(&gmapsupp, e))?;

    Ok(BuildOutput {
        gmapsupp,
        tile_count: tile_imgs.len(),
        tile_imgs,
        overview_img: overview,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shell command, since `Command` builders borrow rather than move.
    fn cmd(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.arg("-c").arg(script);
        c
    }

    /// A cancelled build must stop the child process, not wait for it.
    ///
    /// `Command::output` blocks until the child exits, so before this the Cancel button
    /// did nothing during splitting and compiling — the two stages that take minutes —
    /// and the build ran to completion after the UI said it had stopped.
    #[test]
    fn cancelling_kills_a_running_child_instead_of_waiting_for_it() {
        let cancel = Cancel::new();
        let c = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            c.cancel();
        });

        let started = std::time::Instant::now();
        let err = run(cmd("sleep 30"), "sleep", &cancel).unwrap_err();
        let elapsed = started.elapsed();

        assert!(matches!(err, Error::Cancelled), "got {err:?}");
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "waited {elapsed:?} for a child that should have been killed"
        );
    }

    /// Cancelling before the child is even spawned must still stop the build.
    #[test]
    fn an_already_cancelled_build_does_not_run_to_completion() {
        let cancel = Cancel::new();
        cancel.cancel();
        let started = std::time::Instant::now();
        let err = run(cmd("sleep 30"), "sleep", &cancel).unwrap_err();
        assert!(matches!(err, Error::Cancelled), "got {err:?}");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    /// A child that fails on its own is still reported as a failure, with its output.
    #[test]
    fn a_failing_child_reports_its_own_error_not_a_cancellation() {
        let err = run(cmd("echo boom >&2; exit 3"), "thing", &Cancel::new()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("thing failed"), "{msg}");
        assert!(msg.contains("boom"), "{msg}");
    }

    /// Both pipes are drained on threads: a child that writes more than a pipe buffer
    /// holds would otherwise block forever, hanging every large build.
    #[test]
    fn a_child_that_floods_both_pipes_does_not_deadlock() {
        // 512 KiB on each pipe, well past the 64 KiB typical buffer.
        let log = run(
            cmd("yes stdoutline | head -c 524288; yes errline | head -c 524288 >&2"),
            "flood",
            &Cancel::new(),
        )
        .unwrap();
        assert!(log.len() >= 1_048_576, "captured only {} bytes", log.len());
        assert!(log.contains("stdoutline"));
        assert!(log.contains("errline"));
    }
}
