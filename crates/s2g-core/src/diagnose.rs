//! Plain-language interpretation of build failures (SPEC.md FR-73, §12).
//!
//! mkgmap and splitter report their problems in their own vocabulary, and the useful
//! part is usually one line buried in a hundred. This turns the recognised ones into a
//! statement of what failed and what to do about it, and says plainly when it does not
//! recognise something rather than inventing a cause.

use crate::error::Error;

/// What went wrong, in terms a user can act on.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosis {
    /// One sentence naming the problem.
    pub summary: String,
    /// What to do, when there is something specific to suggest.
    pub suggestion: Option<String>,
    /// True when the cause was recognised. False means the summary is the raw error,
    /// which is honest rather than a guess dressed up as an explanation.
    pub recognised: bool,
}

impl Diagnosis {
    fn known(summary: &str, suggestion: &str) -> Self {
        Self {
            summary: summary.to_string(),
            suggestion: Some(suggestion.to_string()),
            recognised: true,
        }
    }
}

/// Interpret a build failure.
pub fn diagnose(error: &Error) -> Diagnosis {
    match error {
        Error::Cancelled => Diagnosis {
            summary: "The build was cancelled.".into(),
            suggestion: None,
            recognised: true,
        },
        Error::InsufficientSpace {
            path,
            need,
            available,
        } => Diagnosis::known(
            &format!(
                "Not enough disk space: the build needs about {} and {} is free in {}.",
                bytes(*need),
                bytes(*available),
                path.display()
            ),
            "Free some space, choose a smaller area, or point the data folder at another \
             volume on the Data screen. Deleting cached elevation tiles is safe — they \
             download again when needed.",
        ),
        // Before the "not downloaded" rules: a damaged swissTLM3D mentions
        // swissTLM3D, and reporting it as absent would send the user to download
        // something that is already there.
        Error::NotFound(msg) if msg.contains("is damaged") => Diagnosis::known(
            "The downloaded data is damaged and has been set aside.",
            "Download it again on the Data screen. The damaged copy was moved rather \
             than deleted, so nothing was lost that could be inspected.",
        ),
        Error::NotFound(msg) if msg.contains("swissTLM3D") => Diagnosis::known(
            "The national swissTLM3D dataset has not been downloaded.",
            "Download it on the Data screen. It is about 10 GB once unpacked.",
        ),
        Error::NotFound(msg) if msg.contains("outside the swissTLM3D coverage") => {
            Diagnosis::known(
                "The selected area lies outside the data's coverage.",
                "Choose an area inside Switzerland or Liechtenstein.",
            )
        }
        Error::NotFound(msg) if msg.contains("swissBOUNDARIES3D") => Diagnosis::known(
            "The administrative boundaries dataset has not been downloaded.",
            "Download swissBOUNDARIES3D on the Data screen, or choose the area another way.",
        ),
        Error::Zip(msg) => from_tool_output(msg),
        Error::Io { path, source } => Diagnosis {
            summary: format!("Could not read or write {}: {source}", path.display()),
            suggestion: Some(
                "Check that the folder exists and is writable, and that the device or \
                 volume is still connected."
                    .into(),
            ),
            recognised: true,
        },
        Error::Http { url, .. } | Error::Status { url, .. } => Diagnosis::known(
            &format!("A download failed: {url}"),
            "Check the network connection and try again. Downloads resume where they \
             stopped rather than starting over.",
        ),
        Error::ChecksumMismatch { .. } => Diagnosis::known(
            "A downloaded file did not match its published checksum.",
            "The download was discarded rather than used. Try again; if it keeps \
             happening, the copy on the server may have changed.",
        ),
        other => Diagnosis {
            summary: other.to_string(),
            suggestion: None,
            recognised: false,
        },
    }
}

/// Recognise the failures the java tools actually produce.
fn from_tool_output(log: &str) -> Diagnosis {
    let lower = log.to_lowercase();

    if lower.contains("outofmemoryerror") || lower.contains("java heap space") {
        return Diagnosis::known(
            "The map compiler ran out of memory for an area this size.",
            "Build a smaller area, or coarsen the contour interval. Contours are the \
             largest part of a build by far.",
        );
    }
    if lower.contains("invalid type") && lower.contains("style file") {
        return Diagnosis::known(
            "The cartography style uses a map type the compiler rejects.",
            "This is a fault in the shipped style rather than anything you did. Please \
             report it with the details below.",
        );
    }
    if lower.contains("could not open style") {
        return Diagnosis::known(
            "The cartography style could not be read.",
            "The installation looks incomplete. Reinstall the app, or check that the \
             style folder is present next to it.",
        );
    }
    if lower.contains("no such file") || lower.contains("cannot find") {
        return Diagnosis::known(
            "A file the build needed was missing.",
            "If a dataset was deleted or moved while the build was running, download it \
             again on the Data screen.",
        );
    }
    if lower.contains("did not write an overview map") {
        return Diagnosis::known(
            "The compiler produced no overview map, so the result would list on the \
             device but draw nothing.",
            "Please report this: it usually means the area contained no mappable \
             features at all.",
        );
    }
    if lower.contains("produced no tiles") {
        return Diagnosis::known(
            "The area contained nothing to map.",
            "Choose a larger area, or check that the layers you need are switched on in \
             the layer panel.",
        );
    }
    if lower.contains("exceeds") && lower.contains("tiles") {
        // Reached only after --max-nodes has already been retuned to the ceiling, so
        // the advice that is left is not "denser tiles" -- that was tried.
        return Diagnosis::known(
            "The map needs more tiles than this device can hold, even with the densest \
             tiles the compiler supports.",
            "Split the area into several map sets — the build screen offers a plan — or \
             choose a smaller area or coarser contours.",
        );
    }

    Diagnosis {
        summary: "The map compiler failed.".into(),
        suggestion: None,
        recognised: false,
    }
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn disk_space_reports_both_numbers_and_what_to_do() {
        let d = diagnose(&Error::InsufficientSpace {
            path: PathBuf::from("/data"),
            need: 7_855_670_912,
            available: 6_556_266_496,
        });
        assert!(d.recognised);
        assert!(d.summary.contains("7.3 GB"), "{}", d.summary);
        assert!(d.summary.contains("6.1 GB"), "{}", d.summary);
        assert!(d.summary.contains("/data"));
        assert!(d.suggestion.unwrap().contains("elevation tiles"));
    }

    #[test]
    fn a_missing_dataset_says_which_screen_to_use() {
        let d = diagnose(&Error::NotFound(
            "swissTLM3D is not downloaded; fetch it on the Data screen".into(),
        ));
        assert!(d.recognised);
        assert!(d.suggestion.unwrap().contains("Data screen"));
    }

    /// The real out-of-memory output from mkgmap on an oversized area.
    #[test]
    fn a_java_heap_failure_is_explained_in_terms_of_the_build() {
        let d = diagnose(&Error::Zip(
            "mkgmap (tiles and overview) failed:\nException in thread \"main\" \
             java.lang.OutOfMemoryError: Java heap space\n\tat uk.me.parabola..."
                .into(),
        ));
        assert!(d.recognised);
        assert!(d.summary.contains("out of memory"), "{}", d.summary);
        assert!(d.suggestion.unwrap().contains("contour"));
    }

    /// The exact failure this project hit when the slope types were out of range.
    #[test]
    fn an_invalid_style_type_is_named_as_our_fault_not_the_users() {
        let d = diagnose(&Error::Zip(
            "mkgmap (tiles and overview) failed:\nSEVERE (global): Error in style: \
             Error: invalid type 0x10220 for POLYGON in style file polygons, line 61"
                .into(),
        ));
        assert!(d.recognised);
        let s = d.suggestion.unwrap();
        assert!(s.contains("rather than anything you did"), "{s}");
    }

    /// SPEC.md §12: "Corrupt cache detected — quarantine, offer re-download."
    #[test]
    fn damaged_data_says_it_was_set_aside_and_can_be_downloaded_again() {
        let d = diagnose(&Error::NotFound(
            "the swissTLM3D data at /cache/x.gpkg is damaged (file is not a database). \
             It has been moved to /cache/.quarantine/x so it cannot be reused. Download \
             it again on the Data screen."
                .into(),
        ));
        assert!(d.recognised);
        assert!(d.suggestion.unwrap().contains("Download it again"));
    }

    /// The tile-count message is only reached after retuning has already been tried,
    /// so suggesting denser tiles would be advice the build had already taken.
    #[test]
    fn an_unfittable_tile_count_suggests_map_sets_not_denser_tiles() {
        let d = diagnose(&Error::Zip(
            "4100 tiles exceeds the 2000 this device accepts, even with the densest \
             tiles the map compiler supports."
                .into(),
        ));
        assert!(d.recognised);
        let s = d.suggestion.unwrap();
        assert!(s.contains("map sets"), "{s}");
        assert!(!s.to_lowercase().contains("denser"), "already tried: {s}");
    }

    #[test]
    fn an_unrecognised_failure_says_so_rather_than_guessing() {
        let d = diagnose(&Error::Zip("something nobody has seen before".into()));
        assert!(
            !d.recognised,
            "an unknown failure must not claim to be understood"
        );
        assert_eq!(d.suggestion, None);
    }

    /// SPEC.md §12: "Checksum mismatch — discard, warn, offer retry; never use
    /// unverified data." The discarding is in `download`; this is the explanation.
    #[test]
    fn a_downloaded_file_that_fails_its_checksum_is_not_used() {
        let d = diagnose(&Error::ChecksumMismatch {
            algo: "sha256".into(),
            expected: "abc".into(),
            actual: "def".into(),
        });
        assert!(d.recognised);
        assert!(d.summary.contains("did not match"), "{}", d.summary);
        let s = d.suggestion.unwrap();
        assert!(s.contains("discarded rather than used"), "{s}");
        assert!(s.contains("Try again"), "no retry offered: {s}");
    }

    #[test]
    fn cancellation_is_not_presented_as_an_error_to_fix() {
        let d = diagnose(&Error::Cancelled);
        assert!(d.recognised);
        assert_eq!(d.suggestion, None);
        assert!(d.summary.contains("cancelled"));
    }

    #[test]
    fn an_empty_area_suggests_the_layer_panel() {
        let d = diagnose(&Error::Zip(
            "splitter failed:\nsplitter produced no tiles; the input may be empty".into(),
        ));
        assert!(d.recognised);
        assert!(d.suggestion.unwrap().contains("layer panel"));
    }
}
