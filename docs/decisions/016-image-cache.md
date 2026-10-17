# 016: Image cache

Status: accepted · 2026-10-16

## Context

Phase 3 shows hundreds of posters. Decoding a JPEG on the UI thread would drop
frames, refetching every poster on each launch would be slow on a home
network, and Slint cannot blur at draw time, so the Home carousel's passed
cards need a pre-blurred copy (decision 011). Issue #17 asks `couchfin-core`
for async loading with a disk cache. `CLAUDE.md` keeps core free of Slint, so
the cache hands out plain pixels.

## Decision

### API

`ImageCache<A: JellyfinApi>::new(api, &CacheDir, ImageCacheConfig)`, then
`load(&session, &ImageRequest, Variant) -> Result<ImageLoad, ImageError>`:

- `Variant::{Sharp, Blurred}`; the blurred copy has the same size.
- `ImageLoad::{Ready(Pixels), NoImage}`. `ApiError::ItemNotFound` (the item
  has no such image) is `NoImage`, a placeholder case, not an error.
- `Pixels` is straight RGBA8 with `width()`, `height()` and `rgba()`; clones
  share the bytes (`Arc<[u8]>`). The UI copies them into a
  `slint::SharedPixelBuffer<Rgba8Pixel>`.
- `ImageError::{Api, Decode, Io, Cache}` (`thiserror`). `Io` comes from
  `clear()`; `Cache` means a blocking task died.
- `CacheDir` is injected like `ConfigDir`: `system()` is
  `$XDG_CACHE_HOME/couchfin/images`.

### Keys and files

The key is `{item}_{kind}_{max_width}_{tag|none}`. A changed image has a new
tag, so it is a new key; old files age out through eviction, with no
invalidation logic. The server is not in the key: item IDs are GUIDs.

Each key has up to two files in one flat directory, written atomically
(`fs_util`, mode 0600):

- `{key}.orig`: the bytes as the server sent them (JPEG, PNG or WebP).
- `{key}.blur{sigma bits}.png`: the blurred copy, lossless. The sigma is in
  the name, so a changed sigma never serves a stale blur.

Blurred pixels loaded from disk equal the freshly blurred ones. A blurred
request with only the original on disk blurs that, so it needs no download.

### Caps and eviction

- Disk: 500 MB by default. After each write the directory is scanned and the
  oldest files by modification time are deleted until it fits, never the file
  just written. A read sets the modification time to now, so it means "last
  used". A memory hit does not touch the file, so a poster that stays in
  memory can age on disk; the next restart simply refetches it.
- Memory: 256 MB of decoded pixels by default, least recently used first
  (a linear scan for the oldest; a few hundred entries). An image larger than
  the whole budget is not kept.
- A file that fails to decode is deleted and fetched again; a failure to
  read or write the cache is logged and treated as a miss. The cache never
  fails a load.
- Bytes from the server that don't decode give `ImageError::Decode` and are
  not stored.

### Concurrency

- **Off the caller's thread:** disk access, decoding, blurring and PNG
  encoding run in `tokio::task::spawn_blocking`. The UI calls `load` from a
  tokio task and posts the result to the event loop.
- **One fetch per key:** a per-key async mutex. The first caller does the
  work inside its own future; later callers wait, then find the result in
  memory. Sharp and blurred requests for one key share the original download.
  (If the first caller fails, each waiter retries by itself. That includes a
  404, which costs one cheap request per waiter.)
- **In-flight limit:** a semaphore (default 6) around the HTTP request only.
- **Cancellation:** the work is not spawned detached, so dropping the `load`
  future drops the request and releases the key for the next caller. A
  blocking task already running finishes (milliseconds) and is discarded.

### Blur

`image::imageops::fast_blur`, a box-blur approximation of a Gaussian, with
sigma 4.0 by default (`ImageCacheConfig::blur_sigma`). On a 300x450 poster
that is soft but still recognisable, and it takes about 11 ms in a release
build (JPEG decode 3 ms, PNG encode 1 ms). Sigma 2 to 6 cost about the same,
so blurring a downscaled copy would not pay for the extra resize and the
quality loss; the blur runs at full size. A debug build is 20 times slower
(250 ms), so tests use 64x96 images.

### Formats and licences

The `image` crate with only `jpeg`, `png` and `webp` (no default features, so
no rayon, GIF or AVIF). The new crates are `image`, `png`, `zune-jpeg`,
`zune-core`, `image-webp`, `moxcms`, `pxfm`, `fdeflate`, `byteorder-lite`,
`quick-error`, `bytemuck`, `crc32fast`, `flate2`, `simd-adler32` and
`miniz_oxide`. All are dual-licensed with at least one of MIT, Apache-2.0,
BSD-3-Clause or Zlib, which are already allowed, so `deny.toml` is unchanged.

## Consequences

- The UI task wires `load` to the carousel and stage B of #18.
- The disk cache can exceed its cap briefly between a write and its eviction.
- Two processes sharing the directory are safe (atomic writes, tolerant
  deletes) but not coordinated.
