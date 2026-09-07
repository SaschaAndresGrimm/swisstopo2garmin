//! Recovery from a build that never finished (SPEC.md §12, "App killed mid-build").
//!
//! A build writes tens to hundreds of megabytes into its work directory and spawns Java
//! child processes that outlive their parent. If the app is killed — a crash, a forced
//! quit, a laptop losing power — both are left behind: the files silently consume disk,
//! and an orphaned `mkgmap` keeps a core busy indefinitely.
//!
//! Two things make this detectable without bookkeeping that a `SIGKILL`ed process could
//! not have written:
//!
//! * A **marker file** in the work directory, written before the first stage and removed
//!   when the build ends by any route. Its presence plus a dead owner process is the
//!   definition of an orphan.
//! * The Java children **identify themselves**. Their command lines name our jars and
//!   our work directory, so they can be found by inspection rather than by a pid list we
//!   would have had to write down in advance. That also survives pid reuse, which a
//!   recorded pid does not.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::recipe::Recipe;

/// The file a running build leaves in its work directory.
const MARKER: &str = "in-progress.json";

/// What a running build records about itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub schema_version: u32,
    /// The process that started the build. Liveness of this pid is what separates a
    /// running build from an orphaned one.
    pub pid: u32,
    /// RFC 3339, for the "started 4 hours ago" the user needs to recognise it.
    pub started_at: String,
    /// The whole recipe, so resuming needs nothing from the UI's memory — which is
    /// exactly what a crash destroyed.
    pub recipe: Recipe,
    /// The cached-region key this build would use, so recovery can say whether the
    /// expensive stages survived.
    pub region_key: String,
}

impl Marker {
    pub const SCHEMA_VERSION: u32 = 1;
}

/// A live build's marker, removed when this value is dropped.
///
/// Drop rather than an explicit call, because the failure paths are numerous — error,
/// cancellation, a panic in a stage — and every one of them must clear the marker or the
/// next start would offer to recover a build that finished perfectly well.
pub struct ActiveBuild {
    marker: PathBuf,
}

impl ActiveBuild {
    /// Record that a build is running in `work_dir`.
    pub fn begin(work_dir: &Path, recipe: &Recipe, region_key: &str, now: &str) -> Result<Self> {
        std::fs::create_dir_all(work_dir).map_err(|e| Error::io(work_dir, e))?;
        let marker = work_dir.join(MARKER);
        let m = Marker {
            schema_version: Marker::SCHEMA_VERSION,
            pid: std::process::id(),
            started_at: now.to_string(),
            recipe: recipe.clone(),
            region_key: region_key.to_string(),
        };
        std::fs::write(&marker, serde_json::to_vec_pretty(&m)?)
            .map_err(|e| Error::io(&marker, e))?;
        Ok(Self { marker })
    }

    pub fn marker_path(&self) -> &Path {
        &self.marker
    }
}

impl Drop for ActiveBuild {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.marker);
    }
}

/// A build that was interrupted, and what can be done about it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orphan {
    pub recipe: Recipe,
    pub work_dir: PathBuf,
    pub started_at: String,
    /// Reclaimable by discarding this build's work directory.
    pub bytes: u64,
    /// True when the cached region for this recipe is still present, so a resumed build
    /// skips extract, elevation and contours — about 80 % of the work.
    pub resumable: bool,
    /// Java processes still running for this build.
    pub stray_pids: Vec<u32>,
}

/// How a process list is obtained. A trait so recovery is testable without spawning
/// real Java processes and killing them.
pub trait ProcessProbe {
    /// Whether a pid belongs to a running process, and its command line.
    fn command_line(&self, pid: u32) -> Option<String>;
    /// Every running process whose command line mentions `needle`.
    fn matching(&self, needle: &str) -> Vec<(u32, String)>;
}

/// The real process list, read from the platform's process tool.
pub struct SystemProbe;

impl SystemProbe {
    /// `pid<TAB>command` for every process, or an empty list when the platform tool is
    /// unavailable. Unavailability must not be an error: failing to enumerate processes
    /// means recovery offers less, not that the app cannot start.
    fn all(&self) -> Vec<(u32, String)> {
        #[cfg(unix)]
        let out = std::process::Command::new("ps")
            .args(["-A", "-o", "pid=,command="])
            .output();
        // Windows `tasklist` does not report command lines, so the process class is
        // queried directly. Untested on Windows hardware: if it fails, `all` returns
        // nothing and recovery simply reports no stray processes.
        #[cfg(windows)]
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_Process | ForEach-Object \
                 { \"$($_.ProcessId) $($_.CommandLine)\" }",
            ])
            .output();

        let Ok(out) = out else { return Vec::new() };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let line = line.trim_start();
                let (pid, rest) = line.split_once(char::is_whitespace)?;
                Some((pid.parse().ok()?, rest.trim().to_string()))
            })
            .collect()
    }
}

impl ProcessProbe for SystemProbe {
    fn command_line(&self, pid: u32) -> Option<String> {
        self.all()
            .into_iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, c)| c)
    }

    fn matching(&self, needle: &str) -> Vec<(u32, String)> {
        self.all()
            .into_iter()
            .filter(|(pid, cmd)| *pid != std::process::id() && cmd.contains(needle))
            .collect()
    }
}

/// Find interrupted builds under `<cache_root>/builds`.
///
/// A marker whose owner process is still alive is a build in progress — possibly this
/// one, possibly a second copy of the app — and is deliberately left alone.
pub fn scan(cache_root: &Path, probe: &dyn ProcessProbe) -> Vec<Orphan> {
    let builds = cache_root.join("builds");
    let Ok(entries) = std::fs::read_dir(&builds) else {
        return Vec::new();
    };
    let regions = crate::stage_cache::RegionCache::new(cache_root);

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let work_dir = entry.path();
        let marker_path = work_dir.join(MARKER);
        if !marker_path.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&marker_path) else {
            continue;
        };
        // An unreadable or future-versioned marker is still evidence of an interrupted
        // build; it just cannot be resumed. Reporting the directory is more useful than
        // leaving it to accumulate silently.
        let marker: Option<Marker> = serde_json::from_slice(&bytes).ok();
        let Some(marker) = marker.filter(|m| m.schema_version <= Marker::SCHEMA_VERSION) else {
            out.push(Orphan {
                recipe: Recipe::new("(unreadable build record)", "", unknown_area()),
                work_dir: work_dir.clone(),
                started_at: String::new(),
                bytes: tree_bytes(&work_dir),
                resumable: false,
                stray_pids: Vec::new(),
            });
            continue;
        };

        if is_live(&marker, probe) {
            continue;
        }
        out.push(Orphan {
            resumable: regions.get(&marker.region_key).is_some(),
            stray_pids: stray_children(&work_dir, probe),
            recipe: marker.recipe,
            started_at: marker.started_at,
            bytes: tree_bytes(&work_dir),
            work_dir,
        });
    }
    out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    out
}

/// Whether the process that wrote a marker is still running it.
///
/// A dead pid is the common case. A *live* pid that belongs to something else entirely
/// is the case pid reuse creates, and treating that as a running build would leave the
/// work directory forever — so the command line has to agree.
fn is_live(marker: &Marker, probe: &dyn ProcessProbe) -> bool {
    if marker.pid == std::process::id() {
        return true;
    }
    match probe.command_line(marker.pid) {
        Some(cmd) => cmd.contains(crate::recovery::APP_PROCESS_NAME),
        None => false,
    }
}

/// The substring that identifies this application in a process list. The binary name
/// from `Cargo.toml`, which is what `ps` shows.
pub const APP_PROCESS_NAME: &str = "swisstopo2garmin";

/// Java processes still working on an abandoned build.
///
/// Matched on the work directory, which every `splitter` and `mkgmap` invocation names
/// on its command line. That makes the match specific to *this* build rather than to
/// Java in general — the user may well be running something else on the JVM.
fn stray_children(work_dir: &Path, probe: &dyn ProcessProbe) -> Vec<u32> {
    let needle = work_dir.to_string_lossy().to_string();
    probe
        .matching(&needle)
        .into_iter()
        .filter(|(_, cmd)| cmd.contains("mkgmap") || cmd.contains("splitter"))
        .map(|(pid, _)| pid)
        .collect()
}

/// Ask a stray process to exit, then insist.
///
/// `SIGKILL` after `SIGTERM` because an out-of-memory JVM can stop responding to the
/// polite signal, and the point of this is to free the CPU it is burning.
pub fn terminate(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    Ok(())
}

/// Discard an interrupted build: kill what is still running, then delete its files.
///
/// The cached region is deliberately kept. It is verified-complete by construction —
/// `RegionCache::put` only stores a finished clip — and it is the expensive part.
pub fn discard(orphan: &Orphan) -> Result<u64> {
    for pid in &orphan.stray_pids {
        terminate(*pid)?;
    }
    let bytes = tree_bytes(&orphan.work_dir);
    if orphan.work_dir.exists() {
        std::fs::remove_dir_all(&orphan.work_dir).map_err(|e| Error::io(&orphan.work_dir, e))?;
    }
    Ok(bytes)
}

fn unknown_area() -> crate::recipe::AreaSelection {
    crate::recipe::AreaSelection::BBox {
        min_e: 0.0,
        min_n: 0.0,
        max_e: 0.0,
        max_n: 0.0,
    }
}

fn tree_bytes(dir: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for e in entries.flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => total += tree_bytes(&e.path()),
            Ok(_) => total += e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => {}
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::AreaSelection;
    use std::collections::HashMap;

    /// A process list we control, so the tests never spawn or kill anything real.
    #[derive(Default)]
    struct FakeProbe {
        running: HashMap<u32, String>,
    }

    impl FakeProbe {
        fn with(pid: u32, cmd: &str) -> Self {
            let mut p = Self::default();
            p.running.insert(pid, cmd.to_string());
            p
        }
        fn and(mut self, pid: u32, cmd: &str) -> Self {
            self.running.insert(pid, cmd.to_string());
            self
        }
    }

    impl ProcessProbe for FakeProbe {
        fn command_line(&self, pid: u32) -> Option<String> {
            self.running.get(&pid).cloned()
        }
        fn matching(&self, needle: &str) -> Vec<(u32, String)> {
            self.running
                .iter()
                .filter(|(_, cmd)| cmd.contains(needle))
                .map(|(p, c)| (*p, c.clone()))
                .collect()
        }
    }

    fn recipe(name: &str) -> Recipe {
        Recipe::new(
            name,
            "edge-840",
            AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_190_000.0,
                max_e: 2_610_000.0,
                max_n: 1_200_000.0,
            },
        )
    }

    /// Write a marker as if a build with `pid` were running, without starting one.
    fn plant(cache_root: &Path, dir: &str, pid: u32, key: &str) -> PathBuf {
        let work = cache_root.join("builds").join(dir);
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("region.osm.pbf"), vec![0u8; 4096]).unwrap();
        let m = Marker {
            schema_version: Marker::SCHEMA_VERSION,
            pid,
            started_at: "2026-09-07T10:00:00Z".into(),
            recipe: recipe(dir),
            region_key: key.into(),
        };
        std::fs::write(work.join(MARKER), serde_json::to_vec(&m).unwrap()).unwrap();
        work
    }

    /// The marker must be gone when the build ends, whatever route it ended by.
    /// Otherwise every successful build would be offered for recovery on next start.
    #[test]
    fn a_finished_build_leaves_no_marker() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("builds").join("test");
        let marker = {
            let active = ActiveBuild::begin(&work, &recipe("test"), "abc", "now").unwrap();
            assert!(active.marker_path().is_file());
            active.marker_path().to_path_buf()
        };
        assert!(!marker.exists(), "the marker outlived the build");
    }

    /// A panic mid-build is the case a normal cleanup call would miss.
    #[test]
    fn a_panicking_build_leaves_no_marker() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("builds").join("test");
        let marker = work.join(MARKER);
        let r = std::panic::catch_unwind({
            let work = work.clone();
            move || {
                let _active = ActiveBuild::begin(&work, &recipe("test"), "abc", "now").unwrap();
                panic!("a stage blew up");
            }
        });
        assert!(r.is_err(), "the test needs the panic to happen");
        assert!(!marker.exists(), "an unwound build left its marker behind");
    }

    #[test]
    fn a_build_whose_process_is_gone_is_reported_with_what_it_costs() {
        let dir = tempfile::tempdir().unwrap();
        plant(dir.path(), "grindelwald", 999_001, "nosuchregion");

        let found = scan(dir.path(), &FakeProbe::default());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].recipe.name, "grindelwald");
        assert_eq!(found[0].bytes, 4096 + marker_len(&found[0].work_dir));
        assert!(
            !found[0].resumable,
            "no cached region, so nothing to resume"
        );
        assert!(found[0].stray_pids.is_empty());
    }

    fn marker_len(work_dir: &Path) -> u64 {
        std::fs::metadata(work_dir.join(MARKER)).unwrap().len()
    }

    /// A second copy of the app, mid-build, must not have its files deleted under it.
    #[test]
    fn a_build_that_is_still_running_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        plant(dir.path(), "running", 999_002, "k");
        let probe = FakeProbe::with(
            999_002,
            "/Applications/swisstopo2garmin.app/MacOS/swisstopo2garmin",
        );
        assert!(scan(dir.path(), &probe).is_empty());
    }

    /// Pids are recycled. A live pid that is somebody else's editor is not our build,
    /// and treating it as one would leave the work directory on disk forever.
    #[test]
    fn a_reused_pid_belonging_to_another_program_does_not_protect_a_build() {
        let dir = tempfile::tempdir().unwrap();
        plant(dir.path(), "reused", 999_003, "k");
        let probe = FakeProbe::with(999_003, "/usr/bin/vim notes.txt");
        let found = scan(dir.path(), &probe);
        assert_eq!(
            found.len(),
            1,
            "a recycled pid must not look like a live build"
        );
    }

    /// The whole reason for the region cache: recovery can say the expensive 80 % is
    /// still there, which is the difference between "resume" and "start again".
    #[test]
    fn a_surviving_cached_region_makes_the_build_resumable() {
        let dir = tempfile::tempdir().unwrap();
        let cache = crate::stage_cache::RegionCache::new(dir.path());
        let built = dir.path().join("region.osm.pbf");
        std::fs::write(&built, b"pbf").unwrap();
        cache
            .put(
                "abc123",
                &built,
                &crate::stage_cache::RegionStats {
                    features: 1,
                    nodes: 1,
                    ways: 1,
                    contour_lines: 0,
                    slope_areas: 0,
                    per_layer: vec![],
                    source_release: "r".into(),
                },
            )
            .unwrap();
        plant(dir.path(), "resumable", 999_004, "abc123");

        let found = scan(dir.path(), &FakeProbe::default());
        assert_eq!(found.len(), 1);
        assert!(found[0].resumable);
        // The recipe comes back whole, so resuming needs nothing the crash destroyed.
        assert_eq!(found[0].recipe.device_id, "edge-840");
    }

    /// An orphaned mkgmap keeps a core busy indefinitely, which is the symptom users
    /// notice (a hot, loud laptop) long before they notice the disk.
    #[test]
    fn java_children_of_an_abandoned_build_are_found_by_their_command_line() {
        let dir = tempfile::tempdir().unwrap();
        let work = plant(dir.path(), "strays", 999_005, "k");
        let w = work.to_string_lossy().to_string();
        let probe = FakeProbe::with(
            4242,
            &format!("java -Xmx2g -jar /opt/mkgmap.jar --output-dir={w}/img"),
        )
        .and(
            4243,
            &format!("java -jar /opt/splitter.jar {w}/region.osm.pbf"),
        )
        // Somebody else's JVM, and our own jar for a *different* build.
        .and(4244, "java -jar /Users/me/gradle-wrapper.jar build")
        .and(
            4245,
            "java -jar /opt/mkgmap.jar --output-dir=/somewhere/else",
        );

        let found = scan(dir.path(), &probe);
        let mut pids = found[0].stray_pids.clone();
        pids.sort();
        assert_eq!(pids, vec![4242, 4243], "matched the wrong processes");
    }

    /// Recovery must not be blocked by a marker it cannot parse — a half-written file
    /// from a power cut, or one from a future version.
    #[test]
    fn an_unreadable_marker_is_still_reported_as_reclaimable() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("builds").join("truncated");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("region.osm.pbf"), vec![0u8; 100]).unwrap();
        std::fs::write(work.join(MARKER), b"{\"schemaVersion\": 1, \"pi").unwrap();

        let found = scan(dir.path(), &FakeProbe::default());
        assert_eq!(found.len(), 1);
        assert!(
            !found[0].resumable,
            "an unparseable build cannot be resumed"
        );
        assert!(found[0].bytes > 100);
    }

    #[test]
    fn a_marker_from_a_future_version_is_not_interpreted() {
        let dir = tempfile::tempdir().unwrap();
        let work = plant(dir.path(), "future", 999_006, "k");
        let mut m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(work.join(MARKER)).unwrap()).unwrap();
        m["schemaVersion"] = serde_json::json!(99);
        std::fs::write(work.join(MARKER), serde_json::to_vec(&m).unwrap()).unwrap();

        let found = scan(dir.path(), &FakeProbe::default());
        assert_eq!(found.len(), 1);
        assert!(!found[0].resumable);
    }

    /// Build directories with no marker are finished builds. Their outputs are the
    /// user's maps; deleting them would be data loss.
    #[test]
    fn a_directory_without_a_marker_is_not_touched() {
        let dir = tempfile::tempdir().unwrap();
        let finished = dir.path().join("builds").join("finished");
        std::fs::create_dir_all(&finished).unwrap();
        std::fs::write(finished.join("gmapsupp.img"), b"a users map").unwrap();
        assert!(scan(dir.path(), &FakeProbe::default()).is_empty());
    }

    #[test]
    fn discarding_deletes_the_work_directory_and_reports_the_bytes() {
        let dir = tempfile::tempdir().unwrap();
        plant(dir.path(), "discard-me", 999_007, "k");
        let found = scan(dir.path(), &FakeProbe::default());
        let freed = discard(&found[0]).unwrap();
        assert!(freed >= 4096);
        assert!(!found[0].work_dir.exists());
    }

    /// The cached region is complete by construction, and it is the expensive part.
    #[test]
    fn discarding_a_build_keeps_the_cached_region() {
        let dir = tempfile::tempdir().unwrap();
        let cache = crate::stage_cache::RegionCache::new(dir.path());
        let built = dir.path().join("r.osm.pbf");
        std::fs::write(&built, b"pbf").unwrap();
        cache
            .put(
                "keepme",
                &built,
                &crate::stage_cache::RegionStats {
                    features: 1,
                    nodes: 1,
                    ways: 1,
                    contour_lines: 0,
                    slope_areas: 0,
                    per_layer: vec![],
                    source_release: "r".into(),
                },
            )
            .unwrap();
        plant(dir.path(), "keeper", 999_008, "keepme");
        let found = scan(dir.path(), &FakeProbe::default());
        discard(&found[0]).unwrap();
        assert!(
            cache.get("keepme").is_some(),
            "discarding threw away the expensive part"
        );
    }

    #[test]
    fn an_absent_cache_root_scans_cleanly() {
        assert!(scan(Path::new("/nonexistent/s2g"), &FakeProbe::default()).is_empty());
    }

    /// The regions directory lives under `builds/` and is not a build.
    #[test]
    fn the_region_cache_directory_is_not_mistaken_for_an_interrupted_build() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("builds").join("regions")).unwrap();
        std::fs::write(
            dir.path()
                .join("builds")
                .join("regions")
                .join("abc.osm.pbf"),
            b"pbf",
        )
        .unwrap();
        assert!(scan(dir.path(), &FakeProbe::default()).is_empty());
    }

    /// The parser, against the real platform tool. Without this the probe could be
    /// silently returning nothing and every orphan would report no stray processes.
    #[test]
    fn the_system_probe_can_see_the_process_running_this_test() {
        let me = std::process::id();
        let cmd = SystemProbe.command_line(me);
        assert!(
            cmd.is_some(),
            "the process list parser found nothing for our own pid"
        );
        assert!(
            SystemProbe.command_line(u32::MAX).is_none(),
            "an impossible pid must not be reported as running"
        );
    }

    /// `matching` must exclude our own process, or a build would report itself.
    #[test]
    fn the_system_probe_never_matches_itself() {
        let own = SystemProbe.command_line(std::process::id()).unwrap();
        let word = own.split('/').next_back().unwrap_or(&own).to_string();
        assert!(
            !SystemProbe
                .matching(&word)
                .iter()
                .any(|(p, _)| *p == std::process::id()),
            "the probe matched the running process"
        );
    }
}
