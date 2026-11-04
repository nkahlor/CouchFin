use std::sync::Arc;

use slint::ComponentHandle;

use crate::api::{ApiError, JellyfinClient, secret::Secret};
use crate::{HomeView, LoginView, MainWindow, Screen};

pub fn install(window: &MainWindow, client: Arc<JellyfinClient>, server_url: &str) {
    let view = window.global::<LoginView>();
    view.set_server_address(server_url.into());
    view.set_server_name(server_url.into());

    load_server_name(window, client.clone());

    let weak = window.as_weak();
    view.on_submit(move |username, password| {
        let username = username.trim().to_string();
        let password = Secret::new(password.as_str());

        let Some(window) = weak.upgrade() else { return };
        let view = window.global::<LoginView>();
        if username.is_empty() {
            view.set_error("Enter your username.".into());
            return;
        }
        view.set_error("".into());
        view.set_busy(true);

        let client = client.clone();
        let weak = weak.clone();
        tokio::spawn(async move {
            let result = client.login_by_name(&username, password).await;

            let _ = weak.upgrade_in_event_loop(move |window| {
                let view = window.global::<LoginView>();
                view.set_busy(false);
                match result {
                    Ok(auth) => {
                        let name = auth.user.name.unwrap_or(username);
                        window.global::<HomeView>().set_user_name(name.into());
                        window.set_screen(Screen::Home);
                    }
                    Err(err) => {
                        eprintln!("login failed: {err}");
                        view.set_error(error_message(&err).into());
                        view.set_password_resets(view.get_password_resets() + 1);
                    }
                }
            });
        });
    });

    view.on_back(|| {
        // TODO: navigate to the server selection screen once it exists.
    });
}

fn load_server_name(window: &MainWindow, client: Arc<JellyfinClient>) {
    let weak = window.as_weak();
    tokio::spawn(async move {
        match client.get_public_info().await {
            Ok(info) => {
                let _ = weak.upgrade_in_event_loop(move |window| {
                    window
                        .global::<LoginView>()
                        .set_server_name(info.server_name.into());
                });
            }
            Err(err) => {
                let message = error_message(&err);
                let _ = weak.upgrade_in_event_loop(move |window| {
                    window.global::<LoginView>().set_error(message.into());
                });
            }
        }
    });
}

fn error_message(err: &ApiError) -> String {
    match err {
        ApiError::Unreachable(_) => {
            "Couldn't reach the server. Check the address and your connection.".into()
        }
        ApiError::Timeout(_) => "The server took too long to respond. Try again.".into(),
        ApiError::InvalidUrl(_) => "The server address isn't valid.".into(),
        ApiError::Request(e) => match e.status().map(|s| s.as_u16()) {
            Some(401) => "Incorrect username or password.".into(),
            Some(400) => "The server rejected the sign-in request.".into(),
            Some(code) if code >= 500 => {
                format!("The server had a problem (error {code}). Try again later.")
            }
            Some(code) => format!("Sign-in failed (error {code})."),
            None if e.is_decode() => {
                "The server sent a response CouchFin doesn't understand.".into()
            }
            None => "Sign-in failed.".into(),
        },
    }
}
