# ADR-017: `comments_visible` SQL VIEW — Deferred to v0.3

## Status
Accepted — 2026-05-26

## Context

ADR-007 (slice 5) acknowledged a defense-in-depth concern in its cons
discussion of option B (lines 88-91): "UI logic must consistently
apply the `WHERE deleted_at IS NULL` filter (one missed `WHERE` and
deleted comments leak — mitigated by acceptance-suite enforcement;
v0.2 may introduce a `comments_visible` SQL VIEW to make it
schema-level)."

Slice-5 wave-decisions.md documented this as a v0.2-candidate.

Slice 7 — closing other v0.2 deferrals (GC + admin-undelete) —
revisits the question: does THIS slice introduce a `comments_visible`
VIEW to elevate the soft-delete invariant from behavioural (every
read path filters `WHERE deleted_at IS NULL`) to schema-level (every
read path SELECTs from a view that already filters)?

The viable shapes were:

- Ship the VIEW only (no read-path migration) — schema-level surface
  exists but is unused.
- Ship the VIEW AND migrate all existing read paths to use it — full
  defense-in-depth; significant scope creep.
- Don't ship the VIEW this slice — defer as a separable concern.

Quality attributes in play: **maintainability (MEDIUM)** — as more
read paths land (US-12+ comment-search, comment-export), the "missed
WHERE" risk surface grows; **separation of concerns (HIGH)** — slice 7
is bundled as "operator-facing GC + recovery"; defense-in-depth
read-path engineering is a different category of work.

## Decision

**Do NOT ship the `comments_visible` VIEW this slice. Defer to v0.3
as its own dedicated slice that retrofits all read paths to use the
VIEW.**

The deferred slice (placeholder name: `comment-read-defensive-engineering`)
would, in a single coherent PR:

1. Add migration `0007_comments_visible_view.sql` creating
   `CREATE VIEW comments_visible AS SELECT * FROM comments WHERE deleted_at IS NULL`.
2. Audit every existing read path in `crates/foundry-store/src/` for
   `FROM comments` references; migrate the ones that semantically
   want "visible only" to `FROM comments_visible`.
3. Update acceptance scenarios as needed; re-run the slice-5
   soft-delete-invariant scenario suite against the migrated paths.
4. Document the convention in `architecture.md` (slice-1) so future
   read paths default to the view.

Slice 7 continues to rely on slice-5's behavioural invariant +
acceptance scenario 9 (`@soft-delete-invariant`) which has been
holding fine. Slice 6 added zero new comment read paths, so the
"missed WHERE" risk hasn't manifested.

## Alternatives Considered

### A: Don't ship the VIEW this slice (chosen)
See Decision. Defer as a v0.3 candidate that ships as its own slice
with full read-path migration.

### B: Ship the VIEW only, no read-path migration (rejected)
- **Pros**: Schema-level enforcement available immediately for any
  future read path that opts in.
- **Cons**: Shipping a view that nothing reads creates schema surface
  area with no immediate value — "I added a view nobody reads" is
  dead weight. Net win is delayed until something actually consumes
  the view; until then, the VIEW is documentation pretending to be
  enforcement.
- **Rejected because**: schema bloat without active enforcement
  benefit; misleading apparent posture.

### C: Ship the VIEW AND migrate all existing read paths (rejected)
- **Pros**: Maximum benefit of the VIEW pattern. The "missed WHERE"
  risk is eliminated structurally for all current read paths.
- **Cons**: Significant scope creep — slice 7 grows to touch every
  existing read path. The acceptance suite needs full re-execution of
  comment-rendering paths. Pushes slice 7 from "small bundled
  deferral closure" to "schema refactor + read-path audit". Mixes
  concerns: this slice is about GC + recovery, not read-side
  defensive engineering.
- **Rejected because**: scope discipline; defense-in-depth deserves
  its own slice with its own DESIGN pass + acceptance coverage.

## Consequences

### Positive
- **Slice 7 stays small.** Bundled as "operator-facing GC + recovery";
  no schema refactor; no read-path audit. ~240 LOC of Rust + ~50
  lines of docs, as scoped.
- **v0.2 RC scope frees up.** The slice-5 wave-decisions.md's
  v0.2-candidate list is reduced explicitly (this ADR moves the VIEW
  off v0.2). The v0.2 release ships the actual GC commitment from
  ADR-007 without entangling unrelated read-path engineering.
- **v0.3 candidate has a clear shape.** The deferred slice has a
  documented name (`comment-read-defensive-engineering`), a clear
  scope (VIEW migration + read-path retrofit), and a clear precedent
  (the slice-5 `@soft-delete-invariant` scenario becomes the
  regression baseline).
- **Behavioural invariant continues to work.** Slice-5 acceptance
  scenario 9 has been holding; slice 6 added zero new comment read
  paths; the "missed WHERE" risk is currently latent, not active.

### Negative
- **The "missed WHERE" risk persists for v0.2.** Every future read
  path (US-12+ comment-search, comment-export, etc.) adds a place
  where the convention could be forgotten. Mitigation: the v0.3
  candidate is explicitly tracked; future read-path slices either
  add their own `@soft-delete-invariant` scenarios or wait for the
  v0.3 VIEW slice.
- **Two coordination overheads instead of one.** The VIEW migration
  ships separately from the GC; operators must adopt two changes in
  successive releases instead of one. Trade-off is judged worth it
  for the scope discipline.

### Neutral
- **Reversibility**: deferring is the most reversible posture
  possible — no schema change, no code change. The v0.3 slice is
  free to choose any shape (VIEW vs RLS policy vs trigger-based
  guard vs static-analysis enforcement vs combination).
- **Slice-5 wave-decisions.md cross-reference**: this ADR explicitly
  supersedes the v0.2-candidate status of the VIEW. Future readers
  of slice-5 wave-decisions.md who follow the candidate-tracker
  forward will land here.

## Verification

This ADR records a deferral; verification is the absence of the
deferred work in this slice:

- No new migration file in `crates/foundry-store/migrations/` this
  slice. `git diff` on the migrations directory confirms.
- No new VIEW reference in `crates/foundry-store/src/` this slice.
- Slice-5 acceptance scenario 9 (`@soft-delete-invariant`) continues
  to pass unchanged in slice 7's PR. The behavioural invariant
  remains the active enforcement mechanism for v0.2.
- The slice-5 wave-decisions.md candidate-tracker entry (if a
  candidate-tracker exists in the v0.3 planning artefact) is
  updated to reflect this deferral; otherwise, slice 7's
  evolution doc captures the deferral for the next planning cycle
  to discover.
