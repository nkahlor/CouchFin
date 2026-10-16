//! Game controllers, read through SDL3 and turned into one [`Input`] enum.
//!
//! Only controllers: the keyboard arrives through the window, so
//! couchfin-ui handles it (docs/decisions/010-gamepad-input.md). Depends on
//! no other Couchfin crate.

mod config;
mod direction;
mod event;
pub mod fake;
mod input;
mod mapper;
mod runner;
mod source;

pub use config::{ConfigError, Deadzone, MapperConfig, Repeat};
pub use event::{Axis, Button, ButtonState, PadEvent, PadId};
pub use input::Input;
pub use mapper::Mapper;
pub use runner::{Gamepads, StartError};
pub use source::{Clock, EventSource, SystemClock, Wakeup};
