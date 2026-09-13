# RED classification — issue-card-delete

Gate output of the pre-DELIVER *fail-for-the-right-reason* check. Produced by
running the `@icd` lane with `@pending` temporarily lifted, then restoring it.
DELIVER reads this file at PREPARE to confirm the RED is genuine.

**Run**: `FOUNDRY_ACCEPTANCE_TAGS=icd cargo test -p foundry-acceptance --test acceptance`  
**Result**: 34 scenarios — **29 RED**, **5 expected-GREEN**, **0 BROKEN**.

**Verdict: PASS.** Zero BROKEN and zero false-GREEN. Every failure reaches its
own assertion and fails because the behaviour is missing; every pass exercises
shipped behaviour and is a regression guard, not a feature assertion.

## Why this gate mattered here

The first run was **35/35 wrong-RED** and the gate blocked it. Three defects in
the tests themselves surfaced, none of which a status-code assertion would have
shown:

1. **SETUP_FAILURE (35 scenarios)** — the Background seeded `workspaces (id, name, slug)`; that table has no `slug` column (`0001_init.sql:8`). No scenario reached an assertion.
2. **SETUP_FAILURE (30 scenarios)** — the lane seed used a bare `ON CONFLICT DO NOTHING`, letting Postgres pick `UNIQUE (project_id, position)` as arbiter. That constraint is `DEFERRABLE` and cannot arbitrate. The very keyword `brief.md` calls load-bearing for lane arrangement bit a test seed; the fix names the non-deferrable `(project_id, slug)` explicitly.
3. **BROKEN — ambiguous steps (10 scenarios)** — cucumber-rs registers every module's steps in ONE global registry, so a step phrase is a *workspace-wide* name. Three phrases collided with `feature_board_lane_reorder` and `feature_board_lane_overflow_menu`. Renamed here, not there.

Four **false-GREEN** oracles were then found and strengthened — each passed only
because nothing had happened yet:

| Scenario | Was vacuous because | Paired with |
|---|---|---|
| The cards beside it are untouched | siblings survive trivially when no delete occurs | `nothing of AUTH-42 remains anywhere` |
| Deleting a card is not a change to the board's lanes | lanes are trivially unchanged when no delete occurs | `nothing of AUTH-42 remains anywhere` |
| Another project's board hears nothing | trivially true while nothing is announced anywhere | `exactly one card was announced as deleted` |
| A refused delete announces nothing | trivially true while no delete announces anything | `the refusal is byte-identical to a card that never existed` |

A fifth was **vacuous by construction**: byte-identical refusal comparison passed
because *both* sides returned the same scaffold `501`. The oracle now also asserts
the status is the uniform `404` (DDD-9) — strictly stronger, correct once shipped,
honestly red until then. And one scenario was a **duplicate** ("a delete that
carries no token announces nothing" restated its US-ICD-01 sibling) and was removed —
the carpaccio *merge them* taste test applied to scenarios.

One oracle was **wrong, not missing**: the AC-2.8 regression asserted the response
body carried the new title, which pinned the harness's `HX-Request` choice rather
than the behaviour. It now asserts the persisted title plus the re-rendered board.

## RED — 29 scenarios

| Scenario | Fails at | Classification |
|---|---|---|
| The confirm names the issue and counts what goes with it | expected the delete confirm dialog (data-modal="delete-issue"); got 75 bytes not carrying it | MISSING_FUNCTIONALITY |
| The confirm says so when nothing goes with the issue | expected the delete confirm dialog (data-modal="delete-issue"); got 75 bytes not carrying it | MISSING_FUNCTIONALITY |
| Confirming removes the card and returns her to the board | expected to land on the board, got 67 bytes at status 501 Not Implemented | MISSING_FUNCTIONALITY |
| Everything hanging off the card goes with it | assertion `left == right` failed: 1 row(s) of AUTH-42 survived in issues | MISSING_FUNCTIONALITY |
| The cards beside it are untouched | assertion `left == right` failed: 1 row(s) of AUTH-42 survived in issues | MISSING_FUNCTIONALITY |
| Deleting a card is not a change to the board's lanes | assertion `left == right` failed: 1 row(s) of AUTH-42 survived in issues | MISSING_FUNCTIONALITY |
| A card filed by someone else can be deleted by any team member | assertion `left == right` failed: board renders ["AUTH-41", "AUTH-42", "AUTH-43"], expected exa | MISSING_FUNCTIONALITY |
| A comment filed while the confirmation is open goes with the issue | assertion `left == right` failed: 1 row(s) of AUTH-42 survived in issues | MISSING_FUNCTIONALITY |
| Deleting a card on a board she is not a member of is refused indistinguishably | assertion `left == right` failed: a refusal must be the uniform non-enumerable 404 (DDD-9) — ne | MISSING_FUNCTIONALITY |
| A non-member is refused at the confirmation as well as at the delete | assertion `left == right` failed: a refusal must be the uniform non-enumerable 404 (DDD-9) — ne | MISSING_FUNCTIONALITY |
| Deleting a card that is already gone is refused indistinguishably | the pre-delete Given must succeed, got 501 Not Implemented — body <!-- __SCAFFOLD__ issue-card- | MISSING_FUNCTIONALITY |
| Deleting a card that never existed is refused the same way | assertion `left == right` failed: a refusal must be the uniform non-enumerable 404 (DDD-9) — ne | MISSING_FUNCTIONALITY |
| The whole path works with scripting switched off | a plain delete link must exist with scripting off (DDD-6): browser NoSuchEl — control not built | MISSING_FUNCTIONALITY |
| The confirmation is a readable page when scripting is switched off | a plain delete link must exist with scripting off (DDD-6): browser NoSuchEl — control not built | MISSING_FUNCTIONALITY |
| The card's own dialog offers Delete where it cannot be mistaken for Save | the edit dialog must offer a Delete control (AC-2.1) | MISSING_FUNCTIONALITY |
| Choosing Delete asks before it acts | the edit dialog must offer a Delete control (AC-2.1); it does not | MISSING_FUNCTIONALITY |
| Choosing Delete does not save a half-typed edit | the edit dialog must offer a Delete control (AC-2.1); it does not | MISSING_FUNCTIONALITY |
| Confirming from the board removes the card without leaving the board | the edit dialog must offer a Delete control (AC-2.1); it does not | MISSING_FUNCTIONALITY |
| The refreshed board keeps every other card and every lane operation | the edit dialog must offer a Delete control (AC-2.1); it does not | MISSING_FUNCTIONALITY |
| Backing out of the confirmation leaves the card alone | the edit dialog must offer a Delete control (AC-2.1); it does not | MISSING_FUNCTIONALITY |
| A real browser carries the delete through | the popup must offer Delete (AC-2.1): browser NoSuchElement — control not built | MISSING_FUNCTIONALITY |
| A non-member deleting from the board is refused indistinguishably | assertion `left == right` failed: a refusal must be the uniform non-enumerable 404 (DDD-9) — ne | MISSING_FUNCTIONALITY |
| A deleted card leaves a board someone else is looking at | the second window still shows AUTH-42 after the delete — a card that is visible but gone is the | MISSING_FUNCTIONALITY |
| The announcement names the card that went | assertion `left == right` failed: expected exactly one IssueDeleted row; a missing one and a sp | MISSING_FUNCTIONALITY |
| Another project's board hears nothing | assertion `left == right` failed: expected exactly one IssueDeleted row; a missing one and a sp | MISSING_FUNCTIONALITY |
| A refused delete announces nothing | assertion `left == right` failed: a refusal must be the uniform non-enumerable 404 (DDD-9) — ne | MISSING_FUNCTIONALITY |
| Comments on other cards still arrive alongside | assertion `left == right` failed: the delete must be announced once | MISSING_FUNCTIONALITY |
| Emptying a lane by deleting it announces every card that went | assertion `left == right` failed: a lane delete with fate=delete must announce every card it de | MISSING_FUNCTIONALITY |
| A second board loses every card of a lane that was deleted | the second window still shows AUTH-41 after the delete — a card that is visible but gone is the | MISSING_FUNCTIONALITY |

## Expected-GREEN — 5 scenarios

These are regression guards over **shipped** behaviour, not feature assertions.
Each would go red if DELIVER broke something it must not touch.

| Scenario | Green because | Guards |
|---|---|---|
| A signed-out delete is refused the same way | the scaffold implements the signed-out guard (`redirect_to("/sign-in")`), the house idiom applied to the new route | that DELIVER keeps the guard when it replaces the scaffold |
| A delete that carries no token is refused before anything is written | the shipped `csrf_middleware` covers the new route by **registration alone** — no per-route work (ADR-009) | that the route stays mounted INSIDE the CSRF layer; it goes red the moment it is not |
| Saving an edit from the card's dialog still works exactly as before | edit-and-save is shipped and untouched | AC-2.8 — that adding a Delete control outside the form does not disturb the form |
| Moving a lane's cards still announces them as moved, never as deleted | the lane `move` fate already emits `IssueUpdated` per card | AC-3.9 — that DDD-2's amendment to ADR-BOARD-LANE-002 stays ONE clause |
| A lane delete that is refused still announces nothing | the shipped last-lane refusal works today | AC-3.10 — that the refusal, the lock discipline and the bounded retry survive the rewiring |

## Restoring `@pending`

`@pending` was lifted only for this run and is restored on the `Feature:` line.
The default, `all` and tag lanes all exclude `@pending`, so the suite is green for
everyone else. DELIVER un-pends one scenario at a time and never re-authors.
