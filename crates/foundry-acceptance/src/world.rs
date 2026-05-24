//! Cucumber-rs world struct.
//!
//! Per `distill/driver.md` §2: per-scenario state lives here.

use crate::support::compose_harness::ComposeStack;
use crate::support::harness::InProcHarness;
use crate::support::sse_client::{SseEvent, SseOpenAttempt, SseSubscription};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::time::Instant;

#[derive(cucumber::World, Default, Debug)]
#[world(init = Self::default)]
pub struct FoundryWorld {
    // ---- US-01 docker-compose harness ----
    pub compose: Option<ComposeStack>,
    pub compose_bootstrap_url: Option<String>,
    pub admin_already_claimed: bool,

    // ---- US-05+ in-process harness ----
    pub harness: Option<InProcHarness>,
    pub http: Option<reqwest::Client>,

    /// Raw bootstrap-token strings indexed by the name they were minted
    /// under in the Background step (e.g. "valid-token-001").
    pub minted_tokens: HashMap<String, String>,

    /// Last response captured by a When step (consumed by Then).
    pub last_status: Option<StatusCode>,
    pub last_body: Option<String>,
    pub last_headers: Option<HeaderMap>,

    /// Identity of the latest invite generated through `/invites`, used
    /// by the "invite is recorded as valid for 7 days" assertion.
    pub last_invite_id: Option<uuid::Uuid>,

    /// Session cookie value (`foundry_session=...`) captured after a
    /// successful claim. Stored separately because `reqwest`'s cookie
    /// jar requires HTTPS for `Secure` cookies and we test over plain
    /// http://127.0.0.1.
    pub session_cookie_header: Option<String>,

    // ---- US-06 timing scratch (sign-in response latency) ----
    /// Wall-clock for the most recent /sign-in response. Used to
    /// compare unknown-email vs wrong-password timing.
    pub us_06_last_response_ms: Option<u64>,
    /// Baseline captured for the wrong-password scenario; the
    /// unknown-email scenario asserts its own time is within 50ms of
    /// this baseline.
    pub us_06_wrong_pw_response_ms: Option<u64>,

    // ---- US-07 project-create scratch ----
    /// Email of the signed-in user for the current scenario (drives
    /// `signed_in_post` and the post-redirect board fetch).
    pub us_07_signed_in_email: Option<String>,
    /// Password matching `us_07_signed_in_email`. Stored in the world
    /// because the test harness re-authenticates for each follow-up
    /// HTTP request (no cookie jar — see harness::client).
    pub us_07_signed_in_password: Option<String>,
    /// Name of the last project the When step attempted to create.
    /// Used by the "no second project is created" assertion.
    pub us_07_last_attempted_name: Option<String>,
    /// Slug of the last team a When step targeted. Currently
    /// informational; reserved for diagnostics on failed assertions.
    pub us_07_last_team_slug: Option<String>,

    // ---- US-08 file-issue scratch ----
    /// Slug of the last project a US-08 When step posted to. Used by
    /// the board-fetch assertion to reconstruct the GET URL.
    pub us_08_last_project_slug: Option<String>,
    /// Team slug of the last US-08 When step's target project. Same
    /// reason as above.
    pub us_08_last_team_slug: Option<String>,
    /// Per-request latencies captured in the performance scenario.
    /// Length matches the number of POSTs the When step issued.
    pub us_08_latencies_ms: Vec<u64>,

    // ---- US-09 realtime SSE ----
    /// Active SSE subscriptions for this scenario, keyed by
    /// `(subscriber_name, project_name)` so the same scenario can hold
    /// two subscriptions (Mei + Hiroshi on the same project).
    pub us_09_subscriptions: HashMap<(String, String), SseSubscription>,
    /// Wall-clock instant the most recent When step started; the
    /// matching Then step uses it to compute per-event arrival latency
    /// (so timing is per-scenario, not per-suite).
    pub us_09_last_action_started_at: Option<Instant>,
    /// Captured open attempt for the @error 401/403 scenarios.
    pub us_09_last_open_attempt: Option<SseOpenAttempt>,
    /// Open status for Rita's authenticated-but-forbidden 403 scenario.
    pub us_09_last_open_status: Option<StatusCode>,
    /// Mei's session cookie (`foundry_session=...`) once she signs in.
    /// Cached so subsequent steps don't re-authenticate.
    pub us_09_mei_cookie: Option<String>,
    /// Rita's session cookie. Separate slot so a single scenario can
    /// hold both at once (the 403 scenario signs Rita in but never
    /// touches Mei).
    pub us_09_rita_cookie: Option<String>,
    /// Most-recent event matched by the "observes event" Then step,
    /// fed to the follow-up "the event's project key is ..." step.
    pub us_09_last_event: Option<SseEvent>,

    // ---- US-10 comments ----
    /// Last issue key a comment scenario targeted (e.g. "AUTH-3"). Used
    /// by the issue-page-render Then step to know which page to GET.
    pub us_10_last_issue_key: Option<String>,
    /// Body of the most recent issue-page GET captured in a US-10 Then
    /// step, so subsequent Then steps that assert different selectors
    /// on the same body don't re-fetch.
    pub us_10_last_issue_body: Option<String>,

    // ---- US-12 keyboard-nav response capture ----
    /// Body of the most-recent GET captured by a US-12 When step. The
    /// US-12 scenarios make exactly one GET and then run multiple Then
    /// assertions against the cached body.
    pub us_12_last_get_body: Option<String>,
}
