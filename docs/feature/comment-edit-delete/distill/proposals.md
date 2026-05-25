# DISTILL proposals — comment-edit-delete (slice 5)

**Mode**: propose
**Owner (this wave)**: acceptance-designer (Quinn)
**Status**: AWAITING USER DECISION on D1–D5; the slice-5 scenarios in
`features/us-10-comment-edit-delete.feature` are written against the
**recommended** option for each open question (see § Pick-flag table at
the bottom). The user's overrides will be applied in a follow-up
`execute --finalize` pass; for each override, the scenarios that depend
on the now-unrecommended option are renamed / re-tagged / re-bodied
without restructuring the suite.

---

## 0. What is open

Five small open questions inherited verbatim from
`docs/feature/comment-edit-delete/design/wave-decisions.md` § "Open
Questions for DISTILL" (lines 120-150). All five are bounded — no
question expands slice scope or contradicts the seven D1-D7 picks the
DESIGN wave already locked.

For each, options + recommendation below. The acceptance-designer's
recommendation is anchored on the slice-2 precedent, the slice-5
quality drivers (simplicity HIGH, security HIGH), and per-scenario
suite-time budget (≤200ms target per scenario per slice-2 driver.md).

---

## D1 — NFR tag set for slice-5 scenarios

**Question**: does the "edited indicator render correctness" need a
fresh `@nfr-*` tag, or does it ride existing `@nfr-perf-03` /
`@nfr-sec-05` / `@nfr-sec-06`?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Reuse existing tags** | Tag `@nfr-perf-03` on the realtime-fan-out-of-CommentEdited and CommentDeleted scenarios (same 1s p99 budget as slice-2 CommentAdded). Tag `@nfr-sec-05` on the edit re-render (XSS sanitizer must still hold on edit). Tag `@nfr-sec-06` on the non-author 403 + admin-only delete-any scenarios. No new tag. | Smallest tag surface. No new NFR rows to maintain in the matrix. Treats slice 5 as a behavioural extension of slice-2 surfaces, which is what it is. | "Edited indicator render correctness" is implicit — it's not pinned to a named NFR row. Slightly weaker traceability if a future audit asks "which scenario covers the edited indicator?". Answer: the realtime `CommentEdited` scenario + the PATCH walking-skeleton both render the card with the indicator. |
| **B. Add `@nfr-ui-01` (edited indicator)** | New NFR tag for "edited indicator must render whenever `updated_at IS NOT NULL`". | Explicit traceability. Future-proofs against an "edited" indicator regression slipping past the suite. | New tag means new NFR row in the matrix. Per slice-2 precedent (coverage-matrix.md), NFR tags are reserved for *cross-cutting* quality attributes (perf, sec). A render-correctness assertion is a normal functional contract, not an NFR. |
| **C. Add `@nfr-sec-07` (admin authority)** | New NFR tag specifically for "admin can delete any comment, non-admin cannot". | Pins the admin-authority contract to a named NFR. | Same matrix-bloat concern as B. The admin-authority contract is already authorization, which `@nfr-sec-06` covers semantically. |

**Recommendation: A (reuse existing tags)**.

Rationale: (a) `@nfr-perf-03` already represents the 1s p99 fanout SLA;
the slice-5 CommentEdited / CommentDeleted realtime scenarios MUST hit
the same budget, so they are the same NFR cell, not a new one;
(b) `@nfr-sec-05` already covers "sanitizer must hold"; the edit re-render
inherits that contract by re-running `render_comment_markdown` per
ADR-007; (c) `@nfr-sec-06` already covers "authorization at every
endpoint"; the non-author 403 + admin-only delete-any scenarios are
exactly that surface. The "edited indicator render correctness" is a
functional contract, not an NFR — it shows up as a positive assertion
inside the PATCH walking skeleton ("comment card carries the 'edited'
marker"). No new tag.

---

## D2 — Walking-skeleton coverage for `GET …/comments/{id}/edit`

**Question**: does the inline edit-form fragment get its own
`@walking_skeleton` (real spawn_app + real Postgres + real htmx
fragment), or does it ride the PATCH walking skeleton (which itself
exercises the form round-trip)?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Standalone `@walking_skeleton` for GET edit-form** | A dedicated scenario "Author requests the inline edit-form fragment for their own comment" that opens spawn_app + posts a comment + GETs the edit-form URL + asserts the fragment shape (textarea with raw markdown, save/cancel buttons, correct action URL). Tagged `@walking_skeleton @real-io @driving_adapter`. | Maximum visibility — the GET edit-form endpoint is a distinct driving adapter and gets its own user-journey assertion. Aligns with the "every driving adapter has at least one WS scenario" rule (Mandate 6). | Adds ~150ms suite-time. Two walking skeletons in one slice (PATCH WS + GET WS) feels heavy for a "lit-up deferred AC" slice; slice-2 used 3 WS scenarios total across 3 features. |
| **B. Bundle GET under the PATCH WS** | The PATCH walking-skeleton scenario starts with a GET edit-form (asserts the form fragment exists and contains the raw markdown), then submits the PATCH and asserts the re-rendered card. Single `@walking_skeleton` scenario covers both endpoints. | Smaller suite-time (~200ms total for the joint scenario, vs ~150 + ~200 if separate). Tells the canonical user story end-to-end: "author clicks Edit, sees the form, submits, sees the updated card" — that IS the walking skeleton for the edit flow. | Slightly heavier single scenario (more Then steps). Some Mandate-6 reviewers may prefer one-driving-adapter-per-WS. |
| **C. Bundle PLUS a focused GET smoke** | B above, plus a separate non-WS `@real-io` "Edit-form fragment renders raw markdown source, not rendered HTML" scenario that pins the source-vs-rendered contract specifically. | Bundle covers the journey; smoke pins the easily-regressed "show raw markdown, not HTML" property. Both observable from one cheap GET. | Three closely-related scenarios feels redundant; the PATCH WS already asserts the textarea contains the raw markdown. |

**Recommendation: B (bundle GET under PATCH WS)**.

Rationale: (a) the slice-5 design says "the slice-5 PR ships author-edit
and admin-delete" — author-edit IS the end-to-end edit journey, which
naturally includes the GET edit-form + PATCH submit + render in one
scenario; (b) suite-time pressure (slice-3 already dropped concurrency
to 6 per `tests/acceptance.rs` line 90; further WS scenarios add
linearly to wall-clock); (c) Mandate 6 is satisfied — the GET
edit-form IS exercised via subprocess (the bundled WS GETs it before
the PATCH); driving-adapter coverage holds. If the slice-5 reviewer
flags single-WS-per-adapter, we can split in v0.2 with no test-body
changes.

---

## D3 — Cancel-edit handler URL shape

**Question**: the cancel path is a server round-trip — `GET
…/comments/{id}` should return the un-edited comment card fragment.
Does this URL reuse an existing handler ("show single comment" may not
exist yet) or get a thin new handler?

The DESIGN wave-decisions.md frames this as DISTILL+DELIVER joint
decision: DISTILL writes the test-side shape (what cancel asserts);
DELIVER picks the production-side handler.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Thin new GET `…/comments/{id}` handler** | DELIVER adds a fourth handler `show_single_comment` that returns the rendered comment card fragment for `id`. Slice-5 acceptance asserts: GET on the existing comment id returns 200 + a fragment containing the comment's body text + zero `<form>` elements (proves it's the card, not the form). | Conceptually clean — each htmx affordance gets a dedicated server-side rendering endpoint. RESTful (`GET /comments/{id}` is the natural resource). Matches the slice-2 fragment-per-affordance pattern. | New handler = new code surface (~30 LOC). Doubles the slice-5 GET endpoint count (edit-form + single-comment). Cancel is a rare flow, so this is over-investment for a marginal UX path. |
| **B. Reuse existing issue-detail page GET, htmx-target one comment** | The cancel button's `hx-get` targets `…/issues/{n}` with `hx-select="#comment-{id}"` (htmx's selective-DOM-pluck attribute). No new server handler — the existing issue-detail GET already renders all comment cards; htmx extracts the relevant one and swaps it in. | Zero new server code. Reuses an already-tested rendering path (the issue-detail page IS the slice-2 WS). | Bandwidth-wasteful — the cancel button GETs an entire issue page to extract one card. For a long issue thread (50 comments + attachments), this is multi-KB for what should be ~200 bytes. |
| **C. Defer cancel — show the original form-with-content, no server round-trip** | The cancel button is purely client-side: alpine.js (or htmx with `hx-on:click`) hides the edit form and re-shows the cached pre-edit card markup. No server round-trip. | Fastest UX (zero round-trip). | Adds an alpine.js / hx-on dependency to cancel. Doesn't match the slice-1 server-rendered-fragments pattern. Cached-markup approach is hard to test in cucumber-rs without a browser harness. |

**Recommendation: A (thin new GET handler), but accept the WS does NOT
cover cancel**.

Rationale: (a) cancel is a real user affordance; testing it via
subprocess + scraper assertion requires a server endpoint that returns
the card; (b) option B's "hx-select on the whole issue page" violates
the "minimise wire bandwidth" principle that slice-2 SSE fan-out
already paid attention to; (c) option C is client-only and unreachable
from cucumber-rs without `@manual`. The cost of A is ~30 LOC of
production handler + 1 small `@real-io` scenario (~100ms).

**Test-side proposal**: ONE non-WS `@real-io` scenario "Author cancels
the edit and the original card returns via server round-trip" that:
- GET edit-form (puts the form in the response body — but we discard
  it)
- GET `…/comments/{id}` (the cancel URL)
- Asserts the response is a comment-card fragment (contains the body
  text in rendered HTML), NOT a form (zero `<form>` elements, zero
  `<textarea>` elements)
- Latency budget: <100ms (single GET, no DB write)

This is a thin contract test for the cancel endpoint. The walking
skeleton (D2) does NOT cover cancel — the WS asserts the
save-edit-then-card-replaces-in-place flow, not cancel.

If the DELIVER crafter prefers B (reuse issue-detail page), the test
still passes — the assertion is "response is a card-shaped fragment
containing this comment's body", which both A and B satisfy. The
recommendation is for the production-side choice; the test is
crafted to admit both.

---

## D4 — 410-Gone htmx UX wording

**Question**: exact text for the "this comment was deleted, refresh to
see the latest state" fragment returned by the 410 handler.

This is pure UX copy. Three plausible drafts:

| Option | Draft text | Tone |
|---|---|---|
| **A. Terse** | `This comment has been deleted. Refresh to see the latest state.` | Direct, neutral. Matches the slice-2 `bad_request_fragment("Comment cannot be empty")` tone. |
| **B. Apologetic / context** | `This comment was removed. The thread has changed since you loaded the page — refresh to see what's there now.` | Friendly, explains the why. ~20% more text. |
| **C. Actionable affordance** | `This comment was deleted. <button hx-get="…/issues/{n}" hx-target="body">Refresh</button>` | Embeds a click-to-refresh button. Saves a manual page reload. |

**Recommendation: A (terse)**.

Rationale: (a) matches the existing slice-2 error-fragment tone
(`Comment cannot be empty`, `Comment is too long`); (b) keeps the
fragment small (a single `<p>` element fits in ~70 bytes); (c) doesn't
introduce a button handler the slice-5 PR would also need to test;
(d) the 410 status itself carries the semantic — the prose just
humanises it.

**Test-side**: the 410-Gone scenarios assert two things — the response
status is 410, and the response body contains the literal substring
`This comment has been deleted`. The exact wording can shift in a v0.2
copy-polish pass without re-touching the assertion (substring match,
not equality).

---

## D5 — Admin-undelete operator runbook recipe

**Question**: slice 5 does NOT ship a UI to undelete a tombstoned
comment, but the schema supports it (`UPDATE comments SET deleted_at
= NULL, deleted_by = NULL`). Is the operator runbook addition a
slice-5 deliverable or a v0.2 follow-up?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Ship in slice 5** | Add a 10-line section to `RELEASING.md` (or similar operator doc) with the exact `psql` recipe to undelete a comment. Tag in the slice-5 PR. | Closes the audit loop — the moderation story is "soft-delete is reversible by the operator", and that story isn't fully told without the recipe. | Slice 5 is already largest application extension since slice 2 (~340 LOC + scenarios per `wave-decisions.md` § Reuse Analysis); adding doc work expands scope. |
| **B. Defer to v0.2** | Slice 5 ships the schema (soft-delete columns); v0.2 adds the runbook recipe + possibly an admin-UI undelete affordance. Slice 5's `wave-decisions.md` § Constraints item 1 already records "v0.2 follow-up may introduce comments_visible VIEW" — same shelf. | Keeps slice 5 focused on the user-facing behaviour. The schema supports undelete regardless; an operator who needs it before v0.2 can write the `UPDATE` themselves (it's one line). | Risk that an operator hits the "Devansh deleted Mei's comment, Mei objects, how do I undo this?" scenario before v0.2 ships and has no documented recipe. Mitigation: the ADR-007 ("Comment Soft-Delete with Tombstone") already documents the schema affords undelete; a savvy operator can derive the UPDATE statement. |

**Recommendation: B (defer to v0.2)**.

Rationale: (a) slice 5 is already the largest extension since slice 2
(per `wave-decisions.md` Reuse Analysis ~340 LOC); adding docs work
adds review burden for marginal value; (b) the ADR-007 schema doc
already documents the undelete capability ("undelete is a single
UPDATE"); a literate operator can derive the recipe; (c) the natural
v0.2 follow-up bundles the runbook with the GC task (ADR-007's
"alternative C", deferred to v0.2 per same wave-decisions.md) — both
are operator concerns and ship together cleanly.

**Slice-5 outcome of this recommendation**: NOTHING ships in slice 5
for D5. The `wave-decisions.md` "Open Questions Resolved" table records
"v0.2 follow-up" for D5 and nothing more. No scenario, no doc, no
production code.

If the user overrides to A, the slice-5 finalize pass adds one
sentence to `crates/foundry-acceptance/tests/features/us-10-comment-edit-delete.feature`
header comment pointing to the runbook section, and writes the runbook
section to `docs/architecture/operator-runbook.md` (existing
slice-3 file). No new scenario; the runbook is documentation, not
behaviour.

---

## Pick-flag table — what to send back

When you reply, please pick one option per row. Default-accept means
"reply with nothing different and we ship the recommendation".

| ID | Question | Recommendation | Default-accept |
|---|---|---|---|
| D1 | NFR tag set for slice-5 scenarios | **A** (reuse existing `@nfr-perf-03`/`@nfr-sec-05`/`@nfr-sec-06`) | yes |
| D2 | Walking-skeleton coverage for GET edit-form | **B** (bundle GET under PATCH WS) | yes |
| D3 | Cancel-edit handler URL shape | **A** (thin new GET `…/comments/{id}`), test admits B too | yes |
| D4 | 410-Gone htmx UX wording | **A** (terse: "This comment has been deleted. Refresh to see the latest state.") | yes |
| D5 | Admin-undelete operator runbook recipe | **B** (defer to v0.2) | yes |

After your picks land, this DISTILL agent runs `execute --finalize`
with the picked options and:

1. Writes `wave-decisions.md` (DDD-numbered D1-D5)
2. Updates `features/us-10-comment-edit-delete.feature` if any pick
   changed the scenario shape (only D2 → A or D5 → A would force
   scenario adds; D1 / D3 / D4 changes only re-tag / re-word)
3. Writes `red-classification.md` after running the pre-DELIVER
   fail-for-right-reason gate
4. Marks `coverage-matrix.md` Definition-of-Done items complete

---

## Scope confirmation

The scenario plan from the user brief (7-9 scenarios in the new
`.feature` file) is what's being scaffolded. Concretely:

| # | Scenario title (working) | Tier | Tags |
|---|---|---|---|
| 1 | Comment author edits their own comment and the updated text replaces the original in the thread | WS | `@walking_skeleton @real-io @driving_adapter @us-10 @slice5 @comment-edit` |
| 2 | A non-author cannot edit someone else's comment | error | `@real-io @error @us-10 @slice5 @comment-edit @nfr-sec-06` |
| 3 | Workspace admin deletes any comment and remaining viewers see it disappear | functional | `@real-io @us-10 @slice5 @comment-delete @admin` |
| 4 | Comment author deletes their own comment | functional | `@real-io @us-10 @slice5 @comment-delete` |
| 5 | An open subscriber receives a CommentEdited event when another viewer edits | realtime | `@real-io @us-10 @slice5 @comment-edit @realtime @nfr-perf-03` |
| 6 | An open subscriber receives a CommentDeleted event when another viewer deletes | realtime | `@real-io @us-10 @slice5 @comment-delete @realtime @nfr-perf-03` |
| 7 | PATCH on an already soft-deleted comment returns 410 Gone with an htmx fragment | error | `@real-io @error @us-10 @slice5 @gone` |
| 8 | DELETE on an already soft-deleted comment returns 410 Gone | error | `@real-io @error @us-10 @slice5 @gone` |
| 9 | The issue page lists only non-deleted comments (soft-delete invariant) | invariant | `@real-io @us-10 @slice5 @soft-delete-invariant` |
| 10 (optional, conditional on D3 pick) | Author cancels the edit and the original card returns | functional | `@real-io @us-10 @slice5 @comment-edit @cancel` |

10 scenarios total — slightly above the 7-9 ceiling. Justifications:
- Scenarios 7 + 8 are both needed because PATCH and DELETE go through
  DIFFERENT handlers (one updates, the other tombstones); a single
  scenario can't cover both verbs' 410 behaviour.
- Scenario 10 is conditional on D3 = A; if D3 = B (reuse
  issue-detail page) or C (no server cancel), drop scenario 10 from
  the suite. Scaffolded with `@skip` until D3 lands.

If you want to drop to 7-9, the recommendation is to merge scenarios
7+8 (test both verbs in a single Scenario Outline with `<verb>`
parameter — Gherkin idiom that cucumber-rs supports). I haven't done
this in the scaffold because the existing slice-2 `.feature` files
prefer enumerated scenarios over Scenario Outlines (zero outlines in
slice 2). Tell me to switch if you want the smaller suite footprint.

---

## Suite-time delta (per slice-2 driver.md budget format)

| Scenario | Cost estimate | Notes |
|---|---|---|
| 1 PATCH WS (POST comment + GET edit-form + PATCH + GET issue page) | ~250ms | Three round-trips + one render check |
| 2 non-author 403 | ~150ms | POST as Mei + PATCH as Hiroshi + 403 |
| 3 admin delete | ~200ms | POST as Mei + DELETE as Devansh + GET issue page (verify card gone) |
| 4 author delete | ~150ms | POST as Mei + DELETE as Mei + GET issue page |
| 5 CommentEdited fanout | ~200ms | Open SSE + POST + PATCH + receive |
| 6 CommentDeleted fanout | ~200ms | Open SSE + POST + DELETE + receive |
| 7 PATCH on tombstone -> 410 | ~150ms | POST + DELETE + PATCH-on-tombstone -> 410 |
| 8 DELETE on tombstone -> 410 | ~150ms | POST + DELETE + DELETE-again -> 410 |
| 9 soft-delete invariant on list | ~150ms | POST + DELETE + GET issue page (verify zero cards) |
| 10 cancel (conditional) | ~100ms | POST + GET-cancel + assert card |
| **Subtotal (without #10)** | **~1.6s** | comfortably under the 20s slice-5 budget |
| **+ #10 if D3=A** | **~1.7s** | still well within budget |

For reference, slice-2 added ~7.7s. Slice-5 at ~1.7s is far smaller
because every scenario is fast (no SSE quiet-window waits, no
heartbeat scenarios, no NFR-PERF-03 10-issue burst).

---

## What I did NOT invent

Per the task brief's strict "DO NOT contradict the 5 invented-detail
flags" instruction, the scaffolded scenarios use exactly the production
shape pinned in `architecture.md`:

- Migration columns: `updated_at TIMESTAMPTZ NULL`, `deleted_at
  TIMESTAMPTZ NULL`, `deleted_by UUID NULL REFERENCES users(id)`
  (per architecture.md line 127-130). The tests assert no schema-shape
  details — they observe via the issue-page render + SSE event
  contents. So a v0.2 change to `ON DELETE SET NULL` does not red the
  tests.
- Partial index `idx_comments_issue_live` (line 135-136) — not
  observed by the tests at all; an implementation detail.
- `EventPayload.deleted: Option<bool>` (line 178-180) — the SSE
  fanout scenarios assert `event_type` (`CommentEdited` /
  `CommentDeleted`) and `comment_id`, not the `deleted` bool. A
  v0.2 change to enum doesn't red the tests.
- GET edit-form URL `…/comments/{id}/edit` (line 196 + line 104) —
  used directly in the WS scenario. If the user overrides to
  `…/comments/{id}?action=edit`, the test changes by 5 chars.
- htmx swap target `id="comment-{uuid}"` (line 196 + line 176-178) —
  used in the issue-page-render scraper assertions. If slice-2 used
  a different shape and slice-5 aligns, the test changes by 5 chars.
  This is what the wave-decisions.md "software-crafter aligns to the
  existing convention during GREEN" note refers to; the
  acceptance-designer flags this here so the user can pre-empt.
