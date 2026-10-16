//! Entering an address, logging in, logging out and changing server, against
//! `FakeJellyfin`.

mod common;

use common::{Harness, saved, token, tokens_with, url};
use couchfin_core::fake::FakeJellyfin;
use couchfin_core::{
    ApiError, EnterAddress, Home, LogIn, LoginAction, LoginError, LoginScreen, Password, Request,
    ServerUrl, ServerUrlError, TokenStoreError,
};

fn server() -> ServerUrl {
    url("http://jellyfin.lan:8096")
}

fn fake() -> FakeJellyfin {
    FakeJellyfin::new(server())
        .with_server_info("Living Room", "10.11.2")
        .with_user("alice", "hunter2")
}

fn address(text: &str) -> LoginAction {
    LoginAction::SubmitAddress(text.to_owned())
}

fn login(username: &str, password: &str) -> LoginAction {
    LoginAction::SubmitLogin {
        username: username.to_owned(),
        password: Password::new(password),
    }
}

/// A first run that has got as far as the address screen.
async fn at_address_screen(fake: FakeJellyfin) -> Harness {
    let harness = Harness::first_run(fake);
    harness.flow.start().await;
    harness
}

/// A first run that has checked the server and shows the login form.
async fn at_login_screen() -> Harness {
    let harness = at_address_screen(fake()).await;
    harness.flow.dispatch(address("jellyfin.lan:8096")).await;
    harness
}

#[tokio::test]
async fn a_checked_address_shows_the_login_form_with_the_server_name() {
    let harness = at_address_screen(fake()).await;

    harness.flow.dispatch(address(" jellyfin.lan:8096 ")).await;

    let LoginScreen::LogIn(login) = harness.screen() else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(login.server, server());
    assert_eq!(login.info.server_name, "Living Room");
    assert_eq!(login.username, "");
    assert_eq!(login.request, Request::Idle);
    assert_eq!(harness.stored().server, Some(server()));
    // While the check ran, the address screen showed it in flight.
    assert!(
        harness
            .seen()
            .contains(&LoginScreen::EnterAddress(EnterAddress {
                address: " jellyfin.lan:8096 ".to_owned(),
                request: Request::InFlight,
            }))
    );
}

#[tokio::test]
async fn the_canonical_address_is_used_and_stored_after_a_redirect() {
    let canonical = url("https://jellyfin.lan/jellyfin");
    let fake = FakeJellyfin::new(canonical.clone())
        .with_redirect_from(server())
        .with_user("alice", "hunter2");
    let harness = at_address_screen(fake).await;

    harness.flow.dispatch(address("jellyfin.lan:8096")).await;
    harness.flow.dispatch(login("alice", "hunter2")).await;

    let LoginScreen::Home(home) = harness.screen() else {
        panic!("expected Home, got {:?}", harness.screen());
    };
    assert_eq!(home.server, canonical);
    assert_eq!(harness.stored().server, Some(canonical.clone()));
    assert!(harness.stored_token(&canonical).is_some());
}

#[tokio::test]
async fn an_invalid_address_is_refused_without_a_request() {
    let harness = at_address_screen(fake()).await;

    harness.flow.dispatch(address("ftp://jellyfin.lan")).await;

    assert_eq!(
        harness.screen(),
        LoginScreen::EnterAddress(EnterAddress {
            address: "ftp://jellyfin.lan".to_owned(),
            request: Request::Failed(LoginError::InvalidAddress(
                ServerUrlError::UnsupportedScheme("ftp".to_owned())
            )),
        })
    );
}

#[tokio::test]
async fn an_unreachable_address_shows_the_error_on_the_address_screen() {
    let harness = at_address_screen(fake()).await;

    harness.flow.dispatch(address("elsewhere.lan")).await;

    let LoginScreen::EnterAddress(EnterAddress { address, request }) = harness.screen() else {
        panic!("expected EnterAddress, got {:?}", harness.screen());
    };
    assert_eq!(address, "elsewhere.lan");
    assert!(
        matches!(
            request,
            Request::Failed(LoginError::Api(ApiError::Unreachable { .. }))
        ),
        "{request:?}"
    );
}

#[tokio::test]
async fn a_login_goes_home_and_stores_the_session() {
    let harness = at_login_screen().await;

    harness.flow.dispatch(login("alice", "hunter2")).await;

    let LoginScreen::Home(Home {
        server: at,
        info,
        user,
        token_not_saved,
    }) = harness.screen()
    else {
        panic!("expected Home, got {:?}", harness.screen());
    };
    assert_eq!(at, server());
    assert_eq!(info.server_name, "Living Room");
    assert_eq!(user.name, "alice");
    assert_eq!(token_not_saved, None);
    assert!(harness.stored_token(&server()).is_some());
    assert_eq!(harness.stored().username.as_deref(), Some("alice"));
}

#[tokio::test]
async fn a_token_that_cannot_be_stored_still_goes_home_and_says_so() {
    let harness = at_login_screen().await;
    let broken = TokenStoreError::Io {
        detail: "read-only file system".into(),
    };
    harness.tokens.fail_with(broken.clone());

    harness.flow.dispatch(login("alice", "hunter2")).await;

    let LoginScreen::Home(home) = harness.screen() else {
        panic!("expected Home, got {:?}", harness.screen());
    };
    // The UI can warn that the next start will ask to log in again.
    assert_eq!(home.token_not_saved, Some(broken));
}

#[tokio::test]
async fn a_wrong_password_shows_the_error_and_keeps_the_username() {
    let harness = at_login_screen().await;

    harness.flow.dispatch(login("alice", "wrong")).await;

    let LoginScreen::LogIn(login) = harness.screen() else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(login.username, "alice");
    assert_eq!(
        login.request,
        Request::Failed(LoginError::Api(ApiError::InvalidCredentials))
    );
    assert_eq!(harness.stored_token(&server()), None);
}

#[tokio::test]
async fn a_failed_recheck_after_a_redirected_login_goes_back_to_the_address() {
    let harness = at_login_screen().await;
    // Every call now redirects, so the re-check of the address fails as well.
    let moved = ApiError::Redirected {
        server: Some(url("https://jellyfin.lan")),
    };
    harness.fake.fail_with(moved.clone());

    harness.flow.dispatch(login("alice", "hunter2")).await;

    assert_eq!(
        harness.screen(),
        LoginScreen::EnterAddress(EnterAddress {
            address: "http://jellyfin.lan:8096/".to_owned(),
            request: Request::Failed(LoginError::Api(moved)),
        })
    );
}

#[tokio::test]
async fn log_out_deletes_the_token_and_shows_the_login_form() {
    let harness = at_login_screen().await;
    harness.flow.dispatch(login("alice", "hunter2")).await;

    harness.flow.dispatch(LoginAction::LogOut).await;

    let LoginScreen::LogIn(LogIn {
        username, request, ..
    }) = harness.screen()
    else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(username, "alice");
    assert_eq!(request, Request::Idle);
    assert_eq!(harness.stored_token(&server()), None);
}

#[tokio::test]
async fn change_server_from_the_login_form_goes_back_to_the_address() {
    let harness = at_login_screen().await;

    harness.flow.dispatch(LoginAction::ChangeServer).await;

    assert_eq!(
        harness.screen(),
        LoginScreen::EnterAddress(EnterAddress {
            address: "http://jellyfin.lan:8096/".to_owned(),
            request: Request::Idle,
        })
    );
}

#[tokio::test]
async fn switching_to_another_server_drops_the_old_servers_token() {
    // The saved server is gone, so the restore fails and the user picks the
    // fake's address instead.
    let old = url("http://old.lan:8096");
    let harness = Harness::new(
        fake(),
        &saved(&old, "alice"),
        tokens_with(&old, &token("old-token")),
    );
    harness.flow.start().await;
    harness.flow.dispatch(LoginAction::ChangeServer).await;

    harness.flow.dispatch(address("jellyfin.lan:8096")).await;

    assert!(matches!(harness.screen(), LoginScreen::LogIn(_)));
    assert_eq!(harness.stored().server, Some(server()));
    assert_eq!(harness.stored_token(&old), None);
}

#[tokio::test]
async fn an_unreachable_address_changes_nothing_stored() {
    let old = url("http://old.lan:8096");
    let harness = Harness::new(
        fake(),
        &saved(&old, "alice"),
        tokens_with(&old, &token("old-token")),
    );
    harness.flow.start().await;
    harness.flow.dispatch(LoginAction::ChangeServer).await;

    harness.flow.dispatch(address("elsewhere.lan")).await;

    assert_eq!(harness.stored().server, Some(old.clone()));
    assert!(harness.stored_token(&old).is_some());
}

#[tokio::test]
async fn actions_that_do_not_fit_the_screen_are_ignored() {
    let harness = at_address_screen(fake()).await;
    let before = harness.screen();

    harness.flow.dispatch(login("alice", "hunter2")).await;
    harness.flow.dispatch(LoginAction::LogOut).await;
    harness.flow.dispatch(LoginAction::Retry).await;
    harness.flow.dispatch(LoginAction::ChangeServer).await;

    assert_eq!(harness.screen(), before);
}

#[test]
fn dispatch_futures_can_be_spawned_on_tokio() {
    fn assert_send<T: Send + 'static>(_: T) {}
    let harness = Harness::first_run(fake());
    let flow = harness.flow;
    assert_send(async move { flow.dispatch(LoginAction::Retry).await });
}
