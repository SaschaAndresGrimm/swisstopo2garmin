//! Measuring this program's own cost, so a regression is caught by CI rather than by a
//! user with a canton to build (SPEC.md NFR-1, NFR-2, §13.6).
//!
//! Two things are worth measuring, and they need different treatment.
//!
//! **Rates** — seconds per square kilometre, per feature, per point — extrapolate to the
//! scales that matter. A canton is 2,000 km² and cannot be built in CI, but the rate
//! measured on a 1 km² fixture multiplied by 2,000 is a defensible prediction, and a rate
//! that regresses by an order of magnitude is what actually happens when somebody
//! accidentally makes an inner loop quadratic.
//!
//! **Peak memory** is not a rate, and its NFR is not a number so much as a shape: peak
//! RSS must not grow with the size of the input (NFR-2). That is testable at fixture
//! scale — run the same stage over 1×, 4× and 16× input and see whether the peak follows
//! — and it is a stronger statement than any single threshold, because it is the property
//! that makes the national build possible at all.

/// Peak resident set size of this process in bytes, where the platform will say.
///
/// Linux keeps a high-water mark in `/proc/self/status`, which is exactly what is
/// wanted: sampling a current RSS misses the peak between samples. macOS and Windows
/// expose the same figure only through APIs this crate cannot reach without `unsafe`
/// (the workspace forbids it), so there it returns `None` and the tests that need a
/// peak skip rather than assert something they cannot measure.
pub fn peak_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Current resident set size in bytes, where the platform will say.
///
/// Less useful than the peak but available on macOS through `ps`, which is enough for a
/// developer running the harness locally to see roughly what a stage costs.
pub fn current_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        let kb: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        Some(kb * 1024)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// One measured rate, and the ceiling it is allowed to reach.
#[derive(Debug, Clone)]
pub struct Rate {
    pub name: &'static str,
    /// What was measured, in the unit named by `unit`.
    pub measured: f64,
    /// The most this may be before CI fails.
    pub ceiling: f64,
    pub unit: &'static str,
}

impl Rate {
    pub fn within_budget(&self) -> bool {
        self.measured <= self.ceiling
    }

    /// A line for the CI log, in the same shape whether it passed or failed, so a run's
    /// history can be read by grepping for the measurement name.
    pub fn report(&self) -> String {
        format!(
            "{:<32} {:>12.4} {:<14} ceiling {:>12.4}  {}",
            self.name,
            self.measured,
            self.unit,
            self.ceiling,
            if self.within_budget() { "ok" } else { "OVER" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The point of the module: on Linux, where CI's performance job runs, the peak has
    /// to be readable. Elsewhere it must return `None` rather than a wrong number.
    #[test]
    fn memory_is_readable_where_the_platform_says_so() {
        let peak = peak_rss_bytes();
        if cfg!(target_os = "linux") {
            let peak = peak.expect("Linux keeps VmHWM in /proc/self/status");
            assert!(
                peak > 1_000_000,
                "a running test process uses more than 1 MB"
            );
            assert!(peak < 100_000_000_000, "implausible peak: {peak}");
        } else {
            assert!(
                peak.is_none(),
                "a peak was reported where it cannot be measured"
            );
        }
    }

    #[test]
    fn current_memory_is_readable_on_the_development_platforms() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            let rss = current_rss_bytes().expect("ps or /proc should report an RSS");
            assert!(rss > 1_000_000, "implausibly small RSS: {rss}");
        }
    }

    #[test]
    fn a_rate_over_its_ceiling_is_reported_as_over() {
        let ok = Rate {
            name: "contours",
            measured: 0.5,
            ceiling: 1.0,
            unit: "s/km2",
        };
        let over = Rate {
            measured: 1.5,
            ..ok.clone()
        };
        assert!(ok.within_budget());
        assert!(!over.within_budget());
        assert!(ok.report().ends_with("ok"));
        assert!(over.report().ends_with("OVER"));
        // Exactly at the ceiling passes: a threshold nobody may reach is a threshold
        // that has to be edited every time the hardware changes.
        assert!(Rate {
            measured: 1.0,
            ..ok
        }
        .within_budget());
    }
}
