//! Numbers and durations as the detail pages show them.

use std::time::Duration;

use couchfin_core::{Card, Target};

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

/// The details strip's line for a card: "S2 · E4" for an episode, "Library"
/// for a library tile, otherwise "2016 · 1 h 56 min" (whichever of the two
/// the item has).
pub(crate) fn card_meta(card: &Card) -> String {
    if matches!(card.target, Target::Library { .. }) {
        return "Library".to_owned();
    }
    if let Some(label) = &card.subtitle {
        return label.clone();
    }
    [year(card.year), runtime(card.runtime)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}
