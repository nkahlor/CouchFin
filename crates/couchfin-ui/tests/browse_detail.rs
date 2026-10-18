//! The movie and series pages bound to couchfin-core's detail states.

mod common;

use std::time::Duration;

use common::library::{Shell, World, picture};
use common::press;
use couchfin_core::{
    ApiError, Episode, Header, LibraryAction, LibraryState, ScreenState, Season, SeriesState,
};
use couchfin_ui::{MovieView, Screen, SeriesView};
use slint::platform::Key;
use slint::{ComponentHandle, Model};

/// The first movie of the library, opened from its grid, with its poster as
/// the backdrop (the fake has no backdrops; core picks the image, the page
/// only shows it).
fn movie() -> (World, LibraryState) {
    let world = World::new(5);
    world.open_big();
    world.send(LibraryAction::OpenGridItem(0));
    let mut state = world.state();
    let ScreenState::Movie(movie) = state.screens.last_mut().expect("a screen") else {
        panic!("a movie is on top");
    };
    let header = movie.header.as_mut().expect("the movie loaded");
    header.backdrop = header.poster.clone();
    header.resume_from = Some(Duration::from_secs(90));
    header.runtime = Some(Duration::from_mins(116));
    header.year = Some(2016);
    header.rating = Some("PG-13".into());
    (world, state)
}

fn header() -> Header {
    Header {
        title: "Severance".into(),
        year: Some(2022),
        runtime: Some(Duration::from_mins(50)),
        rating: Some("TV-MA".into()),
        overview: Some("Office workers split their memories.".into()),
        resume_from: None,
        backdrop: None,
        poster: None,
    }
}

fn episodes(season: u32, count: u32) -> Vec<Episode> {
    (1..=count)
        .map(|n| Episode {
            id: format!("e{season}-{n}").parse().expect("an ID"),
            number: Some(n),
            title: format!("S{season} episode {n}"),
            runtime: Some(Duration::from_mins(42)),
            watched: n == 1,
        })
        .collect()
}

/// A series page on top of Home, on `selected` with 3 seasons.
fn series(selected: usize, episodes: Vec<Episode>) -> LibraryState {
    let mut state = World::new(5).state();
    state.screens.push(ScreenState::Series(SeriesState {
        id: "series-1".parse().expect("an ID"),
        header: Some(header()),
        seasons: (1..=3)
            .map(|n| Season {
                id: format!("season-{n}").parse().expect("an ID"),
                name: format!("Season {n}"),
                number: Some(n),
            })
            .collect(),
        selected_season: selected,
        episodes,
        loading: false,
        error: None,
    }));
    state
}

#[test]
fn the_movie_page_shows_the_header_formatted() {
    let shell = Shell::new();
    let (_world, state) = movie();
    shell.show(state);
    assert_eq!(shell.window.get_screen(), Screen::Movie);
    let view = shell.window.global::<MovieView>();
    assert_eq!(view.get_title(), "Big 00000");
    assert_eq!(view.get_year(), "2016");
    assert_eq!(view.get_runtime(), "1 h 56 min");
    assert_eq!(view.get_rating(), "PG-13");
    assert!(view.get_resumable());
    assert_eq!(view.get_error(), "");
}

#[test]
fn the_movie_backdrop_is_fetched_wide_and_cleared_on_leaving() {
    let shell = Shell::new();
    let (world, state) = movie();
    shell.show(state);
    let backdrop = shell
        .loader
        .started()
        .into_iter()
        .find(|(item, ..)| item.len() > 3)
        .expect("a backdrop load");
    // A wide fixed box (1280x720 at scale 1), not a poster's size.
    assert_eq!(backdrop.1, 1_280, "{}", backdrop.1);
    shell.loader.finish(&backdrop.0, picture());
    shell.feed.pump();
    let view = shell.window.global::<MovieView>();
    assert_eq!(view.get_backdrop().size().width, 2);

    world.send(LibraryAction::Back);
    shell.update(world.state());
    assert_eq!(view.get_backdrop().size().width, 0, "cleared with the page");
}

#[test]
fn back_and_a_failed_load_on_the_movie_page() {
    let shell = Shell::new();
    let (_world, mut state) = movie();
    let ScreenState::Movie(movie) = state.screens.last_mut().expect("a screen") else {
        panic!("a movie is on top");
    };
    movie.error = Some(ApiError::ItemNotFound);
    shell.show(state);
    assert_eq!(
        shell.window.global::<MovieView>().get_error(),
        "That's no longer in your library."
    );
    shell.commands.taken();
    press(&shell.window, Key::Escape);
    assert_eq!(shell.commands.taken(), ["Back"]);
}

#[test]
fn the_series_page_lists_seasons_and_the_selected_ones_episodes() {
    let shell = Shell::new();
    shell.show(series(1, episodes(2, 8)));
    assert_eq!(shell.window.get_screen(), Screen::Series);
    let view = shell.window.global::<SeriesView>();
    assert_eq!(view.get_title(), "Severance");
    assert_eq!(view.get_runtime(), "50 min");
    assert_eq!(view.get_selected_season(), 1);
    let seasons: Vec<_> = view.get_seasons().iter().map(|s| s.to_string()).collect();
    assert_eq!(seasons, ["Season 1", "Season 2", "Season 3"]);
    let list: Vec<_> = view.get_episodes().iter().collect();
    assert_eq!(list.len(), 8);
    assert_eq!(list[0].title, "S2 episode 1");
    assert_eq!(list[0].number, 1);
    assert_eq!(list[0].runtime, "42 min");
    assert!(list[0].watched && !list[1].watched);
}

#[test]
fn choosing_a_season_sends_it_and_core_catching_up_changes_nothing() {
    let shell = Shell::new();
    shell.show(series(0, episodes(1, 8)));
    shell.commands.taken();
    press(&shell.window, Key::RightArrow);
    assert_eq!(shell.commands.taken(), ["SelectSeason(1)"]);
    let view = shell.window.global::<SeriesView>();
    assert_eq!(view.get_selected_season(), 1, "the page moved at once");

    // A state published before core got to it must not move the page back.
    shell.update(series(0, episodes(1, 8)));
    assert_eq!(view.get_selected_season(), 1);

    // Core's answer: the season and its episodes.
    shell.update(series(1, episodes(2, 5)));
    assert_eq!(view.get_selected_season(), 1);
    assert_eq!(view.get_episodes().row_count(), 5);
    assert_eq!(
        view.get_episodes().row_data(0).expect("an episode").title,
        "S2 episode 1"
    );
}

#[test]
fn core_may_choose_the_season_when_the_user_hasnt() {
    let shell = Shell::new();
    shell.show(series(0, episodes(1, 3)));
    shell.update(series(2, episodes(3, 3)));
    assert_eq!(shell.window.global::<SeriesView>().get_selected_season(), 2);
}

#[test]
fn another_series_starts_with_its_own_lists() {
    let shell = Shell::new();
    shell.show(series(2, episodes(3, 6)));
    let mut next = series(0, Vec::new());
    let ScreenState::Series(series) = next.screens.last_mut().expect("a screen") else {
        panic!("a series is on top");
    };
    series.id = "series-2".parse().expect("an ID");
    series.loading = true;
    shell.update(next);
    let view = shell.window.global::<SeriesView>();
    assert_eq!(view.get_selected_season(), 0);
    assert_eq!(view.get_episodes().row_count(), 0);
}

#[test]
fn back_on_the_series_page_goes_back_and_an_error_shows() {
    let shell = Shell::new();
    let mut state = series(0, episodes(1, 3));
    let ScreenState::Series(series) = state.screens.last_mut().expect("a screen") else {
        panic!("a series is on top");
    };
    series.error = Some(ApiError::Status { status: 500 });
    shell.show(state);
    assert_eq!(
        shell.window.global::<SeriesView>().get_error(),
        "The server had a problem. Try again in a moment."
    );
    shell.commands.taken();
    press(&shell.window, Key::Escape);
    assert_eq!(shell.commands.taken(), ["Back"]);
}
