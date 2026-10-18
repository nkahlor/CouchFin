//! Drives the real app wiring from a test thread while Slint's testing
//! backend runs the event loop on the main thread, as `couchfin` does.

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use couchfin_ui::MainWindow;
use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

pub mod jellyfin;
pub mod steam;

thread_local! {
    /// The window under test. It lives on the UI thread, so only closures
    /// passed to `ui` touch it.
    pub static WINDOW: RefCell<Option<MainWindow>> = const { RefCell::new(None) };
}

/// Runs `f` on the UI thread and returns its result.
pub fn ui<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    let (send, receive) = mpsc::channel();
    slint::invoke_from_event_loop(move || {
        let _ = send.send(f());
    })
    .expect("the event loop is running");
    receive
        .recv_timeout(Duration::from_secs(10))
        .expect("the UI thread answers")
}

/// Runs `f` with the window under test, on the UI thread.
pub fn with_window<R: Send + 'static>(f: impl FnOnce(&MainWindow) -> R + Send + 'static) -> R {
    ui(move || WINDOW.with(|window| f(window.borrow().as_ref().expect("a window is open"))))
}

/// Closes the window under test, as quitting the app would.
pub fn close_window() {
    ui(|| {
        if let Some(window) = WINDOW.with(|window| window.borrow_mut().take()) {
            window.hide().expect("window hides");
        }
    });
}

fn labels(window: &MainWindow) -> Vec<String> {
    i_slint_backend_testing::ElementQuery::from_root(window)
        .find_all()
        .iter()
        .filter_map(ElementHandle::accessible_label)
        .map(|label| label.to_string())
        .filter(|label| !label.is_empty())
        .collect()
}

/// Waits until an element with this accessible label is on screen.
pub fn wait_for(label: impl Into<String>) {
    let label: String = label.into();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let wanted = label.clone();
        let (found, shown) = with_window(move |window| {
            let shown = labels(window);
            (shown.contains(&wanted), shown)
        });
        if found {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{label:?} never appeared; the screen shows {shown:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Presses and releases a key in the window under test.
pub fn press(key: impl Into<char>) {
    let text: slint::SharedString = key.into().into();
    with_window(move |window| {
        let window = window.window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
    });
}

/// Types text into the focused field.
pub fn type_text(text: &str) {
    for ch in text.chars() {
        press(ch);
    }
}

/// Empties the focused field of `len` characters.
pub fn erase(len: usize) {
    for _ in 0..len {
        press(Key::Backspace);
    }
}

/// The "Selected: ..." note on Home, if any.
pub fn selected() -> Option<String> {
    with_window(|window| {
        labels(window)
            .into_iter()
            .find(|label| label.starts_with("Selected: "))
    })
}
