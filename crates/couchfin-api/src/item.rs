//! The one item type every library screen shows: a movie, a series, a season
//! or an episode, with only the fields decision 014 needs.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::ids::{ImageTag, ItemId};

/// What an [`Item`] is.
///
/// A kind this client has no variant for (a box set, a playlist, a book) is
/// [`ItemKind::Other`] with Jellyfin's name for it, so one unexpected item
/// never fails a whole page.
///
/// # Examples
///
/// ```
/// use couchfin_api::ItemKind;
///
/// assert_eq!(ItemKind::from("Movie"), ItemKind::Movie);
/// assert_eq!(ItemKind::from("BoxSet"), ItemKind::Other("BoxSet".to_owned()));
/// assert_eq!(ItemKind::Episode.as_str(), "Episode");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// A film.
    Movie,
    /// A TV show; its children are seasons.
    Series,
    /// One season of a series; its children are episodes.
    Season,
    /// One episode of a season.
    Episode,
    /// Any other type, as Jellyfin names it.
    Other(String),
}

impl ItemKind {
    /// Jellyfin's name for the kind, as used in `includeItemTypes`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Movie => "Movie",
            Self::Series => "Series",
            Self::Season => "Season",
            Self::Episode => "Episode",
            Self::Other(name) => name,
        }
    }
}

impl From<&str> for ItemKind {
    fn from(name: &str) -> Self {
        match name {
            "Movie" => Self::Movie,
            "Series" => Self::Series,
            "Season" => Self::Season,
            "Episode" => Self::Episode,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// The tags of the images an item has. A `None` means the item has no such
/// image, so there is nothing to request.
///
/// # Examples
///
/// ```
/// use couchfin_api::ImageTags;
///
/// assert_eq!(ImageTags::default().primary, None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageTags {
    /// The poster (or episode still).
    pub primary: Option<ImageTag>,
    /// The 16:9 thumbnail.
    pub thumb: Option<ImageTag>,
    /// The first backdrop (a wide background image).
    pub backdrop: Option<ImageTag>,
}

/// What the signed-in user has done with an item.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use couchfin_api::UserData;
///
/// let data = UserData {
///     played: false,
///     position: Duration::from_secs(600),
///     played_percentage: Some(10.0),
/// };
/// assert!(!data.played);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct UserData {
    /// Whether the user has finished it.
    pub played: bool,
    /// Where playback stopped; zero if it hasn't started.
    pub position: Duration,
    /// How far through it is, 0 to 100, when the server says.
    pub played_percentage: Option<f64>,
}

/// A movie, series, season or episode.
///
/// Missing or null fields are `None`, and `runtime` and `position` are
/// already converted from Jellyfin's 100-nanosecond ticks.
/// `season_number` is the season's own number on a season and the season an
/// episode belongs to on an episode; `episode_number` is set only on
/// episodes.
///
/// # Examples
///
/// ```
/// use couchfin_api::{Item, ItemKind};
///
/// let item: Item = serde_json::from_str(
///     r#"{ "Id": "abc", "Name": "Arrival", "Type": "Movie",
///          "RunTimeTicks": 69600000000, "SomeNewField": true }"#,
/// )?;
/// assert_eq!(item.kind, ItemKind::Movie);
/// assert_eq!(item.runtime.map(|d| d.as_secs()), Some(6960));
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "RawItem")]
pub struct Item {
    /// The item's ID.
    pub id: ItemId,
    /// The title.
    pub name: String,
    /// What kind of item it is.
    pub kind: ItemKind,
    /// The year it was released or first aired.
    pub production_year: Option<u32>,
    /// How long it runs.
    pub runtime: Option<Duration>,
    /// The synopsis. Lists leave it out to stay small; `item` and
    /// `episodes` include it.
    pub overview: Option<String>,
    /// The age rating, such as `PG-13`.
    pub official_rating: Option<String>,
    /// Which images it has.
    pub image_tags: ImageTags,
    /// The user's progress, when the server sent it.
    pub user_data: Option<UserData>,
    /// For a season or episode, the series' title.
    pub series_name: Option<String>,
    /// For a season or episode, the series' ID.
    pub series_id: Option<ItemId>,
    /// The season number (see above).
    pub season_number: Option<u32>,
    /// The episode number within its season.
    pub episode_number: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawItem {
    id: ItemId,
    name: String,
    #[serde(rename = "Type", default)]
    kind: String,
    production_year: Option<i64>,
    run_time_ticks: Option<i64>,
    overview: Option<String>,
    official_rating: Option<String>,
    #[serde(default)]
    image_tags: HashMap<String, Option<String>>,
    #[serde(default)]
    backdrop_image_tags: Vec<String>,
    user_data: Option<RawUserData>,
    series_name: Option<String>,
    series_id: Option<ItemId>,
    index_number: Option<i64>,
    parent_index_number: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawUserData {
    #[serde(default)]
    played: bool,
    #[serde(default)]
    playback_position_ticks: i64,
    played_percentage: Option<f64>,
}

/// Jellyfin counts time in 100 ns ticks. A negative count is nonsense, so it
/// is dropped rather than failing the page.
fn ticks(ticks: i64) -> Option<Duration> {
    let ticks = u64::try_from(ticks).ok()?;
    Some(Duration::from_nanos(ticks.saturating_mul(100)))
}

/// Tags are cache keys, so one that doesn't validate is treated as absent.
pub(crate) fn image_tag(tags: &HashMap<String, Option<String>>, kind: &str) -> Option<ImageTag> {
    tags.get(kind)?.as_deref()?.parse().ok()
}

impl From<RawItem> for Item {
    fn from(raw: RawItem) -> Self {
        let kind = ItemKind::from(raw.kind.as_str());
        let number = |n: Option<i64>| n.and_then(|n| u32::try_from(n).ok());
        let (season_number, episode_number) = match kind {
            ItemKind::Season => (number(raw.index_number), None),
            ItemKind::Episode => (number(raw.parent_index_number), number(raw.index_number)),
            _ => (None, None),
        };
        let image_tags = ImageTags {
            primary: image_tag(&raw.image_tags, "Primary"),
            thumb: image_tag(&raw.image_tags, "Thumb"),
            backdrop: raw.backdrop_image_tags.first().and_then(|t| t.parse().ok()),
        };
        Self {
            id: raw.id,
            name: raw.name,
            kind,
            production_year: number(raw.production_year),
            runtime: raw.run_time_ticks.and_then(ticks),
            overview: raw.overview,
            official_rating: raw.official_rating,
            image_tags,
            user_data: raw.user_data.map(|d| UserData {
                played: d.played,
                position: ticks(d.playback_position_ticks).unwrap_or_default(),
                played_percentage: d.played_percentage,
            }),
            series_name: raw.series_name,
            series_id: raw.series_id,
            season_number,
            episode_number,
        }
    }
}
