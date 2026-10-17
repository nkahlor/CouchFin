//! The image cache itself.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use couchfin_api::{ApiError, ImageRequest, JellyfinApi, Session};
use tokio::sync::Semaphore;

use super::disk::Disk;
use super::flights::Flights;
use super::job::Job;
use super::key::Key;
use super::memory::Memory;
use super::{CacheDir, ImageCacheConfig, ImageError, Pixels};

/// The result of [`ImageCache::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageLoad {
    /// The decoded pixels.
    Ready(Pixels),
    /// The item has no such image: show a placeholder.
    NoImage,
}

/// Loads item images for the UI.
///
/// - Decoded pixels are cached in memory, bounded in bytes, and the encoded
///   files on disk, bounded in bytes with least-recently-used eviction.
/// - Decoding and file access run on tokio's blocking pool, never
///   on the caller's thread.
/// - Concurrent loads of one image share one fetch, and at most
///   [`ImageCacheConfig::max_in_flight`] fetches run at once.
/// - Dropping a [`load`](Self::load) future cancels its fetch.
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::FakeJellyfin;
/// use couchfin_api::{ImageKind, ImageRequest, LibraryKind, Session};
/// use couchfin_core::{
///     CacheDir, ImageCache, ImageCacheConfig, ImageLoad, JellyfinApi, Password,
/// };
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let url = "http://fake.lan:8096".parse()?;
/// let fake = FakeJellyfin::new(url)
///     .with_user("alice", "hunter2")
///     .with_library("Movies", LibraryKind::Movies)
///     .with_movie("Movies", "Brazil");
/// let url = fake.server().clone();
/// let login = fake.authenticate_by_name(&url, "alice", &Password::new("hunter2")).await?;
/// let session = Session::new(url, login.access_token);
/// let movie = fake.item_named("Brazil").ok_or("no such movie")?;
///
/// let dir = tempfile::tempdir()?;
/// let cache = ImageCache::new(fake, &CacheDir::new(dir.path()), ImageCacheConfig::default());
/// let request = ImageRequest::new(movie.id, ImageKind::Primary, 300);
///
/// let ImageLoad::Ready(pixels) = cache.load(&session, &request).await? else {
///     return Err("the fake has a poster for every movie".into());
/// };
/// assert_eq!(pixels.rgba().len(), (pixels.width() * pixels.height() * 4) as usize);
/// # Ok(())
/// # }
/// ```
pub struct ImageCache<A: JellyfinApi> {
    api: A,
    disk: Arc<Disk>,
    memory: Mutex<Memory>,
    flights: Flights,
    fetches: Semaphore,
}

impl<A: JellyfinApi> ImageCache<A> {
    /// A cache fetching through `api` and storing files in `dir`. Nothing
    /// touches the disk until the first image arrives.
    #[must_use]
    pub fn new(api: A, dir: &CacheDir, config: ImageCacheConfig) -> Self {
        Self {
            api,
            disk: Arc::new(Disk::new(dir.path().to_path_buf(), config.disk_limit_bytes)),
            memory: Mutex::new(Memory::new(config.memory_limit_bytes)),
            flights: Flights::default(),
            fetches: Semaphore::new(config.max_in_flight.max(1)),
        }
    }

    /// The image `request` names, as pixels.
    ///
    /// Looks in memory, then on disk, and only then asks the server. Call it
    /// from a tokio task and hand the pixels to the UI thread; it needs a
    /// multi-threaded runtime or at least one with blocking threads.
    ///
    /// # Errors
    ///
    /// [`ImageError::Api`] if the server could not be asked (a missing image
    /// is [`ImageLoad::NoImage`] instead), [`ImageError::Decode`] if the
    /// server sent something that is not a JPEG, PNG or WebP image, and
    /// [`ImageError::Cache`] if a decoding task died.
    pub async fn load(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> Result<ImageLoad, ImageError> {
        let key = Key::new(request);
        if let Some(pixels) = self.remembered(&key) {
            return Ok(ImageLoad::Ready(pixels));
        }
        let _flight = self.flights.enter(&key).await;
        // Whoever was ahead of us may have loaded it.
        if let Some(pixels) = self.remembered(&key) {
            return Ok(ImageLoad::Ready(pixels));
        }
        match self.produce(session, request, &key).await {
            Ok(pixels) => {
                self.memory().insert(key, pixels.clone());
                Ok(ImageLoad::Ready(pixels))
            }
            Err(ImageError::Api(ApiError::ItemNotFound)) => Ok(ImageLoad::NoImage),
            Err(error) => Err(error),
        }
    }

    /// Forgets every cached image, in memory and on disk (a "clear cache"
    /// button).
    ///
    /// # Errors
    ///
    /// [`ImageError::Io`] if a cache file could not be deleted.
    pub async fn clear(&self) -> Result<(), ImageError> {
        self.memory().clear();
        let disk = Arc::clone(&self.disk);
        blocking(move || disk.clear())
            .await?
            .map_err(|error| ImageError::Io {
                detail: error.to_string(),
            })
    }

    fn memory(&self) -> MutexGuard<'_, Memory> {
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn remembered(&self, key: &Key) -> Option<Pixels> {
        self.memory().get(key)
    }

    async fn produce(
        &self,
        session: &Session,
        request: &ImageRequest,
        key: &Key,
    ) -> Result<Pixels, ImageError> {
        let job = Job::new(Arc::clone(&self.disk), key);
        let cached = job.clone();
        if let Some(pixels) = blocking(move || cached.cached()).await? {
            return Ok(pixels);
        }
        let image = {
            let _permit = self
                .fetches
                .acquire()
                .await
                .map_err(|error| ImageError::Cache {
                    detail: error.to_string(),
                })?;
            self.api.image(session, request).await?
        };
        blocking(move || job.downloaded(&image.bytes)).await?
    }
}

/// Runs `work` on the blocking pool.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ImageError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ImageError::Cache {
            detail: error.to_string(),
        })
}
