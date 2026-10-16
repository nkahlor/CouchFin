# 009: All styling values live in `Theme`

Status: accepted · 2026-10-16

## Context

The Phase 1 screens put their colours, text sizes and main spacing in a Slint
global, `Theme` (`crates/couchfin-ui/ui/theme.slint`). About twenty values were
still written inline in screens and widgets: control heights, gutters,
paddings, small gaps, the busy indicator's size, the unfocused border width
and the bold font weight. We may want more than one theme later (for example
light, high contrast or large text), and every screen added with inline values
would make that harder.

## Decision

- **Every colour, length and font weight is a named `Theme` property.**
  Screens and widgets refer to them (`Theme.control-height`), never to
  literals (`96px`). Names say what a value is for (`field-padding`,
  `status-height`), not what it is, so a theme can change one use without
  touching another. Zero lengths, percentages, durations, angles and opacity
  maths aren't styling, and stay inline.
- **A test enforces it.** `crates/couchfin-ui/tests/theme_tokens.rs` scans
  every `.slint` file except `theme.slint` and fails on a hex colour, a colour
  function (`rgb()`, `Colors.*`), a non-zero length with a unit, or a numeric
  `font-weight`. Comments and string literals (text shown on screen) are
  ignored. A rule in CLAUDE.md points agents at it.

## Not done yet: switching themes

`Theme`'s properties are `out` with fixed values, so there is still one theme,
chosen when the app is built. Switching at runtime, when we need it, takes:

1. `theme.slint`: an `in property` selecting the theme (an enum), with each
   property computed from it, or `in-out` properties that Rust sets from a
   palette. Screens don't change; they keep reading `Theme.*`.
2. couchfin-core: the user's choice is app state, stored in `Settings`
   (`settings.toml`), and the UI renders whichever theme core reports.
3. A settings screen, reachable by controller.

A theme that changes typefaces also needs a `font-family` property and a
bundled font; Couchfin uses Slint's default font today.

## Consequences

- A new value means adding a property to `theme.slint` first. That costs one
  line, and a theme then covers it automatically.
- The scanner is a small hand-written check, not a Slint parser. If it ever
  flags something that isn't styling, narrow the scanner and add a case to its
  self-test rather than adding the value to `Theme` just to silence it.
