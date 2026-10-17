//! Phase 3's "Done when" checks that CI can run, through the whole app:
//! `app::connect_with` with the real `JellyfinClient` and image cache
//! against a wiremock Jellyfin holding 2,000 generated movies with
//! placeholder posters, and the UI on Slint's testing backend.
//!
//! - The first row's posters are on screen within 1 s of launch, which
//!   includes checking the stored session and Home appearing.
//! - Opening the library and scrolling its grid end to end shows every item
//!   and asks the server for every page exactly once.
//!
//! The 60 fps half of the check needs a GPU: `examples/perf_grid.rs`, on the
//! TV. One test, since Slint's testing event loop starts once per process.

#[allow(dead_code)] // shared helpers; this binary uses only some
mod common;

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::time::{Duration, Instant};

use common::jellyfin::{self, RESUMABLE};
use common::{WINDOW, close_window, press, ui, wait_for, with_window};
use couchfin_core::CacheDir;
use couchfin_ui::{HomeView, LibraryView, MainWindow, Screen};
use slint::platform::Key;
use slint::{ComponentHandle, LogicalSize, Model};
use tokio::runtime::Runtime;

const ITEMS: usize = 2_000;
const ITEMS_I32: i32 = 2_000;
/// The first row must show this many posters, a screenful on a TV.
const FIRST_ROW_POSTERS: usize = 5;
const PAGE_SIZE: usize = 50;

fn launch(address: String, cache: std::path::PathBuf, runtime: tokio::runtime::Handle) {
    ui(move || {
        let (api, flow) = jellyfin::restoring(&address);
        let window = MainWindow::new().expect("window is created");
        window.window().set_size(LogicalSize::new(1920.0, 1080.0));
        app::connect_with(&window, flow, api, Some(CacheDir::new(cache)), &runtime);
        window.show().expect("window is shown");
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
    });
}

/// How many cards of Home's first row have their picture set.
fn first_row_posters() -> usize {
    with_window(|window| {
        let Some(row) = window.global::<HomeView>().get_rows().row_data(0) else {
            return 0;
        };
        row.cards
            .iter()
            .filter(|c| c.image.size().width > 0)
            .count()
    })
}

fn checks(runtime: &Runtime) {
    let server = runtime.block_on(jellyfin::mount(ITEMS));
    let cache = tempfile::tempdir().expect("temporary directory");
    // The clock starts at launch, before the stored session is checked and
    // Home appears, which is stricter than starting it when Home appears
    // (polling for that would be coarser than the posters take).
    let started = Instant::now();
    launch(
        server.uri(),
        cache.path().to_owned(),
        runtime.handle().clone(),
    );
    wait_for("Signed in as alice");
    let home_after = started.elapsed();
    let deadline = started + Duration::from_secs(10);
    while first_row_posters() < FIRST_ROW_POSTERS.min(RESUMABLE) {
        assert!(Instant::now() < deadline, "the first row never got posters");
        std::thread::sleep(Duration::from_millis(2));
    }
    let took = started.elapsed();
    println!(
        "Home seen after {} ms, first row posters set after {} ms (both from launch)",
        home_after.as_millis(),
        took.as_millis()
    );
    assert!(
        took < Duration::from_secs(1),
        "the first row took {took:?} from launch to show posters"
    );

    // Down to Libraries (past Recently Added) and open the grid.
    press(Key::DownArrow);
    press(Key::DownArrow);
    press(Key::Return);
    let deadline = Instant::now() + Duration::from_secs(10);
    while with_window(|w| {
        w.get_screen() != Screen::Library || w.global::<LibraryView>().get_item_count() != ITEMS_I32
    }) {
        assert!(Instant::now() < deadline, "the grid never opened");
        std::thread::sleep(Duration::from_millis(5));
    }

    // Scroll to the end the way a held D-pad does: a row per press.
    for _ in 0..(ITEMS / 4) {
        press(Key::DownArrow);
        std::thread::sleep(Duration::from_millis(2));
    }
    wait_for(jellyfin::movie_name(ITEMS - 1));

    // Every item arrived, and every page was asked for exactly once.
    let last = with_window(|w| {
        w.global::<LibraryView>()
            .get_items()
            .row_data(ITEMS - 1)
            .map(|card| card.title.to_string())
    });
    assert_eq!(last, Some(jellyfin::movie_name(ITEMS - 1)));
    let starts = runtime.block_on(jellyfin::page_requests(&server));
    let unique: BTreeSet<usize> = starts.iter().copied().collect();
    assert_eq!(
        starts.len(),
        unique.len(),
        "a page was asked for twice: {starts:?}"
    );
    let all: BTreeSet<usize> = (0..ITEMS / PAGE_SIZE).map(|p| p * PAGE_SIZE).collect();
    assert_eq!(unique, all, "every page was loaded");
    close_window();
}

#[test]
fn phase_3_done_when_checks() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let runtime = Runtime::new().expect("tokio runtime starts");
    let driver = std::thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| checks(&runtime)));
        let _ = slint::quit_event_loop();
        outcome
    });
    slint::run_event_loop_until_quit().expect("the event loop runs");
    if let Err(panic) = driver.join().expect("the driver thread finishes") {
        resume_unwind(panic);
    }
}
