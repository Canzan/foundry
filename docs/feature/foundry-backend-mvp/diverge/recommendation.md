# Recommendation — Foundry Backend MVP

## TL;DR

**Recommend Direction D1 — "Boring Monolith"** with three explicit follow-on hooks (D2's mode-switching at v0.5, D5's OIDC mode at v0.3, doltgresql evaluation at v1.0). License Foundry as **AGPLv3**. The recommendation is derivable from the scoring matrix below — D1 wins on weighted total, and the dissenting case (D5) is documented.

## DVF Filter

Applied to each direction. Threshold for elimination: DVF total < 6.

| Direction | Desirability | Feasibility | Viability | Total | Status |
|-----------|-------------|-------------|-----------|-------|--------|
| D1 Boring Monolith | 5 (matches "under an hour" outcome directly) | 5 (boring tech, well-known crates) | 5 (lowest ops cost = highest self-host adoption) | 15 | Pass |
| D2 Two-Mode Binary | 4 (option value, not user-visible) | 4 (mode dispatch adds some code) | 4 (good story for v1) | 12 | Pass |
| D3 Web Components | 4 | 3 (smaller community, more authoring care) | 4 | 11 | Pass |
| D4 CP/DP Split | 3 (premature for the 5-50 person target) | 4 | 3 (extra container hurts adoption) | 10 | Pass (but weak) |
| D5 OIDC-Delegated | 5 for enterprise, 3 for indie → 4 average | 4 (openidconnect crate is solid) | 4 | 12 | Pass |

All five survive DVF. Proceed to taste scoring.

## Weights (Developer Tool Profile)

This is a developer tool, so we use the developer-tool column from the taste-evaluation rubric — with one explicit adjustment.

| Criterion | Default Weight (dev tool) | Foundry Weight | Rationale for adjustment |
|-----------|--------------------------|----------------|---------------------------|
| DVF average | 25% | 25% | Unchanged |
| T1 Subtraction | 15% | 20% | The JTBD's "single dev productive in a day" outcome is dominated by what we don't ship; raise weight. |
| T2 Concept Count | 20% | 20% | Unchanged |
| T3 Progressive Disclosure | 15% | 10% | First interaction for an *operator* is `docker compose up`, not a UI flow. T3 matters less than for an end-user product. |
| T4 Speed-as-Trust | 25% | 25% | Critical — this whole project is justified by "we want Linear's speed." |

**Total = 100%. Weights locked before scoring.**

## Taste Scoring Matrix

| Direction | DVF (avg of D/V/F /5) | T1 Sub | T2 Concept | T3 Prog | T4 Speed | Weighted Total |
|-----------|-----------------------|--------|------------|---------|----------|---------------|
| **D1 Boring Monolith** | 5.0 | 5 | 5 | 4 | 5 | **4.90** |
| D2 Two-Mode Binary | 4.0 | 3 | 3 | 4 | 5 | 3.85 |
| D3 Web Components | 3.7 | 4 | 3 | 4 | 5 | 3.93 |
| D4 CP/DP Split | 3.3 | 2 | 2 | 3 | 5 | 3.18 |
| **D5 OIDC-Delegated** | 4.0 | 4 | 3 | 3 | 5 | **3.95** |

### Score derivation (per criterion, per direction)

**T1 Subtraction (could one more thing be removed?)**
- D1: 5 — Postgres + axum + htmx + askama is already the minimum coherent stack.
- D2: 3 — adds mode-dispatch we don't need for MVP.
- D3: 4 — replaces alpine with WCs (smaller dep), but adds component-author concept.
- D4: 2 — pre-splits into two binaries we don't yet need.
- D5: 4 — removes our password store but adds an OIDC dependency.

**T2 Concept Count (new concepts for a first-time contributor)**
- D1: 5 — axum routes, sqlx queries, askama templates, htmx attributes, SSE. All boring.
- D2: 3 — adds "modes" concept.
- D3: 3 — adds web-component lifecycle concept (unfamiliar to Rust devs).
- D4: 2 — two binaries, shared types crate, internal API boundary.
- D5: 3 — adds OIDC flow concept (often new to Rust-only devs).

**T3 Progressive Disclosure (first interaction)**
- D1: 4 — `docker compose up`, browse, register, go. Five exposed surfaces (issues, projects, teams, comments, labels).
- D2: 4 — same as D1 in default mode.
- D3: 4 — same as D1.
- D4: 3 — operator must understand cp vs dp before deploying.
- D5: 3 — first interaction requires choosing IdP setup (bundled vs external).

**T4 Speed-as-Trust (perceived responsiveness)**
- All 5 — Rust + server-render + SSE delivers the same perceived speed regardless of direction. This is not a meaningful discriminator across these directions because they all use the same hot path.

### Weighted-total math (D1, shown explicitly)

```
D1 = (5.0 × 0.25) + (5 × 0.20) + (5 × 0.20) + (4 × 0.10) + (5 × 0.25)
   = 1.25 + 1.00 + 1.00 + 0.40 + 1.25
   = 4.90
```

D5 weighted total = (4.0 × 0.25) + (4 × 0.20) + (3 × 0.20) + (3 × 0.10) + (5 × 0.25) = 1.00 + 0.80 + 0.60 + 0.30 + 1.25 = **3.95**

D3 weighted total = (3.7 × 0.25) + (4 × 0.20) + (3 × 0.20) + (4 × 0.10) + (5 × 0.25) = 0.925 + 0.80 + 0.60 + 0.40 + 1.25 = **3.975**

(D3 is essentially tied with D5; D1 wins clearly.)

## Recommendation: D1 — Boring Monolith

D1 wins the scoring matrix at 4.90 vs the next-best at 3.95 — a one-point gap on a 5-point scale. The win is on T1 (we ship the minimum coherent stack) and T2 (every concept a contributor needs is something they already recognize: HTTP, SQL, HTML, sessions, polling). The matrix follows from the JTBD: the strongest opportunity scores were #1 (hour-to-running) and #3 (contributor productivity), both of which T1 and T2 dominate.

### Why D1 scores well

- **Subtraction**: The stack is already minimal. There is no "and Redis" or "and Celery" or "and a separate frontend container." Postgres carries data, sessions, the queue, and pubsub. We are not paying for capabilities we don't need.
- **Concept count**: A new contributor needs to know axum (HTTP), sqlx (SQL), askama (templates), htmx (attributes), and tower-sessions (cookies). All five are widely-known, doc-rich, and chosen specifically because their conceptual surface is small.
- **Speed-as-Trust**: Rust + server-render is the fastest server-side stack available in 2026 for this workload class. Server-side render of an issue list page should hit P95 < 50ms on commodity hardware. SSE on Postgres LISTEN/NOTIFY is the lowest-latency realtime fanout we can build without operating Redis.

### Core trade-off

D1 trades **future-flexibility option value** for **present-moment simplicity**. If we ever need to split the data plane and control plane (Direction D4) or scale workers separately, that is a refactor — not a no-op. We accept this because:

1. The 5-50 person team target won't need that split for 12-24 months at the earliest.
2. The codebase will be small enough at that point (~10-15k LOC estimated) that the refactor is a sprint, not a rewrite.
3. We retain the optionality cheaply by keeping the domain code feature-cohesive (workspaces/, issues/, etc.) — when we split, we split at module boundaries.

### Key risk

**R**: The outbox-polling + LISTEN/NOTIFY pattern hasn't been operated at scale by us. If it turns out to be flaky under load, we'd graduate to `river-rs` or `apalis` with Postgres backend — both drop-in replacements. **Mitigation**: build the outbox abstraction behind a trait, so swapping the implementation is a 1-file change.

### Hire criteria for D1

A team chooses D1 when:
- They have 1-2 Rust devs and need contributor-friendly code more than enterprise-feature-readiness.
- Self-host setup time is a real adoption blocker.
- They are OK starting without SSO and adding it later as the OIDC mode (D5 becomes a feature, not the architecture).
- They want to ship something credible in 8-12 weeks, not 6 months.

## Dissenting Case: D5 — OIDC-Delegated

The scoring matrix's next-best is D5 at 3.95. It deserves explicit treatment because the score gap is small relative to the strategic difference.

**Why D5 might be the right call instead:**

- **Segment targeting**: If our actual target customer is the 50-500 person engineering org that already runs Authentik or Keycloak, D5's day-one SSO is a *binary* qualifier — without it, Foundry doesn't get past procurement. The indie segment becomes secondary.
- **Linear-replacement realism**: Most teams big enough to chafe at Linear's per-seat pricing are also big enough to require SSO. D1 + "SSO coming in v0.3" may not land in 2026 — that's an excuse, not a sale.
- **Security surface**: Not shipping a password store is a real benefit. Every bcrypt-default-cost CVE, every password-reset-token-leak, every rate-limit-on-login bug we don't have to write is a win.

**Why we still recommend D1**: The JTBD framing explicitly names "team of 5-50 people" and "developer who wants to run one in under an hour." D5 sacrifices the indie/early-stage path that we believe is the entry funnel for Foundry adoption. Adopters start indie, then bring it to work, then bring SSO requirements. **D1 with OIDC as a v0.3 opt-in mode (essentially merging in D5's stack as a feature flag) gets us both** — and the recommendation explicitly bakes that in below.

If the user reverses this choice (segment is enterprise-only from day one), this entire recommendation flips to D5 cleanly. The scoring matrix and weights are the same; only the segment assumption changes.

## Other Cross-Cutting Recommendations (carried into DISCUSS)

| Decision | Recommended choice | Reasoning |
|----------|-------------------|-----------|
| Persistence library | `sqlx` | "Plain SQL, compile-checked." Beats sea-orm on T2 (concept count) and diesel on async-fit. |
| Templates | `askama` | Compile-checked, Jinja-shaped (familiar to most web devs), no macro DSL learning curve. |
| Frontend interaction | htmx 2.x + alpine.js | The dominant stable pair in 2026. WCs (D3) deferred unless a specific component demands it. |
| Realtime | SSE + LISTEN/NOTIFY → tokio broadcast | One-way fan-out for issue updates. WS deferred. |
| Background | Postgres `outbox` table polled with `pg_notify` wake-up | No Redis. Graduate to river-rs only if needed. |
| Auth (v1) | `tower-sessions` with Postgres store + argon2id | Stateless app tier, no Redis. |
| Auth (v2, opt-in mode) | OIDC via `openidconnect` crate; HttpOnly JWT cookie | Add as a feature flag for shops with an IdP. |
| Database | Vanilla Postgres for MVP | doltgresql is a v1.0 experiment, not an MVP risk. |
| License | **AGPLv3** | Defensive against SaaS reclone; permissive deps; dual-licensing path documented for v2. |
| Deployment story | docker-compose first, K8s manifests in a later wave | Match "under an hour" outcome. |

## Taste Filter Scorecard (the four user-specified taste filters)

| Filter | D1 Score (1-5) | Notes |
|--------|---------------|-------|
| **Approachability** (Rust dev productive in a day) | 5 | Smallest concept count; main.rs reads as the system map. |
| **Upgrade-ability** (pin but bump easily) | 5 | All chosen crates have strong semver; we'll use `cargo update --precise` patterns and pin to minor versions. |
| **Boring tech where boring wins** | 5 | Postgres for everything is the boring play. |
| **Resilience** (multi-replica, sessions/cache/files explicit) | 4 | Sessions: Postgres. Cache: none in MVP — graduate to in-process tower-http cache + LISTEN/NOTIFY invalidation later. File uploads: Postgres `bytea` or S3-compatible (env-configurable) — flag for DISCUSS. |
| **License hygiene** (every dep compatible with AGPL) | 5 | Verified in research.md §5 — all chosen crates are permissive (MIT/Apache-2.0/BSD), compatible with AGPLv3. |

## "What we'd write in DISCUSS next" — Preview

A handful of user stories that fall out of D1 cleanly. Owner: product-owner agent. Sketched here so the DISCUSS wave knows what to expect.

1. **As a self-hoster**, I can `docker compose up` and have a working tracker at `localhost:3000` in under 5 minutes, with one admin invite link printed to the logs.
2. **As an admin**, I can create my first workspace, team, and project through the htmx UI without touching the database.
3. **As a developer**, I can create, view, edit, and close an issue using only keyboard shortcuts that match Linear's defaults for the same actions.
4. **As a developer**, I see issue updates (state, assignee, comments) appear without a page refresh, within 1 second of another user's action.
5. **As an operator**, I can run 3 replicas of the app behind a load balancer and lose any one of them without losing user sessions or in-flight realtime subscriptions.
6. **As an operator**, I can take a single `pg_dump` and restore it on another machine to get a complete copy of the running tracker.
7. **As a contributor**, the repo's README walks me from `git clone` to a green test run in 10 minutes on a fresh laptop.
8. **As an enterprise evaluator** (deferred to v0.3), I can flip a config flag and Foundry authenticates users against my Authentik/Keycloak/Dex instance via OIDC.

These will be sized and prioritized by the product-owner wave. The architecture from D1 makes all eight achievable without revisiting topology choices.

## Decision Statement (handoff to DISCUSS)

> **Proceed with Direction D1 ("Boring Monolith"): a single axum binary, vanilla Postgres for data + sessions + queue + pubsub, htmx 2.x + alpine.js frontend, SSE realtime, AGPLv3 license — assuming (a) the indie/early-stage segment is the entry funnel, (b) OIDC SSO is acceptable as a v0.3 opt-in mode rather than a v1.0 requirement, and (c) operator tolerance for a single docker-compose-up deploy is the primary "hour-to-trial" measure of success.**

If any of (a), (b), or (c) is wrong, the next-best direction is D5 with cookie+session as the fallback mode — same stack, identity flipped.

## Open Decision Questions for the User (top 3)

The user should resolve these in the converge step before product-owner writes stories:

1. **Segment**: indie/early-stage first (recommendation stands → D1), or enterprise-with-SSO from day one (flip → D5)?
2. **License**: AGPLv3 (defensive, recommended) or Apache-2.0 (maximum-adoption, defensible-loss)? This is reversible early, painful late.
3. **File uploads in MVP**: in-Postgres `bytea` (simplest, scale ceiling ~1GB/issue total) or S3-compatible-only with no local fallback (forces minio in docker-compose)? The recommendation is "make it env-configurable, default to Postgres bytea for the under-an-hour story, document minio for production."
