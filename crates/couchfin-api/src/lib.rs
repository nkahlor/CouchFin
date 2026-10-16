//! Typed Jellyfin HTTP client: authentication, items, images and playback
//! reporting.
//!
//! Depends on no other Couchfin crate, so it can be tested on its own against
//! a mock server.
//!
//! - [`JellyfinApi`] is the trait callers depend on. [`JellyfinClient`]
//!   implements it over HTTP, and [`fake::FakeJellyfin`] in memory for tests.
//! - [`ServerUrl`], [`DeviceId`], [`UserId`], [`ServerId`], [`AccessToken`]
//!   and [`Password`] are validated newtypes; the two secrets never show in
//!   `Debug` output.
//! - [`ApiError`] tells apart an unreachable server, a timeout, a server that
//!   isn't Jellyfin, a wrong password and a rejected token.
//!
//! The design is recorded in `docs/decisions/006-api-client-design.md`.
//!
//! # Examples
//!
//! Check a server, log in, and confirm the token works, as the login screen
//! does:
//!
//! ```
//! use couchfin_api::fake::FakeJellyfin;
//! use couchfin_api::{JellyfinApi, Password, ServerUrl, Session};
//!
//! let server: ServerUrl = "http://jellyfin.lan:8096".parse()?;
//! let api = FakeJellyfin::new(server.clone()).with_user("alice", "hunter2");
//!
//! tokio::runtime::Builder::new_current_thread()
//!     .build()?
//!     .block_on(async {
//!         let info = api.public_system_info(&server).await?;
//!         let login = api
//!             .authenticate_by_name(&server, "alice", &Password::new("hunter2"))
//!             .await?;
//!         let session = Session::new(server.clone(), login.access_token);
//!         let me = api.current_user(&session).await?;
//!         println!("{} on {} {}", me.name, info.server_name, info.version);
//!         Ok::<(), couchfin_api::ApiError>(())
//!     })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod api;
mod auth_header;
mod client;
mod dto;
mod error;
pub mod fake;
mod ids;
mod secret;
mod server_url;

pub use api::{JellyfinApi, Session};
pub use auth_header::{CLIENT_NAME, CLIENT_VERSION, DEVICE_NAME};
pub use client::{BuildError, JellyfinClient};
pub use dto::{AuthenticationResult, PublicSystemInfo, User};
pub use error::ApiError;
pub use ids::{DeviceId, InvalidValueError, ServerId, UserId};
pub use secret::{AccessToken, Password};
pub use server_url::{ServerUrl, ServerUrlError};
