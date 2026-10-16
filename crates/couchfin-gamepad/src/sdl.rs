//! The real event source: SDL3's gamepad subsystem, and nothing else of
//! SDL (no video, no window).
//!
//! SDL treats the thread that starts its event subsystem as the one that
//! must pump events (`SDL_EventsThreadID` in SDL's `SDL_InitSubSystem`; the
//! video thread only takes over when video is started, which it never is
//! here). So [`SdlEventSource::open`] runs on the gamepad thread and every
//! SDL object stays there. The one thing that crosses threads is the
//! wakeup, which uses `SDL_PushEvent`, documented as thread-safe.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use sdl3::event::{Event, EventSender};
use sdl3::gamepad::{self as sdl_pad, Gamepad};
use sdl3::joystick::JoystickId;
use sdl3::{EventPump, EventSubsystem, GamepadSubsystem, Sdl};

use crate::config::MapperConfig;
use crate::event::{Axis, Button, ButtonState, PadEvent, PadId};
use crate::hints::SdlHints;
use crate::input::Input;
use crate::runner::{Gamepads, StartError};
use crate::source::{EventSource, SystemClock, Wakeup};

impl Gamepads {
    /// Starts the gamepad thread reading every connected controller through
    /// SDL, on the system clock. See [`SdlEventSource`].
    ///
    /// ```no_run
    /// use couchfin_gamepad::{Gamepads, MapperConfig, SdlHints};
    ///
    /// let gamepads = Gamepads::start_sdl(MapperConfig::default(), SdlHints::default(), |input| {
    ///     println!("{input:?}");
    /// })?;
    /// // ... run the app; dropping `gamepads` stops the thread and SDL.
    /// # Ok::<(), couchfin_gamepad::StartError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`StartError::Sdl`] if SDL's gamepad support won't start, or SDL is
    /// already running in this process. [`StartError::Thread`] if the
    /// thread can't be created.
    pub fn start_sdl(
        config: MapperConfig,
        hints: SdlHints,
        on_input: impl FnMut(Input) + Send + 'static,
    ) -> Result<Self, StartError> {
        Self::start(
            config,
            SystemClock,
            move || SdlEventSource::open(&hints),
            on_input,
        )
    }
}

/// Controllers read through SDL3's gamepad API.
///
/// - SDL applies each controller's mapping, so buttons arrive by position
///   (south, east, ...) on every pad it knows, including the Steam Deck,
///   Steam Controllers and Steam Input's virtual pad.
/// - Every controller SDL reports is opened. Steam's environment variables
///   (`SDL_GAMECONTROLLER_IGNORE_DEVICES` and friends) reach SDL untouched,
///   so under Steam Input only the virtual pad is seen, not the physical
///   pad behind it as well.
/// - Connects and disconnects are logged at info level with the pad's
///   name and type; nothing is logged per button or axis.
///
/// It isn't `Send`: create it on the thread that will read it, which
/// [`Gamepads::start_sdl`] does. Only one can exist per process at a time.
pub struct SdlEventSource {
    // Fields drop in order: pads close before the subsystems shut down.
    pads: HashMap<JoystickId, Gamepad>,
    pump: EventPump,
    events: EventSubsystem,
    gamepads: GamepadSubsystem,
    _sdl: Sdl,
}

impl SdlEventSource {
    /// Sets `hints`, then starts SDL with only its gamepad subsystem (which
    /// brings in joysticks and events). Controllers already connected
    /// arrive as [`PadEvent::Added`] on the first wait.
    ///
    /// # Errors
    ///
    /// [`StartError::Sdl`] with SDL's message if it won't start.
    pub fn open(hints: &SdlHints) -> Result<Self, StartError> {
        for (name, value) in hints.iter() {
            if !sdl3::hint::set(name, value) {
                // SDL lets an environment variable win over a hint.
                tracing::debug!(name, "an environment variable overrides this SDL hint");
            }
        }
        let sdl = sdl3::init().map_err(sdl_error)?;
        let gamepads = sdl.gamepad().map_err(sdl_error)?;
        let events = sdl.event().map_err(sdl_error)?;
        let pump = sdl.event_pump().map_err(sdl_error)?;
        let version = sdl3::version::version();
        tracing::debug!(%version, "SDL gamepad support started");
        Ok(Self {
            pads: HashMap::new(),
            pump,
            events,
            gamepads,
            _sdl: sdl,
        })
    }

    /// Takes the next pending event without blocking.
    fn poll(&mut self) -> Option<Event> {
        // The sdl3 crate unwraps an SDL button or axis number it doesn't
        // know. A newer system SDL could send one; skip that event rather
        // than lose the thread. The event has left SDL's queue by then.
        loop {
            if let Ok(event) = panic::catch_unwind(AssertUnwindSafe(|| self.pump.poll_event())) {
                return event;
            }
            tracing::warn!("skipped an SDL event the sdl3 crate couldn't read");
        }
    }

    fn handle(&mut self, event: &Event, events: &mut Vec<PadEvent>) {
        match *event {
            Event::GamepadAdded { which, .. } => self.added(which, events),
            Event::GamepadRemoved { which, .. } => {
                let name = self.pads.remove(&which).and_then(|pad| pad.name());
                tracing::info!(pad = which.raw(), name, "controller disconnected");
                events.push(PadEvent::Removed(pad_id(which)));
            }
            Event::GamepadButtonDown { which, button, .. } => {
                events.push(button_event(which, button, ButtonState::Down));
            }
            Event::GamepadButtonUp { which, button, .. } => {
                events.push(button_event(which, button, ButtonState::Up));
            }
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => events.push(PadEvent::Axis {
                pad: pad_id(which),
                axis: axis_of(axis),
                value,
            }),
            // Quit is our own wakeup (signal handlers are off by default).
            // Raw joystick, sensor and touchpad events aren't needed.
            _ => {}
        }
    }

    fn added(&mut self, which: JoystickId, events: &mut Vec<PadEvent>) {
        match self.gamepads.open(which) {
            Ok(pad) => {
                tracing::info!(
                    pad = which.raw(),
                    name = pad.name(),
                    kind = pad.r#type().string(),
                    real_kind = pad.real_type().string(),
                    vendor = pad.vendor_id().map(|id| format!("{id:04x}")),
                    product = pad.product_id().map(|id| format!("{id:04x}")),
                    "controller connected",
                );
                self.pads.insert(which, pad);
                events.push(PadEvent::Added(pad_id(which)));
            }
            Err(error) => {
                tracing::warn!(pad = which.raw(), %error, "couldn't open a controller");
            }
        }
    }
}

impl EventSource for SdlEventSource {
    type Wakeup = SdlWakeup;

    fn wakeup(&self) -> SdlWakeup {
        SdlWakeup {
            sender: self.events.event_sender(),
        }
    }

    /// Waits in `SDL_WaitEventTimeout`, then drains the queue with
    /// `SDL_PollEvent`. With no SDL window, SDL waits by checking the
    /// controllers about once a millisecond, sleeping in between; that is
    /// how SDL reads controllers on Linux with or without a window, so this
    /// thread never spins and adds no latency of its own.
    fn wait(&mut self, timeout: Option<Duration>, events: &mut Vec<PadEvent>) {
        let first = match timeout {
            None => Some(self.pump.wait_event()),
            Some(timeout) => self.pump.wait_event_timeout_ms(whole_millis(timeout)),
        };
        let mut next = first;
        while let Some(event) = next {
            self.handle(&event, events);
            next = self.poll();
        }
    }
}

/// Wakes an [`SdlEventSource`] by pushing a quit event, which it ignores.
pub struct SdlWakeup {
    sender: EventSender,
}

impl Wakeup for SdlWakeup {
    fn wake(&self) {
        // Fails only once SDL has shut down, when nothing is waiting.
        let _ = self.sender.push_event(Event::Quit { timestamp: 0 });
    }
}

/// Rounds up, so a wait never ends just before a deadline and spins.
fn whole_millis(timeout: Duration) -> u32 {
    let millis = timeout.as_nanos().div_ceil(1_000_000);
    // SDL takes a signed 32-bit count; anything longer is "a long time".
    u32::try_from(millis).map_or(i32::MAX.cast_unsigned(), |millis| {
        millis.min(i32::MAX.cast_unsigned())
    })
}

fn sdl_error(error: impl std::fmt::Display) -> StartError {
    StartError::Sdl(error.to_string())
}

fn pad_id(which: JoystickId) -> PadId {
    PadId(which.raw())
}

fn button_event(which: JoystickId, button: sdl_pad::Button, state: ButtonState) -> PadEvent {
    PadEvent::Button {
        pad: pad_id(which),
        button: button_of(button),
        state,
    }
}

fn button_of(button: sdl_pad::Button) -> Button {
    use sdl_pad::Button as Sdl;
    match button {
        Sdl::South => Button::South,
        Sdl::East => Button::East,
        Sdl::West => Button::West,
        Sdl::North => Button::North,
        Sdl::Back => Button::View,
        Sdl::Guide => Button::Guide,
        Sdl::Start => Button::Start,
        Sdl::LeftStick => Button::LeftStick,
        Sdl::RightStick => Button::RightStick,
        Sdl::LeftShoulder => Button::LeftShoulder,
        Sdl::RightShoulder => Button::RightShoulder,
        Sdl::DPadUp => Button::DPadUp,
        Sdl::DPadDown => Button::DPadDown,
        Sdl::DPadLeft => Button::DPadLeft,
        Sdl::DPadRight => Button::DPadRight,
        _ => Button::Other,
    }
}

fn axis_of(axis: sdl_pad::Axis) -> Axis {
    use sdl_pad::Axis as Sdl;
    match axis {
        Sdl::LeftX => Axis::LeftX,
        Sdl::LeftY => Axis::LeftY,
        Sdl::RightX => Axis::RightX,
        Sdl::RightY => Axis::RightY,
        Sdl::TriggerLeft => Axis::LeftTrigger,
        Sdl::TriggerRight => Axis::RightTrigger,
    }
}
