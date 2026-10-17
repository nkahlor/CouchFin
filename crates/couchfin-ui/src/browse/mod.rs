//! Binds Home, the library grid and the detail pages to couchfin-core's
//! [`LibraryState`], and fills their pictures through an [`ImageLoader`].
//!
//! One-way, as for sign-in: core's state goes in through a [`Feed`], the
//! screens' callbacks come out as [`Command::Library`] actions, and the UI
//! decides nothing about what to show. See decision 015.

mod detail;
mod feed;
mod format;
mod geometry;
mod grid;
mod home;
mod loader;
mod view;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use couchfin_core::{LibraryAction, LibraryState};
use slint::ComponentHandle;

pub use feed::Feed;
pub use loader::{Guard, ImageLoader};

use crate::Command;
use crate::generated::{HomeView, LibraryView, MainWindow, MovieView, SeriesView};
use feed::Msg;
use view::Browser;

/// The state behind the callbacks and the feed. Lives as long as the window
/// does, through the callbacks that hold it.
pub(crate) struct Inner {
    window: slint::Weak<MainWindow>,
    send: Box<dyn Fn(Command)>,
    browser: RefCell<Browser>,
}

/// Binds the browsing screens of `window` and returns the [`Feed`] that fills
/// them.
///
/// - `loader` fetches pictures, only for cards on screen or just off it, and
///   cancels the loads for ones that scroll away.
/// - `on_command` receives a [`Command::Library`] for each action the
///   screens send: opening a card, focus moves, paging the grid, choosing a
///   season, going back.
///
/// Sign-in's own callbacks, and Back on Home, stay with [`bind`](crate::bind).
/// Call it on the UI thread, once per window.
pub fn install(
    window: &MainWindow,
    loader: Arc<dyn ImageLoader>,
    on_command: impl Fn(Command) + 'static,
) -> Feed {
    let feed = Feed::new();
    let inner = Rc::new(Inner {
        window: window.as_weak(),
        send: Box::new(on_command),
        browser: RefCell::new(Browser::new(loader, feed.clone())),
    });
    feed.register(Rc::downgrade(&inner));
    inner.browser.borrow().install(window);

    let home = window.global::<HomeView>();
    home.on_card_activated({
        let inner = Rc::clone(&inner);
        move |row, index| {
            if let (Ok(row), Ok(index)) = (usize::try_from(row), usize::try_from(index)) {
                (inner.send)(Command::Library(LibraryAction::OpenCard { row, index }));
            }
        }
    });
    home.on_focus_changed({
        let inner = Rc::clone(&inner);
        move |row, index| {
            if let (Ok(row), Ok(index)) = (usize::try_from(row), usize::try_from(index)) {
                (inner.send)(Command::Library(LibraryAction::FocusChanged { row, index }));
            }
            inner.refresh();
        }
    });

    let library = window.global::<LibraryView>();
    library.on_items_needed({
        let inner = Rc::clone(&inner);
        move |first, last| inner.items_needed(first, last)
    });
    library.on_item_activated({
        let inner = Rc::clone(&inner);
        move |index| {
            if let Ok(index) = usize::try_from(index) {
                (inner.send)(Command::Library(LibraryAction::OpenGridItem(index)));
            }
        }
    });
    library.on_back({
        let inner = Rc::clone(&inner);
        move || inner.back()
    });

    window.global::<MovieView>().on_back({
        let inner = Rc::clone(&inner);
        move || inner.back()
    });
    window
        .global::<MovieView>()
        .on_play(|| tracing::info!("Play pressed; playback arrives in Phase 4"));

    let series = window.global::<SeriesView>();
    series.on_season_selected({
        let inner = Rc::clone(&inner);
        move |index| {
            if let Ok(index) = usize::try_from(index) {
                inner.browser.borrow_mut().picked_season(index);
                (inner.send)(Command::Library(LibraryAction::SelectSeason(index)));
            }
        }
    });
    series.on_episode_activated(|index| {
        tracing::info!(index, "Episode activated; playback arrives in Phase 4");
    });
    series.on_back({
        let inner = Rc::clone(&inner);
        move || inner.back()
    });
    feed
}

impl Inner {
    fn back(&self) {
        (self.send)(Command::Library(LibraryAction::Back));
    }

    fn refresh(&self) {
        if let Some(window) = self.window.upgrade() {
            self.browser.borrow_mut().refresh(&window);
        }
    }

    fn items_needed(&self, first: i32, last: i32) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let (Ok(first), Ok(last)) = (usize::try_from(first), usize::try_from(last)) else {
            return;
        };
        let range = self.browser.borrow_mut().visible(&window, first, last);
        if let Some((first, last)) = range {
            (self.send)(Command::Library(LibraryAction::NeedItems { first, last }));
        }
    }

    /// Applies what the feed collected, in order.
    fn receive(&self, messages: Vec<Msg>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let mut messages = messages.into_iter().peekable();
        let mut browser = self.browser.borrow_mut();
        while let Some(message) = messages.next() {
            match message {
                // A newer state is already waiting: skip this one.
                Msg::State(_) if matches!(messages.peek(), Some(Msg::State(_))) => {}
                Msg::State(state) => browser.apply(&window, *state),
                Msg::Show => browser.show(&window),
                Msg::Reset => browser.reset(&window),
                Msg::Image {
                    slot,
                    ticket,
                    loaded,
                } => browser.image(&window, &slot, ticket, &loaded),
            }
        }
    }
}

/// The window screen that shows the top of `state`.
pub(crate) fn state_screen(state: &LibraryState) -> crate::generated::Screen {
    use couchfin_core::ScreenState;

    use crate::generated::Screen;
    match state.current() {
        ScreenState::Home(_) => Screen::Home,
        ScreenState::Library(_) => Screen::Library,
        ScreenState::Movie(_) => Screen::Movie,
        ScreenState::Series(_) => Screen::Series,
    }
}
