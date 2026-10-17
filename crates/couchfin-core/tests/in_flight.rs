//! Actions taken while a request is still running: the stale result must not
//! overwrite what the user has moved on to.

use std::future::Future;
use std::sync::Arc;

use couchfin_api::{
    AuthenticationResult, Image, ImageRequest, Item, ItemId, ItemsQuery, Library, Page,
    ServerCheck, Session,
};
use couchfin_core::fake::{FakeJellyfin, MemorySettings, MemoryTokens};
use couchfin_core::{
    ApiError, EnterAddress, JellyfinApi, LoginAction, LoginFlow, LoginScreen, Password, Request,
    ServerUrl, Settings, User,
};
use tokio::sync::Semaphore;

/// The fake, except that `check_server` waits until the test opens the gate.
#[derive(Clone)]
struct Gated {
    inner: FakeJellyfin,
    gate: Arc<Semaphore>,
}

impl JellyfinApi for Gated {
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send {
        let (inner, gate, address) = (self.inner.clone(), Arc::clone(&self.gate), address.clone());
        async move {
            let _permit = gate.acquire().await;
            inner.check_server(&address).await
        }
    }

    fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> impl Future<Output = Result<AuthenticationResult, ApiError>> + Send {
        self.inner.authenticate_by_name(server, username, password)
    }

    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send {
        self.inner.current_user(session)
    }

    fn libraries(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<Vec<Library>, ApiError>> + Send {
        self.inner.libraries(session)
    }

    fn items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> impl Future<Output = Result<Page<Item>, ApiError>> + Send {
        self.inner.items(session, query)
    }

    fn item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> impl Future<Output = Result<Item, ApiError>> + Send {
        self.inner.item(session, id)
    }

    fn resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.inner.resume(session, limit)
    }

    fn next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.inner.next_up(session, limit)
    }

    fn latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.inner.latest(session, parent, limit)
    }

    fn seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.inner.seasons(session, series)
    }

    fn episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.inner.episodes(session, series, season)
    }

    fn image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> impl Future<Output = Result<Image, ApiError>> + Send {
        self.inner.image(session, request)
    }
}

fn is_checking(screen: &LoginScreen) -> bool {
    matches!(
        screen,
        LoginScreen::EnterAddress(EnterAddress {
            request: Request::InFlight,
            ..
        })
    )
}

#[tokio::test]
async fn change_server_while_a_check_is_in_flight_drops_its_result() {
    let server: ServerUrl = "http://jellyfin.lan:8096".parse().unwrap();
    let gate = Arc::new(Semaphore::new(0));
    let api = Gated {
        inner: FakeJellyfin::new(server),
        gate: Arc::clone(&gate),
    };
    let settings = MemorySettings::new();
    let flow = Arc::new(LoginFlow::new(
        api,
        Settings::load_or_create(&settings),
        settings,
        MemoryTokens::new(),
    ));
    flow.start().await;

    let check = tokio::spawn({
        let flow = Arc::clone(&flow);
        async move {
            flow.dispatch(LoginAction::SubmitAddress("jellyfin.lan:8096".into()))
                .await;
        }
    });
    while !is_checking(&flow.screen()) {
        tokio::task::yield_now().await;
    }
    // A second submit while the first is in flight is ignored.
    flow.dispatch(LoginAction::SubmitAddress("other.lan".into()))
        .await;
    assert!(is_checking(&flow.screen()));

    flow.dispatch(LoginAction::ChangeServer).await;
    gate.add_permits(1);
    check.await.unwrap();

    assert_eq!(
        flow.screen(),
        LoginScreen::EnterAddress(EnterAddress {
            address: "jellyfin.lan:8096".to_owned(),
            request: Request::Idle,
        })
    );
}
