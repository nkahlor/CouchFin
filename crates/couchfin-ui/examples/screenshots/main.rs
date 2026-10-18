//! Renders every Couchfin screen and state to PNG, so whoever builds UI can
//! look at it before a human has to (docs/decisions/018-visual-review.md).
//!
//! ```sh
//! cargo run -p couchfin-ui --example screenshots -- target/screenshots
//! ```
//!
//! Uses Slint's software renderer, so it needs no GPU or display. That
//! renderer draws no shadows or glow, so the focused card's glow, and
//! anything else only the TV's femtovg renderer draws, still needs the TV.
//! Files are named `<screen>-<height>-<state>.png`, at 1920x1080 and 1280x720.

mod art;
mod browse;
mod detail;
mod fixtures;
mod login;
mod sheets;
mod studio;

use std::path::PathBuf;

use anyhow::{Context, Result};
use studio::Studio;

fn main() -> Result<()> {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: screenshots <out-dir>")?;
    let mut studio = Studio::new(out)?;
    for size in studio::SIZES {
        login::shoot(&mut studio, size)?;
        browse::shoot(&mut studio, size)?;
        detail::shoot(&mut studio, size)?;
    }
    println!("wrote {} screenshots", studio.count());
    sheets::write(studio.dir())?;
    Ok(())
}
