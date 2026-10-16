# 008: Login screens in couchfin-ui and the app wiring

Status: accepted · 2026-10-16

## Context

Phase 1 (#13) needs a server address screen, a username and password
screen, and a home screen showing the server and who is signed in.
`couchfin-core` owns the login flow as state, `LoginFlow` and its
`LoginScreen` ([decision 007](007-session-persistence.md)), so the screens
must be pure views: render what core says, send actions back (one-way data
flow, `CLAUDE.md`). Controller input arrives in Phase 2, but every action
must already work by keyboard, and Steam's on-screen keyboard types into
whatever field has focus.

## Decision

### Views

- **One window, one global per screen.** `MainWindow` has a `screen`
  property (`Connecting`, `ConnectFailed`, `Address`, `Login`, `Home`) and
  shows only that screen. Each screen's data and callbacks live in an
  exported Slint global (`ConnectView`, `AddressView`, `LoginView`,
  `HomeView`), so the properties of all of them don't share one flat
  namespace. They are named `…View` so they don't clash with core's
  `LoginScreen`. Switching screens inside one window means Steam only ever
  sees one fullscreen window.
- **Only the current screen exists.** Each screen is an `if` in
  `MainWindow`, so switching away destroys it, along with anything typed
  into it, and the new screen focuses its first control in `init`. A text
  field takes focus with the cursor after any prefilled text.
- **The password lives only in its field and in `submit`.** No property
  holds it, so nothing can read it back or log it by accident from a getter.
  The field masks it (`InputType.password`) and reports an empty
  `accessible-value`. Its inner `TextInput` has `accessible-role: none`,
  which makes Slint drop every `accessible-*` property from it
  (`i-slint-compiler`'s `lower_accessibility` pass), so the default
  `accessible-value: text` can't leak it either. A test types a password and
  checks every element's label, value, description and placeholder.
- **Own widgets, not `std-widgets`.** The standard `LineEdit` reports its
  text as `accessible-value` even in password mode (`accessible-value <=>
  text` in every style) and adds a "show password" icon; its sizes and
  focus outline are built for a desk. `TextField` and `Button` are a few
  dozen lines each, sized for a TV, with a thick focus ring on fields and a
  filled highlight on buttons.
- **Keys map to intents in one place.** `widgets/keys.slint` turns a key
  into next (Tab, Down), previous (Shift+Tab, Up), activate (Enter, Space),
  back (Escape) or backspace. Every control moves focus explicitly to its
  neighbour rather than relying on Slint's tree-order Tab chain, so the
  order is written down next to the controls and Phase 2 can send the same
  intents from the D-pad and the A and B buttons. Backspace deletes in a
  field and means back everywhere else. Focus doesn't wrap.
- **Busy blocks input in the view.** While `busy`, fields are read-only and
  submit isn't sent, so a second Enter can't start a second request. Back
  still works, so core can cancel.

### Binding (`src/bind.rs`)

- **`render(window, &LoginScreen)` and `bind(window, on_command)`** are the
  whole binding. `render` fills the screen's view and then switches
  `screen`; `bind` turns callbacks into `Command::Login(LoginAction)` or
  `Command::Quit`. Neither decides anything about the flow.
- **Back.** Core leaves quitting to the UI, so `render` sets
  `MainWindow.back-quits` from the screen and every Back reads it:

  | Screen | Back |
  |---|---|
  | `Starting`, a failed `Restoring`, `Home` | quit |
  | `Restoring` while checking | `ChangeServer` (abandons the check) |
  | `EnterAddress`, idle or failed | quit |
  | `EnterAddress` while checking | `ChangeServer` (abandons the check) |
  | `LogIn` | `ChangeServer` |

  Back on Home quits rather than signing out, because Back is the B button
  in Phase 2 and one press shouldn't throw away a sign-in. Home has a Sign
  out button (`LogOut`) instead, which isn't focused on arrival, so an Enter
  still held from signing in can't press it.
- **Clearing the password.** Core never holds the password after a request,
  so on every `LogIn` whose request is `Failed`, `render` bumps
  `LoginView.password-resets`, and the page's `changed` handler empties and
  focuses the field. A counter rather than a flag, so two failures in a row
  with the same message still clear it.
- **Errors are logged once, at render**, through `tracing` with their
  `Display` text (log text by decision 006), and shown through `wording`.
  Neither the password nor the token reaches the binding's logs: the
  password goes straight from the callback into a `Password`.

### Wording (`src/wording.rs`)

Every error the flow can show gets short, plain text with no technical
detail. The functions match each variant with no wildcard, so a new variant
fails the build until it has wording, and `tests/wording.rs` pins the text
for every variant.

| Error | Wording |
|---|---|
| `ServerUrlError::Empty` | Type your server's address. |
| `ServerUrlError::Invalid` | That doesn't look like a server address. Check it and try again. |
| `ServerUrlError::UnsupportedScheme` | Use an address that starts with http:// or https://. |
| `ServerUrlError::HasCredentials` | Leave your username and password out of the address. You'll sign in next. |
| `ApiError::Unreachable` | Couldn't reach the server. Check the address, and that the server is on. |
| `ApiError::Timeout` | The server took too long to answer. Check that it's running, then try again. |
| `ApiError::NotJellyfin` | That address isn't a Jellyfin server. Check it and try again. |
| `ApiError::InvalidCredentials` | Wrong username or password. |
| `ApiError::TokenRejected` | You've been signed out. Please sign in again. |
| `ApiError::Redirected { server: Some(s) }` | The server has moved to {s}. Check that's your server before you sign in. |
| `ApiError::Redirected { server: None }` | The server sent Couchfin somewhere unexpected. Check the address and try again. |
| `ApiError::InsecureRedirect` | The server tried to switch to an unencrypted connection, so Couchfin stopped. |
| `ApiError::Status` | The server had a problem. Try again in a moment. |
| any `TokenStoreError` (Home notice) | Couchfin couldn't save your sign-in, so you'll need to sign in again next time. |

A failed restore shows "Couldn't connect to {server}" above the API wording,
with Retry and Change server.

### App (`app/src/lib.rs`, `app/src/main.rs`)

- `main` builds a tokio multi-thread runtime, the real stores
  (`FileSettings`, `SystemTokens`) and `JellyfinClient` with the stored
  device ID, makes `MainWindow` fullscreen and runs Slint's event loop on the
  main thread.
- `app::connect(window, flow, runtime)` is the wiring, in a library target
  so `app/tests/login_end_to_end.rs` runs exactly it. The flow's `on_change`
  clones each screen and renders it with `slint::invoke_from_event_loop`;
  each `Command::Login` is spawned on the runtime; `Command::Quit` quits the
  event loop.
- Logging is `tracing-subscriber`'s formatter on stderr, `info` unless
  `RUST_LOG` says otherwise. File logging is a later task.

## Consequences

- The end-to-end test runs the real client against wiremock with the
  keyring unavailable, so the real Secret Service and Steam's on-screen
  keyboard are checked only on the TV.
- `SystemTokens::system` connects to D-Bus before the window opens. A hung
  Secret Service could delay the first frame by up to its 3 s timeout.
  Moving that onto the runtime is a possible follow-up.
- If Slint's `LineEdit` stops exposing password text, the own `TextField`
  can still stay, for the 10-foot sizing and focus ring.
