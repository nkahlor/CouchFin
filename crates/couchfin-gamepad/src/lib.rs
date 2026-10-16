//! Game controllers, read through SDL3 and turned into one `Input` enum
//! (directions, select, back, menu and later playback controls).
//!
//! Only controllers: the keyboard arrives through the window, so
//! couchfin-ui handles it (docs/decisions/010-gamepad-input.md). Depends on
//! no other Couchfin crate.
