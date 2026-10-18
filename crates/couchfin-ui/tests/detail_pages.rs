//! The movie and series detail pages on Slint's testing backend: header,
//! Play or Resume, seasons and episodes, focus, callbacks and Back.

mod common;

use common::browse::{movie, series};
use common::home::{SCREENS, fully_on_screen};
use common::{labelled, press, shows, the};
use couchfin_ui::MovieView;
use slint::ComponentHandle;
use slint::LogicalSize;
use slint::platform::Key;

#[test]
fn the_movie_page_shows_its_header() {
    let (window, _) = movie();
    for text in [
        "Arrival",
        "2016",
        "1 h 56 min",
        "PG-13",
        "A linguist is recruited to talk to visitors.",
        "Play",
    ] {
        assert!(shows(&window, text), "{text:?} is shown");
    }
}

#[test]
fn the_button_says_resume_when_there_is_a_position() {
    let (window, _) = movie();
    window.global::<MovieView>().set_resumable(true);
    assert!(shows(&window, "Resume"));
    assert!(labelled(&window, "Play").is_empty());
}

#[test]
fn play_fires_a_callback_and_says_playback_arrives_soon() {
    let (window, calls) = movie();
    assert!(labelled(&window, "Playback arrives soon").is_empty());
    press(&window, Key::Return);
    assert_eq!(calls.taken(), ["play"]);
    assert!(shows(&window, "Playback arrives soon"));
}

#[test]
fn back_goes_back_from_the_movie_page() {
    let (window, calls) = movie();
    press(&window, Key::Escape);
    press(&window, Key::Backspace);
    assert_eq!(calls.taken(), ["back", "back"]);
}

#[test]
fn nothing_is_cut_off_on_the_movie_page_at_either_size() {
    for (width, height) in SCREENS {
        let (window, _) = movie();
        window.window().set_size(LogicalSize::new(width, height));
        press(&window, Key::Return);
        for text in ["Arrival", "PG-13", "Play", "Playback arrives soon"] {
            assert!(
                fully_on_screen(&window, &the(&window, text)),
                "{width}x{height}: {text:?} is cut off"
            );
        }
    }
}

#[test]
fn the_series_page_shows_header_seasons_and_episodes() {
    let (window, _) = series(8);
    for text in [
        "Severance",
        "2022",
        "TV-MA",
        "Season 1",
        "Season 2",
        "Season 3",
    ] {
        assert!(shows(&window, text), "{text:?} is shown");
    }
    assert!(shows(&window, "Episode title 1"));
    assert!(!labelled(&window, "42 min").is_empty());
}

#[test]
fn left_and_right_choose_a_season_and_ask_for_its_episodes() {
    let (window, calls) = series(8);
    press(&window, Key::RightArrow);
    press(&window, Key::RightArrow);
    press(&window, Key::RightArrow);
    press(&window, Key::LeftArrow);
    assert_eq!(calls.taken(), ["season(1)", "season(2)", "season(1)"]);
}

#[test]
fn down_enters_the_episodes_up_and_down_walk_them_and_enter_picks_one() {
    let (window, calls) = series(30);
    press(&window, Key::DownArrow);
    press(&window, Key::DownArrow);
    press(&window, Key::DownArrow);
    press(&window, Key::Return);
    assert_eq!(calls.taken(), ["episode(2)"]);
    assert!(shows(&window, "Playback arrives soon"));
    press(&window, Key::UpArrow);
    press(&window, Key::Return);
    assert_eq!(calls.taken(), ["episode(1)"]);
    // Up from the first episode goes back to the seasons.
    press(&window, Key::UpArrow);
    press(&window, Key::UpArrow);
    press(&window, Key::RightArrow);
    assert_eq!(calls.taken(), ["season(1)"]);
}

#[test]
fn a_long_season_scrolls_to_keep_the_focused_episode_on_screen() {
    for (width, height) in SCREENS {
        let (window, _) = series(30);
        window.window().set_size(LogicalSize::new(width, height));
        press(&window, Key::DownArrow);
        for _ in 0..29 {
            press(&window, Key::DownArrow);
        }
        common::home::settle();
        let last = the(&window, "Episode title 30");
        assert!(
            fully_on_screen(&window, &last),
            "{width}x{height}: the last episode is cut off"
        );
    }
}

#[test]
fn back_goes_back_from_the_series_page() {
    let (window, calls) = series(8);
    press(&window, Key::Escape);
    press(&window, Key::DownArrow);
    press(&window, Key::Backspace);
    assert_eq!(calls.taken(), ["back", "back"]);
}
