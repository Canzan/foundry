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

## 2026-10-04 increment: build stamp (US-RVF-02)

**Delivered:** 2026-10-04. **Ships in:** v0.8.0, tagged after push. Prod is held for approval by
design. Full wave run: DISCUSS + DESIGN, then DISTILL with a DESIGN revision, then DELIVER.
Record: `docs/feature/release-version-footer/feature-delta.md` (the 2026-10-04 sections) and
`deliver/ac7-demo.md`. This increment narrows the "Git SHA or build date" out-of-scope line above.
A version API endpoint stays out.

### Summary

`Foundry v0.7.0` named a release, not a build. Every `:main` image between two tags read the same,
and so did a stale image. The footer is now
`<footer class="site-footer" data-commit="<sha7>">Foundry v<version> · <commit date></footer>`.

- A new `crates/foundry-app/build.rs` bakes the SHA and the commit date. For each field it uses a
  non-blank `FOUNDRY_STAMP_*` input, else git, else `unknown`. It never fails the build and adds no
  crates.
- Its rerun directives keep the stamp fresh across commits, detached HEADs, packed refs, worktrees
  and env changes.
- The container has no `.git`, so both publish workflows compute the stamp, refuse to publish an
  empty one, and pass it in as Docker build-args.
- A check-arch rule (`publish-stamp:`) holds both workflows and the Dockerfile to that shape.
- RELEASING.md now verifies a rollout by the date and `data-commit`, not just the version.

### Commits

| Commit | What |
|---|---|
| `99ad7a9` | DISCUSS + DESIGN (D5-D14, AC-4..AC-10, DDD-1..17) |
| `ef7dc39` | DISTILL (amended `@rvf`, 11 scaffolds) + DESIGN revision (DDD-6, -8, -13, -14) |
| `c7568bd` | DELIVER roadmap phase 02 |
| `00f02a5` | 02-01: `build.rs`, `views::SiteFooter` / `site_footer()`, footer partial, `base.html`, 8 unit tests, `@rvf` live, AC-7 demo |
| `11b757a` | 02-02: Dockerfile ARGs, both publish workflows, `publish-stamp` check-arch rule, RELEASING.md, docker demo |
| `3f3897a` | Single-fault fixtures for the two surviving rule faults |

### Gates

- Lanes: rvf 3/3, canzan-theme-system 28/28, pwa-mobile-rendering 14/14, blr 26/26, cdf 54/54.
- Unit: foundry-app `build_stamp_tests` 8/8; xtask 46/46. Static: check-arch, fmt, clippy
  `-D warnings`.
- AC-7 demo rows 1-8 PASS (scratch clone). `docker build` without build-args served `unknown`, and
  with them served the given values, from a running image.
- Peer review: **APPROVED**, zero defects (nw-software-crafter-reviewer, 2026-10-04)
- Full CI: **GREEN** on `3f3897a`: exit 0, all gates, 911/911 scenarios and 6341/6341 steps, browser lane run (2026-10-04)

### Faults

- **02-01: 8/8 killed.** Build-time date, SHA in text or missing `data-commit`, blank treated as
  given, git failure panics, `vunknown`, trailing separator, unescaped field, missing refs-dir
  directive.
- **02-02: 4/4 killed, after `3f3897a`.** At `11b757a`, two rule faults survived: the
  build-time-date check removed and the runtime-stage-ARG check removed. They were killed once each
  had a fixture of its own.

### Lessons

- **Watching only the files that exist misses the file that appears.** The design review found
  that after `git pack-refs`, or in a fresh clone, the branch's loose ref is absent. Nothing
  watches it, so the next commit creates it and changes no watched path, and the stamp goes stale.
  The fix is to watch the nearest existing parent directory. canzan-lift's `build.rs` had the same
  gap. It was fixed there as `c30ec4a4` and released as canzan-lift v0.4.2 (2026-10-04).
- **A bug that needs a sequence needs that sequence in the demo.** The gap reproduces only with
  pack → build → commit → build. An ordinary commit on a branch with a loose ref re-stamps, so a
  demo of "commit, rebuild" alone would have passed with the bug present.
- **A fixture that changes two things at once lets a fault survive.** Each DISTILL fixture for the
  publish rule broke two checks, so deleting either check still left the test red. One fault per
  fixture, each asserting its own `file:line` message, is what made the rule's tests able to kill
  faults.
- **A lane oracle can be blind on the day of the commit.** A build-time date equals the commit
  date on the commit day, so `@rvf` cannot see that fault then. It was killed by a past-dated
  commit in the demo instead.
- **A rule that cannot be unit-tested can still gate a step.** Cargo's rerun directives are only
  observable by building. Making the recorded AC-7 demo a COMMIT precondition is what caught and
  proved the refs-dir fix.

### Follow-ups

- **Operator, first Forgejo push (OQ-D4).** Confirm the `build-and-publish` runner has `git`. If it
  does not, the empty-stamp guard fails the job by design.
- **Operator, first Forgejo CI run (DDD-14).** Record whether checkout had `.git`. Probably not
  (`rust:1.85-slim`), in which case both build.rs and the oracle read `unknown`, consistently.
- **Operator, after v0.8.0 is approved (AC-8).** Check that the prod `/sign-in` footer's
  `data-commit` and date equal `git log -1 --format='%h %cd' --date=short v0.8.0`, and record it in
  the feature-delta DELIVER section. An unchanged prod footer before approval is expected.
- **v0.8.0 release chore.** `CHANGELOG.md` `[Unreleased]` has no build-stamp entry yet.
- **Nit.** The `views::RELEASE_VERSION` doc comment still says `base.html` reads it by path; it is
  now read through `site_footer()`.
