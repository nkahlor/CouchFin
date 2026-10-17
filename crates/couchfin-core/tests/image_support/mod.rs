//! Helpers for the image cache tests: a fake server that counts image calls,
//! can serve chosen bytes slowly, and a ready-made session and cache.
#![allow(dead_code)]

use std::future::Future;
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use couchfin_api::{
    ApiError, AuthenticationResult, Image, ImageKind, ImageRequest, Item, ItemId, ItemsQuery,
    JellyfinApi, Library, Page, Password, ServerCheck, ServerUrl, Session, User,
};
use couchfin_core::fake::FakeJellyfin;
use couchfin_core::{CacheDir, ImageCache, ImageCacheConfig};
use image::{ImageFormat, Rgba, RgbaImage};
use tempfile::TempDir;

/// A 64x96 image (a 2:3 poster) with hard edges, so blurring visibly changes it.
pub fn checkerboard() -> RgbaImage {
    RgbaImage::from_fn(64, 96, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            Rgba([250, 20, 20, 255])
        } else {
            Rgba([10, 10, 240, 255])
        }
    })
}

/// [`checkerboard`], encoded.
pub fn encoded(format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    // JPEG has no alpha channel.
    image::DynamicImage::ImageRgba8(checkerboard())
        .to_rgb8()
        .write_to(&mut out, format)
        .expect("encoding works");
    out.into_inner()
}

#[derive(Default)]
struct State {
    image_calls: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    served: Mutex<Option<Vec<u8>>>,
    delay: Mutex<Duration>,
}

/// The fake server, counting `image` calls.
///
/// Without [`TestApi::serve`] it answers like the fake (a 1x1 PNG for items
/// it knows, `ItemNotFound` for others). With it, it serves those bytes for
/// every item, after the configured delay.
#[derive(Clone)]
pub struct TestApi {
    pub fake: FakeJellyfin,
    state: Arc<State>,
}

impl TestApi {
    pub fn new(fake: FakeJellyfin) -> Self {
        Self {
            fake,
            state: Arc::default(),
        }
    }

    /// Serves `bytes` (as a PNG) for every image request.
    pub fn serve(&self, bytes: Vec<u8>) {
        *self.state.served.lock().expect("lock") = Some(bytes);
    }

    pub fn delay_images(&self, delay: Duration) {
        *self.state.delay.lock().expect("lock") = delay;
    }

    /// How many image requests reached the server.
    pub fn image_calls(&self) -> usize {
        self.state.image_calls.load(Ordering::SeqCst)
    }

    /// The most image requests that were running at the same moment.
    pub fn peak_in_flight(&self) -> usize {
        self.state.peak.load(Ordering::SeqCst)
    }

    /// Image requests running now.
    pub fn in_flight(&self) -> usize {
        self.state.active.load(Ordering::SeqCst)
    }
}

/// Counts a running request, and uncounts it even if the caller drops it.
struct Running<'a>(&'a State);

impl<'a> Running<'a> {
    fn start(state: &'a State) -> Self {
        let now = state.active.fetch_add(1, Ordering::SeqCst) + 1;
        state.peak.fetch_max(now, Ordering::SeqCst);
        Self(state)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl JellyfinApi for TestApi {
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send {
        self.fake.check_server(address)
    }

    fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> impl Future<Output = Result<AuthenticationResult, ApiError>> + Send {
        self.fake.authenticate_by_name(server, username, password)
    }

    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send {
        self.fake.current_user(session)
    }

    fn libraries(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<Vec<Library>, ApiError>> + Send {
        self.fake.libraries(session)
    }

    fn items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> impl Future<Output = Result<Page<Item>, ApiError>> + Send {
        self.fake.items(session, query)
    }

    fn item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> impl Future<Output = Result<Item, ApiError>> + Send {
        self.fake.item(session, id)
    }

    fn resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.fake.resume(session, limit)
    }

    fn next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.fake.next_up(session, limit)
    }

    fn latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.fake.latest(session, parent, limit)
    }

    fn seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.fake.seasons(session, series)
    }

    fn episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.fake.episodes(session, series, season)
    }

    fn image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> impl Future<Output = Result<Image, ApiError>> + Send {
        let state = Arc::clone(&self.state);
        let fake = self.fake.clone();
        let (session, request) = (session.clone(), request.clone());
        async move {
            state.image_calls.fetch_add(1, Ordering::SeqCst);
            let _running = Running::start(&state);
            let delay = *state.delay.lock().expect("lock");
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            let served = state.served.lock().expect("lock").clone();
            match served {
                Some(bytes) => Ok(Image {
                    bytes,
                    content_type: "image/png".to_owned(),
                }),
                None => fake.image(&session, &request).await,
            }
        }
    }
}

pub fn server_url() -> ServerUrl {
    "http://fake.lan:8096".parse().expect("URL is valid")
}

/// A server with one movie library of `count` generated movies.
pub fn server(count: usize) -> TestApi {
    TestApi::new(
        FakeJellyfin::new(server_url())
            .with_user("alice", "hunter2")
            .with_generated_library("Movies", count),
    )
}

pub async fn log_in(api: &TestApi) -> Session {
    let result = api
        .authenticate_by_name(&server_url(), "alice", &Password::new("hunter2"))
        .await
        .expect("alice can log in");
    Session::new(server_url(), result.access_token)
}

/// The primary-image request for the `n`th item of the library, with its tag.
pub async fn poster(api: &TestApi, session: &Session, n: usize) -> ImageRequest {
    let library = api.fake.library_named("Movies").expect("library exists");
    let query = ItemsQuery::default().in_parent(library.id).page(n, 1);
    let page = api.items(session, &query).await.expect("items");
    let item = page.items.first().expect("item exists");
    let tag = item.image_tags.primary.clone().expect("has a poster");
    ImageRequest::new(item.id.clone(), ImageKind::Primary, 300).with_tag(tag)
}

/// A cache over `api` in `dir`.
pub fn cache_in(api: &TestApi, dir: &TempDir, config: ImageCacheConfig) -> ImageCache<TestApi> {
    ImageCache::new(api.clone(), &CacheDir::new(dir.path()), config)
}
