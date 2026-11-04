use std::sync::Arc;

use slint::ComponentHandle;

use crate::app::{App, AppError};
use crate::ui::{LoginView, MainWindow, Screen, ServerSelectionView};

pub fn bind(window: &MainWindow, app: Arc<App>) {
    let weak = window.as_weak();
    window
        .global::<ServerSelectionView>()
        .on_submit(move |url| {
            let url = url.trim().to_string();
            if url.is_empty() {
                if let Some(window) = weak.upgrade() {
                    window
                        .global::<ServerSelectionView>()
                        .set_error("Enter your server's address.".into());
                }
                return;
            }

            let (app, weak) = (app.clone(), weak.clone());
            tokio::spawn(async move {
                let result = app.connect(&url).await;

                let _ = weak.upgrade_in_event_loop(move |window| {
                    let view = window.global::<ServerSelectionView>();
                    match result {
                        Ok(info) => {
                            view.set_error("".into());
                            let login = window.global::<LoginView>();
                            login.set_server_url(info.url.into());
                            login.set_server_name(info.name.into());
                            window.set_screen(Screen::Login);
                        }
                        Err(err) => {
                            eprintln!("connect failed: {err}");
                            view.set_error(error_message(&err).into());
                        }
                    }
                });
            });
        });
}

fn error_message(err: &AppError) -> String {
    use crate::api::ApiError;
    match err {
        AppError::Api(ApiError::Unreachable(_)) => {
            "Couldn't reach the server. Check the address and your connection.".into()
        }
        AppError::Api(ApiError::Timeout(_)) => {
            "The server took too long to respond. Try again.".into()
        }
        AppError::Api(ApiError::InvalidUrl(_)) => "That isn't a valid server address.".into(),
        AppError::Api(ApiError::Request(_)) => {
            "That address doesn't look like a Jellyfin server.".into()
        }
        AppError::NoServer => "No server selected.".into(),
    }
}
