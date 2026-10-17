//! `LoginFlow::session` and `LoginAction::SessionExpired`: handing the
//! logged-in session to the library flow, and signing out when the server
//! rejects it mid-browse.

mod common;

use common::{Harness, saved, tokens_with, url};
use couchfin_core::fake::FakeJellyfin;
use couchfin_core::{
    ApiError, JellyfinApi, LogIn, LoginAction, LoginError, LoginScreen, Password, Request,
    ServerUrl,
};

fn server() -> ServerUrl {
    url("http://jellyfin.lan:8096")
}

fn fake() -> FakeJellyfin {
    FakeJellyfin::new(server())
        .with_server_info("Living Room", "10.11.2")
        .with_user("alice", "hunter2")
}

async fn logged_in() -> Harness {
    let harness = Harness::first_run(fake());
    harness.flow.start().await;
    harness
        .flow
        .dispatch(LoginAction::SubmitAddress("jellyfin.lan:8096".into()))
        .await;
    harness
        .flow
        .dispatch(LoginAction::SubmitLogin {
            username: "alice".into(),
            password: Password::new("hunter2"),
        })
        .await;
    assert!(matches!(harness.screen(), LoginScreen::Home(_)));
    harness
}

#[tokio::test]
async fn there_is_no_session_before_login() {
    let harness = Harness::first_run(fake());
    assert_eq!(harness.flow.session(), None);
    harness.flow.start().await;
    assert_eq!(harness.flow.session(), None);
}

#[tokio::test]
async fn the_session_is_available_after_login() {
    let harness = logged_in().await;
    let session = harness.flow.session().expect("on Home");
    assert_eq!(session.server(), &server());
    assert_eq!(
        Some(session.token().clone()),
        harness.stored_token(&server())
    );
    // And it works.
    assert!(harness.fake.current_user(&session).await.is_ok());
}

#[tokio::test]
async fn the_session_is_available_after_a_restore() {
    let fake = fake();
    let token = fake
        .authenticate_by_name(fake.server(), "alice", &Password::new("hunter2"))
        .await
        .expect("login")
        .access_token;
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );
    harness.flow.start().await;
    let session = harness.flow.session().expect("restored to Home");
    assert_eq!(session.token(), &token);
}

#[tokio::test]
async fn the_session_goes_with_log_out() {
    let harness = logged_in().await;
    harness.flow.dispatch(LoginAction::LogOut).await;
    assert_eq!(harness.flow.session(), None);
}

#[tokio::test]
async fn the_session_goes_with_change_server() {
    let harness = logged_in().await;
    harness.flow.dispatch(LoginAction::LogOut).await;
    harness.flow.dispatch(LoginAction::ChangeServer).await;
    assert_eq!(harness.flow.session(), None);
}

#[tokio::test]
async fn session_expired_forgets_the_token_and_says_why() {
    let harness = logged_in().await;

    harness.flow.dispatch(LoginAction::SessionExpired).await;

    let LoginScreen::LogIn(LogIn {
        server: form_server,
        username,
        request,
        ..
    }) = harness.screen()
    else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(form_server, server());
    assert_eq!(username, "alice");
    assert_eq!(
        request,
        Request::Failed(LoginError::Api(ApiError::TokenRejected))
    );
    assert_eq!(harness.stored_token(&server()), None);
    assert_eq!(harness.flow.session(), None);
}

#[tokio::test]
async fn a_second_session_expired_is_ignored() {
    let harness = logged_in().await;
    harness.flow.dispatch(LoginAction::SessionExpired).await;
    let before = harness.screen();
    let published = harness.seen().len();

    harness.flow.dispatch(LoginAction::SessionExpired).await;

    assert_eq!(harness.screen(), before);
    assert_eq!(harness.seen().len(), published);
}

#[tokio::test]
async fn session_expired_is_ignored_off_home() {
    let harness = Harness::first_run(fake());
    harness.flow.start().await;
    let before = harness.screen();
    harness.flow.dispatch(LoginAction::SessionExpired).await;
    assert_eq!(harness.screen(), before);
}
