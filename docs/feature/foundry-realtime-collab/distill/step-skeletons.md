# Step Skeletons — Slice 2 (Realtime Collab)

These are signatures only; bodies arrive in DELIVER. Phrases listed
under "Background (inherited)" already exist in slice 1's step files
and MUST be reused unchanged — registering them twice will collide
(cucumber-rs treats step phrases as globally unique).

The skeletons follow the slice-1 organisation: one file per US-N under
`crates/foundry-acceptance/src/steps/`. Shared helpers (SSE consumer,
HTML scraper assertions, heartbeat env override) live in
`crates/foundry-acceptance/src/support/`.

## Background — inherited unchanged from slice 1

These are defined in slice 1's step files; slice 2 features call them
verbatim and do not redefine them.

```rust
// Defined in steps/us_05_bootstrap.rs (or shared via support/)
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(...);

// Defined in steps/us_07_project_create.rs
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(...);

// Defined in steps/us_06_signin.rs
#[given(regex = r"^(\w+) is signed in$")]
async fn member_is_signed_in(...);

#[given(regex = r"^(\w+) is signed out$")]
async fn member_is_signed_out(...);

// Defined in steps/us_08_file_issue.rs
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(...);

#[given(regex = r#"^the "([^"]+)" project already has issues (\w+)-(\d+) through (\w+)-(\d+)$"#)]
async fn project_has_issues_range(...);

#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(...);

#[when(regex = r#"^(\w+) files an issue against "([^"]+)" with title "([^"]*)"$"#)]
async fn file_issue(...);

#[then(regex = r"^the response status is 400 or 422$")]
async fn status_400_or_422(...);

#[then(regex = r#"^the response is an htmx fragment containing "([^"]+)"$"#)]
async fn response_htmx_fragment_containing(...);

#[then(regex = r"^the response is not a full HTML page$")]
async fn response_not_full_page(...);
```

## US-09 — `crates/foundry-acceptance/src/steps/us_09_realtime_sse.rs`

```rust
use crate::support::harness::{InProcHarness, signed_in_post};
use crate::support::sse_client::{
    open_sse_subscription, open_sse_subscription_unauthenticated, SseEvent, SseSubscription,
};
use crate::support::heartbeat_env::{clear_heartbeat_override, override_heartbeat_ms};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::StatusCode;
use std::time::{Duration, Instant};

// --- Given: open subscription ---------------------------------------

#[given(regex = r#"^(\w+) has an open subscription to events on "([^"]+)"$"#)]
async fn member_has_open_subscription(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
);

#[given(regex = r#"^a member "([^"]+)" belongs to the team "Partners"$"#)]
async fn member_belongs_to_partners(world: &mut FoundryWorld, email: String);
// Note: the existing `member_belongs_to_team` step would match, BUT
// the "Partners" team does not exist by default in slice 1's seed.
// This step seeds the Partners team + Rita's membership.

#[given(regex = r"^Rita is signed in$")]
async fn rita_is_signed_in(world: &mut FoundryWorld);

#[given(regex = r"^the heartbeat interval is configured to (\d+) milliseconds for this scenario$")]
async fn heartbeat_interval_override(world: &mut FoundryWorld, ms: u64);

// --- When: realtime actions -----------------------------------------

#[when(regex = r#"^an anonymous request attempts to subscribe to events on "([^"]+)"$"#)]
async fn anonymous_subscribes(world: &mut FoundryWorld, project_name: String);

#[when(regex = r#"^Rita attempts to subscribe to events on "([^"]+)"$"#)]
async fn rita_subscribes(world: &mut FoundryWorld, project_name: String);

#[when(regex = r#"^(\w+) changes the state of "(\w+)-(\d+)" to "([^"]+)"$"#)]
async fn member_changes_issue_state(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    number: i32,
    new_state: String,
);

#[when(regex = r"^(\d+) milliseconds pass with no issue activity on \"([^\"]+)\"$")]
async fn quiet_window(world: &mut FoundryWorld, ms: u64, project_name: String);

#[when(regex = r#"^(\w+) files (\d+) issues against "([^"]+)" sequentially, each with a unique title, pausing (\d+) milliseconds between$"#)]
async fn member_files_n_issues_with_pause(
    world: &mut FoundryWorld,
    who: String,
    count: u32,
    project_name: String,
    pause_ms: u64,
);

// --- Then: realtime observations ------------------------------------

#[then(regex = r#"^within (\d+) milliseconds (\w+) observes an? "([^"]+)" event for "(\w+)-(\d+)" on "([^"]+)"$"#)]
async fn member_observes_event_within(
    world: &mut FoundryWorld,
    timeout_ms: u64,
    who: String,
    event_type: String,
    prefix: String,
    number: i32,
    project_name: String,
);

#[then(regex = r#"^the event's project key is "([^"]+)"$"#)]
async fn event_project_key(world: &mut FoundryWorld, key_prefix: String);

#[then(regex = r#"^the event payload reports state "([^"]+)"$"#)]
async fn event_payload_state(world: &mut FoundryWorld, expected: String);

#[then(regex = r#"^within (\d+) milliseconds (\w+) has received zero events on her "([^"]+)" subscription$"#)]
async fn member_received_zero_events(
    world: &mut FoundryWorld,
    wait_ms: u64,
    who: String,
    project_name: String,
);

#[then(regex = r"^the subscription is refused with status (\d+)$")]
async fn subscription_refused_with_status(world: &mut FoundryWorld, expected_status: u16);

#[then(regex = r#"^(\w+) receives no events on a closed stream$"#)]
async fn member_receives_no_events_closed_stream(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the response body contains a sign-in prompt$")]
async fn response_contains_signin_prompt(world: &mut FoundryWorld);

#[then(regex = r#"^(\w+)'s stream has received at least (\d+) keepalive heartbeats$"#)]
async fn stream_received_n_heartbeats(world: &mut FoundryWorld, who: String, n: u32);

#[then(regex = r#"^(\w+) receives (\d+) "([^"]+)" events whose keys are (\w+)-(\d+) through (\w+)-(\d+)$"#)]
async fn member_receives_n_events_with_keys(
    world: &mut FoundryWorld,
    who: String,
    count: u32,
    event_type: String,
    prefix1: String,
    first: i32,
    prefix2: String,
    last: i32,
);

#[then(regex = r"^every per-event arrival latency is at most (\d+) milliseconds$")]
async fn every_arrival_latency_at_most(world: &mut FoundryWorld, ms: u64);

#[then(regex = r"^the median per-event arrival latency is at most (\d+) milliseconds$")]
async fn median_arrival_latency_at_most(world: &mut FoundryWorld, ms: u64);
```

## US-10 — `crates/foundry-acceptance/src/steps/us_10_comments.rs`

```rust
use crate::support::harness::InProcHarness;
use crate::support::html_assertions::{
    assert_element_with_text, assert_link_with_rel, assert_no_element,
};
use crate::support::sse_client::SseEvent;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// --- When: comment ------------------------------------------

#[when(regex = r#"^(\w+) comments on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn member_comments_on_issue(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    number: i32,
    body: String,
);

// --- Then: comment-rendered HTML --------------------------

// Selector convention: `.comment[data-author="<email>"]` wraps each
// rendered comment in the issue page. The Then step looks up the issue
// page, parses, and asserts inside the comment block.
#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing a <(\w+)> element with text "([^"]+)"$"#)]
async fn issue_page_comment_contains_element_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    who: String,
    tag: String,
    text: String,
);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing an <a> element whose href is "([^"]+)" and whose rel attribute contains "([^"]+)"$"#)]
async fn issue_page_comment_link_with_rel(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    who: String,
    href: String,
    rel_fragment: String,
);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) that does NOT contain any <(\w+)> element$"#)]
async fn issue_page_comment_no_element(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    who: String,
    tag: String,
);

#[then(regex = r"^the comment is recorded as authored by (\w+)$")]
async fn comment_recorded_authored_by(world: &mut FoundryWorld, who: String);

#[then(regex = r#"^no comment is recorded on "(\w+)-(\d+)"$"#)]
async fn no_comment_recorded(world: &mut FoundryWorld, prefix: String, number: i32);

#[then(regex = r#"^the event payload's author email is "([^"]+)"$"#)]
async fn event_author_email(world: &mut FoundryWorld, email: String);

#[then(regex = r"^the response status is 403$")]
async fn status_403(world: &mut FoundryWorld);
```

## US-12 — `crates/foundry-acceptance/src/steps/us_12_keyboard_nav.rs`

```rust
use crate::support::harness::InProcHarness;
use crate::support::html_assertions::collect_attributes;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// --- Given: seed extra titled issues -------------------------------

#[given(regex = r#"^the "([^"]+)" project already has an issue titled "([^"]+)"$"#)]
async fn project_already_has_issue_titled(
    world: &mut FoundryWorld,
    project_name: String,
    title: String,
);

// --- When: server-contract GETs ------------------------------------

#[when(regex = r#"^(\w+) opens the project board for "([^"]+)"$"#)]
async fn member_opens_project_board(world: &mut FoundryWorld, who: String, project_name: String);

#[when(regex = r#"^(\w+) requests the new-issue modal for "([^"]+)" as an htmx request$"#)]
async fn member_requests_new_issue_modal(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
);

#[when(regex = r#"^(\w+) searches "([^"]+)" for the query "([^"]+)"$"#)]
async fn member_searches_project(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
    query: String,
);

#[when(regex = r"^(\w+) requests the keyboard-help overlay$")]
async fn member_requests_keyboard_help(world: &mut FoundryWorld, who: String);

// --- Then: data attributes + fragment shape ------------------------

#[then(regex = r#"^the rendered page contains an element with attribute data-issue-key="(\w+)-(\d+)"$"#)]
async fn page_contains_data_issue_key(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
);

#[then(regex = r"^the data-issue-key elements appear in the document in ascending issue-number order$")]
async fn data_issue_key_ascending_order(world: &mut FoundryWorld);

#[then(regex = r#"^the response is an htmx fragment containing a form posting to "([^"]+)"$"#)]
async fn fragment_contains_form_posting_to(world: &mut FoundryWorld, action: String);

#[then(regex = r#"^the response contains an input named "([^"]+)"$"#)]
async fn response_contains_input_named(world: &mut FoundryWorld, name: String);

#[then(regex = r"^the response marks the title input as autofocused$")]
async fn response_marks_title_autofocused(world: &mut FoundryWorld);

#[then(regex = r"^the response is an htmx fragment$")]
async fn response_is_htmx_fragment(world: &mut FoundryWorld);

#[then(regex = r#"^the response lists exactly one matching issue whose title contains "([^"]+)"$"#)]
async fn lists_one_issue_title_contains(world: &mut FoundryWorld, fragment: String);

#[then(regex = r#"^the response does NOT list the issue titled "([^"]+)"$"#)]
async fn response_does_not_list_title(world: &mut FoundryWorld, title: String);

#[then(regex = r#"^the response lists exactly one matching issue whose key is "(\w+)-(\d+)"$"#)]
async fn lists_one_issue_with_key(world: &mut FoundryWorld, prefix: String, number: i32);

#[then(regex = r"^the response is a valid HTML fragment$")]
async fn response_is_valid_html_fragment(world: &mut FoundryWorld);

#[then(regex = r#"^the response describes the "([^"]+)" shortcut as "([^"]+)"$"#)]
async fn response_describes_shortcut(world: &mut FoundryWorld, shortcut: String, label: String);

// --- Manual scenario stubs -----------------------------------------

#[given(regex = r"^a human reviewer is performing the keyboard-drill checklist$")]
async fn manual_reviewer_begins(world: &mut FoundryWorld);

#[when(regex = r"^the reviewer follows the documented steps$")]
async fn manual_reviewer_follows(world: &mut FoundryWorld);

#[then(regex = r"^the reviewer signs off on the keyboard-shortcut behaviour for this release$")]
async fn manual_reviewer_signs_off(world: &mut FoundryWorld);
// @manual scenarios MUST call `world.app.cucumber_skip("manual")` in the
// Given body so the automated runner reports them as Skipped, not
// Passed, and CI surfaces them as a checklist artifact (precedent: US-01).
```

## Production-side RED scaffolds (Mandate 7)

The scaffolds DISTILL produces are RUST source stubs that compile but
panic when invoked. They live in the production crates (NOT in the test
tree) so step-definition imports succeed and the failures are
classified RED (not BROKEN). The DELIVER wave replaces them with real
implementations.

Files to add (each carries `// SCAFFOLD: true` per the
nw-test-design-mandates Rust scaffold convention):

```
crates/foundry-realtime/src/listener.rs   # PgListener task + reconnect loop
crates/foundry-realtime/src/sse.rs        # SseHandler axum route + broadcast subscription
crates/foundry-realtime/src/publisher.rs  # extend slice-1 publisher to fire IssueUpdated/CommentAdded
crates/foundry-app/src/routes/comments.rs # POST/GET comment endpoints
crates/foundry-app/src/routes/search.rs   # GET search endpoint (ILIKE-based)
crates/foundry-app/src/routes/keyboard_help.rs # GET /keyboard-help
```

Each function body is `panic!("Not yet implemented -- RED scaffold");`
so the Red Gate Snapshot classifies the test as RED.
