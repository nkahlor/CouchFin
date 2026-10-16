//! Phase 1's "Done when" checks, end to end: the real `JellyfinClient`
//! against a wiremock server, with settings and tokens in a temporary config
//! directory. The keyring is unavailable, as under gamescope, so the token
//! goes to the 0600 file.

use couchfin_api::JellyfinClient;
use couchfin_core::{
    ApiError, ConfigDir, EnterAddress, FallbackTokens, FileSettings, FileTokens, KeyringTokens,
    LoginAction, LoginError, LoginFlow, LoginScreen, Password, Request, RestoreStatus, Restoring,
    ServerUrl, Settings, TokenStore,
};
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request as HttpRequest, ResponseTemplate};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

type Tokens = FallbackTokens<KeyringTokens, FileTokens>;
type Flow = LoginFlow<JellyfinClient, FileSettings, Tokens>;

/// One launch of the app over the config directory `dir`.
fn launch(dir: &TempDir) -> Flow {
    let config = ConfigDir::new(dir.path());
    let settings_file = FileSettings::new(&config);
    let settings = Settings::load_or_create(&settings_file);
    let api = JellyfinClient::new(settings.device_id.clone()).expect("client builds");
    let tokens = FallbackTokens::new(
        KeyringTokens::unavailable("no Secret Service in tests"),
        FileTokens::new(&config),
    );
    LoginFlow::new(api, settings, settings_file, tokens)
}

fn file_tokens(dir: &TempDir) -> FileTokens {
    FileTokens::new(&ConfigDir::new(dir.path()))
}

fn server_url(server: &MockServer) -> ServerUrl {
    server.uri().parse().expect("wiremock URI is valid")
}

/// An address where nothing listens. (A dropped `MockServer` goes back to
/// wiremock's pool and keeps answering.)
fn closed_port() -> ServerUrl {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("can bind a local port");
    let port = listener.local_addr().expect("socket has an address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
        .parse()
        .expect("local URL is valid")
}

fn sends_token(request: &HttpRequest) -> bool {
    request
        .headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.ends_with(&format!(", Token=\"{TOKEN}\"")))
}

async fn jellyfin() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ServerName": "Living Room",
            "Version": "10.11.2",
            "Id": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
        })))
        .mount(&server)
        .await;
    server
}

fn user_json() -> serde_json::Value {
    json!({ "Id": "38a5a5bbdc3549ccb0b07fc7e43c1d43", "Name": "alice" })
}

async fn accept_login(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "User": user_json(),
            "AccessToken": TOKEN,
            "ServerId": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
        })))
        .mount(server)
        .await;
}

async fn log_in(flow: &Flow, server: &MockServer, password: &str) {
    flow.start().await;
    flow.dispatch(LoginAction::SubmitAddress(server.uri()))
        .await;
    flow.dispatch(LoginAction::SubmitLogin {
        username: "alice".into(),
        password: Password::new(password),
    })
    .await;
}

#[tokio::test]
async fn after_a_restart_the_stored_token_skips_login_and_users_me_answers() {
    let server = jellyfin().await;
    accept_login(&server).await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .and(sends_token)
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json()))
        .expect(2) // once after logging in, once after the restart
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&dir);
    log_in(&first, &server, "hunter2").await;
    assert!(matches!(first.screen(), LoginScreen::Home(_)));
    drop(first);

    let second = launch(&dir);
    second.start().await;

    let LoginScreen::Home(home) = second.screen() else {
        panic!("expected Home, got {:?}", second.screen());
    };
    assert_eq!(home.server, server_url(&server));
    assert_eq!(home.info.server_name, "Living Room");
    assert_eq!(home.info.version, "10.11.2");
    assert_eq!(home.user.name, "alice");
}

#[tokio::test]
async fn a_wrong_password_shows_the_error_on_the_login_form() {
    let server = jellyfin().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let flow = launch(&dir);

    log_in(&flow, &server, "wrong").await;

    let LoginScreen::LogIn(login) = flow.screen() else {
        panic!("expected LogIn, got {:?}", flow.screen());
    };
    assert_eq!(
        login.request,
        Request::Failed(LoginError::Api(ApiError::InvalidCredentials))
    );
    assert_eq!(file_tokens(&dir).load(&server_url(&server)), Ok(None));
}

#[tokio::test]
async fn an_unreachable_address_shows_the_error_on_the_address_screen() {
    let address = closed_port().to_string();
    let dir = tempfile::tempdir().unwrap();
    let flow = launch(&dir);
    flow.start().await;

    flow.dispatch(LoginAction::SubmitAddress(address)).await;

    let LoginScreen::EnterAddress(EnterAddress { request, .. }) = flow.screen() else {
        panic!("expected EnterAddress, got {:?}", flow.screen());
    };
    assert!(
        matches!(
            request,
            Request::Failed(LoginError::Api(ApiError::Unreachable { .. }))
        ),
        "{request:?}"
    );
}

#[tokio::test]
async fn a_revoked_token_goes_back_to_the_login_form_and_is_deleted() {
    let server = jellyfin().await;
    accept_login(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&dir);
    // Logging in reaches /Users/Me once, which then says the token is gone.
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json()))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    log_in(&first, &server, "hunter2").await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    drop(first);

    let second = launch(&dir);
    second.start().await;

    let LoginScreen::LogIn(login) = second.screen() else {
        panic!("expected LogIn, got {:?}", second.screen());
    };
    assert_eq!(login.username, "alice");
    assert_eq!(
        login.request,
        Request::Failed(LoginError::Api(ApiError::TokenRejected))
    );
    assert_eq!(file_tokens(&dir).load(&server_url(&server)), Ok(None));
}

#[tokio::test]
async fn a_server_that_is_down_at_startup_keeps_the_token_for_a_retry() {
    let dir = tempfile::tempdir().unwrap();
    let address = closed_port();
    let config = ConfigDir::new(dir.path());
    let mut settings = Settings::load_or_create(&FileSettings::new(&config));
    settings.server = Some(address.clone());
    couchfin_core::SettingsStore::save(&FileSettings::new(&config), &settings).unwrap();
    let token = couchfin_core::AccessToken::new(TOKEN).unwrap();
    file_tokens(&dir).save(&address, &token).unwrap();

    let flow = launch(&dir);
    flow.start().await;

    let LoginScreen::Restoring(Restoring {
        status: RestoreStatus::Failed(error),
        ..
    }) = flow.screen()
    else {
        panic!("expected a failed restore, got {:?}", flow.screen());
    };
    assert!(matches!(error, ApiError::Unreachable { .. }), "{error:?}");
    assert_eq!(file_tokens(&dir).load(&address), Ok(Some(token)));
}

#[tokio::test]
async fn a_login_redirected_elsewhere_rechecks_and_asks_again() {
    let server = jellyfin().await;
    let moved = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(301).insert_header(
            "Location",
            format!("{}/Users/AuthenticateByName", moved.uri()),
        ))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let flow = launch(&dir);

    log_in(&flow, &server, "hunter2").await;

    let LoginScreen::LogIn(login) = flow.screen() else {
        panic!("expected LogIn, got {:?}", flow.screen());
    };
    // The address itself still checks out, so the form stays on it and says
    // where the login was sent.
    assert_eq!(login.server, server_url(&server));
    assert_eq!(
        login.request,
        Request::Failed(LoginError::Api(ApiError::Redirected {
            server: Some(server_url(&moved))
        }))
    );
    assert!(moved.received_requests().await.unwrap().is_empty());
}
