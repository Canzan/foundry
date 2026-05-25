# Wave Decisions — comment-edit-delete (Slice 5)

DISTILL-wave decisions that gate DELIVER. Finalized 2026-05-25 from the
staged DISTILL pass after user landed picks on D1–D5 from
`proposals.md`. Slice 5 inherits slice-1/2/3/4 patterns verbatim per
the project's Architecture of Reference at
`docs/architecture/atdd-infrastructure-policy.md` and adds only the
deltas listed below.

## Strategy: C (all real adapters) — inherited

Slice 5 inherits Strategy C from slices 1–4 per
`docs/architecture/atdd-infrastructure-policy.md` (mode = `inherit`).
**No new policy rows needed** — DESIGN wave-decisions.md § Reuse
Analysis records ZERO new ports. Every adapter the slice-5 scenarios
exercise was already recorded by slice 1/2/3:

- HTTP API driving port — `reqwest::Client` against `spawn_app` (policy "Driving" row 1)
- Real Postgres per-scenario schema rotation — `PgPool` against the shared `testcontainers-rs` Postgres-16 container (policy "Driven internal" row 1)
- Real `pg_notify` / LISTEN / `tokio::sync::broadcast` SSE fan-out (same `PgPool`)
- Real `pulldown-cmark` + `ammonia` sanitization (pure function in `foundry-core/src/markdown.rs`; no adapter abstraction, no policy row)
- Real `tower-sessions` PG store + CSRF middleware (policy "Driven internal" row 2)

SSE consumer (`support/sse_client.rs`) and HTML scraper helpers
(`support/html_assertions.rs`) are reused verbatim from slice 2. **No
new support files in slice 5.**

## Tier composition: Tier A only — Mandate 10 condition not met

10 automated scenarios, **none chained** (each opens a fresh
`spawn_app` + workspace seed; every scenario re-posts its own
Mei-authored comment as a precondition rather than reusing prior
scenario state). No domain-rich input space. Mandate 10's ≥3-chained +
domain-rich threshold is not crossed. **Tier B is NOT emitted.**

## PBT input mode: example-only — Mandate 9 layer constraint

All 10 scenarios run at layer 3+ (real HTTP, real Postgres, real SSE).
Per Mandate 9, layer 3+ tests are example-only. No PBT decorators, no
generated inputs. Sad paths (non-author 403, two 410 tombstone
scenarios) are named examples per Mandate 11.

## ADR-style decision table (D1–D5 finalized)

### D1 — NFR tag set for slice-5 scenarios

| Option | Status | Rationale |
|---|---|---|
| **A. Reuse existing `@nfr-perf-03` / `@nfr-sec-05` / `@nfr-sec-06`** | **CHOSEN** | `@nfr-perf-03` already represents the 1s p99 fan-out SLA — the new `CommentEdited` / `CommentDeleted` realtime scenarios are the same NFR cell, not a new one. `@nfr-sec-05` covers "sanitizer must hold" and the edit re-render inherits that contract by re-running `render_comment_markdown` per ADR-007. `@nfr-sec-06` covers "authorization at every endpoint" and the non-author 403 + admin-only delete scenarios are exactly that surface. The "edited indicator render correctness" is a functional contract (positive assertion inside the WS), not an NFR. |
| B. Add `@nfr-ui-01` (edited indicator) | DEFERRED | New NFR row for a render-correctness contract; per slice-2 precedent NFR tags are reserved for cross-cutting quality attributes. Reviewable post-MVP if regression data justifies. |
| C. Add `@nfr-sec-07` (admin authority) | DEFERRED | Pin admin-authority to a named NFR row; same matrix-bloat concern as B, and `@nfr-sec-06` already covers the authorization surface semantically. |

### D2 — Walking-skeleton coverage for `GET …/comments/{id}/edit`

| Option | Status | Rationale |
|---|---|---|
| A. Standalone `@walking_skeleton` for GET edit-form (separate from PATCH WS) | DEFERRED | Two WS scenarios in one slice (~350 ms total) for what is naturally one user journey. Reviewable as a v0.2 split if a single-driving-adapter-per-WS reviewer flags it; cost is a tag edit. |
| **B. Bundle GET under the PATCH WS — one WS scenario, GET-then-PATCH end-to-end** | **CHOSEN** | The user story IS "author clicks Edit, sees the form, submits, sees the updated card." Single WS scenario tells it end-to-end (~250 ms). Mandate 6 is satisfied — the GET edit-form is exercised via subprocess inside the bundled WS. |
| C. Bundle PLUS a focused GET smoke | DEFERRED | Three closely-related scenarios for the same affordance; the bundled WS already asserts the textarea contains the raw markdown source. |

### D3 — Cancel-edit handler URL shape

| Option | Status | Rationale |
|---|---|---|
| **A. Thin new GET `…/comments/{id}` handler** | **CHOSEN** | Each htmx affordance gets a dedicated server-rendered fragment endpoint. RESTful (`GET /comments/{id}` is the natural resource). Matches slice-2's fragment-per-affordance pattern. Cost is ~30 LOC + ~100 ms test. The DISTILL test asserts a card-shaped (not form-shaped) fragment, which BOTH A and B satisfy — so if DELIVER prefers B at GREEN, the test stays green. |
| B. Reuse issue-detail page GET with htmx `hx-select` | DEFERRED | Bandwidth-wasteful — cancel GETs an entire issue page to extract one card. The test admits B per the card-shaped-not-form-shaped assertion. |
| C. Defer cancel — client-side alpine.js show/hide | DEFERRED | Adds an alpine.js dependency; unreachable from cucumber-rs without a browser harness; breaks the server-rendered-fragments pattern. |

Cancel handler URL is finalized as `GET /workspaces/{w}/projects/{p}/issues/{n}/comments/{id}` (the thin new handler). DELIVER implements `show_single_comment` returning the rendered comment card fragment.

### D4 — 410-Gone htmx UX wording

| Option | Status | Rationale |
|---|---|---|
| **A. Terse: "This comment has been deleted. Refresh to see the latest state."** | **CHOSEN** | Matches the slice-2 error-fragment tone (`Comment cannot be empty`, `Comment is too long`). Single `<p>` element (~70 bytes). The 410 status itself carries the semantic — the prose just humanises it. Test asserts substring match (not equality), so a v0.2 copy polish does not red the suite. |
| B. Apologetic / context wording | DEFERRED | ~20% larger payload; tone drifts from the existing slice-2 fragments. |
| C. Embedded refresh button | DEFERRED | Introduces a button handler the slice would also need to test; out of scope for a tombstone fragment. |

### D5 — Admin-undelete operator runbook recipe

| Option | Status | Rationale |
|---|---|---|
| A. Ship runbook recipe in slice 5 | DEFERRED | Slice 5 is already the largest extension since slice 2 (~340 LOC). Adding doc work expands review burden for marginal value. |
| **B. Defer admin-undelete operator runbook to v0.2** | **CHOSEN** | ADR-007 already documents the schema affords undelete ("undelete is a single UPDATE"); a literate operator can derive the recipe. The natural v0.2 follow-up bundles the runbook with the GC task (ADR-007 "alternative C", also deferred to v0.2 per same DESIGN wave-decisions.md) — both are operator concerns and ship together cleanly. |

**Slice-5 outcome of D5 = B**: NOTHING ships in slice 5 for the runbook. No scenario, no doc, no production code. The v0.2 follow-up bundles cleanly with the ADR-007 GC task.

## Structural decisions (no user pick — locked by inheritance)

| ID  | Question | Pick | Captured in |
|-----|----------|------|-------------|
| DD-1 | Strategy (per port-class default) | C — all real adapters per policy file | `docs/architecture/atdd-infrastructure-policy.md` (inherited) |
| DD-2 | New step file vs extending slice-2 | NEW file `us_10_comment_edit_delete.rs`; slice-2 `us_10_comments.rs` left intact | `crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs` + `lib.rs` registration |
| DD-3 | Scaffold-RED mechanism | Step bodies `panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this")`; production code NOT touched per task brief | step file body + `red-classification.md` |
| DD-4 | Force-link discipline | `tests/acceptance.rs` adds `use foundry_acceptance::steps::us_10_comment_edit_delete as _us_10_edit;` next to the existing `_us_10` import | `crates/foundry-acceptance/tests/acceptance.rs` line ~44 |
| DD-5 | World additions | Four `Option` / `HashMap`-default fields appended after slice-4 US-13 block with header `// ---- US-10 edit/delete (slice 5) ----`; all defaulted so existing scenarios unaffected | `crates/foundry-acceptance/src/world.rs` (bottom) |
| DD-6 | Scope reconciliation (DISCUSS vs DESIGN) | Zero contradictions — DISCUSS § US-10 UAT scenarios fully covered by D1–D7 DESIGN picks | this file § "Reconciliation" below |
| DD-7 | Reviewer dispatch deferred to PR time | Per slice-4 wave-decisions.md line 209 — no in-DISTILL reviewer parallel-dispatch | this file § "Final Wave Review Gate" |

## Reconciliation (HARD GATE)

Per nw-distill § "Wave-Decision Reconciliation HARD GATE". Files read:

- `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-10 (lines 1057–1175) — 4 ACs + 6 UAT scenarios; slice 5 inherits US-10 from `foundry-backend-mvp` (no separate slice-5 DISCUSS)
- `docs/feature/comment-edit-delete/design/wave-decisions.md` — D1–D7 picks + Reuse Analysis
- `docs/feature/comment-edit-delete/design/architecture.md` + `design/adrs/ADR-006..ADR-009.md`
- No `docs/feature/comment-edit-delete/devops/` directory (slice 5 has no infra changes; per nw-distill § Graceful Degradation = WARN, default to slice-1/2/3 infrastructure recorded in policy file)

**Reconciliation result: PASSED — 0 contradictions** across DISCUSS / DESIGN.

## Scenarios per file table

| File | Scenarios | Of which @walking_skeleton | Of which @error | Of which @realtime |
|---|---|---|---|---|
| `features/us-10-comment-edit-delete.feature` (slice 5, NEW) | 10 | 1 | 3 | 2 |
| `crates/foundry-acceptance/tests/features/us-10-comments.feature` (slice 2, unchanged) | 6 | 1 | 3 | 1 |

Total US-10 surface after slice 5: **16 scenarios across 2 files** — slice 2 ships POST + GET + sanitize; slice 5 ships PATCH + DELETE + admin-delete + 410-Gone + soft-delete invariant + realtime fan-out of the new event types + cancel.

Error-path ratio for slice 5: 3 of 10 = 30% — below the 40% nw-distill target. **Justification**: slice 5 is a behavioural extension of slice 2; CSRF rejection, empty body, and non-member access errors are already covered by slice 2's US-10 suite. Adding bogus error duplications would lower signal quality. The slice-5-specific errors (non-author edit 403; PATCH-on-tombstone 410; DELETE-on-tombstone 410) cover the NEW failure surfaces. Same justification slice 2 used (coverage-matrix.md row 71).

Scenario count of 10 is kept (one above the 7-9 prompt ceiling); the user picked to keep scenarios 7 + 8 split per verb (PATCH-on-tombstone vs DELETE-on-tombstone) rather than merging into a Scenario Outline, preserving the slice-2 convention of enumerated scenarios.

## Tag conventions added

Inherited from slice 1/2/3/4 (unchanged):
`@walking_skeleton`, `@real-io`, `@driving_adapter`, `@error`,
`@nfr-perf-03`, `@nfr-sec-05`, `@nfr-sec-06`, `@us-NN`, `@manual`,
`@docker-compose`, `@slice1`..`@slice4`.

Added in slice 5 (deltas only):

- `@slice5` — every scenario in the new feature file
- `@comment-edit-delete` — feature-level (mirrors slice-2's `@comments`)
- `@comment-edit` — sub-area: PATCH + GET edit-form (4 scenarios)
- `@comment-delete` — sub-area: DELETE (4 scenarios)
- `@admin` — scenarios exercising the admin-only authorization branch (1 scenario)
- `@realtime` — reused from slice 2 (added to 2 slice-5 scenarios)
- `@gone` — scenarios pinning the 410-Gone semantics (2 scenarios)
- `@soft-delete-invariant` — the invariant scenario that proves the list-query filter holds (1 scenario)
- `@cancel` — the D3-A cancel scenario (1 scenario; now LOCKED by D3 = A pick)

Per D1 = A: no new `@nfr-*` tag added in slice 5.

## CI invocation

Matching slice-2/3/4 style:

```bash
# Full suite (slices 1+2+3+4+5)
cargo test -p foundry-acceptance --test acceptance

# Slice-5 only (DELIVER iteration)
FOUNDRY_ACCEPTANCE_TAGS=@slice5 cargo test -p foundry-acceptance --test acceptance

# Slice 5 + slice 2 US-10 surface (regression while editing slice 5)
FOUNDRY_ACCEPTANCE_TAGS="@slice5 or @us-10" cargo test -p foundry-acceptance --test acceptance

# Narrow band by sub-area
FOUNDRY_ACCEPTANCE_TAGS=@comment-edit cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@comment-delete cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@gone cargo test -p foundry-acceptance --test acceptance
```

Concurrency cap stays at `--max-concurrent-scenarios 6` (inherited from slice 3).

## Suite-time budget

| Scenario | Cost | Notes |
|---|---|---|
| 1 PATCH WS (POST + GET edit-form + PATCH + GET issue page) | ~250 ms | D2 = B bundles GET |
| 2 non-author 403 | ~150 ms | POST as Mei + PATCH as Hiroshi |
| 3 admin delete | ~200 ms | POST as Mei + DELETE as Devansh + GET issue page |
| 4 author delete | ~150 ms | POST as Mei + DELETE as Mei + GET issue page |
| 5 CommentEdited fanout | ~200 ms | Open SSE + POST + PATCH + receive |
| 6 CommentDeleted fanout | ~200 ms | Open SSE + POST + DELETE + receive |
| 7 PATCH on tombstone → 410 | ~150 ms | POST + DELETE + PATCH-on-tombstone |
| 8 DELETE on tombstone → 410 | ~150 ms | POST + DELETE + DELETE-again |
| 9 soft-delete invariant on list | ~150 ms | POST + DELETE + GET issue page |
| 10 cancel (D3 = A) | ~100 ms | POST + GET-cancel + assert card-shape |
| **Slice-5 subtotal** | **~1.7 s** | within the 20 s slice budget |
| Slice 1 baseline | ~3.5 s | |
| Slice 2 added | ~7.7 s | |
| Slice 3 added | ~80 s | attachments + backup/restore |
| Slice 4 added | ~30 s | walking-skeleton nested `cargo test` |
| **Slice 1+2+3+4+5 projected total** | **~123 s** | slice-5 delta negligible |

Slice-3 + slice-4 dominance is structural; the default fast loop
strips them via `@docker-compose` and `@walking_skeleton @us-13`
exclusion (fast-loop projected total < 30 s).

## Open Decisions for DELIVER

| Decision | DISTILL status | DELIVER inheritance |
|---|---|---|
| Comment-card swap target shape (`id="comment-{uuid}"` vs `id="comment_{uuid}"`) | DESIGN flagged as "software-crafter aligns to the existing convention during GREEN"; DISTILL scaffold tests do NOT assert the literal id shape, only the structural property "the card for this comment is present / absent" | DELIVER picks the literal shape at GREEN; tests stay green either way |
| GET edit-form URL suffix (`/comments/{id}/edit` vs `/comments/{id}?action=edit`) | DESIGN architecture.md line 104 picked `/comments/{id}/edit`; DISTILL step bodies hard-code this URL | DELIVER inherits; if user overrides DESIGN, DISTILL step body changes by 5 chars |
| Comment soft-delete column types (TIMESTAMPTZ NULL vs ON DELETE SET NULL for deleted_by) | DESIGN "Decision-driven invented detail" item 1; DISTILL tests do NOT touch the schema directly | DELIVER picks; tests stay green |
| Partial index `idx_comments_issue_live` (DESIGN item 2) | DISTILL tests do NOT exercise the index directly (they read via the list endpoint) | DELIVER inherits; a v0.2 drop of the partial index does not red the suite |
| `EventPayload.deleted: Option<bool>` (ADR-008) | DISTILL realtime scenarios assert `event_type` + `comment_id`, not the `deleted` bool | DELIVER picks the enum shape; tests stay green |

The cancel handler URL is no longer open — D3 = A locked the thin new GET `…/comments/{id}` handler. DELIVER implements `show_single_comment`.

## DELIVER Pre-flight Checklist

**Migration**
- [ ] `0006_comments_edit_delete.sql` adds `updated_at TIMESTAMPTZ NULL`, `deleted_at TIMESTAMPTZ NULL`, `deleted_by UUID NULL REFERENCES users(id)` + partial index `idx_comments_issue_live` (architecture.md lines 127–136)

**Store methods (3 new, unit-tested in DELIVER's PBT phase per ADR-025 D2)**
- [ ] `Store::update_comment_with_outbox` — `comments` UPDATE + outbox `CommentEdited` in one txn
- [ ] `Store::soft_delete_comment_with_outbox` — `comments` UPDATE setting `deleted_at` + `deleted_by` + outbox `CommentDeleted` in one txn
- [ ] `Store::find_comment_by_id` — returns `Option<CommentRow>` including soft-deleted rows (drives 404-vs-410 dispatch)

**Handlers (3 new, wired into `build_router`)**
- [ ] `submit_edit_comment` (PATCH `/workspaces/{w}/projects/{p}/issues/{n}/comments/{id}`)
- [ ] `submit_delete_comment` (DELETE same URL)
- [ ] `show_edit_form` (GET `…/comments/{id}/edit`) — textarea fragment with raw markdown source
- [ ] `show_single_comment` (GET `…/comments/{id}`) — rendered comment card fragment **per D3 = A**

**Event surface**
- [ ] `EventPayload` extended with `deleted: Option<bool>` per ADR-008; `schema_version` stays at 1
- [ ] SSE handler match arms added for `CommentEdited` / `CommentDeleted` (extending existing `CommentAdded` match per ADR-008)

**Render + invariants**
- [ ] List query in issue-detail handler filters `WHERE deleted_at IS NULL` (verified by soft-delete-invariant scenario)
- [ ] 410-Gone handler returns terse fragment "This comment has been deleted. Refresh to see the latest state." **per D4 = A** (substring match; copy polish OK at v0.2)
- [ ] `render_comment_card` carries author-conditional Edit affordance (only the comment author sees Edit; author + admins see Delete)
- [ ] Edit-rendered card includes "edited" indicator whenever `updated_at IS NOT NULL` (UAT 3, asserted inside WS)

**Plumbing**
- [ ] `Store::probe()` extended with migration-0006 column-existence assertion (architecture.md § Earned Trust)
- [ ] PATCH + DELETE handlers carry CSRF middleware (per ADR-009)

**Regression**
- [ ] All 10 slice-5 scenarios GREEN end-to-end against real Postgres + real axum
- [ ] No regression in 6 slice-2 US-10 scenarios + 37 other slice-1–4 scenarios
- [ ] Per D5 = B — admin-undelete operator runbook is EXPLICITLY DEFERRED to v0.2 (bundles with ADR-007 GC task); ship no runbook content this slice

## Decision-driven invented detail (slice 5 DISTILL deltas only)

Only one phrasing flag was introduced by DISTILL beyond the DESIGN
"invented detail" set:

- **Scenario 7 title uses "soft-deleted"** — the literal scenario title
  is "PATCH on a comment that has already been soft-deleted returns 410
  Gone with an htmx fragment". The word "soft-deleted" surfaces in the
  scenario title only (not in any Given/When/Then body). CM-B flagged
  it for Pillar 1 review. Recommendation kept: the slice-5 user
  terminology IS that — moderation audit treats the row as recoverable;
  ADR-007 § Consequences names it directly. Replace with
  "already-removed" if a Pillar 1 reviewer insists; one-line title edit,
  no body changes.

All five DESIGN-side invented-detail flags (htmx swap target id shape,
GET edit-form URL suffix, soft-delete column types, partial index,
`EventPayload.deleted` enum shape) are unchanged by DISTILL — see
Open Decisions for DELIVER above.

## Final Wave Review Gate

Per slice-4 wave-decisions.md line 209 — the project pattern defers
the 4-reviewer wave-gate to PR time (legacy per-wave file layout, all
slices 1–4 reviewer-approved under this convention). No in-DISTILL
parallel reviewer dispatch. The PR will carry the DESIGN ADRs + this
DISTILL artifact set + DELIVER work for reviewers to inspect
simultaneously.
