# RED Classification — Feature B "htmx Web Tier" (pre-DELIVER gate)

Per `nw-distill` §"Pre-DELIVER fail-for-the-right-reason gate". DELIVER reads
this at the RED-phase entry gate (ADR-025) to confirm RED is genuine.

## How this was produced

Ran the full browser lane against the real in-process harness + a real
testcontainers Postgres (Docker up):

```bash
cargo test -p foundry-acceptance --test acceptance
```

(Default lane — excludes `@docker-compose`/`@manual`/`@slow`; includes all
Feature B in-process scenarios.) To run JUST the htmx-2 / slow set use
`FOUNDRY_ACCEPTANCE_TAGS=all`.

## Result

```
[Summary]
157 scenarios (148 passed, 9 failed)
1335 steps (1326 passed, 9 failed)
```

- **148 passed** includes ALL previously-green scenarios (the existing
  board/comment/sign-in regression net is UNAFFECTED — NFR-WEBB-COMPAT-01) PLUS
  the 14 Feature B regression-guard scenarios that assert UNCHANGED behaviour
  (authz affordance gating, non-enumerable error, CSRF contract, sanitization,
  author/body/edited render, the data-* markers, the post/edit/file
  interactions).
- **9 failed** — exactly the genuine user-visible deltas, ALL in `us-b0*`
  feature files. **Zero existing scenarios regressed.**

## Per-scenario classification (the 9 RED)

| # | Scenario (feature:line) | Failure | Class |
|---|---|---|---|
| 1 | us-b01:47 WS — board links vendored stylesheet/scripts | `board page links no vendored /static stylesheet … RED until DELIVER moves it onto base.html` | **MISSING_FUNCTIONALITY** ✅ |
| 2 | us-b01:57 — empty board inviting empty state | `empty board shows no file-the-first-issue guidance` (today: bare `No issues yet`) | **MISSING_FUNCTIONALITY** ✅ |
| 3 | us-b01:73 @error — render failure → clean 500 | `expected a clean 500 from a failed render, got 200 OK` | **MISSING_FUNCTIONALITY** ✅ (render-failure seam unwired) |
| 4 | us-b02:35 WS — vendored htmx/Alpine/CSS served | `vendored asset was not served (got 404) … static/ is empty + /static route unmounted` | **MISSING_FUNCTIONALITY** ✅ |
| 5 | us-b02:46 — htmx asset is non-empty | `vendored htmx script is empty or stub-sized (0 bytes)` | **MISSING_FUNCTIONALITY** ✅ |
| 6 | us-b03:46 WS — live card == reloaded card affordances | `live card actions=false, reloaded card actions=true; the live OOB card must carry the SAME .comment-actions` | **MISSING_FUNCTIONALITY** ✅ — **THE bug fix** (`comments.rs:841` omits OOB affordances) |
| 7 | us-b04:40 WS — sign-in links vendored stylesheet | `sign-in page links no vendored /static stylesheet … RED until DELIVER moves it onto base.html` | **MISSING_FUNCTIONALITY** ✅ |
| 8 | us-b04:65 — forgot links vendored stylesheet | `forgot-password page links no vendored /static stylesheet …` | **MISSING_FUNCTIONALITY** ✅ |
| 9 | us-b05:38 WS — served htmx asset is version 2 | `served htmx asset does not report a 2.x version (it is unvendored/htmx-1 today)` | **MISSING_FUNCTIONALITY** ✅ |

**All 9 are category-1 MISSING_FUNCTIONALITY** (the assertion fires because the
behaviour is unimplemented). Zero `IMPORT_ERROR` / `FIXTURE_BROKEN` /
`SETUP_FAILURE` (category 2), zero `WRONG_ASSERTION` / `OBSERVABLE_NOT_AT_PORT`
(category 3). The Background + Given steps all succeed (the harness, seeding,
sign-in, and comment-post wiring are real and green) — the failure is in the
behaviour, not the fixture.

**Gate verdict: PASS — RED is genuine; handoff to DELIVER is unblocked.**

## Two test-bugs found and fixed during this gate (wrong-RED → corrected)

The first gate run surfaced 12 failures including 3 wrong-RED (category 2/3),
which were FIXED before this final classification (the gate working as designed):

1. **Edit-comment form field** — the edit step posted `body` but the
   `EditCommentForm` (comments.rs:48) expects `body_markdown`; the PATCH 422'd
   (`missing field body_markdown`) so the comment never re-rendered. Fixed the
   step to post `body_markdown`. (Affected the US-B03 edited-marker + US-B05
   post-and-edit scenarios — now GREEN regression guards.)
2. **Admin team-membership** — the admin-affordance scenario signed Devansh
   (workspace admin) in, but the issue page is team-membership-gated, so he got
   "Not a team member". Added a Feature-B Given (`the workspace admin Devansh
   also belongs to the Backend team`) that adds ONLY the team membership for the
   existing admin user (a plain precondition row). Now GREEN.

After the fix: 9 failures, all category-1. The fixes are committed in the step
module; this is the audit trail of the gate doing its job (`nw-distill`
§"Why this gate matters").

## Reproduce

```bash
cd /Users/jeffbailey/Projects/foss/leading/foundry
cargo test -p foundry-acceptance --test acceptance 2>&1 | grep -E '\[Summary\]|Step panicked'
```

Expect: `157 scenarios (148 passed, 9 failed)`; the 9 panics are the table
above. (Requires Docker for the testcontainers Postgres.)
