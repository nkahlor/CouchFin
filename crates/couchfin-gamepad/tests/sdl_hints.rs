//! Which SDL hints Couchfin sets, and which it leaves to Steam.

use couchfin_gamepad::{HintError, SdlHints};

fn pairs(hints: &SdlHints) -> Vec<(String, String)> {
    hints
        .iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

#[test]
fn by_default_sdl_leaves_the_signal_handlers_alone() {
    // Otherwise SDL turns Ctrl+C and SIGTERM into a quit event nobody reads.
    assert_eq!(
        pairs(&SdlHints::default()),
        [("SDL_NO_SIGNAL_HANDLERS".to_owned(), "1".to_owned())]
    );
}

#[test]
fn hints_can_be_added_and_replaced() {
    let hints = SdlHints::default()
        .with("SDL_JOYSTICK_HIDAPI_STEAMDECK", "0")
        .and_then(|hints| hints.with("SDL_NO_SIGNAL_HANDLERS", "0"))
        .expect("allowed hints");
    assert_eq!(
        pairs(&hints),
        [
            ("SDL_JOYSTICK_HIDAPI_STEAMDECK".to_owned(), "0".to_owned()),
            ("SDL_NO_SIGNAL_HANDLERS".to_owned(), "0".to_owned()),
        ]
    );
}

#[test]
fn an_empty_set_sets_nothing() {
    assert_eq!(pairs(&SdlHints::none()), []);
}

#[test]
fn the_hints_steam_uses_to_hide_devices_are_refused() {
    for name in [
        "SDL_GAMECONTROLLER_IGNORE_DEVICES",
        "SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT",
        "SDL_GAMECONTROLLERCONFIG",
        "SDL_GAMECONTROLLERCONFIG_FILE",
    ] {
        assert_eq!(
            SdlHints::default().with(name, "0x28de/0x1205"),
            Err(HintError::SteamOwned(name.to_owned())),
            "{name}"
        );
    }
}
