//! Copying a built map onto a device, and surviving the device going away
//! (SPEC.md FR-80…FR-82, §12).
//!
//! A Garmin mounted over USB is the least reliable filesystem this program writes to.
//! It gets unplugged mid-copy, it fills up, and a flaky link corrupts bytes without
//! changing the file's length. Three consequences shape this module:
//!
//! * The copy goes to a `.part` file and is renamed into place, so an interrupted write
//!   can never leave a truncated `gmapsupp.img` the device would try to load.
//! * The result is verified by re-reading it **from the device** and comparing digests,
//!   because a mid-transfer corruption has exactly the right size (FR-82).
//! * When anything fails, the device is put back the way it was found — the partial file
//!   removed and any backup restored — and if that cleanup cannot be completed, the
//!   report says precisely which file is left where, since at that point the user is the
//!   only one who can finish the job.

use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// What an install did, once it is known to have worked.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub target: PathBuf,
    /// Where the previous map was moved, when one was replaced.
    pub backup: Option<PathBuf>,
    /// Digest of the file as read back from the device.
    pub sha256: String,
}

/// Copy `source` to `target`, verify it, and never leave the device worse off.
///
/// `backup` moves an existing target aside instead of overwriting it (§12, "Target file
/// already exists — confirm, offer backup, never silently overwrite"). Confirmation is
/// the caller's job: this is told what was decided.
pub fn install(source: &Path, target: &Path, backup: bool) -> Result<Installed> {
    install_with(source, target, backup, |from, to| std::fs::copy(from, to))
}

/// The copy step is injectable so the failure that matters most — the device vanishing
/// part-way through — can be tested, which is impossible with a real cable.
pub fn install_with(
    source: &Path,
    target: &Path,
    backup: bool,
    copy: impl Fn(&Path, &Path) -> io::Result<u64>,
) -> Result<Installed> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }

    // The backup is made first, so a failure after this point has something to restore.
    let backup_path = if target.exists() && backup {
        let bak = backup_name(target);
        std::fs::rename(target, &bak).map_err(|e| Error::io(&bak, e))?;
        Some(bak)
    } else {
        None
    };

    let part = partial_name(target);
    if let Err(e) = copy(source, &part) {
        return Err(rollback(&part, backup_path.as_deref(), target, e));
    }
    if let Err(e) = std::fs::rename(&part, target) {
        return Err(rollback(&part, backup_path.as_deref(), target, e));
    }

    // Re-read from the device rather than trusting the copy. A truncated file has the
    // wrong length, but a link that corrupts a block does not, and that is the failure
    // that produces a map the device loads and then draws wrongly.
    let source_digest = crate::cache::sha256_of(source).map_err(|e| Error::io(source, e))?;
    let device_digest = match crate::cache::sha256_of(target) {
        Ok(d) => d,
        Err(e) => return Err(rollback(&part, backup_path.as_deref(), target, e)),
    };
    if source_digest != device_digest {
        // Leave nothing half-installed: the device would try to load it.
        let _ = std::fs::remove_file(target);
        if let Some(bak) = &backup_path {
            let _ = std::fs::rename(bak, target);
        }
        return Err(Error::ChecksumMismatch {
            algo: "sha256".into(),
            expected: source_digest,
            actual: device_digest,
        });
    }

    Ok(Installed {
        target: target.to_path_buf(),
        backup: backup_path,
        sha256: device_digest,
    })
}

/// Put the device back as it was, and say what could not be put back.
///
/// The interesting case is the second failure: the device was unplugged, so the partial
/// file cannot be deleted and the backup cannot be restored. Nothing in this program can
/// fix that, and the only useful thing left to do is name the files.
fn rollback(part: &Path, backup: Option<&Path>, target: &Path, cause: io::Error) -> Error {
    let mut stranded: Vec<String> = Vec::new();

    if part.exists() && std::fs::remove_file(part).is_err() {
        stranded.push(format!("an incomplete {}", part.display()));
    }
    if let Some(bak) = backup {
        if bak.exists() && std::fs::rename(bak, target).is_err() {
            stranded.push(format!("the previous map, still named {}", bak.display()));
        }
    }

    let detail = if stranded.is_empty() {
        "Nothing was left on the device, so it is safe to try again.".to_string()
    } else {
        format!(
            "The device still holds {}. Reconnect it and delete that file, or try again.",
            stranded.join(" and ")
        )
    };
    Error::Io {
        path: target.to_path_buf(),
        source: io::Error::new(cause.kind(), format!("{cause}. {detail}")),
    }
}

/// `gmapsupp.img` -> `gmapsupp.img.part`.
///
/// Appended rather than substituted: `Path::with_extension` would turn
/// `swisstopo.map.img` into `swisstopo.map.part` and overwrite an unrelated file.
fn partial_name(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

fn backup_name(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn a_clean_install_verifies_the_copy_on_the_device() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"a garmin map");
        let target = dir.path().join("Garmin").join("gmapsupp.img");

        let done = install(&src, &target, false).unwrap();
        assert_eq!(done.target, target);
        assert_eq!(done.backup, None);
        assert_eq!(std::fs::read(&target).unwrap(), b"a garmin map");
        assert_eq!(done.sha256, crate::cache::sha256_of(&src).unwrap());
        // No leftovers.
        assert!(!partial_name(&target).exists());
    }

    /// SPEC.md §12: "Target file already exists — confirm, offer backup, never silently
    /// overwrite." Confirmation is the UI's; the backup is here.
    #[test]
    fn an_existing_map_is_kept_when_a_backup_was_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"the new map");
        let target = map(dir.path(), "gmapsupp.img", b"the old map");

        let done = install(&src, &target, true).unwrap();
        let bak = done.backup.expect("a backup should have been made");
        assert_eq!(std::fs::read(&bak).unwrap(), b"the old map");
        assert_eq!(std::fs::read(&target).unwrap(), b"the new map");
        // `.img.bak`, not `.bak` replacing the extension.
        assert!(
            bak.to_string_lossy().ends_with("gmapsupp.img.bak"),
            "{bak:?}"
        );
    }

    #[test]
    fn without_a_backup_the_existing_map_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"the new map");
        let target = map(dir.path(), "gmapsupp.img", b"the old map");

        let done = install(&src, &target, false).unwrap();
        assert_eq!(done.backup, None);
        assert_eq!(std::fs::read(&target).unwrap(), b"the new map");
    }

    /// SPEC.md §12: "Device unplugged mid-copy — abort, report incomplete file on
    /// device, offer retry."
    ///
    /// The device is gone, so the partial file cannot be deleted. The report has to name
    /// it, because at that point the user is the only one who can remove it.
    #[test]
    fn an_unplugged_device_reports_the_incomplete_file_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"a garmin map");
        let target = dir.path().join("gmapsupp.img");

        // A copy that writes something and then fails, leaving a file that cannot be
        // removed -- which is what an unplugged volume does.
        let err = install_with(&src, &target, false, |_from, to| {
            std::fs::write(to, b"half a ma").unwrap();
            // Make the partial undeletable by turning it into a non-empty directory,
            // the portable way to fail a `remove_file`.
            let stuck = to.with_extension("part.stuck");
            std::fs::create_dir_all(&stuck).unwrap();
            std::fs::write(stuck.join("x"), b"x").unwrap();
            std::fs::remove_file(to).unwrap();
            std::fs::rename(&stuck, to).unwrap();
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "device not configured",
            ))
        })
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("device not configured"), "no cause: {msg}");
        assert!(
            msg.contains("gmapsupp.img.part"),
            "the file is not named: {msg}"
        );
        assert!(msg.contains("delete that file"), "no way forward: {msg}");
        // And no map was installed.
        assert!(
            !target.exists(),
            "a partial map was left where the device loads from"
        );
    }

    /// The recoverable version of the same failure: the partial could be cleaned up, so
    /// the user needs to know only that retrying is safe.
    #[test]
    fn a_failed_copy_that_could_be_cleaned_up_says_retrying_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"a garmin map");
        let target = dir.path().join("gmapsupp.img");

        let err = install_with(&src, &target, false, |_from, to| {
            std::fs::write(to, b"half").unwrap();
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "no space left on device",
            ))
        })
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("no space left"), "{msg}");
        assert!(msg.contains("safe to try again"), "{msg}");
        assert!(
            !partial_name(&target).exists(),
            "the partial file was not cleaned up"
        );
        assert!(!target.exists());
    }

    /// The worst outcome would be losing the map that was already on the device in
    /// exchange for one that never arrived.
    #[test]
    fn a_failed_copy_restores_the_map_that_was_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"the new map");
        let target = map(dir.path(), "gmapsupp.img", b"the old map");

        install_with(&src, &target, true, |_from, to| {
            std::fs::write(to, b"half").unwrap();
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "device went away",
            ))
        })
        .unwrap_err();

        assert!(target.exists(), "the device lost the map it had");
        assert_eq!(std::fs::read(&target).unwrap(), b"the old map");
        assert!(
            !backup_name(&target).exists(),
            "the backup was not moved back"
        );
    }

    /// FR-82: a link that corrupts a block leaves the length unchanged, so only a
    /// digest read back from the device catches it.
    #[test]
    fn a_corrupted_copy_of_the_right_length_is_caught_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let src = map(dir.path(), "built.img", b"a garmin map");
        let target = map(dir.path(), "gmapsupp.img", b"the old map");

        let err = install_with(&src, &target, true, |_from, to| {
            // Same length, one byte wrong.
            std::fs::write(to, b"a garmin mip").unwrap();
            Ok(12)
        })
        .unwrap_err();

        assert!(
            matches!(err, Error::ChecksumMismatch { .. }),
            "expected a checksum mismatch, got {err}"
        );
        // Nothing half-installed, and the previous map is back.
        assert_eq!(std::fs::read(&target).unwrap(), b"the old map");
    }

    /// `with_extension` would turn `swisstopo.map.img` into `swisstopo.map.part` and
    /// overwrite a file that has nothing to do with this install.
    #[test]
    fn the_temporary_names_are_appended_not_substituted() {
        let t = Path::new("/Volumes/GARMIN/Garmin/swisstopo.map.img");
        assert_eq!(
            partial_name(t),
            Path::new("/Volumes/GARMIN/Garmin/swisstopo.map.img.part")
        );
        assert_eq!(
            backup_name(t),
            Path::new("/Volumes/GARMIN/Garmin/swisstopo.map.img.bak")
        );
    }
}
