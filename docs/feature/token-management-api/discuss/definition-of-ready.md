# Definition of Ready — Token-Management API

DoR is a hard gate: every story passes all 9 items before DESIGN. The one item that is NOT yet
satisfied is **Dependencies (Item 8)**: the Q-AUTHZ crux must be ratified by the user before DESIGN
begins. All other items pass with evidence below.

## Per-story DoR validation

### US-TMA00 (infrastructure-only — route group + authz-gate seam)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear, domain language | PASS | "foundry-api has issue/comment routes but no token routes; no place applies the ratified authz decision." |
| 2. User/persona specific | PASS | The platform; enables Sven/Dana's automation. `@infrastructure` with rationale (no user decision alone). |
| 3. 3+ domain examples real data | PASS | route reachable / no bearer / single authz home. |
| 4. UAT 3-7 scenarios | PASS | 3 scenarios. |
| 5. AC from UAT | PASS | 4 AC derived. |
| 6. Right-sized | PASS | scaffold-only, ~0.5-1 day, folded into Slice 1, never standalone. |
| 7. Technical notes | PASS | extend `routes<S>()`, reuse `MachinePrincipal`/`status_for`, dispatch via `Services`. |
| 8. Dependencies tracked | PASS | depends on the ratified Q-AUTHZ model (where the authz gate lives). |
| 9. Outcome KPIs | PASS (N/A) | infrastructure; enables KPI 1. |

### US-TMA01 (GET list — walking skeleton)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear | PASS | "audit pipeline can only see tokens via HTML scrape or DB; `list_tokens` shipped but no JSON surface." |
| 2. Persona specific | PASS | Dana's audit pipeline (security automation); + integrator pipeline. |
| 3. 3+ examples real data | PASS | two tokens (`ci-issue-filer`, `slack-relay`, priya@acme.dev) / empty `[]` / non-admin 403. |
| 4. UAT 3-7 | PASS | 4 scenarios. |
| 5. AC from UAT | PASS | 5 AC. |
| 6. Right-sized | PASS | ~1-1.5 days; calls SHIPPED `list_tokens`; 4 scenarios. |
| 7. Technical notes | PASS | `list_tokens` via `Services`; mirrors `TokenView` (no value field). |
| 8. Dependencies | PASS | US-TMA00; ratified Q-AUTHZ. |
| 9. Outcome KPIs | PASS | KPI 1 (Who/Does what/By how much/Measured/Baseline). |

### US-TMA02 (revoke)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear | PASS | "rotation/incident needs programmatic kill; `revoke_token` shipped, no JSON route." |
| 2. Persona specific | PASS | automation agent (rotation job / incident runbook). |
| 3. 3+ examples | PASS | leaked CI token / idempotent re-revoke / cross-workspace probe. |
| 4. UAT 3-7 | PASS | 4 scenarios. |
| 5. AC from UAT | PASS | 4 AC. |
| 6. Right-sized | PASS | ~1-1.5 days; calls SHIPPED `revoke_token` + SHIPPED denylist. |
| 7. Technical notes | PASS | `revoke_token`; verb = Q-REVOKE-VERB. |
| 8. Dependencies | PASS | US-TMA00/01; ratified Q-AUTHZ + Q-REVOKE-VERB. |
| 9. Outcome KPIs | PASS | KPI 2. |

### US-TMA03 (revoke-self / rotation)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear | PASS | "hands-free rotation needs the retire step programmatic; the old credential must revoke itself." |
| 2. Persona specific | PASS | automation agent (scheduled rotation job). |
| 3. 3+ examples | PASS | clean rotation / revoke-self-then-reuse / re-run idempotent. |
| 4. UAT 3-7 | PASS | 3 scenarios. |
| 5. AC from UAT | PASS | 3 AC. |
| 6. Right-sized | PASS | ~0.5-1 day; revoke-self is a subset of `revoke_token`. |
| 7. Technical notes | PASS | in-flight request succeeds; denylist bites next call. |
| 8. Dependencies | PASS | US-TMA02. |
| 9. Outcome KPIs | PASS | KPI 3. |

### US-TMA04 (stable contract)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear | PASS | "integrator must branch reliably; ensure new routes inherit the shipped envelope." |
| 2. Persona specific | PASS | integrator (Sven Aarø). |
| 3. 3+ examples | PASS | branchable codes / read-after-write / no prose-only error. |
| 4. UAT 3-7 | PASS | 3 scenarios. |
| 5. AC from UAT | PASS | 3 AC. |
| 6. Right-sized | PASS | ~0.5-1 day; reuses `status_for` unchanged. |
| 7. Technical notes | PASS | reuse `status_for`, `ErrorBody`/`ErrorDetail`. |
| 8. Dependencies | PASS | US-TMA01/02. |
| 9. Outcome KPIs | PASS | KPI 4. |

### US-TMA05 (refusal boundary + rate guardrail)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem clear | PASS | "a programmatic surface invites probing; make non-enumerability + escalation-bounding + abuse-throttling explicit + tested." |
| 2. Persona specific | PASS | security automation (Dana) + the evil caller (Malicious Mike). |
| 3. 3+ examples | PASS | cross-workspace probe / non-management probe / no mint surface to escalate. |
| 4. UAT 3-7 | PASS | 5 scenarios. |
| 5. AC from UAT | PASS | 5 AC. |
| 6. Right-sized | PASS | ~1-1.5 days; reuses non-enumerable refusals + identical 401; the rate guardrail mechanism is DESIGN. |
| 7. Technical notes | PASS | reuse `revoke_token` + `token_auth`; no-mint route is the v1 Q-AUTHZ expression; rate mechanism = DESIGN. |
| 8. Dependencies | PASS | US-TMA01/02; ratified Q-AUTHZ + Q-RATE-LIMIT default. |
| 9. Outcome KPIs | PASS | KPI 5 (guardrail). |

## Feature-level DoR

| Item | Status | Evidence/Issue |
|------|--------|----------------|
| Jobs grounded (job_id on every story) | PASS | every story has a `job_id`; `jobs.yaml` `story_job_map` is the canonical mirror; US-TMA00 is `infrastructure-only` with rationale. |
| Elevator Pitch on every non-`@infrastructure` story | PASS | US-TMA01..05 each have Before/After/Decision with a real `/api/v1` entry point + concrete JSON output. |
| Walking skeleton identified | PASS | US-TMA00 + US-TMA01 (GET list, authorized-vs-refused) — safest-authz-first. |
| Slices by outcome | PASS | 3 slices: inventory / revoke+rotate / trust-the-contract. |
| NFRs (security-heavy) | PASS | `nfrs.md` SEC-01..08 (SEC-08 = the escalation model), REL/PERF/CON/DATA + invariants. |
| Out of scope explicit | PASS | `out-of-scope.md` (mint deferred, no OAuth, no API platform, no key rotation). |
| Outcome KPIs | PASS | `outcome-kpis.md` 5 KPIs + guardrails + DEVOPS handoff. |

## DoR Status: BLOCKED pending one ratification

- **9-item per-story checklist: PASS for all 6 stories** (evidence above).
- **Single blocker (Item 8 / feature-level):** the **Q-AUTHZ crux** (authz/escalation model) must be
  **ratified by the user** before DESIGN. The stories + slices are written to the recommended option
  (c — bearer LIST+REVOKE, MINT human-only). If the user picks (a) or (c)+(b), a MINT slice
  (US-TMA06) is added with the corresponding capability gate + guardrails, and US-TMA05's no-mint-route
  AC is replaced with the capability + anti-self-replication ACs.
- Secondary, non-blocking confirmations (have safe defaults): Q-REVOKE-VERB, Q-REVOKE-SELF (default
  YES), Q-RATE-LIMIT (default guardrail+cap), Q-LIST-SHAPE (default mirror `TokenView`).

Once Q-AUTHZ is ratified, this feature is READY for DESIGN.
