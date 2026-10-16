//! Login-flow screens built by hand, and a window bound to them that records
//! the commands it sends.

use couchfin_core::{
    ApiError, EnterAddress, Home, LogIn, LoginAction, LoginError, LoginScreen, PublicSystemInfo,
    Request, RestoreStatus, Restoring, ServerUrl, TokenStoreError, User,
};
use couchfin_ui::{Command, MainWindow, bind, render};
use slint::ComponentHandle;

use super::{Calls, init};

pub const SERVER: &str = "https://jellyfin.example/";

pub fn server() -> ServerUrl {
    SERVER.parse().expect("valid URL")
}

pub fn info() -> PublicSystemInfo {
    PublicSystemInfo {
        server_name: "Living Room".into(),
        version: "10.11.2".into(),
        id: "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f"
            .parse()
            .expect("valid ID"),
    }
}

pub fn restoring(status: RestoreStatus) -> LoginScreen {
    LoginScreen::Restoring(Restoring {
        server: server(),
        status,
    })
}

pub fn enter_address(address: &str, request: Request) -> LoginScreen {
    LoginScreen::EnterAddress(EnterAddress {
        address: address.into(),
        request,
    })
}

pub fn log_in(request: Request) -> LoginScreen {
    LoginScreen::LogIn(LogIn {
        server: server(),
        info: info(),
        username: "alice".into(),
        request,
    })
}

pub fn home(token_not_saved: Option<TokenStoreError>) -> LoginScreen {
    LoginScreen::Home(Home {
        server: server(),
        info: info(),
        user: User {
            id: "4be3b9b3e1a54d7c9b6f2a1c0d9e8f7a"
                .parse()
                .expect("valid ID"),
            name: "alice".into(),
        },
        token_not_saved,
    })
}

pub fn failed(error: ApiError) -> Request {
    Request::Failed(LoginError::Api(error))
}

/// A command as text, so tests can compare them. Shows the password, which
/// only test passwords ever reach.
pub fn describe(command: Command) -> String {
    match command {
        Command::Quit => "Quit".into(),
        Command::Login(LoginAction::SubmitAddress(address)) => format!("SubmitAddress({address})"),
        Command::Login(LoginAction::SubmitLogin { username, password }) => {
            format!("SubmitLogin({username}, {})", password.expose_secret())
        }
        Command::Login(LoginAction::Retry) => "Retry".into(),
        Command::Login(LoginAction::ChangeServer) => "ChangeServer".into(),
        Command::Login(LoginAction::LogOut) => "LogOut".into(),
    }
}

/// A shown window, bound and rendering `screen`, recording its commands.
pub fn bound(screen: &LoginScreen) -> (MainWindow, Calls<String>) {
    init();
    let window = MainWindow::new().expect("window is created");
    let commands = Calls::new();
    let recorder = commands.clone();
    bind(&window, move |command| recorder.record(describe(command)));
    render(&window, screen);
    window.show().expect("window is shown");
    (window, commands)
}

/// Lets Slint run its change handlers, as the event loop does each frame.
pub fn settle() {
    slint::platform::update_timers_and_animations();
}
