//! The library grid and detail pages on the testing backend, with their
//! callbacks recorded as text, so the parity table can compare them.

use std::cell::RefCell;
use std::rc::Rc;

use couchfin_ui::{CardData, EpisodeData, LibraryView, MainWindow, MovieView, Screen, SeriesView};
use slint::{ComponentHandle, LogicalSize, Model, ModelNotify, ModelRc, ModelTracker, VecModel};

use super::{Calls, shown};

/// A model of `count` placeholder items that notes which places were read,
/// as the grid reads only the places it draws.
pub struct Counted {
    count: usize,
    read: Rc<RefCell<Vec<bool>>>,
    notify: ModelNotify,
}

impl Counted {
    pub fn new(count: usize) -> (Self, Rc<RefCell<Vec<bool>>>) {
        let read = Rc::new(RefCell::new(vec![false; count]));
        let model = Self {
            count,
            read: Rc::clone(&read),
            notify: ModelNotify::default(),
        };
        (model, read)
    }
}

impl Model for Counted {
    type Data = CardData;

    fn row_count(&self) -> usize {
        self.count
    }

    fn row_data(&self, row: usize) -> Option<CardData> {
        *self.read.borrow_mut().get_mut(row)? = true;
        Some(CardData {
            title: format!("Item {}", row + 1).into(),
            ..Default::default()
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

pub struct Library {
    pub window: MainWindow,
    pub activated: Calls<i32>,
    pub backs: Calls<()>,
    pub needed: Calls<(i32, i32)>,
    pub read: Rc<RefCell<Vec<bool>>>,
}

/// The library grid of `count` items in a window of the given size.
pub fn library_at(count: usize, width: f32, height: f32) -> Library {
    let window = shown(Screen::Library);
    window.window().set_size(LogicalSize::new(width, height));
    let view = window.global::<LibraryView>();
    let (model, read) = Counted::new(count);
    view.set_title("Movies".into());
    view.set_item_count(i32::try_from(count).expect("a sensible count"));
    view.set_items(ModelRc::new(model));
    let (activated, backs, needed) = (Calls::new(), Calls::new(), Calls::new());
    let recorder = activated.clone();
    view.on_item_activated(move |index| recorder.record(index));
    let recorder = backs.clone();
    view.on_back(move || recorder.record(()));
    let recorder = needed.clone();
    view.on_items_needed(move |first, last| recorder.record((first, last)));
    super::home::settle();
    Library {
        window,
        activated,
        backs,
        needed,
        read,
    }
}

/// The movie page, with its callbacks recorded as text.
pub fn movie() -> (MainWindow, Calls<String>) {
    let window = shown(Screen::Movie);
    window.window().set_size(LogicalSize::new(1920.0, 1080.0));
    let view = window.global::<MovieView>();
    view.set_title("Arrival".into());
    view.set_year("2016".into());
    view.set_runtime("1 h 56 min".into());
    view.set_rating("PG-13".into());
    view.set_overview("A linguist is recruited to talk to visitors.".into());
    let calls = Calls::new();
    let recorder = calls.clone();
    view.on_play(move || recorder.record("play".into()));
    let recorder = calls.clone();
    view.on_back(move || recorder.record("back".into()));
    (window, calls)
}

/// The series page with three seasons and `episodes` episodes in the
/// selected one, with its callbacks recorded as text.
pub fn series(episodes: usize) -> (MainWindow, Calls<String>) {
    let window = shown(Screen::Series);
    window.window().set_size(LogicalSize::new(1920.0, 1080.0));
    let view = window.global::<SeriesView>();
    view.set_title("Severance".into());
    view.set_year("2022".into());
    view.set_runtime("50 min".into());
    view.set_rating("TV-MA".into());
    view.set_overview("Office workers split their memories.".into());
    view.set_seasons(ModelRc::new(VecModel::from(vec![
        "Season 1".into(),
        "Season 2".into(),
        "Season 3".into(),
    ])));
    let list: Vec<_> = (1..=episodes)
        .map(|n| EpisodeData {
            number: i32::try_from(n).expect("a sensible count"),
            title: format!("Episode title {n}").into(),
            runtime: "42 min".into(),
            watched: n == 1,
        })
        .collect();
    view.set_episodes(ModelRc::new(VecModel::from(list)));
    let calls = Calls::new();
    let recorder = calls.clone();
    view.on_season_selected(move |index| recorder.record(format!("season({index})")));
    let recorder = calls.clone();
    view.on_episode_activated(move |index| recorder.record(format!("episode({index})")));
    let recorder = calls.clone();
    view.on_back(move || recorder.record("back".into()));
    (window, calls)
}

/// The library grid with its callbacks recorded as text.
pub fn library_calls(count: usize) -> (MainWindow, Calls<String>) {
    let library = library_at(count, 1920.0, 1080.0);
    let calls = Calls::new();
    let view = library.window.global::<LibraryView>();
    let recorder = calls.clone();
    view.on_item_activated(move |index| recorder.record(format!("item({index})")));
    let recorder = calls.clone();
    view.on_back(move || recorder.record("back".into()));
    let recorder = calls.clone();
    view.on_items_needed(move |first, last| recorder.record(format!("needed({first},{last})")));
    (library.window, calls)
}
