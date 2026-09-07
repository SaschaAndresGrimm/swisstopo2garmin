//! Guards on the *shape* of the codebase rather than on its behaviour.
//!
//! Each one exists because the same mistake was made more than once and its symptom did
//! not look like a bug: "the data is missing", "the timestamp is wrong". A comment would
//! not have stopped the next occurrence; these do.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// Every Rust source file in the workspace, as (path, contents).
fn rust_sources() -> Vec<(PathBuf, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    let mut stack = vec![root.join("crates"), root.join("src-tauri").join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // target/ holds generated code that may legitimately contain anything.
                if path.file_name().map(|n| n == "target").unwrap_or(false) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push((
                    path.clone(),
                    std::fs::read_to_string(&path).unwrap_or_default(),
                ));
            }
        }
    }
    assert!(
        out.len() > 40,
        "the source walk found only {} files",
        out.len()
    );
    out
}

/// Nothing outside `datasets` may guess where a dataset lives.
///
/// Two cache layouts exist — the app's `<collection>/<item>/` and the flat `winter/`
/// and `routes/` directories the Milestone 5 spikes wrote — and code that probes one of
/// them directly works on the machine it was written on and fails on everyone else's.
/// This has now happened three times: the pipeline (finding 5.2), the estimator, and
/// `list_presets`, where every preset reported its data missing while the downloads sat
/// unseen in the other layout.
///
/// A test rather than a comment, because the failure looks like missing data rather
/// than like a bug.
#[test]
fn only_the_datasets_module_knows_where_datasets_live() {
    let mut offenders = Vec::new();
    for (path, text) in rust_sources() {
        // datasets.rs is where this knowledge belongs; its tests build both layouts.
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "datasets.rs" || name == "cache.rs" {
            continue;
        }
        for probe in ["join(\"winter\")", "join(\"routes\")"] {
            if text.contains(probe) {
                offenders.push(format!("{}: {probe}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these probe a dataset layout directly instead of asking datasets::\n  {}",
        offenders.join("\n  ")
    );
}

/// Only `clock` may implement the calendar.
///
/// The civil-from-days conversion had been copied into both the pipeline and the IPC
/// layer before a third caller wanted it, and the two copies had already drifted in
/// their handling of negative timestamps. A wrong date in a manifest is invisible until
/// somebody tries to reproduce a build from it.
///
/// `719_468` is the epoch shift the algorithm cannot be written without, which makes it
/// a reliable fingerprint for a fourth copy.
#[test]
fn only_the_clock_module_implements_the_calendar() {
    let offenders: Vec<String> = rust_sources()
        .into_iter()
        .filter(|(path, text)| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            // clock.rs is where it belongs, and this file names the constant in order
            // to search for it.
            name != "clock.rs"
                && name != "structure.rs"
                && (text.contains("719_468") || text.contains("719468"))
        })
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "these implement the calendar themselves instead of calling clock::\n  {}",
        offenders.join("\n  ")
    );
}
