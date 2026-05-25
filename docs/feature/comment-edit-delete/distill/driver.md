# DISTILL Driver Design — Slice 5 Acceptance Harness (Comment Edit/Delete)

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md`,
`coverage-matrix.md`, `wave-decisions.md`, `proposals.md`. This
document is an **additive delta** to:

- `docs/feature/foundry-backend-mvp/distill/driver.md` (slice 1)
- `docs/feature/foundry-realtime-collab/distill/driver.md` (slice 2 —
  SSE consumer + HTML assertions + heartbeat env override)
- `docs/feature/foundry-operator-grade/distill/driver.md` (slice 3 —
  multi-replica + backup-restore + attachments)
- `docs/feature/foundry-contributor-onboarding/distill/driver.md`
  (slice 4 — subprocess walking skeleton, readme inspect)

Everything not mentioned here is inherited unchanged.

## 1. What slice 5 reuses (zero new infrastructure)

| Adapter / helper | Reused from | Slice-5 use |
|---|---|---|
| `InProcHarness::spawn` + `spawn_app` | slice 1 `harness.rs` | All 10 slice-5 scenarios spawn an in-process axum on `127.0.0.1:0` with per-scenario PG schema |
| Per-scenario PG schema rotation | slice 1 (`fresh_schema_pool`) | Per-scenario `comments`, `users`, `teams`, `projects`, `issues` isolation |
| `signed_in_post` + cookie capture | slice 1 + slice-2 `us_10_comments::sign_in_and_capture_cookie` | Slice 5 step bodies follow the same sign-in dance (DELIVER copies the helper into `us_10_comment_edit_delete.rs` or extracts into `support/`) |
| CSRF middleware double-submit | slice 1 `csrf::csrf_middleware` | PATCH + DELETE inherit unchanged (per ADR-009) |
| `support/sse_client.rs` | slice 2 `driver.md` § 2a | Slice-5 realtime scenarios (#5, #6) open subscriptions via `open_sse_subscription` and wait via `subscription.wait_for(...)` |
| `support/html_assertions.rs` | slice 2 `driver.md` § 2b | Slice-5 issue-page assertions use `assert_comment_has_element_with_text`, `assert_comment_has_no_element`. May need ONE additional helper (`assert_comment_block_inner_text_contains` / `_does_not_contain`); DELIVER scoped or extracted. |
| Testcontainers Postgres-16 container | slice 1 | Same shared container; no new resource pressure |
| `support/heartbeat_env::override_heartbeat_ms` | slice 2 `driver.md` § 2c | Slice 5 does NOT exercise heartbeat (no quiet-window scenarios); the override remains available but unused |

## 2. What slice 5 adds to the harness

**Nothing new in `support/`.** Per the slice-5 DESIGN wave-decisions.md
§ Reuse Analysis, slice 5 introduces zero new ports, zero new adapters,
zero new external integrations. The acceptance harness mirrors this:
zero new files under `crates/foundry-acceptance/src/support/`.

The slice-5 work lands in exactly THREE existing-file edits + ONE new
step file:

1. NEW: `crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`
   (the step body file — scaffolded RED in DISTILL, filled in by
   DELIVER).
2. EDIT: `crates/foundry-acceptance/src/lib.rs` — append one line in
   the `pub mod steps { ... }` block to register the new module.
3. EDIT: `crates/foundry-acceptance/tests/acceptance.rs` — append one
   force-link `use foundry_acceptance::steps::us_10_comment_edit_delete
   as _us_10_edit;` next to the existing `_us_10` import.
4. EDIT: `crates/foundry-acceptance/src/world.rs` — append four
   `Option`/`HashMap`-typed fields under a new `// ---- US-10
   edit/delete (slice 5) ----` block at the bottom of the
   `FoundryWorld` struct (matching the slice-4 convention).

All four edits are test-infrastructure changes; production code is
untouched per the task brief.

## 3. World struct additions (`FoundryWorld`)

Slice 5 adds four fields. All default to empty `HashMap` or `None`;
existing slice-1-through-slice-4 scenarios are unaffected.

```rust
// ---- US-10 edit/delete (slice 5) ----
/// Map (issue_key_prefix, issue_number, author_email) -> comment_id
/// captured by the "previously posted a comment" Given. The matching
/// When step looks up the id to address PATCH/DELETE. Keyed by author
/// so a single scenario can hold both Mei's and Hiroshi's comments at
/// once (non-author-403 scenario).
pub us_10_5_last_comment_id_by_author: HashMap<(String, i32, String), uuid::Uuid>,
/// Map (issue_key_prefix, issue_number, body_substring) -> comment_id
/// captured by the "previously posted a comment" Given. Lets the
/// soft-delete-invariant scenario address a specific one of Mei's
/// two comments by body fragment (since both are by the same author,
/// the author-keyed map collapses them).
pub us_10_5_last_comment_id_by_body: HashMap<(String, i32, String), uuid::Uuid>,
/// Body of the most recent GET /comments/{id}/edit response. Cached
/// by the When step that requests it so multiple Then assertions on
/// the form fragment can share the same response body.
pub us_10_5_last_edit_form_body: Option<String>,
/// Raw markdown source of the most recently posted comment per
/// (issue_key_prefix, issue_number, author_email). Used by the
/// "textarea value is the raw markdown source" assertion in the
/// PATCH walking-skeleton scenario.
pub us_10_5_last_posted_body: HashMap<(String, i32, String), String>,
```

The slice-2 `us_10_last_issue_body` field is reused for the issue-page
re-fetch invalidation pattern: after a PATCH or DELETE, the step body
sets `world.us_10_last_issue_body = None` so the next Then-step
issue-page assertion re-GETs.

The slice-2 `us_09_last_event` field is reused for the SSE-event
assertions: scenarios 5 + 6 capture into the same slot the slice-2
`within …milliseconds Hiroshi observes a "CommentEdited" event …`
step writes to.

## 4. Step phrase contracts (slice-5 inventory)

Per `step-skeletons.md`. Slice 5 registers **NEW** phrases only — no
existing slice-2/3/4 phrase is touched. The new phrases:

### Givens (3 new)
- `^(\w+) has previously posted a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$`
- `^(\w+) has deleted (?:her|his) own comment on "(\w+)-(\d+)"$`

### Whens (8 new)
- `^(\w+) requests the edit form for (?:her|his) comment on "(\w+)-(\d+)"$`
- `^(\w+) submits an edit to (?:her|his) comment on "(\w+)-(\d+)" with body "([\s\S]*)"$`
- `^(\w+) submits an edit to (\w+)'s comment on "(\w+)-(\d+)" with body "([\s\S]*)"$`
- `^(\w+) submits an edit to (?:her|his) soft-deleted comment on "(\w+)-(\d+)" with body "([\s\S]*)"$`
- `^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)"$`
- `^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)" again$`
- `^(\w+) deletes (?:her|his) own "([\s\S]+)" comment on "(\w+)-(\d+)"$`
- `^(\w+) deletes (\w+)'s comment on "(\w+)-(\d+)"$`
- `^(\w+) cancels the edit on (?:her|his) comment on "(\w+)-(\d+)"$`

### Thens (10 new)
- `^the response is an htmx fragment containing a textarea whose value is the raw markdown source of (?:her|his) comment$`
- `^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) with an "edited" indicator$`
- `^the issue page for "(\w+)-(\d+)" does NOT show a comment by (\w+) containing the text "([\s\S]+)"$`
- `^the issue page for "(\w+)-(\d+)" still shows a comment by (\w+) containing the text "([\s\S]+)"$`
- `^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing the text "([\s\S]+)"$`
- `^the issue page for "(\w+)-(\d+)" no longer shows a comment by (\w+)$`
- `^the response status is 200$`
- `^the response status is 410$`
- `^the response is an htmx fragment containing the text "([\s\S]+)"$`
- `^the response is an htmx fragment that does NOT contain a <(\w+)> element$`
- `^the event payload's comment author email is "([^"]+)"$`

### Inherited (reused unchanged from slice 2)

- `^(\w+) has an open subscription to events on "([^"]+)"$` (us_09_realtime_sse.rs)
- `^within (\d+) milliseconds (\w+) observes an? "([^"]+)" event for "(\w+)-(\d+)" on "([^"]+)"$` (us_09_realtime_sse.rs — pattern matches CommentEdited / CommentDeleted because event_type is captured as `\w+`)
- `^the response status is 403$` (us_10_comments.rs)
- `^the response is an htmx fragment containing "([^"]+)"$` (slice-1)
- Background phrases for workspace/member/team/project/issue/sign-in seeding (slice-1 modules; reused verbatim — see slice-2 `step-skeletons.md` § "Background" for the full list).

cucumber-rs treats step phrases as globally unique; the new phrases
above were verified non-colliding by compile (`cargo check -p
foundry-acceptance --tests` passes) + by the slice-5 scenario run (all
60 background steps registered and resolved against existing handlers).

## 5. Per-scenario isolation — unchanged

The slice-1 invariant holds: per-scenario PG schema, shared container.
Slice 5 introduces no new resource. The two realtime scenarios (5 + 6)
open SSE subscriptions that are torn down via the slice-2 `_shutdown`
oneshot at scenario end.

## 6. Real-I/O budget — slice 5 adds ~1.7s on top of slice 4

Per `proposals.md` § "Suite-time delta":

| Scenario | Cost estimate | Notes |
|---|---|---|
| 1 PATCH WS (POST + GET edit-form + PATCH + GET issue page) | ~250ms | 3 RT + 1 render |
| 2 non-author 403 | ~150ms | Mei POST + Hiroshi PATCH-403 |
| 3 admin DELETE | ~200ms | Mei POST + Devansh DELETE + GET issue page |
| 4 author DELETE | ~150ms | Mei POST + Mei DELETE + GET issue page |
| 5 CommentEdited realtime | ~200ms | Open SSE + POST + PATCH + receive |
| 6 CommentDeleted realtime | ~200ms | Open SSE + POST + DELETE + receive |
| 7 PATCH on tombstone -> 410 | ~150ms | POST + DELETE + PATCH-tombstone |
| 8 DELETE on tombstone -> 410 | ~150ms | POST + DELETE + DELETE-again |
| 9 soft-delete invariant on list | ~150ms | 2x POST + DELETE + GET issue page |
| 10 cancel (conditional on D3 = A) | ~100ms | POST + GET-cancel |
| **Total (slice 5)** | **~1.7s** | well within 20s ceiling |

After slice 5, total suite wall-clock projects to ~30-35s, still under
the 60s top-line budget from slice-1 driver.md § 7. No CI sharding
needed.

## 7. Tag conventions (additions only)

Inherited (unchanged): see `wave-decisions.md` § "Tag conventions
added".

Added in slice 5:
- `@slice5`, `@comment-edit-delete`, `@comment-edit`, `@comment-delete`,
  `@admin`, `@gone`, `@soft-delete-invariant`, `@cancel`.

`@realtime` is reused from slice 2 (now appears on 2 slice-5 scenarios
in addition to slice-2's 2 scenarios).
`@nfr-perf-03`, `@nfr-sec-06`, `@us-10` are reused unchanged.

## 8. CI invocation (delta only)

The slice-1/2/3 invocations stay as-is. The slice-5 scenarios pick up
automatically because they live under the same feature-files root.
The `--max-concurrent-scenarios 6` cap from slice 3 holds (slice 5
adds no PG-contention-sensitive scenarios; each scenario is one
round-trip per verb).

Local fast loop for slice-5-only iteration:

```bash
cargo test -p foundry-acceptance --test acceptance -- -t "@slice5"
```

Add `--retry 1` if a realtime scenario flakes under load (the slice-2
arrival-latency assertion already tolerates 2s ceiling per
`coverage-matrix.md` row; under macOS Docker pressure the SSE
keep-alive path may need a one-shot retry — observed in slice-3 work).

## 9. Standing rules carried into DELIVER (additions)

- Every PATCH/DELETE/GET-edit-form scenario's step body MUST sign in
  as the actor under test BEFORE issuing the verb. The slice-2 helper
  `sign_in_and_capture_cookie` in `us_10_comments.rs` is the pattern;
  DELIVER may copy or extract it (suggest: extract to
  `support/auth_helper.rs` to dedupe across slice 2 and slice 5 —
  optional, not required).
- The CSRF token for DELETE rides the `HX-CSRF` header (per ADR-009).
  DELIVER must NOT send `_csrf` as a form field for DELETE; the body
  is empty.
- The "edited" indicator assertion is structural (an element with a
  marker class or text inside the comment block), not pixel-perfect.
  Reviewers should accept any rendering that the scraper assertion
  pins. DELIVER picks the literal marker (suggest `.comment-edited`
  class or a trailing ` (edited 2 minutes ago)` `<small>` element;
  the slice-5 .feature file does not pin the choice).
- The 410-Gone body assertion uses substring match (`This comment has
  been deleted`), NOT equality, so a future copy-polish pass can
  rewrite the wording without red-ing the slice-5 suite.
