//! Phase 1's "Done when" checks through the whole app: `app::connect` with
//! the real `JellyfinClient` against wiremock, settings and tokens in a
//! temporary config directory (the keyring unavailable, as under gamescope),
//! and the UI on Slint's testing backend, typed into like a keyboard.
//!
//! Slint's testing event loop can start once per process, so this binary
//! has a single test that runs the checks in order, one app launch each.

#[allow(dead_code)] // shared helpers; this binary uses only some
mod common;

use std::collections::BTreeSet;
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use common::{WINDOW, close_window, erase, press, type_text, ui, wait_for};
use couchfin_api::JellyfinClient;
use couchfin_core::{
    ConfigDir, FallbackTokens, FileSettings, FileTokens, KeyringTokens, LoginFlow, Settings,
};
use couchfin_ui::MainWindow;
use serde_json::json;
use slint::ComponentHandle;
use slint::platform::Key;
use tempfile::TempDir;
use tokio::runtime::{Handle, Runtime};
use wiremock::matchers::{body_partial_json, header_regex, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const UNREACHABLE: &str =
    "Couldn't reach the server. Check the address, and that the server is on.";

/// Opens the app over `dir`, as one launch of `couchfin` does.
fn launch(dir: &TempDir, runtime: &Handle) {
    let config = ConfigDir::new(dir.path());
    let runtime = runtime.clone();
    ui(move || {
        let settings_file = FileSettings::new(&config);
        let settings = Settings::load_or_create(&settings_file);
        let api = JellyfinClient::new(settings.device_id.clone()).expect("client builds");
        let tokens = FallbackTokens::new(
            KeyringTokens::unavailable("no Secret Service in tests"),
            FileTokens::new(&config),
        );
        let flow = LoginFlow::new(api.clone(), settings, settings_file, tokens);
        let window = MainWindow::new().expect("window is created");
        app::connect_with(&window, flow, api, None, &runtime);
        window.show().expect("window is shown");
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
    });
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
    let user = json!({ "Id": "38a5a5bbdc3549ccb0b07fc7e43c1d43", "Name": "alice" });
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .and(body_partial_json(
            json!({ "Username": "alice", "Pw": "hunter2" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "User": user,
            "AccessToken": TOKEN,
            "ServerId": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .and(header_regex("authorization", &format!("Token=\"{TOKEN}\"")))
        .respond_with(ResponseTemplate::new(200).set_body_json(user))
        .named("token accepted")
        .mount(&server)
        .await;
    server
}

/// An address where nothing listens.
fn closed_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("can bind a local port");
    let port = listener.local_addr().expect("socket has an address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

/// The device IDs the server saw, from every request's auth header.
async fn device_ids(server: &MockServer) -> BTreeSet<String> {
    let requests = server.received_requests().await.expect("recording is on");
    requests
        .iter()
        .filter_map(|request| request.headers.get("authorization"))
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split("DeviceId=\"").nth(1))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect()
}

fn checks(runtime: &Runtime) {
    let server = runtime.block_on(jellyfin());
    let address = server.uri();
    let dir = tempfile::tempdir().expect("temporary directory");

    // First run: an unreachable address shows a clear error (Done when 4).
    launch(&dir, runtime.handle());
    wait_for("Connect to Jellyfin");
    let unreachable = closed_port();
    type_text(&unreachable);
    press(Key::Return);
    wait_for(UNREACHABLE);

    // The real address leads to the login form, which names the server.
    erase(unreachable.len());
    type_text(&address);
    press(Key::Return);
    wait_for("Sign in to Living Room");
    wait_for(format!("Connected to {address}/"));

    // A wrong password shows a clear error and empties the field (4).
    type_text("alice");
    press(Key::Return);
    type_text("wrong");
    press(Key::Return);
    wait_for("Wrong username or password.");

    // The right one reaches Home with the server and the user (1).
    type_text("hunter2");
    press(Key::Return);
    wait_for("Signed in as alice");
    wait_for("Living Room");
    wait_for("Jellyfin 10.11.2");
    close_window();

    // A restart skips login: the stored token still works (3).
    launch(&dir, runtime.handle());
    wait_for("Signed in as alice");
    close_window();

    // One device ID across both launches, so Devices lists one entry (2).
    assert_eq!(runtime.block_on(device_ids(&server)).len(), 1);

    // A token revoked on the server sends the next start back to sign in.
    runtime.block_on(revoke(&server));
    launch(&dir, runtime.handle());
    wait_for("You've been signed out. Please sign in again.");
    wait_for("Sign in to Living Room");

    // Back from the login form goes to the address; Back there quits.
    press(Key::Escape);
    wait_for("Connect to Jellyfin");
    press(Key::Escape);
    assert!(
        !event_loop_answers(),
        "Back on the address screen didn't quit"
    );
}

/// Whether the event loop still runs closures sent to it.
fn event_loop_answers() -> bool {
    let (send, receive) = std::sync::mpsc::channel();
    let _ = slint::invoke_from_event_loop(move || {
        let _ = send.send(());
    });
    receive
        .recv_timeout(std::time::Duration::from_secs(1))
        .is_ok()
}

/// Forgets every mock and recorded request: the server answers again, but
/// no longer accepts the stored token.
async fn revoke(server: &MockServer) {
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ServerName": "Living Room",
            "Version": "10.11.2",
            "Id": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(server)
        .await;
}

#[test]
fn phase_1_done_when_checks() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let runtime = Runtime::new().expect("tokio runtime starts");
    let driver = std::thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| checks(&runtime)));
        // Stop the event loop whatever happened; on success the last
        // Escape has already asked it to quit.
        let _ = slint::quit_event_loop();
        outcome
    });
    slint::run_event_loop_until_quit().expect("the event loop runs");
    if let Err(panic) = driver.join().expect("the driver thread finishes") {
        resume_unwind(panic);
    }
}
