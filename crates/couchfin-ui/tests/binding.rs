//! `render` and `bind` with real login-flow screens: startup, a failed
//! restore, and Home. Forms are in `binding_forms.rs`.

mod common;

use common::flow::{SERVER, bound, home, restoring};
use common::{labelled, press, shows, the};
use couchfin_core::{ApiError, LoginScreen, RestoreStatus, TokenStoreError};
use couchfin_ui::render;
use slint::platform::Key;

#[test]
fn starting_shows_progress_and_back_quits() {
    let (window, commands) = bound(&LoginScreen::Starting);
    assert!(shows(&window, "Starting…"));
    press(&window, Key::Escape);
    assert_eq!(commands.taken(), ["Quit"]);
}

#[test]
fn restoring_shows_the_server_and_back_changes_server() {
    let (window, commands) = bound(&restoring(RestoreStatus::Checking));
    assert!(shows(&window, &format!("Connecting to {SERVER}…")));
    assert!(labelled(&window, "Retry").is_empty());
    press(&window, Key::Escape);
    assert_eq!(commands.taken(), ["ChangeServer"]);
}

#[test]
fn a_failed_restore_explains_and_offers_retry_then_change_server() {
    let (window, commands) = bound(&restoring(RestoreStatus::Failed(ApiError::Timeout)));
    assert!(shows(&window, &format!("Couldn't connect to {SERVER}")));
    assert!(shows(
        &window,
        "The server took too long to answer. Check that it's running, then try again."
    ));
    assert!(labelled(&window, &format!("Connecting to {SERVER}…")).is_empty());

    press(&window, Key::Return);
    press(&window, Key::Tab);
    press(&window, Key::Return);
    press(&window, Key::Escape);
    assert_eq!(commands.taken(), ["Retry", "ChangeServer", "Quit"]);

    the(&window, "Retry").invoke_accessible_default_action();
    the(&window, "Change server").invoke_accessible_default_action();
    assert_eq!(commands.taken(), ["Retry", "ChangeServer"]);
}

#[test]
fn retrying_goes_back_to_progress() {
    let (window, _) = bound(&restoring(RestoreStatus::Failed(ApiError::Timeout)));
    render(&window, &restoring(RestoreStatus::Checking));
    assert!(shows(&window, &format!("Connecting to {SERVER}…")));
    assert!(labelled(&window, "Retry").is_empty());
}

#[test]
fn home_shows_the_server_version_and_user() {
    let (window, _) = bound(&home(None));
    assert!(shows(&window, "Living Room"));
    assert!(shows(&window, "Jellyfin 10.11.2"));
    assert!(shows(&window, "Signed in as alice"));
    assert!(
        labelled(
            &window,
            "Couchfin couldn't save your sign-in, so you'll need to sign in again next time."
        )
        .is_empty()
    );
}

#[test]
fn home_notes_a_sign_in_that_couldnt_be_saved() {
    let error = TokenStoreError::Io {
        detail: "permission denied".into(),
    };
    let (window, _) = bound(&home(Some(error)));
    assert!(shows(
        &window,
        "Couchfin couldn't save your sign-in, so you'll need to sign in again next time."
    ));
}

#[test]
fn home_signs_out_with_its_button_and_back_quits() {
    let (window, commands) = bound(&home(None));
    // Arriving on Home must not put a press of Enter on Sign out.
    press(&window, Key::Return);
    assert_eq!(commands.taken().len(), 0);
    press(&window, Key::DownArrow);
    press(&window, Key::Return);
    press(&window, Key::Escape);
    the(&window, "Sign out").invoke_accessible_default_action();
    assert_eq!(commands.taken(), ["LogOut", "Quit", "LogOut"]);
}

#[test]
fn errors_never_show_their_log_detail() {
    let error = ApiError::Unreachable {
        detail: "dns error: failed to lookup address".into(),
    };
    let (window, _) = bound(&restoring(RestoreStatus::Failed(error)));
    for element in common::all_elements(&window) {
        let label = element.accessible_label().unwrap_or_default();
        assert!(!label.contains("dns error"), "{label:?}");
    }
}
