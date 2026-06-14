# Definition of Ready — invite-accept-flow

9-item hard gate. Each item must PASS with evidence before DESIGN handoff.

## US-01 — Accept a valid invite and get signed in

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Priya Nair, first-admin of Northwind, has only a dead invite link and cannot sign in." |
| 2 | User/persona with specific characteristics | PASS | First-admin, never signed in, received link out-of-band, wants to start now. |
| 3 | 3+ domain examples with real data | PASS | Priya Nair (river-stone-lantern-92); Marcus Liu/Westgate near-fresh; 6d23h boundary. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (form renders, valid accept, near-fresh, near-expiry). |
| 5 | AC derived from UAT | PASS | AC-01.1..01.6 in acceptance-criteria.md, each traced to FR. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Thin adapter over shipped seams; 4 scenarios; ~1.5 days. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses InviteToken/hash_password/session/resolve_active_workspace; new GET/POST + consume TX; OD-1. |
| 8 | Dependencies resolved or tracked | PASS | Shipped seams available; OD-1 (used_at column) tracked as open decision for DESIGN. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (90% activation within 7d), baseline 0%, measured by consumed-with-session/issued. |

**US-01 DoR: PASSED**

## US-02 — Refuse invalid links safely

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Forwarded/re-clicked/probed links must be honest for Priya yet non-enumerable for attackers; never twice-consumable. |
| 2 | User/persona with specific characteristics | PASS | Legit re-clicker (Priya) AND Malicious Mike probing URLs. |
| 3 | 3+ domain examples with real data | PASS | Mike's all-zero id; Priya re-click after success; double-click concurrency. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 5 scenarios (expired, tampered, unknown, re-use, concurrency). |
| 5 | AC derived from UAT | PASS | AC-02.1..02.9, each traced to NFR-1/2/3/5/6. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 5 scenarios; uniform refusal + atomic consume; ~1.5 days. |
| 7 | Technical notes | PASS | Atomic guarded consume; uniform refusal page; CSRF; no-secret logging; diverges from bootstrap claim messages. |
| 8 | Dependencies tracked | PASS | Depends on US-01 + OD-1; CSRF/session shipped. |
| 9 | Outcome KPIs | PASS | KPI-2 (100% byte-identical refusals, guardrail) + KPI-3 (0 double-consumes, guardrail). |

**US-02 DoR: PASSED**

## US-03 — Correct password mistakes without losing the invite

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | A rejected password must not strand the admin with a dead link. |
| 2 | User/persona with specific characteristics | PASS | First-time password-setter; Careless Cathy who skips the requirement. |
| 3 | 3+ domain examples with real data | PASS | Priya "pizza"->river-stone-lantern-92; Marcus mismatch; 12-char boundary. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (weak, mismatch, threshold). |
| 5 | AC derived from UAT | PASS | AC-03.1..03.4, traced to FR-5/NFR-4. |
| 6 | Right-sized | PASS | 3 scenarios; inline validation only; <1 day. |
| 7 | Technical notes | PASS | NFR-4 min-12 (OD-2 ratification); hash only after validation. |
| 8 | Dependencies tracked | PASS | Depends on US-01; OD-2 (threshold) tracked. |
| 9 | Outcome KPIs | PASS | KPI-4 (80% recovery-completion), baseline 0%. |

**US-03 DoR: PASSED**

---

## Overall DoR: PASSED (pending peer-review gate)

All three stories pass all 9 items. Two open decisions (OD-1 single-use column, OD-2 password
threshold) are **DESIGN-wave inputs**, not DoR blockers: requirements are written solution-neutrally
and the open decisions are explicitly tracked. OD-3 (JTBD job_id traceability) is a documentation
formality — see Open decisions. Peer review (nw-product-owner-reviewer) is the final hard gate.
