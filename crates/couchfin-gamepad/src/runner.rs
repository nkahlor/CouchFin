//! The gamepad thread: waits on an [`EventSource`], maps what arrives and
//! hands each [`Input`] to a callback.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::config::MapperConfig;
use crate::input::Input;
use crate::mapper::Mapper;
use crate::source::{Clock, EventSource, Wakeup};

/// Why the gamepad thread couldn't start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// SDL couldn't start its gamepad support, for example because the
    /// library is missing or another part of the program is already using
    /// SDL.
    #[error("couldn't start SDL's gamepad support: {0}")]
    Sdl(String),
    /// The operating system wouldn't start the thread.
    #[error("couldn't start the gamepad thread")]
    Thread(#[source] std::io::Error),
    /// The thread stopped before its event source was open.
    #[error("the gamepad thread stopped while starting")]
    Stopped,
}

/// The running gamepad thread. Dropping it (or calling
/// [`stop`](Self::stop)) stops the thread and waits for it to finish.
///
/// The thread sleeps in [`EventSource::wait`] until an event arrives or the
/// next repeat is due, takes every pending event in one go, maps them with
/// a [`Mapper`] and calls `on_input` for each [`Input`], in order.
///
/// # Examples
///
/// ```
/// use std::sync::mpsc;
/// use couchfin_gamepad::fake::FakePads;
/// use couchfin_gamepad::{Button, Gamepads, Input, MapperConfig, PadId};
///
/// let pads = FakePads::new();
/// let source = pads.source();
/// let (send, inputs) = mpsc::channel();
/// let gamepads = Gamepads::start(
///     MapperConfig::default(),
///     pads.clock(),
///     move || Ok(source),
///     move |input| send.send(input).expect("the test is listening"),
/// )?;
///
/// pads.press(PadId(0), Button::South);
/// assert_eq!(inputs.recv(), Ok(Input::Select));
/// gamepads.stop();
/// # Ok::<(), couchfin_gamepad::StartError>(())
/// ```
pub struct Gamepads {
    stop: Arc<AtomicBool>,
    wakeup: Box<dyn Wakeup>,
    thread: Option<JoinHandle<()>>,
}

impl Gamepads {
    /// Starts the gamepad thread, opens the event source on it with `open`,
    /// and returns once the source is open. Opening happens on the new
    /// thread because SDL expects the thread that starts it to be the one
    /// that reads its events.
    ///
    /// `on_input` runs on the gamepad thread, so it should hand the input
    /// on (to the UI thread, say) rather than do slow work.
    ///
    /// # Errors
    ///
    /// What `open` returned, [`StartError::Thread`] if the thread couldn't
    /// be created, or [`StartError::Stopped`] if `open` panicked.
    pub fn start<S, C>(
        config: MapperConfig,
        clock: C,
        open: impl FnOnce() -> Result<S, StartError> + Send + 'static,
        mut on_input: impl FnMut(Input) + Send + 'static,
    ) -> Result<Self, StartError>
    where
        S: EventSource + 'static,
        C: Clock,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let (opened, ready) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("couchfin-gamepad".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || {
                    let mut source = match open() {
                        Ok(source) => source,
                        Err(error) => {
                            let _ = opened.send(Err(error));
                            return;
                        }
                    };
                    let wakeup: Box<dyn Wakeup> = Box::new(source.wakeup());
                    if opened.send(Ok(wakeup)).is_err() {
                        return;
                    }
                    run(
                        &mut source,
                        &clock,
                        Mapper::new(config),
                        &stop,
                        &mut on_input,
                    );
                }
            })
            .map_err(StartError::Thread)?;

        match ready.recv() {
            Ok(Ok(wakeup)) => Ok(Self {
                stop,
                wakeup,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(mpsc::RecvError) => {
                let _ = thread.join();
                Err(StartError::Stopped)
            }
        }
    }

    /// Stops the thread and waits for it to finish. Same as dropping.
    pub fn stop(self) {}
}

impl Drop for Gamepads {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.wakeup.wake();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("the gamepad thread panicked");
        }
    }
}

impl fmt::Debug for Gamepads {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gamepads")
            .field("stopping", &self.stop.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// The thread's loop. A stop is noticed after every wait, and the wake that
/// goes with it makes sure the wait ends.
fn run<S: EventSource>(
    source: &mut S,
    clock: &impl Clock,
    mut mapper: Mapper,
    stop: &AtomicBool,
    on_input: &mut impl FnMut(Input),
) {
    let mut events = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        let timeout = mapper
            .next_deadline()
            .map(|deadline| deadline.saturating_duration_since(clock.now()));
        events.clear();
        source.wait(timeout, &mut events);
        if stop.load(Ordering::SeqCst) {
            break;
        }
        for input in mapper.update(&events, clock.now()) {
            on_input(input);
        }
    }
}
