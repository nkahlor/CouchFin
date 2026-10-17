//! Phase 3's frame-rate check, for the TV: scrolls a 2,000-item library grid
//! end to end in the real UI and prints how long the frames took.
//!
//! It starts an in-process wiremock Jellyfin with 2,000 generated movies and
//! placeholder posters (the same fixture as `tests/phase3_browse.rs`), runs
//! the real window fullscreen with the real renderer, opens the grid and
//! presses Down at controller repeat speed until the last row, measuring
//! the time between rendered frames with Slint's rendering notifier. GitHub's
//! runners have no GPU, so a person runs this on the TV:
//!
//! ```sh
//! cargo run --release -p app --example perf_grid
//! ```
//!
//! The target is a 95th-percentile frame time of at most 16.7 ms.

// Statistics on a few thousand frames: the casts can't lose anything.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

#[allow(dead_code)] // the test uses the rest
#[path = "../tests/common/jellyfin.rs"]
mod jellyfin;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use couchfin_core::CacheDir;
use couchfin_ui::{HomeView, Intent, LibraryView, MainWindow, Screen, navigate};
use slint::{ComponentHandle, Model, RenderingState, Timer, TimerMode};

const ITEMS: usize = 2_000;
const ITEMS_I32: i32 = 2_000;
/// How often a held D-pad repeats (decision 010).
const REPEAT: Duration = Duration::from_millis(60);
/// A frame this long or longer misses 60 fps.
const BUDGET: Duration = Duration::from_micros(16_700);

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    WaitForHome,
    OpenGrid,
    WaitForGrid,
    Scroll(usize),
    Done,
}

fn report(frames: &[Duration]) {
    if frames.is_empty() {
        println!("no frames were drawn while scrolling");
        return;
    }
    let mut sorted = frames.to_vec();
    sorted.sort();
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
    let mean = frames.iter().sum::<Duration>() / u32::try_from(frames.len()).unwrap_or(u32::MAX);
    let over = frames.iter().filter(|frame| **frame > BUDGET).count();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("frames:            {}", frames.len());
    println!("mean:              {:.2} ms", ms(mean));
    println!("p95:               {:.2} ms", ms(at(0.95)));
    println!("p99:               {:.2} ms", ms(at(0.99)));
    println!("worst:             {:.2} ms", ms(at(1.0)));
    println!(
        "over 16.7 ms:      {over} ({:.1}%)",
        over as f64 * 100.0 / frames.len() as f64
    );
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("warn")
        .init();
    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    let server = runtime.block_on(jellyfin::mount(ITEMS));
    let cache = tempfile::tempdir().context("a cache directory")?;
    let (api, flow) = jellyfin::restoring(&server.uri());

    let window = MainWindow::new().context("creating the main window")?;
    window.window().set_fullscreen(true);
    let _flow = app::connect_with(
        &window,
        flow,
        api,
        Some(CacheDir::new(cache.path())),
        runtime.handle(),
    );

    // Frame times, recorded only while the grid scrolls.
    let frames = Rc::new(RefCell::new(Vec::<Duration>::new()));
    let stage = Rc::new(RefCell::new(Stage::WaitForHome));
    let last_frame = Rc::new(RefCell::new(None::<Instant>));
    window
        .window()
        .set_rendering_notifier({
            let (frames, stage, last) = (frames.clone(), stage.clone(), last_frame.clone());
            move |state, _| {
                if !matches!(state, RenderingState::AfterRendering) {
                    return;
                }
                let now = Instant::now();
                let previous = last.borrow_mut().replace(now);
                if let (Some(previous), Stage::Scroll(_)) = (previous, *stage.borrow()) {
                    frames.borrow_mut().push(now - previous);
                }
            }
        })
        .context("this renderer can't report frames")?;

    // Drives the run from the event loop, so presses reach the window as a
    // controller's would.
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, REPEAT, {
        let (weak, stage, frames) = (window.as_weak(), stage.clone(), frames.clone());
        let started = Instant::now();
        move || {
            let Some(window) = weak.upgrade() else { return };
            let next = match *stage.borrow() {
                Stage::WaitForHome => {
                    let rows = window.global::<HomeView>().get_rows();
                    let ready = window.get_screen() == Screen::Home
                        && rows
                            .row_data(0)
                            .is_some_and(|row| row.cards.iter().any(|c| c.image.size().width > 0));
                    ready.then_some(Stage::OpenGrid)
                }
                Stage::OpenGrid => {
                    // Continue Watching, Recently Added, then Libraries.
                    for _ in 0..2 {
                        navigate(&window, Intent::Down);
                    }
                    navigate(&window, Intent::Activate);
                    Some(Stage::WaitForGrid)
                }
                Stage::WaitForGrid => (window.get_screen() == Screen::Library
                    && window.global::<LibraryView>().get_item_count() == ITEMS_I32)
                    .then_some(Stage::Scroll(0)),
                Stage::Scroll(n) => {
                    navigate(&window, Intent::Down);
                    // Rows of at least four items, so this reaches the end.
                    Some(if n >= ITEMS / 4 {
                        Stage::Done
                    } else {
                        Stage::Scroll(n + 1)
                    })
                }
                Stage::Done => {
                    println!(
                        "scrolled {ITEMS} items in {:.1} s",
                        started.elapsed().as_secs_f64()
                    );
                    report(&frames.borrow());
                    let _ = slint::quit_event_loop();
                    None
                }
            };
            if let Some(next) = next {
                *stage.borrow_mut() = next;
            }
        }
    });

    window.run().context("running the event loop")
}
