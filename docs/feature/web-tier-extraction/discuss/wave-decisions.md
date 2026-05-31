# Web-Tier Extraction — DISCUSS Wave Decisions

> DRIVER-CORRECTED (2026-05-30). This is the file DESIGN reads FIRST. The primary driver was
> corrected from "restyle without Rust + boundary confidence" to **"expose a first-class JSON
> API (read + write, machine-token auth) so external agents/integrations can drive Foundry
> programmatically."** See Decision D7 (driver correction) and D8 (oversize + recommended
> split). Remaining assumptions are flagged inline.

## Feature Summary

Expose a **first-class JSON API** (read AND write, authenticated by a machine token) so
external agents and integrations can drive Foundry programmatically, AND build a real
**htmx + Alpine.js** web tier — with the API tier and the web tier as **PEER consumers of one
presentation-neutral core**, all as a **code (module) separation that still ships as ONE binary**
(no network hop, no second service). The JSON API is the headline outcome; the web-tier
templating is the secondary peer track. Target end state:

```
foundry (one binary)
├── foundry-api    PRIMARY: first-class JSON API (read+write) + machine-token auth; emits JSON
│     calls ↓ in-process                                              (peer consumer of core)
├── foundry-core / foundry-store / foundry-auth / foundry-realtime   (existing, made neutral)
│     calls ↑ in-process                                              (peer consumer of core)
└── foundry-web    SECONDARY: template engine + static asset pipeline; renders HTML
```

Feature type: **cross-cutting** (a programmatic JSON API + a backend architecture refactor +
user-facing UI surfaces).

## Phase 1 — Discovery & Job Grounding

### Driver correction RESOLVES the dominant strawman risk

The 2026-05-30 user correction (D7) confirmed the PRIMARY driver: the first-class JSON API. This
**resolves** the previously-dominant process risk ("jobs/personas are Luna-derived, not
validated"). The primary job (jtbd-web-4) is now USER-CONFIRMED. The remaining jobs are
re-ranked secondary and still benefit from later validation, but the feature is no longer at
risk of solving the wrong problem.

### Missing DIVERGE (RISK, downgraded)

There is still **no `docs/feature/web-tier-extraction/diverge/` directory** (no validated
`recommendation.md`/`job-analysis.md`). With the primary driver now user-confirmed, the residual
risk is lower:

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| Primary driver wrong | RESOLVED | — | User confirmed jtbd-web-4 (first-class JSON API) on 2026-05-30 (D7). |
| Secondary jobs/personas (jtbd-web-1/2/3) are Luna-derived, not DIVERGE-validated | Medium | Low | Grounded in backend-mvp JTBD + code-surface reading; they are the *secondary* track now, so drift has bounded impact. Confirm before DESIGN of Feature B. |

### What was grounded by reading the actual code (not assumed)

- Foundry is **already** an htmx app, but every HTML surface is emitted as inline
  `format!` string literals interleaved with auth + store logic inside handlers
  (`issues.rs`, `comments.rs`, `signin.rs`, `projects.rs`, `events.rs`).
- A single handler (e.g. `issues::submit_create`) does FOUR things: session extraction,
  `is_team_member` authz, `find_*`/`insert_*_with_outbox` store calls, and `format!`
  HTML rendering + htmx detection. This is the mixed surface to be separated.
- `crates/foundry-app/templates/` and `crates/foundry-app/static/` **exist but are EMPTY**:
  no template engine, no vendored htmx/Alpine, no CSS.
- It is a **single axum binary** (`foundry-app`). There is **no JSON API tier today** —
  the JSON tier is net-new architecture, carved out of the mixed handlers.
- CSRF uses a **double-submit cookie** plus an **`HX-CSRF` / `hx-csrf` header** read
  client-side (cookie is non-HttpOnly by design); `/bootstrap` is CSRF-exempt.
- Sessions are **Postgres-backed via tower-sessions** (no Redis).
- The codebase mixes **both `hx-` and `data-hx-` prefixes** (e.g. `hx-swap-oob` and
  `data-hx-fragment` in the same file) — relevant to a future htmx 2 normalization.
- Existing acceptance scenarios assert on **rendered HTML substrings** (e.g. the literal
  "Backlog", the issue key, `.attachments-empty`). The extraction must keep these green.

### Happy path / emotional arc / shared artifacts / error paths

Covered in `journey.md` for the three key surfaces (board, issue-detail+comments, sign-in).
Gate: happy path, emotional arc, shared artifacts, and error/recovery paths all mapped.

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate) — RE-RUN post-correction

### Scope Assessment: OVERSIZED — split recommended (was PASS under the read-only-API strawman)

Promoting the JSON API to a first-class read+write surface plus a net-new machine-token auth
surface changes the verdict. Re-run:

| Signal | Threshold | This feature now | Trips? |
|--------|-----------|------------------|--------|
| User-story count | >10 | 8 | Borderline-No |
| Bounded contexts / modules touched | >3 | 2 net-new (`foundry-api`, `foundry-web`) over existing core/store | No |
| Walking-skeleton integration points | >5 | 3 for the WS (api ↔ core seam, JSON serialization, acceptance harness) | No |
| Estimated effort | >2 weeks | ~16-22 days | **YES** |
| Independent shippable user outcomes | "multiple that could ship separately" | **2** — (JSON read+write API + machine-token auth) vs (web templating + asset build-out + htmx2) | **YES** |
| New auth surface | qualitative | machine-token auth is net-new security, separable from templating | **YES** |

**Three signals trip → propose a split** (per Core Principle 8). See Decision D8.

The two outcomes are genuinely independent: "Programmatic Foundry" (the JSON API + machine
tokens) delivers the primary job with NO web-tier templating, and "Foundry looks like a product"
(the htmx template build-out) delivers the secondary jobs with NO API work. They share only the
presentation-neutral core seam, which the API track proves first and the web track reuses.

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **D7** | **PRIMARY DRIVER CORRECTED (2026-05-30, user-confirmed): the headline outcome is a first-class JSON API — reads AND writes (create/update issues, comments, projects, state) authenticated by a MACHINE TOKEN — so external agents/integrations can drive Foundry programmatically.** The htmx web tier and the JSON API tier are PEER consumers of one presentation-neutral core; the API is NOT a read-only afterthought to prove a boundary. | The original strawman wrongly assumed the driver was "restyle without Rust" (jtbd-web-1) + "boundary confidence" (jtbd-web-2). The user corrected this. Consequence: jtbd-web-4 is now primary; slice order flips to lead with the JSON API; machine-token auth becomes a new (additive) auth surface in scope; JSON writes are in scope. |
| **D8** | **OVERSIZE: split recommended** into **Feature A — "Programmatic Foundry"** (US-W05a/b/c + US-W06: JSON read+write API + machine-token auth + peer-consumer core separation + boundary guard) and **Feature B — "Foundry looks like a product"** (US-W01..W04: htmx web-tier template/asset build-out + later htmx2). Everything stays in THIS discuss set; the split is a flagged recommendation for the user to ratify before DESIGN. No separate feature directories created yet. | Three oversize signals trip post-correction (two independent shippable outcomes, a net-new auth surface, ~16-22 days). The two outcomes ship independently and share only the core seam. Per Core Principle 8, propose the split rather than carry a 2-outcome feature. Delivery sequence: Feature A first (proves the neutral core), then Feature B reuses it. |
| **D9** | **SPLIT RATIFIED (2026-05-31, user-confirmed).** The D8 split is accepted. **DESIGN is scoped to Feature A — "Programmatic Foundry" ONLY**: stories US-W05a, US-W05b, US-W05c, US-W06 (JSON read+write API + machine-token auth + web/api/core separation + CI-enforced boundary guard). Feature B (US-W01..W04, htmx web-tier build-out) is DEFERRED to its own feature, to be started later via `/nw:new` and to reuse the presentation-neutral core seam that Feature A proves first. The `web-tier-extraction` workspace now effectively carries Feature A; Feature B will get its own feature directory when initiated. | User ratified the recommended split and chose "design Feature A now." Scoping DESIGN to A keeps the design surface focused on the confirmed primary driver (the JSON API) and the new machine-token auth surface, and avoids designing the web-tier UI track before its requirements are revisited (jtbd-web-1/2/3 remain Luna-derived, not DIVERGE-validated). |
| D1 | Separation is a **module/crate boundary inside one binary**, not a service split | Preserves the brand promise: one binary, one Postgres, `docker compose up`. In-process calls = no network hop, no latency tax on the Linear-feel budget. The JSON API tier is a PEER module, not a second process. |
| D2 (REVISED) | **Slice 1 = read the board's issues as JSON** via `foundry-api` over a presentation-neutral core seam (the SAME core call the web board will later use), existing acceptance scenarios stay green. Slice 2 = machine-token auth + JSON writes + boundary guard. The board-into-`foundry-web` template work moves to Slice 3. | Prove the now-PRIMARY job (first-class API) on the smallest real surface first. Riskiest assumption first: *can core feed a non-HTML consumer through the same path the UI uses?* — i.e. is core genuinely presentation-neutral. (Was: "Slice 1 = extract the board into foundry-web"; reordered by D7.) |
| D3 | **htmx 2 migration is DEFERRED to DESIGN** | Per the brief and the backend-mvp out-of-scope note. DISCUSS only records that the `hx-`/`data-hx-` prefix split and version choice are downstream design decisions. No version is committed here. |
| D4 | **No new runtime services, no Node build step** in scope | The asset pipeline vendors htmx/Alpine/CSS into `static/` shipped in the image; rendering is server-side. Honors "no new infra". (Whether a build-time Node/esbuild step is acceptable is an open question for DESIGN — see open questions.) |
| D5 (REVISED) | **BROWSER auth model UNCHANGED; machine-token auth ADDED as an additive surface** | The browser path (`HX-CSRF` header + double-submit cookie + Postgres sessions + argon2id + brute-force delay) is an invariant (NFRs). The new machine-token credential (US-W05b) is additive — it does not alter the browser flow, and API writes reuse the SAME authorization core enforces for the UI. The token MECHANISM is DESIGN; the requirement + security constraints are captured as NFR-WEB-API-SEC-01..03. |
| D6 | Output uses the **LEGACY per-feature layout** (separate files under `discuss/`), NOT the SSOT/feature-delta model | Decided with the user; `docs/product/` does not exist and we are intentionally not migrating. Mirrors `foundry-backend-mvp/discuss/`. |

## Requirements Summary

- 8 user stories, 1 explicitly `@infrastructure` (US-W06, boundary guard, folded into Slice 2).
- **PRIMARY track — first-class JSON API (jtbd-web-4):**
  - US-W05a — read the board's issues as JSON over a presentation-neutral core seam (Slice 1).
  - US-W05b — machine-token authentication for programmatic clients (Slice 2).
  - US-W05c — create/update issues and comments via JSON, rule-parity with the UI (Slice 2).
- **SECONDARY track — htmx web-tier templating (jtbd-web-1/3):**
  - US-W01+US-W02 — board → `foundry-web` template + vendored assets (Slice 3).
  - US-W03 — issue detail + comment thread → templates (Slice 4).
  - US-W04 — sign-in + forgot-password → templates (Slice 5).
- NFRs: NEW NFR-WEB-API (versioned contract, content negotiation, machine-token security, write
  rule-parity) + boundary honesty + core-presentation-neutral + in-process latency (no hop) +
  accessibility/keyboard preserved + browser CSRF/sessions unchanged + acceptance scenarios stay
  green + no new runtime services.
- Note: US-W05 (the old single read-only JSON story) is SUPERSEDED by US-W05a/b/c.

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no new runtime services. The JSON API tier is a PEER
  module in the one binary, NOT a second process.
- Web and API are PEER consumers of one presentation-neutral core: no JSON handler renders HTML;
  no web handler renders JSON; no tier touches the DB directly; core knows nothing about format.
- The JSON API is first-class: reads AND writes. API writes enforce the SAME authz/validation/
  sanitization and travel the SAME outbox as the equivalent UI action (no privileged back door).
- Machine-token auth is additive: it does not alter the existing browser session/CSRF model.
- The existing acceptance suite (`foundry-acceptance`) must stay green throughout — including the
  browser session/CSRF scenarios as the machine-token path is added.
- Solution-neutral: this wave does NOT pick the template engine, the htmx version, the JSON
  serialization/route/negotiation/versioning mechanism, or the machine-token mechanism (issuance/
  format/storage/rotation/revocation/scoping). Those belong to DESIGN.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| Core is secretly HTML-shaped, so the JSON API cannot be a true peer (kills the primary premise) | Medium | High | Slice 1 (US-W05a) is exactly this test: prove the board's issues flow as JSON through the same core call the UI uses BEFORE building auth/writes. Riskiest assumption first. |
| Machine-token auth is a NEW security surface; getting it wrong is a credential-leak risk | Medium | High | DESIGN must treat it with password-model rigor; security constraints fixed as NFR-WEB-API-SEC-01..03 (revocable, scope-bounded, secret-handled, additive). DESIGN open question flagged. |
| API writes diverge from UI rules (different authz/validation/sanitization) — API becomes a back door | Medium | High | NFR-WEB-API-CON-02 mandates rule-parity via the SAME core write+outbox path; paired API-vs-UI acceptance scenarios enforce it. |
| Template engine adds render latency above the ≤200ms budget (NFR-WEB-PERF-01) | Medium | High | Benchmark the render path on Slice 3's board before extracting other web surfaces; reject engines that miss the budget. |
| Acceptance scenarios assert on HTML substrings; templating changes whitespace/markup | High | Medium | The web slices keep the asserted substrings ("Backlog", issue key, `data-*` markers) byte-stable; treat them as a render contract. |
| Boundary erodes over time (api emits HTML, or a web handler reaches the pool) | Medium | Medium | US-W06 ships a structural guard (crate-graph / lint), folded into Slice 2 — enforced once the write surface exists, not just documented. |
| htmx 2 normalization (hx-/data-hx- split) leaks into DISCUSS scope | Low | Low | Explicitly deferred to DESIGN (D3, out-of-scope.md). |
| Secondary jobs/personas lack DIVERGE validation | Medium | Low | Primary driver now user-confirmed (D7); secondary track grounded in backend-mvp + code reading; confirm before DESIGN of Feature B. |
| Oversize: feature carries two independent outcomes | High | Medium | Split recommended (D8): Feature A (API) + Feature B (web). User ratifies before DESIGN. |
