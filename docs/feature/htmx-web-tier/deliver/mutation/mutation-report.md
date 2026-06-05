# Mutation Report — Feature B (`htmx-web-tier`), DELIVER Phase 5

- **Tool**: `cargo-mutants 25.3.1`
- **Date**: 2026-06-04
- **Scope**: feature-scoped to the genuinely-mutatable Rust logic added/changed by Feature B.
- **Test lane**: fast in-process `cargo test -p foundry-app --lib` (via `--cargo-arg --lib`).
  The slow `@real-io` testcontainer acceptance lane was deliberately NOT invoked
  (avoids the known false-survivor caveat where the `foundry` binary must be rebuilt;
  see `memory/cargo-mutants-realio-subprocess.md`).
- **Gate**: ≥80% kill rate on the scoped logic, with the security-adjacent
  **affordance predicate** as the centerpiece.

## Why scoped

Feature B is overwhelmingly Askama `.html` templates + vendored asset blobs, which
`cargo-mutants` does not mutate. The mutatable Rust logic is thin and lives in
`crates/foundry-app/src/`. We scope to the pure builders/predicates that have FAST
`--lib` unit coverage and HONESTLY exclude the surfaces whose only coverage is the
acceptance lane (see "Excluded from this pass").

## Results per target

### PRIMARY — `comments.rs::build_comment_card` (affordance predicate, ADR-006/007)

The security-adjacent core: `can_edit = is_author` (author-only edit), and
`can_delete = is_author || actor_is_admin` (author-or-admin delete). A mutation
flipping `can_delete` to always-true, or `can_edit` away from author-only, is a real
authorisation bug.

| Metric | Before | After |
|---|---|---|
| Mutants found | 3 | 3 |
| Unviable (don't compile) | 1 (`-> Default::default()`) | 1 |
| Viable | 2 | 2 |
| Caught | 0 | **2** |
| Missed | 2 | 0 |
| **Kill rate (viable)** | **0%** | **100%** |

Survivors fixed (real gaps — there were NO `--lib` tests for `build_comment_card` at all):

| Mutant | Verdict |
|---|---|
| `744:35 replace == with != in build_comment_card` | real-gap-now-killed |
| `751:31 replace \|\| with && in build_comment_card` | real-gap-now-killed |

**≥80% gate on the affordance predicate: MET (100%).**

### SECONDARY — `projects.rs` board builder + helpers

Scope: `build_board_page`, `column_label_to_state`, `issue_card`, `issue_key_string`,
`slugify` — the board view-model builders. Run with `--cargo-arg --lib`.

| Metric | Before | After |
|---|---|---|
| Mutants found | 14 | 14 |
| Unviable | 2 (`build_board_page`/`issue_card -> Default::default()`) | 2 |
| Viable | 12 | 12 |
| Caught | 5 | **12** |
| Missed | 7 | 0 |
| **Kill rate (viable)** | **42%** | **100%** |

Survivors fixed (one new test pins the whole state→column mapping + the filter):

| Mutant | Verdict |
|---|---|
| `580:37 replace == with != in build_board_page` (issue-state filter) | real-gap-now-killed |
| `631:5 column_label_to_state -> ""` | real-gap-now-killed |
| `631:5 column_label_to_state -> "xyzzy"` | real-gap-now-killed |
| `632:9 delete match arm "Backlog"` | real-gap-now-killed |
| `633:9 delete match arm "Todo"` | real-gap-now-killed |
| `634:9 delete match arm "In-Progress"` | real-gap-now-killed |
| `635:9 delete match arm "Done"` | real-gap-now-killed |

The pre-existing `populated_board_renders...` test only placed issues in `backlog`
and `in_progress`, so the `Todo`/`Done` mapping arms and the filter direction were
unconstrained.

## Aggregate scoped kill rate

| Surface | Viable | Caught | Kill rate |
|---|---|---|---|
| `build_comment_card` (affordance predicate) | 2 | 2 | 100% |
| `projects.rs` board builder + helpers | 12 | 12 | 100% |
| **Total scoped** | **14** | **14** | **100%** |

## Tests added (test-first; each confirmed RED→GREEN against real production code)

- `crates/foundry-app/src/comments.rs` →
  `affordance_tests::affordances_follow_author_and_admin_rules` — table-driven over the
  full author × admin matrix (6 cases). Pins ADR-006 (edit author-only) and ADR-007
  (delete author-or-admin), including the admin-moderator (`edit=F, delete=T`) and
  anonymous (`edit=F, delete=F`) rows that kill the two predicate mutants. Port-to-port
  at domain scope: `build_comment_card`'s signature IS the affordance-decision port.
- `crates/foundry-app/src/projects.rs` →
  `board_render_tests::each_issue_lands_in_exactly_its_state_column` — one issue per
  state; asserts each card appears under its OWN `data-column` region and NOT under the
  other three (regions bounded before the all-keys `#kb-items` carrier). Kills the
  state→column arm mutations and the `==`→`!=` filter flip in one assertion.

No production code was changed; only test additions. Tests assert observable outcomes
(view-model fields / rendered selector placement), not internal structure.

## EXCLUDED FROM THIS PASS (coverage honesty)

These surfaces were NOT killed in this pass; excluding them avoids reporting false
survivors. Each is named with the reason:

1. **Askama `.html` templates + vendored `/static` asset blobs** — the bulk of Feature B.
   `cargo-mutants` does not mutate non-Rust files; there is nothing to mutate. Their
   rendered-selector contract is covered by `board_render_tests` (substring/selector
   assertions) and the acceptance suite.

2. **`projects.rs::render_500` (`549:5 -> Default::default()`)** — CONFIRMED MISSED under
   `--lib` (probed directly). It is the render-failure→500 mapping (US-B01 `@error`,
   error-and-observability.md §"Render-error handling"), observable only through a live
   `AppState` HTTP response. Its only kill comes from the `@real-io` acceptance scenario.
   Reporting it as a survivor in the `--lib` lane would be a FALSE survivor per the
   known cargo-mutants caveat (binary must be rebuilt for the acceptance lane to count).
   **Excluded: acceptance-covered + false-survivor/runtime-cost.**

3. **`projects.rs::render_board` Ok/Err string mutants + the
   `force_board_render_failure` test-injection Err arm** — same reason: the Err arm is
   reachable only with a live `AppState` flag flip exercised by the US-B01 `@error`
   acceptance scenario, not by `--lib`.

4. **HTTP handler bodies** (`show_issue`, `submit_comment`, `submit_edit_comment`,
   `submit_delete_comment`, `show_single_comment`, `show_edit_form`, `show_board`,
   `submit_create`, etc.) and their authz/route wiring — covered by the `@real-io`
   testcontainer acceptance suite, NOT by `--lib`. Mutating them under `--lib` reports
   false SURVIVED; mutating them under the acceptance lane hits the rebuild caveat and
   is slow. **Excluded: acceptance-covered, by design.** Note the
   `submit_delete_comment` authz predicate (`416/417`, author-or-admin) mirrors the
   `build_comment_card` predicate now pinned at 100% in the `--lib` lane; the handler
   itself remains acceptance-verified.

## Post-run safety

- `git status`: only the two intended test additions (`comments.rs`, `projects.rs`) +
  this report. (`docs/.../execution-log.json` carried a pre-existing working-tree change
  not authored by this pass; the DES hook blocks direct edits to it.)
- `cargo build -p foundry-app`: green.
- `cargo test -p foundry-app --lib`: 10 passed, 0 failed.
- `cargo fmt -p foundry-app --check`: clean. `cargo clippy -p foundry-app --lib`: clean.
