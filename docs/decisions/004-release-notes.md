# 004: CI jobs, coverage gate and release notes

Status: accepted · 2026-10-15

## Context

The plan asks for every check to run on each PR in the dev container image,
for CI to fail if coverage of `couchfin-api` or `couchfin-core` drops below
80%, and for a `v*` tag to publish a GitHub Release with notes generated from
the Conventional Commits. PRs are merged with merge commits, so history holds
both the PR's own commits and a "Merge pull request #N" commit for each PR.

## Decision

- **One job per check** (`fmt`, `clippy`, `test`, `doc`, `deny`, `coverage`),
  each in `ghcr.io/nkahlor/couchfin-dev:latest`. They run in parallel, a
  failure names the check that broke, and branch protection can require them
  by name. The image is private, so each job pulls it with `GITHUB_TOKEN` and
  `packages: read`. `Swatinem/rust-cache` caches `~/.cargo` and `target/`;
  only `main` saves the cache, and PRs restore it.
- **Per-crate coverage gate.** `cargo llvm-cov --fail-under-lines` only checks
  the workspace total, so `.github/scripts/coverage_gate.py` reads the JSON
  summary and checks each crate's `src/` separately. A crate with no
  executable lines yet is **skipped with a warning annotation**: 0 of 0 lines
  is neither 0% nor 100%, and failing would block every PR until the crate has
  code. Once it has a single executable line, the gate applies. A crate name
  that doesn't exist fails, so a typo can't switch the gate off.
- **git-cliff for release notes**, pinned (2.14.2) and configured in
  `cliff.toml`. GitHub's built-in generated notes list PRs grouped by label,
  and we don't label PRs. git-cliff parses commit types and groups the notes by
  them (Features, Bug fixes, ...). It drops every non-conventional commit,
  which removes the merge commits while keeping the PR commits they bring in.
  Each entry ends in a short SHA, which GitHub links to the commit.
- **Explicit tag ranges.** git-cliff's `--latest` and `--current` take the
  range between the two newest tags and ignore `ignore_tags`. The release
  workflow finds the previous tag with `git describe` and passes the range:
  a final release starts at the previous final release (so the notes for
  `v0.2.0` also cover what `v0.2.0-rc.1` shipped), a pre-release at the
  previous tag of any kind.
- **Releases re-run CI.** `release.yml` calls `ci.yml` (`workflow_call`), so a
  tag on a commit that fails the checks publishes nothing. A final tag must
  equal `v` plus the `app` version in Cargo; a tag with a hyphen is published
  as a pre-release and skips that check.

## Consequences

- CI uses the image's `latest` tag, so a Containerfile change only reaches CI
  after it merges and `devcontainer.yml` pushes it. A PR that needs a new tool
  in CI has to land the image change first.
- The release binary links against the dev image's Fedora 44 libraries
  (glibc, libmpv, SDL3 since decision 010), so it runs on Fedora 44 or newer, such as current
  Bazzite. The Phase 6 Flatpak replaces it as the way to install Couchfin.
- Commits that aren't Conventional Commits don't appear in the release notes.
- git-cliff is installed at runtime in the `publish` job, the one exception to
  "nothing installed at runtime", because only releases need it.

## Update (2026-10-16)

- The repo and the dev image are now public. The jobs still pull with
  `GITHUB_TOKEN` and `packages: read`, which costs nothing and keeps CI working
  if the package is ever made private again.
- Merging is now squash-only, with the PR title as the commit title, so each PR
  lands as one Conventional Commit. The merge commits described in the context
  above exist only in the history before this change. Dropping non-conventional
  commits still handles them.
