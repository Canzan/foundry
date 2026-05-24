# Foundry MVP — Error Handling & Observability

Application-level only. Metrics endpoints, dashboards, alerting are system-designer's territory; this document covers what the application *exposes* and how it produces well-formed logs and errors.

## Error Type Strategy

### Per-crate `Error` enum via `thiserror`

Each library crate owns its own error enum.

| Crate | Error type | Variants |
|---|---|---|
| `foundry-core` | `DomainError` | `InvalidEmail`, `InvalidSlug`, `TitleTooLong`, `IllegalStateTransition`, ... |
| `foundry-store` | `StoreError` | `Sqlx(sqlx::Error)`, `NotFound`, `UniqueViolation { constraint: &'static str }`, `MigrationFailed(MigrateError)` |
| `foundry-auth` | `AuthError` | `InvalidCredentials`, `SessionExpired`, `BootstrapTokenUsed`, `BootstrapTokenExpired`, `InviteExpired`, `CsrfMismatch`, `Forbidden`, `RateLimitDelay(Duration)` |
| `foundry-realtime` | `RealtimeError` | `NotifyFailed`, `ListenConnectionLost`, `PayloadTooLarge` |
| `foundry-app` | `AppError` | wraps the above; one variant per source crate plus app-specific (`TemplateRender(askama::Error)`, `BadRequest(String)`) |

`thiserror` derives `Display`, `Error`, and source chains. We do not use `anyhow` in library crates (it erases types); we use it only in `main.rs` for the startup path where errors are terminal.

### HTTP boundary: `AppError -> Response`

`foundry-app::error` implements `axum::response::IntoResponse` for `AppError`:

```rust
// crates/foundry-app/src/error.rs (illustrative)
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, user_message, log_level) = match &self {
            AppError::Auth(AuthError::InvalidCredentials) =>
                (StatusCode::UNAUTHORIZED, "Invalid email or password", Level::INFO),
            AppError::Auth(AuthError::Forbidden) =>
                (StatusCode::FORBIDDEN, "You do not have access to this resource", Level::WARN),
            AppError::Auth(AuthError::CsrfMismatch) =>
                (StatusCode::FORBIDDEN, "Form expired, please retry", Level::WARN),
            AppError::Store(StoreError::NotFound) =>
                (StatusCode::NOT_FOUND, "Not found", Level::INFO),
            AppError::Store(StoreError::UniqueViolation { constraint }) =>
                (StatusCode::CONFLICT, friendly_constraint_message(constraint), Level::INFO),
            AppError::BadRequest(msg) =>
                (StatusCode::BAD_REQUEST, msg.as_str(), Level::INFO),
            // ...
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong", Level::ERROR),
        };

        // Log with full chain at chosen level; never leak internal details to user.
        emit_log(log_level, &self, status);

        // For htmx requests, return a tiny error fragment; for full-page, render error page.
        if is_htmx(&parts) {
            (status, [("HX-Retarget", "#error-banner")], Html(render_error_fragment(user_message))).into_response()
        } else {
            (status, Html(render_error_page(status, user_message))).into_response()
        }
    }
}
```

### Why this shape

- **User-visible messages are deliberately bland.** No "table 'issues' constraint 'foo' violated" leaks. The error message that operators need (for triage) goes to logs with the `request_id`; the user sees a stable, non-information-disclosing string.
- **htmx-aware**: htmx clients get a fragment to swap into a designated `#error-banner` element. Full-page clients get a rendered HTML error page (using the same askama templates as success pages).
- **Status codes are not extras to think about**: every variant explicitly maps to a status.
- **`Level::INFO` for expected user errors** (404, 401, validation failures); `Level::WARN` for security-relevant rejections; `Level::ERROR` only for 500s — these are the alerting threshold for the system-designer's dashboards.

## Request ID (NFR-OBS-04)

### Source

Every incoming request gets a `request_id: Uuid` (UUIDv7, time-ordered). Source priority:
1. If the request has an `X-Request-Id` header with a valid UUID, use it (allows upstream tracing).
2. Otherwise, generate fresh.

### Propagation

The `request_id` middleware (one of the first in the layer stack):

```rust
// crates/foundry-app/src/middleware/request_id.rs (sketch)
pub async fn request_id_layer(mut req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(uuid::Uuid::now_v7);

    req.extensions_mut().insert(RequestId(request_id));

    // Open a tracing span scoped to this request_id; every log inside this future
    // automatically carries the request_id field.
    let span = tracing::info_span!("http_request",
        request_id = %request_id,
        method = %req.method(),
        path = req.uri().path()
    );

    let mut response = span.in_scope(|| next.run(req)).await;
    response.headers_mut().insert("x-request-id", request_id.to_string().parse().unwrap());
    response
}
```

The `tracing` span ensures every subsequent log line inside the request automatically carries `request_id` — handlers do not have to thread it manually. The response header gives the client the ID for support tickets.

## Structured Logging (NFR-OBS-01)

### Stack

`tracing` + `tracing-subscriber` + `tracing-subscriber::fmt::layer().json()`. All other libraries that use `log` are bridged via `tracing-log` (one line in `main.rs`).

### Format

JSON line per event, e.g.:

```json
{
  "timestamp": "2026-05-23T15:14:22.123456Z",
  "level": "INFO",
  "target": "foundry_app::routes::issues",
  "message": "issue created",
  "request_id": "01979e2d-7a3c-7000-8001-abc123",
  "user_id": "01979e2d-7a3c-7000-8001-def456",
  "workspace_id": "01979e2d-7a3c-7000-8001-789abc",
  "project_id": "01979e2d-7a3c-7000-8001-abcdef",
  "issue_id": "01979e2d-7a3c-7000-8001-fedcba",
  "issue_key": "AUTH-9",
  "duration_ms": 12
}
```

### Where the standard fields come from

| Field | Source |
|---|---|
| `timestamp`, `level`, `target`, `message` | tracing default |
| `request_id` | request_id middleware span |
| `user_id`, `workspace_id` | auth middleware span (added after session resolution) |
| `project_id`, `issue_id`, `issue_key` | handler/service `.instrument(span!(...))` |
| `duration_ms` | per-handler `tower-http::trace::TraceLayer` |

### Levels

- `TRACE`: developer-only, off by default.
- `DEBUG`: dev/staging on, prod off. Per-handler entry/exit.
- `INFO`: significant business events (sign-in, create issue, send invite).
- `WARN`: recovered conditions (rate-limited request, CSRF mismatch, SMTP soft-fail).
- `ERROR`: 5xx-class failures and unexpected panics.

`RUST_LOG=info` is the production default. Overridable via env.

### No log-to-file (NFR-OBS-01)

Container writes to stdout; orchestrator captures. No `tracing-subscriber::fmt::FileWriter`. This avoids two failure modes: full-disk-from-logs and "logs survive container destruction." Both are operator concerns we punt to system-designer.

## Healthchecks (NFR-OBS-02)

Both endpoints live in `foundry-app/src/routes/health.rs`, mounted at `/healthz` and `/readyz`, unauthenticated, exempt from logging middleware (otherwise the LB poller floods logs).

### `/healthz` — process liveness

```text
GET /healthz -> 200 OK, body: "ok"
```

Returns 200 if and only if the process is alive enough to handle HTTP. Does *not* check DB. If a replica's DB connection is broken, we still want `/healthz` to be green so the orchestrator does not restart the replica when the real problem is shared (DB outage); `/readyz` is what flips.

### `/readyz` — traffic readiness

```text
GET /readyz -> 200 OK if ready; 503 with JSON body if not
```

Checks (in order; short-circuit on first fail):

1. Are migrations applied? (queried once at startup, cached as a flag)
2. Is the DB pool currently able to serve queries? (`SELECT 1` with 2s timeout against a pool conn; cached for 1s to avoid hammering DB per probe)
3. Is the replica currently in graceful shutdown? (`AtomicBool::draining`)

503 body shape:

```json
{ "status": "not_ready", "reason": "db_unreachable", "since": "2026-05-23T15:14:00Z" }
```

The structured body helps system-designer's alerting differentiate "DB down" from "draining" from "still booting."

## Graceful Shutdown (NFR-AVAIL-02)

`axum::serve(...).with_graceful_shutdown(shutdown_signal())` where `shutdown_signal()` awaits `tokio::signal::ctrl_c()` or SIGTERM. On signal:

1. Set `AppState::draining = true` (causes `/readyz` to 503 from here on).
2. Sleep for `PRE_SHUTDOWN_GRACE_SECONDS` (default 5) so LBs notice and drain.
3. axum's graceful shutdown stops accepting new connections; waits up to `SHUTDOWN_GRACE_SECONDS` (default 15, total wall-clock 20s) for in-flight requests.
4. Close the broadcast channel (SSE subscribers see a clean close; their EventSource reconnects to other replicas).
5. Drop the pool (sqlx waits for in-flight queries).
6. Exit 0.

K8s `terminationGracePeriodSeconds` must be >= 20s — flagged for system-designer.

## Metrics (NFR-OBS-03)

Application exposes metrics via the `metrics` + `metrics-exporter-prometheus` crates on a *sidecar HTTP port* (default 9090, configurable via `METRICS_PORT`). The sidecar listener is a separate `axum::serve` bound to a different `SocketAddr`. Metrics endpoint is unauthenticated by design; operators firewall it.

Default metric set (NFR-OBS-03 contract):

| Metric | Type | Labels |
|---|---|---|
| `http_requests_total` | counter | `path`, `method`, `status` |
| `http_request_duration_seconds` | histogram | `path`, `method` |
| `db_connections_in_use` | gauge | — |
| `db_connections_idle` | gauge | — |
| `sse_subscribers_total` | gauge | `project_id` (low cardinality; project count is small) |
| `outbox_pending_jobs` | gauge | — |
| `bootstrap_tokens_unclaimed` | gauge | — |
| `signin_attempts_total` | counter | `result=success|fail|delayed` |

The `path` label is **the route template** (e.g., `/team/:team_slug/project/:project_slug`), not the raw URI — otherwise cardinality explodes.

System-designer owns dashboarding and alerting; we just expose the data.

## Panic Handling

`std::panic::set_hook` installed in `main.rs` to log the panic via tracing before the default behavior (process exit) runs. Inside an axum handler, panics are caught by `tower::catch_panic` and converted to 500 responses (rather than crashing the whole process). For background tasks (`PgListener`, cleanup task), panics are caught at the task root and the task is restarted with exponential backoff.

## Error Budget for the MVP (informational)

Slice 1 has no SLO yet. NFR-PERF-01's 200ms P95 is the closest thing. Once telemetry exists in slice 2+, system-designer can propose SLOs against the metrics emitted above.

## Forward-compat: distributed tracing (deferred)

OpenTelemetry / OTLP exporters integrate cleanly with `tracing` via the `tracing-opentelemetry` crate. Not in MVP because (a) there is one binary today, (b) NFR-OBS doesn't require it, (c) adding it now costs concepts. The instrumentation (request_id, spans) is already shaped to translate without rework. Document in `docs/operations/observability.md` (slice-3) when added.
