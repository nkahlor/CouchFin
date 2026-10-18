//! How long Home takes to fill with pictures from a slow server: the real
//! image cache and HTTP client against wiremock with 100 ms of latency per
//! reply, behind the real browsing screens (#49).
//!
//! The numbers printed (`cargo test -p couchfin-ui --test image_budget --
//! --nocapture`) are the ones quoted in decision 016.

mod common;

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::home::row_cards;
use common::library::{FakeLoader, Shell, World};

use couchfin_core::{
    AccessToken, CacheDir, ImageCache, ImageCacheConfig, ImageLoad, ImageRef, ImageSize,
    JellyfinClient, LibraryState, RowKind, ScreenState,
};
use couchfin_ui::{Guard, HomeView, ImageLoader, ImageState};
use image::{ImageFormat, Rgb, RgbImage};
use slint::{ComponentHandle, Model};
use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// What a home network round trip costs.
const LATENCY: Duration = Duration::from_millis(100);
/// The blurred placeholders are in the cards this long after Home appears:
/// they need no network.
const PLACEHOLDERS_BY: Duration = Duration::from_millis(50);
/// Every picture on screen has arrived within this long, per wave of
/// requests (8 in flight), plus [`SLACK`]: a 100 ms round trip and the
/// decoding. The screen holds about 14 cards (two rows of posters and
/// thumbnails at 1080p, #65), so two waves.
const PER_WAVE: Duration = Duration::from_millis(150);
const SLACK: Duration = Duration::from_millis(250);
/// CI's shared two-core runners, where requests and decodes compete for CPU,
/// get twice as long.
const CI_FACTOR: u32 = 2;
/// Requests in flight at once: `ImageCacheConfig::default().max_in_flight`.
const IN_FLIGHT: usize = 8;
const ROWS: usize = 6;

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The app's loader over a real cache and client.
struct WireLoader {
    cache: Arc<ImageCache<JellyfinClient>>,
    session: couchfin_core::Session,
    runtime: Handle,
}

impl ImageLoader for WireLoader {
    fn load(
        &self,
        image: &ImageRef,
        size: ImageSize,
        done: Box<dyn FnOnce(ImageLoad) + Send>,
    ) -> Guard {
        let (cache, session) = (Arc::clone(&self.cache), self.session.clone());
        let request = image.request(size);
        let task = self.runtime.spawn(async move {
            done(
                cache
                    .load(&session, &request)
                    .await
                    .unwrap_or(ImageLoad::NoImage),
            );
        });
        Guard::new(AbortOnDrop(task))
    }
}

/// A poster-sized JPEG with some texture, so decoding costs something.
fn poster_jpeg() -> Vec<u8> {
    let channel = |value: u32| u8::try_from(value % 251).unwrap_or(0);
    let image = RgbImage::from_fn(280, 420, |x, y| {
        Rgb([channel(x * 7 + y * 3), channel(x * y), channel(x + y * 5)])
    });
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Jpeg)
        .expect("encoding works");
    out.into_inner()
}

/// Home with `ROWS` rows of distinct pictures.
fn many_rows() -> LibraryState {
    let mut state = World::new(40).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    let template = home
        .rows
        .iter()
        .find(|row| row.title.starts_with("Recently Added"))
        .expect("a Recently Added row")
        .clone();
    home.rows = (0..ROWS)
        .map(|r| {
            let mut row = template.clone();
            row.title = format!("Row {r}");
            row.kind = RowKind::RecentlyAdded(format!("lib-{r}").parse().expect("id"));
            for (n, card) in row.cards.iter_mut().enumerate() {
                if let Some(image) = &mut card.image {
                    image.item = format!("r{r}c{n}").parse().expect("id");
                }
            }
            row
        })
        .collect();
    state
}

/// The cards on screen as (row, index in its row).
fn on_screen(shell: &Shell) -> Vec<(usize, usize)> {
    (0..ROWS)
        .flat_map(|row| {
            row_cards(&shell.window, &format!("Row {row}"))
                .into_iter()
                .filter_map(move |card| Some((row, card.accessible_item_index()?)))
        })
        .collect()
}

fn card(shell: &Shell, row: usize, index: usize) -> couchfin_ui::CardData {
    shell
        .window
        .global::<HomeView>()
        .get_rows()
        .row_data(row)
        .and_then(|row| row.cards.row_data(index))
        .expect("a card")
}

fn has_picture(shell: &Shell, (row, index): (usize, usize)) -> bool {
    card(shell, row, index).image.size().width > 0
}

#[test]
fn the_first_screenful_of_home_fills_within_the_budget() {
    let runtime = Runtime::new().expect("runtime");
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(
        Mock::given(method("GET"))
            .and(path_regex(r"^/Items/[^/]+/Images/[A-Za-z]+$"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(poster_jpeg(), "image/jpeg")
                    .set_delay(LATENCY),
            )
            .mount(&server),
    );
    let client = JellyfinClient::new(
        "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42"
            .parse()
            .expect("device id"),
    )
    .expect("client builds");
    let session = couchfin_core::Session::new(
        server.uri().parse().expect("URL"),
        AccessToken::new("0123456789abcdef0123456789abcdef").expect("token"),
    );
    let dir = tempfile::tempdir().expect("dir");
    let cache = Arc::new(ImageCache::new(
        client,
        &CacheDir::new(dir.path()),
        ImageCacheConfig::default(),
    ));
    let loader = WireLoader {
        cache,
        session,
        runtime: runtime.handle().clone(),
    };
    let shell = Shell::with_loader(FakeLoader::default(), Arc::new(loader));

    let started = Instant::now();
    shell.feed.state(many_rows());
    shell.feed.show();
    shell.feed.pump();
    let placeholders_at = started.elapsed();
    common::flow::settle();
    let visible = on_screen(&shell);
    for &(row, index) in &visible {
        let data = card(&shell, row, index);
        assert!(data.placeholder.size().width > 0, "row {row} card {index}");
        assert_eq!(data.image_state, ImageState::Loading);
    }
    assert!(
        placeholders_at <= PLACEHOLDERS_BY,
        "placeholders took {placeholders_at:?}, budget {PLACEHOLDERS_BY:?}"
    );
    let rows_shown = visible
        .iter()
        .map(|(row, _)| row)
        .max()
        .map_or(0, |r| r + 1);
    assert!(rows_shown >= 2, "a TV shows more than one row: {visible:?}");

    let mut filled_at = None;
    while started.elapsed() < Duration::from_secs(10) {
        shell.feed.pump();
        if visible.iter().all(|&at| has_picture(&shell, at)) {
            filled_at = Some(started.elapsed());
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let missing = visible
        .iter()
        .filter(|&&at| !has_picture(&shell, at))
        .count();
    eprintln!(
        "{} cards on screen in {rows_shown} rows: placeholders after {placeholders_at:?}, pictures after {filled_at:?} ({missing} missing)",
        visible.len(),
    );

    let filled_at = filled_at.expect("every card on screen gets its picture");
    let waves = u32::try_from(visible.len().div_ceil(IN_FLIGHT)).expect("a few waves");
    let mut budget = PER_WAVE * waves + SLACK;
    if std::env::var_os("CI").is_some() {
        budget *= CI_FACTOR;
    }
    assert!(
        filled_at <= budget,
        "pictures took {filled_at:?}, budget {budget:?}"
    );
}
