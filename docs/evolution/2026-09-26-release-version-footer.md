# Evolution: release-version-footer (every page says which release is serving)

**Shipped:** 2026-09-26, in v0.5.0. **Finalized:** 2026-10-04.
**Commits:** `0cd73c3` (step 01-01, the footer), `8f06528` (step 01-02, `chore(release): v0.5.0`),
`00dc174` (feature record and RELEASING.md). Lean path by user decision (2026-09-26): DISCUSS, then
DELIVER. There was no DESIGN or DISTILL wave and no ADR. Trunk-based, no PR.

## What shipped

Every full page, signed in or out, ends with a small muted `<footer class="site-footer">` that reads
`Foundry v<version>`. The version is `foundry-app`'s `CARGO_PKG_VERSION`, exposed as
`views::RELEASE_VERSION` and read by path from `base.html`, so no page struct changed. htmx fragments
never extend `base.html`, so they carry no footer. After the 100vh app shell, the footer is pulled up
by its own 16px into the shell's bottom padding (`.app-shell + .site-footer`), so no authed page,
the board included, gains a scrollbar. The stylesheet was re-hashed `438142d2` → `f9143163`
(base.html link, three `lib.rs` cache-policy test literals, VENDOR.md).

## Decisions (DISCUSS, locked)

- **D1:** one site-wide footer in `base.html`, including sign-in, so no account is needed to read it.
- **D2:** the text is the compiled crate version. The crates are bumped together, so it equals the tag
  without the build pipeline passing anything in.
- **D3:** muted, small and non-interactive, using existing `--cz-*` tokens only. It changes no page's
  layout or scroll, and the CSS change carries the D18 re-hash.
- **D4:** it ships as v0.5.0, together with `card-pointer-drag` slice 01.

## Work completed

| Step | Commit | Phases (execution-log) |
|---|---|---|
| 01-01 footer | `0cd73c3` | PREPARE, RED_ACCEPTANCE, GREEN, COMMIT: PASS. RED_UNIT: SKIPPED (no unit test; `env!` constant covered by `@rvf`) |
| 01-02 release chore | `8f06528` | PREPARE, GREEN, COMMIT: PASS. RED_ACCEPTANCE, RED_UNIT: SKIPPED (version bump and changelog only) |

Tests: `release-version-footer.feature` (`@rvf`) has 3 HTTP-lane scenarios: sign-in page, board page,
and new-issue modal fragment (no footer). The oracle reads the version from `foundry-app/Cargo.toml`
at test compile time. It does not use the production constant or a literal, so a release bump needs
no test edit. 0 new unit tests.

## Gates recorded

- `0cd73c3` message: lanes green are rvf, canzan-theme-system, pwa-mobile-rendering, cdf, cpd, kb and
  blr. check-arch, fmt, clippy `-D warnings` and xtask smoke also passed.
- Not recorded: scenario/step counts, a full `cargo xtask ci` run, a peer review, a mutation run
  (the project policy is per-feature), DES integrity, and whether smoke ran for 01-02. The roadmap was
  approved by the orchestrator on the lean path.

## How it is used

RELEASING.md "Verifying a deployment": after a tag push, the rollout is done when `/sign-in` on the
instance shows the new version. Until then the previous pod is still serving. (`00dc174` wrote
`/signin`; `c2980f1` corrected it on 2026-09-27.) On 2026-10-04 the footer read `Foundry v0.7.0` on
both https://foundry.unintelligent-design.us/sign-in (dev) and https://foundry.jeffbailey.us/sign-in
(prod). It was used to verify the v0.6.0 through v0.7.0 rollouts.

## Gaps

- No DESIGN or DISTILL section exists in `feature-delta.md`. The waves were skipped, and the user chose
  on 2026-10-04 not to back-fill them.
- The feature-delta preamble says "one acceptance scenario"; three shipped.
- Roadmap 01-01 says "htmx fragment and OOB responses carry no footer". Only the new-issue modal
  fragment is asserted, and no OOB response is tested.
- AC-3 (no regression) is evidenced by the seven lanes named above, not by a recorded full-suite run.

## Follow-ups

None recorded. The DISCUSS out-of-scope list (git SHA or build date, a version API endpoint, pipeline
changes) stands.
