//! The Phase 1 home screen on Slint's testing backend: it shows the server
//! and who is signed in, and Back asks to go back.

mod common;

use common::{Calls, labelled, press, shown, shows};
use couchfin_ui::{HomeView, MainWindow, Screen};
use slint::ComponentHandle;
use slint::platform::Key;

fn home_screen() -> (MainWindow, Calls<()>) {
    let window = shown(Screen::Home);
    let state = window.global::<HomeView>();
    state.set_server_name("Living Room".into());
    state.set_server_version("10.11.2".into());
    state.set_username("alice".into());
    let backs = Calls::new();
    let recorder = backs.clone();
    state.on_back(move || recorder.record(()));
    (window, backs)
}

#[test]
fn shows_the_server_name_version_and_username() {
    let (window, _) = home_screen();
    assert!(shows(&window, "Living Room"));
    assert!(shows(&window, "Jellyfin 10.11.2"));
    assert!(shows(&window, "Signed in as alice"));
}

#[test]
fn escape_and_backspace_ask_to_go_back() {
    let (window, backs) = home_screen();
    press(&window, Key::Escape);
    press(&window, Key::Backspace);
    assert_eq!(backs.taken().len(), 2);
}

#[test]
fn switching_to_home_takes_focus_for_back() {
    let window = shown(Screen::Login);
    let backs = Calls::new();
    let recorder = backs.clone();
    window
        .global::<HomeView>()
        .on_back(move || recorder.record(()));
    window.set_screen(Screen::Home);
    press(&window, Key::Escape);
    assert_eq!(backs.taken().len(), 1);
}

#[test]
fn only_the_current_screen_is_shown() {
    let window = shown(Screen::Home);
    assert!(labelled(&window, "Server address").is_empty());
    assert!(labelled(&window, "Password").is_empty());
    window.set_screen(Screen::Address);
    assert!(shows(&window, "Server address"));
    assert!(labelled(&window, "Password").is_empty());
}
