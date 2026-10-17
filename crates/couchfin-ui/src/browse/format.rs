//! Numbers and durations as the detail pages show them.

use std::time::Duration;

/// "1 h 56 min", "42 min"; empty when there is no runtime.
pub(crate) fn runtime(runtime: Option<Duration>) -> String {
    let Some(runtime) = runtime else {
        return String::new();
    };
    // Round to the nearest minute, but never show a video as 0 min.
    let minutes = ((runtime.as_secs() + 30) / 60).max(1);
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// The year, or empty.
pub(crate) fn year(year: Option<u32>) -> String {
    year.map(|year| year.to_string()).unwrap_or_default()
}
