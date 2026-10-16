//! Binds `MainWindow` to couchfin-core's login flow: [`render`] shows a
//! [`LoginScreen`], and [`bind`] turns the window's callbacks into
//! [`Command`]s. Neither decides anything about the flow; core does.

use std::rc::Rc;

use couchfin_core::{LoginAction, LoginScreen, Password, Request, RestoreStatus};
use slint::ComponentHandle;

use crate::generated::{AddressView, ConnectView, HomeView, LoginView, MainWindow, Screen};
use crate::wording;

/// What the window asks for.
#[derive(Debug)]
pub enum Command {
    /// Send this action to the login flow (`LoginFlow::dispatch`).
    Login(LoginAction),
    /// Quit the app: Back on a screen with nothing to go back to.
    Quit,
}

/// Shows `screen` in `window`, replacing whatever it showed before.
///
/// Failures are logged here, with the error's `Display` text, and shown
/// with [`wording`]. After a failed sign-in the password field is emptied,
/// since core no longer has the password.
pub fn render(window: &MainWindow, screen: &LoginScreen) {
    // Back means "change server" where core accepts that, and "quit" where
    // there is nothing behind the screen.
    let back_quits = match screen {
        LoginScreen::Starting | LoginScreen::Home(_) => true,
        LoginScreen::Restoring(restoring) => restoring.status != RestoreStatus::Checking,
        LoginScreen::EnterAddress(form) => form.request != Request::InFlight,
        LoginScreen::LogIn(_) => false,
    };
    window.set_back_quits(back_quits);

    // Each view is filled before the screen switches, so the new page
    // starts with its data.
    let next = match screen {
        LoginScreen::Starting => {
            let view = window.global::<ConnectView>();
            view.set_server("".into());
            view.set_error("".into());
            Screen::Connecting
        }
        LoginScreen::Restoring(restoring) => {
            let view = window.global::<ConnectView>();
            view.set_server(restoring.server.as_str().into());
            if let RestoreStatus::Failed(error) = &restoring.status {
                tracing::warn!(%error, "couldn't check the stored session");
                view.set_error(wording::api_error(error).into());
                Screen::ConnectFailed
            } else {
                view.set_error("".into());
                Screen::Connecting
            }
        }
        LoginScreen::EnterAddress(form) => {
            let view = window.global::<AddressView>();
            let (busy, error) = request(&form.request, "couldn't use the server address");
            view.set_address(form.address.as_str().into());
            view.set_busy(busy);
            view.set_error(error.into());
            Screen::Address
        }
        LoginScreen::LogIn(form) => {
            let view = window.global::<LoginView>();
            let (busy, error) = request(&form.request, "couldn't sign in");
            view.set_server_name(form.info.server_name.as_str().into());
            view.set_server_address(form.server.as_str().into());
            view.set_username(form.username.as_str().into());
            view.set_busy(busy);
            view.set_error(error.into());
            if matches!(form.request, Request::Failed(_)) {
                view.set_password_resets(view.get_password_resets().wrapping_add(1));
            }
            Screen::Login
        }
        LoginScreen::Home(home) => {
            let view = window.global::<HomeView>();
            view.set_server_name(home.info.server_name.as_str().into());
            view.set_server_version(home.info.version.as_str().into());
            view.set_username(home.user.name.as_str().into());
            let notice = home.token_not_saved.as_ref().map(|error| {
                tracing::warn!(%error, "the sign-in wasn't saved");
                wording::token_not_saved(error)
            });
            view.set_notice(notice.unwrap_or_default().into());
            Screen::Home
        }
    };
    window.set_screen(next);
}

/// A form's busy flag and error message, logging a failure.
fn request(request: &Request, failure: &str) -> (bool, String) {
    match request {
        Request::Idle => (false, String::new()),
        Request::InFlight => (true, String::new()),
        Request::Failed(error) => {
            tracing::warn!(%error, "{failure}");
            (false, wording::login_error(error))
        }
    }
}

/// Sends a [`Command`] to `on_command` for each request the window makes.
///
/// The password from the sign-in form goes straight into a [`Password`],
/// whose `Debug` output is redacted; nothing here logs it.
pub fn bind(window: &MainWindow, on_command: impl Fn(Command) + 'static) {
    let send = Rc::new(on_command);
    let login = |action: fn() -> LoginAction| {
        let send = Rc::clone(&send);
        move || send(Command::Login(action()))
    };
    let back = {
        let send = Rc::clone(&send);
        let window = window.as_weak();
        move || {
            let quits = window
                .upgrade()
                .is_none_or(|window| window.get_back_quits());
            send(if quits {
                Command::Quit
            } else {
                Command::Login(LoginAction::ChangeServer)
            });
        }
    };

    let connect = window.global::<ConnectView>();
    connect.on_retry(login(|| LoginAction::Retry));
    connect.on_change_server(login(|| LoginAction::ChangeServer));
    connect.on_back(back.clone());

    let address = window.global::<AddressView>();
    address.on_submit({
        let send = Rc::clone(&send);
        move |address| send(Command::Login(LoginAction::SubmitAddress(address.into())))
    });
    address.on_back(back.clone());

    let form = window.global::<LoginView>();
    form.on_submit({
        let send = Rc::clone(&send);
        move |username, password| {
            send(Command::Login(LoginAction::SubmitLogin {
                username: username.into(),
                password: Password::new(password.as_str()),
            }));
        }
    });
    form.on_back(back.clone());

    let home = window.global::<HomeView>();
    home.on_sign_out(login(|| LoginAction::LogOut));
    home.on_back(back);
}
