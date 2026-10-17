//! Library states from core's real `LibraryFlow` over its fake Jellyfin, a
//! window with the browsing screens installed, and a fake image loader that
//! records what was asked and lets a test finish each load.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use couchfin_core::fake::{FakeJellyfin, MemorySettings, MemoryTokens};
use couchfin_core::{
    ImageLoad, ImageRef, LibraryAction, LibraryFlow, LibraryState, LoginAction, LoginFlow,
    Password, Pixels, Settings,
};
use couchfin_ui::{Feed, Guard, ImageLoader, MainWindow, bind, install, render};
use slint::ComponentHandle;
use tokio::runtime::Runtime;

use super::flow::{describe, home};
use super::{Calls, init};

/// A flow over a fake server with one movie library, "Big", of `movies`
/// generated movies, "Big 00001" of them half watched.
pub struct World {
    runtime: Runtime,
    flow: LibraryFlow<FakeJellyfin>,
}

impl World {
    pub fn new(movies: usize) -> Self {
        let fake = FakeJellyfin::new("http://jellyfin.lan:8096".parse().expect("valid URL"))
            .with_user("alice", "hunter2")
            .with_generated_library("Big", movies)
            .with_resume("Big 00001", Duration::from_secs(600));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let flow = runtime.block_on(async {
            let (settings, tokens) = (MemorySettings::new(), MemoryTokens::new());
            let login = LoginFlow::new(
                fake.clone(),
                Settings::load_or_create(&settings),
                settings,
                tokens,
            );
            login.start().await;
            login
                .dispatch(LoginAction::SubmitAddress("jellyfin.lan:8096".into()))
                .await;
            login
                .dispatch(LoginAction::SubmitLogin {
                    username: "alice".into(),
                    password: Password::new("hunter2"),
                })
                .await;
            let session = login.session().expect("logged in");
            let flow = LibraryFlow::new(fake, session).with_page_size(50);
            flow.start().await;
            flow
        });
        Self { runtime, flow }
    }

    pub fn send(&self, action: LibraryAction) {
        self.runtime.block_on(self.flow.dispatch(action));
    }

    pub fn state(&self) -> LibraryState {
        self.flow.state()
    }

    /// Opens the "Big" tile on Home: the library grid.
    pub fn open_big(&self) {
        let libraries = self
            .state()
            .home()
            .rows
            .iter()
            .position(|row| row.title == "Libraries")
            .expect("a Libraries row");
        self.send(LibraryAction::OpenCard {
            row: libraries,
            index: 0,
        });
    }
}

/// One image load as the loader saw it.
pub struct Load {
    pub image: ImageRef,
    pub width: u32,
    alive: Arc<AtomicBool>,
    done: Option<Box<dyn FnOnce(ImageLoad) + Send>>,
}

impl Load {
    /// Not cancelled (its guard is still held).
    pub fn wanted(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

/// An `ImageLoader` that starts nothing: it records each request and holds
/// its guard's state, and the test finishes loads one by one.
#[derive(Clone, Default)]
pub struct FakeLoader {
    loads: Arc<Mutex<Vec<Load>>>,
}

struct Held(Arc<AtomicBool>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl ImageLoader for FakeLoader {
    fn load(
        &self,
        image: &ImageRef,
        max_width: u32,
        done: Box<dyn FnOnce(ImageLoad) + Send>,
    ) -> Guard {
        let alive = Arc::new(AtomicBool::new(true));
        self.lock().push(Load {
            image: image.clone(),
            width: max_width,
            alive: Arc::clone(&alive),
            done: Some(done),
        });
        Guard::new(Held(alive))
    }
}

impl FakeLoader {
    fn lock(&self) -> MutexGuard<'_, Vec<Load>> {
        self.loads.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every load started so far, as (item, width, still wanted).
    pub fn started(&self) -> Vec<(String, u32, bool)> {
        self.lock()
            .iter()
            .map(|l| (l.image.item.as_str().to_owned(), l.width, l.wanted()))
            .collect()
    }

    /// The loads still wanted and not finished, by item.
    pub fn pending(&self) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|l| l.wanted() && l.done.is_some())
            .map(|l| l.image.item.as_str().to_owned())
            .collect()
    }

    /// The loads that were cancelled before they finished.
    pub fn cancelled(&self) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|l| !l.wanted() && l.done.is_some())
            .map(|l| l.image.item.as_str().to_owned())
            .collect()
    }

    /// The request of the `n`th load, for tests that need its image kind.
    pub fn image(&self, n: usize) -> ImageRef {
        self.lock()[n].image.clone()
    }

    /// Finishes a load that was cancelled, as a racing answer would.
    pub fn finish_cancelled(&self, item: &str, result: ImageLoad) {
        let done = self
            .lock()
            .iter_mut()
            .find(|l| !l.wanted() && l.image.item.as_str() == item)
            .and_then(|l| l.done.take())
            .unwrap_or_else(|| panic!("no cancelled load for {item}"));
        done(result);
    }

    /// Finishes the load for `item` that is still wanted.
    pub fn finish(&self, item: &str, result: ImageLoad) {
        let done = self
            .lock()
            .iter_mut()
            .find(|l| l.wanted() && l.image.item.as_str() == item)
            .and_then(|l| l.done.take())
            .unwrap_or_else(|| panic!("no pending load for {item}"));
        done(result);
    }
}

/// A small opaque picture.
pub fn picture() -> ImageLoad {
    ImageLoad::Ready(Pixels::from_rgba(2, 3, vec![200; 2 * 3 * 4]).expect("2x3 RGBA"))
}

/// A shown window with sign-in and the browsing screens installed. Commands
/// from both are recorded as text.
pub struct Shell {
    pub window: MainWindow,
    pub feed: Feed,
    pub loader: FakeLoader,
    pub commands: Calls<String>,
}

impl Shell {
    pub fn new() -> Self {
        init();
        let window = MainWindow::new().expect("window is created");
        let commands = Calls::new();
        let loader = FakeLoader::default();
        let recorder = commands.clone();
        bind(&window, move |command| recorder.record(describe(command)));
        let recorder = commands.clone();
        let feed = install(&window, Arc::new(loader.clone()), move |command| {
            recorder.record(describe(command));
        });
        render(&window, &home(None));
        window
            .window()
            .set_size(slint::LogicalSize::new(1920.0, 1080.0));
        window.show().expect("window is shown");
        Self {
            window,
            feed,
            loader,
            commands,
        }
    }

    /// Gives the screens `state`, shows the library and applies the queue.
    pub fn show(&self, state: LibraryState) {
        self.feed.state(state);
        self.feed.show();
        self.feed.pump();
        super::flow::settle();
    }

    /// Gives the screens a newer state.
    pub fn update(&self, state: LibraryState) {
        self.feed.state(state);
        self.feed.pump();
        super::flow::settle();
    }
}
