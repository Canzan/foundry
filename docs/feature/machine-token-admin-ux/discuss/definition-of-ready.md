# Definition of Ready — Machine-Token Admin UX

> 9-item hard gate per story. Evidence-backed. Stories: US-MT00 (`@infrastructure`),
> US-MT01..MT06 (user-visible). Two items remain conditionally open pending user
> confirmation of the open questions — flagged explicitly at the bottom.

## DoR Validation

### Story: US-MT00 (signer-in-AppState + created_by migration, `@infrastructure`)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | Verifier-only AppState; no `created_by` (verified in code + migration 0007). |
| User/persona identified | PASS (infra) | The platform; enables Priya (US-MT01) + Dana (US-MT06). `infrastructure_rationale` present. |
| 3+ domain examples | PASS | Issuer boot, verifier-only boot, NULL `created_by` row. |
| UAT scenarios (3-7) | PASS | 3 scenarios. |
| AC derived from UAT | PASS | 4 AC trace to the scenarios. |
| Right-sized | PASS | Substrate change; folds into Slice 1. |
| Technical notes | PASS | Reuses `MachineTokenSigner`; Q1 flagged; forward-only migration. |
| Dependencies tracked | PASS | None upstream; blocks US-MT01/MT06. |
| Outcome KPIs defined | PASS | Issuer-capability + 100% `created_by` on new rows. |
| **DoR Status** | **PASSED** | |

### Story: US-MT01 (mint, value shown once) — Walking Skeleton

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | No product mint path; env/test-key only (verified). |
| User/persona identified | PASS | Priya (admin) + Marco (integration owner). |
| 3+ domain examples | PASS | Mint for CI bot; navigate-away-before-copy; issuing-not-enabled. |
| UAT scenarios (3-7) | PASS | 4 scenarios (happy, never-re-shown, anxiety, edge). |
| AC derived from UAT | PASS | 6 AC trace to scenarios. |
| Right-sized | PASS | 4 scenarios, single demoable mint flow. |
| Technical notes | PASS | Reuses `mint`+`insert_machine_token`; depends US-MT00; Q1/Q6 flagged. |
| Dependencies tracked | PASS | Depends US-MT00; authz reuses US-MT05's check. |
| Outcome KPIs defined | PASS | <2 min time-to-token; 0 values persisted/logged. |
| **DoR Status** | **PASSED** | |

### Story: US-MT02 (list)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | No product list; DB-only today. |
| User/persona identified | PASS | Dana (reviewer) + Priya. |
| 3+ domain examples | PASS | Three tokens; empty workspace; revoked row visible. |
| UAT scenarios (3-7) | PASS | 3 scenarios (incl. cross-workspace isolation). |
| AC derived from UAT | PASS | 4 AC trace. |
| Right-sized | PASS | Pure read over `list_machine_tokens`. |
| Technical notes | PASS | Reuses `list_machine_tokens`; Q6 flagged. |
| Dependencies tracked | PASS | Authz reuses US-MT05; precedes US-MT03/MT06. |
| Outcome KPIs defined | PASS | 100% issued tokens visible. |
| **DoR Status** | **PASSED** | |

### Story: US-MT03 (revoke)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | No revoke surface; DB-only. |
| User/persona identified | PASS | Dana + Priya. |
| 3+ domain examples | PASS | Revoke→next-call-refused; idempotent re-revoke; cross-workspace. |
| UAT scenarios (3-7) | PASS | 4 scenarios (incl. evil-user). |
| AC derived from UAT | PASS | 6 AC trace. |
| Right-sized | PASS | Flag-flip over the SHIPPED denylist. |
| Technical notes | PASS | Reuses `revoke_machine_token` + denylist; depends US-MT02. |
| Dependencies tracked | PASS | Depends US-MT02. |
| Outcome KPIs defined | PASS | Revoke→refusal within one request. |
| **DoR Status** | **PASSED** | |

### Story: US-MT04 (scope + expiry)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | Defaults-only forces over-granting. |
| User/persona identified | PASS | Priya. |
| 3+ domain examples | PASS | Team-scoped 30d; expiry at cap; over-cap/cross-workspace team. |
| UAT scenarios (3-7) | PASS | 3 scenarios (incl. evil-user). |
| AC derived from UAT | PASS | 4 AC trace. |
| Right-sized | PASS | Adds inputs over US-MT01. |
| Technical notes | CONDITIONAL | Reuses `scope`/`exp`; **Q3 numbers/vocabulary are DESIGN** — flagged, not blocking DISCUSS. |
| Dependencies tracked | PASS | Depends US-MT01. |
| Outcome KPIs defined | PASS | Non-default share > 0; 0 over-cap. |
| **DoR Status** | **PASSED (with Q3 open for DESIGN)** | |

### Story: US-MT05 (admin-only authz)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | Privileged action needs an enforced boundary. |
| User/persona identified | PASS | Carlos (non-admin) refused; Priya allowed. |
| 3+ domain examples | PASS | Admin allowed; non-admin refused; cross-workspace. |
| UAT scenarios (3-7) | PASS | 3 scenarios (2 evil-user). |
| AC derived from UAT | PASS | 3 AC trace. |
| Right-sized | PASS | Reuses `is_workspace_admin`; explicit boundary + tests. |
| Technical notes | PASS | Reuses `is_workspace_admin`; refusal shape (404/403) is DESIGN. |
| Dependencies tracked | PASS | Check reused by US-MT01/02/03. |
| Outcome KPIs defined | PASS | 100% non-admin refused; 0 leaks. |
| **DoR Status** | **PASSED** | |

### Story: US-MT06 (audit: minted-by + last-used)

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| Problem statement clear | PASS | List lacks issuer + liveness. |
| User/persona identified | PASS | Dana. |
| 3+ domain examples | PASS | Attribute+triage; never-used; NULL created_by. |
| UAT scenarios (3-7) | PASS | 3 scenarios. |
| AC derived from UAT | PASS | 3 AC trace. |
| Right-sized | PASS | Two columns over US-MT02. |
| Technical notes | PASS | Depends US-MT00 (`created_by`) + US-MT02; reuses `last_used_at`. |
| Dependencies tracked | PASS | Depends US-MT00 + US-MT02. |
| Outcome KPIs defined | PASS | 100% post-feature tokens show issuer. |
| **DoR Status** | **PASSED** | |

## Overall: PASSED (conditional)

All 7 stories meet the 9-item DoR with evidence. Two items are **conditionally open** and
must be confirmed by the user/DESIGN before BUILD — neither blocks the DISCUSS handoff
because the requirement + risk are captured and the substrate is shipped:

1. **Q1 / NFR-MT-SEC-04 — signing-key-in-AppState security posture.** The single highest-
   priority confirmation. DISCUSS captures the requirement + risk; the user must accept the
   posture and DESIGN must specify the at-rest mechanism.
2. **Q3 — scope vocabulary + TTL default/cap numbers** for US-MT04. DISCUSS fixes that bounds
   EXIST and are server-enforced; DESIGN picks the numbers and confirms the vocabulary.

Q6 (surface: web UI vs API vs both) is captured surface-neutrally in every story's Elevator
Pitch (both candidate entry points named), so it does not block DoR.
