//! Realistic data for every screen: long, short and non-ASCII titles, rows of
//! varied length, episodes with subtitles and progress, and generated
//! pictures, some of the wrong shape or missing.

use slint::{Image, ModelRc, VecModel};

use couchfin_ui::{CardData, CardShape, EpisodeData, RowData};

use crate::art;

/// Titles as a real library has them.
const TITLES: [&str; 12] = [
    "The Lord of the Rings: The Fellowship of the Ring (Extended Edition)",
    "Up",
    "Amélie",
    "千と千尋の神隠し",
    "Crouching Tiger, Hidden Dragon",
    "Se7en",
    "Dr. Strangelove or: How I Learned to Stop Worrying and Love the Bomb",
    "Привет, мир",
    "Arrival",
    "Pirates of the Caribbean: Dead Men Tell No Tales",
    "Wall-E",
    "Léon: The Professional",
];

/// What a card's picture is, to exercise a slot's fitting.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pictures {
    /// Every picture arrives in its slot's shape, bar a few oddly shaped.
    Varied,
    /// Some pictures never come (the server has none).
    SomeMissing,
    /// No picture has loaded yet: every card shows the placeholder.
    NoneLoaded,
}

fn title(n: usize) -> &'static str {
    TITLES[n % TITLES.len()]
}

fn seed(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

fn pictures(shape: CardShape, n: usize, mode: Pictures) -> Image {
    let missing = match mode {
        Pictures::Varied => false,
        Pictures::SomeMissing => n % 3 == 1,
        Pictures::NoneLoaded => true,
    };
    if missing {
        return Image::default();
    }
    let s = seed(n * 5 + 1);
    match shape {
        CardShape::Poster => match n % 7 {
            3 => art::square(s),
            5 => art::banner(s),
            _ => art::poster(s),
        },
        CardShape::Thumb => match n % 6 {
            4 => art::poster(s),
            _ => art::thumb(s),
        },
    }
}

fn card(shape: CardShape, n: usize, mode: Pictures) -> CardData {
    CardData {
        title: title(n).into(),
        image: pictures(shape, n, mode),
        ..CardData::default()
    }
}

fn row(title: &str, shape: CardShape, cards: Vec<CardData>) -> RowData {
    RowData {
        title: title.into(),
        shape,
        cards: ModelRc::new(VecModel::from(cards)),
        current: 0,
    }
}

fn posters(count: usize, offset: usize, mode: Pictures) -> Vec<CardData> {
    (0..count)
        .map(|n| card(CardShape::Poster, n + offset, mode))
        .collect()
}

/// Continue Watching: thumbnails with episode subtitles and resume bars.
fn continue_watching(mode: Pictures) -> RowData {
    let subtitles = [
        "S2 · E4",
        "Movie",
        "S11 · E23",
        "S1 · E1",
        "Movie",
        "S4 · E10",
    ];
    let cards = (0..8)
        .map(|n| CardData {
            subtitle: subtitles[n % subtitles.len()].into(),
            #[allow(clippy::cast_precision_loss)]
            progress: 0.08 + (n as f32 * 0.37) % 0.85,
            ..card(CardShape::Thumb, n, mode)
        })
        .collect();
    row("Continue Watching", CardShape::Thumb, cards)
}

fn next_up(mode: Pictures) -> RowData {
    let cards = (0..6)
        .map(|n| CardData {
            subtitle: format!("S{} · E{}", n % 3 + 1, n * 2 + 1).into(),
            ..card(CardShape::Thumb, n + 3, mode)
        })
        .collect();
    row("Next Up", CardShape::Thumb, cards)
}

fn libraries(mode: Pictures) -> RowData {
    let names = [
        "Movies",
        "TV Shows",
        "Home Videos",
        "Documentaries and Concerts",
    ];
    let cards = names
        .iter()
        .enumerate()
        .map(|(n, name)| CardData {
            title: (*name).into(),
            ..card(CardShape::Thumb, n + 1, mode)
        })
        .collect();
    row("Libraries", CardShape::Thumb, cards)
}

/// Home with every row kind: many rows, one card to twenty-five, thumbnails
/// and posters.
pub fn home_rows(mode: Pictures) -> Vec<RowData> {
    vec![
        continue_watching(mode),
        next_up(mode),
        row(
            "Recently Added in Movies",
            CardShape::Poster,
            posters(14, 0, mode),
        ),
        row(
            "Recently Added in TV Shows",
            CardShape::Poster,
            posters(3, 5, mode),
        ),
        row(
            "Recently Added in Home Videos",
            CardShape::Poster,
            posters(1, 8, mode),
        ),
        row(
            "Recently Added in Anime",
            CardShape::Poster,
            posters(25, 3, mode),
        ),
        libraries(mode),
    ]
}

/// A library of `count` posters, loaded for the first `loaded` places.
pub fn library_items(count: usize, loaded: usize) -> Vec<CardData> {
    (0..count)
        .map(|n| {
            if n < loaded {
                card(CardShape::Poster, n, Pictures::Varied)
            } else {
                CardData::default()
            }
        })
        .collect()
}

/// A season's episodes: long and short titles, watched and not.
pub fn episodes(count: usize) -> Vec<EpisodeData> {
    let names = [
        "Good News About Hell",
        "Pilot",
        "The We We Are, Part Two: The Revenge of the Return of the Sequel",
        "あの日",
        "Hello, Ms. Cobel",
        "What's for Dinner?",
    ];
    (0..count)
        .map(|n| EpisodeData {
            number: i32::try_from(n + 1).unwrap_or(0),
            title: names[n % names.len()].into(),
            runtime: format!("{} min", 38 + n * 3).into(),
            watched: n < 3,
        })
        .collect()
}

/// The overview of a title that fills its box.
pub const LONG_OVERVIEW: &str = "A linguist is recruited by the military to communicate with \
    alien visitors who have landed in twelve mysterious ships around the world. As she works to \
    decode their language, tension mounts among the nations, and she begins to see her own life \
    and choices in an entirely new way, with consequences that reach well beyond the first \
    contact itself.";

/// A backdrop picture.
pub fn backdrop() -> Image {
    art::backdrop(2)
}
