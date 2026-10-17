//! The startup and sign-in screens, rendered from core's real login states so
//! the wording is the app's own.

use anyhow::Result;
use couchfin_core::{
    ApiError, EnterAddress, LogIn, LoginError, LoginScreen, PublicSystemInfo, Request,
    RestoreStatus, Restoring, ServerUrl,
};
use couchfin_ui::{Screen, render};

use crate::studio::Studio;

fn server() -> Result<ServerUrl> {
    Ok("https://jellyfin.example/".parse()?)
}

fn info() -> Result<PublicSystemInfo> {
    Ok(PublicSystemInfo {
        server_name: "Living Room".into(),
        version: "10.11.2".into(),
        id: "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f".parse()?,
    })
}

fn failed(error: ApiError) -> Request {
    Request::Failed(LoginError::Api(error))
}

fn address(text: &str, request: Request) -> LoginScreen {
    LoginScreen::EnterAddress(EnterAddress {
        address: text.into(),
        request,
    })
}

fn login(request: Request) -> Result<LoginScreen> {
    Ok(LoginScreen::LogIn(LogIn {
        server: server()?,
        info: info()?,
        username: "alice".into(),
        request,
    }))
}

fn restoring(status: RestoreStatus) -> Result<LoginScreen> {
    Ok(LoginScreen::Restoring(Restoring {
        server: server()?,
        status,
    }))
}

/// Every login-flow state at one size.
pub fn shoot(studio: &mut Studio, size: (u32, u32)) -> Result<()> {
    let unreachable = ApiError::Unreachable {
        detail: "connection refused".into(),
    };
    let states: Vec<(&str, &str, LoginScreen)> = vec![
        ("address", "idle", address("", Request::Idle)),
        (
            "address",
            "typed",
            address("jellyfin.example:8096", Request::Idle),
        ),
        (
            "address",
            "busy",
            address("jellyfin.example:8096", Request::InFlight),
        ),
        (
            "address",
            "error",
            address("jellyfin.example:8096", failed(unreachable.clone())),
        ),
        ("login", "idle", login(Request::Idle)?),
        ("login", "busy", login(Request::InFlight)?),
        (
            "login",
            "error",
            login(failed(ApiError::InvalidCredentials))?,
        ),
        (
            "connecting",
            "checking",
            restoring(RestoreStatus::Checking)?,
        ),
        (
            "connect-failed",
            "unreachable",
            restoring(RestoreStatus::Failed(unreachable))?,
        ),
        (
            "connect-failed",
            "timeout",
            restoring(RestoreStatus::Failed(ApiError::Timeout))?,
        ),
    ];
    for (name, state, flow) in states {
        let mut session = studio.open(Screen::Connecting, size)?;
        render(&session.window, &flow);
        session.start()?;
        session.snap(name, state)?;
    }
    Ok(())
}
