# 006: couchfin-api client design

Status: accepted · 2026-10-16

## Context

Phase 1 needs `couchfin-api` to check a server (`GET /System/Info/Public`),
log in (`POST /Users/AuthenticateByName`) and confirm a stored token
(`GET /Users/Me`), sending the `Authorization: MediaBrowser ...` header with a
stable device ID so the server lists Couchfin under Dashboard > Devices.
`couchfin-core` (#12) and the login screens (#13) build on this API, and
`CLAUDE.md` asks every crate to ship a fake so the others can test without it.

## Decision

- **A stateless client.** `JellyfinClient` holds only the HTTP client and the
  `DeviceId`. Calls before login take a `ServerUrl`; calls after take a
  `Session` (server plus `AccessToken`). Core owns the session, persists it,
  and hands it back after a restart, so there is no hidden login state to
  keep in sync.
- **A trait with `impl Future + Send` methods.** `JellyfinApi` declares
  `fn ... -> impl Future<Output = Result<_, ApiError>> + Send` rather than
  `async fn`, so the futures can be spawned on tokio's multi-threaded runtime.
  The cost is that it isn't dyn-compatible: core takes it as a generic
  (`App<A: JellyfinApi>`). We chose generics over boxed futures (or the
  `async-trait` crate) because there is exactly one real implementation and
  one fake, so dynamic dispatch buys nothing and boxing costs an allocation
  per call.
- **The fake is always compiled** (`couchfin_api::fake::FakeJellyfin`), not
  behind a cargo feature. It is about 250 lines of safe code with no
  dependencies of its own. A feature would have to be turned on by every
  crate that tests with it, and doctests in this crate would need it too.
  The fake answers on one `ServerUrl` (others are `Unreachable`, except
  addresses set up to redirect to it), knows users
  and passwords, issues and revokes tokens, and can be told to fail every
  call with a given `ApiError`. Clones share state, so a test keeps a handle
  while the code under test owns another.
- **Errors by what the user can do.** `ApiError` has `Unreachable` (DNS,
  refused, TLS), `Timeout`, `NotJellyfin` (404 on a Jellyfin endpoint, a web
  page, JSON missing required fields, or a reply cut off mid-body: the server
  was reached, so that isn't `Unreachable`), `InvalidCredentials` (401 on
  login), `TokenRejected` (401 on a logged-in call, so core sends the user
  back to login), `Redirected` and `InsecureRedirect` (see below) and `Status`
  for any other HTTP error. A timeout is `Timeout` whether it happens while
  connecting or while reading the body. It is `Clone + PartialEq` so the fake
  can return scripted errors, and it exposes no reqwest types. Requests time
  out after 10 s by default (`JellyfinClient::DEFAULT_TIMEOUT`).
- **`Display` is log text, not UI copy.** `ApiError`'s messages are for logs
  and diagnostics, and some carry technical detail (an error chain, a URL, a
  status code). The UI (#13) picks its own wording by matching on the
  variant; the `ApiError` rustdoc example shows the shape of that mapping.
- **The header needs no escaping, because nothing unsafe can reach it.**
  Jellyfin's parser
  (`Jellyfin.Server.Implementations/Security/AuthorizationContext.cs`) splits
  on commas outside quotes, strips the quotes and URL-decodes each value, and
  matches keys case-sensitively. The header is
  `MediaBrowser Client="Couchfin", Device="Couchfin", DeviceId="<id>",
  Version="<crate version>"`, plus `, Token="<token>"` once logged in. The
  names and version are constants, and `DeviceId`, `AccessToken`, `UserId`
  and `ServerId` accept only URL-unreserved characters (`A-Z a-z 0-9 . _ ~ -`,
  1 to 256 long), which pass through URL-decoding unchanged and can't close a
  quote. A token from the server that breaks this rule fails the login with
  `NotJellyfin`. The header value is marked sensitive, which keeps it out of
  hyper's debug output.
- **The device ID belongs to the caller.** `DeviceId::generate()` makes a
  UUID v4, and `as_str()`/`parse()` round-trip it. Core stores it once per
  install, because a new ID every launch would add a new Devices entry each
  time.
- **Secrets can't be printed by accident.** `AccessToken` and `Password` have
  a redacting `Debug`, no `Display` and no `Serialize`; the value is read
  through `expose_secret()`. `ServerUrl` rejects `user:password@` URLs, so
  error details, which include the URL, carry no credentials. The crate logs
  nothing itself.
- **`ServerUrl` normalises what people type.** It trims, assumes `http://`
  when there is no scheme (Jellyfin's default), allows only http and https,
  drops a query, a fragment and a pasted web-client path (`/web/...`), keeps a
  reverse-proxy base path, and always ends in `/`.
- **TLS is rustls with the ring provider.** reqwest 0.13's `rustls` feature
  pulls in aws-lc-rs, which builds a large C library (with CMake on some
  setups) and carries the OpenSSL licence. We use `rustls-no-provider` plus
  rustls's `ring` feature.
- **No process-wide TLS state.** With `rustls-no-provider`, reqwest's own TLS
  setup needs rustls's process-wide default crypto provider, so a library
  would have to install one. That is global state the app doesn't expect,
  and if anything else installed a different provider first, ours would
  silently lose. Instead, `JellyfinClientBuilder::build` makes its own
  `rustls::ClientConfig` with ring passed in explicitly
  (`ClientConfig::builder_with_provider`, safe default protocol versions) and
  hands it to reqwest with `tls_backend_preconfigured`. Building a client
  leaves `CryptoProvider::get_default()` unset, and `tests/tls_provider.rs`
  checks that in its own test binary.
- **Certificates are verified as reqwest would.** The config uses
  rustls-platform-verifier (a direct dependency, the version reqwest uses),
  as reqwest does by default: the system's trust store, so a certificate
  the system trusts (such as a home CA added to Bazzite) works, plus any CAs
  from `JellyfinClient::builder(..).add_root_certificates_pem(..)`, for a home
  CA that isn't in the system store. The HTTPS tests use that to trust a
  throwaway CA, and check that an untrusted certificate is `Unreachable`.
  `Verifier::new_with_extra_roots` isn't available on every platform
  rustls-platform-verifier supports (Android, for one), but it is on Linux,
  Couchfin's only target. No OpenSSL is linked.

## Redirects and canonical addresses

Added after the first real-server test (2026-10-16). The server answered
`http://<host>/Users/AuthenticateByName` with `301` to `https://`, as a
reverse proxy forcing HTTPS does. reqwest followed it, which HTTP clients do
by turning the POST into a GET without the body, and on the cross-origin hop
it also dropped the `Authorization` header. Jellyfin saw an anonymous GET and
answered 401, which became "wrong username or password".

- **reqwest never follows redirects** (`Policy::none()`); each call decides.
- **`check_server` follows them by hand** (up to 10), because it is a GET
  with no token or password. Where they end, minus `System/Info/Public`, is
  the server's canonical address, keeping any reverse-proxy base path
  (matched case-insensitively, as Jellyfin is). It returns that as
  `ServerCheck::server` next to the public info, and callers use it for
  every later request: core stores it (#12), and the UI can show
  "Connected to https://…" (#13). Redirects that end somewhere other than
  `System/Info/Public`, loops, and redirects without a `Location` are
  `NotJellyfin`. `check_server` replaced `public_system_info`, so no caller
  can read the info without also getting the canonical address.
- **Login and token calls never follow a redirect.** Following would resend a
  password or token to an address the user didn't choose, and a POST can't
  be safely replayed anyway. A 301, 302, 303, 307 or 308 becomes
  `ApiError::Redirected { server }`, where `server` is the new base address
  when the target is the same endpoint under another base, so the UI can say
  "the server moved to https://…, use that address", or core can run
  `check_server` again. A redirect is never `InvalidCredentials` or
  `TokenRejected`.
- **Never downgrade.** Any redirect from `https` to a non-`https` URL is
  `ApiError::InsecureRedirect`, on every call, because following it would
  later send the token in plaintext.
- `FakeJellyfin::with_redirect_from(old)` makes the fake behave the same way
  for `old`: `check_server` returns the canonical address, and the other
  calls fail with `Redirected`.

## Consequences

- No new licences: ring (Apache-2.0 AND ISC), rustls-webpki and untrusted
  (ISC) and the rest are already on the `deny.toml` allow list. ring brings in
  a third `getrandom` version, a duplicate warning only.
- `couchfin-core` and the UI depend on `JellyfinApi` generically, and test
  with `FakeJellyfin`; only `app` constructs `JellyfinClient`.
- If the device name ever becomes configurable (for example the hostname),
  it can contain spaces or other characters, and the header builder must then
  URL-encode values.
- Later phases add methods to `JellyfinApi` that take a `Session`. Adding a
  method breaks other implementors, which today is only the fake.
- `crates/couchfin-api/examples/probe.rs` checks a real server by hand
  (`check_server`, then login and `/Users/Me` on the canonical address it
  prints), reading the password without echo.
- The redirect tests generate a throwaway CA with `rcgen` (a dev-dependency,
  MIT OR Apache-2.0, using ring) and serve HTTPS from a small rustls server
  in `tests/common/tls.rs`, since wiremock only speaks plain HTTP.

## Library endpoints

Added for decision 014's Home, library grid and detail pages. Paths come from
Jellyfin's OpenAPI spec (checked against 12.2.0, which still serves them):
the 10.9+ forms without `/Users/{id}`.

- **Calls.** Every call takes a `Session` and nothing else about the user.
  `libraries` (`/UserViews`), `items`
  (`/Items`), `item` (`/Items/{id}`), `resume` (`/UserItems/Resume`, video
  only), `next_up` (`/Shows/NextUp`), `latest` (`/Items/Latest`), `seasons`
  and `episodes` (`/Shows/{id}/Seasons`, `/Episodes?seasonId=`) and `image`
  (`/Items/{id}/Images/{Primary|Backdrop|Thumb}?quality=96&tag=` plus `maxWidth=`, or `fillWidth=&fillHeight=` for a box). They go
  through the same `send_once` as `current_user`: redirects are never
  followed, a 401 is `TokenRejected`.
- **No `userId` is sent.** Every one of these endpoints takes `userId` as an
  optional query parameter and passes it through
  `RequestHelpers.GetUserId(User, userId)` (jellyfin 10.11,
  `Jellyfin.Api`), which returns the authenticated user when it is omitted
  (and refuses another user unless the caller is an administrator). That
  covers `UserViewsController.GetUserViews` (`/UserViews`),
  `ItemsController.GetItems` (`/Items`) and `GetResumeItems`
  (`/UserItems/Resume`), `UserLibraryController.GetItem` (`/Items/{id}`) and
  `GetLatestMedia` (`/Items/Latest`), and `TvShowsController.GetNextUp`,
  `GetSeasons` and `GetEpisodes`. The resolved ID is also the one used to
  build the DTOs, so user data (played, position, percentage) belongs to the
  token's user. So `Session` stays server plus token, and core needs no
  `UserId` next to it.
- **Wire shapes.** Lists are `{Items, TotalRecordCount, StartIndex}`, except
  `/Items/Latest`, which is a bare array. `Page::total` falls back to the
  item count if the total is missing. `items` always sends
  `sortBy=SortName`; the grid has no other sort (decision 014).
- **One `Item` type.** Movie, series, season and episode share one struct;
  `ItemKind::Other(String)` keeps any other type. Jellyfin's `IndexNumber` and
  `ParentIndexNumber` become `season_number` and `episode_number` by kind, so
  callers don't need to know that. Ticks become `Duration`. Oddities (negative
  numbers, an image tag with unsafe characters) become `None` rather than
  failing the page, since one bad item in 2,000 must not blank a grid.
  `overview` is only sent by `item` and `episodes` (`fields=Overview`).
- **`ApiError::ItemNotFound`.** On item-scoped calls (`item`, `seasons`,
  `episodes`, `image`) a 404 means the item or image is gone, not that the
  address isn't Jellyfin (the rule for the other endpoints). The variant is
  new, so exhaustive matches on `ApiError` need an arm.
- **Images** return the bytes and `Content-Type` as `Image`; a reply without
  a content type is `NotJellyfin`, since the cache could not decode it.
  `ImageRequest` carries the image tag, so a cache keyed on
  `(item, kind, size, tag)` never serves a stale image. `ItemId` and
  `ImageTag` use the same validation as the other IDs, so they are safe in
  the URL path.
- **Image parameters (#49).** Every request sends `quality=96`, as
  jellyfin-web does (the server's default is 90). `ImageRequest::max_width`
  alone sends `maxWidth`; with `with_fill_height` it sends `fillWidth` and
  `fillHeight` (names checked against Jellyfin's OpenAPI spec: "width/height
  of box to fill"). The app only uses boxes, with a fixed size per card shape
  (decision 016), so the same URLs repeat and the server's resize cache hits.
- **BlurHash (#49).** `Item::blur_hashes` and `Library::primary_blur_hash`
  parse `ImageBlurHashes` (`{ "Primary": { "<tag>": "<hash>" } }`). A
  `BlurHash` is checked for the base 83 alphabet and for the length its own
  header promises; a bad one is dropped, since a placeholder is optional.
  Look one up with `ImageBlurHashes::get(kind, &tag)`: the tag is part of
  the key, so a replaced image never gets the old image's blur. Only
  Primary, Thumb and the first Backdrop are read.
- **The fake** serves the same calls from a catalog built with `with_library`,
  `with_movie`, `with_series`, `with_resume`, `with_next_up` and
  `with_generated_library(name, 2_000)` (movies named `"{name} 00000"` and up,
  all with a 1x1 PNG). It pages like the server: names sorted ignoring case,
  `total` counting all matches, a page past the end empty. It needs an issued
  token. Anything implementing `JellyfinApi` outside this
  crate (core's `Gated` test wrapper) must delegate the new methods.
