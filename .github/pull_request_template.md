<!--
PR title: Conventional Commits, e.g. `feat(api): authenticate by name`,
`fix(ui): keep focus on back`. The title becomes the squash commit message and
the release notes entry. One task per PR. See CLAUDE.md, "Before opening a PR".
-->

## Task

<!-- The plan task this PR covers. Use "Closes #N" when the PR finishes the task, "Part of #N" when it doesn't. -->

Closes #

Crate(s) touched:

<!-- If you changed another crate's public API, explain why here. -->

## What changed

## Red test output

<!-- The failing test run from before the fix (red-green-refactor), so reviewers can see the test catches the bug. -->

```text

```

## Docs

- [ ] Docs updated
- [ ] No docs affected

Comments and docs I re-read (doc comments, README, `docs/`, CLAUDE.md):

-

## Checklist

- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo llvm-cov`, `cargo deny check` and `cargo doc` all pass locally
- [ ] Every change has tests, and the red test output is above
- [ ] Commit messages and the PR title follow Conventional Commits
- [ ] The PR is small and covers one task
- [ ] Every new UI action is reachable by controller (or this PR has no UI changes)
- [ ] UI changes: `layout_checks` pass, and I opened the contact sheets for the screens I touched once, checked them against the CLAUDE.md list (overlap or clipping, alignment, focus, contrast, 720p, images fill slots) and list what I checked below (or this PR has no UI changes)

Visual check (UI PRs): what I checked, what changed, CI artifact `screenshots-<sha>`:

-

## "Done when" check for the TV

<!-- Which "Done when" check from docs/PLAN.md the human still needs to run on the TV, and how. Write "None" and say why if there isn't one. -->
