# Web-Tier Extraction — Out of Scope

> DRIVER-CORRECTED (2026-05-30). The JSON API is now first-class: **JSON reads AND writes
> (create/update issues, comments, projects, state) and machine-token authentication are now
> IN scope.** This file states what remains OUT, so DESIGN/DELIVER do not do hidden work and so
> "does the API also do X?" has a clean answer.

The single most important framing: **this is a MODULE boundary inside one binary, not a
service split, and not a frontend rewrite.** Web and API are PEER consumers of one core.
Everything below follows from that.

---

## Now IN scope (changed by the 2026-05-30 driver correction)

- **JSON API writes** — create/update issues and comments (and issue state change) via JSON
  (US-W05c). Was previously deferred; now a headline deliverable.
- **Machine-token authentication** — a first-class machine credential for programmatic clients
  (US-W05b), additive to the browser session/CSRF model. Was previously deferred; now in scope
  as a REQUIREMENT (the token MECHANISM remains a DESIGN decision — see "Deferred to DESIGN").

---

## Hard non-goals (Won't have — defining the shape of the feature)

### NOT a two-service split
- The web tier and api tier are **crates/modules in the one `foundry` binary**, calling
  core/store **in-process**. We are explicitly NOT creating a separate api service, a
  separate web server, a gateway, or any network hop between tiers.
- **Why**: the README brand promise is "one Postgres, one binary, one `docker compose up`".
  A network boundary would add latency (hurting jtbd-outcome-4 Linear-feel) and an ops tax.
- **Re-evaluation trigger**: only if Foundry ever needs independent scaling of web vs api —
  not foreseen for the self-host segment.

### NOT a SPA / React / Vue / Svelte rewrite
- The web tier is **server-rendered HTML + htmx fragments + Alpine.js**, exactly as today —
  just rendered from templates instead of `format!` literals. We are NOT introducing a
  client-side SPA framework, client-side routing, or a JSON-hydrated frontend.
- **Why**: server-rendered htmx is the chosen architecture (README + backend-mvp); an SPA
  would contradict it and require a Node toolchain and a client-state model.

### NOT a Node/JS runtime service or a CDN dependency
- Assets are vendored and served by the binary. We are NOT adding a Node server, a runtime
  JS bundler service, or a CDN dependency. (A *build-time* asset step is an open DESIGN
  question; a *runtime* service is a hard non-goal.)
- **Why**: "no new runtime services", air-gap-friendliness, one-binary ethos.

### NOT a change to the BROWSER authentication / authorization model
- The existing browser path is **unchanged**: CSRF (double-submit + `HX-CSRF` header), sessions
  (tower-sessions Postgres), password auth (argon2id), brute-force delay, non-enumerable errors,
  membership/admin/authorship checks. The machine-token surface (now in scope, US-W05b) is
  **purely additive** — it adds a new credential type for machines without altering the human
  browser flow, and API writes reuse the SAME authorization core enforces for the UI.
- **Why**: the browser path is where bugs cluster; the new auth surface must not perturb it. The
  token MECHANISM (issuance/format/storage/rotation/revocation/scoping) is DESIGN, not DISCUSS.
- **Still out**: OIDC / SSO / external identity providers (v0.3+); per-endpoint OAuth scopes
  beyond the team/admin scoping implied by US-W05b; changing the human password/session model.

### NOT new infrastructure
- No Redis, no S3, no message broker, no second database, no new container. Topology stays
  foundry + postgres.

---

## Deferred to DESIGN (decisions this wave deliberately does NOT make)

### htmx 2 migration / version choice
- **Why deferred**: per the brief and backend-mvp out-of-scope, htmx 2 is a downstream design
  decision. DISCUSS only records that (a) the codebase mixes `hx-` and `data-hx-` prefixes
  today, and (b) normalizing them and choosing/upgrading the htmx version is DESIGN's call.
- **Tracked signal**: the prefix split lives in `issues.rs`/`comments.rs` and is preserved
  as-is in Slice 1; DESIGN normalizes when it picks a version.
- **Target**: DESIGN wave of this feature (version pin) / later (htmx 4 per backend-mvp note).

### Template engine choice
- **Why deferred**: solution-neutral in DISCUSS. The NFR (render budget, one-partial rule) is
  the constraint; the engine is DESIGN's to pick.

### JSON content negotiation, route prefix, serialization shape, and API versioning mechanism
- **Why deferred**: US-W05a/c establish that JSON read+write endpoints exist, reuse core, and
  expose a STABLE versioned contract; the exact `/api` prefix vs `Accept`-header negotiation,
  the serde request/response shapes, status-code conventions, PATCH-vs-PUT/idempotency, and the
  versioning mechanism (URL vs header vs media type) are DESIGN. The CONSTRAINT (stable contract,
  no HTML from JSON handlers, write rule-parity with the UI) is fixed in `nfrs.md` (NFR-WEB-API).

### Machine-token mechanism (issuance, format, storage, rotation, revocation, scoping)
- **Why deferred**: US-W05b establishes the REQUIREMENT for a first-class, additive,
  admin-governable machine token and its security constraints (NFR-WEB-API-SEC-01..03). HOW the
  token is minted (CLI? admin screen? bootstrap?), its format/prefix, how it is hashed/stored,
  whether it rotates/expires, and the scoping granularity are all DESIGN. This is a NEW security
  surface — DESIGN must treat it with password-model rigor.

### Boundary-guard mechanism
- **Why deferred**: US-W06 requires that CI fails on a boundary violation; whether that is
  cargo-deny, cargo-modules, a clippy lint, or a custom xtask is DESIGN.

### Asset build pipeline details
- **Why deferred**: whether assets are pre-minified at build time, the directory layout, and
  cache-busting strategy are DESIGN, constrained only by "no runtime service, no CDN".

---

## Deferred to later feature versions (smaller scope)

### Broader API resource coverage (projects CRUD, attachments, search, listing/pagination filters)
- **Why deferred**: US-W05a/c cover the SMALLEST real read+write surface (board issues read;
  issue+comment create/update; issue state change) to prove the first-class API end-to-end.
  Full projects CRUD over JSON, attachment upload/download via JSON, search/query endpoints, and
  rich list filtering/pagination follow the same pattern once proven.
- **Target**: follow-on slices after this feature lands; same approach, lower risk.

### API rate-limiting / quotas / abuse protection
- **Why out (decided)**: a self-hosted single-binary tool with admin-issued, revocable tokens
  does not need per-token rate-limiting or quotas in this feature. Revocation (US-W05b) is the
  abuse control. Add rate-limiting only if a hosted/multi-tenant deployment emerges.
- **Target**: only if Foundry offers a hosted/multi-tenant mode (not foreseen for self-host).

### OpenAPI / generated SDK / API documentation generation
- **Why out (decided)**: the stable versioned contract (NFR-WEB-API-CON-01) is the guarantee;
  auto-generating an OpenAPI spec, client SDKs, or a docs site is a separate enhancement and is
  not required to ship a usable first-class API.
- **Target**: post-feature enhancement once the contract has stabilized.

### Outbound webhooks / API-push notifications to external systems
- **Why out (decided)**: this feature makes Foundry DRIVABLE (inbound API). Foundry NOTIFYING
  external systems (outbound webhooks, event subscriptions) is the inverse direction and a
  separate feature; the internal outbox exists but is not exposed externally here.
- **Target**: separate feature (e.g. "Foundry webhooks") if integrator demand appears.

### Extracting non-core surfaces (attachments, projects-create, bootstrap, SSE) to templates
- **Why deferred**: this feature extracts the THREE highest-value surfaces (board, issue+
  comments, sign-in) plus the JSON tier and the guard. The remaining surfaces
  (`attachments.rs`, `projects::show_create_form`, `bootstrap.rs`, `events.rs` HTML) follow
  the same pattern and can be extracted incrementally once the pattern is proven.
- **Target**: follow-on slices after this feature lands; same approach, lower risk.

### Mobile-responsive polish
- **Why deferred**: inherits backend-mvp's desktop-first scope. The template/CSS pipeline
  makes responsive work *easier* later, but explicit mobile polish is out of scope here.
- **Target**: v0.4 (per backend-mvp out-of-scope).

### Design system / theming / dark mode
- **Why deferred**: the CSS in scope is "looks intentional, accessible, consistent" — not a
  token-based design system, theming, or dark mode.
- **Target**: post-extraction enhancement.

---

## Re-evaluation Triggers

| Item | Trigger | Likely target |
|------|---------|---------------|
| Two-service split | Web and api need independent scaling (not foreseen for self-host) | none planned |
| Broader API resource coverage (projects/attachments/search) | Core read+write API proven; integrators need more resources | follow-on slices |
| API rate-limiting / quotas | A hosted/multi-tenant deployment emerges | only if hosted mode |
| OpenAPI / SDK generation | Contract stable; external integrators want generated clients | post-feature |
| Outbound webhooks | Integrators need Foundry to push events out | separate feature |
| OIDC / SSO for humans | Enterprise identity requirement | v0.3+ |
| Remaining surface extraction | This feature's pattern proven green on board/issue/sign-in | follow-on slices |
| htmx 2 normalization | DESIGN picks the version | DESIGN of this feature (Feature B if split) |
| Mobile polish | ≥3 reports of unusable mobile board | v0.4 |
