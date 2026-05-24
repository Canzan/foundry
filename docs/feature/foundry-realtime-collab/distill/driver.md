# DISTILL Driver Design — Slice 2 Acceptance Harness (Realtime Collab)

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md`,
`coverage-matrix.md`. This document is an **additive delta** to
`docs/feature/foundry-backend-mvp/distill/driver.md` — everything that is
not mentioned here is inherited unchanged from slice 1.

## 1. What slice 1 already provides (inherited, do not re-build)

From `crates/foundry-acceptance/`:

- `support/harness.rs` — `InProcHarness::spawn(now)`, `ensure_postgres()`,
  `fresh_schema_pool()`, `drop_schema()`, `signed_in_post()`.
- `support/compose_harness.rs` — US-01 docker-compose driver (untouched by
  slice 2).
- `world.rs` — `FoundryWorld` struct, per-scenario state, the
  `harness: Option<InProcHarness>` slot that all in-process scenarios use.
- The testcontainers Postgres-16 container shared across the suite, per-
  scenario schema rotation, the cucumber-rs runner at
  `tests/acceptance.rs`, and the tag-filtering CI plumbing.

Slice 2 plugs into the same world struct and the same per-scenario isolation.

## 2. What slice 2 adds to the harness

Three new support modules in `crates/foundry-acceptance/src/support/`:

### 2a. `sse_client.rs` — minimal SSE consumer

A purpose-built SSE-line parser layered on `reqwest::Response::bytes_stream()`.
Rationale: `eventsource-client` (the obvious off-the-shelf option) is
MIT-licensed and AGPL-compatible, **but** it pulls in `hyper-rustls` and
adds a third async-stream dependency footprint for what amounts to ~80
lines of SSE-line parsing. We roll our own to keep the new-dependency
surface at zero. Cite this decision back when the third realtime
consumer scenario lands.

Public surface:

```rust
pub struct SseSubscription {
    pub project_slug: String,
    received: Arc<Mutex<Vec<SseEvent>>>,
    /// Wall-clock instant each event arrived (drives latency assertions).
    arrival_times: Arc<Mutex<Vec<Instant>>>,
    _shutdown: oneshot::Sender<()>,
    open_status: StatusCode,
}

#[derive(Clone, Debug)]
pub struct SseEvent {
    pub event_type: String,           // "IssueCreated", "IssueUpdated", "CommentAdded", "keepalive"
    pub payload_json: Option<serde_json::Value>,
    pub raw_data: String,             // for diagnostic dumps on assert failure
}

/// Open an SSE subscription against the in-process app, signed in as
/// `email` / `password`. Returns once the response headers are in (so
/// the open_status field is populated) and a background tokio task is
/// draining `data:` / event-name / `:keepalive` lines into the
/// `received` vec.
pub async fn open_sse_subscription(
    harness: &InProcHarness,
    http: &reqwest::Client,
    email: &str,
    password: &str,
    team_slug: &str,
    project_slug: &str,
) -> SseSubscription;

/// Drop a closed/refused subscription and capture the open status. Used
/// by the @error scenarios that assert 401/403.
pub async fn open_sse_subscription_unauthenticated(
    base: &str,
    team_slug: &str,
    project_slug: &str,
) -> SseOpenAttempt;  // carries status + body

pub struct SseOpenAttempt {
    pub status: StatusCode,
    pub body: String,
}

impl SseSubscription {
    /// Wait up to `timeout` for an event matching `predicate`.  Returns
    /// the event and the per-event arrival latency relative to a caller-
    /// provided `started_at`.  None on timeout.
    pub async fn wait_for(
        &self,
        timeout: Duration,
        started_at: Instant,
        predicate: impl Fn(&SseEvent) -> bool,
    ) -> Option<(SseEvent, Duration)>;

    /// Drain everything currently received (no wait). Used by the
    /// "received zero events" assertion.
    pub fn drain(&self) -> Vec<SseEvent>;
}
```

Parsing rules (SSE spec subset we need):

- Lines beginning with `:` are comments (heartbeats: `:keepalive`).
- Lines beginning with `event:` set the next dispatch's event name.
- Lines beginning with `data:` accumulate into the dispatch's data buffer.
- An empty line dispatches the buffered event into `received`.
- We attempt `serde_json::from_str` on the accumulated data; on failure
  we leave `payload_json = None` and keep the raw string — so the
  heartbeat `:keepalive` line surfaces as an `SseEvent { event_type:
  "keepalive", payload_json: None, raw_data: "keepalive" }`.

### 2b. `html_assertions.rs` — scraper-based HTML structure assertions

Used by US-10 markdown-rendering scenarios and US-12 data-attribute
scenarios. Thin layer over the `scraper` crate that ships in slice 1
workspace deps.

```rust
/// Assert the document contains at least one descendant of `root_selector`
/// matching `element_selector` with the given text. Returns the matching
/// element for further inspection. Panics with a diagnostic body dump on
/// miss.
pub fn assert_element_with_text(
    body: &str,
    root_selector: &str,        // e.g. ".comment[data-author='mei@acme.com']"
    element_selector: &str,     // e.g. "strong"
    expected_text: &str,
) -> ElementSnapshot;

/// Assert NO `element_selector` descendant exists under `root_selector`.
pub fn assert_no_element(body: &str, root_selector: &str, element_selector: &str);

/// Assert an `<a>` element exists with the given href and that its rel
/// attribute contains the given fragment.
pub fn assert_link_with_rel(body: &str, root_selector: &str, href: &str, rel_fragment: &str);

/// Collect all elements matching a selector AND a given attribute.
/// Returns them in document order. Used for the data-issue-key ordering
/// assertion in US-12.
pub fn collect_attributes(body: &str, selector: &str, attribute: &str) -> Vec<String>;
```

All four helpers accept either a full HTML document or an htmx fragment;
`scraper::Html::parse_fragment` is forgiving.

### 2c. `heartbeat_env.rs` — per-scenario heartbeat interval override

The production heartbeat interval is 25 seconds (`SSE_HEARTBEAT_MS` env
var). The heartbeat scenario in US-09 needs a much shorter interval to
complete in under a second.

```rust
/// Set the heartbeat interval for the NEXT `InProcHarness::spawn`. The
/// override is read by `AppState::from_env` (production) or directly by
/// `spawn_app` (tests). Cleared at scenario teardown so unrelated
/// scenarios get the default.
pub fn override_heartbeat_ms(ms: u64);
pub fn clear_heartbeat_override();
```

`InProcHarness::spawn` will be tweaked in DELIVER to read this override
and plumb it into `AppState.sse_heartbeat`. The slice-1 spawn signature
does not need to change for the other 37 scenarios — they get the
default 25_000 ms because they finish in milliseconds and never see a
heartbeat.

## 3. World struct additions (`FoundryWorld`)

Slice 2 adds five fields. All default to `None` / empty; the existing
slice-1 scenarios are unaffected.

```rust
pub struct FoundryWorld {
    // ... existing slice-1 fields ...

    // ---- US-09 / US-10 SSE subscriptions ----
    /// Subscriptions opened by the current scenario, keyed by the
    /// subscriber's email + project slug pair. Allows two subscribers
    /// (Mei + Hiroshi) in the same scenario.
    pub us_09_subscriptions: HashMap<(String, String), SseSubscription>,
    /// Wall-clock instant the last When step started; used so the
    /// matching Then step can compute per-event latency.
    pub us_09_last_action_started_at: Option<Instant>,
    /// Captured open-attempt for @error scenarios (401/403).
    pub us_09_last_open_attempt: Option<SseOpenAttempt>,

    // ---- US-10 comments ----
    /// Last issue key a comment scenario targeted (e.g. "AUTH-3"). Used
    /// by the issue-page-render Then step.
    pub us_10_last_issue_key: Option<String>,

    // ---- US-12 keyboard-nav response capture ----
    /// Body of the most-recent GET captured by a US-12 When step.
    pub us_12_last_get_body: Option<String>,
}
```

## 4. Per-scenario isolation — unchanged

The slice-1 invariant holds: one Postgres container per `cargo test`,
fresh schema per scenario. SSE subscriptions live entirely in-process
inside the `InProcHarness.app` AppState; when the harness is dropped at
scenario teardown, the tokio runtime tears down the background SSE
reader tasks via the `_shutdown` oneshot. No cross-scenario bleed.

The one nuance: the per-replica LISTEN connection (one `PgConnection`
held by a background task per `AppState`) is bound to the per-scenario
pool. When the schema is dropped at After, the LISTEN connection closes
naturally. We accept a small noise log "LISTEN connection lost" at
teardown; DELIVER may add an explicit shutdown signal if it is loud.

## 5. Real-I/O budget — slice 2 adds ≤20 s on top of slice 1's ~8 s

| Scenario | Cost estimate | Notes |
|---|---|---|
| US-09 walking skeleton | ~150ms | One SSE open + one issue POST + ~50ms RTT for fan-out |
| US-09 cross-project negative | ~2.6s | Must wait the full 2500ms ceiling to assert ZERO events; cannot shortcut without false negatives |
| US-09 anonymous refused | ~50ms | Single GET, no real subscription |
| US-09 cross-team refused | ~150ms | Sign-in as Rita + refused open |
| US-09 heartbeat | ~750ms | 700ms quiet window + 50ms slack |
| US-09 IssueUpdated | ~150ms | Mirror of walking skeleton |
| US-09 NFR-PERF-03 (10 sequential) | ~2.5s | 10 POSTs × ~100ms + fan-out + measurement |
| US-10 walking skeleton | ~150ms | POST comment + GET issue page |
| US-10 realtime delivery | ~200ms | Open SSE + POST comment + receive |
| US-10 XSS sanitization | ~150ms | POST comment + GET issue page |
| US-10 empty body 400 | ~100ms | One POST + assertions |
| US-10 whitespace-only 400 | ~100ms | One POST + assertions |
| US-10 non-member 403 | ~150ms | Sign-in as Rita + POST + 403 |
| US-12 data-issue-key | ~100ms | One GET + scraper walk |
| US-12 modal fragment | ~100ms | One GET with HX-Request header |
| US-12 search by substring | ~100ms | One GET against pre-seeded issues |
| US-12 search by key | ~100ms | One GET |
| US-12 keyboard-help | ~80ms | One GET against static-ish page |
| **Subtotal** | **~7.7s** | comfortably under the 20s ceiling |
| **Suite total (slice 1 + slice 2)** | **~15-16s** | within the 60s top-line budget from slice-1 driver.md §7 |

The US-09 cross-project negative is the only scenario that consumes a
noticeable wait. We accept this trade-off: a shorter wait would risk
false negatives (event arrived just after we stopped looking and the
slice-2 wiring was actually broken).

## 6. Tag conventions (additions only)

Inherited from slice 1: `@slice1`, `@walking_skeleton`, `@real-io`,
`@driving_port`, `@driving_adapter`, `@error`, `@nfr-perf-01`,
`@nfr-sec-06`, `@manual`, `@docker-compose`, `@us-NN`.

Added in slice 2:

- `@slice2` — every scenario in this slice.
- `@realtime` — scenarios that exercise the SSE pipeline.
- `@comments` — scenarios that exercise the comment endpoints.
- `@keyboard` — scenarios that pin the server contracts the keyboard
  shortcuts depend on.
- `@nfr-perf-03` — the realtime-latency scenario.
- `@nfr-sec-05` — the markdown sanitization scenario.

`@manual` is reused unchanged from slice 1 (the US-12 browser drill).

## 7. CI invocation (delta only)

The slice-1 invocation stays as-is. The slice-2 scenarios pick up
automatically because they live under the same feature-files root. No
new CI stage is needed — the budget delta lands inside the existing
Stage B (~60s ceiling).

Local fast loop:

```bash
# Slice 2 only:
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@slice2 and not @manual"

# Realtime-only subset:
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@us-09 or @us-10"
```

## 8. Standing rules carried into DELIVER (additions)

- Every SSE-consuming Then step asserts the **arrival latency** computed
  from the `Instant` captured at the When step, not wall-clock against
  a global anchor. This makes per-scenario timing assertions independent
  of the previous scenario's runtime.
- The SSE reader task is the **only** code path that reads from a live
  SSE response in tests; step bodies invoke `subscription.wait_for(...)`
  or `subscription.drain()`. No step body should call
  `response.bytes_stream()` directly.
- Heartbeat assertions tolerate a 2× safety margin (a `200ms` interval
  must produce ≥2 heartbeats in a 700ms window, not ≥3). Heartbeats are
  driven by `tokio::time::interval` which can drift under load.
- The `@manual` US-12 scenario MUST NOT be auto-skipped silently. The
  test runner records the manual checklist as a CI artifact so QA
  cannot forget to run it; precedent set by US-01's `@manual` in slice 1.
