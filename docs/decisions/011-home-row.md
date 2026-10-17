# 011: Controller intents and Home's row of cards

Status: accepted · 2026-10-16. Home became a stack of these rows in
[decision 015](015-browse-screens.md): the carousel is now `CarouselRow`, Down
moves to the next row, and Sign out is reached by Down from the bottom row.

## Context

Phase 2 (#15) asks for a Home screen with a horizontal row of ten
placeholder title cards and a focus highlight that reads from a sofa. Its
"Done when" asks that the D-pad and the arrow keys both move focus, A and
Enter select, and B and Escape go back. [Decision 010](010-gamepad-input.md)
keeps the keyboard in Slint and has `app` pass each controller input into
couchfin-ui, so focus logic exists once. #14 builds that wiring in parallel,
so the entry point had to be fixed first.

## Decision

### `Intent` and `navigate`

- **A controller press becomes its key.** `couchfin_ui::navigate(window,
  intent)` sends the key press and release for the `Intent` (`Up`, `Down`,
  `Left`, `Right`, `Activate`, `Back`) to the window: the arrow keys, Enter
  and Escape. Every control already handles those keys through
  `widgets/keys.slint`, so a controller gets exactly the keyboard's
  behaviour with no second focus path. A screen can't forget the
  controller, because it never sees it.
- **Text fields.** Left and Right move the cursor in a field, for the
  controller as for the keyboard, and Up and Down still move between
  controls. We kept that rather than making Left and Right leave a field:
  a form has its controls in a column, so Up and Down are enough to move
  on, and editing an address with Steam's on-screen keyboard needs the
  cursor.
- **Activate is Enter and Back is Escape**, not Space or Backspace. Enter
  presses a button and submits a field; Space would type a space in a
  field. Backspace deletes in a field, so B would delete instead of going
  back.
- **Repeat stays with the caller.** `navigate` presses and releases at
  once. Holding the D-pad repeats by calling it again, as couchfin-gamepad
  does on its own thread (decision 010).
- **Parity is a test.** `tests/navigate.rs` runs every pair of intents,
  then Enter and some typing, on every screen and state (starting,
  restoring, failed restore, address idle, checking and failed, login idle
  and busy, Home), once through `navigate` and once through the keys, and
  compares the commands sent and every element's accessible label, value
  and selection after each step. A new screen joins its table.

### The row

- **One focus target for the row.** The row is a single `FocusScope` with
  a `current` index; the cards (`widgets/card.slint`) take no focus and
  draw the highlight when `current` and the row has focus. Moving along the
  row is one property change, and the row remembers its card while Sign
  out has focus, so Up returns to the card you left with no extra state.
  Each card is an accessibility `list-item` with its index, the count and
  `item-selected` for the current card, and its default action selects it.
- **Focus map.** On arrival the row has focus, on the first card, so an
  Enter held from signing in selects a card rather than signing out. Left
  and Right stop at the ends: on a TV a wrap from the last card to the
  first looks like a jump, and the first card is never far away. Down (or
  Tab) goes to Sign out, and Up (or Shift+Tab) comes back. Up on the row
  and Left or Right on Sign out do nothing. Back still quits, by decision
  008, so the B button can't throw a sign-in away.
- **A carousel anchored on the left.** After review the row became a
  carousel like HBO Max's, replacing an earlier version that kept the
  current card centred. The current card always sits in one slot,
  `Theme.carousel-inset` (160 px) from the screen's left edge. Right slides
  the whole row left by one card and Left slides it back. Focus never
  travels across the screen, not even on the last card: there the row
  keeps sliding and empty space opens on the right. The row bleeds past
  the page margin to the screen's edge, and the inset is less than a card
  and its gap, so the card before the current one peeks in, cut off by
  the edge. On Title 1 nothing is there. The slide is
  `content-x = -current × slot` on a non-interactive `Flickable`, so screen
  readers still list the cards out of view. Cards are placed by `x`, not
  in a layout, because their size follows the row's.
- **Motion that keeps up with the D-pad.** `current` changes at once on
  every press. Only the drawing animates: the slide (150 ms), the active
  card's growth, ring and glow, and the dimming (120 ms). A held D-pad
  repeats every 60 ms or so (couchfin-gamepad, or the OS keyboard repeat),
  and each press retargets the running slide from where it is, so focus
  is never behind the input. The row has no repeat throttle of its own:
  repeat belongs to whatever sends the presses.
- **Selecting.** There is no detail page until Phase 3, so Enter on a card
  calls `HomeView.card-activated(index)`, which the binding logs, and the
  page shows "Selected: {title}" in the header so it is visible on the TV.
  No `Command` yet: Phase 3 adds one with the item to open.
- **Sizes for 10 feet.** Posters are 2:3. Their height is whatever the
  screen leaves for the row, between `Theme.poster-min-height` and
  `poster-max-height` (420 px, so about six cards across 1920 px). The
  row's height constraints are set on the row itself, because the cards'
  size follows the row's; letting them feed back makes a binding loop.
  At 1280×720 the header, the row, Sign out and the notice all fit.
- **The highlight is light; the scale does the work.** The current card in
  a focused row grows to 110% about its centre, gets a 3 px ring in
  `Theme.focus` and a soft glow (`drop-shadow` in `Theme.card-glow`), and
  its title turns white and bold. The growth is a real size change, not
  `transform-scale`, so tests can measure it and every renderer draws it.
  The poster grows past the card's slot, so its neighbours never move. The
  ring is drawn over the poster so an image can't cover it. When focus is
  on Sign out the row keeps its place but drops the highlight.
- **Cards already passed are dimmed, not blurred (yet).** The cards left of
  the current one are meant to look slightly blurred. Slint 1.18 can't blur
  content: its only blur properties are `drop-shadow-blur` and
  `inner-shadow-blur` (checked in i-slint-compiler 1.18.1). For now a
  passed card gets a shade of the background over its poster
  (`Theme.passed-card-shade`) and a dimmer title (`Theme.passed-title`),
  both cross-faded over 120 ms; cards from the current one on are
  untouched. We use colours with alpha rather than `opacity`, because
  opacity costs a render layer per card, and Slint's software renderer
  draws it wrongly.
- **Phase 3 brings the blur from the image cache.** The card already has
  slots for it: `image` (the poster), `blurred-image`, and the `passed`
  state that picks the treatment. couchfin-core's image cache will also
  make a blurred copy of each poster, and a passed card cross-fades to
  that copy under the shade. The card stays logic-free: the row says which
  cards are passed, and core supplies both images. Both are empty for the
  placeholders.
- Every new value is a `Theme` property.

## Consequences

- `tests/home_carousel.rs` checks:
  - the current card's x is the same slot for every card from the first to
    the last, at 1920×1080 and 1280×720;
  - for every card after the first, the one before it peeks in across the
    left edge, and nothing is there on Title 1;
  - after the last card, only empty space;
  - passed cards are shaded and the rest aren't;
  - the active card is larger and ringed and the others aren't, and Sign
    out drops the highlight;
  - focus moves at once while the row is still sliding.

  Slint's testing backend only finds elements at least partly on screen,
  which these tests rely on.
- The parity test runs a keyboard window and a `navigate` window in step,
  60 ms apart, part-way through the animations, and compares where every
  element is, its size and its opacity, so a carousel that slides, grows
  or dims differently for a controller fails.
- Software-renderer snapshots show the size and ring but not the glow. The
  app's femtovg renderer draws all three.
- Phase 3 replaces the hard-coded `titles` in `screens/home.slint` with
  rows from the library, and fills `image` and `blurred-image`. Several rows
  will need Up and Down between rows, each remembering its own card, with
  the same one-`FocusScope`-per-row pattern.

## Update (2026-10-16, decision 014)

In Phase 3, Home becomes a vertical stack of these carousel rows: Continue
Watching, Next Up, Recently Added per library, and Libraries. Up and Down
move between rows, and each row remembers its own focused card. The carousel
behaviour above applies to every row.
