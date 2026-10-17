//! Contact sheets: a handful of screenshots shrunk to 960x540 and tiled two
//! across and three down, so one image (about 1920x1620) shows six screens.
//! Looking at an image costs tokens on every later turn, so agents view
//! these, not the full-size files (CLAUDE.md, "Before opening a PR").

use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::{FilterType, overlay, resize};
use image::{Rgb, RgbImage};

const TILE: (u32, u32) = (960, 540);
const GAP: u32 = 8;

/// The sheets: a name, then its tiles in reading order (left to right, top
/// to bottom) as `<screen>-<height>-<state>` file stems.
const SHEETS: [(&str, [&str; 6]); 4] = [
    (
        "sheet-login",
        [
            "address-1080-idle",
            "address-1080-error",
            "login-1080-idle",
            "login-1080-error",
            "connecting-1080-checking",
            "connect-failed-1080-unreachable",
        ],
    ),
    (
        "sheet-home",
        [
            "home-1080-focus-row1-card1",
            "home-1080-focus-row3-card5",
            "home-1080-missing-images-row3-card2",
            "home-1080-focus-signout",
            "home-720-focus-row3-card5",
            "home-720-notice-and-row-error",
        ],
    ),
    (
        "sheet-browse",
        [
            "library-1080-full-top",
            "library-1080-full-scrolled-row12-card-right",
            "library-720-full-scrolled-row4",
            "library-1080-unloaded-pages-scrolled-into-unloaded",
            "home-1080-images-loading-row3-card3",
            "home-1080-notice-and-row-error",
        ],
    ),
    (
        "sheet-detail",
        [
            "movie-1080-long-title-resume",
            "movie-1080-short-title-no-backdrop",
            "movie-720-long-title-resume",
            "series-1080-seasons-and-episodes-start",
            "series-1080-seasons-and-episodes-episodes-scrolled",
            "series-720-seasons-and-episodes-episodes-scrolled",
        ],
    ),
];

/// Writes the sheets next to the screenshots they are made from.
pub fn write(dir: &Path) -> Result<()> {
    for (name, tiles) in SHEETS {
        let width = 2 * TILE.0 + GAP;
        let height = 3 * TILE.1 + 2 * GAP;
        let mut sheet = RgbImage::from_pixel(width, height, Rgb([255, 0, 255]));
        for (n, stem) in tiles.iter().enumerate() {
            let path = dir.join(format!("{stem}.png"));
            let shot = image::open(&path)
                .with_context(|| format!("reading {}", path.display()))?
                .to_rgb8();
            let small = resize(&shot, TILE.0, TILE.1, FilterType::Triangle);
            let n = u32::try_from(n)?;
            let (x, y) = (n % 2 * (TILE.0 + GAP), n / 2 * (TILE.1 + GAP));
            overlay(&mut sheet, &small, i64::from(x), i64::from(y));
        }
        sheet.save(dir.join(format!("{name}.png")))?;
        println!(
            "{name}.png, left to right, top to bottom: {}",
            tiles.join(", ")
        );
    }
    Ok(())
}
