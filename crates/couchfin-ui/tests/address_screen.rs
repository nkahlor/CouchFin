//! The server address screen, driven by keyboard on Slint's testing backend:
//! it shows the address, submits it with Enter or the Connect button, moves
//! focus in order, and shows busy and error states.

mod common;

use common::{Calls, labelled, press, shift_tab, shown, shows, the, type_text};
use couchfin_ui::{AddressView, MainWindow, Screen};
use slint::ComponentHandle;
use slint::platform::Key;

const BUSY: &str = "Checking the server…";

/// The address screen with its submit and back callbacks recorded.
fn address_screen() -> (MainWindow, Calls<String>, Calls<()>) {
    let window = shown(Screen::Address);
    let submits = Calls::new();
    let backs = Calls::new();
    let state = window.global::<AddressView>();
    let recorder = submits.clone();
    state.on_submit(move |address| recorder.record(address.to_string()));
    let recorder = backs.clone();
    state.on_back(move || recorder.record(()));
    (window, submits, backs)
}

fn address(window: &MainWindow) -> String {
    window.global::<AddressView>().get_address().to_string()
}

#[test]
fn shows_a_prefilled_address() {
    let (window, _, _) = address_screen();
    window
        .global::<AddressView>()
        .set_address("http://jellyfin.lan:8096".into());
    let field = the(&window, "Server address");
    assert_eq!(
        field.accessible_value().as_deref(),
        Some("http://jellyfin.lan:8096")
    );
}

#[test]
fn typing_goes_into_the_address_field() {
    let (window, _, _) = address_screen();
    type_text(&window, "jellyfin.lan");
    assert_eq!(address(&window), "jellyfin.lan");
}

#[test]
fn enter_submits_the_typed_address() {
    let (window, submits, _) = address_screen();
    type_text(&window, "jellyfin.lan");
    press(&window, Key::Return);
    assert_eq!(submits.taken(), ["jellyfin.lan"]);
}

#[test]
fn the_connect_button_submits_the_typed_address() {
    let (window, submits, _) = address_screen();
    type_text(&window, "jellyfin.lan");
    the(&window, "Connect").invoke_accessible_default_action();
    assert_eq!(submits.taken(), ["jellyfin.lan"]);
}

#[test]
fn tab_moves_from_the_field_to_connect_and_shift_tab_moves_back() {
    let (window, submits, _) = address_screen();
    type_text(&window, "a");
    press(&window, Key::Tab);
    type_text(&window, "x");
    assert_eq!(address(&window), "a", "focus left the field");
    press(&window, Key::Return);
    assert_eq!(submits.taken(), ["a"], "Enter pressed Connect");

    shift_tab(&window);
    type_text(&window, "b");
    assert_eq!(address(&window), "ab");
}

#[test]
fn arrow_keys_move_focus_like_tab() {
    let (window, submits, _) = address_screen();
    type_text(&window, "a");
    press(&window, Key::DownArrow);
    type_text(&window, "x");
    assert_eq!(address(&window), "a");
    press(&window, Key::Return);
    assert_eq!(submits.taken(), ["a"]);

    press(&window, Key::UpArrow);
    type_text(&window, "b");
    assert_eq!(address(&window), "ab");
}

#[test]
fn escape_asks_to_go_back_from_the_field_and_the_button() {
    let (window, _, backs) = address_screen();
    press(&window, Key::Escape);
    assert_eq!(backs.taken().len(), 1);
    press(&window, Key::Tab);
    press(&window, Key::Escape);
    assert_eq!(backs.taken().len(), 1);
}

#[test]
fn backspace_edits_the_field_instead_of_going_back() {
    let (window, _, backs) = address_screen();
    type_text(&window, "ab");
    press(&window, Key::Backspace);
    assert_eq!(address(&window), "a");
    assert_eq!(backs.taken().len(), 0);
}

#[test]
fn shows_and_hides_an_error() {
    let (window, _, _) = address_screen();
    let state = window.global::<AddressView>();
    assert!(labelled(&window, "No server answered at that address").is_empty());
    state.set_error("No server answered at that address".into());
    assert!(shows(&window, "No server answered at that address"));
    state.set_error("".into());
    assert!(labelled(&window, "No server answered at that address").is_empty());
}

#[test]
fn busy_shows_progress_and_ignores_submit_and_typing() {
    let (window, submits, _) = address_screen();
    let state = window.global::<AddressView>();
    type_text(&window, "a");
    assert!(labelled(&window, BUSY).is_empty());

    state.set_busy(true);
    assert!(shows(&window, BUSY));
    type_text(&window, "x");
    press(&window, Key::Return);
    the(&window, "Connect").invoke_accessible_default_action();
    assert_eq!(address(&window), "a");
    assert_eq!(submits.taken().len(), 0);

    state.set_busy(false);
    assert!(labelled(&window, BUSY).is_empty());
    press(&window, Key::Return);
    assert_eq!(submits.taken(), ["a"]);
}
