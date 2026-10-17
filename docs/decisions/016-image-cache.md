# 016: Image cache

Status: accepted · 2026-10-16 (blurred variant dropped, see the update at the end)

## Context

Phase 3 shows hundreds of posters. Decoding a JPEG on the UI thread would drop
frames, refetching every poster on each launch would be slow on a home
network, so the cache is needed. Issue #17 asks `couchfin-core`
for async loading with a disk cache. `CLAUDE.md` keeps core free of Slint, so
the cache hands out plain pixels.

## Decision

### API

`ImageCache<A: JellyfinApi>::new(api, &CacheDir, ImageCacheConfig)`, then
`load(&session, &ImageRequest) -> Result<ImageLoad, ImageError>`:

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

The key is `{item}_{kind}_{width}x{height}_{tag|none}` (height 0 for a plain `maxWidth` request). A changed image has a new
tag, so it is a new key; old files age out through eviction, with no
invalidation logic. The server is not in the key: item IDs are GUIDs.

Each key has one file in one flat directory, written atomically
(`fs_util`, mode 0600):

- `{key}.orig`: the bytes as the server sent them (JPEG, PNG or WebP).

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
  not stored. That includes partial files, see "Partial files" below.

### Concurrency

- **Off the caller's thread:** disk access, decoding run in `tokio::task::spawn_blocking`. The UI calls `load` from a
  tokio task and posts the result to the event loop.
- **One fetch per key:** a per-key async mutex. The first caller does the
  work inside its own future; later callers wait, then find the result in
  memory.
  (If the first caller fails, each waiter retries by itself. That includes a
  404, which costs one cheap request per waiter.)
- **In-flight limit:** a semaphore (default 12, see the #49 update) around the HTTP request only.
- **Cancellation:** the work is not spawned detached, so dropping the `load`
  future drops the request and releases the key for the next caller. A
  blocking task already running finishes (milliseconds) and is discarded.

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

## Update, 2026-10-16: the blurred variant was dropped

`Variant`, the blur step, `ImageCacheConfig::blur_sigma` and the
`{key}.blur{sigma bits}.png` files are gone (#48), and the cache stores one
image per poster. Most of the UI is grids and rows where the blur does
nothing, and it doubled the decode, disk and memory work for every poster.
Passed carousel cards are dimmed instead (decision 011).

**Upgrade:** old `*.blur*.png` files in users' caches are never read again.
They are not deleted on open; they count towards the disk cap and the
least-recently-used eviction removes them, oldest first, as new posters
arrive (or "clear cache" removes them at once). `tests/image_cache.rs` checks
that a leftover blurred file changes nothing.

## Update, 2026-10-17: partial files are errors (#50)

A poster appeared half grey. zune-jpeg, the `image` crate's JPEG decoder,
fills the blocks missing from a truncated file with grey and reports success,
so a cut-off download decoded "fine", was shown, and was written to the disk
cache (where it would have come back on every launch). PNG and WebP decoders
do report truncation.

- `images/complete.rs` checks a file before decoding: a JPEG must end with its
  end marker (`FF D9`, ignoring zero padding), a WebP must be as long as its
  RIFF header says. Failing files are `ImageError::Decode`.
- A decoded image wider than the request's `max_width` is also a `Decode`
  error: the server scales down, never up, so it is not the image we asked for.
- The same checks run on files read back from disk, so a partial file left by
  an older version is deleted and fetched again.
- A body shorter than its `Content-Length` and a load cancelled mid-download
  were already safe: hyper reports the first as a body error, and dropping the
  `load` future drops the request before anything is written. Tests now pin
  both.
- Nothing partial is cached, so the next request retries. Causes are logged
  at debug level under the target `couchfin::images`
  (`RUST_LOG=couchfin::images=debug`).

## Update, 2026-10-17: near-instant posters (#49)

Posters were slow and some never appeared. jellyfin-web feels instant on the
same server because of how it asks, so Couchfin now does the same.

- **Stable sizes.** `ImageSize::of(shape, scale)`: a fixed logical box per
  shape (poster 280x420, thumb and library tile 448x252, backdrop 1280x720)
  times the display scale snapped up to 1.0, 1.5 or 2.0. It is requested as
  `fillWidth`/`fillHeight` with `quality=96` (decision 006). The old size
  followed the live card width rounded to 32 px: many distinct URLs, each a
  slow server-side resize. Grid and Home posters share one size, so a poster
  seen on Home is already cached when its grid opens. The cache key changed
  (`{w}x{h}`), so old `.orig` files age out through eviction.
- **A fill box can exceed one dimension** (cover), so the "wider than
  requested" check from #50 applies only to plain `maxWidth` requests. The
  completeness check applies to all.
- **Every visible row loads.** Home requests the focused row's cards
  nearest-first, then every other row that starts on screen (computed from the
  window height and the row heights), plus one row below the fold and the row
  above. Per row it keeps the visible cards plus a margin. The grid already
  loaded visible plus a margin.
- **BlurHash placeholders.** Core decodes the hash (`blurhash` crate, MIT, no
  default features) to 24x36 (poster) or 32x18 (thumb) RGBA when it builds the
  `Card`, which happens on the async runtime, never the UI thread. The card
  shows it scaled up from the first frame; the real picture fades in over it
  in `Theme.image-fade-duration` (150 ms). `ImageState` is `loading` (the
  placeholder shows), `ready` or `missing`. A card whose item has no
  picture, or whose load answers `NoImage`, is `missing`, so the card can show
  its title; a failed load stays `loading`.
- **Dev builds optimise dependencies** (`[profile.dev.package."*"]
  opt-level = 2`), so `cargo run` decodes at near release speed (it was about
  20x slower). Cost: dependencies compile slower once, and again only when
  `Cargo.lock` or the profile changes; our own crates stay at opt-level 0.
- **HTTP/2** is on (reqwest `http2`). It is negotiated by ALPN over HTTPS
  only, so a plain `http://` server on the LAN stays HTTP/1.1, where each
  in-flight request uses its own connection.
  reqwest does not add ALPN to a TLS config it is handed (ours, for the
  platform verifier), so `couchfin-api` sets `h2` and `http/1.1` itself;
  `tests/tls_http2.rs` checks a server is offered `h2` first.
- **In-flight limit 12 (was 6).** The first screen is 12 to 16 pictures.
  Six at a time made that two or three round trips; twelve is one, and the
  waiting semaphore is FIFO, so the focused row still goes first. On HTTP/1.1
  twelve connections to a LAN server is modest, and HTTP/2 multiplexes them
  over one.
- **Time budget** (`couchfin-ui/tests/image_budget.rs`): wiremock with 100 ms
  latency per picture, 12 cards in the 2 rows a 1080p window shows. Before:
  first screen complete after about 520 ms (6 in flight, unoptimised
  decoding). After: about 260 ms; placeholders in the cards after under 5 ms.
  Budget: pictures within 350 ms (one 100 ms round trip, decoding and
  connection set-up, with slack for a busy CI machine) and placeholders
  within 50 ms.
