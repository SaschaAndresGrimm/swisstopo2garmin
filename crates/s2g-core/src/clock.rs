//! UTC timestamps, in one place.
//!
//! Manifests, provenance records and the build markers all need an RFC 3339 string, and
//! the algorithm had been written out twice by the time a third caller wanted it. It
//! stays hand-written rather than pulling in a date-time crate for one format — but it
//! stays here, once, with the conversion separated from the clock so it can be tested
//! against known instants instead of only for plausible shape.

/// Now, as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    rfc3339(secs)
}

/// A Unix timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Civil-from-days is Howard Hinnant's algorithm, which is exact for the whole proleptic
/// Gregorian range and needs no leap-year special cases.
pub fn rfc3339(unix_secs: i64) -> String {
    let (days, rem) = (unix_secs.div_euclid(86_400), unix_secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known instants, so a wrong month or an off-by-one day fails here rather than
    /// silently misdating every manifest the project ever writes.
    #[test]
    fn known_instants_convert_exactly() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1), "1970-01-01T00:00:01Z");
        assert_eq!(rfc3339(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(rfc3339(86_400), "1970-01-02T00:00:00Z");
        // 2026-09-07T12:34:56Z, checked against `date -u -r`.
        assert_eq!(rfc3339(1_788_784_496), "2026-09-07T12:34:56Z");
        // Leap day, and the day after: 2024-02-29 and 2024-03-01.
        assert_eq!(rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_709_251_200), "2024-03-01T00:00:00Z");
        // A century year divisible by 400 is a leap year: 2000-02-29.
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        // Before the epoch, which div_euclid handles and a plain `/` would not.
        assert_eq!(rfc3339(-1), "1969-12-31T23:59:59Z");
    }

    /// Every month boundary in a non-leap year, which is where the 153-day month
    /// polynomial would show a fencepost error.
    #[test]
    fn every_month_starts_on_day_one() {
        // 2026-01-01T00:00:00Z.
        let mut t = 1_767_225_600i64;
        let lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (i, days) in lengths.iter().enumerate() {
            assert_eq!(
                rfc3339(t),
                format!("2026-{:02}-01T00:00:00Z", i + 1),
                "month {} started wrong",
                i + 1
            );
            t += days * 86_400;
        }
        assert_eq!(rfc3339(t), "2027-01-01T00:00:00Z");
    }

    #[test]
    fn now_is_a_plausible_rfc3339_string() {
        let s = now_rfc3339();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z'), "{s}");
        let year: i32 = s[..4].parse().unwrap();
        assert!(year >= 2026, "the clock says {s}");
    }
}
