//! Runs couchfin-core's library flow and image cache for the browsing
//! screens: started when sign-in reaches Home, dropped when it leaves or the
//! server rejects the token.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use couchfin_api::Session;
use couchfin_core::{
    CacheDir, ImageCache, ImageCacheConfig, ImageLoad, ImageRef, JellyfinApi, LibraryAction,
    LibraryFlow,
};
use couchfin_ui::{Command, Feed, Guard, ImageLoader, MainWindow, install};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

type Expired = Box<dyn Fn() + Send + Sync>;

/// Cancels its task when dropped: the load's guard.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct Active<A: JellyfinApi> {
    flow: Arc<LibraryFlow<A>>,
    /// Cleared on stop, so a late publish from a dropped flow is ignored.
    live: Arc<AtomicBool>,
    cache: Option<(Arc<ImageCache<A>>, Session)>,
}

/// What the callbacks and the image loader share.
struct Shared<A: JellyfinApi> {
    active: Mutex<Option<Active<A>>>,
    runtime: Handle,
}

impl<A: JellyfinApi> Shared<A> {
    fn active(&self) -> MutexGuard<'_, Option<Active<A>>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Fetches pictures through the image cache of the running library.
struct CacheLoader<A: JellyfinApi>(Arc<Shared<A>>);

impl<A: JellyfinApi + 'static> ImageLoader for CacheLoader<A> {
    fn load(
        &self,
        image: &ImageRef,
        max_width: u32,
        done: Box<dyn FnOnce(ImageLoad) + Send>,
    ) -> Guard {
        let cache = self
            .0
            .active()
            .as_ref()
            .and_then(|active| active.cache.clone());
        let Some((cache, session)) = cache else {
            done(ImageLoad::NoImage);
            return Guard::new(());
        };
        let request = image.request(max_width);
        let task = self.0.runtime.spawn(async move {
            match cache.load(&session, &request).await {
                Ok(loaded) => done(loaded),
                Err(error) => {
                    tracing::warn!(%error, "couldn't load an image");
                    done(ImageLoad::NoImage);
                }
            }
        });
        Guard::new(AbortOnDrop(task))
    }
}

/// The library side of the app.
pub(crate) struct Library<A: JellyfinApi> {
    api: A,
    cache_dir: Option<CacheDir>,
    shared: Arc<Shared<A>>,
    feed: Feed,
    expired: OnceLock<Expired>,
}

impl<A: JellyfinApi + Clone + 'static> Library<A> {
    /// Installs the browsing screens on `window`. Library actions from them
    /// are spawned on `runtime`.
    pub(crate) fn new(
        window: &MainWindow,
        api: A,
        cache_dir: Option<CacheDir>,
        runtime: &Handle,
    ) -> Arc<Self> {
        let shared = Arc::new(Shared {
            active: Mutex::new(None),
            runtime: runtime.clone(),
        });
        let feed = install(window, Arc::new(CacheLoader(Arc::clone(&shared))), {
            let shared = Arc::clone(&shared);
            move |command| {
                if let Command::Library(action) = command {
                    dispatch(&shared, action);
                }
            }
        });
        Arc::new(Self {
            api,
            cache_dir,
            shared,
            feed,
            expired: OnceLock::new(),
        })
    }

    /// What to do when the server rejects the token: the app's login flow
    /// takes it from there.
    pub(crate) fn on_expired(&self, expired: impl Fn() + Send + Sync + 'static) {
        // Set once, right after construction.
        let _ = self.expired.set(Box::new(expired));
    }

    /// Sign-in reached Home: show the library, starting it first if needed.
    pub(crate) fn start(self: &Arc<Self>, session: Session) {
        let mut active = self.shared.active();
        if active.is_none() {
            let live = Arc::new(AtomicBool::new(true));
            let flow = Arc::new(
                LibraryFlow::new(self.api.clone(), session.clone()).on_change({
                    let (feed, live) = (self.feed.clone(), Arc::clone(&live));
                    let me = Arc::downgrade(self);
                    move |state| {
                        if !live.load(Ordering::SeqCst) {
                            return;
                        }
                        feed.state(state.clone());
                        if state.session_expired
                            && let Some(me) = me.upgrade()
                        {
                            // Off the flow's lock: dropping it from inside
                            // its own observer would deadlock.
                            let runtime = me.shared.runtime.clone();
                            runtime.spawn(async move { me.expire() });
                        }
                    }
                }),
            );
            let cache = self.cache_dir.as_ref().map(|dir| {
                let cache = ImageCache::new(self.api.clone(), dir, ImageCacheConfig::default());
                (Arc::new(cache), session)
            });
            *active = Some(Active {
                flow: Arc::clone(&flow),
                live,
                cache,
            });
            self.shared.runtime.spawn(async move { flow.start().await });
        }
        drop(active);
        self.feed.show();
    }

    /// Sign-in left Home: drop the library and its pictures.
    pub(crate) fn stop(&self) {
        let dropped = self.shared.active().take();
        if let Some(active) = dropped {
            active.live.store(false, Ordering::SeqCst);
        }
        self.feed.reset();
    }

    fn expire(&self) {
        self.stop();
        if let Some(expired) = self.expired.get() {
            expired();
        }
    }
}

fn dispatch<A: JellyfinApi + 'static>(shared: &Shared<A>, action: LibraryAction) {
    let flow = shared.active().as_ref().map(|a| Arc::clone(&a.flow));
    if let Some(flow) = flow {
        shared
            .runtime
            .spawn(async move { flow.dispatch(action).await });
    }
}
