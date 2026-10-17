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
  changes. Sign out is part of the page: the last item in the scrolling
  strip, right after the last row, so it never floats over content (#54). It
  was chosen over a compact header because a header button would sit above
  the rows it is reached after.
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
- **Empty rows.** Core drops empty rows (014), so the page draws every
  row it is given and Up and Down move one row at a time. (Stage A stepped
  over up to two empty ones; that guard is gone.) Enter on a row with no
  card does nothing.
- **Vertical layout (#51, #53, #54).** Top to bottom: a one-line header
  (server, version, who is signed in), the details strip, the clipped rows
  viewport, then one full-width line each for the row error and the notice.
  Cards carry no title; the strip (`DetailsStrip`) shows the focused card's
  title and one line (`CardData.meta`: "2016 · 1 h 56 min", "S2 · E4",
  "Library"), formatted in the UI from plain `Card.year`/`runtime` in core.
  A card whose picture will never come (`image-missing`: no image ref, or
  the server answered `NoImage`) draws its title inside, centred, at most
  three lines, elided; a picture still loading draws plain. Row titles sit
  above the cards at the carousel inset, outside the clipped card area.
  Elements with a size but no x/y are centred by Slint, so these set `y`.
- **Sizes.** The poster height shares the viewport among
  `Theme.home-rows-fit` poster rows (2.5; 3 from `home-tall-screen`), less
  a row's fixed parts, between `Theme.home-poster-min-height` and
  `poster-max-height`. About 2.5 rows show at 720p and 3 at 1080p. A
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
- The parity table in `tests/navigate.rs` covers the multi-row Home, the
  grid and both detail pages. Running all 36 intent pairs on every screen
  and state took about 100 s in a debug build, so each scenario runs six
  pairs (every intent first once and second once, rotated by a per-scenario
  salt, so across the table every pair is covered) plus the mid-animation
  lockstep comparison: about 11 s.
- `Screen` gained `Library`, `Movie` and `Series`.

## Binding to core (stage B)

`couchfin_ui::install(window, loader, on_command)` binds the browsing
screens and returns a `Feed`; `app` owns the `LibraryFlow` and `ImageCache`
(`app/src/library.rs`).

- **State in.** `LibraryFlow::on_change` calls `Feed::state`, which queues
  the state and wakes the event loop (`invoke_from_event_loop`). States
  collapse to the latest, so a burst of page loads is one update. `show()`
  and `reset()` tell the UI when sign-in is on Home, so the screens never
  fight `render` over the window's `Screen`.
- **Actions out.** Callbacks become `Command::Library(LibraryAction)`:
  `OpenCard`, `FocusChanged` (fired for moves along and between rows),
  `NeedItems` (the visible range plus half a screen each side, once per
  distinct range), `OpenGridItem`, `SelectSeason`, `Back`. Back on Home is
  still `bind`'s: it quits, since Home is always the root.
- **Models.** Home keeps one `VecModel<RowData>` with a `VecModel<CardData>`
  per row, matched to core's rows by `RowKind`, so rows arriving or a
  refresh change cells in place. The page's own `current` wins over core's
  `focused` for rows already there; a new row starts on core's. The focused
  row and the scroll counts live in `HomeView` and are corrected when rows
  arrive above the focused one. The grid's `items` is a lazy `Model` that
  builds a `CardData` from `GridState::item(i)` only when Slint reads it
  (an empty card while loading). The season the user picked is kept until
  core confirms it.
- **Errors.** Typed `error` fields are worded by `wording::browse_error`
  (exhaustive); Home shows `status` instead of rows while it has none.
- **Pictures.** An `ImageLoader` (implemented in `app` over `ImageCache`
  on tokio) starts a load per wanted card and returns a `Guard`; dropping it
  aborts the load. After every state, focus move or scroll the UI recomputes
  the wanted set and cancels the rest. Wanted: on Home the focused row,
  one behind and two ahead, from two cards behind the current one to the
  last that fits plus two; in the grid the visible places plus half a screen
  each side, nearest first, with finished pictures kept three margins out
  and let go beyond that (2,000 posters would otherwise hold about a
  gigabyte); on a detail page its backdrop (`ImageKind::Backdrop`, chosen by
  core). Widths are the on-screen size times the scale factor, rounded up to
  32 px. `NoImage` keeps the
  placeholder. Pixels become a `slint::Image` on the UI thread (one copy).
- **Session.** `app` drops the `LibraryFlow` when `session_expired` is set
  and sends `LoginAction::SessionExpired`; leaving Home does the same
  without that action.
- **Checks.** `app/tests/phase3_browse.rs` (wiremock, 2,000 items) asserts
  the first row's posters within 1 s of launch and an end-to-end scroll with
  every page requested once; `app/examples/perf_grid.rs` measures frame
  times on the TV.
