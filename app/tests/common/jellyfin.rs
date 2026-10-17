//! A wiremock Jellyfin with a generated library of placeholder posters,
//! for the Phase 3 checks: `tests/phase3_browse.rs` in CI and
//! `examples/perf_grid.rs` on the TV. Included by both (the example with
//! `#[path]`), so it uses only external crates.

use std::io::Cursor;

use couchfin_api::{AccessToken, JellyfinClient, ServerUrl};
use couchfin_core::fake::{MemorySettings, MemoryTokens};
use couchfin_core::{LoginFlow, Settings, SettingsStore, TokenStore};
use image::{ImageFormat, RgbaImage};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

pub const TOKEN: &str = "0123456789abcdef0123456789abcdef";
pub const LIBRARY: &str = "library0";
/// How many resumable movies Continue Watching holds.
pub const RESUMABLE: usize = 8;

pub fn movie_id(n: usize) -> String {
    format!("movie{n:05}")
}

pub fn movie_name(n: usize) -> String {
    format!("Movie {n:04}")
}

fn movie(n: usize, resume: bool) -> Value {
    let mut item = json!({
        "Id": movie_id(n),
        "Name": movie_name(n),
        "Type": "Movie",
        "ProductionYear": 2000 + n % 24,
        "RunTimeTicks": 72_000_000_000_i64,
        "Overview": "A placeholder movie.",
        "ImageTags": { "Primary": format!("tag{n}") },
    });
    if resume {
        item["UserData"] = json!({
            "Played": false,
            "PlaybackPositionTicks": 18_000_000_000_i64,
            "PlayedPercentage": 25.0,
        });
    }
    item
}

fn page(items: &[Value], total: usize, start: usize) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "Items": items,
        "TotalRecordCount": total,
        "StartIndex": start,
    }))
}

fn query(request: &Request, name: &str) -> Option<usize> {
    request
        .url
        .query_pairs()
        .find(|(key, _)| key == name)
        .and_then(|(_, value)| value.parse().ok())
}

/// A small PNG: the placeholder poster every item has.
pub fn poster() -> Vec<u8> {
    let image = RgbaImage::from_pixel(40, 60, image::Rgba([120, 80, 200, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("PNG encodes");
    bytes.into_inner()
}

/// A Jellyfin with one movie library of `items` movies, `RESUMABLE` of them
/// half-watched, and a placeholder poster for every item.
pub async fn mount(items: usize) -> MockServer {
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
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({ "Id": "38a5a5bbdc3549ccb0b07fc7e43c1d43", "Name": "alice" }),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/UserViews"))
        .respond_with(page(
            &[json!({
                "Id": LIBRARY,
                "Name": "Movies",
                "CollectionType": "movies",
                "ImageTags": { "Primary": "library-tag" },
            })],
            1,
            0,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/UserItems/Resume"))
        .respond_with(page(
            &(0..RESUMABLE).map(|n| movie(n, true)).collect::<Vec<_>>(),
            RESUMABLE,
            0,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Shows/NextUp"))
        .respond_with(page(&[], 0, 0))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Items/Latest"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                (0..16)
                    .map(|n| movie(items - 1 - n, false))
                    .collect::<Vec<_>>(),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(move |request: &Request| {
            let start = query(request, "startIndex").unwrap_or(0).min(items);
            let limit = query(request, "limit").unwrap_or(items);
            let end = (start + limit).min(items);
            page(
                &(start..end).map(|n| movie(n, false)).collect::<Vec<_>>(),
                items,
                start,
            )
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/Items/movie\d+$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(movie(0, true)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/Items/[^/]+/Images/[A-Za-z]+$"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(poster(), "image/png"))
        .mount(&server)
        .await;
    server
}

/// The `startIndex` of every page of the library the server was asked for.
pub async fn page_requests(server: &MockServer) -> Vec<usize> {
    server
        .received_requests()
        .await
        .expect("recording is on")
        .iter()
        .filter(|request| request.url.path() == "/Items")
        .filter_map(|request| query(request, "startIndex"))
        .collect()
}

/// A login flow that restores a stored session on `server` and reaches Home,
/// and the client it shares with the library.
pub fn restoring(
    address: &str,
) -> (
    JellyfinClient,
    LoginFlow<JellyfinClient, MemorySettings, MemoryTokens>,
) {
    let url: ServerUrl = address.parse().expect("a server address");
    let settings = MemorySettings::new();
    let mut stored = Settings::load_or_create(&settings);
    stored.server = Some(url.clone());
    settings.save(&stored).expect("settings save");
    let tokens = MemoryTokens::new();
    tokens
        .save(&url, &AccessToken::new(TOKEN).expect("a valid token"))
        .expect("token saves");
    let api = JellyfinClient::new(stored.device_id.clone()).expect("client builds");
    let flow = LoginFlow::new(api.clone(), stored, settings, tokens);
    (api, flow)
}
