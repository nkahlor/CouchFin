//! Typed Jellyfin HTTP client: authentication, items, images and playback
//! reporting.
//!
//! Depends on no other Couchfin crate, so it can be tested on its own against
//! a mock server.
//!
//! - [`JellyfinApi`] is the trait callers depend on. [`JellyfinClient`]
//!   implements it over HTTP, and [`fake::FakeJellyfin`] in memory for tests.
//! - [`ServerUrl`], [`DeviceId`], [`UserId`], [`ServerId`], [`ItemId`],
//!   [`ImageTag`], [`AccessToken`] and [`Password`] are validated newtypes; the two secrets never show in
//!   `Debug` output.
//! - [`ApiError`] tells apart an unreachable server, a timeout, a server that
//!   isn't Jellyfin, a wrong password, a rejected token and a redirect.
//! - [`JellyfinApi::check_server`] is the only call that follows redirects,
//!   and it returns the server's canonical address ([`ServerCheck::server`])
//!   for every later call.
//!
//! - The library calls ([`JellyfinApi::libraries`], [`JellyfinApi::items`],
//!   [`JellyfinApi::resume`], [`JellyfinApi::image`] and the rest) return one
//!   [`Item`] type, [`Library`] views and [`Page`]s, and need a [`Session`]
//!   (they act for the user the token belongs to).
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
//! let typed: ServerUrl = "http://jellyfin.lan:8096".parse()?;
//! let api = FakeJellyfin::new(typed.clone()).with_user("alice", "hunter2");
//!
//! tokio::runtime::Builder::new_current_thread()
//!     .build()?
//!     .block_on(async {
//!         let check = api.check_server(&typed).await?;
//!         let server = check.server; // canonical: store and use this one
//!         let login = api
//!             .authenticate_by_name(&server, "alice", &Password::new("hunter2"))
//!             .await?;
//!         let session = Session::new(server, login.access_token);
//!         let me = api.current_user(&session).await?;
//!         let info = check.info;
//!         println!("{} on {} {}", me.name, info.server_name, info.version);
//!         Ok::<(), couchfin_api::ApiError>(())
//!     })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod api;
mod auth_header;
mod blur_hash;
mod builder;
mod client;
mod dto;
mod error;
pub mod fake;
mod http;
mod ids;
mod image;
mod item;
mod library;
mod library_query;
mod query;
mod redirect;
mod secret;
mod server_url;

pub use api::{JellyfinApi, Session};
pub use auth_header::{CLIENT_NAME, CLIENT_VERSION, DEVICE_NAME};
pub use blur_hash::{BlurHash, ImageBlurHashes, InvalidBlurHash};
pub use builder::{BuildError, JellyfinClientBuilder};
pub use client::JellyfinClient;
pub use dto::{AuthenticationResult, PublicSystemInfo, ServerCheck, User};
pub use error::ApiError;
pub use ids::{DeviceId, ImageTag, InvalidValueError, ItemId, ServerId, UserId};
pub use image::{Image, ImageKind, ImageRequest};
pub use item::{ImageTags, Item, ItemKind, UserData};
pub use library::{Library, LibraryKind, Page};
pub use query::ItemsQuery;
pub use secret::{AccessToken, Password};
pub use server_url::{ServerUrl, ServerUrlError};
