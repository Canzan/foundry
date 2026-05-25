# Evolution — comment-edit-delete (Slice 5)

**Finalized**: 2026-05-25
**Ship commit**: [e0da38e](../../) — "Slice 5: US-10 comment edit + delete — author + admin moderation"
**Wave coverage**: DESIGN → DISTILL → DELIVER (DISCUSS inherited from slice-1 `stories.md` § US-10; DEVOPS not applicable — zero infra changes)

## Feature summary

Closes the carried-forward US-10 commitment from slice 2: comment
authors can edit and delete their own comments, workspace admins can
delete any comment, viewers see "edited" indicators and realtime
disappearance, and the soft-delete tombstone preserves moderation
audit trail. 10 new acceptance scenarios green; full suite 92/92.

This is the first slice in the project to traverse the full nWave
workflow end-to-end (`/nw:design` → `/nw:distill` → `/nw:deliver` as
distinct dispatched waves with propose-mode option resolution). All
prior slices ran DESIGN/DISTILL/DELIVER together or skipped some
waves entirely.

## Business context

US-10 AC 2 ("Author can edit/delete own comments; admin can delete
any") was documented in slice-1 `stories.md` but explicitly deferred
in slice-2 (`docs/feature/foundry-realtime-collab/distill/coverage-matrix.md`
flagged it as a "known gap, routed to slice 3"). Slice 3 shipped
operator-grade work instead. The 2026-05-25 evolution sweep surfaced
the open commitment, and this slice resolves it.

## Key decisions

### From DESIGN (`docs/feature/comment-edit-delete/design/`)

- **ADR-006 — Always-editable.** No edit-window policy. Zero clock
  dependency in the authorization check. The "edited" indicator
  (per D4) carries enough revision-awareness to handle the audit
  concern without a time wall. Reversible — adding a window in v0.2
  is a one-line authz check addition.
- **ADR-007 — Soft tombstone delete.** `deleted_at TIMESTAMPTZ NULL`
  + `deleted_by UUID NULL REFERENCES users(id)` columns. List
  queries filter `WHERE deleted_at IS NULL`. Full moderation audit
  trail; undelete is trivial (`UPDATE ... SET deleted_at = NULL`).
  90-day hard-delete GC deferred to v0.2 (the B schema is a strict
  subset of C). Slice-5 scenario 9 explicitly enforces the
  soft-delete invariant — a forgotten `WHERE deleted_at IS NULL`
  reds the test.
- **ADR-008 — Two new SSE `event_type` values.** `CommentEdited` +
  `CommentDeleted` rather than a polymorphic `CommentMutated` with
  sub_type. Matches the inherited `IssueCreated`/`IssueUpdated`
  pattern. `schema_version` stays at 1; new payload field
  `deleted: Option<bool>` uses `#[serde(default)]` for forward
  compatibility.
- **ADR-009 — CSRF inheritance.** PATCH and DELETE inherit the
  slice-1 `csrf::csrf_middleware`. htmx `hx-delete` empty-body case
  carries the token via the `HX-CSRF` header (already alpine.js
  convention from slice 2).
- **Q4 = "edited" label + timestamp only.** No diff history table.
  Verbatim AC match. Privacy-friendly.
- **Q5 = inline replace via htmx `hx-swap=outerHTML`.** No modal
  stack. Preserves markdown source (the textarea contains raw
  `body_markdown`, not rendered HTML). Server-side conditional
  affordance — `render_comment_card` emits the Edit/Delete buttons
  only when `comment.author_id == actor.user_id || actor.role == admin`.
- **Q6 = 410 Gone for soft-deleted resources.** 404 for genuinely
  non-existent (the row was never there). Matches slice-1 bootstrap
  precedent (used token → 410).

### From DISTILL (`docs/feature/comment-edit-delete/distill/`)

- **Strategy C inherited.** Zero new ports → zero new rows appended
  to `docs/architecture/atdd-infrastructure-policy.md`. Every
  adapter is reused (PATCH/DELETE hit the existing `spawn_app` +
  per-scenario schema rotation; SSE uses the existing
  `support/sse_client.rs`).
- **Tier A only (Mandate 10).** 10 scenarios, none chained per
  Pillar 2; Tier B state-machine PBT not warranted.
- **PBT input mode: example-only (Mandate 9).** All 10 scenarios
  run at layer 3+ (real HTTP + real Postgres).
- **D1 = reuse existing `@nfr-*` tags.** `@nfr-perf-03` (1s p99
  fanout), `@nfr-sec-05` (sanitizer), `@nfr-sec-06` (authz)
  inherited from slice 2. No new NFR tag for "edited indicator" —
  it's a functional concern, not an NFR.
- **D2 = bundle GET edit-form under PATCH walking-skeleton.** One
  WS scenario, GET-then-PATCH end-to-end. Suite-time efficient.
- **D3 = new thin `GET /comments/{id}` handler for cancel-edit.**
  Returns the un-edited card fragment. The test asserts on
  "card-shaped, not form-shaped" so the production-side URL has
  some latitude.
- **D4 = terse 410 wording.** "This comment has been deleted.
  Refresh to see the latest state." Assertion uses substring match
  so copy can polish in v0.2.
- **D5 = defer admin-undelete operator runbook to v0.2.** Bundles
  cleanly with the v0.2 GC task per ADR-007 Consequences.

### From DELIVER (extracted from `e0da38e` commit body)

- **No new crate deps.** `pulldown-cmark` + `ammonia` reused for the
  edit re-render path (security argument for re-rendering on edit,
  not on read).
- **`foundry-core` still I/O-free.** `cargo tree -p foundry-core`
  unchanged; the slice-1 architectural promise holds.
- **`render_comment_card` rewired with conditional affordances + "edited" marker.**
  Single template branch decides Edit/Delete button visibility from
  the actor's identity passed in at render time. No alpine.js
  authorship check; server is the source of truth.
- **`probe()` extended to check the slice-6 migration columns.** The
  existing `Store::probe()` now also verifies `comments.updated_at`
  and `comments.deleted_at` exist — the "probe the substrate lie
  that the migration applied but we didn't notice" pattern.
- **Migration 0006** adds 3 nullable columns + a partial index
  `idx_comments_issue_live ON comments (issue_id, created_at) WHERE deleted_at IS NULL`
  to keep the hot path narrow.
- **Two new routes** in `crates/foundry-app/src/lib.rs` (one chains
  GET/PATCH/DELETE on the same path; one is the cancel-edit single-
  comment GET).

## 3 deviations from DESIGN (back-propagated for next-feature reference)

1. **Added "Devansh" persona to slice-2 `us_07_project_create.rs::identity_for`**.
   cucumber-rs requires globally-unique step phrases. The
   `(\w+) is signed in` step is owned by slice-2; adding a parallel
   "admin is signed in" step would have collided. The minimal
   additive change was to add the persona to the existing
   `identity_for` map. Admin password is
   `admin-password-from-bootstrap` (mirroring slice-1 us_06 + slice-3
   US-03 conventions).
2. **Admin DELETE path skips the team-membership check**.
   Architecture explicitly permits "admin moderates any team's
   comments" (ADR-006 § DELETE authz table). The handler resolves
   admin status first and short-circuits the team-membership gate.
   Without this, a workspace admin who is not a member of the team
   would get 403 on the moderation path the design explicitly grants.
3. **OOB-swap fragment elides Edit/Delete buttons**. The slice-2
   `render_comment_card_oob` (only used by POST realtime fan-out)
   keeps a button-free shape; affordances arrive on the next
   full-page render. Invisible to the test suite (no scenario
   asserts on the OOB card shape) but keeps the OOB payload simple
   and avoids piping team/project/issue-number context through the
   POST handler signature.

## Steps completed

All work via direct TDD against the 10 pre-scaffolded RED scenarios
from DISTILL. Single ship commit `e0da38e` enumerates the delivered
scope:

### Production changes (`crates/`)

- `foundry-store/migrations/0006_comments_edit_delete.sql` — 3 cols + partial index
- `foundry-store/src/lib.rs` — `find_comment_by_id`, `update_comment_with_outbox`, `soft_delete_comment_with_outbox`, `is_workspace_admin`; `CommentRow` gains `author_id` + `edited`; new `CommentLookupRow`; `list_comments_for_issue` filters tombstones; `probe()` checks 0006 columns
- `foundry-app/src/comments.rs` — 4 new handlers (`show_edit_form`, `submit_edit_comment`, `submit_delete_comment`, `show_single_comment`) + 3 error fragments + `render_comment_card` rewired with conditional affordances + "edited" marker
- `foundry-app/src/lib.rs` — 2 new routes
- `foundry-realtime/src/lib.rs` — `EventPayload.deleted: Option<bool>` (per ADR-008)

### Test changes (`crates/foundry-acceptance/`)

- `tests/features/us-10-comment-edit-delete.feature` — 10 scenarios (promoted from DISTILL)
- `src/steps/us_10_comment_edit_delete.rs` — 21 step bodies (3 Givens, 8 Whens, 10 Thens), all panics replaced with real implementations
- `src/steps/us_07_project_create.rs` — added "Devansh" persona (deviation 1)
- `src/world.rs` + `src/lib.rs` + `tests/acceptance.rs` — 4 new fields + module reg + force-link (all authored by DISTILL; consumed unchanged)

### DESIGN / DISTILL artefacts (`docs/feature/comment-edit-delete/`)

- `design/architecture.md`, `wave-decisions.md`, `proposals.md`, `adrs/ADR-006..009.md`
- `distill/wave-decisions.md`, `driver.md`, `coverage-matrix.md`, `step-skeletons.md`, `proposals.md`, `red-classification.md`, `features/us-10-comment-edit-delete.feature`

## All 10 slice-5 scenarios GREEN (verified at `e0da38e`)

1. ✅ Walking-skeleton edit flow: GET edit-form → PATCH → re-rendered card
2. ✅ Non-author edit attempt → 403 + original text unchanged
3. ✅ Admin deletes any comment → row tombstoned + viewers see removal
4. ✅ Author deletes own comment → same removal flow
5. ✅ CommentEdited fanout via SSE to other viewers
6. ✅ CommentDeleted fanout via SSE
7. ✅ PATCH on already-tombstoned comment → 410 Gone
8. ✅ DELETE on already-tombstoned comment → 410 Gone
9. ✅ Soft-delete invariant: list query filters out tombstones
10. ✅ Cancel-edit returns the un-edited card fragment

## Verification at HEAD (`e0da38e`)

- `cargo xtask ci` → all gates green
- `cargo test --workspace` — 92/92 scenarios pass, 784/784 steps
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `cargo deny check` clean (zero new deps, zero new licenses, zero new advisories)
- `cargo build --release -p foundry-app` (no features) — release binary contains no test-support seams
- Suite-time delta: slice 5 adds ~1.7 s to the default loop (within DISTILL projection)
- Full default suite at HEAD: ~123 s wall-clock (well within the 60 s top-line was per-slice; total is the cumulative)
- Sanity grep `__SCAFFOLD__` / "RED scaffold" / `panic.*Not yet implemented` across `crates/`: **0 hits**

## Lessons learned

1. **First end-to-end nWave wave traversal works.** All five prior
   slices bypassed `/nw:design`, `/nw:distill`, and `/nw:deliver` as
   dispatched commands. This slice ran each as a separate dispatched
   wave with propose-mode option resolution and produced cleaner
   artefacts (proposals.md historical record, scaffold RED
   classification, comprehensive DELIVER pre-flight checklist) than
   the ad-hoc slices. Future slices should follow this pattern.
2. **DISTILL pre-flight checklist is the right handoff shape.** The
   crafter agent executed item-by-item through the 7-section
   checklist (Migration / Store / Handlers / Event surface /
   Render+invariants / Plumbing / Regression) without needing to
   re-read DESIGN docs mid-stream. Bullet-list pre-flights beat
   prose handoffs for crafter throughput.
3. **Propose mode + small open-question budget keeps the flow
   moving.** DESIGN had 6 open questions; DISTILL had 5; each
   resolved with 2–3 option tables and recommended picks. The user
   accepted all recommendations across both waves — meaning the
   agent's first-pass options were well-calibrated. Decision
   latency would have spiked if any wave had presented unjustified
   options or skipped the recommendation column.
4. **The legacy per-wave layout still serves.** Two waves used
   directory-of-files (DESIGN: 7 files; DISTILL: 7 files); DELIVER
   produced one commit. No SSOT migration triggered. The slice-4
   `wave-decisions.md` line 209 documenting the legacy-layout choice
   continues to hold.
5. **cucumber-rs step-phrase global uniqueness is a real constraint.**
   Deviation 1 (adding "Devansh" to slice-2 `identity_for`) was
   forced by this. Future slices that add new persona types should
   plan to extend the existing personas map rather than creating
   parallel sign-in steps.
6. **Probe extension at every migration is cheap insurance.** The
   `Store::probe()` check for `comments.updated_at` / `deleted_at`
   would have caught a forgotten migration before any acceptance
   scenario fired. The "probe the substrate lie that the migration
   applied" pattern is worth applying to every future schema-altering
   slice.

## Issues encountered

- **None blocking.** The flow ran cleanly: DESIGN propose →
  picks → finalize → DISTILL propose → picks → finalize → DELIVER
  direct TDD. Three minor deviations (above) all documented in the
  return summary and folded into this evolution doc.
- **DELIVER ran direct, not via DES orchestrator.** Per project
  convention (5 prior slices all bypassed the orchestrator), this
  slice continued the pattern. DES tooling is available globally
  but the project hasn't established the per-step roadmap.json /
  execution-log.json practice; this slice didn't change that.

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/comment-edit-delete/` has no inbound external
references; the slice's design context flows downward from DESIGN
→ DISTILL → the production code at `crates/foundry-app/src/comments.rs`
+ `crates/foundry-store/migrations/0006_*.sql`. The DESIGN ADRs
(006–009) carry forward as the documented justification for the
edit-window / tombstone / event-shape / CSRF posture decisions.

## Open items for v0.1 RC

1. **90-day GC task** for tombstoned comments — per ADR-007
   Consequences. Slice-6 candidate; bundles cleanly with the
   admin-undelete operator runbook (D5 deferral) and would also
   close the GDPR-friendly storage-bound posture noted in proposals.
2. **Comment-revisions history** (Q4 option B/C) — currently no
   diff history. Promote to v0.2 if operator telemetry shows
   dispute frequency justifies it.
3. **Comment-search** — slice-3 ILIKE search hits issue title only.
   If/when comment body becomes searchable, the soft-delete filter
   must extend there too (currently moot — no comment search index
   exists).
