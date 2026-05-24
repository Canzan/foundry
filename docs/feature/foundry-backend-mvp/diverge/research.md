# Research — Foundry Backend MVP

Evidence-grounded scan of the OSS issue-tracker landscape and the relevant 2026 Rust web ecosystem. Each subsection cites the project and the specific signal we're taking from it.

## 1. Competitive OSS Issue-Tracker Landscape

### 1.1 Plane.so (https://github.com/makeplane/plane)
- **Stack**: Django REST + Next.js + Postgres + Redis + Celery. ~70k LOC across two repos.
- **What they got right**: Linear-style UX, kanban+spreadsheet+gantt views, public roadmaps, well-marketed open core. Active community.
- **Where they over-engineered for our JTBD**: Five-container minimum (api, worker, beat, web, redis). Setup is 30+ minutes for a non-Python team. Self-host is a second-class citizen vs Plane Cloud — schema migrations frequently break community installs.
- **Where they under-engineered**: Python's GIL means each "fast" view is one of N workers; serious teams add gunicorn+nginx tuning. No realtime out of the box (long-poll + occasional WS).
- **Signal for Foundry**: A Rust single-binary or 2-container deploy is a *real* differentiator against Plane. Linear-feel is achievable; what they get wrong is operability.

### 1.2 OpenProject (https://github.com/opf/openproject)
- **Stack**: Rails + Postgres + Memcached + Puma. Mature (~15 years). PM-heavy feature set (gantt, budgets, BIM).
- **What they got right**: Self-host is first-class, has paid+OSS editions, well-documented K8s helm chart.
- **Where they over-engineered for our JTBD**: PM-tool DNA shows everywhere — too many concepts (work packages vs tasks vs phases). Onboarding tax is large.
- **Signal for Foundry**: Don't build PM-tool. Stay in dev-tracker lane.

### 1.3 Taiga (https://github.com/taigaio/taiga-back)
- **Stack**: Django + Angular + Postgres + RabbitMQ. Active but slower than the others.
- **What they got right**: Scrum/kanban primitives are clean.
- **Where they under-engineered**: Realtime is a paid add-on (Taiga Events sidecar). UX has not kept pace.
- **Signal for Foundry**: Realtime must be in-core, not a sidecar.

### 1.4 Tuleap (https://tuleap.org)
- **Stack**: PHP + MySQL. Enterprise ALM. Heavy.
- **Signal for Foundry**: Cautionary tale — every plugin became a config knob; the tool is now hard to evaluate without a deck.

### 1.5 Gitea/Forgejo issues (https://codeberg.org/forgejo/forgejo)
- **Stack**: Go single-binary + sqlite-or-postgres. Issues are a sub-system inside the forge.
- **What they got right**: **Single-binary deploy** — closest match to our "under an hour" outcome. Go's runtime makes 10MB images credible. Forge integration means issues + code are colocated.
- **Where they under-engineered for our JTBD**: The issues UX is forge-shaped (GitHub-issues clone), not workflow-shaped (Linear). No cycles/sprints, no priorities-as-first-class, no triage queue.
- **Signal for Foundry**: Forgejo proves single-binary self-host works at scale. Adopt that topology; pair with Linear-class UX. A Rust binary can match Go's deploy story.

### 1.6 Sentry (https://github.com/getsentry/sentry)
- **Stack**: Django + React + Postgres + Redis + Kafka + Clickhouse + Symbolicator (Rust). Multi-process.
- **What they got right**: Self-host story exists, container topology is documented.
- **Where they over-engineered for our JTBD**: Eight services minimum. Self-host is a hardship mode.
- **Signal for Foundry**: We are not Sentry — we are *one* domain (issues), not a telemetry pipeline. Match Forgejo's simplicity, not Sentry's surface.

### 1.7 Linear (proprietary, reference point)
- Front-end: TypeScript + React + custom GraphQL client with client-side state machine. Backend known to be Postgres + Redis + GraphQL.
- **What we are NOT trying to clone**: their offline-first sync engine (multi-quarter project of its own). MVP can be "online-only with optimistic UI" without losing the Linear *feel*.

### 1.8 Mastodon (Rails contrast)
- **Stack**: Rails + Sidekiq + Redis + Postgres + Elasticsearch.
- **Signal**: Self-hosters complain primarily about Sidekiq+Redis operational complexity and Rails memory. A *queue-by-Postgres* (e.g. river-rs/pgmq) strategy is increasingly common in Rust/Go projects to eliminate this exact pain.

## 2. Rust Web-Stack State of the Art (2026)

### 2.1 axum (https://github.com/tokio-rs/axum)
- Current stable: axum 0.8.x as of early 2026 (axum 0.7 → 0.8 migration was painful around Handler trait + State; 0.8.x is now the steady ground).
- Tower middleware ecosystem is the strongest in Rust — auth, tracing, timeouts, request limits all available as drop-in layers.
- Pattern: nest a `Router::new()` per domain module, share `AppState` via extractors.
- **Signal**: axum 0.8.x is boring-good. No reason to choose actix or rocket for this MVP unless the team has incumbent skill — both are fine but axum's middleware story is best-aligned with our "easy to bump deps" filter.

### 2.2 sqlx vs sea-orm vs diesel

| Aspect | sqlx | sea-orm | diesel |
|--------|------|---------|--------|
| Style | SQL-first; compile-time-checked queries against live DB | Active Record; entity macros | DSL/query builder; compile-time-checked |
| Async | Native | Native (built on sqlx) | Sync core (async wrapper exists, not idiomatic) |
| Migration tool | `sqlx-cli`, simple SQL files | `sea-orm-cli`, similar | `diesel migration`, mature |
| Cognitive load for new contributor | Low — it's SQL | Medium — entity DSL to learn | Medium-high — query DSL, schema.rs codegen |
| Forces SQL literacy | Yes | Hidden | Hidden |
| Macro magic | Moderate (`query!`, `query_as!`) | Heavy | Heavy |
| 2026 momentum | Strong, broad adoption | Strong, growing | Stable, slower |
| `taste/approachability` filter score | Best | Middle | Worst (DSL barrier) |

- **Signal**: For this JTBD's "Rust dev productive in a day" filter, **sqlx wins**. It also keeps the door open to swapping Postgres for doltgresql or similar (see §4) because the queries are plain SQL.

### 2.3 htmx 2.x ecosystem (2026)
- htmx 2.0 shipped 2024; 2.x is the stable line. Removes IE compat, cleaner extension API, `hx-on:*` syntax.
- htmx 3.x in alpha late 2025; htmx 4 beta (mentioned in the prompt) is the *future* but not the present.
- alpine.js 3.x is the dominant lightweight companion for client-side state islands; ~15KB gzipped.
- Server-rendering templates in Rust: `askama` (Jinja-ish, compile-time-checked) is the boring default; `maud` (DSL macros) is more idiomatic but adds learning tax; `minijinja` is a runtime option (Pythonic templates).
- **Signal**: htmx 2.x + alpine.js + askama is the well-trodden Rust+htmx path in 2026. We can flag htmx 4 migration as a future wave; the 2.x → 4.x story is closer to 2.x → 3.x (less disruptive than 1.x → 2.x).

### 2.4 Auth in Rust
- **Cookie+session**: `tower-sessions` crate (with `tower-sessions-sqlx-store`) is the path-of-least-resistance. Postgres-backed sessions = stateless app tier without Redis.
- **JWT**: `jsonwebtoken` crate is fine for service-to-service or SPA scenarios. For htmx (cookie-default), JWT is over-engineered.
- **OIDC delegation**: `openidconnect` crate works against Keycloak, Authentik, Dex, Auth0. Adds a startup-config knob.
- **Signal**: Cookie+session backed by Postgres meets the JTBD. OIDC should be an opt-in mode for org users who already run Authentik/Keycloak (this is *the* enterprise differentiator for self-host).

### 2.5 Realtime
- **SSE (Server-Sent Events)**: Trivial in axum — `Sse<impl Stream>`. One-way, reconnects automatically, no special middleware. Plays well with HTTP/2 multiplexing and most ingress.
- **WebSocket**: `axum::extract::ws`. Bidirectional but needs sticky session or pub-sub for multi-replica fan-out. More ops surface.
- **htmx + SSE**: `htmx-ext-sse` is the canonical extension; pages subscribe and swap DOM fragments on server events. This is exactly the pattern Hotwire/Turbo Streams use in Rails.
- **Signal**: SSE wins for this MVP. Postgres `LISTEN/NOTIFY` → tokio broadcast → SSE to subscribed clients is a 200-line pattern. WS only if we later need typing-indicator-style bidirectional events.

### 2.6 Background work
- **In-process tokio tasks**: Fine for fire-and-forget; lost on restart.
- **`apalis` (https://github.com/geofmureithi/apalis)**: Tokio-native job queue, Postgres/Redis/SQS backends. Idiomatic.
- **`river-rs`** and **`pgmq`** (Postgres-backed queues): Eliminate Redis. River specifically targets the "I just want Postgres" crowd; the Go original is widely deployed.
- **Signal**: Postgres-backed queue is the *resilience-without-Redis* story. For MVP, even simpler: `tokio::spawn` for non-durable work + a single `outbox`-style table polled every N seconds for the durable subset (send-email, webhook-out). We can adopt apalis/river later without a rewrite.

## 3. Architecture / Reliability / Maintainability / Security / Testing Gates Pre-Check

The skills repo at `/Users/jeffbailey/Projects/skills/src/` defines the review gates. Pre-checking the dimensions against our JTBD:

- **review-architecture**: A modular monolith with feature-cohesive directories (`workspaces/`, `issues/`, `comments/`, `auth/`, `realtime/`) beats a generic `controllers/services/repositories/` split. Avoid god-module utilities. Dependencies must point inward (HTTP/DB depend on domain, not vice versa).
- **review-reliability**: Liveness vs readiness probes separated. Graceful SIGTERM in axum (axum has `with_graceful_shutdown`). Outbound calls (webhooks) need explicit timeouts. Health check that hits Postgres for readiness, simple 200 for liveness.
- **review-maintainability**: No macro-heavy DSLs (this kills `sea-orm` and `maud` for the recommended path). Functions under 50 lines. Avoid bespoke abstractions — use crates the wider Rust community already knows.
- **review-security**: Cookies must be `HttpOnly`+`Secure`+`SameSite=Lax`. CSRF tokens on state-changing requests (axum-csrf or hand-rolled double-submit). bcrypt/argon2 password hashing (argon2id preferred 2026). Parameterized queries enforced by sqlx. No raw SQL string concat.
- **review-testing**: Happy-path-only is acceptable per scope, but the *shape* of tests must be unit-heavy with one or two integration tests per domain module hitting a real test Postgres. No flaky network tests.

The recommended direction (next doc) is built to pass these gates by default.

## 4. doltgresql Assessment (one paragraph, evidence-grounded)

[doltgresql](https://github.com/dolthub/doltgresql) is a Postgres-protocol implementation of Dolt's branching/versioned database (Dolt's wire protocol was previously MySQL-only). As of 2026 it's beta-quality — fine for evaluation, *not yet* the safe default for an issue tracker that will hold years of customer planning data. The compelling use case for Foundry would be **per-feature schema branches** (try a custom-field schema on a branch, merge after acceptance) and **time-travel queries** (what did the backlog look like at 3pm yesterday?). Both are genuinely powerful for a tracker. The right MVP posture: keep vanilla Postgres + use `sqlx` so the SQL is portable; design the schema with explicit `created_at`/`updated_at`/`deleted_at` columns so that even *without* doltgresql we have point-in-time queryability for the next 12 months. Flag doltgresql as a focused experiment for v2 once the wire-protocol is GA, not as a MVP risk.

## 5. License Hygiene Reference

The license decision is its own taste call (see directions.md for the trade-off matrix), but the dependency graph constrains us:

- **axum (MIT)**, **tokio (MIT)**, **sqlx (Apache-2.0 OR MIT)**, **tower (MIT)**, **askama (MIT OR Apache-2.0)**, **tower-sessions (MIT OR Apache-2.0)**, **argon2 crate (MIT OR Apache-2.0)**, **htmx (BSD-2)**, **alpine.js (MIT)** — all permissive.
- All choices are compatible with **MIT**, **Apache-2.0**, or **AGPLv3** for Foundry itself.
- License choice for Foundry is a *strategic* call (defensive vs maximally-adoptable), not a *compatibility* call. See directions.md.

## 6. Key Takeaways Feeding the Directions

1. **Single-binary deploy is achievable and differentiated.** Forgejo proves it; Rust can match.
2. **Postgres for everything (data + sessions + queue + pubsub) is the resilience-without-Redis play.** Backed by river/pgmq/LISTEN-NOTIFY patterns. This is the boring-tech-wins move.
3. **sqlx + askama + htmx 2.x + alpine.js + tower-sessions** is the "Rust dev productive in a day" baseline.
4. **SSE > WS for MVP.** Less ops surface; sufficient for issue-update fan-out.
5. **Auth: cookie+session baseline, OIDC as an opt-in mode.** This serves both indie self-hosters and enterprise.
6. **Modular-monolith feature-cohesive layout** beats generic n-tier layout for the contributor-onboarding outcome.
7. **License**: AGPLv3 is the strategic anti-Linear-clone defense; Apache-2.0 is the maximum-adoption play. Pick deliberately, document the reasoning.
