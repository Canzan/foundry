# Foundry Backend MVP — Out of Scope

Items intentionally deferred from this MVP slice. Each item links back to the JTBD outcomes (`docs/feature/foundry-backend-mvp/diverge/jtbd.md`) or to the rationale in `docs/feature/foundry-backend-mvp/diverge/recommendation.md`.

The purpose of this list is to make the scope boundary explicit so:

- DESIGN and DELIVER don't accidentally do hidden work on these.
- Future feature directories have a starting point.
- Users / contributors who ask "does Foundry do X?" can be answered cleanly: "Not in v0.1; planned for vN."

---

## Deferred to later versions

### OIDC / SSO authentication

- **Why deferred**: DIVERGE decision (a) — the indie/early-stage segment is the entry funnel; enterprise SSO is the v0.3 opt-in mode (D5 becomes a feature, not the architecture).
- **JTBD link**: Outcome #2 (data sovereignty) is still served by US-01 + US-03; SSO addresses outcome #5 (agentic workflows via service-account tokens) plus the enterprise procurement gate.
- **Target version**: v0.3.
- **Open work to track**: openidconnect crate spike; JWT cookie vs session migration story; Pocket-ID/Authentik/Keycloak/Dex provider matrix.

### Agentic workflow runner (ollama, agentgateway.dev)

- **Why deferred**: Direction D1's outbox table is the seam for future agentic integration; building the runner before the JSON API and outbox have shipped is premature.
- **JTBD link**: Outcome #5 — explicitly marked "acceptable" in DIVERGE (JSON API exists; agentic hooks live in the outbox).
- **Target version**: v0.4 or v0.5 — once API surface is stable.
- **Open work to track**: outbox event schema; agent auth model (service accounts? OIDC client_credentials?); rate-limit story.

### Forgejo CD pipelines

- **Why deferred**: CI/CD integration is downstream of the issue tracker itself. Foundry needs a stable v0.1 before it integrates with anything.
- **JTBD link**: Tangential to outcome #5 (agentic substrate); not in the under-served outcome set.
- **Target version**: v0.5+.

### htmx 4 beta migration

- **Why deferred**: htmx 2.x is the stable production target for 2026. NFR-MIG note: keep `hx-*` surface minimal and prefer vanilla EventSource for SSE, both of which reduce htmx-4 migration cost when the time comes.
- **JTBD link**: Outcome #7 (upgrade between minor versions) — handled at *Foundry* version layer; htmx is a transitive concern.
- **Target version**: v0.5 or whenever htmx 4 hits stable.

### doltgresql experiment

- **Why deferred**: DIVERGE: vanilla Postgres for MVP; doltgresql is a v1.0 experiment, not an MVP risk.
- **JTBD link**: Indirect — branched-issue-graphs are a possible v1.0 differentiator but not in the under-served outcome set today.
- **Target version**: v1.0 candidate experiment.

### Full Kubernetes manifests

- **Why deferred**: docker-compose is the "under an hour" path. K8s manifests are a follow-on for ops-mature operators; we commit to K8s-*translatable* docker-compose now (NFR-PORT-01) without shipping K8s manifests.
- **JTBD link**: Outcome #6 (multi-replica) is delivered via docker-compose scaling; K8s is a deployment-substrate choice that doesn't change the application architecture.
- **Target version**: v0.4 (after first 100 self-hosters validate docker-compose path).

### Large test-suite buildup (>200 tests)

- **Why deferred**: MVP test focus is: (a) every UAT scenario in stories.md has at least one acceptance test; (b) every NFR has at least one validating test or benchmark. Beyond that, test coverage grows organically as bug fixes accrue.
- **JTBD link**: Outcome #3 (contributor productivity) requires a *fast* test suite, not necessarily a *large* one.
- **Target version**: Ongoing; no specific build-up milestone.

---

## Deferred (smaller scope, called out in stories)

These were discovered during story writing as places we could expand but explicitly chose not to.

### Multi-workspace per Foundry instance

- **Why deferred**: MVP supports ONE workspace per instance (US-05). Multi-tenancy adds significant authentication, authorization, and UI complexity. Operators wanting multi-workspace can run multiple Foundry instances on different ports/subdomains.
- **JTBD link**: Outcome #1 — keeping the install model simple.
- **Target version**: v0.4+.

### Cross-team issue assignment

- **Why deferred**: US-07 + US-08: assignees are scoped to the issue's team's members. Cross-team assignment introduces visibility/permission complexity.
- **JTBD link**: Linear-parity table-stakes — but most teams default to team-scoped assignment.
- **Target version**: v0.3 (likely a single-flag addition).

### Custom issue states / workflows

- **Why deferred**: US-07: state list is fixed (Backlog/Todo/In-Progress/Done/Cancelled). Custom workflows are a big surface and DIVERGE deliberately left them as v0.4+.
- **JTBD link**: Important for Linear-parity in some teams, deferrable for others.
- **Target version**: v0.4.

### Threaded comments

- **Why deferred**: US-10: linear comment list only. Threading adds UI complexity for marginal value in issue tracking (where most discussion is short).
- **JTBD link**: Not under-served (Linear itself doesn't have threaded issue comments).
- **Target version**: Won't (deliberate non-goal unless evidence appears).

### SSE event replay on reconnect

- **Why deferred**: US-09 documented limitation: reconnected clients see a "may have missed events" toast and refresh. Replay needs an event-store and `Last-Event-Id` semantics — non-trivial.
- **JTBD link**: Outcome #4 (Linear-feel realtime) — partially served; degrades to refresh after reconnect.
- **Target version**: v0.4.

### Postgres full-text search

- **Why deferred**: US-12: search uses simple ILIKE for MVP. Postgres FTS works but adds tsvector columns + GIN indexes + ranking complexity.
- **JTBD link**: Outcome #4 — search speed only matters at scale (>5,000 issues per workspace).
- **Target version**: v0.3 (likely; trigger is operator complaints about slow search at scale).

### Email notifications for issue activity

- **Why deferred**: US-05 covers invite emails. Activity emails (mentions, assignments) require an opt-in preferences model + an email queue. Realtime UI updates partially substitute.
- **JTBD link**: Indirect — discussion co-location.
- **Target version**: v0.3.

### Mobile-responsive UI

- **Why deferred**: MVP targets desktop browsers. The htmx + alpine stack is responsive-friendly but explicit mobile polish (touch keyboard shortcuts, mobile board view) is out of scope.
- **Target version**: v0.4.

### Rate limiting

- **Why deferred**: US-06 ships brute-force-delay for sign-in (NFR-SEC-02) but no general API rate limiting. Self-host with trusted users mitigates risk for MVP.
- **JTBD link**: Indirect — security guardrail.
- **Target version**: v0.3 when public-facing instances appear.

### In-app notifications (bell icon, notification feed)

- **Why deferred**: Realtime board updates (US-09) partially substitute. A persistent notification feed is a real feature, not deliverable in MVP.
- **Target version**: v0.3.

### Audit log UI

- **Why deferred**: Admin-visible audit trail of user actions. Important for compliance-driven adoption. Data exists (we'll have created_by/updated_by columns) but a UI is post-MVP.
- **Target version**: v0.4.

---

## Explicit non-goals for v0.1 (Won't list)

- **No GraphQL API**: REST + htmx is the API surface. GraphQL is a "won't have" unless an explicit driver emerges.
- **No real-time collaborative editing of descriptions** (Google-Docs-style multi-cursor): too complex for the value; users edit serially with realtime save indication.
- **No browser-side encryption / E2EE**: server sees all data. Trust model = "you run the server."
- **No mobile native apps**: web only.
- **No issue templates per project**: nice-to-have, deferred.
- **No SLAs, time tracking, sprint planning**: out of the JTBD strategic frame; "next layer of Linear features" if v0.1 succeeds.

---

## Re-evaluation Triggers

Items above should be re-evaluated when:

- **OIDC**: First 5 enterprise evaluators say "can't proceed without SSO." Trigger: v0.3.
- **K8s manifests**: First operator reports they cannot deploy to their orchestrator without manifests. Trigger: v0.4.
- **Postgres FTS**: First operator reports search latency >500ms at >5,000 issues. Trigger: v0.3.
- **Email activity notifications**: User survey shows ≥50% feel "I miss things." Trigger: v0.3.
- **Custom workflows**: ≥3 user requests for state customization. Trigger: v0.4.

Each trigger is a signal, not a commitment.
