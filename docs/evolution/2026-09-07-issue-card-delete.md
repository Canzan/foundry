# issue-card-delete — evolution archive

Delete an issue from either surface that opens it: the edit popup htmx swaps into
`#modal-root`, and the full issue page. One confirm dialog, one write port, one
meaning of "deleted".

Waves: DISCUSS → DESIGN → DISTILL → DELIVER (no DISCOVER, no DEVOPS, no SPIKE).
Delivered in **no-commit mode** — nothing was committed or pushed.

## What shipped

- A `GET` confirm dialog + `POST` confirm at `…/issues/{n}/delete`, deliberately **not** an HTTP `DELETE` verb, so the whole path works with scripting disabled.
- One store primitive, `issue_delete::delete_issues_with_outbox`, that **both** issue-delete callers route through — the single-card path and the lane `DeleteCards` fate.
- Every authz refusal in the HTML adapter converged on the uniform non-enumerable `404`; `non_member_page` deleted in all five copies.
- `board-live.js` — foundry's **first browser-side live-update surface**.
- 34 acceptance scenarios; full lane 632/632.

## The five things worth remembering

### 1. The requested approach was wrong, and the codebase said so before any code was written.

The ask was to follow `comment-edit-delete` — a soft tombstone. Prior Wave
Consultation found that issues **already** hard-delete through the shipped lane
fate (`registry.yaml` OUT-4), and that `board-lane-overflow-menu` D1 had explicitly
declined archive because it "would create a second way for a card to be invisible".
A tombstone here would have been exactly that. The comment tombstone exists to
preserve a comment's *position in a thread*; an issue holds no such position, so the
precedent's own rationale did not transfer.

Reading the SSOT before writing the stories is what turned a plausible instruction
into the right decision.

### 2. `non_member_page` leaked twice, and only a converged delete exposed it.

It answered **403** *and* named the team in its body. The status separated "this team
exists but is not yours" from "no such team"; the body confirmed existence outright.
DISCUSS AC-1.8 required a uniform 404 for the non-member case; DESIGN's DDD-9
contradicted it by saying "match the module's own idiom". DISTILL encoded AC-1.8, and
step 01-05's RED run printed `left: 403  right: 404`.

The user then chose to converge **upward** — 20 call sites across 5 modules, and the
leaking helper deleted rather than left as a trap for the next feature to reach for.
NFR-SEC-06 survived intact: it requires the auth check to *exist and refuse*; the 403
was only its exemplar observable.

### 3. Three waves assumed a capability the application never had.

AC-3.4 said a second browser drops the deleted card. The journey had Priya glance at
her other monitor. DESIGN verified `EventPayload` already declared every field
`IssueDeleted` needed — true, and beside the point.

**foundry shipped no client-side SSE consumer at all.** The `/events` endpoint, the
outbox → LISTEN → broadcast → SSE topology and its server-side subscribers were all
green; nothing in `static/js/` had ever opened an `EventSource`. DISTILL's oracle
polled a document with no mechanism to change, and stayed `@pending` long enough that
it never ran red until step 03-01.

Caught not by review but by a crafter trying to make a test pass and finding nothing
to make it pass *with*.

### 4. An oracle whose message asserts more than its code does is worse than no oracle.

`then_looking_at_board` read `body.contains(expected) || body.is_empty()` while its
panic said "the redirect must target {expected}". `redirect_to` returns a **303 with an
empty body**, so the second arm was always true and the destination was never checked.
D8 — "303 to the board, never a re-render of a page for a resource that no longer
exists" — had no working guard for the whole delivery.

Proven by sabotage, not by inspection: pointing the redirect at `/sign-in` left the old
oracle **passing 15/16**. The fixed oracle caught it in both lanes. The same pass found
that the browser-lane assertion was additionally *racy* — `fantoccini`'s `click()`
returns on dispatch, not on navigation — a flake the weak assertion had been hiding.

### 5. The lane fate had been silently lying to second viewers since it shipped.

`delete_lane_with_fate`'s `DeleteCards` arm destroyed N cards and announced nothing —
recorded in `lanes.rs` as deliberate "parity with `delete_issue_cascade`, which emits
nothing". That parity was correct when written; `delete_issue_cascade` was dead code
with zero callers, and this feature's primitive replaced it. Routing both callers
through one function made the parity run the other way. ADR-BOARD-LANE-002 is amended
in exactly one clause, and its transaction — `FOR UPDATE` lock, last-lane gate,
membership snapshot, FK strand-guard, ≤3 bounded retry — came through untouched.

## Numbers

| | |
|---|---|
| Acceptance scenarios | 34 (`@icd`), 0 `@pending` |
| Full default lane | 632/632, 4387 steps |
| DELIVER steps | 12, all `GREEN=PASS`; 3 blocked-then-resolved |
| DES events | 63, integrity `exit 0` |
| Mutation | 7 viable on store+services, 7 killed (100%); app-handler layer unmeasured |
| Migrations added | **0** — schema head still `0015_project_lanes.sql` |
| New `EventPayload` fields | **0** — `schema_version` still 1 |
| JavaScript added | 1 file, 110 lines, 0 dependencies |

## Artifacts

- `docs/feature/issue-card-delete/feature-delta.md` — the 4-wave record
- `docs/product/architecture/adr-issue-delete-001-one-hard-delete-primitive.md`
- `docs/product/architecture/adr-issue-delete-002-get-post-confirm-not-delete-verb.md`
- `docs/product/architecture/brief.md` § "An issue has one delete"
- `docs/product/journeys/journey-issue-card-delete.yaml`, `jobs.yaml` § `job-issue-card-delete`
- `registry.yaml` OUT-11 / OUT-12; OUT-4 amended

## Successors

- **Converge the three destructive-action shapes.** Comment delete uses the `DELETE` verb; lane delete is `GET`+`POST` htmx-only; issue delete is `GET`+`POST` with both carriers. ADR-ISSUE-DELETE-002 makes the third the intended target without converging the first two.
- **A general live board.** `board-live.js` handles `IssueDeleted` alone. Cards appearing, moving and retitling in place is a feature in its own right.
- **`data-events-url` on `#board-columns`.** The consumer derives its endpoint by stripping `/report` off a link href. Doing it properly needs the attribute in both `board.html` and the OOB envelope, kept in sync — deferred deliberately.
- **A `check-arch` rule pinning the one-primitive invariant.** Nothing currently stops a third caller writing its own `DELETE FROM issues`.
- **Mutation coverage for the app-handler layer**, which needs an oracle cheaper than the full acceptance lane.
