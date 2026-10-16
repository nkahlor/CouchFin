# 013: Merging Steam's emulated keys with the controller

Status: accepted · 2026-10-16

## Context

Found on the TV: with Steam running but Couchfin not launched from it,
Steam Input's Desktop Layout makes the controller *also* type arrow keys,
Enter and Escape. Pressing D-pad Right echoed `^[[C` in the terminal while
the gamepad example printed `Right`; with Steam quit, only `Right`. Every
press therefore reached the UI twice, as a gamepad `Intent` through
`navigate()` and as a real key through Slint, and a hold moved by 1, 3, 5.
SDL can't filter this, because the keys come from the window.
[Decision 010](010-gamepad-input.md) only covered duplicate gamepads.

The fix must be right whether Steam is open or not: one press moves once even
if the key lands a few milliseconds before or after the pad event; a hold
gives only our gamepad repeat (not Steam's held key or the OS key repeat);
keyboard alone and controller alone behave as before; and a real keyboard
used together with a controller still works.

## Decision

couchfin-ui owns a `DuplicateGuard`, fed by a `Navigator` that sits in front
of both paths: controller intents go through `Navigator::navigate`, and the
window offers every key to `Navigator` first (a `capture-key-pressed` handler
on a `FocusScope` around the screens in `main-window.slint`, calling the
`filter-key` callback; returning true swallows the key). Rules, per intent:

1. **Duplicate window:** a pad intent and a key of the same intent within
   `merge_window` (50 ms), in either order, are one press. The second is
   dropped. Each event merges once, so two quick pad presses with one key
   between them still count twice.
2. **Hold:** a key *repeat* (the OS auto-repeat flag) within `hold_window`
   (450 ms) of a controller event of that intent is dropped. 450 ms outlasts
   the gap between the gamepad's own repeats (400 ms, then 60 ms).
3. **Merged run:** repeats that follow a merged press are dropped while each
   follows the last within `run_gap` (1 s), until the next real key press.
   This covers the one-shot buttons (A, B), which the gamepad never repeats
   but Steam's Enter and Escape do.
4. Keys that aren't navigation keys (anything typed into a field) are never
   looked at. Different intents never merge: a real keyboard Left at the same
   moment as a pad Right counts.

Keyboard-only input matches no controller event and passes through
unchanged, including its own auto-repeat; controller-only input is never
dropped. The timings are in `GuardConfig`, and time comes from an injected
`Clock`, so tests are exact. `app::connect_gamepads_with` takes both;
`connect_gamepads` uses the defaults and the system clock.

`Navigator::navigate` injects the key with `navigate()` as before, under a
flag that lets the capture handler pass it, so injected keys aren't counted as
the keyboard.

## Why this and not the alternatives

- *Suppress keyboard repeats while a direction is active* alone misses the
  first press, which needs the duplicate window anyway, and cannot be
  right in both arrival orders.
- *Detecting Steam* (its environment, a process list) is fragile and wrong
  when Steam's layout differs; timing works with Steam open or closed.
- *Filtering in the app or gamepad crate* would put keyboard knowledge where
  it doesn't belong and break the dependency rule; the UI is where both
  paths meet.

The cost: a real keyboard press of the same intent within 50 ms of a
controller press is merged, and so is a keyboard repeat of that intent within
450 ms of a controller event. Nobody types that fast on both devices.

## Testing

`app/tests/steam_desktop_layout.rs` runs the real wiring
(`connect_gamepads_with`, `FakePads`, keys dispatched to the window) against
`app/tests/common/steam.rs`, a fixture that simulates the Desktop Layout: for
each fake press it sends the matching key at -5, +2 or +15 ms, and for a hold
a key-down plus OS repeat (500 ms, then every 33 ms) until release. The guard's
rules also have unit-style tests in `couchfin-ui/tests/duplicate_guard.rs`.

## Debugging

At `tracing` debug level (`RUST_LOG=couchfin=debug`) the app logs each
controller `Input`, each `Intent` sent through the guard and each
navigation key, with its source and a monotonic millisecond timestamp, and
every dropped duplicate with the reason. Nothing is logged at info level per
event, and text typed into a field is never logged.
