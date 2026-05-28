# Feature: handler-instrumentation
# Slice: 6 (cross-cutting infrastructure — emits the metric series the
#         already-shipped Grafana "Foundry Overview" dashboard panels
#         reference)
#
# JTBD: outcome-1 (operators stand up Foundry and confirm it's healthy
#       within an hour — empty Grafana panels are a deploy-time
#       correctness failure this slice removes)
#
# Inheritance (DEVOPS slice — commit c7cb715):
#   - `metrics_exporter_prometheus` recorder + sidecar axum listener on
#     METRICS_PORT (default 9090) are already wired
#   - `/metrics` endpoint exists; `/healthz` on the sidecar exists
#   - Grafana dashboard JSON ships at
#     observability/grafana-dashboards/foundry-overview.json
#   - Slice 6 adds the EMISSION code so the dashboard's panel queries
#     resolve to real series (slice 6's whole purpose)
#
# Slice-6 driving adapters (per architecture.md):
#   - `GET /metrics` on the sidecar listener (Prometheus's pull surface;
#     all 9 automated scenarios scrape it)
#   - `request_tracking_layer()` tower middleware on the main router
#     (every routed request triggers it — observed via the scrape)
#   - Background `tokio::time::interval` poll task in main.rs
#     (scenarios #5 + #6)
#   - `SubscriberGauge` RAII guard in foundry-realtime (scenarios #7 + #8)
#   - `metrics_server::probe()` startup self-scrape (scenario #9)
#
# Driven adapters exercised (all reused; ZERO new infrastructure per
# architecture.md § Reuse Analysis):
#   - `metrics_exporter_prometheus::PrometheusHandle` (render path)
#   - `metrics::counter!` / `histogram!` / `gauge!` facades (emission)
#   - sqlx `Pool::size()` + `Pool::num_idle()` (read-only accessors)
#   - axum `MatchedPath` extractor (route template label)
#   - Rust `Drop` trait (RAII guard for SSE lifetimes)
#   - real Postgres per-scenario schema (slice-1 inherited)
#   - `assert_cmd::Command::cargo_bin("foundry")` subprocess driver
#     (slice-3 inherited; slice-6 is its second use)
#   - `reqwest::Client` in scrape direction (slice-1 inherited)
#
# Layer / PBT mode declaration (per nw-test-design-mandates Mandate 9):
#   - Layer 3+ (real subprocess + real HTTP scrape + real Postgres).
#   - Example-only. No proptest. Sad paths enumerated explicitly per
#     Mandate 11. PBT belongs at layers 1-2 (unit), which is DELIVER's
#     responsibility (the cardinality-key unit test per D2 = A;
#     SubscriberGauge panic-unwind test per ADR-013 § Verification;
#     probe failure-injection test per ADR-014 § Verification).
#
# Test invocation pattern (slice-6 deviation from slice-2/5 in-process
# default per DD-2):
#   - Each scenario spawns a foundry subprocess via
#     `assert_cmd::Command::cargo_bin("foundry")` because the
#     in-process `InProcHarness` deliberately SKIPS `install_recorder()`
#     to avoid the "global recorder already installed" panic on the
#     second scenario. The /metrics substrate requires real recorder
#     install + real sidecar listener; only the subprocess path
#     provides both honestly.
#   - Per-scenario PG schema (slice-1 pattern); ephemeral METRICS_PORT
#     + FOUNDRY_PORT.
#   - Documented in driver.md § 1-2 + proposals.md § "How slice-6
#     scenarios run".
#
# Two `@walking_skeleton` scenarios (DD-11 deviation from "exactly one
# WS per feature file" convention):
#   - #1 covers the request-path metric flow (every HTTP request →
#     middleware → recorder → scrape)
#   - #9 covers the process-startup-probe flow (process boots → sidecar
#     binds → self-scrape probe asserts the recorder accepted the
#     startup counter)
#   Each is a structurally different "wired end-to-end" contract;
#   demoting either to non-WS would lose the deploy-time-correctness
#   demo or the metric-emission demo. Flagged for user override.

@slice6 @handler-instrumentation @metrics
Feature: The Grafana "Foundry Overview" dashboard panels light up because the application emits the metric series they reference
  An operator who runs `docker compose up -d` and opens the bundled
  Grafana dashboard sees the panels resolve to real data: request
  rates by route and status, request latency histograms, the Postgres
  connection pool gauge, and the SSE subscriber gauge. The bytes flow
  from a tower middleware on every routed request (ADR-010), a 5s
  pool-polling task (ADR-012), and a RAII-bound SSE subscriber gauge
  (ADR-013) — converging into a single Prometheus recorder rendered by
  the sidecar listener on the metrics port. The process refuses to
  start if its own `/metrics` endpoint isn't reachable with the
  startup counter line present (ADR-014).

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3
    And Mei is signed in

  @walking_skeleton @real-io @driving_adapter @nfr-obs-03
  Scenario: Operator scrapes the metrics endpoint after a single comment POST and sees the request reflected in the counter and the histogram
    # Walking-skeleton wiring proof. "Sample is greater than 0" is
    # the contract because the comment POST requires HTTP-mediated
    # pre-setup (sign-in + CSRF fetch) — the counter sees those
    # too. The substrate-lie probe is "counter went up at all",
    # which scenarios #2 and #4 tighten to the exact-N invariants.
    Given the operator's foundry instance is running
    When Mei posts a comment on "AUTH-3" with body "First comment from the metrics-instrumented build."
    And the operator scrapes the metrics endpoint
    Then the scrape returns HTTP 200
    And the scrape body contains the line "http_requests_total"
    And the scrape body's "http_requests_total" sample is greater than 0
    And the scrape body's "http_request_duration_seconds" histogram has at least one bucket with count >= 1

  @real-io @nfr-obs-03
  Scenario: The request counter breakdown distinguishes route templates, methods, and statuses
    Given the operator's foundry instance is running
    When the operator issues 5 HTTP requests across the routes "/healthz" and "/readyz"
    And the operator scrapes the metrics endpoint
    Then the scrape returns HTTP 200
    And the scrape body's "http_requests_total" sample sums to 5
    And the scrape body contains a sample for "http_requests_total" with labels "path=/healthz,method=GET,status=200"
    And the scrape body contains a sample for "http_requests_total" with labels "path=/readyz,method=GET,status=200"

  @real-io @cardinality @nfr-obs-03
  Scenario: A request to a parameterized route emits the route template as the path label, never the concrete URI, and carries no forbidden high-cardinality labels
    Given the operator's foundry instance is running
    When Mei posts a comment on "AUTH-3" with body "Cardinality probe — concrete URI must NOT appear in the path label."
    And the operator scrapes the metrics endpoint
    Then the scrape body's "http_requests_total" sample's "path" label is "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments"
    And the scrape body's "http_requests_total" samples carry only the label keys "path,method,status"
    And the scrape body's "http_requests_total" samples do NOT carry any of the label keys "user_id,workspace_id,team_id,project_id,issue_id,comment_id,session_id,request_id"

  @real-io @nfr-obs-03
  Scenario: After the operator issues N HTTP requests, the counter sum across all label combinations equals N exactly
    Given the operator's foundry instance is running
    When the operator issues 7 HTTP requests to "/healthz"
    And the operator scrapes the metrics endpoint
    Then the scrape body's "http_requests_total" sample sums to 7

  @real-io @nfr-obs-03 @serial
  Scenario: The Postgres connection pool gauge reflects the in-use connection count within one polling interval
    # The gauge is updated by a 1-second poll task in main.rs (slice-6
    # ADR-012). A single-instant scrape can sample a transient idle
    # window even while traffic is in-flight, so the contract is
    # "eventually within one polling-interval-and-then-some" rather
    # than "right now". The 10-second deadline covers 10+ poll ticks at
    # the 1s METRICS_POOL_POLL_SECONDS test cadence. Per-scrape timeout
    # is bounded inside the helper (POLL_SCRAPE_TIMEOUT, 750ms) so a
    # single slow scrape under @all-load contention can't monopolise
    # the deadline. See
    # docs/feature/slice-6-scenario-hardening/distill/wave-decisions.md.
    #
    # @serial: the step generates load with 32 tokio::spawn'd /readyz
    # hammer tasks to keep the subprocess pool's in_use > 0. Under @all
    # those load-generator tasks share the test runtime with 5 sibling
    # scenarios and get starved — they don't sustain enough requests to
    # saturate the pool, so the gauge never rises and the bounded-poll
    # times out (flaked ~1/3 at max_connections=100, ~1/5 at 300).
    # De-contended, the hammer tasks get the CPU they need. (The raised
    # max_connections=300 ceiling — see harness.rs — is what makes adding
    # this 3rd @serial scenario safe from the PoolTimedOut that 3 serial
    # scenarios hit at the old 100 ceiling.)
    Given the operator's foundry instance is running
    When Mei holds an open database connection for 6 seconds
    Then the scrape body's "db_connections_in_use" sample is eventually greater than 0 within 10 seconds

  @real-io @startup-register @nfr-obs-03
  Scenario: Immediately after process start, the connection-pool gauge is scrapable at value 0 so Grafana sees the metric line without a delay
    # The register-at-0 contract is "the metric line is present immediately, so
    # Grafana never shows no-data" — asserted by the HTTP 200 + contains-the-line
    # steps below. The exact value at the scrape instant is racy: a startup/readyz
    # query can hold a pool connection when the 1s poll samples, so the idle gauge
    # reads 1 briefly (flaked ~40% in release mode, default and @all lanes). Assert
    # the idle pool settles to 0 within a short window instead of at one instant.
    Given the operator's foundry instance is running
    When the operator scrapes the metrics endpoint immediately
    Then the scrape returns HTTP 200
    And the scrape body contains the line "db_connections_in_use"
    And the scrape body's "db_connections_in_use" sample settles to 0 within 5 seconds

  @real-io @sse @nfr-obs-03
  Scenario: When a viewer opens an SSE subscription the subscriber gauge increments and returns to zero after the viewer closes cleanly
    Given the operator's foundry instance is running
    And Mei has subscribed to events on "Auth v2"
    When the operator scrapes the metrics endpoint
    Then the scrape body contains the line "sse_subscribers_total"
    And the scrape body's "sse_subscribers_total" sample is greater than 0
    When Mei abruptly disconnects from the SSE stream
    And the operator's foundry instance has been running for at least 1 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body's "sse_subscribers_total" sample returns to 0

  @real-io @sse @error @nfr-obs-03
  Scenario: When a viewer's SSE stream is abruptly dropped mid-poll the subscriber gauge still decrements via the RAII guard's Drop
    Given the operator's foundry instance is running
    And Mei has subscribed to events on "Auth v2"
    When the operator scrapes the metrics endpoint
    Then the scrape body's "sse_subscribers_total" sample is greater than 0
    When Mei abruptly disconnects from the SSE stream
    And the operator's foundry instance has been running for at least 1 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body's "sse_subscribers_total" sample returns to 0

  @walking_skeleton @real-io @startup-probe @nfr-obs-03
  Scenario: The process refuses to serve traffic until the self-scrape probe confirms the metrics endpoint is reachable and the startup counter line is present
    Given the operator's foundry instance is running
    When the operator scrapes the metrics endpoint
    Then the scrape returns HTTP 200
    And the scrape body contains the line "foundry_app_startup_total"
    And the scrape body's "foundry_app_startup_total" sample has value 1
    And the foundry subprocess is alive

  @manual @nfr-perf-05
  Scenario: The request-tracking middleware adds no more than ten microseconds P95 of overhead per request
    # Manual scenario — verifies the slice-6 D7 performance budget
    # (≤10µs P95 per request added by the request_tracking_layer).
    # Reason for manual classification: cucumber-rs cannot reliably
    # measure 10µs at scenario granularity (the JVM-style "lots of
    # iterations + statistical analysis" pattern doesn't fit a
    # scenario-per-iteration BDD harness). The contract is enforced
    # by the criterion microbench DELIVER ships at
    # `crates/foundry-app/benches/middleware_overhead.rs` (per
    # architecture.md § "Performance budget" measurement plan).
    # This scenario is the contract anchor; the criterion bench is
    # the enforcement.
    Given the operator's foundry instance is running
    When the operator runs the middleware overhead criterion microbench across the 27 routes
    Then the bench reports added P95 overhead below 10 microseconds
