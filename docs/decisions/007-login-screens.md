# 007: Login screens in couchfin-ui

Status: accepted · 2026-10-16

## Context

Phase 1 (#13) needs a server address screen, a username and password
screen, and a home screen showing the server and who is signed in.
`couchfin-core` (#12) owns the login flow as state, so the screens must be
pure views: render what core says, send requests back (one-way data flow,
`CLAUDE.md`). Controller input arrives in Phase 2, but every action must
already work by keyboard, and Steam's on-screen keyboard types into whatever
field has focus. The work was split: the views first (stage A), then binding
them to core and wiring `app` once #12 merged (stage B).

## Decision

- **One window, one global per screen.** `MainWindow` has a `screen`
  property (`Screen::Address`, `Login`, `Home`) and shows only that screen.
  Each screen's data and callbacks live in an exported Slint global
  (`AddressScreen`, `LoginScreen`, `HomeScreen`), so the app binds a screen
  without the properties of all three sharing one flat namespace. Switching
  screens inside one window means Steam only ever sees one fullscreen
  window.
- **Only the current screen exists.** Each screen is an `if` in
  `MainWindow`, so switching away destroys it, along with anything typed
  into it, and the new screen focuses its first control in `init`.
- **The UI owns the wording.** Properties carry data (names, an address) or
  a message the UI chose; `ApiError`'s `Display` is log text
  ([decision 006](006-api-client-design.md)). Busy text ("Checking the
  server…", "Signing in…") is fixed in the markup.
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
  focus outline are built for a desk.
  `TextField` and `Button` are a few dozen lines each, sized for a TV, with
  a thick focus ring on fields and a filled highlight on buttons.
- **Keys map to intents in one place.** `widgets/keys.slint` turns a key
  into next (Tab, Down), previous (Shift+Tab, Up), activate (Enter, Space),
  back (Escape) or backspace. Every control moves focus explicitly to its
  neighbour rather than relying on Slint's tree-order Tab chain, so the
  order is written down next to the controls and Phase 2 can send the same
  intents from the D-pad and the A and B buttons. Backspace deletes in a
  field and means back on buttons and the home screen. Focus doesn't wrap.
- **Busy blocks input in the view.** While `busy`, fields are read-only and
  submit isn't sent, so a second Enter can't start a second request. Back
  still works, so core can decide whether to cancel.
- **Tests go through the public API on Slint's testing backend.** They type
  and press keys with `WindowEvent`s, read fields through the in-out
  properties or accessible values, and check focus by where typing lands or
  which callback Enter fires, with no test-only properties.

## Consequences

- Stage B binds the globals to core's state, maps each core error variant
  to a message, and replaces `HelloWindow` in `app` with `MainWindow`.
- After a failed login the password stays in its field. Clearing it would
  need core to say so (for example a property the screen watches), which
  stage B can add if wanted.
- If Slint's `LineEdit` stops exposing password text, the own `TextField`
  can still stay, for the 10-foot sizing and focus ring.
