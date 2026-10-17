//! Renders `MainWindow` with Slint's software renderer into a buffer, with a
//! clock the example moves by hand, and saves PNGs. No GPU or display.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use couchfin_ui::{MainWindow, Screen};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Key, Platform, WindowAdapter, WindowEvent, update_timers_and_animations};
use slint::{ComponentHandle, PhysicalSize, PlatformError, Rgb8Pixel};

thread_local! {
    static CLOCK: Cell<Duration> = const { Cell::new(Duration::ZERO) };
    static WINDOW: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
}

/// A platform whose windows draw with the software renderer and whose clock
/// stands still until [`Session::settle`] moves it, so animations end in the
/// same place every run.
struct Offscreen;

impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        WINDOW.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&window)));
        Ok(window)
    }

    fn duration_since_start(&self) -> Duration {
        CLOCK.with(Cell::get)
    }
}

/// The two screens Couchfin is checked on.
pub const SIZES: [(u32, u32); 2] = [(1920, 1080), (1280, 720)];

/// Where the PNGs go, and how many have been written.
pub struct Studio {
    out: PathBuf,
    count: usize,
}

impl Studio {
    /// Installs the offscreen platform and creates `out`.
    pub fn new(out: PathBuf) -> Result<Self> {
        slint::platform::set_platform(Box::new(Offscreen))
            .map_err(|e| anyhow::anyhow!("platform already set: {e}"))?;
        std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
        Ok(Self { out, count: 0 })
    }

    /// The output directory.
    pub fn dir(&self) -> &std::path::Path {
        &self.out
    }

    /// How many PNGs were written.
    pub fn count(&self) -> usize {
        self.count
    }

    /// A fresh window on `screen` at the given size. Fill its views, then
    /// call [`Session::start`].
    pub fn open(&mut self, screen: Screen, (width, height): (u32, u32)) -> Result<Session<'_>> {
        let window = MainWindow::new()?;
        window.set_screen(screen);
        Ok(Session {
            studio: self,
            window,
            width,
            height,
        })
    }
}

/// One window being photographed from several states.
pub struct Session<'a> {
    studio: &'a mut Studio,
    /// The window, for setting the views before [`Session::start`].
    pub window: MainWindow,
    width: u32,
    height: u32,
}

impl Session<'_> {
    /// Shows the window at its size, active, as on a TV.
    pub fn start(&self) -> Result<()> {
        self.window.show()?;
        self.software()?
            .set_size(PhysicalSize::new(self.width, self.height));
        self.window
            .window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        self.settle();
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn software(&self) -> Result<Rc<MinimalSoftwareWindow>> {
        WINDOW
            .with(|slot| slot.borrow().clone())
            .context("the window was not created")
    }

    /// Lets every animation finish, as the event loop would over a second.
    #[allow(clippy::unused_self)] // A method, so callers hold a session.
    pub fn settle(&self) {
        for _ in 0..10 {
            CLOCK.with(|clock| clock.set(clock.get() + Duration::from_millis(100)));
            update_timers_and_animations();
        }
    }

    /// Presses and releases each key, then lets animations finish.
    pub fn press(&self, key: Key, times: usize) {
        for _ in 0..times {
            let text: slint::SharedString = key.into();
            let window = self.window.window();
            window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window.dispatch_event(WindowEvent::KeyReleased { text });
            self.settle();
        }
    }

    /// Writes `<screen>-<height>-<state>.png`.
    pub fn snap(&mut self, screen: &str, state: &str) -> Result<()> {
        let software = self.software()?;
        let (width, height) = (self.width as usize, self.height as usize);
        let mut buffer = vec![Rgb8Pixel::default(); width * height];
        self.window.window().request_redraw();
        software.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, width);
        });
        let bytes: Vec<u8> = buffer.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        let image = image::RgbImage::from_raw(self.width, self.height, bytes)
            .context("the buffer is the window's size")?;
        let path = self
            .studio
            .out
            .join(format!("{screen}-{}-{state}.png", self.height));
        image
            .save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        self.studio.count += 1;
        Ok(())
    }
}
