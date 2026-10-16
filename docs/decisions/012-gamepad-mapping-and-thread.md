# 012: How gamepad buttons map, repeat and reach the app

Status: accepted · 2026-10-16

## Context

Decision 010 chose SDL3's gamepad API on a dedicated thread, with the keyboard
left to Slint. Building `couchfin-gamepad` (issue #14) left these questions:
which button means what on every kind of pad, how the stick and held
directions behave, which controllers count, and whether SDL may run on a
thread that isn't the process's main thread when Slint owns that one.

## Decision

### Mapping

| Pad | `Input` |
|---|---|
| D-pad, left stick | `Up`, `Down`, `Left`, `Right`, repeating while held |
| South button (A on Xbox and Steam pads, Cross on Sony) | `Select` |
| East button (B on Xbox and Steam pads, Circle on Sony) | `Back` |
| Start (Xbox Menu, Sony Options, Nintendo Plus) | `Menu` |

- **Buttons go by position, not label.** SDL3 reports buttons by position
  (south, east, ...) and applies each controller's mapping, so every pad SDL
  knows works the same. On a Nintendo-layout pad the bottom button is
  labelled B and the right one A, so there B selects and A goes back. That
  matches Steam's own UI on the Deck and in Big Picture, and under Steam
  Input every pad reaches Couchfin as Steam's Xbox-style virtual pad anyway,
  laid out by Steam's own Nintendo-layout setting. Following labels instead
  would need SDL's per-type button labels; it can be added later if wanted.
- Select, Back and Menu fire once, on the press. The View, Guide, face
  buttons west and north, bumpers, stick clicks, triggers and the right
  stick are unused for now. Playback controls will join `Input` in Phase 4;
  the enum is exhaustive on purpose, so a new input is a compile error in the
  app rather than a silently ignored case.
- **Every connected pad is listened to**, each with its own held directions.
  On a couch anyone may pick up any pad, and Steam's virtual pad comes and
  goes with Steam Input.

### Directions and repeat

- A held direction fires on the press, again after 400 ms, then every 60 ms,
  close to a desktop keyboard. `Repeat` makes both configurable (neither may
  be zero).
- The D-pad and the left stick are separate sources. On the D-pad the most
  recent press repeats; releasing it stops the repeat even if an older
  direction is still down, as with a keyboard.
- A late wake fires one repeat and restarts the interval from then, rather
  than catching up in a burst that would overshoot.
- Disconnecting a pad, or SDL reporting it again (`Added`), drops whatever it
  was holding, so its repeat stops.
- **Stick:** engages when its stronger axis reaches about 64% of full travel
  (21000) *and* the stick points within 30 degrees of that axis. In the
  60 degree wedges between, nothing engages. Once engaged, the direction is
  locked until the stick's distance from the centre falls below about 43%
  (14000): it can't switch to another direction mid-hold, and passing
  through a diagonal doesn't cancel it. The gap is hysteresis, so a stick
  resting near a threshold can't flicker. The stick is read after each batch
  of events, so a push that arrives as separate X and Y events is judged as
  one position. `Deadzone` makes press, release and cone configurable
  (`new(press, release)` and `with_cone(degrees)`, validated).
  - *Update, after the TV test:* the first version engaged at half travel
    (16384) on the stronger axis and held until that axis fell below a third
    (10923). A thumb pushing Right drifted down, its X relaxed below the
    release point while Y was still high, and the stick re-engaged as Down,
    which on Home leaves the carousel. Judging release by distance from the
    centre, locking, the cone and the higher threshold fix this.
- The right stick is left out until something needs it (seeking, perhaps).

### Threading

- SDL's own definition of "the main thread", for event pumping, is the
  thread that started its event subsystem; the video thread takes over only
  once SDL video starts, which it never does here. `SDL_InitSubSystem`
  records that thread (`SDL_EventsThreadID` in `src/SDL.c`, with the comment
  "This is the thread which should be pumping events"). The `sdl3` crate's
  own main-thread check is likewise "the first thread that called
  `sdl3::init`". So SDL runs entirely on the `couchfin-gamepad` thread, with
  Slint on the process's main thread, and every SDL object stays on the
  gamepad thread. Only the wakeup crosses threads, through `SDL_PushEvent`,
  which SDL documents as thread-safe.
- `Gamepads::start` opens the source on the new thread and returns once it is
  open, so an SDL failure comes back as an error to the caller.
- The thread blocks in `SDL_WaitEventTimeout` until an event or the next
  repeat deadline, then drains the queue with `SDL_PollEvent` and maps the
  whole batch. A Steam Deck's stream of stick events therefore never delays a
  button press behind it.
- With no SDL window, SDL waits by checking the controllers about once a
  millisecond and sleeping in between. That is how SDL reads controllers on
  Linux with or without a window (with a controller open it polls at the same
  rate even when a window exists), so it adds no latency, and the thread never
  spins. The cost is small: about 0.08 s of CPU in 5 s idle, in a debug build.
- Stopping sets a flag and pushes a quit event to end the wait; dropping
  `Gamepads` does both and joins the thread, which shuts SDL down. Only one
  SDL source can run per process at a time.
- The `sdl3` crate unwraps an SDL button or axis number it doesn't know. The
  system SDL isn't pinned, so a newer one with a new button could hit that;
  the source catches the panic and skips that one event instead of losing
  controller input.

### Steam Input and hints

- Nothing in Couchfin sets `SDL_GAMECONTROLLER_IGNORE_DEVICES`,
  `..._IGNORE_DEVICES_EXCEPT`, `SDL_GAMECONTROLLERCONFIG` or
  `..._CONFIG_FILE`: Steam passes them as environment variables to hide the
  physical pad behind its virtual one, and SDL reads them. `SdlHints`
  refuses those names, and SDL lets an environment variable win over a hint
  anyway.
- `SdlHints` holds the hints set before SDL starts. The default has one,
  `SDL_NO_SIGNAL_HANDLERS=1`: otherwise SDL turns Ctrl+C and SIGTERM into a
  quit event that nothing reads, and the app couldn't be stopped that way.
  Others, such as `SDL_JOYSTICK_HIDAPI_STEAMDECK`, can be added in code or set
  as environment variables.

## Consequences

- With no SDL window, SDL never drops controller input for lack of focus, so
  Couchfin reacts to the pad even when another window is in front. Under
  Steam that's harmless (Steam stops feeding the game while its overlay is
  open), but outside Steam it may need a focus check in the app later.
- Connects and disconnects are logged at info level with the pad's name,
  type, real type and USB IDs, which shows on the TV whether Steam's virtual
  pad or the physical one is in use. Nothing is logged per button or axis.
- `cargo run -p couchfin-gamepad --example gamepads` prints every input, to
  check a controller without the app.
