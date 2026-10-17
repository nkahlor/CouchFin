//! The flow's steps: each starts a screen with `begin`, awaits the server,
//! and ends with `finish`.

use couchfin_api::{ApiError, JellyfinApi, Password, ServerCheck, ServerUrl, Session};

use super::screen::{
    EnterAddress, Home, LogIn, LoginError, LoginScreen, Request, RestoreStatus, Restoring,
};
use super::{LoginFlow, Ticket};
use crate::settings::SettingsStore;
use crate::tokens::TokenStore;

/// Where [`LoginAction::Retry`](super::LoginAction::Retry) applies.
pub(super) fn restore_failed(screen: &LoginScreen) -> bool {
    matches!(
        screen,
        LoginScreen::Restoring(Restoring {
            status: RestoreStatus::Failed(_),
            ..
        })
    )
}

fn enter_address(address: String, request: Request) -> LoginScreen {
    LoginScreen::EnterAddress(EnterAddress { address, request })
}

fn failed(error: impl Into<LoginError>) -> Request {
    Request::Failed(error.into())
}

fn login_form(check: ServerCheck, username: String, request: Request) -> LoginScreen {
    LoginScreen::LogIn(LogIn {
        server: check.server,
        info: check.info,
        username,
        request,
    })
}

impl<A: JellyfinApi, S: SettingsStore, T: TokenStore> LoginFlow<A, S, T> {
    fn remembered_username(&self) -> String {
        self.settings().username.unwrap_or_default()
    }

    /// Startup, and Retry: picks the first screen from what is stored.
    pub(super) async fn restore(&self, allowed: fn(&LoginScreen) -> bool) {
        if !allowed(&self.screen()) {
            return;
        }
        let Some(server) = self.settings().server else {
            self.begin(|screen| {
                allowed(screen).then(|| (enter_address(String::new(), Request::Idle), ()))
            });
            return;
        };
        let Some(token) = self.load_token(&server) else {
            // A server but no session: check it as if it had been typed.
            let address = server.to_string();
            let next = enter_address(address.clone(), Request::InFlight);
            if let Some((ticket, ())) = self.begin(|screen| allowed(screen).then_some((next, ()))) {
                self.check_address(ticket, address, server).await;
            }
            return;
        };
        let next = LoginScreen::Restoring(Restoring {
            server: server.clone(),
            status: RestoreStatus::Checking,
        });
        if let Some((ticket, ())) = self.begin(|screen| allowed(screen).then_some((next, ()))) {
            self.check_session(ticket, Session::new(server, token))
                .await;
        }
    }

    /// Checks a stored session: the server first (for its name, and to see
    /// whether it moved), then the token.
    async fn check_session(&self, ticket: Ticket, session: Session) {
        let server = session.server().clone();
        let restore_failed = |error| {
            LoginScreen::Restoring(Restoring {
                server: server.clone(),
                status: RestoreStatus::Failed(error),
            })
        };
        let check = match self.api.check_server(&server).await {
            Ok(check) => check,
            Err(error) => return self.finish(ticket, restore_failed(error)),
        };
        if check.server != server {
            // The address now redirects. The token is never sent to the new
            // address the server picked; `remember` drops it.
            self.remember(ticket, &check.server, None);
            let moved = ApiError::Redirected {
                server: Some(check.server.clone()),
            };
            let form = login_form(check, self.remembered_username(), failed(moved));
            return self.finish(ticket, form);
        }
        if !self.is_current(ticket) {
            return;
        }
        match self.api.current_user(&session).await {
            Ok(user) => self.finish_home(
                ticket,
                Home {
                    server,
                    info: check.info,
                    user,
                    token_not_saved: None,
                },
                session,
            ),
            Err(ApiError::TokenRejected) => {
                self.forget_token(&server);
                let form = login_form(
                    check,
                    self.remembered_username(),
                    failed(ApiError::TokenRejected),
                );
                self.finish(ticket, form);
            }
            Err(error @ ApiError::Redirected { .. }) => {
                self.recheck(ticket, &server, self.remembered_username(), error)
                    .await;
            }
            Err(error) => self.finish(ticket, restore_failed(error)),
        }
    }

    pub(super) async fn submit_address(&self, address: String) {
        let parsed = address.parse::<ServerUrl>();
        let request = match &parsed {
            Ok(_) => Request::InFlight,
            Err(error) => failed(error.clone()),
        };
        let next = enter_address(address.clone(), request);
        let started = self.begin(|screen| match screen {
            LoginScreen::EnterAddress(form) if form.request != Request::InFlight => {
                Some((next, ()))
            }
            _ => None,
        });
        if let (Some((ticket, ())), Ok(server)) = (started, parsed) {
            self.check_address(ticket, address, server).await;
        }
    }

    /// Checks `server` (typed as `address`) and shows its login form.
    async fn check_address(&self, ticket: Ticket, address: String, server: ServerUrl) {
        match self.api.check_server(&server).await {
            Ok(check) => {
                self.remember(ticket, &check.server, None);
                let form = login_form(check, self.remembered_username(), Request::Idle);
                self.finish(ticket, form);
            }
            Err(error) => self.finish(ticket, enter_address(address, failed(error))),
        }
    }

    pub(super) async fn submit_login(&self, username: String, password: Password) {
        let started = self.begin(|screen| match screen {
            LoginScreen::LogIn(form) if form.request != Request::InFlight => {
                let check = ServerCheck {
                    server: form.server.clone(),
                    info: form.info.clone(),
                };
                let next = login_form(check.clone(), username.clone(), Request::InFlight);
                Some((next, check))
            }
            _ => None,
        });
        let Some((ticket, check)) = started else {
            return;
        };
        let server = check.server.clone();
        let form = |request| login_form(check.clone(), username.clone(), request);
        let login = match self
            .api
            .authenticate_by_name(&server, &username, &password)
            .await
        {
            Ok(login) => login,
            Err(error @ ApiError::Redirected { .. }) => {
                return self.recheck(ticket, &server, username.clone(), error).await;
            }
            Err(error) => return self.finish(ticket, form(failed(error))),
        };
        if !self.is_current(ticket) {
            return;
        }
        let session = Session::new(server.clone(), login.access_token);
        let token_not_saved = self.save_token(&server, session.token());
        self.remember(ticket, &server, Some(&username));
        match self.api.current_user(&session).await {
            Ok(user) => self.finish_home(
                ticket,
                Home {
                    server,
                    info: check.info.clone(),
                    user,
                    token_not_saved,
                },
                session,
            ),
            Err(error) => self.finish(ticket, form(failed(error))),
        }
    }

    /// After a redirected login or token check: the address has changed, so
    /// check it again and ask for the login there, saying why.
    async fn recheck(&self, ticket: Ticket, server: &ServerUrl, username: String, cause: ApiError) {
        match self.api.check_server(server).await {
            Ok(check) => {
                self.remember(ticket, &check.server, None);
                self.finish(ticket, login_form(check, username, failed(cause)));
            }
            Err(error) => self.finish(ticket, enter_address(server.to_string(), failed(error))),
        }
    }

    pub(super) fn change_server(&self) {
        self.begin(|screen| {
            let address = match screen {
                LoginScreen::Restoring(restoring) => restoring.server.to_string(),
                LoginScreen::LogIn(form) => form.server.to_string(),
                LoginScreen::EnterAddress(form) if form.request == Request::InFlight => {
                    form.address.clone()
                }
                _ => return None,
            };
            Some((enter_address(address, Request::Idle), ()))
        });
    }

    pub(super) fn log_out(&self, request: Request) {
        let started = self.begin(|screen| match screen {
            LoginScreen::Home(home) => {
                let check = ServerCheck {
                    server: home.server.clone(),
                    info: home.info.clone(),
                };
                let form = login_form(check, home.user.name.clone(), request);
                Some((form, home.server.clone()))
            }
            _ => None,
        });
        if let Some((_, server)) = started {
            self.forget_token(&server);
        }
    }
}
