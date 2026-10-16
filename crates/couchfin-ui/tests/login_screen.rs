//! The login screen, driven by keyboard on Slint's testing backend: it shows
//! which server the password goes to, submits with Enter or the Sign in
//! button, moves focus in order, never exposes the password, and shows busy
//! and error states.

mod common;

use common::{Calls, all_elements, labelled, press, shift_tab, shown, shows, the, type_text};
use couchfin_ui::{LoginView, MainWindow, Screen};
use slint::ComponentHandle;
use slint::platform::Key;

const BUSY: &str = "Signing in…";
const SECRET: &str = "hunter2-very-secret";

type Login = (String, String);

/// The login screen for a server at `https://jellyfin.example/`, with its
/// submit and back callbacks recorded.
fn login_screen() -> (MainWindow, Calls<Login>, Calls<()>) {
    let window = shown(Screen::Login);
    let state = window.global::<LoginView>();
    state.set_server_name("Living Room".into());
    state.set_server_address("https://jellyfin.example/".into());
    let submits = Calls::new();
    let backs = Calls::new();
    let recorder = submits.clone();
    state.on_submit(move |user, password| recorder.record((user.into(), password.into())));
    let recorder = backs.clone();
    state.on_back(move || recorder.record(()));
    (window, submits, backs)
}

fn username(window: &MainWindow) -> String {
    window.global::<LoginView>().get_username().to_string()
}

fn login(user: &str, password: &str) -> Login {
    (user.to_owned(), password.to_owned())
}

#[test]
fn shows_the_server_name_and_the_address_the_password_goes_to() {
    let (window, _, _) = login_screen();
    assert!(shows(&window, "Sign in to Living Room"));
    assert!(shows(&window, "Connected to https://jellyfin.example/"));
}

#[test]
fn shows_a_prefilled_username() {
    let (window, _, _) = login_screen();
    window.global::<LoginView>().set_username("alice".into());
    assert_eq!(
        the(&window, "Username").accessible_value().as_deref(),
        Some("alice")
    );
}

#[test]
fn enter_moves_from_username_to_password_then_submits() {
    let (window, submits, _) = login_screen();
    type_text(&window, "alice");
    press(&window, Key::Return);
    assert!(
        submits.taken().is_empty(),
        "Enter on username only moves on"
    );
    type_text(&window, "pw");
    press(&window, Key::Return);
    assert_eq!(submits.taken(), [login("alice", "pw")]);
    assert_eq!(username(&window), "alice");
}

#[test]
fn the_sign_in_button_submits_the_typed_values() {
    let (window, submits, _) = login_screen();
    type_text(&window, "alice");
    press(&window, Key::Tab);
    type_text(&window, "pw");
    the(&window, "Sign in").invoke_accessible_default_action();
    assert_eq!(submits.taken(), [login("alice", "pw")]);
}

#[test]
fn an_empty_password_is_submitted_as_typed() {
    let (window, submits, _) = login_screen();
    type_text(&window, "alice");
    the(&window, "Sign in").invoke_accessible_default_action();
    assert_eq!(submits.taken(), [login("alice", "")]);
}

#[test]
fn tab_visits_username_password_sign_in_then_change_server() {
    let (window, submits, backs) = login_screen();
    type_text(&window, "u");
    press(&window, Key::Tab);
    type_text(&window, "p");
    press(&window, Key::Tab);
    type_text(&window, "x");
    assert_eq!(username(&window), "u");
    press(&window, Key::Return);
    assert_eq!(submits.taken(), [login("u", "p")], "Sign in was focused");
    press(&window, Key::Tab);
    press(&window, Key::Return);
    assert_eq!(backs.taken().len(), 1, "Change server was focused");
    assert_eq!(submits.taken().len(), 0);
}

#[test]
fn shift_tab_and_up_walk_back_to_username() {
    let (window, submits, _) = login_screen();
    for _ in 0..3 {
        press(&window, Key::Tab);
    }
    shift_tab(&window);
    press(&window, Key::Return);
    assert_eq!(submits.taken().len(), 1, "back on Sign in");
    press(&window, Key::UpArrow);
    type_text(&window, "p");
    press(&window, Key::UpArrow);
    type_text(&window, "u");
    assert_eq!(username(&window), "u");
    press(&window, Key::Return);
    press(&window, Key::Return);
    assert_eq!(submits.taken(), [login("u", "p")]);
}

#[test]
fn down_moves_focus_like_tab() {
    let (window, submits, backs) = login_screen();
    press(&window, Key::DownArrow);
    press(&window, Key::DownArrow);
    press(&window, Key::Return);
    assert_eq!(submits.taken().len(), 1);
    press(&window, Key::DownArrow);
    press(&window, Key::Return);
    assert_eq!(backs.taken().len(), 1);
}

#[test]
fn escape_asks_to_go_back() {
    let (window, _, backs) = login_screen();
    press(&window, Key::Escape);
    assert_eq!(backs.taken().len(), 1);
}

#[test]
fn the_change_server_button_asks_to_go_back() {
    let (window, _, backs) = login_screen();
    the(&window, "Change server").invoke_accessible_default_action();
    assert_eq!(backs.taken().len(), 1);
}

#[test]
fn the_password_never_shows_in_any_accessible_text() {
    let (window, _, _) = login_screen();
    press(&window, Key::Tab);
    type_text(&window, SECRET);

    let field = the(&window, "Password");
    assert_ne!(field.accessible_value().as_deref(), Some(SECRET));
    for element in all_elements(&window) {
        for text in [
            element.accessible_label(),
            element.accessible_value(),
            element.accessible_description(),
            element.accessible_placeholder_text(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                !text.contains("hunter2"),
                "{:?} exposes the password",
                element.id()
            );
        }
    }
}

#[test]
fn the_password_reaches_only_the_submit_callback() {
    let (window, submits, _) = login_screen();
    press(&window, Key::Tab);
    type_text(&window, SECRET);
    let state = window.global::<LoginView>();
    for visible in [
        state.get_username(),
        state.get_server_name(),
        state.get_server_address(),
        state.get_error(),
    ] {
        assert!(!visible.contains("hunter2"));
    }
    press(&window, Key::Return);
    assert_eq!(submits.taken(), [login("", SECRET)]);
}

#[test]
fn shows_and_hides_an_error() {
    let (window, _, _) = login_screen();
    let state = window.global::<LoginView>();
    state.set_error("Wrong username or password".into());
    assert!(shows(&window, "Wrong username or password"));
    state.set_error("".into());
    assert!(labelled(&window, "Wrong username or password").is_empty());
}

#[test]
fn busy_shows_progress_and_ignores_submit_and_typing() {
    let (window, submits, _) = login_screen();
    let state = window.global::<LoginView>();
    type_text(&window, "alice");
    assert!(labelled(&window, BUSY).is_empty());

    state.set_busy(true);
    assert!(shows(&window, BUSY));
    type_text(&window, "x");
    the(&window, "Sign in").invoke_accessible_default_action();
    assert_eq!(username(&window), "alice");
    assert_eq!(submits.taken().len(), 0);

    state.set_busy(false);
    assert!(labelled(&window, BUSY).is_empty());
    the(&window, "Sign in").invoke_accessible_default_action();
    assert_eq!(submits.taken(), [login("alice", "")]);
}

#[test]
fn switching_to_login_focuses_the_username() {
    let window = shown(Screen::Address);
    window.set_screen(Screen::Login);
    type_text(&window, "alice");
    assert_eq!(username(&window), "alice");
}
