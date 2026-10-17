//! Couchfin: a controller-first Jellyfin client for Steam Big Picture.

use anyhow::Context as _;
use couchfin_api::JellyfinClient;
use couchfin_core::{ConfigDir, FileSettings, LoginFlow, Settings, SystemTokens};
use couchfin_gamepad::{Gamepads, MapperConfig, SdlHints};
use couchfin_ui::MainWindow;
use slint::ComponentHandle as _;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    // stderr for now; Steam hides it, so a log file is a later task.
    // RUST_LOG overrides the level. Nothing logs tokens or passwords.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    let dir = ConfigDir::system().context("finding the config directory (is $HOME set?)")?;
    let settings_file = FileSettings::new(&dir);
    let settings = Settings::load_or_create(&settings_file);
    let api = JellyfinClient::new(settings.device_id.clone()).context("setting up HTTPS")?;
    let tokens = SystemTokens::system(&dir);
    let flow = LoginFlow::new(api.clone(), settings, settings_file, tokens);

    let window = MainWindow::new().context("creating the main window")?;
    // Couchfin runs on a TV from Steam, so it always covers the whole screen.
    window.window().set_fullscreen(true);
    let _flow = app::connect(&window, flow, api, runtime.handle());
    // Held until the event loop ends; dropping it stops the gamepad thread.
    let _gamepads = app::connect_gamepads(&window, |deliver| {
        Gamepads::start_sdl(MapperConfig::default(), SdlHints::default(), deliver)
    });
    window.run().context("running the event loop")
}
