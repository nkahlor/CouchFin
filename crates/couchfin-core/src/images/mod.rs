//! Loading item images: fetched once, decoded off the UI thread, kept on
//! disk and in memory,.
//!
//! The design is recorded in `docs/decisions/016-image-cache.md`.

mod cache;
mod cache_dir;
mod complete;
mod config;
mod disk;
mod error;
mod flights;
mod job;
mod key;
mod memory;
mod pixels;

pub use cache::{ImageCache, ImageLoad};
pub use cache_dir::CacheDir;
pub use config::ImageCacheConfig;
pub use error::ImageError;
pub use pixels::Pixels;
