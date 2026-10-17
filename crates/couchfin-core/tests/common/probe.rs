//! A fake Jellyfin that counts calls and can hold a call until the test
//! opens its gate, for testing concurrency and stale results.
#![allow(dead_code, clippy::unwrap_used)]

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use couchfin_api::{
    AuthenticationResult, Image, ImageRequest, Item, ItemId, ItemsQuery, Library, Page,
    ServerCheck, Session,
};
use couchfin_core::fake::FakeJellyfin;
use couchfin_core::{
    ApiError, JellyfinApi, LibraryAction, LibraryFlow, LibraryState, Password, ServerUrl, User,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct Probe {
    pub fake: FakeJellyfin,
    calls: Arc<Mutex<Vec<String>>>,
    gates: Arc<Mutex<HashMap<&'static str, Arc<Semaphore>>>>,
}

impl Probe {
    pub fn new(fake: FakeJellyfin) -> Self {
        Self {
            fake,
            calls: Arc::default(),
            gates: Arc::default(),
        }
    }

    /// Holds every later call of `method` until [`Probe::open`].
    pub fn close(&self, method: &'static str) {
        self.gates
            .lock()
            .unwrap()
            .insert(method, Arc::new(Semaphore::new(0)));
    }

    pub fn open(&self, method: &'static str) {
        if let Some(gate) = self.gates.lock().unwrap().get(method) {
            gate.add_permits(10_000);
        }
    }

    /// How many times `method` was called.
    pub fn calls(&self, method: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == method)
            .count()
    }

    async fn enter(&self, method: &'static str) {
        self.calls.lock().unwrap().push(method.to_owned());
        let gate = self.gates.lock().unwrap().get(method).cloned();
        if let Some(gate) = gate {
            gate.acquire().await.unwrap().forget();
        }
    }

    /// A session for the fake's user `alice` (password `hunter2`).
    pub async fn session(&self) -> Session {
        let server = self.fake.server().clone();
        let login = self
            .fake
            .authenticate_by_name(&server, "alice", &Password::new("hunter2"))
            .await
            .unwrap();
        Session::new(server, login.access_token)
    }
}

pub fn server() -> ServerUrl {
    "http://jellyfin.lan:8096".parse().expect("valid URL")
}

/// Movies (Arrival, Dune), Shows (Severance: two seasons of three episodes)
/// and an empty library. Arrival and one episode are partly watched, and
/// one episode is up next.
pub fn catalog() -> FakeJellyfin {
    use couchfin_api::LibraryKind;
    use std::time::Duration;
    FakeJellyfin::new(server())
        .with_user("alice", "hunter2")
        .with_library("Movies", LibraryKind::Movies)
        .with_library("Shows", LibraryKind::TvShows)
        .with_library("Empty", LibraryKind::Movies)
        .with_movie("Movies", "Arrival")
        .with_movie("Movies", "Dune")
        .with_series("Shows", "Severance", 2, 3)
        .with_resume("Arrival", Duration::from_mins(50))
        .with_resume("Severance S2E3", Duration::from_mins(15))
        .with_next_up("Severance S1E2")
}

/// A library flow over a [`Probe`], with a log of every state it published.
pub struct Harness {
    pub flow: Arc<LibraryFlow<Probe>>,
    pub probe: Probe,
    pub seen: Arc<Mutex<Vec<LibraryState>>>,
}

impl Harness {
    pub async fn new(fake: FakeJellyfin, page_size: usize) -> Self {
        let probe = Probe::new(fake);
        let session = probe.session().await;
        let seen: Arc<Mutex<Vec<LibraryState>>> = Arc::default();
        let log = Arc::clone(&seen);
        let flow = LibraryFlow::new(probe.clone(), session)
            .with_page_size(page_size)
            .on_change(move |state| log.lock().unwrap().push(state.clone()));
        Self {
            flow: Arc::new(flow),
            probe,
            seen,
        }
    }

    /// The fixture catalog, started: Home has loaded.
    pub async fn started() -> Self {
        let harness = Self::new(catalog(), 50).await;
        harness.flow.start().await;
        harness
    }

    pub async fn send(&self, action: LibraryAction) {
        self.flow.dispatch(action).await;
    }

    /// Runs `action` in the background, for tests that hold a call open.
    pub fn spawn(&self, action: LibraryAction) -> tokio::task::JoinHandle<()> {
        let flow = Arc::clone(&self.flow);
        tokio::spawn(async move { flow.dispatch(action).await })
    }

    /// Lets background tasks run until the state satisfies `done`.
    pub async fn wait_until(&self, done: impl Fn(&LibraryState) -> bool) {
        for _ in 0..1_000 {
            if done(&self.flow.state()) {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the state never matched; last: {:#?}", self.flow.state());
    }

    pub fn state(&self) -> LibraryState {
        self.flow.state()
    }

    /// The Home row of this kind's index.
    pub fn row_index(&self, title: &str) -> usize {
        self.state()
            .home()
            .rows
            .iter()
            .position(|row| row.title == title)
            .unwrap_or_else(|| panic!("no row {title:?}"))
    }
}

macro_rules! gated {
    ($self:ident, $name:literal, $call:ident($($arg:expr),*)) => {{
        let this = $self.clone();
        async move {
            this.enter($name).await;
            this.fake.$call($($arg),*).await
        }
    }};
}

impl JellyfinApi for Probe {
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send {
        let address = address.clone();
        gated!(self, "check_server", check_server(&address))
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
        let session = session.clone();
        gated!(self, "libraries", libraries(&session))
    }

    fn items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> impl Future<Output = Result<Page<Item>, ApiError>> + Send {
        let (session, query) = (session.clone(), query.clone());
        gated!(self, "items", items(&session, &query))
    }

    fn item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> impl Future<Output = Result<Item, ApiError>> + Send {
        let (session, id) = (session.clone(), id.clone());
        gated!(self, "item", item(&session, &id))
    }

    fn resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        let session = session.clone();
        gated!(self, "resume", resume(&session, limit))
    }

    fn next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        let session = session.clone();
        gated!(self, "next_up", next_up(&session, limit))
    }

    fn latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        let (session, parent) = (session.clone(), parent.clone());
        gated!(self, "latest", latest(&session, &parent, limit))
    }

    fn seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        let (session, series) = (session.clone(), series.clone());
        gated!(self, "seasons", seasons(&session, &series))
    }

    fn episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        let (session, series, season) = (session.clone(), series.clone(), season.clone());
        gated!(self, "episodes", episodes(&session, &series, &season))
    }

    fn image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> impl Future<Output = Result<Image, ApiError>> + Send {
        self.fake.image(session, request)
    }
}
