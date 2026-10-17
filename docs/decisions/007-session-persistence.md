# 007: Session persistence and the login flow

Status: accepted · 2026-10-16

## Context

Phase 1 task #12 asks `couchfin-core` to persist the server URL and the
access token (the token in the system keyring, falling back to a file only the
user can read) and to skip login when the stored token still works. The login
screens (#13) are built in `couchfin-ui` as pure views and bind to this crate.
`CLAUDE.md` asks for business logic in core, one-way data flow (screens render
core's state and send actions back), enums instead of booleans, and fakes for
everything external.

The UI runs Slint's event loop on the main thread. Network calls run on a
tokio runtime. `JellyfinApi` futures borrow the client and their arguments
(decision 006).

## Decision

### The login flow is an async controller with plain-data state

- **`LoginFlow<A: JellyfinApi, S: SettingsStore, T: TokenStore>`** owns the
  API, the stores and the current **`LoginScreen`**: `Starting`,
  `Restoring { server, status: Checking | Failed(ApiError) }`,
  `EnterAddress { address, request }`,
  `LogIn { server, info, username, request }` and
  `Home { server, info, user, token_not_saved }`. `request` is
  `Request::{Idle, InFlight, Failed(LoginError)}`. Every field is public, so
  UI tests can build any screen.
- **The UI sends `LoginAction`s**: `SubmitAddress(String)`,
  `SubmitLogin { username, password }`, `Retry`, `ChangeServer`, `LogOut` and
  `SessionExpired`.
  An action that doesn't fit the current screen is ignored, including a second
  submit while one is in flight.
- **Methods take `&self` and return `Send` futures** (`start()`,
  `dispatch(action)`). The app keeps the flow in an `Arc` and spawns each call
  on tokio, so the UI thread never waits. The state sits behind a
  `std::sync::Mutex` that is never held across an `.await`.
- **Changes are pushed through one callback**, `on_change(|screen| ..)`,
  called with every new screen, in order, while the flow is locked. Slint
  binds it with `slint::invoke_from_event_loop`. `screen()` reads the current
  screen at any time. Calling the observer under the lock keeps the order of
  updates right when two tasks finish close together. The cost is that the
  observer must not call back into the flow, as its doc says.
- **Stale results are dropped.** Each action that starts a screen bumps a
  generation counter, and a request applies its result (and its side effects:
  saving settings, deleting tokens) only if the counter hasn't moved. So
  `ChangeServer` during a check really abandons the check.

We chose this over a pure state machine that returns effects for an executor
to run. With effects, the executor (in the app or UI) would have to know
which API call each effect means and feed the results back as events, which
pushes flow knowledge out of core. With async methods, #13 needs one spawn per
action and one callback.

### The flow

- **Startup** loads settings. With no server it asks for an address. With a
  server but no token it checks that server and shows its login form. With
  both it shows `Restoring`, calls `check_server` (for the name and version,
  and to notice a move), then `current_user`:
  - 200: Home.
  - `TokenRejected`: delete the token, and show the login form with
    `Failed(TokenRejected)` so the UI can say why.
  - Any other error from either call: `Restoring` with `Failed(error)`. The
    token is kept, and the user can `Retry` or `ChangeServer`.
  - The check returns a different canonical address (the server now
    redirects): the token is deleted, never sent to the new address, and the
    login form for the new address shows `Failed(Redirected)`.
- **Submitting an address** parses it (`ServerUrlError` becomes
  `LoginError::InvalidAddress`, with no request sent), then calls
  `check_server`. The canonical `ServerCheck::server` is stored and used from
  then on.
- **Logging in** calls `authenticate_by_name` on the canonical address,
  stores the token and the username, then calls `/Users/Me` for Home. If the
  token can't be stored anywhere, Home still opens and carries the
  `TokenStoreError` in `token_not_saved`, so the UI can warn that the next
  start will ask again.
- **`ApiError::Redirected`** from login or `/Users/Me` means the address
  changed, so the flow runs `check_server` again and shows the login form
  there with `Failed(Redirected)`. If that check fails, it goes back to the
  address form.
- **Moving to another server** (a successful check of a different address)
  deletes the previous server's token.
- **`LogOut`** deletes the token and shows the login form with the username
  filled in. It doesn't revoke the token on the server yet (follow-up).
- **`SessionExpired`** (on Home) is for a token the server rejects while
  browsing (decision 017). It does what a rejected token at startup does:
  deletes the token and shows the login form with
  `Failed(Api(TokenRejected))`.
- **`LoginFlow::session()`** returns the `Session` while on Home, else
  `None`, so the app can build a `LibraryFlow`. The token is not part of
  `LoginScreen`, which the UI renders.
- **Errors are data.** `LoginError` wraps `ServerUrlError` and `ApiError`;
  their `Display` is for logs. The UI picks the wording by matching. Core
  re-exports the API types the UI needs (`ServerUrl`, `ServerUrlError`,
  `ApiError`, `Password`, `PublicSystemInfo`, `User`, `UserId`, `DeviceId`,
  `AccessToken`, `JellyfinApi`) and `couchfin_api::fake::FakeJellyfin` as
  `couchfin_core::fake::FakeJellyfin`, so `couchfin-ui` needn't depend on
  `couchfin-api`.

### Persistence

- **Settings** (`device_id`, `server`, `username`) are TOML in
  `$XDG_CONFIG_HOME/couchfin/settings.toml` (`ConfigDir::system()`, via the
  `directories` crate). `Settings::load_or_create` creates the `DeviceId` on
  the first run and saves it, so the server's Devices list keeps one entry. It
  never fails: an unparseable file is replaced with fresh settings, and an
  unreadable one is left alone while fresh settings are used for that run.
  Both are logged. Writes go to a temporary file that is then renamed.
- **Tokens** are stored per server, keyed by the canonical address, so a token
  is only ever sent back to the server that issued it. `TokenStore` has
  `load`, `save` and `delete`.
- **Keyring: `keyring-core` plus `zbus-secret-service-keyring-store`, not the
  `keyring` crate.** `keyring` 4 is now a thin wrapper. Its `v1` feature
  installs the zbus Secret Service store as a process-wide default the first
  time an entry is made, and its own docs tell applications that want control
  over the store to use `keyring-core` and the store crate directly. Doing
  that lets us pass the store in (`KeyringTokens::new(Arc<CredentialStore>)`),
  so tests use keyring-core's mock store and never the real keyring, and it
  avoids global state, for the same reason as decision 006's TLS choice. The
  store is the same one `keyring` uses on Linux: zbus (pure Rust) with the
  `crypto-rust` feature, so neither libdbus nor OpenSSL is linked, and it
  builds here and in CI.
- **Entries** use service `Couchfin` and the server's canonical address as
  the user name.
- **When the fallback is used.** `KeyringTokens` maps keyring-core errors to
  `TokenStoreError`: `NoEntry` means no token, an entry that isn't text is
  `Invalid`, and everything else is `Unavailable`. That covers no D-Bus
  session bus, no Secret Service, a locked or inaccessible collection,
  platform failures and timeouts. `FallbackTokens` falls back to the file
  only on `Unavailable`:
  - `save` writes to the keyring and then deletes any copy in the file, or
    writes to the file if the keyring is unavailable;
  - `load` reads the keyring, then the file if the keyring had nothing or was
    unavailable;
  - `delete` clears both, and reports a keyring error after clearing the
    file.

  `SystemTokens::system(dir)` is the app's store. If connecting to the Secret
  Service fails, it uses `KeyringTokens::unavailable`, which always answers
  `Unavailable`, so everything goes to the file.
- **Timeouts.** Every keyring call runs on its own thread and gives up after
  3 s (`KeyringTokens::DEFAULT_TIMEOUT`). A locked KWallet or GNOME Keyring
  can wait forever on an unlock prompt that nobody sees under gamescope.
  After one timeout the store reports `Unavailable` straight away for the
  rest of the run. A thread that never finishes is left behind.
- **The token file** is `tokens.toml` (a table of server address to token)
  in the config directory. It is created with mode 0600: the mode is set
  when the temporary file is created (`create_new` plus `mode`), and the
  temporary file is then renamed over the old one. Its permissions are never
  set after writing. A missing config directory is created with mode 0700. An
  existing token file that group or others can access is tightened to 0600
  when it is next read, with a warning in the log. Parse errors don't quote
  the file, since that could print a token.
- **Store calls block.** The stores are synchronous and are called from the
  flow's futures. A file write or a D-Bus call with a timeout is short, and it
  keeps core free of a tokio dependency. If that ever shows up on a tokio
  worker, the app can run the flow on a runtime with spare workers, or core
  can wrap the calls in `spawn_blocking`.
- **Never logged:** tokens and passwords. `TokenStoreError` details come
  from I/O, D-Bus and parse errors, none of which carry the token.
  `KeyringTokens`'s `Debug` shows only the store vendor.

### Fakes

`couchfin_core::fake` holds `MemorySettings` and `MemoryTokens` (clones share
state, and `fail_with` scripts errors), plus the re-exported `FakeJellyfin`.
Like the API's fake, they are always compiled.

## Consequences

- **Licences.** `directories` depends on `dirs-sys`, which depends on
  `option-ext` (MPL-2.0). MPL-2.0 is copyleft per file: only changes to
  option-ext's own files would have to be shared, its source is on crates.io,
  and we use it unmodified. `deny.toml` allows MPL-2.0 for `option-ext` only,
  as it does for Slint's licence. The keyring and zbus crates are MIT or
  Apache-2.0.
- `keyring-core` appears in the public API (`KeyringTokens::new` takes an
  `Arc<keyring_core::CredentialStore>`). That lets the app pick another store
  later, such as the kernel keyutils store.
- The real Secret Service is never exercised by automated tests. CI has no
  D-Bus. The TV check covers it, along with the file fallback under gamescope.
- A token saved to the file while the keyring was down stays there until the
  next login or logout, even once the keyring works again. Moving it into the
  keyring on load is a possible follow-up.
- `LogOut` forgets the token locally only. Revoking it on the server
  (`POST /Sessions/Logout`) needs a new `couchfin-api` method.
- Adding more screens later (libraries, player) means a navigation stack
  above `LoginFlow`. Home here is only the login flow's end state.
