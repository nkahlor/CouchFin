# 018: Agents look at the UI they build

Status: accepted · 2026-10-16

## Context

#44 passed every test and still shipped overlapping titles and other layout
problems, because nothing in the build ever looked at the screen. Agents can
read PNGs, but each image costs tokens now and on every later turn, so
"look at everything" is too expensive to be a habit.

## Decision

- **A screenshot command:** `cargo run -p couchfin-ui --example screenshots
  -- <out-dir>` renders every screen and state at 1920x1080 and 1280x720
  from realistic fixtures (long, short and non-ASCII titles; many rows of
  varied length; generated pictures, some the wrong shape or missing;
  episodes with progress; error states). It uses Slint's **software
  renderer** into a buffer: no GPU or display, so it runs in CI and in the
  dev container. The renderer is a dev-dependency feature, so the app still
  links only femtovg.
- **Layout checks come first, with no images.** `tests/layout_checks.rs` walks
  the testing backend's element tree on the same fixtures and fails on
  overlapping text, text spilling out of its card, and text or buttons off
  screen. Known failures are `#[ignore = "fixed by #N"]` so CI stays green,
  and the fix removes the `ignore`. The checks see boxes, not clipping, so
  they cannot replace a look.
- **Contact sheets, to look cheaply.** The command also writes four sheets
  (six screens shrunk to 960x540 each) so one image shows six screens. Agents
  open only the sheets for the screens they touched, once, as the last step.
  Full-size PNGs are for inspecting a known problem.
- **CI** runs the command in the dev container on pull requests and uploads
  the PNGs as `screenshots-<sha>` for 14 days. It is not a required check.
- **UI PRs** are one screen or component each, run the layout checks, look at
  the sheets and list what they checked (CLAUDE.md, "Looking at the UI").

## Limits

The software renderer draws no drop shadows or glow and other femtovg-only
effects, and it uses the host's fonts, so text metrics and missing glyphs
(CJK on a host without a CJK font) differ from the TV. What depends on those
still needs a look on the TV. Golden-image comparison is left for later: the
fixtures and renderer are deterministic, so it can be added to the same
command.

## Alternatives

- **Screenshots through femtovg:** needs a GL context in CI; rejected.
- **Always view every PNG:** too costly; rejected for the budgeted rule above.
