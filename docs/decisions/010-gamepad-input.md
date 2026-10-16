# 010: Game controllers through SDL3, keyboard through the window

Status: accepted · 2026-10-16

## Context

Phase 2 adds controller navigation. The plan named the `sdl2` crate and a
`couchfin-input` crate that mapped "SDL controller and keyboard" events to one
`Action` enum. Before writing it, we checked the choice against what matters
most for a couch app: input that feels instant, no input bugs, a wide range of
controllers, and the Steam Deck and Steam Controllers later on.

Findings (October 2026):

- **Duplicate input under Steam.** With Steam Input on, Steam adds a virtual
  Xbox pad and leaves the physical controller visible. It tells SDL to hide
  the physical one through `SDL_GAMECONTROLLER_IGNORE_DEVICES(_EXCEPT)`.
  Programs that read controllers some other way often see both, so every
  press arrives twice (steam-for-linux #10175, #8132; RPCS3 #14119). The
  pure-Rust gilrs library was the main alternative, and gilrs-core 0.6.8's
  Linux backend has no such filtering.
- **Steam hardware without Steam.** SDL3 has its own drivers for the Steam
  Deck and both Steam Controllers (gyro, trackpads, paddles; capacitive stick
  touch and grip sense on the 2026 controller, whose mapping SDL merged in May
  2026). A library that only reads the kernel's evdev devices gets the basic
  gamepad. Under Steam, everything is Steam's virtual pad, so this matters
  when Couchfin runs outside Steam.
- **Steam's own input API** (ISteamInput) needs a Steam App ID and licence, so
  it isn't available to a non-Steam shortcut. Steam-side features come from a
  Steam Input template instead (Phase 6).
- **SDL2 vs SDL3.** SDL3 is current (3.4.18), and Fedora 44 ships it. Fedora's
  SDL2 is already `sdl2-compat` running on SDL3. The `sdl3` Rust crate is
  pre-1.0 (0.20.0) and warns of bugs and missing features, but its gamepad API
  covers what Couchfin needs: events, real controller type, touchpads,
  sensors, LED, power and connection state, and hints.
- **Responsiveness depends on how events are read**, not much on the
  library. Steam Input reportedly polls at 125 Hz (8 ms). The Steam Deck's
  sticks and IMU send a constant stream of events, and apps that handle one
  event per frame fall behind on real presses. SDL has had a Deck-specific
  lag regression, fixed by a hint.

## Decision

1. **SDL3 through the `sdl3` crate, pinned to an exact version,** for game
   controllers only (the gamepad subsystem, no SDL window). It sits behind a
   trait in the crate, with a fake for tests, so the mapping logic is tested
   without hardware. If the crate lacks something we need, the fallback is
   `sdl3-sys` for that piece. That would need an `unsafe` exception for the
   crate, so it comes back as its own decision first.
2. **The crate is `couchfin-gamepad`** (was `couchfin-input`), and its enum is
   `Input`. The name says what it owns: controllers. "Gamepad" is SDL3's own
   term, and "controller" already means something else here (decision 007
   calls `LoginFlow` a controller). `Input` avoids confusion with core's
   `LoginAction`, the actions screens send back to couchfin-core.
3. **The keyboard stays in Slint.** Slint owns the window, so it receives key
   events; SDL only reads keyboards for a window it owns. couchfin-ui already
   maps keys to navigation intents (`ui/widgets/keys.slint`), and text fields
   need the raw keys for typing and Steam's on-screen keyboard. couchfin-ui
   gains a public way to feed an intent into that same handler
   (`couchfin_ui::navigate` with an `Intent`,
   [decision 011](011-home-row.md)). `app` maps
   each gamepad `Input` to an intent and passes it in, so both kinds of input
   meet in one place, and focus logic exists once. couchfin-gamepad and
   couchfin-ui still never depend on each other.
4. **How events are read, for snappy input:**
   - a dedicated thread waits on SDL events and drains every pending event
     each time it wakes;
   - deadzones (with hysteresis) and held-direction repeat are handled on that
     thread, with a delay and rate close to the OS keyboard repeat
     (about 400 ms, then about 60 ms) and configurable;
   - each `Input` goes to the UI thread at once.

   SDL hints, such as the Steam Deck HIDAPI one, stay configurable.

## Consequences

- The dev image installs `SDL3-devel` instead of `SDL2-devel`. The Phase 6
  Flatpak bundles SDL3.
- The WSL development host has no SDL3, so building against real SDL happens
  in the dev image and CI. The mapping logic runs anywhere through the fake.
- If remappable bindings are ever wanted, the binding table would move into
  couchfin-core's settings, read by both couchfin-gamepad and couchfin-ui.
- A new `sdl3` crate release is a deliberate upgrade, tested on the TV and
  later on a Steam Deck.
