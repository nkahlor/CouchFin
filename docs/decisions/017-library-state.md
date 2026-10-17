# 017: Library state and the navigation stack

Status: accepted · 2026-10-16

## Context

Decision 014 puts the logic for which Home rows exist, paging and the
navigation stack in couchfin-core. This records how `LibraryFlow` does it. It
follows `LoginFlow` (decision 007): plain-data state, an `on_change`
observer, `Send` futures that take `&self`, and a counter that drops stale
results.

## Decision

### The stack

`LibraryState { screens, session_expired }`. `screens` is the stack, Home
first, and the last one is on screen. Each `ScreenState` is plain data:
`Home(HomeState)`, `Library(GridState)`, `Movie(MovieState)` or
`Series(SeriesState)`. Selecting a card pushes; `Back` pops one level. Back on
Home does nothing in core: `LibraryState::at_root()` tells the UI when to quit.
Home is never popped, so its rows and their focus survive.

Cards carry a `Target` (`Library`, `Movie`, `Series { season }`). An episode
card leads to its series on the episode's season. `OpenCard { row, index }`
and `OpenGridItem(index)` resolve a card in core, so the UI never needs to
know item kinds.

### Actions

`OpenCard`, `OpenGridItem`, `Open(Target)`, `SelectSeason`, `Back`,
`FocusChanged { row, index }`, `NeedItems { first, last }`, `Refresh`. An
action that doesn't fit the top screen is ignored. `Refresh` on Home keeps
the rows until the new ones arrive; on other screens it reloads the screen
in place.

### Home rows

Loaded concurrently (`futures-util` joins them; core stays runtime-free), and
each is published as soon as it arrives, so the first row can show quickly. A
row's `order` keeps them sorted whatever the arrival order. Rows with no
cards are dropped in core. Order: Continue Watching, Next Up, Recently Added
in each movie or TV library, Libraries (music and other kinds get no tile).
Continue Watching cards carry progress (0 to 1) from the user data;
episodes show the series title and an `S2 · E4` subtitle. Each row keeps its
`focused` index.

### Paging

`GridState.total` is `None` until the first page arrives, then `items` has
one `Option<Card>` slot per item, so the UI's lazy model reads by position.
`NeedItems` loads the pages covering the range (size 50 by default,
`with_page_size`), skipping pages loaded or in flight: a page is claimed in
the state before its request starts, and an unclaimed-on-failure page can be
asked for again. Sorted by title, movies and series. Opening a library loads
page 0.

### Stale results

Every pushed screen has a serial number. A request carries its screen's
serial, and its result is applied only if that screen is still on the stack.
So a result for a popped screen is dropped, and so is one for an earlier
visit to the same item. Episodes also check that their season is still
selected.

### Errors and session expiry

Errors are typed (`ApiError`) in the failed screen's `error` field; no text.
`ApiError::TokenRejected` is different: it sets `LibraryState::session_expired`,
publishes, and the flow stops navigating. The app's `on_change` observer sees
the flag, drops the `LibraryFlow` and sends `LoginAction::SessionExpired` (not `LogOut`)
to the `LoginFlow`, which forgets the token and shows the login form with
`ApiError::TokenRejected`, so the UI says why it is asking again.

## Consequences

- Images are not loaded here. Cards carry an `ImageRef` (item, kind, tag)
  and `ImageRef::request(size)` builds the request for the image cache.
- A `LibraryFlow` needs an api `Session`. `LoginFlow::session()` returns it
  while the screen is Home (the token stays out of `LoginScreen`), so the app
  builds the library flow from it.
- A grid of 2,000 items clones its slots on every publish. That is cheap
  next to the requests; if the Phase 3 performance check shows otherwise,
  publish a grid delta instead.
