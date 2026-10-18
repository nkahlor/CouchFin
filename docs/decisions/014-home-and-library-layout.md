# 014: Home and library layout

Status: accepted · 2026-10-16

## Context

Phase 3 replaces the placeholder row with the user's real library. The plan
said "Home rows for Continue Watching plus one row per library, a library
grid and a detail page", which leaves open what the rows are, how they
behave, and where browsing logic lives. This records the structure, so the
API, core and UI tasks build the same thing. It follows how 10-foot apps
(HBO Max, Netflix, the official Jellyfin TV clients) organise content.

## Decision

### Home: a vertical stack of carousel rows

Top to bottom; a row with no items is hidden:

1. **Continue Watching:** `GET /UserItems/Resume`. 16:9 thumbnails with a
   progress bar. It comes first, because carrying on is the most common
   thing to do on a TV.
2. **Next Up:** `GET /Shows/NextUp`, the next unwatched episode of each
   series in progress. 16:9 thumbnails.
3. **Recently Added in {library}:** one row per movie or TV library,
   `GET /Items/Latest` with that library as the parent. 2:3 posters, about 16
   items each, so Home stays fast. This is what "one row per library" means.
4. **Libraries ("My Media"):** `GET /UserViews`, one tile per library.
   Selecting one opens its grid.

Every row is the carousel from decision 011: the focused card is anchored on
the left, the previous card peeks in, and passed cards are dimmed. Up and Down move between rows. The page scrolls so the
focused row stays at the same height on screen, the vertical counterpart of
the horizontal anchor. Each row remembers its own focused card. The row title
sits above the row. Thumbnail rows and poster rows have different card
shapes but the same anchor slot.

### Library: a poster grid

Selecting a library opens a grid of 2:3 posters for the whole library,
sorted by title. It pages from the server as you scroll, and only draws what
is on screen. A carousel suits about 16 items, not 2,000. The grid is what
the Phase 3 performance check measures (60 fps while scrolling a 2,000-item
library, the first row of posters within 1 s).

### Detail page

- **Movie:** backdrop, title, year, runtime, rating, overview, and a Play or
  Resume button. Until playback exists (Phase 4), the button says it is
  coming rather than doing nothing.
- **Series:** the same header, then its seasons, then a season's episodes
  as a list.

### Navigation

A stack in couchfin-core: Home → library grid → detail → season → … Back
always goes back one level, and Home keeps its row positions. Screens render
core's state and send actions back (CLAUDE.md, one-way data flow). The
logic for which rows exist, paging, and the stack lives in core, not the UI.

## Not in M1

A featured banner at the top of Home, search (it needs an on-screen keyboard
flow), sort and filter options, collections as their own section, and
following the Home layout set on the server. Music and live TV were already
non-goals.

## Consequences

- couchfin-api needs more than the plan first listed:
  - `/Shows/NextUp` and `/Items/Latest`
  - single-item details (`/Items/{id}`)
  - seasons and episodes
  - Backdrop and Thumb images as well as Primary
- couchfin-core gains a task the plan didn't have: library state (Home rows,
  hiding empty ones, the paged grid, detail loading) and the navigation
  stack.
- couchfin-ui's task covers multi-row Home, the grid, and the movie and
  series detail pages.
