//! Tunable limits of the image cache.

/// Limits for an [`ImageCache`](super::ImageCache). The defaults suit a
/// couch-sized library; change them with the `with_*` methods.
///
/// # Examples
///
/// ```
/// use couchfin_core::ImageCacheConfig;
///
/// let config = ImageCacheConfig::default().with_max_in_flight(2);
/// assert_eq!(config.max_in_flight, 2);
/// assert_eq!(config.disk_limit_bytes, 500 * 1024 * 1024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageCacheConfig {
    /// The most bytes the cache directory may hold; the least recently used
    /// files go first. Default 500 MB.
    pub disk_limit_bytes: u64,
    /// The most bytes of decoded pixels kept in memory. Default 256 MB.
    pub memory_limit_bytes: usize,
    /// The most image downloads at once. Default 6.
    pub max_in_flight: usize,
    /// Gaussian sigma, in pixels, of the blurred variant. Default 4.0, which
    /// reads as slightly blurred on a poster about 300 pixels wide.
    pub blur_sigma: f32,
}

impl Default for ImageCacheConfig {
    fn default() -> Self {
        Self {
            disk_limit_bytes: 500 * 1024 * 1024,
            memory_limit_bytes: 256 * 1024 * 1024,
            max_in_flight: 6,
            blur_sigma: 4.0,
        }
    }
}

impl ImageCacheConfig {
    /// Sets [`disk_limit_bytes`](Self::disk_limit_bytes).
    #[must_use]
    pub fn with_disk_limit_bytes(mut self, bytes: u64) -> Self {
        self.disk_limit_bytes = bytes;
        self
    }

    /// Sets [`memory_limit_bytes`](Self::memory_limit_bytes).
    #[must_use]
    pub fn with_memory_limit_bytes(mut self, bytes: usize) -> Self {
        self.memory_limit_bytes = bytes;
        self
    }

    /// Sets [`max_in_flight`](Self::max_in_flight); at least 1.
    #[must_use]
    pub fn with_max_in_flight(mut self, count: usize) -> Self {
        self.max_in_flight = count.max(1);
        self
    }

    /// Sets [`blur_sigma`](Self::blur_sigma).
    #[must_use]
    pub fn with_blur_sigma(mut self, sigma: f32) -> Self {
        self.blur_sigma = sigma;
        self
    }
}
