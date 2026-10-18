//! Every row on screen loads its pictures, not just the focused row and its
//! neighbours, and a missing picture is told apart from one still loading
//! (#49).

mod common;

use common::library::{Shell, World, picture};
use couchfin_core::{ImageLoad, LibraryState, RowKind, ScreenState};
use couchfin_ui::{HomeView, ImageState};
use slint::{ComponentHandle, LogicalSize, Model};

const ROWS: usize = 6;

/// Home with `ROWS` rows of distinct pictures; the last card of row 0 has
/// no picture at all.
fn rows() -> LibraryState {
    let mut state = World::new(40).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    let template = home
        .rows
        .iter()
        .find(|row| row.title.starts_with("Recently Added"))
        .expect("a Recently Added row")
        .clone();
    home.rows = (0..ROWS)
        .map(|r| {
            let mut row = template.clone();
            row.title = format!("Row {r}");
            row.kind = RowKind::RecentlyAdded(format!("lib-{r}").parse().expect("id"));
            for (n, card) in row.cards.iter_mut().enumerate() {
                if let Some(image) = &mut card.image {
                    image.item = format!("r{r}c{n}").parse().expect("id");
                }
            }
            row
        })
        .collect();
    home.rows[0].cards[5].image = None;
    home.rows[0].cards[5].placeholder = None;
    state
}

fn tall_shell() -> Shell {
    let shell = Shell::new();
    // Tall enough for every row to be on screen at once.
    shell
        .window
        .window()
        .set_size(LogicalSize::new(1920.0, 4200.0));
    shell
}

fn state_of(shell: &Shell, row: usize, index: usize) -> ImageState {
    shell
        .window
        .global::<HomeView>()
        .get_rows()
        .row_data(row)
        .and_then(|row| row.cards.row_data(index))
        .expect("a card")
        .image_state
}

#[test]
fn every_row_on_screen_loads_its_first_cards() {
    let shell = tall_shell();
    shell.show(rows());

    let pending = shell.loader.pending();

    for row in 0..ROWS {
        assert!(
            pending.contains(&format!("r{row}c0")),
            "row {row} is on screen but not loading: {pending:?}"
        );
    }
}

#[test]
fn rows_far_below_the_screen_still_wait() {
    let shell = Shell::new();
    shell
        .window
        .window()
        .set_size(LogicalSize::new(1920.0, 1080.0));
    let mut many = rows();
    let ScreenState::Home(home) = &mut many.screens[0] else {
        panic!("Home is first");
    };
    let template = home.rows[1].clone();
    for r in ROWS..ROWS + 6 {
        let mut row = template.clone();
        row.title = format!("Row {r}");
        row.kind = RowKind::RecentlyAdded(format!("lib-{r}").parse().expect("id"));
        for (n, card) in row.cards.iter_mut().enumerate() {
            if let Some(image) = &mut card.image {
                image.item = format!("r{r}c{n}").parse().expect("id");
            }
        }
        home.rows.push(row);
    }
    shell.show(many);

    let pending = shell.loader.pending();

    assert!(pending.contains(&"r0c0".to_owned()));
    assert!(
        !pending.contains(&format!("r{}c0", ROWS + 5)),
        "{pending:?}"
    );
}

#[test]
fn loading_ready_and_missing_are_three_different_states() {
    let shell = tall_shell();
    shell.show(rows());

    // No picture at all: missing from the start, never requested.
    assert_eq!(state_of(&shell, 0, 5), ImageState::Missing);
    // A picture on its way: loading, with its blurred placeholder.
    assert_eq!(state_of(&shell, 0, 0), ImageState::Loading);

    shell.loader.finish("r0c0", picture());
    shell.loader.finish("r0c1", ImageLoad::NoImage);
    shell.feed.pump();

    assert_eq!(state_of(&shell, 0, 0), ImageState::Ready);
    assert_eq!(state_of(&shell, 0, 1), ImageState::Missing);
    assert_eq!(state_of(&shell, 0, 2), ImageState::Loading);
}
