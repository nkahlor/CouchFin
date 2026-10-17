//! Turning API items into the cards and headers the screens show.

use std::time::Duration;

use couchfin_api::{ImageKind, Item, ItemKind, Library};

use super::state::{Card, CardShape, Episode, Header, ImageRef, Season, Target};

/// Where selecting `item` goes; `None` for kinds with no screen.
fn target(item: &Item) -> Option<Target> {
    match item.kind {
        ItemKind::Movie => Some(Target::Movie {
            id: item.id.clone(),
        }),
        ItemKind::Series => Some(Target::Series {
            id: item.id.clone(),
            season: None,
        }),
        // An episode opens its series, on the episode's season.
        ItemKind::Episode => Some(Target::Series {
            id: item.series_id.clone()?,
            season: item.season_number,
        }),
        _ => None,
    }
}

fn image(item: &Item, shape: CardShape) -> Option<ImageRef> {
    let tags = &item.image_tags;
    // An episode's primary image is its 16:9 still.
    let (kind, tag) = match (shape, &item.kind) {
        (CardShape::Poster, _) | (CardShape::Thumb, ItemKind::Episode) => {
            (ImageKind::Primary, tags.primary.as_ref())
        }
        (CardShape::Thumb, _) => tags
            .thumb
            .as_ref()
            .map(|t| (ImageKind::Thumb, Some(t)))
            .or_else(|| {
                tags.backdrop
                    .as_ref()
                    .map(|t| (ImageKind::Backdrop, Some(t)))
            })
            .unwrap_or((ImageKind::Primary, tags.primary.as_ref())),
    };
    Some(ImageRef {
        item: item.id.clone(),
        kind,
        tag: tag?.clone(),
    })
}

/// How far through `item` the user is, 0 to 1, if they started it.
fn progress(item: &Item) -> Option<f32> {
    let data = item.user_data.as_ref()?;
    let fraction = match (data.played_percentage, item.runtime) {
        (Some(percent), _) => percent / 100.0,
        (None, Some(runtime)) if !runtime.is_zero() => {
            data.position.as_secs_f64() / runtime.as_secs_f64()
        }
        _ => return None,
    };
    // f32 is plenty for a progress bar.
    #[allow(clippy::cast_possible_truncation)]
    let fraction = fraction.clamp(0.0, 1.0) as f32;
    (fraction > 0.0).then_some(fraction)
}

fn episode_label(item: &Item) -> Option<String> {
    match (item.season_number, item.episode_number) {
        (Some(s), Some(e)) => Some(format!("S{s} · E{e}")),
        (None, Some(e)) => Some(format!("E{e}")),
        _ => None,
    }
}

/// A card for `item`, or `None` for an item with no screen (a box set).
/// Progress is shown only when `with_progress` (Continue Watching).
pub(super) fn card(item: &Item, shape: CardShape, with_progress: bool) -> Option<Card> {
    let is_episode = item.kind == ItemKind::Episode;
    Some(Card {
        target: target(item)?,
        title: if is_episode {
            item.series_name
                .clone()
                .unwrap_or_else(|| item.name.clone())
        } else {
            item.name.clone()
        },
        subtitle: if is_episode {
            episode_label(item)
        } else {
            None
        },
        progress: if with_progress { progress(item) } else { None },
        image: image(item, shape),
        year: item.production_year,
        runtime: item.runtime,
    })
}

/// The tile for a library, with its cover.
pub(super) fn library_card(library: &Library) -> Card {
    Card {
        target: Target::Library {
            id: library.id.clone(),
            name: library.name.clone(),
        },
        title: library.name.clone(),
        subtitle: None,
        progress: None,
        image: library.primary_image.as_ref().map(|tag| ImageRef {
            item: library.id.clone(),
            kind: ImageKind::Primary,
            tag: tag.clone(),
        }),
        year: None,
        runtime: None,
    }
}

/// The header fields of a movie or series.
pub(super) fn header(item: &Item) -> Header {
    let resume_from = item
        .user_data
        .as_ref()
        .filter(|data| !data.played && data.position > Duration::ZERO)
        .map(|data| data.position);
    let tags = &item.image_tags;
    let of = |kind, tag: &Option<couchfin_api::ImageTag>| {
        tag.as_ref().map(|tag| ImageRef {
            item: item.id.clone(),
            kind,
            tag: tag.clone(),
        })
    };
    Header {
        title: item.name.clone(),
        year: item.production_year,
        runtime: item.runtime,
        rating: item.official_rating.clone(),
        overview: item.overview.clone(),
        resume_from,
        backdrop: of(ImageKind::Backdrop, &tags.backdrop),
        poster: of(ImageKind::Primary, &tags.primary),
    }
}

pub(super) fn season(item: &Item) -> Season {
    Season {
        id: item.id.clone(),
        name: item.name.clone(),
        number: item.season_number,
    }
}

pub(super) fn episode(item: &Item) -> Episode {
    Episode {
        id: item.id.clone(),
        number: item.episode_number,
        title: item.name.clone(),
        runtime: item.runtime,
        watched: item.user_data.as_ref().is_some_and(|data| data.played),
    }
}
