# Web-Tier Extraction — Definition of Ready

> DRIVER-CORRECTED (2026-05-30). Re-validated against the 9 DoR items after the JSON-API-first
> correction. Adds DoR rows for the new primary-track stories US-W05a/b/c; US-W05 (old read-only)
> is superseded. The previous CONDITIONAL gate (unconfirmed strawman driver) is now RESOLVED —
> the user confirmed the primary driver (jtbd-web-4). One NEW conditional is surfaced: the
> machine-token auth MECHANISM is unspecified — that is an acceptable DESIGN input, not a DoR
> blocker. Validation covers the 9 DoR items + the Elevator-Pitch invariant + the `job_id`
> JTBD-traceability hard gate.

Items checked (per the LeanUX skill):

1. Problem statement clear, in domain language.
2. User/persona with specific characteristics.
3. ≥3 domain examples with real data.
4. UAT scenarios in Given/When/Then (3-7 per story).
5. AC derived from UAT.
6. Right-sized (S/M/L, 1-3 days, 3-7 scenarios).
7. Technical notes capture constraints/dependencies.
8. Dependencies resolved or tracked.
9. Outcome KPIs defined with measurable targets.

Plus: every non-`@infrastructure` story carries an **Elevator Pitch** (Before/After/Decision)
AND a `job_id` referencing an entry in `jobs.yaml`; `@infrastructure` stories use
`job_id: infrastructure-only` with an `infrastructure_rationale`.

---

## Per-Story DoR Validation

### US-W05a: Read the board's issues as JSON over a presentation-neutral core seam (PRIMARY, Slice 1)

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | No JSON API today; programmatic reads require scraping HTML + forging CSRF. |
| 2. Persona | PASS | Devansh (integrator/operator) + a maintainer validating core neutrality. |
| 3. Examples (≥3) | PASS | Board issues as JSON; empty project → `[]`; api must not emit HTML. |
| 4. UAT (3-7) | PASS | 4 scenarios. |
| 5. AC from UAT | PASS | 5 ACs trace to scenarios. |
| 6. Right-sized | PASS | M, 2-3 days, 4 scenarios. |
| 7. Tech notes | PASS | Read-path only; route/negotiation/serde = DESIGN; walking-skeleton proof of neutral core. |
| 8. Dependencies | PASS | None (entry slice). |
| 9. Outcome KPI | PASS | ≥1 read endpoint returns valid JSON, 0 HTML bytes; 100% data via same core call as UI. |
| Elevator Pitch | PASS | Before/After (`curl … Accept: application/json` → JSON array) / Decision present. |
| job_id | PASS | jtbd-web-4 (in jobs.yaml). |
| **Verdict** | **READY** | |

### US-W05b: Authenticate programmatic clients with a machine token (PRIMARY, Slice 2)

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Only credential today is a human browser session + CSRF; unusable for unattended clients/agents. |
| 2. Persona | PASS | Devansh (automation author), an agent builder, a workspace admin (issuer/revoker). |
| 3. Examples | PASS | Token-authenticated read; revoked token refused; out-of-scope token forbidden. |
| 4. UAT | PASS | 5 scenarios. |
| 5. AC | PASS | 5 ACs from scenarios. |
| 6. Right-sized | PASS | M, 3 days, 5 scenarios. |
| 7. Tech notes | PASS | REQUIREMENTS ONLY; mechanism (format/storage/rotation/scope/issuance) = DESIGN; NEW auth surface flagged. |
| 8. Dependencies | PASS | US-W05a (tracked); Slice 2. |
| 9. KPI | PASS | 100% of API endpoints reachable with a token alone; revoked tokens refused next use. |
| Elevator Pitch | PASS | Before/After (`Authorization: Bearer …` machine principal) / Decision present. |
| job_id | PASS | jtbd-web-4. |
| **Verdict** | **READY** (with DESIGN-input conditional: token mechanism unspecified — acceptable) | |

### US-W05c: Create and update issues and comments through the JSON API (PRIMARY, Slice 2)

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Reads+auth make Foundry observable, not drivable; writes live only behind htmx browser forms. |
| 2. Persona | PASS | Agent builder, Devansh (scripting), a maintainer ensuring rule-parity. |
| 3. Examples | PASS | Create issue via JSON; update state + add comment; invalid write rejected like the UI. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 6 ACs from scenarios. |
| 6. Right-sized | PASS | M-L, 3-4 days, 6 scenarios — upper bound noted; split issues/comments writes if it grows. |
| 7. Tech notes | PASS | JSON shapes/status/idempotency = DESIGN; rule-parity (NFR-WEB-API-CON-02) is load-bearing; reuses outbox. |
| 8. Dependencies | PASS | US-W05a + US-W05b (tracked); Slice 2. |
| 9. KPI | PASS | ≥4 write ops succeed via JSON with 100% rule-parity to UI; 0 HTML bytes in write responses. |
| Elevator Pitch | PASS | Before/After (`POST … -d '{"title":…}'` → created issue JSON) / Decision present. |
| job_id | PASS | jtbd-web-4. |
| **Verdict** | **READY** | |

### US-W05: (SUPERSEDED — see US-W05a/b/c)

The original read-only US-W05 is replaced by US-W05a (read), US-W05b (machine-token auth), and
US-W05c (writes). No standalone DoR row; its content is absorbed and elevated above.

### US-W01: Extract the issue board into a web tier over a core seam

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Mixed handler does session+authz+store+`format!` HTML; board is highest-traffic surface. |
| 2. Persona | PASS | Jamal (Rust contributor) + Mei (member), specific contexts. |
| 3. Examples (≥3) | PASS | Board renders via web tier; empty board; forbidden DB reach. |
| 4. UAT (3-7) | PASS | 5 scenarios. |
| 5. AC from UAT | PASS | 6 ACs trace to scenarios. |
| 6. Right-sized | PASS | M, 3 days, 5 scenarios. |
| 7. Tech notes | PASS | Solution-neutral on engine; reuse existing store calls; render contract noted. |
| 8. Dependencies | PASS | None (entry; paired with US-W02). |
| 9. Outcome KPI | PASS | 100% of board-visual changes touch 0 store files / 0 sqlx sites (PR diff). |
| Elevator Pitch | PASS | Before/After (real URL → styled board) / Decision present. |
| job_id | PASS | jtbd-web-2 (in jobs.yaml). |
| **Verdict** | **READY** | |

### US-W02: Render the board from vendored assets

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | `static/`+`templates/` empty; unstyled board reads as prototype, breaks trust. |
| 2. Persona | PASS | Mei/team first impression; Devansh screenshots it. |
| 3. Examples | PASS | Air-gapped styled board; keyboard-only user; broken asset path. |
| 4. UAT | PASS | 4 scenarios. |
| 5. AC | PASS | 5 ACs from scenarios. |
| 6. Right-sized | PASS | M, 2-3 days, 4 scenarios. |
| 7. Tech notes | PASS | No CDN / no runtime service constraint; htmx version not chosen (deferred). |
| 8. Dependencies | PASS | US-W01 (tracked); ships in Slice 1. |
| 9. KPI | PASS | 0 external CDN requests on board; styled-board check green on no-egress host. |
| Elevator Pitch | PASS | Before/After (browser → styled, vendored) / Decision present. |
| job_id | PASS | jtbd-web-3. |
| **Verdict** | **READY** | |

### US-W03: Move issue detail + comment thread to templates

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | ≥3 `format!` comment-render sites; OOB card omits buttons → live≠reloaded divergence. |
| 2. Persona | PASS | Jamal restyling; Mei posting/editing (no behavior change). |
| 3. Examples | PASS | Live card matches reloaded; edit + "(edited)"; non-author 403 + deleted 410. |
| 4. UAT | PASS | 6 scenarios. |
| 5. AC | PASS | 6 ACs from scenarios. |
| 6. Right-sized | PASS | M, 3 days, 6 scenarios. |
| 7. Tech notes | PASS | OOB-divergence resolved via shared partial; sanitization stays in core (NFR). |
| 8. Dependencies | PASS | US-W01 (tracked); Slice 2. |
| 9. KPI | PASS | comment-render sites ≥3 → 1 partial; 0 authz logic in web tier. |
| Elevator Pitch | PASS | Before/After (real issue URL → one partial across paths) / Decision present. |
| job_id | PASS | jtbd-web-1. |
| **Verdict** | **READY** | |

### US-W04: Move sign-in + forgot-password to templates

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Auth screens are standalone `format!` HTML, no shared layout; first-land surface. |
| 2. Persona | PASS | Mei returning; first-time evaluator landing on /sign-in. |
| 3. Examples | PASS | Styled sign-in + same cookie; wrong password non-enumerable; CSRF cookie absent on GET. |
| 4. UAT | PASS | 4 scenarios. |
| 5. AC | PASS | 5 ACs from scenarios. |
| 6. Right-sized | PASS | S-M, 2 days, 4 scenarios. |
| 7. Tech notes | PASS | Full-page = lowest fragment risk; brute-force delay untouched. |
| 8. Dependencies | PASS | US-W01 (tracked); Slice 3. |
| 9. KPI | PASS | 100% auth screens extend one base layout; 0 duplicated head/asset boilerplate. |
| Elevator Pitch | PASS | Before/After (/sign-in → styled, same cookie) / Decision present. |
| job_id | PASS | jtbd-web-3. |
| **Verdict** | **READY** | |

### US-W06: Lock the web/api boundary with a structural guard `@infrastructure`

| Item | Status | Evidence |
|------|--------|----------|
| 1. Problem | PASS | Review-only boundary erodes; re-creates the mixed-handler problem. |
| 2. Persona | PASS | Maintainers/contributors; CI context. |
| 3. Examples | PASS | Clean PR passes; web pool dep fails; api HTML fails. |
| 4. UAT | PASS | 3 scenarios. |
| 5. AC | PASS | 3 ACs from scenarios. |
| 6. Right-sized | PASS | S, 1 day, 3 scenarios. |
| 7. Tech notes | PASS | Mechanism = DESIGN; this is what makes jtbd-web-2 durable. |
| 8. Dependencies | PASS | US-W05a/c (JSON read+write to guard) + US-W01 (web tier to guard); folded into Slice 2, NOT standalone. |
| 9. KPI | PASS | 100% of boundary-violating PRs fail CI; injected-violation test proves it bites. |
| Elevator Pitch | N/A | `@infrastructure` story; carries `infrastructure_rationale` instead (per Dimension 0). |
| job_id | PASS | infrastructure-only + infrastructure_rationale present in stories.md. |
| **Verdict** | **READY** | |

---

## Slice-Level Elevator-Pitch Check (Dimension 0 §5)

No *released* slice is entirely `@infrastructure`:

| Released slice | Has ≥1 user-visible story? |
|----------------|-----------------------------|
| 1 (US-W05a) | YES (JSON read door for machine clients) |
| 2 (US-W05b, US-W05c + US-W06 folded) | YES (machine-token auth + JSON writes; US-W06 rides along) |
| 3 (US-W01, US-W02) | YES (styled board) |
| 4 (US-W03) | YES (issue/comments) |
| 5 (US-W04) | YES (sign-in) |

US-W06 never ships as a standalone slice. PASS.

---

## Aggregate DoR Status

| Story | Slice | Verdict | job_id |
|-------|-------|---------|--------|
| US-W05a | 1 | READY | jtbd-web-4 |
| US-W05b | 2 | READY (DESIGN-input conditional: token mechanism) | jtbd-web-4 |
| US-W05c | 2 | READY | jtbd-web-4 |
| US-W06 | 2 (folded) | READY | infrastructure-only (+rationale) |
| US-W01 | 3 | READY | jtbd-web-2 |
| US-W02 | 3 | READY | jtbd-web-3 |
| US-W03 | 4 | READY | jtbd-web-1 |
| US-W04 | 5 | READY | jtbd-web-3 |

**8 of 8 stories pass all 9 DoR items, plus the Elevator-Pitch invariant (or
infrastructure_rationale), plus the `job_id` JTBD-traceability check.**

**Aggregate verdict: READY for DESIGN handoff.** The previous CONDITIONAL gate (unconfirmed
strawman driver) is **RESOLVED** — the user confirmed the primary driver (jtbd-web-4) on
2026-05-30. One residual DESIGN-input conditional stands (machine-token auth mechanism
unspecified — acceptable, it is precisely a DESIGN decision, not a DoR blocker). The oversize
split (D8) is a recommendation for the user to ratify before/at DESIGN; it does not block DoR.

---

## Open Questions DESIGN Must Resolve (none are DoR blockers; all are DESIGN inputs)

1. **Machine-token mechanism (NEW, highest priority).** US-W05b fixes the REQUIREMENT (a
   first-class, additive, admin-issued, revocable, scope-bounded machine credential) and its
   security constraints (NFR-WEB-API-SEC-01..03), but NOT the mechanism. DESIGN must decide:
   token format/prefix; storage + hashing (never plaintext); issuance UX (CLI? admin screen?
   bootstrap-time?); rotation/expiry; scoping granularity (workspace? team? per-resource?).
   This is a NEW security surface — treat it with the same rigor as the password/session model.

2. **JSON contract surface: negotiation, versioning, serialization, write semantics.** DESIGN
   picks `/api` path prefix vs `Accept`-header negotiation; the versioning mechanism (URL vs
   header vs media type) satisfying NFR-WEB-API-CON-01; the serde request/response shapes;
   status-code conventions; PATCH-vs-PUT and idempotency for writes (US-W05c).

3. **Split ratification (D8).** Confirm whether to formally split into **Feature A —
   "Programmatic Foundry"** (US-W05a/b/c + US-W06) and **Feature B — "Foundry looks like a
   product"** (US-W01..W04), creating separate feature directories, or keep one feature delivered
   in the API-first slice order. Recommendation: split, sequence A before B.

4. **Build-time asset step: allowed or not? (web track only)** US-W02 forbids a new *runtime*
   service and a CDN, but is silent on whether a *build-time* Node/esbuild/minify step is
   acceptable (vendoring pre-built htmx/Alpine avoids it entirely). If "no Node anywhere, even at
   build time" is firm, DESIGN should pick a pure-Rust/vendored-blob asset path. **Confirm the
   build-time tolerance.** (Relevant to Feature B / Slices 3-5 only.)

5. **Secondary-job validation (lower priority).** jtbd-web-1/2/3 are Luna-derived (no DIVERGE
   dir). The primary driver is confirmed; confirm the secondary web-track jobs before DESIGN of
   Feature B.

---

## Risks Surfaced (for DESIGN's risk register)

See `wave-decisions.md` "Risks Surfaced" — chiefly: core not being presentation-neutral (tested
first by Slice 1), the new machine-token security surface, API-vs-UI write rule-parity, template
render latency vs the ≤200 ms budget, substring-asserting acceptance tests vs templating, and
boundary erosion (mitigated by US-W06). The dominant *scope* risk is oversize → the recommended
split (open question 3).
