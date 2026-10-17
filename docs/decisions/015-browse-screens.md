# 015: The browsing screens in the UI

Status: accepted · 2026-10-16

## Context

[Decision 014](014-home-and-library-layout.md) fixed what Home, the library
and the detail pages show. Stage A of #18 builds them as pure views with
placeholder data; Stage B (#42) binds them to couchfin-core's library state.
This records how the views are built, so Stage B knows what to feed them.

## Decision

### Home: rows from a model, one focus target

- **`HomeView.rows` is a model of `RowData`** (`title`, `shape`, `cards`,
  `current`). `CarouselRow` (`widgets/carousel.slint`) is the carousel of
  decision 011 generalised: same anchored slot, peek, dimming, animation
  timings and image slots, but a view only. It takes a model of `CardData`, a
  `CardShape` (`Poster` 2:3, or `Thumb` 16:9 with an optional progress bar and
  a subtitle) and says nothing about focus.
- **One `FocusScope` for all the rows**, as for the single row in 011. The
  page holds `focused-row`; each row's own card lives in `RowData.current`,
  which the screen writes as the user moves. So each row remembers its card
  while another has focus, and core can keep it to reopen Home where it was
  (decision 014: "Home keeps its row positions").
- **Focus map.** Left and Right move along the focused row and stop at its
  ends. Down (or Tab) goes to the next row, Up (or Shift+Tab) to the previous
  one; Up on the first row does nothing. Enter fires
  `card-activated(row, index)`; Escape and Backspace fire `back()`.
- **Sign out is the last stop below the rows.** Down from the bottom row moves
  focus to the Sign out button, and Up from it returns to the bottom row, on
  the card it left. Left and Right on it do nothing. Rows then Sign out is a
  single column, so the D-pad needs no second rule, and nothing in the rows
  changes. Sign out stays at the bottom of the screen where it was.
  (Alternatives: Sign out in the header, reached by Up from the first row,
  would have made a held Up on a long Home end on a destructive button; a
  hidden menu button has no keyboard equivalent.)
- **The focused row stays at one height.** The rows sit in a clipped
  viewport under the header and the strip of rows moves up as focus goes
  down, animated over `Theme.scroll-duration`, so the focused row's top is
  always `Theme.home-row-anchor` below the viewport's top. The offset is
  counted as "poster rows above" and "thumbnail rows above" times each
  shape's fixed height, not measured, so a resize keeps it. Rows after the
  focused one show below it, so the next row's title and first cards peek
  in on a TV.
- **Empty rows.** A row with no cards is not created and takes no room;
  Up and Down step over up to two empty rows in a row. Core is meant to
  drop empty rows before they get here (014), so this is a guard.
- **Sizes.** The poster height is what the viewport leaves after a poster
  row's fixed parts (title, room for the grown card, title line, gap),
  between `Theme.home-poster-min-height` and `poster-max-height`. A
  thumbnail is `Theme.thumb-height-ratio` of that high, so both shapes fit
  the same screen, and the whole focused row fits at 1280x720.

### Library: a grid that draws only what is visible

- **A `ListView` of rows of posters.** Slint's `ListView` creates only the
  rows in view; each row creates only the cards it holds, and each card
  reads its data from `LibraryView.items[place]`. Drawing 2,000 items means
  about three rows of eight cards. The rows are an integer model
  (`ceil(item-count / columns)`), so no model of rows is ever built, and
  `columns` follows the window's width. Chosen over a hand-windowed grid
  because `ListView` also gives the scroll bar and wheel scrolling for free
  and the row height is fixed, which is what it needs.
- **The binding feeds it a flat model and a count.** `item-count` is the
  library's size; `items` is indexed by place and is only read for places on
  screen, so core can implement it as a lazily loading `Model` whose
  `row_data` returns what it has. A place with no data draws an empty card
  (the placeholder look). `items-needed(first, last)` fires whenever the
  visible range changes (scrolling, resizing), with `last` exclusive, so
  core can fetch those places.
- **Focus map.** One focus target, on the first item. Left and Right step
  through the items and stop at the first and last; Up and Down move a row
  (Down from a row above a shorter last row lands on the last item); the
  grid scrolls just enough to show the focused card. Enter fires
  `item-activated(index)`, Escape and Backspace `back()`.
- **Test.** `tests/library_grid.rs` builds 2,000 items and checks that only a
  screenful of card elements exist and that only about that many places were
  read from the model at both screen sizes.

### Detail pages

- Movie and series share `Backdrop` and `DetailHeader`
  (`widgets/detail-header.slint`). The backdrop slot shows the surface colour
  under a scrim when empty.
- **Movie:** the Play or Resume button has focus and is the only control;
  Enter fires `play()` and the page says "Playback arrives soon" (Phase 4
  replaces that). Back fires `back()`.
- **Series:** one focus target that starts on the seasons. Left and Right
  choose a season and fire `season-selected(index)` at once, so core loads
  its episodes; `selected-season` is in-out. Down (or Enter) enters the
  episodes, where Up and Down step and scroll, Up from the first returns to
  the seasons, and Enter fires `episode-activated(index)` and shows the same
  note. Left and Right do nothing on the episodes.

## Consequences

- `card-activated` now carries the row as well as the index; `bind` logs
  both.
- Stage B needs from core: the rows to show (empty ones dropped) with each
  row's remembered `current`; for a library, its name, `item-count` and a
  model that pages in the places named by `items-needed`; for a movie or
  series, the header strings, the seasons and the selected season's
  episodes, and to load a season on `season-selected`.
- The parity table in `tests/navigate.rs` covers the multi-row Home, the
  grid and both detail pages: every pair of intents, and moves deep into
  each, run through `navigate` and through the keys.
- `Screen` gained `Library`, `Movie` and `Series`. Nothing in `app` shows
  them yet; Home's cards still only log.
