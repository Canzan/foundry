# ADR-CANZAN-THEME-003: The static-asset integrity guard is a `cargo xtask check-arch` rule that derives its own input set

## Status

Accepted (canzan-theme-system DESIGN wave, 2026-08-29)

## Context

`docs/feature/htmx-web-tier/design/assets.md:83-97` chose content-hashed
filenames as the cache key ("the hash IS the cache key"), and accepted its one
failure mode — a forgotten rename — on the explicit promise of an
**asset-resolution probe** that "catches a forgotten rename by redding CI, so it
cannot silently ship a stale path" (`assets.md:93-94`). `assets.md:115-126`
specifies that probe in three steps: parse the base layout's `/static/...`
references, assert each resolves on disk, and optionally assert each blob's
sha256 matches `VENDOR.md` — with a gold test that renames a blob and asserts the
check goes red.

**It was never built.** `check-arch` has no asset rule
(`xtask/src/check_arch.rs:56-64` lists eight rules; none touches `static/`), and
no `check-assets` subcommand exists (`xtask/src/main.rs:27-40` dispatches only
`ci | smoke | check-arch | help`). The acceptance suite says so in its own
source: `crates/foundry-acceptance/src/steps/feature_b_web_tier.rs:964-965` —
*"(The previously-cited 'asset-resolution probe' xtask does not exist; this
filesystem check is the real contract.)"*. Two later features re-requested it
(`navigation-bar-linear-ui/design/component-boundaries.md:103`,
`.../adr-004-css-rehash-hand-maintained.md:33`) and it was still not built.

What exists today covers two of five sites and never touches the disk:

| Existing check | What it proves | What it misses |
|---|---|---|
| `feature_pwa_mobile.rs:266-306` | `base.html` and `lib.rs` name the **same** hash | never stats the file; both can name the same *wrong* hash |
| `lib.rs:312-373` cache-policy tests | the policy string for a path *literal* | the literal is hand-typed; a stale one still passes |
| `feature_b_web_tier.rs:951-987` | exactly one `htmx*.js` under `static/vendor/` | one blob only |

The hash literals sit in **three** places in `crates/foundry-app/src/lib.rs`
(329, 346, 365) plus `templates/base.html:6` plus the `VENDOR.md` row — five
hand-maintained sites. This feature re-hashes the stylesheet **three times**
across four slices and adds font and `theme.js` assets, multiplying the literals.
`feature-delta.md` Unresolved #2 records the gap as an accepted risk. This ADR
declines to accept it.

Quality drivers, in the order DISCUSS established: accessibility, testability,
maintainability, operational simplicity. Team of one. No build step, no Node.

## Decision

Add **one function, three assertions** to `xtask/src/check_arch.rs`, following
the existing `fn(&Path) -> Vec<String>` rule shape and appended to the same
`violations` vec at `check_arch.rs:56-64`:

```
check_static_asset_integrity(&root) -> Vec<String>
```

**R1 — every reference resolves.** Scan `crates/foundry-app/templates/**/*.html`,
`crates/foundry-app/src/**/*.rs`, `crates/foundry-app/static/**/*.css` and
`crates/foundry-app/static/manifest.webmanifest` for the token
`/static/<path-chars>`; for each extracted path that carries a file extension,
assert the corresponding file exists under `crates/foundry-app/static/`. Name the
referencing `file:line` and the dangling path on failure.

**R2 — every content-hashed filename is honest.** For every file under
`crates/foundry-app/static/` whose name matches `<stem>.<8 lowercase hex>.<ext>`,
recompute the file's sha256 and assert the first 8 hex digits equal the filename
segment. This is the assertion R1 cannot make: R1 catches *renamed but not
referenced*; R2 catches *edited but not renamed* — a file that still resolves at
its old immutable URL and is therefore pinned stale in every browser for a year.

**R3 — every `VENDOR.md` row is true.** Parse the provenance table in
`crates/foundry-app/static/VENDOR.md`; for each row, assert the named file exists
and that its recomputed sha256 equals the recorded value.

The scan set is **derived, never enumerated**. There is no list of assets to
maintain, so the guard cannot itself go stale: adding a font blob and referencing
it from `@font-face` enrols it automatically.

Four scoping rules, each forced by evidence in the tree:

1. **Scope is `crates/foundry-app/` only.** `crates/foundry-acceptance/` is
   excluded because it deliberately contains non-resolving `/static/` literals as
   negative fixtures — `feature_b_web_tier.rs:486`
   (`/static/css/does-not-exist.css`, expects 404) and `:495`
   (`/static/../Cargo.toml`, path-traversal probe). Scanning it would make the
   guard permanently red.
2. **`#[cfg(test)]` blocks are NOT region-skipped.** This is a deliberate
   departure from `check_no_static_lane_list`'s posture
   (`check_arch.rs:602`): the three stale-hash literals this rule exists to
   protect live *inside* `#[cfg(test)] mod static_cache_policy_tests`
   (`lib.rs:312-373`). A rule that skipped test blocks would miss its entire
   reason for existing.
3. **An extracted path must carry a file extension to be checked.**
   `projects.rs:1048` holds the deliberate *prefix* literal
   `href="/static/css/foundry.` — hash-agnostic by design. Requiring a trailing
   `.<2-5 alphanumerics>` skips prefixes and format templates without an
   allowlist. Documented limit: a typo that also drops the extension is not
   caught by R1 (it is still caught by nothing else — accepted).
4. **`@font-face src` must use absolute `/static/...` URLs.** One uniform matcher
   then covers every reference site in the repo and the rule needs no
   relative-path resolution. This is an authoring constraint on the stylesheet,
   recorded here because the guard depends on it.

Implementation note: `rust_sources` (`check_arch.rs:813-831`) enumerates `*.rs`
only. R1 must also walk `.html`, `.css` and `.webmanifest`, so it needs a
generalised extension-filtered walker rather than a call to that helper. R2 and
R3 need sha256; `xtask` currently depends only on `anyhow`
(`xtask/Cargo.toml:16-17`), so it gains `sha2 = { workspace = true }` — `sha2
0.10` is already a workspace dependency (`Cargo.toml:63`), so this is a new edge
to a crate already in the graph and `cargo deny check bans` is unaffected.

**Gold test (Principle 12c self-application).** The rule is verified by the
injected-violation pattern already established at
`feature_a_programmatic.rs:1456-1702`: copy the tree excluding `target` and
`.git`, plant a violation, run the built `xtask` binary with `--root <copy>`,
assert non-zero exit **and** that the output names the offender — the stronger
two-tier convention used at `feature_a_programmatic.rs:1050-1063`. Two arms: one
`std::fs::rename`s `static/css/foundry.8ce38566.css` to a different hash (R1 must
fire and name `base.html` and the dangling path); one appends a byte to the CSS
without renaming it (R2 must fire and name the file).

**The gold tests are `xtask` unit tests, not acceptance scenarios.** They live in
the `#[cfg(test)]` module beside `check_arch.rs` (which already exists at
`check_arch.rs:864` and already unit-tests the AST detectors against staged
fixture trees — `xtask/Cargo.toml:19-21` carries `tempfile` for exactly this).
Each test builds a temp tree, plants one violation, calls the **rule function
directly**, and asserts it fires and names the offending `file:line`; a paired
test asserts it stays silent on a clean tree.

**Five gold tests — one per rule, no exceptions:**

| Rule | Planted violation | Must name |
|---|---|---|
| R1 | rename `foundry.8ce38566.css` to a different hash | `base.html` and the dangling path |
| R2 | append a byte without renaming | the file whose hash no longer matches its name |
| **R3** | **edit a `VENDOR.md` row to carry a wrong sha256** | **the offending row and its file** |
| S1 | plant `color: #ff0000` in a rule below the seam | the file and line |
| S2 | delete one token from the media block only | the token and the region |

R3's test was missing from an earlier draft of this ADR — a real defect, since R3
is the rule that machine-checks ADR-CANZAN-THEME-002's provenance model. Without
it, a bug in row parsing or a wrong hash algorithm would ship undetected and the
load-bearing integrity claim of the whole font strategy would rest on an
unverified assertion. It is listed first among equals here so the omission cannot
recur.

This corrects an earlier placement in acceptance. `check-arch` has **no driving
port** — it is a pure function of a directory — so a cucumber scenario was always
the wrong shape: it would have driven a subprocess through a BDD harness to test
something with no user, no HTTP surface and no browser. Calling the rule function
directly is both simpler and a stronger assertion, because it can inspect the
returned `Vec<String>` rather than grepping stdout.

## Consequences

- Positive: `assets.md`'s eight-month-old promise is kept. The five-site re-hash
  discipline becomes mechanically enforced instead of "manual discipline backed
  only by red cache-policy tests" (feature-delta System Constraints). The guard
  runs in `cargo xtask ci` gate 3 (`main.rs:191-195`) **and** in `run_smoke`
  (`main.rs:270-308`), so it bites on the fast pre-commit loop, not only in CI.
  The three re-hashes this feature performs are the first beneficiaries. R2 makes
  `Cache-Control: immutable` honest by construction rather than by promise.
- Positive: the scan set is derived, so the guard's coverage grows with the repo.
  It immediately covers three assets nothing tests today —
  `/static/icons/*.png`, `/static/js/form-errors.js`, `/static/js/keyboard.js`.
- Negative: `check-arch`'s module doc (`check_arch.rs:1-39`) frames it as "the
  US-W06 web/api boundary guard", and asset resolution is not a web/api boundary.
  The name has already drifted (`check_app_no_slugify_definition`,
  `check_no_static_lane_list`, `check_app_tenant_scoping` are not web/api
  boundaries either); this ADR accepts the drift and requires the module doc be
  rewritten to describe what the binary actually is — the build-time invariant
  scanner — rather than renaming a command referenced by `ci`, `smoke`, README
  and the acceptance harness for zero functional gain.
- Negative: R3 checks that rows are *true*, not that they are *complete*. Nine
  assets currently ship with no `VENDOR.md` row (`js/*.js` ×4, `icons/*.png` ×4,
  `manifest.webmanifest`). A completeness assertion would force this feature to
  back-fill provenance for assets it does not own. Deferred, named below.
- Negative, and worth stating precisely: **R1 proves the reference resolves in the
  source tree, not in the deployed image.** `static_dir()` (`lib.rs:248-254`)
  resolves cwd-relative at runtime, falling back to `CARGO_MANIFEST_DIR/static`,
  and the Dockerfile `COPY`s `static/` in (assets.md:130-132) — so a build that
  dropped the directory would pass R1 and 404 every asset. That failure has
  happened here before and already has its own guard: `lib.rs:335-372` exists
  because "a deploy whose image omitted the static/ directory 404'd every asset"
  and a CDN pinned those 404s for a year at content-addressed URLs. R1 answers
  *"does the repo reference a file it contains?"*; the acceptance HTTP GETs answer
  *"does the running server serve it?"*; the 404 policy test answers *"and if it
  does not, is the failure at least not cached?"* Three questions, three layers,
  none subsuming another.
- Negative: R2 recomputes sha256 for every file under `static/` on every
  `check-arch` run, including the pre-commit `smoke` loop. At the current ~1.5 MB
  of static assets this is sub-10 ms; if `static/` ever grows to tens of
  megabytes the rule needs a size guard. Recorded, not pre-optimised.
- Deferred, each with its reason: **orphan-blob detection** ("every file under
  `static/` is referenced at least once" — the `.site-header` defect class
  applied to assets) is deferred because it requires an exemption list
  (`VENDOR.md`, `favicon.ico`, anything a browser requests without a reference),
  and a hand-maintained exemption list is exactly the staleness this rule exists
  to eliminate. **Row completeness** is deferred pending the nine back-fill rows.
  **A font byte-budget assertion** is deferred because KPI 7 measures the cold
  board load (CSS + htmx + fonts), which is an acceptance-lane fact, not a
  tree-scan fact; it belongs in US-CTS-03's lane where DISCUSS put it.
- Enforcement of the enforcement: without the gold test this rule is a claim.
  With it, the rule is verified to bite. The gold tests are **not optional and
  not deferrable**, and because they no longer live in the acceptance suite they
  are a **DELIVER obligation carried by this ADR** rather than by Quinn's DISTILL
  scope — see the handoff in `architecture-design.md` §9. **Five rules, five gold
  tests**, landing in the same commit as the rules they verify. `brief.md` asserts
  these guards "are shown to bite rather than assumed to"; an unwritten test would
  make that assertion false for its rule.
- **Placement correction, recorded rather than silently applied.** This ADR
  originally put the gold tests in new acceptance files. DISTILL Decision 4 ruled
  infrastructure out of acceptance scope, which correctly left them unwritten and
  incorrectly left five new rules with no verification owner. Moving them to
  `xtask` unit tests is **not a reduction in coverage** — the same
  injected-violation tests, in a better-suited lane. It is a net improvement on
  three counts: they run in the fast unit lane (`cargo test -p xtask`) instead of
  requiring Postgres and chromedriver; they assert on the rule's returned
  violations rather than on subprocess stdout; and they remove the ~40 lines of
  duplicated tree-copy harness the acceptance placement would have needed. The
  third of `boundary-guard.md`'s three orthogonal layers
  (`docs/feature/web-tier-extraction/design/boundary-guard.md:74-79`) is
  preserved in substance; only its lane moves.

## Alternatives considered

- **A. A `#[test]` in `foundry-app`, as `assets.md:118` offers as an option** —
  Rejected. Decisive objection: it cannot be pointed at a planted-violation tree,
  so the guard itself becomes unverifiable and `assets.md:125-126`'s own gold-test
  requirement ("rename a blob and assert the check goes red") is unsatisfiable.
  Secondary: reaching `templates/` and `static/`'s CSS from a unit test puts a
  repo-layout concern inside the application crate, and it would run in gate 5
  rather than gate 3, after clippy and a full release build.
- **B. A separate `xtask check-assets` subcommand, as `assets.md:117` literally
  specifies** — Rejected, and this ADR supersedes that wording. It would need its
  own `--root` plumbing, its own wiring into both `run_ci` and `run_smoke`, its
  own PASS line and its own gold-test harness — roughly triple the wiring for a
  naming preference. `boundary-guard.md:117` already anticipates the alternative:
  *"if more rules are needed later, `check-arch` grows a rule list."* The rule
  list is the house pattern; a second command is not.
- **C. A `build.rs` assertion in `foundry-app`** — Rejected. foundry has no build
  script today, so this introduces a new mechanism where an existing one fits.
  Build-script results are cached and skipped on unchanged inputs — precisely the
  case where a stale *reference* to an unchanged file must still fail — and a
  build-script panic is a poor diagnostic surface compared to check-arch's
  `- {violation}` list.
- **D. An acceptance test that HTTP-GETs every referenced asset against the live
  server** — Rejected as the *primary* guard, though it proves a strictly stronger
  thing (the running `ServeDir` actually serves the byte). It runs in gate 8,
  behind Docker availability (`main.rs:241-252`), so a broken asset survives the
  entire fast loop; and it can only test paths a step file knows to ask for,
  which reintroduces the hand-maintained list. Retained as a complement: the
  existing per-asset GETs stay, and R1 now guarantees the set they sample from
  is non-dangling.
- **E. Accept the risk, as feature-delta Unresolved #2 proposes** — Rejected. The
  risk was accepted once in `assets.md`, on the strength of a probe that was then
  not built, and has been re-requested by two subsequent features. Three more
  re-hashes land in this feature. The cost of the rule is ~90 lines of Rust in a
  file that already contains eight such rules.
