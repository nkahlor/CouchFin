slint::include_modules!();

mod api;
mod login;

use std::sync::Arc;

use api::JellyfinClient;

// TODO: replace with a server selection screen.
const SERVER_URL: &str = "https://jellyfin.stablesnake.box.ca";

// The UI runs on the main thread; tokio's worker threads run the network calls.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(JellyfinClient::new(SERVER_URL)?);

    let main_window = MainWindow::new()?;
    login::install(&main_window, client, SERVER_URL);

    main_window.window().set_fullscreen(true);
    main_window.run()?;
    Ok(())
}
