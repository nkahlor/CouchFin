//! The in-memory fake behaves like a Jellyfin server for the callers
//! (couchfin-core) that test against it instead of HTTP.

use couchfin_api::fake::FakeJellyfin;
use couchfin_api::{AccessToken, ApiError, JellyfinApi, Password, ServerUrl, Session};

fn url() -> ServerUrl {
    "http://fake.lan:8096".parse().expect("URL is valid")
}

fn fake() -> FakeJellyfin {
    FakeJellyfin::new(url())
        .with_server_info("Living Room", "10.11.2")
        .with_user("alice", "hunter2")
}

async fn log_in(fake: &FakeJellyfin) -> Session {
    let result = fake
        .authenticate_by_name(&url(), "alice", &Password::new("hunter2"))
        .await
        .expect("alice can log in");
    Session::new(url(), result.access_token)
}

#[tokio::test]
async fn reports_its_server_info() {
    let check = fake().check_server(&url()).await.unwrap();
    assert_eq!(check.server, url());
    let info = check.info;
    assert_eq!(info.server_name, "Living Room");
    assert_eq!(info.version, "10.11.2");
}

#[tokio::test]
async fn a_logged_in_session_finds_its_user() {
    let fake = fake();
    let session = log_in(&fake).await;

    let user = fake.current_user(&session).await.unwrap();

    assert_eq!(user.name, "alice");
}

#[tokio::test]
async fn each_login_gets_its_own_token() {
    let fake = fake();
    let first = log_in(&fake).await;
    let second = log_in(&fake).await;
    assert_ne!(first.token(), second.token());
}

#[tokio::test]
async fn a_wrong_password_or_unknown_user_is_invalid_credentials() {
    let fake = fake();
    for (user, password) in [("alice", "nope"), ("bob", "hunter2")] {
        let error = fake
            .authenticate_by_name(&url(), user, &Password::new(password))
            .await
            .unwrap_err();
        assert_eq!(error, ApiError::InvalidCredentials);
    }
}

#[tokio::test]
async fn an_unknown_or_revoked_token_is_rejected() {
    let fake = fake();
    let made_up = Session::new(url(), AccessToken::new("made-up").unwrap());
    assert_eq!(
        fake.current_user(&made_up).await.unwrap_err(),
        ApiError::TokenRejected
    );

    let session = log_in(&fake).await;
    fake.revoke_tokens();
    assert_eq!(
        fake.current_user(&session).await.unwrap_err(),
        ApiError::TokenRejected
    );
}

#[tokio::test]
async fn any_other_url_is_unreachable() {
    let other: ServerUrl = "http://elsewhere.lan".parse().unwrap();
    let error = fake().check_server(&other).await.unwrap_err();
    assert!(matches!(error, ApiError::Unreachable { .. }), "{error:?}");
}

#[tokio::test]
async fn a_scripted_failure_applies_to_every_call_until_cleared() {
    let fake = fake();
    let session = log_in(&fake).await;
    fake.fail_with(ApiError::Timeout);

    assert_eq!(
        fake.check_server(&url()).await.unwrap_err(),
        ApiError::Timeout
    );
    assert_eq!(
        fake.authenticate_by_name(&url(), "alice", &Password::new("hunter2"))
            .await
            .unwrap_err(),
        ApiError::Timeout
    );
    assert_eq!(
        fake.current_user(&session).await.unwrap_err(),
        ApiError::Timeout
    );

    fake.clear_failure();
    assert!(fake.current_user(&session).await.is_ok());
}

#[tokio::test]
async fn clones_share_one_server() {
    let fake = fake();
    let handle = fake.clone();
    let session = log_in(&fake).await;
    handle.revoke_tokens();
    assert_eq!(
        fake.current_user(&session).await.unwrap_err(),
        ApiError::TokenRejected
    );
}

#[tokio::test]
async fn defaults_describe_a_plausible_server() {
    let fake = FakeJellyfin::new(url());
    let info = fake.check_server(&url()).await.unwrap().info;
    assert_ne!(info.server_name, "");
    assert_ne!(info.version, "");
    assert_eq!(fake.server(), &url());
}

#[tokio::test]
async fn an_old_address_redirects_to_the_canonical_one() {
    let old: ServerUrl = "http://old.lan:8096".parse().unwrap();
    let fake = fake().with_redirect_from(old.clone());

    let check = fake.check_server(&old).await.unwrap();
    assert_eq!(check.server, url());

    let redirected = ApiError::Redirected {
        server: Some(url()),
    };
    assert_eq!(
        fake.authenticate_by_name(&old, "alice", &Password::new("hunter2"))
            .await
            .unwrap_err(),
        redirected
    );
    let session = log_in(&fake).await;
    let old_session = Session::new(old, session.token().clone());
    assert_eq!(
        fake.current_user(&old_session).await.unwrap_err(),
        redirected
    );
}
