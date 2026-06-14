# DISTILL — Test Scenarios catalog: invite-accept-flow

> Quinn (nw-acceptance-designer), DISTILL wave. The executable SSOT is
> `crates/foundry-acceptance/tests/features/us-invite-accept.feature` (cucumber-rs).
> This catalog maps each scenario to US-01/02/03 + the security NFRs + D1-D7, states
> the per-scenario RED-state contract, and records the @walking_skeleton/@pending plan.
> Inputs read (SSOT): design/{wave-decisions.md (D1-D7 + RATIFIED OD table 2026-06-14),
> architecture.md, adr-001..005, upstream-changes.md}; discuss/{user-stories.md,
> acceptance-criteria.md, requirements.md, journey-invite-accept-visual.md (E1-E8),
> journey-invite-accept.feature}; docs/architecture/atdd-infrastructure-policy.md.

## Phase-0 / gate log

- `[lang-mode] rust` — `Cargo.toml` present; cucumber-rs per config.
- `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` exists; every
  port in scope (in-process HTTP via `spawn_app`, real PG16 testcontainers + per-scenario
  schema for `invites`/`users`/`workspaces`/`workspace_memberships`/`tower_sessions`, real
  double-submit CSRF, the SHIPPED `resolve_active_workspace`) is ALREADY in the policy
  (Slice-1 + multi-workspace-provisioning + web-provisioning-flow rows). No new row owed.
- `[port-mode] n/a` — no Rust `state_delta.rs` port exists; project precedent (slices 1-6 +
  web-provisioning) uses traditional assertions over port-exposed web observables at LAYER 3,
  permitted by Mandate 8 (state-delta is a layers-1-3 Python pilot; layer-3 real-adapter MAY
  use traditional assertions). No bootstrap — established convention followed.
- **Reconciliation HARD GATE: PASSED — 0 contradictions.** DISCUSS (System Constraints in
  user-stories.md + Scope in requirements.md + OD ratification) vs DESIGN (D1-D7): public
  signed-out route + CSRF-on-POST, single-use one-TX consume, uniform byte-identical refusal,
  first-admins-only, 7-day expiry, min-12 password — all CONSISTENT. The AC-01.3 "302" wording
  is a DOCUMENTED upstream correction to "303 SEE_OTHER" (DESIGN review iter-1 HIGH-1 +
  upstream-changes.md Finding 3; behavior unchanged) — reconciled, not a contradiction.
  Scenarios use business language ("signed in", "lands on the workspace") agnostic to the exact
  status code; the redirect mechanism lives in DELIVER step glue.

## Graceful degradation applied

- `discuss/wave-decisions.md` — NOT FOUND. WARN. DISCUSS decisions are embedded in
  `user-stories.md` (System Constraints), `requirements.md` (Scope), and the DESIGN OD table.
  Reconciliation used those embedded decisions. Non-blocking.
- `devops/` directory — NOT FOUND. WARN. Used the default infra from the existing
  `atdd-infrastructure-policy.md` (which already specifies the in-process HTTP harness + real
  PG16 + real session/CSRF). No environment matrix gap for this in-process web feature.

## Tier decision (Mandate 10)

**Tier A ONLY** (production composition root via `spawn_app`, example-based). Tier B
(state-machine PBT, in-memory doubles) is correctly SKIPPED:
- The feature runs at LAYER 3 (real PG, in-process HTTP, real session/CSRF) per the infra
  policy + config — Mandate 9/11 mandate example-only at layer 3+.
- The input space is security-ADVERSARIAL-ENUMERATED (4 refusal reasons, 2 password errors, 1
  race, 1 CSRF, 1 TOCTOU), best expressed as explicit named examples — exactly the slice-04
  adversarial-matrix posture, not generative PBT.
- The project has NO Rust `InMemoryComposition` and NO `state_delta.rs` port; all 6 prior slices
  + web-provisioning are Tier-A-only real-IO example-based. Introducing Tier B here would
  diverge from every precedent with no payoff.
The three `@property`-tagged scenarios stay EXAMPLE-PINNED at layer 3; their universal-invariant
SHAPE is preserved in the title for the DELIVER crafter (matching the journey feature + slice-04).

## Scenario catalog (18 scenarios; 1 @walking_skeleton + 17 @pending)

| # | Title (abbrev) | Story | AC / NFR | D# / ADR | Tags | RED-state (why it fails now) |
|---|---|---|---|---|---|---|
| 1 | Sets password & lands signed in | US-01 | AC-01.1/01.3/01.4/01.5 | D1/D2/D6, adr-001 | `@walking_skeleton @wiring_e2e @us-01` | No `/invites/accept` route, no `submit_accept`, no `set_first_admin_password_and_consume`, no auto-sign-in landing. |
| 2 | Live invite renders form naming workspace | US-01 | AC-01.1 | D6, adr-002 | `@pending @us-01` | No GET `show_accept_form` handler / set-password template. |
| 3 | Opening page consumes nothing | US-01 | AC-01.2 | D6, adr-001 | `@pending @us-01` | GET is non-committal — no handler exists to BE non-committal; the consume seam is absent. |
| 4 | Just-inside-expiry accepted | US-01 | AC-01.6 | D6, adr-001 | `@pending @us-01` | No liveness read / accept path; `expires_at - 1s` boundary unhandled. |
| 5 | Expired refused, no existence leak | US-02 | AC-02.3, NFR-3 | D3, adr-002 | `@pending @us-02 @error` | No `invite_refusal_page()` uniform refusal. |
| 6 | Just-past-expiry refused (= expired body) | US-02 | AC-02.4, NFR-1 | D3/D6, adr-002 | `@pending @us-02 @error` | No refusal page; `expires_at + 1s` unhandled. |
| 7 | Tampered sig refused identically | US-02 | AC-02.1/02.2, NFR-3 | D3/D6, adr-002 | `@pending @us-02 @error` | No HMAC-fail → uniform refusal arm; byte-identity unprovable. |
| 8 | Unknown id refused identically | US-02 | AC-02.1/02.2, NFR-3 | D3, adr-002 | `@pending @us-02 @error` | No unknown-id → uniform refusal arm. |
| 9 | @property byte-identical across 4 reasons | US-02 | AC-02.1/02.2, NFR-3 | D3, adr-002 | `@pending @us-02 @error @property` | No single refusal page → the four arms cannot be byte-identical (they do not exist). |
| 10 | Consumed invite refused (single-use) | US-02 | AC-02.5, NFR-2 | D1/D2, adr-001 | `@pending @us-02 @error` | No consume marker write → re-open cannot be refused as used. |
| 11 | @property concurrent accepts → once | US-02 | AC-02.6, NFR-2 | D1, adr-001 | `@pending @us-02 @error @property` | No guarded-UPDATE → no race oracle; both/neither would win. |
| 12 | CSRF-less POST refused, no consume | US-02 | AC-02.8, NFR-6 | D4, adr-003 | `@pending @us-02 @error` | No POST mounted under `csrf_middleware`; cookie not minted on GET. |
| 13 | @property no sig/password in logs | US-02 | AC-02.9, NFR-5 | D3, adr-002 | `@pending @us-02 @error @property` | No handler → no tracing-keyed-on-invite_id discipline to verify. |
| 14 | TOCTOU: consumed in GET→POST window refused | US-02 | AC-02.7, NFR-1/2 | D6, adr-001 | `@pending @us-02 @error` | No TX guard → GET→POST window unprotected. |
| 15 | Weak password inline, invite live | US-03 | AC-03.1, FR-5/NFR-4 | D5, adr-004 | `@pending @us-03 @error` | No `check_password_policy`; no inline re-render keeping invite live. |
| 16 | Mismatch inline, invite live | US-03 | AC-03.2, FR-5 | D5, adr-004 | `@pending @us-03 @error` | No confirm-match check / inline re-render. |
| 17 | Valid retry on same invite completes | US-03 | AC-03.4, FR-5 | D5, adr-001 | `@pending @us-03` | No policy-before-consume ordering → retry path absent. |
| 18 | Password exactly at min length accepted | US-03 | AC-03.3, NFR-4 | D5, adr-004 | `@pending @us-03` | No min-12 length-first policy → threshold boundary unhandled. |

## Coverage assertions (verify-before-return)

- **Every story exercised**: US-01 → #1,2,3,4 · US-02 → #5..14 · US-03 → #15,16,17,18.
- **Every security NFR exercised**: NFR-1 expiry → #4,6,14 · NFR-2 single-use/race → #10,11,14 ·
  NFR-3 non-enumerable → #5,7,8,9 · NFR-4 password policy → #15,18 · NFR-5 no-leak → #13 ·
  NFR-6 CSRF → #12.
- **Every AC covered**: AC-01.1 #1,2 · 01.2 #3 · 01.3 #1 · 01.4 #1 · 01.5 #1 · 01.6 #4 ·
  02.1 #7,8,9 · 02.2 #6,7,8,9 · 02.3 #5 · 02.4 #6 · 02.5 #10 · 02.6 #11 · 02.7 #14 · 02.8 #12 ·
  02.9 #13 · 03.1 #15 · 03.2 #16 · 03.3 #18 · 03.4 #17.
- **Sad paths E1-E8 covered**: E1 expired #5,6 · E2 already-used #10 · E3 tampered #7 ·
  E4 unknown-id #8 · E5 weak #15 · E6 mismatch #16 · E7 race #11 (+ TOCTOU #14) · E8 CSRF #12.
- **Non-enumerability bar**: #6,7,8,9 assert byte-identical (status + FULL body) AGAINST the
  canonical expired arm (#5), never merely same-status (the slice-04 4-oracle lesson).
- **Single-use proven exactly-once**: handler-level re-open (#10), guarded-UPDATE race (#11),
  TOCTOU window (#14) — three independent angles on NFR-2.
- **Password-not-consumed-on-error**: #15 (weak) and #16 (mismatch) both assert "invite still
  live and unconsumed"; #17 proves the same invite re-accepts.
- **Error/security ratio**: 11 of 18 carry `@error` = 61% (> 40% mandate).

## Driving-adapter coverage (Mandate / RCA-fix P1)

The DESIGN entry points are the two PUBLIC web routes. Both are exercised via their real HTTP
protocol through the in-process `spawn_app` router (NOT a direct service call):
- `GET /invites/accept?id&sig` → #2,3,4,5,6,7,8 (render + every GET refusal arm).
- `POST /invites/accept` (id+sig+password+confirm+_csrf) → #1,10,11,12,14,15,16,17,18 (consume,
  inline error, CSRF, race, TOCTOU). #1 (`@wiring_e2e`) is the protocol-level wiring proof
  (real HTTP request → real session cookie set → 303 landing).

## Adapter coverage table (Mandate 6)

| Driven adapter | @real-io scenario | Covered by |
|---|---|---|
| `invites` row + SHIPPED `used_at`/`used_by` (real PG, NO migration) | YES | #1 (consume), #10 (re-open refused), #11 (race once), #14 (TOCTOU) |
| `Store::set_first_admin_password_and_consume` (NEW one-TX guarded-UPDATE) | YES | #1, #11, #14, #17 |
| `tower_sessions` Postgres store (auto sign-in) | YES | #1 (session established), #10/#15/#16 (no session) |
| `csrf_middleware` + `ensure_csrf_cookie` (double-submit, cookie on GET) | YES | #12 (CSRF-less refused), #1 (valid token path) |
| `InviteToken::verify` (SHIPPED HMAC) | YES | #7 (tampered), #8 (unknown), #2 (valid) |
| `hash_password` (SHIPPED argon2id) | YES | #1 (hash written), #18 (12-char accepted) |
| `foundry_auth::check_password_policy` (NEW min-12) | YES | #15 (weak), #18 (boundary) |
| `resolve_active_workspace` (SHIPPED) | YES | #1 (lands on `invites.workspace_id`, only that tenant) |

Zero "NO — MISSING" rows. All driven adapters reached with real I/O. No costly-external adapter
in this feature → no `@requires_external` contract smoke needed.

## Test placement

`crates/foundry-acceptance/tests/features/us-invite-accept.feature` — the project's single
acceptance-feature directory (all 50+ existing features live here; cucumber-rs harness
`acceptance.rs` with `harness=false`). NOT the Python `tests/{type}/{feature}/acceptance/`
default — Rust/cucumber-rs precedent governs. Step glue (`src/steps/feature_invite_accept.rs`
+ `world.rs` fields + `lib.rs` registration + the `acceptance.rs` force-link `use`) is DELIVER's
job (per the deliverable instruction: Gherkin only; crate must still COMPILE — this file adds no
undefined-symbol reference and does not edit `acceptance.rs`).

## KPI / observability

No `docs/product/kpi-contracts.yaml` in this brownfield per-feature layout (legacy model). The
US outcome-KPIs (`discuss/outcome-kpis.md`) are product-telemetry ratios (consumed-with-session
/ issued; recover-on-same-invite rate) measured from `invites` + accept telemetry post-merge —
PO-reviewer scope, not DISTILL `@kpi` scenarios. No `@kpi` scenario authored (soft gate, warned).
