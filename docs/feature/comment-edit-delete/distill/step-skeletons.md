# Step Skeletons — Slice 5 (Comment Edit/Delete)

Cucumber-rs step signatures the DELIVER wave fills in. Live in
`crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs` —
this slice-5 work is ADDITIVE; slice-2's `us_10_comments.rs` is NOT
modified.

Step-method bodies are scaffolded RED with
`panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER
finishes this")` per nw-distill § "Mandate 7" (Rust adaptation per the
polyglot matrix — `panic!` is the Rust scaffold idiom that the
cucumber-rs runner classifies as `RED (MISSING_FUNCTIONALITY)`, not
`BROKEN`).

Step-method names follow the slice-1+2+3+4 style: `fn given_*`,
`fn when_*`, `fn then_*` — see `crates/foundry-acceptance/src/steps/us_10_comments.rs`
for tone. The slice-5 file uses `(?:her|his)` regex alternation so
the same phrase reads idiomatically for any persona.

## Background — inherited unchanged from slice 1 + slice 2

These phrases are defined in slice-1/2 step files; slice 5 features
call them verbatim and do not redefine them.

```rust
// us_05_bootstrap.rs (or shared via support/)
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(...);

// us_07_project_create.rs (slice 1)
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(...);
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(...);

// us_06_signin.rs (slice 1)
#[given(regex = r"^(\w+) is signed in$")]
async fn member_is_signed_in(...);

// us_08_file_issue.rs (slice 1)
#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(...);

// us_09_realtime_sse.rs (slice 2)
#[given(regex = r#"^(\w+) has an open subscription to events on "([^"]+)"$"#)]
async fn member_has_open_subscription(...);

#[then(regex = r#"^within (\d+) milliseconds (\w+) observes an? "([^"]+)" event for "(\w+)-(\d+)" on "([^"]+)"$"#)]
async fn member_observes_event_within(...);
// ^ Reused for "CommentEdited" / "CommentDeleted" because the event_type
//   group is `\w+` — no need to redefine for slice 5.

// us_10_comments.rs (slice 2)
#[then(regex = r"^the response status is 403$")]
async fn status_403(...);
```

## World additions

`crates/foundry-acceptance/src/world.rs` — append AFTER the slice-4
US-13 block (matching the slice-4 convention from
`docs/feature/foundry-contributor-onboarding/distill/step-skeletons.md`
line 12).

```rust
// ---- US-10 edit/delete (slice 5) ----
/// Map (issue_key_prefix, issue_number, author_email) -> comment_id
/// captured by the "previously posted a comment" Given. The matching
/// When step looks up the id to address PATCH/DELETE.
pub us_10_5_last_comment_id_by_author: HashMap<(String, i32, String), uuid::Uuid>,
/// Map (issue_key_prefix, issue_number, body_substring) -> comment_id
/// captured by the "previously posted a comment" Given. Lets the
/// soft-delete-invariant scenario address one of Mei's two comments
/// by body fragment.
pub us_10_5_last_comment_id_by_body: HashMap<(String, i32, String), uuid::Uuid>,
/// Body of the most recent GET /comments/{id}/edit response, cached
/// for multiple Then assertions.
pub us_10_5_last_edit_form_body: Option<String>,
/// Raw markdown source of the most recently posted comment per
/// (issue_key_prefix, issue_number, author_email). Drives the
/// "textarea value is the raw markdown source" assertion.
pub us_10_5_last_posted_body: HashMap<(String, i32, String), String>,
```

Slice-2 `us_10_last_issue_body` is reused as the issue-page-fetch
cache invalidation slot (Whens set it to `None` so the next Then
re-GETs).

Slice-2 `us_09_last_event` is reused for the realtime CommentEdited /
CommentDeleted assertions.

## Step force-link

`crates/foundry-acceptance/tests/acceptance.rs` — append next to the
existing `_us_10` import:

```rust
#[allow(unused_imports)]
use foundry_acceptance::steps::us_10_comment_edit_delete as _us_10_edit;
```

`crates/foundry-acceptance/src/lib.rs` — append next to
`pub mod us_10_comments;`:

```rust
pub mod us_10_comment_edit_delete;
```

## Step signatures (the slice-5 contract DELIVER fills in)

Full Rust source with attribute macros + DELIVER implementation
outlines is the SSOT file
`crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`.
The signatures below mirror that file for review convenience.

### Givens

```rust
#[given(regex = r#"^(\w+) has previously posted a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn given_member_previously_posted_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
    body: String,
);

#[given(regex = r#"^(\w+) has deleted (?:her|his) own comment on "(\w+)-(\d+)"$"#)]
async fn given_member_has_deleted_own_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
);
```

### Whens

```rust
#[when(regex = r#"^(\w+) requests the edit form for (?:her|his) comment on "(\w+)-(\d+)"$"#)]
async fn when_member_requests_edit_form(...);

#[when(regex = r#"^(\w+) submits an edit to (?:her|his) comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn when_member_submits_edit_to_own_comment(...);

#[when(regex = r#"^(\w+) submits an edit to (\w+)'s comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn when_non_author_submits_edit_to_others_comment(...);

#[when(regex = r#"^(\w+) submits an edit to (?:her|his) soft-deleted comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn when_member_submits_edit_to_soft_deleted_comment(...);

#[when(regex = r#"^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)"$"#)]
async fn when_member_deletes_own_comment(...);

#[when(regex = r#"^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)" again$"#)]
async fn when_member_deletes_own_comment_again(...);

#[when(regex = r#"^(\w+) deletes (?:her|his) own "([\s\S]+)" comment on "(\w+)-(\d+)"$"#)]
async fn when_member_deletes_own_comment_by_body(...);

#[when(regex = r#"^(\w+) deletes (\w+)'s comment on "(\w+)-(\d+)"$"#)]
async fn when_admin_deletes_others_comment(...);

#[when(regex = r#"^(\w+) cancels the edit on (?:her|his) comment on "(\w+)-(\d+)"$"#)]
async fn when_member_cancels_edit(...);  // conditional on D3 = A
```

### Thens

```rust
#[then(regex = r#"^the response is an htmx fragment containing a textarea whose value is the raw markdown source of (?:her|his) comment$"#)]
async fn then_response_textarea_with_raw_markdown(...);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) with an "edited" indicator$"#)]
async fn then_issue_page_comment_has_edited_indicator(...);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" does NOT show a comment by (\w+) containing the text "([\s\S]+)"$"#)]
async fn then_issue_page_no_comment_with_text(...);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" still shows a comment by (\w+) containing the text "([\s\S]+)"$"#)]
async fn then_issue_page_still_shows_comment_with_text(...);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing the text "([\s\S]+)"$"#)]
async fn then_issue_page_shows_comment_with_text(...);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" no longer shows a comment by (\w+)$"#)]
async fn then_issue_page_no_longer_shows_comment_by(...);

#[then(regex = r"^the response status is 200$")]
async fn then_response_status_200(...);

#[then(regex = r"^the response status is 410$")]
async fn then_response_status_410(...);

#[then(regex = r#"^the response is an htmx fragment containing the text "([\s\S]+)"$"#)]
async fn then_response_fragment_contains_text(...);

#[then(regex = r#"^the response is an htmx fragment that does NOT contain a <(\w+)> element$"#)]
async fn then_response_fragment_no_element(...);

#[then(regex = r#"^the event payload's comment author email is "([^"]+)"$"#)]
async fn then_event_payload_comment_author_email(...);
```

The slice-2 `the response is an htmx fragment containing "{}"` phrase
is reused for the 410-Gone substring assertion (it takes a literal
substring, which is what we want for the "This comment has been
deleted" copy match).

## DELIVER Pre-flight Checklist

DELIVER must satisfy these before merging:

- [ ] `cargo check -p foundry-acceptance --tests` continues to pass
- [ ] All 10 slice-5 scenarios execute against real Postgres + real
      axum and classify GREEN under cucumber-rs (replacing the current
      RED scaffold panics)
- [ ] Migration `0006_comments_edit_delete.sql` is created at
      `crates/foundry-store/migrations/0006_comments_edit_delete.sql`
      per architecture.md § "Migration Shape"
- [ ] `Store::update_comment_with_outbox`, `Store::soft_delete_comment_with_outbox`,
      `Store::find_comment_by_id` are implemented + PBT-tested at unit
      layer (ADR-025 D2 PBT phase)
- [ ] `submit_edit_comment`, `submit_delete_comment`, `show_edit_form`
      (and, conditional on D3 = A, `show_single_comment`) handlers are
      added to `crates/foundry-app/src/comments.rs` and registered in
      `crates/foundry-app/src/lib.rs::build_router`
- [ ] `EventPayload` extended with `deleted: Option<bool>` field per
      ADR-008; `schema_version` stays at 1
- [ ] SSE handler match arms added for `CommentEdited` and
      `CommentDeleted` event types
- [ ] List query gains `WHERE deleted_at IS NULL` filter; verified by
      the soft-delete-invariant scenario
- [ ] `Store::probe()` extended with the migration-0006 column-existence
      assertion per architecture.md § "Earned Trust"
- [ ] Cancel handler URL implementation matches DISTILL D3 user pick
      (default: A — thin new GET `/comments/{id}` handler)
- [ ] PATCH and DELETE handlers carry the existing CSRF middleware per
      ADR-009 (no special handling required — `build_router`-wide
      layering covers them)
- [ ] No regression in the 6 slice-2 US-10 scenarios in
      `tests/features/us-10-comments.feature` (slice-2 file is
      untouched; phrases do not collide)
- [ ] No regression in the slice-1/3/4 scenarios (compile gate + full
      `cargo test` run prove this)
- [ ] Per ADR-007 — operator-runbook recipe for admin-undelete is
      EXPLICITLY DEFERRED to v0.2 (DISTILL D5 recommendation = B); do
      not ship runbook content in this slice unless user overrides D5
- [ ] Step-phrase contract: the 21 new phrases (3 Givens + 8 Whens +
      10 Thens — count per `driver.md` § 4) MUST NOT be renamed in
      GREEN. Awkward phrasings should be surfaced as DELIVER → DISTILL
      retro items, not unilateral renames.

## Production-side scaffolds (Mandate 7) — NOT done by slice-5 DISTILL

Per the task brief:
> DO NOT touch any production code under
> `crates/foundry-app/src/comments.rs`,
> `crates/foundry-store/src/`, migrations, or anything else outside
> the test harness.

This is a project-specific deviation from the nw-distill § "Mandate 7:
RED-Ready Scaffolding" default. The slice-5 task explicitly defers
production-side scaffolding to DELIVER's RED phase (per ADR-025 D2:
DELIVER unskips, writes PBT, then implements). The RED classification
in slice 5 is achieved entirely by step-body panics in
`crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs` —
no production-side `panic!`-shaped scaffolds.

DELIVER picks up production-side scaffolds (or full implementations)
from a clean slate. The acceptance step bodies are the RED contract.
