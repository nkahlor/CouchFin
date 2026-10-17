//! Every error the login flow can show has plain wording, with no technical
//! detail. The wording functions match without a wildcard, so a new error
//! variant fails the build until it gets a message here.

use couchfin_core::{ApiError, LoginError, ServerUrl, ServerUrlError, TokenStoreError};
use couchfin_ui::wording;

const DETAIL: &str = "tcp connect error: Connection refused (os error 111)";

fn moved_to() -> ServerUrl {
    "https://jellyfin.example".parse().expect("valid URL")
}

/// Each API error with the wording a person on the sofa should read.
fn api_cases() -> Vec<(ApiError, String)> {
    vec![
        (
            ApiError::Unreachable {
                detail: DETAIL.into(),
            },
            "Couldn't reach the server. Check the address, and that the server is on.".into(),
        ),
        (
            ApiError::Timeout,
            "The server took too long to answer. Check that it's running, then try again.".into(),
        ),
        (
            ApiError::NotJellyfin {
                detail: DETAIL.into(),
            },
            "That address isn't a Jellyfin server. Check it and try again.".into(),
        ),
        (
            ApiError::InvalidCredentials,
            "Wrong username or password.".into(),
        ),
        (
            ApiError::TokenRejected,
            "You've been signed out. Please sign in again.".into(),
        ),
        (
            ApiError::Redirected {
                server: Some(moved_to()),
            },
            "The server has moved to https://jellyfin.example/. Check that's your server before you sign in."
                .into(),
        ),
        (
            ApiError::Redirected { server: None },
            "The server sent Couchfin somewhere unexpected. Check the address and try again.".into(),
        ),
        (
            ApiError::InsecureRedirect,
            "The server tried to switch to an unencrypted connection, so Couchfin stopped.".into(),
        ),
        (
            ApiError::ItemNotFound,
            "That's no longer in your library.".into(),
        ),
        (
            ApiError::Status { status: 503 },
            "The server had a problem. Try again in a moment.".into(),
        ),
    ]
}

#[test]
fn every_api_error_has_plain_wording() {
    for (error, expected) in api_cases() {
        assert_eq!(wording::api_error(&error), expected, "{error:?}");
    }
}

#[test]
fn login_errors_from_the_server_use_the_api_wording() {
    for (error, expected) in api_cases() {
        assert_eq!(wording::login_error(&LoginError::Api(error)), expected);
    }
}

#[test]
fn every_invalid_address_has_plain_wording() {
    let cases = [
        (ServerUrlError::Empty, "Type your server's address."),
        (
            ServerUrlError::Invalid("relative URL without a base".into()),
            "That doesn't look like a server address. Check it and try again.",
        ),
        (
            ServerUrlError::UnsupportedScheme("ftp".into()),
            "Use an address that starts with http:// or https://.",
        ),
        (
            ServerUrlError::HasCredentials,
            "Leave your username and password out of the address. You'll sign in next.",
        ),
    ];
    for (error, expected) in cases {
        let error = LoginError::InvalidAddress(error);
        assert_eq!(wording::login_error(&error), expected, "{error:?}");
    }
}

#[test]
fn every_token_store_error_warns_that_the_sign_in_wasnt_saved() {
    let expected =
        "Couchfin couldn't save your sign-in, so you'll need to sign in again next time.";
    for error in [
        TokenStoreError::Unavailable {
            detail: DETAIL.into(),
        },
        TokenStoreError::Io {
            detail: DETAIL.into(),
        },
        TokenStoreError::Invalid {
            detail: DETAIL.into(),
        },
    ] {
        assert_eq!(wording::token_not_saved(&error), expected, "{error:?}");
    }
}

#[test]
fn no_wording_carries_technical_detail() {
    let mut messages: Vec<String> = api_cases()
        .iter()
        .map(|(error, _)| wording::api_error(error))
        .collect();
    messages.push(wording::login_error(&LoginError::InvalidAddress(
        ServerUrlError::Invalid(DETAIL.into()),
    )));
    messages.push(wording::login_error(&LoginError::InvalidAddress(
        ServerUrlError::UnsupportedScheme("gopher".into()),
    )));
    for message in messages {
        for detail in ["os error", "Connection refused", "503", "gopher", "HTTP"] {
            assert!(!message.contains(detail), "{message:?} shows {detail:?}");
        }
    }
}
