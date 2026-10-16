# 003: Slint licence

Status: accepted · 2026-10-15

## Context

Couchfin is MIT. Slint, the UI toolkit the plan picks, is licensed
`GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR
LicenseRef-Slint-Software-3.0`. `deny.toml` allows only permissive licences, so
adding Slint needs a deliberate choice between the three.

- **GPL-3.0-only** would make the distributed Couchfin binary GPL, which
  conflicts with shipping it as MIT.
- **Slint Software 3.0** is the paid commercial licence.
- **Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0** is
  free for desktop applications, with conditions.

## Decision

Use Slint under the **royalty-free licence 2.0**, and never under GPL.

`deny.toml` allows `LicenseRef-Slint-Royalty-free-2.0` only for Slint's own
crates, listed one by one in `[licenses] exceptions`. The global allow list is
unchanged, so no other crate can bring in Slint's licence, and GPL stays
rejected everywhere. A new Slint crate (for example after turning on another
renderer) fails `cargo deny check` until it's added to the list.

The other Slint features added no new licences. To keep the licence check to
what Couchfin ships, `deny.toml` now limits the dependency graph to
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. Without that, the
Windows-only `clipboard-win` and `error-code` crates (BSL-1.0) and Apple-only
crates were checked too.

## Obligations

The licence (shipped in each Slint crate's `LICENSES/` folder) lets us
distribute Slint inside an application on these conditions:

1. **Attribution, one of:**
   - (a) show Slint's `AboutSlint` widget on an "About" screen reachable from
     the app's top-level menu, or on its splash screen if there is no About
     screen; or
   - (b) show the "Made with Slint" badge on a public web page, preferably
     where the binaries are downloaded (for us, the GitHub Releases page or
     the README).

   We use **(b)**. The repo is public, so the badge sits at the top of the
   README, and `release.yml` appends `.github/release-footer.md` to every
   GitHub Release's notes, which is where the binaries are downloaded. Releases can be cut from
   Phase 0 onwards (`release.yml`), so this is in place before the first one.
   `app/tests/slint_attribution.rs` fails if either badge is removed. If
   Couchfin is ever distributed somewhere else (Flathub, for example), that
   page needs the badge too, or the app needs option (a).
2. **Desktop applications only.** The licence excludes "Embedded Systems", meaning a
   computer system built to do one task inside a larger mechanical or
   electrical system. A Bazzite/SteamOS PC or a Steam Deck is a general-purpose
   computer running a desktop OS, so Couchfin counts as a desktop
   application. If Couchfin ever targets a dedicated appliance, look at this
   again.
3. **Don't expose Slint's API.** The licence forbids distributing an application
   that exposes Slint's APIs, in part or in total. A future client-side
   extension system must not hand Slint types or `.slint` loading to
   extensions without revisiting this.
4. **Keep Slint's licence notices.** We may not remove licence or copyright
   notices from Slint's source. We vendor none of it, so this only matters if
   that changes.
5. Slint itself can't be redistributed on its own under this licence, only as
   part of Couchfin.

## Consequences

- The Couchfin source stays MIT. The distributed binary includes Slint under
  its royalty-free licence, and the release must carry the attribution above.
- Phase 6 (packaging) has a new task: include Slint's licence text with the
  Flatpak, and give the Flathub listing the badge or switch to option (a).
- If any condition becomes a problem, the choices are a paid Slint licence or
  another toolkit. Moving to GPL would mean relicensing Couchfin.
