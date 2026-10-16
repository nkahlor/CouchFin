//! `Navigator` in front of a real window: it drops the keyboard's copy of a
//! controller press, never touches keys that aren't navigation, and keeps
//! the keyboard alone as it was (decision 013).

mod common;

use std::sync::Arc;

use common::{press, shown, type_text};
use couchfin_ui::{AddressView, GuardConfig, Intent, Navigator, Screen, SystemClock};
use slint::ComponentHandle;
use slint::platform::Key;

#[test]
fn text_typed_right_after_a_controller_press_is_never_dropped() {
    let window = shown(Screen::Address);
    let navigator = Navigator::install(&window, GuardConfig::default(), Arc::new(SystemClock));
    navigator.navigate(&window, Intent::Right);
    type_text(&window, "jellyfin.lan");
    assert_eq!(window.global::<AddressView>().get_address(), "jellyfin.lan");
}

#[test]
fn the_keyboard_copy_of_a_controller_cursor_move_is_dropped() {
    let window = shown(Screen::Address);
    let navigator = Navigator::install(&window, GuardConfig::default(), Arc::new(SystemClock));
    type_text(&window, "ab");
    // The cursor is at the end. Left from the pad, then Steam's Left key,
    // then a typed "X": one step left, so "X" lands between "a" and "b".
    navigator.navigate(&window, Intent::Left);
    press(&window, Key::LeftArrow);
    press(&window, 'X');
    assert_eq!(window.global::<AddressView>().get_address(), "aXb");
}

#[test]
fn the_keyboard_alone_moves_the_cursor_every_time() {
    let window = shown(Screen::Address);
    let _navigator = Navigator::install(&window, GuardConfig::default(), Arc::new(SystemClock));
    type_text(&window, "ab");
    press(&window, Key::LeftArrow);
    press(&window, Key::LeftArrow);
    press(&window, 'X');
    assert_eq!(window.global::<AddressView>().get_address(), "Xab");
}
