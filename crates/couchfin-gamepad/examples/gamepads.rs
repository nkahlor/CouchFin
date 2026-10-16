//! Prints every input from every connected controller, to check a pad
//! without the app: `cargo run -p couchfin-gamepad --example gamepads`.
//! Connects and disconnects are logged too. Ctrl+C quits.

use std::sync::mpsc;

use couchfin_gamepad::{Gamepads, MapperConfig, SdlHints};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let (send, inputs) = mpsc::channel();
    let _gamepads =
        Gamepads::start_sdl(MapperConfig::default(), SdlHints::default(), move |input| {
            let _ = send.send(input);
        })?;
    println!("Waiting for controllers. Press Ctrl+C to quit.");
    for input in inputs {
        println!("{input:?}");
    }
    Ok(())
}
