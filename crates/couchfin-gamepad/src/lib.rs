//! Game controllers, read through SDL3 and turned into one [`Input`] enum.
//!
//! Only controllers: the keyboard arrives through the window, so
//! couchfin-ui handles it (docs/decisions/010-gamepad-input.md). Depends on
//! no other Couchfin crate.
//!
//! - [`Mapper`] is the whole mapping, pure and synchronous: [`PadEvent`]s
//!   and the time in, [`Input`]s out, including repeats of held directions
//!   ([`Repeat`]) and the left stick's [`Deadzone`].
//! - [`Gamepads`] runs a mapper on its own thread. It waits on an
//!   [`EventSource`], takes every pending event at each wake, sleeps until
//!   the next repeat is due, and hands each input to a callback. Dropping
//!   it stops the thread.
//! - [`SdlEventSource`] is the real source (SDL3's gamepad subsystem, no
//!   SDL window), started with [`Gamepads::start_sdl`] and tuned with
//!   [`SdlHints`]. [`fake`] has scripted pads on a virtual clock for tests.
//!
//! | Pad | Input |
//! |---|---|
//! | D-pad, left stick | [`Input::Up`], [`Input::Down`], [`Input::Left`], [`Input::Right`], repeating while held |
//! | South (A on Xbox and Steam pads) | [`Input::Select`] |
//! | East (B on Xbox and Steam pads) | [`Input::Back`] |
//! | Start | [`Input::Menu`] |
//!
//! Buttons go by position, so on a Nintendo-layout pad the bottom button
//! (labelled B) selects and the right one (A) goes back, as in Steam's own
//! UI. Every connected pad is listened to. The choices are in
//! docs/decisions/012-gamepad-mapping-and-thread.md.
//!
//! # Examples
//!
//! ```
//! use std::sync::mpsc;
//! use std::time::Duration;
//! use couchfin_gamepad::fake::FakePads;
//! use couchfin_gamepad::{Button, Gamepads, Input, MapperConfig, PadId};
//!
//! let pads = FakePads::new();
//! let source = pads.source();
//! let (send, inputs) = mpsc::channel();
//! let _gamepads = Gamepads::start(
//!     MapperConfig::default(),
//!     pads.clock(),
//!     move || Ok(source),
//!     move |input| send.send(input).expect("the example is listening"),
//! )?;
//!
//! // Hold the D-pad down for half a second of virtual time.
//! pads.press(PadId(0), Button::DPadDown);
//! pads.pause(Duration::from_millis(500));
//! pads.release(PadId(0), Button::DPadDown);
//! pads.wait_until_idle();
//! // The press, then repeats at 400 and 460 ms.
//! assert_eq!(inputs.try_iter().collect::<Vec<_>>(), [Input::Down; 3]);
//! # Ok::<(), couchfin_gamepad::StartError>(())
//! ```

mod config;
mod direction;
mod event;
pub mod fake;
mod hints;
mod input;
mod mapper;
mod runner;
mod sdl;
mod source;

pub use config::{ConfigError, Deadzone, MapperConfig, Repeat};
pub use event::{Axis, Button, ButtonState, PadEvent, PadId};
pub use hints::{HintError, SdlHints};
pub use input::Input;
pub use mapper::Mapper;
pub use runner::{Gamepads, StartError};
pub use sdl::{SdlEventSource, SdlWakeup};
pub use source::{Clock, EventSource, SystemClock, Wakeup};
