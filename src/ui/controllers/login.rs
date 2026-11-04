use std::sync::Arc;

use slint::ComponentHandle;

use crate::api::secret::Secret;
use crate::app::App;
use crate::ui::{HomeView, LoginView, MainWindow, Screen};

pub fn bind(window: &MainWindow, app: Arc<App>) {
    let view = window.global::<LoginView>();

    let weak = window.as_weak();
    let login_app = app.clone();
    view.on_submit(move |username, password| {
        let username = username.trim().to_string();
        let password = Secret::new(password.as_str());

        let (app, weak) = (login_app.clone(), weak.clone());
        tokio::spawn(async move {
            let result = app.login(&username, password).await;

            let _ = weak.upgrade_in_event_loop(move |window| match result {
                Ok(name) => {
                    window.global::<HomeView>().set_user_name(name.into());
                    window.set_screen(Screen::Home);
                }
                Err(err) => {
                    eprintln!("login failed: {err}");
                }
            });
        });
    });

    let weak = window.as_weak();
    view.on_back(move || {
        app.disconnect();
        if let Some(window) = weak.upgrade() {
            window.set_screen(Screen::ServerSelection);
        }
    });
}
