//! Startup: restoring a stored session, or asking for an address or a login
//! when there is none. Runs against `FakeJellyfin`.

mod common;

use common::{Harness, saved, token, tokens_with, url};
use couchfin_core::fake::{FakeJellyfin, MemoryTokens};
use couchfin_core::{
    AccessToken, ApiError, EnterAddress, JellyfinApi, LogIn, LoginAction, LoginError, LoginScreen,
    Password, Request, RestoreStatus, Restoring, ServerUrl, TokenStoreError,
};

fn server() -> ServerUrl {
    url("http://jellyfin.lan:8096")
}

fn fake() -> FakeJellyfin {
    FakeJellyfin::new(server())
        .with_server_info("Living Room", "10.11.2")
        .with_user("alice", "hunter2")
}

/// A token the fake has issued, as an earlier run would have stored.
async fn issued(fake: &FakeJellyfin) -> AccessToken {
    fake.authenticate_by_name(fake.server(), "alice", &Password::new("hunter2"))
        .await
        .expect("the fake knows alice")
        .access_token
}

#[tokio::test]
async fn a_new_flow_is_starting_until_started() {
    let harness = Harness::first_run(fake());
    assert_eq!(harness.screen(), LoginScreen::Starting);
}

#[tokio::test]
async fn first_run_asks_for_the_address() {
    let harness = Harness::first_run(fake());

    harness.flow.start().await;

    assert_eq!(
        harness.screen(),
        LoginScreen::EnterAddress(EnterAddress {
            address: String::new(),
            request: Request::Idle,
        })
    );
}

#[tokio::test]
async fn a_valid_stored_token_skips_login_and_goes_home() {
    let fake = fake();
    let token = issued(&fake).await;
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );

    harness.flow.start().await;

    let LoginScreen::Home(home) = harness.screen() else {
        panic!("expected Home, got {:?}", harness.screen());
    };
    assert_eq!(home.server, server());
    assert_eq!(home.info.server_name, "Living Room");
    assert_eq!(home.info.version, "10.11.2");
    assert_eq!(home.user.name, "alice");
    // The UI saw the check in progress first.
    assert_eq!(
        harness.seen()[0],
        LoginScreen::Restoring(Restoring {
            server: server(),
            status: RestoreStatus::Checking,
        })
    );
}

#[tokio::test]
async fn a_revoked_token_is_deleted_and_the_login_is_asked_for_again() {
    let fake = fake();
    let token = issued(&fake).await;
    fake.revoke_tokens();
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );

    harness.flow.start().await;

    let LoginScreen::LogIn(login) = harness.screen() else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(login.server, server());
    assert_eq!(login.info.server_name, "Living Room");
    assert_eq!(login.username, "alice");
    assert_eq!(
        login.request,
        Request::Failed(LoginError::Api(ApiError::TokenRejected))
    );
    assert_eq!(harness.stored_token(&server()), None);
}

#[tokio::test]
async fn an_unreachable_server_keeps_the_token_and_offers_a_retry() {
    let fake = fake();
    let token = issued(&fake).await;
    fake.fail_with(ApiError::Timeout);
    let harness = Harness::new(
        fake.clone(),
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );

    harness.flow.start().await;

    assert_eq!(
        harness.screen(),
        LoginScreen::Restoring(Restoring {
            server: server(),
            status: RestoreStatus::Failed(ApiError::Timeout),
        })
    );
    assert_eq!(harness.stored_token(&server()), Some(token));

    fake.clear_failure();
    harness.flow.dispatch(LoginAction::Retry).await;
    assert!(matches!(harness.screen(), LoginScreen::Home(_)));
}

#[tokio::test]
async fn change_server_after_a_failed_restore_offers_the_saved_address() {
    let fake = fake();
    let token = issued(&fake).await;
    fake.fail_with(ApiError::Timeout);
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );
    harness.flow.start().await;

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
async fn a_saved_server_without_a_token_goes_to_its_login() {
    let harness = Harness::new(fake(), &saved(&server(), "alice"), MemoryTokens::new());

    harness.flow.start().await;

    let LoginScreen::LogIn(login) = harness.screen() else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(login.username, "alice");
    assert_eq!(login.request, Request::Idle);
}

#[tokio::test]
async fn a_moved_server_drops_the_old_token_and_asks_to_log_in_at_the_new_address() {
    let new = url("https://jellyfin.example.com");
    let fake = FakeJellyfin::new(new.clone())
        .with_redirect_from(server())
        .with_user("alice", "hunter2");
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token("old-token")),
    );

    harness.flow.start().await;

    let LoginScreen::LogIn(LogIn {
        server: at,
        request,
        ..
    }) = harness.screen()
    else {
        panic!("expected LogIn, got {:?}", harness.screen());
    };
    assert_eq!(at, new);
    assert_eq!(
        request,
        Request::Failed(LoginError::Api(ApiError::Redirected {
            server: Some(new.clone())
        }))
    );
    assert_eq!(harness.stored_token(&server()), None);
    assert_eq!(harness.stored().server, Some(new));
}

#[tokio::test]
async fn a_token_store_that_fails_to_load_means_logging_in_again() {
    let tokens = MemoryTokens::new();
    tokens.fail_with(TokenStoreError::Unavailable {
        detail: "locked".into(),
    });
    let harness = Harness::new(fake(), &saved(&server(), "alice"), tokens);

    harness.flow.start().await;

    assert!(matches!(harness.screen(), LoginScreen::LogIn(_)));
}

#[tokio::test]
async fn start_only_runs_once() {
    let fake = fake();
    let token = issued(&fake).await;
    let harness = Harness::new(
        fake,
        &saved(&server(), "alice"),
        tokens_with(&server(), &token),
    );
    harness.flow.start().await;
    let published = harness.seen().len();

    harness.flow.start().await;

    assert_eq!(harness.seen().len(), published);
}
