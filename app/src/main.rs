//! Couchfin: a controller-first Jellyfin client for Steam Big Picture.

use anyhow::Context as _;
use couchfin_ui::HelloWindow;
use slint::ComponentHandle as _;

fn main() -> anyhow::Result<()> {
    let window = HelloWindow::new().context("creating the main window")?;
    // Couchfin runs on a TV from Steam, so it always covers the whole screen.
    window.window().set_fullscreen(true);
    window.on_quit_requested(|| {
        // This only fails when the event loop has already stopped, in which
        // case the app is quitting anyway.
        let _ = slint::quit_event_loop();
    });
    window.run().context("running the event loop")
}
