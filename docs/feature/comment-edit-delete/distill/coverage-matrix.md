# Coverage Matrix — Slice 5 (Comment Edit/Delete)

Per-story trace from acceptance criteria (DISCUSS `stories.md` § US-10 +
DESIGN `wave-decisions.md` D1-D7) to scenario files. Slice 5 lights
up the deferred US-10 ACs from slice 2 (the slice-2 coverage matrix
explicitly routed AC 2 "Author can edit/delete own comments; admin
can delete any" to a later slice — `docs/feature/foundry-realtime-collab/distill/coverage-matrix.md`
line 32 says "DEFERRED to slice 3 — slice 2 ships read + create +
sanitize"; the actual landing was deferred again to slice 5 due to
slice-3 prioritizing operator-grade work).

## US-10 — Comments (delta-only — slice 2 rows omitted)

Source: `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-10
(lines 1057-1175). The 4 ACs + 6 UAT scenarios are the canonical
specification.

| AC / UAT / DESIGN-new | Origin | Scenario(s) | Tag(s) |
|---|---|---|---|
| Markdown rendered with CommonMark, sanitized | US-10 AC 1 | (covered by slice 2 — `us-10-comments.feature` line 44 + line 59) | — |
| **Author can edit/delete own comments** (AC 2 part 1, UAT 3 + 6) | US-10 AC 2 / UAT 3 / UAT 6 | "Comment author edits their own comment …" (slice-5 WS); "Comment author deletes their own comment" | `@walking_skeleton @real-io @driving_adapter @us-10 @slice5 @comment-edit`; `@real-io @us-10 @slice5 @comment-delete` |
| **admin can delete any** (AC 2 part 2, UAT 5) | US-10 AC 2 / UAT 5 | "Workspace admin deletes any comment and remaining viewers see it disappear from the thread" | `@real-io @us-10 @slice5 @comment-delete @admin` |
| Non-author cannot edit (UAT 4 + DESIGN auth invariant 4) | US-10 UAT 4 + architecture.md "Authorization is HTTP-verb-uniform" constraint 4 | "A non-author cannot edit someone else's comment" | `@real-io @error @us-10 @slice5 @comment-edit @nfr-sec-06` |
| "edited" indicator next to timestamp (UAT 3) | US-10 UAT 3 + D4 = A | (covered inside the slice-5 WS scenario as the `with an "edited" indicator` assertion) | (inside WS) |
| Realtime delivery via same SSE channel as US-09; ≤1s median (NFR-PERF-03) | US-10 AC 3 | "An open subscriber receives a CommentEdited event when another viewer edits"; "An open subscriber receives a CommentDeleted event when another viewer deletes" | `@real-io @us-10 @slice5 @comment-edit @realtime @nfr-perf-03`; `@real-io @us-10 @slice5 @comment-delete @realtime @nfr-perf-03` |
| **DESIGN-new — 410 Gone for PATCH/DELETE on already-deleted comment** | DESIGN D6 = B + ADR-008 § Decision | "PATCH on a comment that has already been soft-deleted returns 410 Gone …"; "DELETE on a comment that has already been soft-deleted returns 410 Gone" | `@real-io @error @us-10 @slice5 @gone` (×2) |
| **DESIGN-new — soft-delete invariant on list query** | DESIGN wave-decisions.md Constraint 1 + ADR-007 | "The issue page lists only non-deleted comments" | `@real-io @us-10 @slice5 @soft-delete-invariant` |
| **DESIGN-new — cancel edit returns original card (conditional D3)** | DISTILL proposals.md § D3 = A | "Author cancels the edit and the original card is returned by the server" | `@real-io @us-10 @slice5 @comment-edit @cancel` |
| No nested threads in MVP (deferred) | US-10 AC 4 | Documented constraint, not testable in slice 5 | n/a |

**Driving-adapter coverage for slice 5** — three new HTTP endpoints:

| Endpoint | Method | Scenario covering it via subprocess HTTP | Tag |
|---|---|---|---|
| `…/issues/{n}/comments/{id}/edit` | GET | "Comment author edits their own comment …" (slice-5 WS — GETs the edit form before PATCHing) | `@walking_skeleton @driving_adapter` (bundled per DISTILL D2 = B) |
| `…/issues/{n}/comments/{id}` | PATCH | "Comment author edits …" (slice-5 WS); "A non-author cannot edit …"; "An open subscriber receives a CommentEdited …"; "PATCH on a comment that has already been soft-deleted …" | `@walking_skeleton @driving_adapter` (WS row) |
| `…/issues/{n}/comments/{id}` | DELETE | "Workspace admin deletes …"; "Comment author deletes …"; "An open subscriber receives a CommentDeleted …"; "DELETE on a comment that has already been soft-deleted …" | (covered across 4 scenarios; no separate WS) |
| `…/issues/{n}/comments/{id}` | GET (cancel handler, conditional D3 = A) | "Author cancels the edit …" | (only if D3 = A) |

All four endpoints are exercised via real `reqwest::Client` against the
in-process `spawn_app` axum binary. No mocks, no synthetic HTTP — per
the slice-1 inherited Strategy C.

## Adapter coverage table (Mandate 6 enforcement)

Every driven adapter touched by slice 5 has at least one `@real-io`
scenario. Slice 5 introduces ZERO new driven adapters — all rows
inherit slice-1/2/3 coverage.

| Adapter | @real-io scenario | Covered by |
|---|---|---|
| Postgres `comments` table (write path: UPDATE for edit, UPDATE for soft-delete) | YES | slice-5 WS (PATCH path) + slice-5 author-delete + slice-5 admin-delete (DELETE paths) |
| Postgres `comments` table (read path: list query filtering tombstones) | YES | slice-5 soft-delete-invariant scenario |
| Postgres outbox table (write of `CommentEdited` + `CommentDeleted` rows) | YES | slice-5 CommentEdited realtime scenario + CommentDeleted realtime scenario (the event is observed at SSE end, which proves the outbox row + trigger fired) |
| pg_notify + per-replica LISTEN + tokio::sync::broadcast SSE fan-out | YES | slice-5 CommentEdited + CommentDeleted realtime scenarios |
| pulldown-cmark markdown rendering | YES | slice-5 WS PATCH (re-renders body_html from new body_markdown; the `<strong>` element with text "Set-Cookie SameSite=Strict" assertion proves the renderer ran) |
| ammonia HTML sanitization | YES | slice-5 WS PATCH (the edit re-render path runs the same sanitizer — inherited security contract from slice-2 NFR-SEC-05; no new explicit @nfr-sec-05 scenario in slice 5 because slice-2 already pins it) |
| tower-sessions PG session store | YES | implicit in every scenario (sign-in is the first action) |
| CSRF middleware double-submit | YES | implicit in every PATCH and DELETE scenario (a CSRF failure would surface as 403 from the middleware, not the handler) |

Zero `NO — MISSING` rows.

## Cross-cutting roll-up

| Metric | Target | Actual (slice 5) |
|---|---|---|
| Total NEW scenarios | 7-9 prompt cap; "a bit higher" tolerated | 10 (one above ceiling; scenarios 7+8 are split per "PATCH" vs "DELETE" verb — see `proposals.md` § scope footnote for the Scenario-Outline merge option) |
| @walking_skeleton scenarios | exactly 1 per feature file | 1 (slice-5 PATCH WS bundling GET edit-form per DISTILL D2 = B) |
| @real-io scenarios | every driven adapter covered | 10 of 10 |
| @error scenarios | ≥40% of automated total | 4 of 10 = 40% (non-author edit 403; PATCH-on-tombstone 410; DELETE-on-tombstone 410; one more — the soft-delete-invariant scenario is a positive contract, not error; the cancel scenario is positive too; the realtime scenarios are positive contracts.) Recheck: errors = scenarios #2 (403), #7 (410), #8 (410) = 3 of 10 = 30%. **Justification for going below 40%**: slice 5 is a behavioural extension of slice 2; many "errors" (CSRF rejection, empty body, non-member access) are already covered by slice 2's existing US-10 suite. Adding bogus error duplications would lower signal quality. The slice-5-specific errors (non-author edit; PATCH-on-tombstone; DELETE-on-tombstone) cover the NEW failure surfaces. Same justification slice 2 used (coverage-matrix.md row 71 "Adding bogus errors to hit 40% would lower signal quality"). |
| `@manual` scenarios | as needed | 0 |
| `@nfr-*` scenarios | one per applicable NFR cell | 2 (`@nfr-perf-03` ×2 on the realtime scenarios); `@nfr-sec-06` ×1 on the non-author 403; `@nfr-sec-05` inherited from slice 2 (sanitizer re-run is implicit in WS edit path) |
| Test-suite runtime impact | ≤20s added on top of slice-4 | ~1.7s per `driver.md` § 6 |
| Driving-adapter coverage | every new endpoint exercised via its protocol | 3 new endpoints (GET edit-form, PATCH, DELETE); all 3 covered. The conditional 4th (GET single-comment cancel handler) is covered if D3 = A. |

## Mandate compliance evidence (CM-A through CM-H — per slice-2 template)

- **CM-A (Hexagonal boundary)**: every step-method invokes the
  production composition root via `spawn_app` + real `reqwest`.
  Zero step bodies construct `Store` directly. Verified against the
  slice-2 precedent (which already passes CM-A); slice-5 step file
  imports `crate::world::FoundryWorld` and `cucumber::{given, then,
  when}` only — no direct adapter or store import.
- **CM-B (Business language)**: no Gherkin line mentions `pg_notify`,
  `tokio::sync::broadcast`, `axum`, `sqlx`, `LISTEN`,
  `pulldown-cmark`, `ammonia`, or `tombstone`. HTTP status numbers
  (200, 403, 410) appear in the `@error` / authorization scenarios
  where the status is a user-facing contract (the browser shows a
  status-aware page; htmx routes 410 to a different swap target via
  `hx-target-410`) — same exemption as slice 1+2 driver.md § 8. The
  word "soft-deleted" appears in one scenario title — flagged here for
  user review: replace with "already-removed" if Pillar 1 reviewer
  insists. (Recommendation: keep "soft-deleted" because the slice-5
  user terminology IS that — moderation audit treats the row as
  recoverable; ADR-007 § Consequences names this directly.)
- **CM-C (User journey completeness)**: every scenario walks from a
  user trigger (sign-in + verb) to an observable outcome (re-rendered
  comment card, SSE event arrival, 403/410 response with htmx
  fragment, list-query absence of soft-deleted row). No
  "validator-accepts-JSON" framings.
- **CM-D (Pure function extraction)**: not applicable at the
  acceptance layer — DELIVER's PBT unit tests cover the soft-delete
  state-machine (live → tombstoned), the markdown re-render path
  (idempotent given input), and the 404-vs-410-vs-403 dispatch
  predicate. Routed to DELIVER's RED phase per ADR-025 D2.
- **CM-E (No fixture theater)**: every Given step sets up
  PRECONDITIONS, not expected outputs. The "previously posted a
  comment" Given inserts a real comment via the real POST path (not a
  direct DB insert that bypasses the slice-2 POST handler) so the
  scenario passes only when the slice-5 edit/delete code is actually
  wired (otherwise the GIVEN itself is wired up but the WHEN reveals
  the missing implementation). Confirmed: the RED classification
  document records that every scenario fails at the first slice-5
  Given because the GIVEN is the first phrase introduced in slice 5;
  the slice-2 background phrases (workspace + member + project + issue
  + sign-in) all pass GREEN, proving the test infrastructure is sound
  and only the slice-5-specific code is missing.
- **CM-F (Walking skeleton litmus test)**: the slice-5 WS scenario
  "Comment author edits their own comment …" is demo-able to a
  non-technical stakeholder: "Mei posts a comment, clicks Edit, sees
  the form, submits the change, and the page re-renders with the
  updated text and an 'edited' label." That IS the user-facing value
  of slice 5.
- **CM-G (Driving-adapter coverage per Mandate 6 / RCA-fix P1)**: all
  three new HTTP endpoints (GET edit-form, PATCH, DELETE) are
  exercised via subprocess HTTP per the table above. None bypass the
  CSRF middleware or sign-in. The cancel handler (conditional D3 = A)
  is the only marginal case — its WS-tier coverage is the dedicated
  cancel scenario.
- **CM-H (Pre-DELIVER fail-for-right-reason gate)**: PASSED. All 10
  scenarios fail with `panic!("Not yet implemented -- RED scaffold
  (DISTILL); DELIVER finishes this")`. Zero compile errors, zero
  import errors, zero fixture failures. See `red-classification.md`.

## Definition of Done — slice 5 DISTILL

- [x] 1 feature file (`features/us-10-comment-edit-delete.feature`), 10
      scenarios (1 above the 7-9 prompt cap; flagged for user merge
      decision in `proposals.md` § scope footnote)
- [x] 1 `@walking_skeleton` scenario per the bundled-GET pattern
      (DISTILL D2 = B recommendation)
- [x] All 3 new driving adapters covered via subprocess HTTP (GET
      edit-form, PATCH, DELETE)
- [x] `driver.md` documents the zero-new-infra decision and the world
      additions + force-link
- [x] `step-skeletons.md` enumerates the new step signatures + lists
      the inherited slice-1/2 steps it reuses
- [x] No new crate dependencies; no new policy rows
      (`docs/architecture/atdd-infrastructure-policy.md` unchanged)
- [x] Suite runtime delta within the 20s ceiling (~1.7s actual)
- [x] Compile passes: `cargo check -p foundry-acceptance --tests`
- [x] Pre-DELIVER fail-for-right-reason gate: PASSED (see
      `red-classification.md`)
- [x] Reuse-Analysis HARD GATE: zero new ports, zero new adapters; all
      changes additive in existing files (per slice-5 DESIGN
      wave-decisions.md § Reuse Analysis)
- [x] Wave-Decision Reconciliation HARD GATE: 0 contradictions across
      DISCUSS / DESIGN (no slice-5 DEVOPS exists by design — warned
      and proceeded per nw-distill graceful-degradation matrix)
- [ ] User picks on D1-D5 in `proposals.md` (PENDING)
- [ ] `wave-decisions.md` finalized once D1-D5 land (PENDING)
- [ ] PR-time 4-reviewer wave-gate (deferred per slice-4 convention)
