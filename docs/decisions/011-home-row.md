# 011: Controller intents and Home's row of cards

Status: accepted · 2026-10-16

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
- **Scrolling.** The row keeps the current card centered, including at the
  first and last cards, with edge padding computed from the row and card
  widths and a scroll offset derived from `current`. This leaves blank space
  at the ends and keeps the selected position fixed as titles move beneath
  it, even after a resize. The content is a non-interactive `Flickable`
  rather than a clipping `Rectangle`, so screen readers still list cards
  out of view.
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
- **The highlight.** The current card in a focused row gets an 8 px ring
  in `Theme.focus` and grows to 108%, both animated over 120 ms; the row
  scrolls over 150 ms. The ring is drawn over the poster so a Phase 3
  image can't cover it. Every new value is a `Theme` property.

## Consequences

- `tests/home_row.rs` checks that the focused card is inside the row at
  1920×1080 and 1280×720 after each move. Slint's testing backend only
  finds elements at least partly on screen, which the tests rely on.
- Slint's software renderer doesn't draw `transform-scale`, so snapshots
  taken with it show only the ring. The app uses femtovg, which draws
  both.
- Phase 3 replaces the hard-coded `titles` in `screens/home.slint` with
  rows from the library, and the card's poster rectangle with an image.
  Several rows will need Up and Down between rows, each remembering its own
  card, with the same one-`FocusScope`-per-row pattern.
