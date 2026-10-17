//! The movie and series detail pages: long and short titles, with and
//! without a backdrop, the season row and the episode list scrolled, and the
//! load-failure state.

use anyhow::Result;
use couchfin_ui::{EpisodeData, MovieView, Screen, SeriesView};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::fixtures;
use crate::studio::Studio;

struct Movie<'a> {
    state: &'a str,
    title: &'a str,
    overview: &'a str,
    backdrop: bool,
    resumable: bool,
    error: &'a str,
}

fn movie(studio: &mut Studio, size: (u32, u32), m: &Movie) -> Result<()> {
    let mut session = studio.open(Screen::Movie, size)?;
    let view = session.window.global::<MovieView>();
    view.set_title(m.title.into());
    view.set_year("2016".into());
    view.set_runtime("1 h 56 min".into());
    view.set_rating("PG-13".into());
    view.set_overview(m.overview.into());
    view.set_resumable(m.resumable);
    view.set_error(m.error.into());
    if m.backdrop {
        view.set_backdrop(fixtures::backdrop());
    }
    session.start()?;
    session.snap("movie", m.state)
}

#[allow(clippy::too_many_arguments)]
fn series(
    studio: &mut Studio,
    size: (u32, u32),
    state: &str,
    title: &str,
    seasons: usize,
    episodes: Vec<EpisodeData>,
    steps: &[(&str, Key, usize)],
) -> Result<()> {
    let mut session = studio.open(Screen::Series, size)?;
    let view = session.window.global::<SeriesView>();
    view.set_title(title.into());
    view.set_year("2022".into());
    view.set_runtime("50 min".into());
    view.set_rating("TV-MA".into());
    view.set_overview(fixtures::LONG_OVERVIEW.into());
    view.set_backdrop(fixtures::backdrop());
    let names: Vec<slint::SharedString> = (1..=seasons)
        .map(|n| format!("Season {n}").into())
        .collect();
    view.set_seasons(ModelRc::new(VecModel::from(names)));
    view.set_episodes(ModelRc::new(VecModel::from(episodes)));
    session.start()?;
    session.snap("series", &format!("{state}-start"))?;
    for (name, key, times) in steps {
        session.press(*key, *times);
        session.snap("series", &format!("{state}-{name}"))?;
    }
    Ok(())
}

/// Both detail pages at one size.
pub fn shoot(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    movie(
        studio,
        size,
        &Movie {
            state: "long-title-resume",
            title: "The Lord of the Rings: The Fellowship of the Ring (Extended Edition)",
            overview: fixtures::LONG_OVERVIEW,
            backdrop: true,
            resumable: true,
            error: "",
        },
    )?;
    movie(
        studio,
        size,
        &Movie {
            state: "short-title-no-backdrop",
            title: "Up",
            overview: "",
            backdrop: false,
            resumable: false,
            error: "",
        },
    )?;
    movie(
        studio,
        size,
        &Movie {
            state: "error",
            title: "Arrival",
            overview: "",
            backdrop: false,
            resumable: false,
            error: "That's no longer in your library.",
        },
    )?;
    series(
        studio,
        size,
        "seasons-and-episodes",
        "Severance",
        6,
        fixtures::episodes(12),
        &[
            ("season-2", Key::RightArrow, 1),
            ("episodes-first", Key::DownArrow, 1),
            ("episodes-scrolled", Key::DownArrow, 9),
        ],
    )?;
    series(
        studio,
        size,
        "long-title-few-episodes",
        "The Adventures of Sherlock Holmes and the Case of the Very Long Title",
        1,
        fixtures::episodes(2),
        &[],
    )?;
    series(
        studio,
        size,
        "no-episodes",
        "Up",
        3,
        Vec::new(),
        &[("season-3", Key::RightArrow, 2)],
    )
}
