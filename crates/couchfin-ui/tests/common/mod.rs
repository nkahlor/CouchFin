//! Helpers shared by the screen tests: show `MainWindow` on Slint's headless
//! testing backend, press keys, and find elements the way a screen reader
//! would.

#![allow(dead_code)] // Each test binary uses a different subset.

pub mod flow;

use std::cell::RefCell;
use std::rc::Rc;

use couchfin_ui::{MainWindow, Screen};
use i_slint_backend_testing::{ElementHandle, ElementQuery, init_no_event_loop};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

/// Shows a fresh window on the testing backend, on the given screen.
pub fn shown(screen: Screen) -> MainWindow {
    // Slint's platform is per thread, and each test runs on its own thread.
    init_no_event_loop();
    let window = MainWindow::new().expect("window is created");
    window.set_screen(screen);
    window.show().expect("window is shown");
    window
}

fn send(window: &MainWindow, text: slint::SharedString) {
    let window = window.window();
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}

/// Presses and releases one key.
pub fn press(window: &MainWindow, key: impl Into<char>) {
    send(window, key.into().into());
}

/// Types text one character at a time, as a keyboard (or Steam's on-screen
/// keyboard) does.
pub fn type_text(window: &MainWindow, text: &str) {
    for ch in text.chars() {
        press(window, ch);
    }
}

/// Holds Shift and presses Tab.
pub fn shift_tab(window: &MainWindow) {
    let slint_window = window.window();
    let shift: slint::SharedString = Key::Shift.into();
    slint_window.dispatch_event(WindowEvent::KeyPressed {
        text: shift.clone(),
    });
    press(window, Key::Tab);
    slint_window.dispatch_event(WindowEvent::KeyReleased { text: shift });
}

/// Every element with this accessible label.
pub fn labelled(window: &MainWindow, label: &str) -> Vec<ElementHandle> {
    ElementHandle::find_by_accessible_label(window, label).collect()
}

/// Whether exactly one element with this accessible label is on screen.
pub fn shows(window: &MainWindow, label: &str) -> bool {
    labelled(window, label).len() == 1
}

/// The one element with this accessible label.
pub fn the(window: &MainWindow, label: &str) -> ElementHandle {
    let mut found = labelled(window, label);
    assert_eq!(found.len(), 1, "expected one element labelled {label:?}");
    found.remove(0)
}

/// Every element in the window, for checks that must hold everywhere.
pub fn all_elements(window: &MainWindow) -> Vec<ElementHandle> {
    ElementQuery::from_root(window).find_all()
}

/// Records each call of a callback, so tests can check what it was given.
pub struct Calls<T>(Rc<RefCell<Vec<T>>>);

impl<T> Clone for Calls<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Clone> Calls<T> {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }

    pub fn record(&self, value: T) {
        self.0.borrow_mut().push(value);
    }

    pub fn taken(&self) -> Vec<T> {
        self.0.borrow_mut().drain(..).collect()
    }
}
