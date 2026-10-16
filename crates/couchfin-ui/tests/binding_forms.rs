//! `render` and `bind` with the address and login forms: prefills, busy and
//! failed requests, the actions they send, and clearing the password.

mod common;

use common::flow::{SERVER, bound, enter_address, failed, log_in, settle};
use common::{labelled, press, shows, the, type_text};
use couchfin_core::{ApiError, LoginError, Request, ServerUrlError};
use couchfin_ui::render;
use slint::platform::Key;

const UNREACHABLE: &str =
    "Couldn't reach the server. Check the address, and that the server is on.";
const WRONG_PASSWORD: &str = "Wrong username or password.";

#[test]
fn the_address_form_is_prefilled_and_submits_what_is_typed() {
    let (window, commands) = bound(&enter_address("jellyfin.lan", Request::Idle));
    assert_eq!(
        the(&window, "Server address").accessible_value().as_deref(),
        Some("jellyfin.lan")
    );
    type_text(&window, ":8096");
    press(&window, Key::Return);
    assert_eq!(commands.taken(), ["SubmitAddress(jellyfin.lan:8096)"]);
}

#[test]
fn back_on_the_idle_address_form_quits() {
    let (window, commands) = bound(&enter_address("", Request::Idle));
    press(&window, Key::Escape);
    assert_eq!(commands.taken(), ["Quit"]);
}

#[test]
fn back_while_checking_an_address_cancels_the_check() {
    let (window, commands) = bound(&enter_address("jellyfin.lan", Request::InFlight));
    assert!(shows(&window, "Checking the server…"));
    press(&window, Key::Return);
    press(&window, Key::Escape);
    assert_eq!(commands.taken(), ["ChangeServer"]);
}

#[test]
fn a_failed_address_check_shows_its_wording_and_stops_the_progress() {
    let (window, _) = bound(&enter_address("jellyfin.lan", Request::InFlight));
    let unreachable = ApiError::Unreachable {
        detail: "connection refused".into(),
    };
    render(&window, &enter_address("jellyfin.lan", failed(unreachable)));
    assert!(shows(&window, UNREACHABLE));
    assert!(labelled(&window, "Checking the server…").is_empty());
}

#[test]
fn an_invalid_address_shows_its_wording() {
    let request = Request::Failed(LoginError::InvalidAddress(ServerUrlError::Empty));
    let (window, _) = bound(&enter_address("", request));
    assert!(shows(&window, "Type your server's address."));
}

#[test]
fn the_login_form_shows_where_the_password_goes_and_the_last_username() {
    let (window, _) = bound(&log_in(Request::Idle));
    assert!(shows(&window, "Sign in to Living Room"));
    assert!(shows(&window, &format!("Connected to {SERVER}")));
    assert_eq!(
        the(&window, "Username").accessible_value().as_deref(),
        Some("alice")
    );
}

#[test]
fn signing_in_sends_the_username_and_password() {
    let (window, commands) = bound(&log_in(Request::Idle));
    press(&window, Key::Return); // username is prefilled; move on
    type_text(&window, "hunter2");
    press(&window, Key::Return);
    assert_eq!(commands.taken(), ["SubmitLogin(alice, hunter2)"]);
}

#[test]
fn back_on_the_login_form_changes_server() {
    let (window, commands) = bound(&log_in(Request::Idle));
    press(&window, Key::Escape);
    the(&window, "Change server").invoke_accessible_default_action();
    assert_eq!(commands.taken(), ["ChangeServer", "ChangeServer"]);
}

#[test]
fn signing_in_shows_progress() {
    let (window, _) = bound(&log_in(Request::InFlight));
    assert!(shows(&window, "Signing in…"));
}

#[test]
fn a_failed_sign_in_shows_its_wording_and_clears_the_password() {
    let (window, commands) = bound(&log_in(Request::Idle));
    press(&window, Key::Tab);
    type_text(&window, "wrong");
    render(&window, &log_in(Request::InFlight));
    render(&window, &log_in(failed(ApiError::InvalidCredentials)));
    settle();
    assert!(shows(&window, WRONG_PASSWORD));
    assert!(labelled(&window, "Signing in…").is_empty());

    // The field is empty and still focused, ready for another try.
    type_text(&window, "right");
    press(&window, Key::Return);
    assert_eq!(commands.taken(), ["SubmitLogin(alice, right)"]);
}

#[test]
fn every_failure_clears_the_password_even_with_the_same_wording() {
    let (window, commands) = bound(&log_in(Request::Idle));
    press(&window, Key::Tab);
    for attempt in ["one", "two"] {
        type_text(&window, attempt);
        render(&window, &log_in(Request::InFlight));
        render(&window, &log_in(failed(ApiError::InvalidCredentials)));
        settle();
    }
    type_text(&window, "three");
    press(&window, Key::Return);
    assert_eq!(commands.taken(), ["SubmitLogin(alice, three)"]);
}

#[test]
fn a_rejected_token_explains_why_it_asks_again() {
    let (window, _) = bound(&log_in(failed(ApiError::TokenRejected)));
    assert!(shows(
        &window,
        "You've been signed out. Please sign in again."
    ));
}
