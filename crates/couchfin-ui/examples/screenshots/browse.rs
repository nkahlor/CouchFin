//! Home and the library grid: every row kind, focus in several places,
//! pictures missing or not yet loaded, and the load-failure states.

use anyhow::Result;
use couchfin_ui::{HomeView, LibraryView, RowData, Screen};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::fixtures::{self, Pictures};
use crate::studio::Studio;

/// Fills Home as the app does once core has loaded it.
fn fill_home(window: &couchfin_ui::MainWindow, rows: Vec<RowData>) {
    let view = window.global::<HomeView>();
    view.set_server_name("Living Room".into());
    view.set_server_version("10.11.2".into());
    view.set_username("alice".into());
    view.set_rows(ModelRc::new(VecModel::from(rows)));
}

/// One window, moved through `steps` of (state name, key, presses); each
/// step is photographed after its moves.
fn home_walk(
    studio: &mut Studio,
    size: (u32, u32),
    mode: Pictures,
    steps: &[(&str, Key, usize)],
) -> Result<()> {
    let mut session = studio.open(Screen::Home, size)?;
    fill_home(&session.window, fixtures::home_rows(mode));
    session.start()?;
    for (state, key, times) in steps {
        session.press(*key, *times);
        session.snap("home", state)?;
    }
    Ok(())
}

fn home_states(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    // Presses are cumulative: each step moves on from the one before.
    home_walk(
        studio,
        size,
        Pictures::Varied,
        &[
            ("focus-row1-card1", Key::Home, 0),
            ("focus-row1-card4", Key::RightArrow, 3),
            ("focus-row1-card8-last", Key::RightArrow, 10),
            ("focus-row3-card1", Key::DownArrow, 2),
            ("focus-row3-card5", Key::RightArrow, 4),
            ("focus-row6-card1", Key::DownArrow, 3),
            ("focus-row6-card25-last", Key::RightArrow, 30),
            ("focus-row7-card1", Key::DownArrow, 1),
            ("focus-signout", Key::DownArrow, 1),
        ],
    )?;
    home_walk(
        studio,
        size,
        Pictures::SomeMissing,
        &[
            ("missing-images-row1-card1", Key::Home, 0),
            ("missing-images-row3-card2", Key::DownArrow, 2),
        ],
    )?;
    home_walk(
        studio,
        size,
        Pictures::NoneLoaded,
        &[
            ("images-loading-row1-card1", Key::Home, 0),
            ("images-loading-row3-card3", Key::DownArrow, 2),
        ],
    )?;
    Ok(())
}

fn home_failures(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    let mut session = studio.open(Screen::Home, size)?;
    fill_home(&session.window, Vec::new());
    session
        .window
        .global::<HomeView>()
        .set_status("Loading your library…".into());
    session.start()?;
    session.snap("home", "loading")?;

    let mut session = studio.open(Screen::Home, size)?;
    fill_home(&session.window, Vec::new());
    let view = session.window.global::<HomeView>();
    view.set_status("Couldn't reach the server. Is it on?".into());
    view.set_error("Couldn't reach the server. Is it on?".into());
    session.start()?;
    session.snap("home", "error-no-rows")?;

    let mut session = studio.open(Screen::Home, size)?;
    fill_home(&session.window, fixtures::home_rows(Pictures::Varied));
    let view = session.window.global::<HomeView>();
    view.set_notice("Signed in, but the sign-in couldn't be saved on this device.".into());
    view.set_error("Couldn't load Recently Added in Anime. The server had a problem.".into());
    session.start()?;
    session.snap("home", "notice-and-row-error")?;
    Ok(())
}

fn library(
    studio: &mut Studio,
    size: (u32, u32),
    state: &str,
    (count, loaded): (usize, usize),
    error: &str,
    steps: &[(&str, Key, usize)],
) -> Result<()> {
    let mut session = studio.open(Screen::Library, size)?;
    let view = session.window.global::<LibraryView>();
    view.set_title("Movies and Other Films We Own on Disc".into());
    view.set_item_count(i32::try_from(count)?);
    view.set_items(ModelRc::new(VecModel::from(fixtures::library_items(
        count, loaded,
    ))));
    view.set_error(error.into());
    session.start()?;
    session.snap("library", &format!("{state}-top"))?;
    for (name, key, times) in steps {
        session.press(*key, *times);
        session.snap("library", &format!("{state}-{name}"))?;
    }
    Ok(())
}

fn library_states(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    library(
        studio,
        size,
        "full",
        (240, 240),
        "",
        &[
            ("scrolled-row4", Key::DownArrow, 3),
            ("scrolled-row12-card-right", Key::DownArrow, 8),
            ("last-item", Key::End, 0),
        ],
    )?;
    library(
        studio,
        size,
        "unloaded-pages",
        (2000, 40),
        "",
        &[("scrolled-into-unloaded", Key::DownArrow, 6)],
    )?;
    library(studio, size, "empty", (0, 0), "", &[])?;
    library(
        studio,
        size,
        "error",
        (60, 30),
        "Couldn't load the rest of this library. The server had a problem.",
        &[],
    )
}

/// Home and the library at one size.
pub fn shoot(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    home_states(studio, size)?;
    home_failures(studio, size)?;
    library_states(studio, size)
}
