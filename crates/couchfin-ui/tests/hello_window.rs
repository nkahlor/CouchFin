//! Drives `HelloWindow` through its public API on Slint's headless testing
//! backend: it shows "Hello", and Back (Escape or Backspace) asks to quit.

use std::cell::Cell;
use std::rc::Rc;

use couchfin_ui::HelloWindow;
use i_slint_backend_testing::{ElementHandle, init_no_event_loop};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

/// Shows a fresh window on the testing backend and counts quit requests.
fn shown_window() -> (HelloWindow, Rc<Cell<u32>>) {
    // Slint's platform is per thread, and each test runs on its own thread.
    init_no_event_loop();
    let window = HelloWindow::new().expect("window is created");
    let quits = Rc::new(Cell::new(0));
    let counter = Rc::clone(&quits);
    window.on_quit_requested(move || counter.set(counter.get() + 1));
    window.show().expect("window is shown");
    (window, quits)
}

fn press(window: &HelloWindow, key: impl Into<char>) {
    let text = slint::SharedString::from(key.into());
    let window = window.window();
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}

#[test]
fn says_hello() {
    let (window, _) = shown_window();
    let greetings: Vec<_> = ElementHandle::find_by_accessible_label(&window, "Hello").collect();
    assert_eq!(greetings.len(), 1);
}

#[test]
fn escape_requests_quit() {
    let (window, quits) = shown_window();
    press(&window, Key::Escape);
    assert_eq!(quits.get(), 1);
}

#[test]
fn backspace_requests_quit() {
    let (window, quits) = shown_window();
    press(&window, Key::Backspace);
    assert_eq!(quits.get(), 1);
}

#[test]
fn other_keys_do_not_request_quit() {
    let (window, quits) = shown_window();
    press(&window, 'a');
    press(&window, Key::Return);
    assert_eq!(quits.get(), 0);
}
