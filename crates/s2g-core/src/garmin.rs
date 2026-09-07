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
    ///
    /// The relative layout is tried **first**, and `vendor/toolchain.env` only as a
    /// fallback. That file is written by `fetch_tools.py` with absolute paths into the
    /// developer's checkout, so a packaged app that trusted it looked for the compiler
    /// inside a directory that exists on exactly one machine — and reported the
    /// toolchain missing everywhere else, with a suggestion to run a script that is not
    /// shipped.
    pub fn discover(root: &Path) -> Result<Self> {
        if let Some(tc) = Self::from_layout(root) {
            return Ok(tc);
        }
        Self::from_env_file(&root.join("vendor").join("toolchain.env"))
    }

    /// The vendored tools as they sit under `<root>/vendor`, by shape rather than by
    /// recorded path. Version directories (`mkgmap-r4924`) are matched by prefix so a
    /// tool update needs no code change.
    fn from_layout(root: &Path) -> Option<Self> {
        let vendor = root.join("vendor");
        let jar = |prefix: &str, name: &str| -> Option<PathBuf> {
            let direct = vendor.join(name);
            if direct.is_file() {
                return Some(direct);
            }
            std::fs::read_dir(&vendor).ok()?.flatten().find_map(|e| {
                let p = e.path();
                let matches = p
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with(prefix))
                    .unwrap_or(false);
                (matches && p.join(name).is_file()).then(|| p.join(name))
            })
        };
        // The JRE is bundled for users; a JDK is what a development checkout has, and
        // either runs the tools.
        let java = ["jre", "jdk"].iter().find_map(|image| {
            let base = vendor.join(image);
            [
                base.join("bin").join("java"),
                base.join("Contents").join("Home").join("bin").join("java"),
                base.join("bin").join("java.exe"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        })?;

        let tc = Self {
            java,
            splitter_jar: jar("splitter", "splitter.jar")?,
            mkgmap_jar: jar("mkgmap", "mkgmap.jar")?,
        };
        tc.check().ok()?;
        Some(tc)
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

    /// The java runtime's own version string.
    pub fn java_version(&self) -> Option<String> {
        let out = Command::new(&self.java).arg("-version").output().ok()?;
        // java prints its version on stderr.
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        text.lines().next().map(|l| l.trim().to_string())
    }

    /// splitter reports its version in the first line of its help output.
    pub fn splitter_version(&self) -> Option<String> {
        let out = Command::new(&self.java)
            .arg("-jar")
            .arg(&self.splitter_jar)
            .arg("--version")
            .output()
            .ok()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        text.lines()
            .find(|l| l.to_lowercase().contains("splitter"))
            .map(|l| l.trim().to_string())
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
        // The device's map manager lists maps by their **family name**, so that is
        // where the recipe's name goes. It used to be the constant
        // "swisstopo2garmin", which would have listed every map identically -- and in
        // fact listed none of them, because pass 2 never passed the names at all and
        // the device showed mkgmap's default "OSM street map" for all of them.
        //
        // The series name carries the product line instead, so the maps still group
        // together, and the description keeps the attribution FR-L1 requires while also
        // naming the map -- whichever field a given device chooses to show, it shows
        // something true and useful.
        let name = clamp_name(name);
        Self {
            family_id,
            product_id: 1,
            map_base,
            family_name: name.clone(),
            series_name: "swisstopo2garmin".into(),
            description: description_for(&name),
        }
    }

    /// The longest description the IMG header accepts.
    ///
    /// **Measured**, from mkgmap's own refusal: `IllegalArgumentException: Description
    /// is too long (max 50)`, thrown from `ImgHeader.setDescription`. It is a
    /// fixed-width field in the header, not a mkgmap policy, so there is no getting
    /// round it.
    pub const MAX_DESCRIPTION_CHARS: usize = 50;

    /// The longest map name that fits the field a device displays.
    ///
    /// **The field is `--description`**, established from the Edge 840 photograph: it
    /// showed "OSM street map", and that string is mkgmap's default for `--description`
    /// and for nothing else (found in `CommandArgsReader.class`, beside the option name).
    /// So the description is what a device lists, and it is capped at
    /// [`MapIdentity::MAX_DESCRIPTION_CHARS`] by the header.
    ///
    /// Forty leaves room inside that cap, and a longer name is trimmed here on a word
    /// boundary rather than cut mid-word by the format.
    ///
    /// One thing about the header is worth writing down, because it looks alarming and
    /// is not: the description is stored in **two chunks**, 20 bytes at offset 0x49 and
    /// 30 more at 0x65, space-padded. So `strings` on a finished map shows a suspicious
    /// 20-character run -- "Grindelwald ski tour" -- and the rest of the name sits a few
    /// bytes later, "ing, wrist". Nothing is truncated at 20; 20 + 30 is exactly the 50
    /// the header allows.
    pub const MAX_NAME_CHARS: usize = 40;

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

/// Wrap a command so it runs at below-normal priority (SPEC.md FR-74).
///
/// Through `nice` rather than `setpriority`, because the workspace forbids unsafe code
/// and a whole dependency for one syscall is not worth it. If `nice` is not there the
/// command runs at normal priority: quietly slower is better than not building at all.
///
/// Windows is untouched. Lowering priority there needs a job object, and pretending
/// otherwise would be worse than the honest gap.
fn at_low_priority(cmd: Command) -> Command {
    #[cfg(unix)]
    {
        let nice = ["/usr/bin/nice", "/bin/nice"]
            .into_iter()
            .find(|p| Path::new(p).is_file());
        if let Some(nice) = nice {
            let mut wrapped = Command::new(nice);
            wrapped.arg("-n").arg("10").arg(cmd.get_program());
            for a in cmd.get_args() {
                wrapped.arg(a);
            }
            if let Some(dir) = cmd.get_current_dir() {
                wrapped.current_dir(dir);
            }
            return wrapped;
        }
    }
    cmd
}

/// The command line as it was actually run, for a failure report.
///
/// SPEC.md §12 requires a Java non-zero exit to show the command line, not only the
/// stderr tail: mkgmap takes thirty-odd arguments and its complaint is frequently about
/// one of them. Written out here rather than reconstructed from the recipe, so what the
/// report shows is what ran — including the `nice` wrapper and the style paths.
///
/// Arguments containing spaces are quoted so the line can be pasted into a shell, which
/// is the first thing anybody investigating one of these does.
fn describe(cmd: &Command) -> String {
    let mut out = quote(&cmd.get_program().to_string_lossy());
    for a in cmd.get_args() {
        out.push(' ');
        out.push_str(&quote(&a.to_string_lossy()));
    }
    out
}

fn quote(s: &str) -> String {
    if s.contains(' ') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
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
fn run(cmd: Command, what: &str, cancel: &Cancel) -> Result<String> {
    use std::io::Read;

    let mut cmd = at_low_priority(cmd);
    let command_line = describe(&cmd);
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
        match child
            .try_wait()
            .map_err(|e| Error::io(Path::new(what), e))?
        {
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
        return Err(Error::Zip(format!(
            "{what} failed (exit {}):\n{command_line}\n{tail}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into())
        )));
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
        return Err(Error::Zip(format!(
            "{what} reported errors:\n{command_line}\n{tail}"
        )));
    }
    Ok(log)
}

/// A map name that fits the header, ending on a word where it can.
///
/// ASCII-folded as well as clamped: the header is written in a Garmin code page, and a
/// name with an umlaut in it is one more thing that can render as a question mark in a
/// list the user cannot correct. `Zürich` is worth showing as `Zurich` rather than
/// risking `Z?rich`.
fn clamp_name(name: &str) -> String {
    let folded: String = name
        .chars()
        .map(|c| match c {
            'ä' | 'à' | 'á' | 'â' => 'a',
            'ö' | 'ò' | 'ó' | 'ô' => 'o',
            'ü' | 'ù' | 'ú' | 'û' => 'u',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'ï' | 'î' | 'í' | 'ì' => 'i',
            'Ä' => 'A',
            'Ö' => 'O',
            'Ü' => 'U',
            'ç' => 'c',
            other => other,
        })
        .collect();
    let trimmed = folded.trim();
    if trimmed.is_empty() {
        return "swisstopo2garmin".into();
    }
    if trimmed.chars().count() <= MapIdentity::MAX_NAME_CHARS {
        return trimmed.to_string();
    }
    let cut: String = trimmed
        .chars()
        .take(MapIdentity::MAX_NAME_CHARS)
        .collect::<String>();
    // Prefer a word boundary, but only if one is reasonably close to the end --
    // otherwise a single long word would be cut to almost nothing.
    let cut = match cut.rfind(' ') {
        Some(i) if i >= MapIdentity::MAX_NAME_CHARS / 2 => cut[..i].to_string(),
        _ => cut,
    };
    // Drop punctuation the cut left dangling. "Grindelwald hiking (" is what naming a
    // map "Grindelwald hiking (wrist)" actually produced.
    cut.trim_end()
        .trim_end_matches(['(', '[', '{', '-', ',', ':', ';', '/'])
        .trim_end()
        .to_string()
}

/// The attribution embedded in every map (FR-L1).
///
/// Carried by `--copyright-message`, which is the field Garmin has for exactly this,
/// rather than appended to the description. The description is what a device *lists*, so
/// putting the copyright there made every row in the map manager read "Grindelwald ski
/// touring (c) swisstopo" -- and at 27 characters of suffix against a 50-character
/// field, it also left too little room for the name.
pub const COPYRIGHT: &str = "swissTLM3D (c) swisstopo";

/// The map's description: the name, and nothing else.
///
/// This is the string a device shows in its map list, so it is the name and only the
/// name. Clamped to the header's own limit, which mkgmap enforces by refusing the build.
fn description_for(name: &str) -> String {
    name.chars()
        .take(MapIdentity::MAX_DESCRIPTION_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The `--max-nodes` a build starts from.
///
/// Below splitter's own default of 1,600,000 (splitter r654 `--help`), because smaller
/// tiles draw and pan faster on the wrist devices, which is where responsiveness is
/// scarcest.
pub const START_MAX_NODES: u32 = 700_000;

/// The most this project will ask splitter for.
///
/// Splitter's documented default, and therefore the largest value its author treats as
/// ordinary. Going beyond it to satisfy a tile budget would be trading a limit we know
/// for one we would be guessing at.
pub const CEILING_MAX_NODES: u32 = 1_600_000;

/// The next `--max-nodes` to try when a split produced more tiles than the device
/// accepts, or `None` when there is nothing left to try (SPEC.md §12, "Tile count
/// exceeds device limit — auto-retune `--max-nodes`, else split into map sets").
///
/// Raising it packs more nodes into each tile and so produces fewer of them. Doubling
/// rather than stepping, because each attempt is a full splitter run: a build that
/// needs three minutes per attempt cannot afford ten attempts to creep up on a value.
///
/// Separated from the retry loop so the policy can be tested without running splitter.
pub fn retune_max_nodes(current: u32, tiles: usize, max_tiles: usize) -> Option<u32> {
    if tiles <= max_tiles || current >= CEILING_MAX_NODES {
        return None;
    }
    Some((current.saturating_mul(2)).min(CEILING_MAX_NODES))
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
        // Mountain refuges and remote inns are building *footprints*, so a rule in the
        // points style would never fire on them: that file only sees nodes. This makes
        // mkgmap generate a point per area first, which the points rules then match.
        // Only areas a points rule actually matches become POIs.
        .arg("--add-pois-to-areas")
        .arg(format!("--family-id={}", id.family_id))
        .arg(format!("--product-id={}", id.product_id))
        .arg(format!("--family-name={}", id.family_name))
        .arg(format!("--series-name={}", id.series_name))
        .arg(format!("--description={}", id.description))
        // FR-L1: the attribution travels inside the file, in the field Garmin has for
        // it, rather than inside the name a device lists.
        .arg(format!("--copyright-message={COPYRIGHT}"))
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
        .arg(format!("--product-id={}", id.product_id))
        // Nothing carries over from pass 1. Without these three, mkgmap writes its own
        // defaults into the gmapsupp's map-set block -- and that block is what the
        // device's map manager lists, so every map this project has ever produced showed
        // up on an Edge 840 as "OSM street map". Found on hardware; the `.img` files
        // contain the strings "OSM map", "OSM map set" and "OSM street map" and do not
        // contain ours.
        .arg(format!("--family-name={}", id.family_name))
        .arg(format!("--series-name={}", id.series_name))
        .arg(format!("--description={}", id.description))
        // The gmapsupp's map-set block has its own name, and its own default of "OSM
        // map set". The option is undocumented -- it appears in neither
        // `mkgmap --help=options` nor the bundled help, and was found as the string
        // `mapset-name` inside `GmapsuppBuilder.class`, beside the default it writes.
        // mkgmap validates option names against its own documentation and so rejects it
        // outright; the `x-` prefix is its escape hatch for undocumented options.
        //
        // Worth the reach: the alternative is shipping maps that name somebody else's
        // project in the device's map manager.
        .arg(format!("--x-mapset-name={}", id.family_name))
        .arg(format!("--copyright-message={COPYRIGHT}"));
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

    /// Below-normal priority must not change what a command does.
    #[test]
    fn low_priority_preserves_the_command_and_its_arguments() {
        let mut original = Command::new("sh");
        original.arg("-c").arg("echo hello");
        let log = run(original, "echo", &Cancel::new()).unwrap();
        assert!(log.contains("hello"), "{log}");

        // And through the wrapper directly, so the argument order is checked even where
        // `nice` is absent.
        let mut c = Command::new("sh");
        c.arg("-c").arg("echo wrapped");
        let wrapped = at_low_priority(c);
        let args: Vec<String> = wrapped
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.iter().any(|a| a == "echo wrapped"), "{args:?}");
        assert!(args.iter().any(|a| a == "-c"), "{args:?}");
    }

    /// A shell command, since `Command` builders borrow rather than move.
    fn cmd(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.arg("-c").arg(script);
        c
    }

    /// A packaged app has no `toolchain.env` worth trusting: `fetch_tools.py` writes it
    /// with absolute paths into the developer's checkout. Discovery must work from the
    /// layout alone (NFR-7).
    #[test]
    fn the_toolchain_is_found_from_the_layout_without_the_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let vendor = root.join("vendor");
        std::fs::create_dir_all(vendor.join("mkgmap-r4924")).unwrap();
        std::fs::create_dir_all(vendor.join("splitter-r654")).unwrap();
        std::fs::create_dir_all(vendor.join("jre").join("bin")).unwrap();
        std::fs::write(vendor.join("mkgmap-r4924").join("mkgmap.jar"), b"jar").unwrap();
        std::fs::write(vendor.join("splitter-r654").join("splitter.jar"), b"jar").unwrap();
        std::fs::write(vendor.join("jre").join("bin").join("java"), b"#!/bin/sh\n").unwrap();

        let tc = Toolchain::discover(root).expect("the layout alone should be enough");
        assert!(
            tc.mkgmap_jar.ends_with("mkgmap-r4924/mkgmap.jar"),
            "{:?}",
            tc.mkgmap_jar
        );
        assert!(tc.splitter_jar.ends_with("splitter-r654/splitter.jar"));
        assert!(tc.java.ends_with("jre/bin/java"));
    }

    /// The layout must win over a stale env file, which is exactly what a bundle built
    /// on a machine that had one would contain.
    #[test]
    fn the_layout_wins_over_a_stale_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let vendor = root.join("vendor");
        std::fs::create_dir_all(vendor.join("mkgmap-r4924")).unwrap();
        std::fs::create_dir_all(vendor.join("splitter-r654")).unwrap();
        std::fs::create_dir_all(vendor.join("jre").join("Contents/Home/bin")).unwrap();
        std::fs::write(vendor.join("mkgmap-r4924").join("mkgmap.jar"), b"jar").unwrap();
        std::fs::write(vendor.join("splitter-r654").join("splitter.jar"), b"jar").unwrap();
        std::fs::write(
            vendor.join("jre").join("Contents/Home/bin").join("java"),
            b"#!/bin/sh\n",
        )
        .unwrap();
        std::fs::write(
            vendor.join("toolchain.env"),
            b"MKGMAP_JAR=/Users/somebody-else/mkgmap.jar\n\
              SPLITTER_JAR=/Users/somebody-else/splitter.jar\n\
              JAVA_BIN=/Users/somebody-else/java\n",
        )
        .unwrap();

        let tc = Toolchain::discover(root).expect("the layout is present and valid");
        assert!(
            tc.mkgmap_jar.starts_with(root),
            "took the path from the stale env file: {:?}",
            tc.mkgmap_jar
        );
    }

    /// With neither, the failure must say what to do rather than nothing.
    #[test]
    fn a_root_with_no_toolchain_says_how_to_get_one() {
        let dir = tempfile::tempdir().unwrap();
        let err = Toolchain::discover(dir.path()).unwrap_err();
        assert!(err.to_string().contains("toolchain.env"), "{err}");
    }

    /// The device's map manager lists maps by family name, so it has to be the map's
    /// own name -- and it used to be the constant "swisstopo2garmin" for every map.
    #[test]
    fn a_map_is_named_after_its_recipe_not_after_the_program() {
        let id = MapIdentity::for_recipe("key", "Grindelwald 6 km ski touring");
        assert_eq!(id.family_name, "Grindelwald 6 km ski touring");
        // The product line stays in the series name, so the maps still group together.
        assert_eq!(id.series_name, "swisstopo2garmin");
        // The description is the string a device lists, so it is the name and nothing
        // else. The attribution has its own field.
        assert_eq!(id.description, "Grindelwald 6 km ski touring");
        assert!(
            COPYRIGHT.contains("swisstopo"),
            "FR-L1 needs an attribution to embed"
        );
    }

    /// The IMG header's description field is 50 characters, from mkgmap's own refusal:
    /// `IllegalArgumentException: Description is too long (max 50)`. Exceeding it fails
    /// the build outright, which is how the limit was found -- one of the six test maps
    /// would not compile.
    #[test]
    fn the_description_always_fits_the_header() {
        for name in [
            "Grindelwald hiking",
            "Grindelwald slope classes",
            "Grindelwald ski touring (wrist)",
            "Berner Oberland and the Jungfrau region with contours",
            &"A".repeat(200),
        ] {
            let id = MapIdentity::for_recipe("k", name);
            let n = id.description.chars().count();
            assert!(
                n <= MapIdentity::MAX_DESCRIPTION_CHARS,
                "{:?} is {n} chars",
                id.description
            );
            assert!(!id.description.ends_with(' '));
        }
    }

    /// Two maps of the same area must not be listed identically, or the map manager is
    /// no help in choosing between them.
    #[test]
    fn two_recipes_of_one_area_are_named_differently() {
        let a = MapIdentity::for_recipe("a", "Grindelwald 6 km");
        let b = MapIdentity::for_recipe("b", "Grindelwald 6 km winter");
        assert_ne!(a.family_name, b.family_name);
        assert_ne!(a.family_id, b.family_id);
    }

    #[test]
    fn a_long_name_is_cut_on_a_word() {
        let id =
            MapIdentity::for_recipe("k", "Berner Oberland and the Jungfrau region with contours");
        assert!(
            id.family_name.chars().count() <= MapIdentity::MAX_NAME_CHARS,
            "{:?} is {} chars",
            id.family_name,
            id.family_name.chars().count()
        );
        assert!(!id.family_name.ends_with(' '));
        // Cut on a word, not mid-word.
        assert!(
            "Berner Oberland and the Jungfrau region with contours".starts_with(&id.family_name),
            "{:?}",
            id.family_name
        );
        assert_eq!(id.family_name, "Berner Oberland and the Jungfrau region");
    }

    /// mkgmap cutting "Grindelwald hiking (wrist)" to twenty characters produced
    /// "Grindelwald hiking (", a dangling bracket. Trimming here can do better.
    #[test]
    fn a_cut_never_leaves_dangling_punctuation() {
        for name in [
            "Grindelwald hiking (wrist)",
            "Grindelwald skimo - winter scheme",
            "Grindelwald, the whole valley",
        ] {
            let cut = MapIdentity::for_recipe("k", name).family_name;
            assert!(
                !cut.ends_with(['(', '[', '{', '-', ',', ':', ';', '/', ' ']),
                "{cut:?} ends on punctuation"
            );
        }
    }

    /// One long word has no boundary to cut on, and must not be reduced to nothing.
    #[test]
    fn a_single_long_word_is_cut_rather_than_emptied() {
        let long = "A".repeat(80);
        let id = MapIdentity::for_recipe("k", &long);
        assert_eq!(id.family_name.chars().count(), MapIdentity::MAX_NAME_CHARS);
    }

    /// The header is written in a Garmin code page, so an umlaut is one more thing that
    /// can arrive as a question mark in a list the user cannot correct.
    #[test]
    fn accented_names_are_folded_for_the_header() {
        assert_eq!(
            MapIdentity::for_recipe("k", "Zürich 10 km").family_name,
            "Zurich 10 km"
        );
        assert_eq!(
            MapIdentity::for_recipe("k", "Genève 8 km").family_name,
            "Geneve 8 km"
        );
    }

    /// An empty or blank name must not produce a nameless map.
    #[test]
    fn a_blank_name_falls_back_to_the_program_name() {
        assert_eq!(
            MapIdentity::for_recipe("k", "   ").family_name,
            "swisstopo2garmin"
        );
        assert_eq!(
            MapIdentity::for_recipe("k", "").family_name,
            "swisstopo2garmin"
        );
    }

    /// SPEC.md §12: "Tile count exceeds device limit — auto-retune `--max-nodes`,
    /// else split into map sets."
    ///
    /// Before this the build simply refused, telling the user to pick a smaller area
    /// when denser tiles would have fitted the same map on the same device.
    #[test]
    fn a_tile_count_within_the_budget_is_not_retuned() {
        assert_eq!(retune_max_nodes(START_MAX_NODES, 40, 100), None);
        assert_eq!(retune_max_nodes(START_MAX_NODES, 100, 100), None);
    }

    #[test]
    fn too_many_tiles_doubles_the_node_budget() {
        assert_eq!(retune_max_nodes(700_000, 150, 100), Some(1_400_000));
    }

    /// The next step must not overshoot splitter's own documented default: past that
    /// we would be trading a limit we know for one we would be guessing at.
    #[test]
    fn retuning_stops_at_the_ceiling_rather_than_doubling_past_it() {
        assert_eq!(
            retune_max_nodes(1_400_000, 150, 100),
            Some(CEILING_MAX_NODES)
        );
        // At the ceiling there is nothing left to try, however far over budget it is.
        assert_eq!(retune_max_nodes(CEILING_MAX_NODES, 10_000, 100), None);
    }

    /// The sequence must terminate, or a build would retry forever on an area that
    /// cannot fit. Three attempts at most, and each one is a full splitter run.
    #[test]
    fn the_retune_sequence_terminates() {
        let mut nodes = START_MAX_NODES;
        let mut attempts = 0;
        // A tile count that never improves: the worst case for termination.
        while let Some(next) = retune_max_nodes(nodes, 10_000, 100) {
            nodes = next;
            attempts += 1;
            assert!(attempts < 10, "retuning did not terminate");
        }
        assert_eq!(attempts, 2, "expected 700k -> 1.4M -> 1.6M");
        assert_eq!(nodes, CEILING_MAX_NODES);
    }

    /// A zero budget must not loop or divide by anything.
    #[test]
    fn a_device_that_accepts_no_tiles_is_not_retuned_forever() {
        assert_eq!(retune_max_nodes(CEILING_MAX_NODES, 1, 0), None);
        assert_eq!(retune_max_nodes(START_MAX_NODES, 1, 0), Some(1_400_000));
    }

    /// SPEC.md §12: "Java tool non-zero exit — show stage, command line, stderr tail,
    /// and a plain-language cause where recognized."
    ///
    /// The stage and the tail were reported; the command line was not, and mkgmap takes
    /// thirty-odd arguments whose complaints are frequently about one of them.
    #[test]
    fn a_failure_reports_the_stage_the_exit_code_and_the_command_line() {
        // The real mkgmap output for an oversized area, which is the failure users
        // actually hit and the one `diagnose` has a rule for.
        let mut c = Command::new("sh");
        c.arg("-c").arg(
            "echo 'Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap \
             space' >&2; exit 3",
        );
        let err = run(c, "mkgmap (tiles and overview)", &Cancel::new()).unwrap_err();
        let msg = err.to_string();

        assert!(
            msg.contains("mkgmap (tiles and overview)"),
            "no stage: {msg}"
        );
        assert!(msg.contains("exit 3"), "no exit code: {msg}");
        assert!(msg.contains("sh -c"), "no command line: {msg}");
        assert!(msg.contains("OutOfMemoryError"), "no stderr tail: {msg}");
        // And the plain-language cause, derived from that same string.
        let d = crate::diagnose::diagnose(&err);
        assert!(d.recognised, "{msg}");
        assert!(d.summary.contains("out of memory"), "{}", d.summary);
    }

    /// Arguments with spaces have to survive as one argument, so the line in the report
    /// can be pasted into a shell to reproduce the failure.
    #[test]
    fn the_reported_command_line_is_pasteable() {
        let mut c = Command::new("java");
        c.arg("-jar").arg("/opt/mkgmap.jar");
        c.arg("--style-file=/Users/me/My Maps/style");
        assert_eq!(
            describe(&c),
            "java -jar /opt/mkgmap.jar \"--style-file=/Users/me/My Maps/style\""
        );
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
