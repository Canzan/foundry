# DISTILL — Acceptance self-review: invite-accept-flow

> Quinn (nw-acceptance-designer), DISTILL wave. Self-review against
> nw-ad-critique-dimensions (Dims 1-9) + the 3 Pillars + Mandate Compliance (CM-A..H)
> before handoff to DELIVER. The executable SSOT is
> `crates/foundry-acceptance/tests/features/us-invite-accept.feature` (18 scenarios).

## Traceability matrix (story / NFR → scenario)

| Source | Scenarios |
|---|---|
| US-01 (accept + sign in, WS) | #1 (WS), #2, #3, #4 |
| US-02 (refuse invalid safely) | #5, #6, #7, #8, #9, #10, #11, #12, #13, #14 |
| US-03 (password-mistake recovery) | #15, #16, #17, #18 |
| NFR-1 expiry | #4 (just-inside), #6 (just-past), #14 (in-TX) |
| NFR-2 single-use atomicity | #10 (re-open), #11 (race), #14 (TOCTOU) |
| NFR-3 non-enumerable refusal | #5, #6, #7, #8, #9 (byte-identical bar) |
| NFR-4 password policy (min-12) | #15 (weak), #18 (boundary) |
| NFR-5 no secret leakage | #13 |
| NFR-6 CSRF on public POST | #12 |
| D1 single-use consume (adr-001) | #1, #10, #11, #14, #17 |
| D2 one-TX consume+write | #1, #14, #17 |
| D3 uniform refusal (adr-002) | #5, #6, #7, #8, #9, #10, #14 |
| D4 public-POST CSRF (adr-003) | #12, #1 |
| D5 password policy (adr-004) | #15, #16, #17, #18 |
| D6 GET non-committal / TOCTOU | #3, #4, #6, #7, #14 |
| D7 LAYER-1e (resolution seam) | #1 (lands via `resolve_active_workspace`) — confirmed at DELIVER check-arch |

No orphan AC; every story + every security NFR has at least one scenario.

## Dimension review (Dims 1-9)

- **Dim 1 — Happy-path bias**: PASS. 11/18 `@error` (61%) > 40%. Every refusal arm, the race,
  CSRF, TOCTOU, both password errors are present.
- **Dim 2 — GWT compliance**: PASS. Each scenario is one behavior, single When (the two
  `When … And …` lines in #11/#9 describe one concurrent/sweep action, the slice-04-sanctioned
  matrix idiom, not two distinct behaviors). 3-6 steps each.
- **Dim 3 — Business-language purity**: PASS. No `HTTP`, `route`, `endpoint`, `SQL`, `tx`,
  `404`, `200`, `argon2id`, `HMAC`, `CSRF-token-as-jargon` in any scenario title or step. "the
  standard 'invite is no longer valid' page", "the request-forgery protection", "a valid
  security token", "the strength policy" are domain phrasings. Technical detail lives in the
  header comment + DELIVER step glue only. (The header comment is engineering provenance, not a
  scenario — exempt by precedent: every shipped feature file carries one.)
- **Dim 4 — Coverage completeness**: PASS. Story→scenario map above; every AC-ID and every
  E1-E8 sad path covered (see test-scenarios.md coverage assertions).
- **Dim 5 — WS user-centricity**: PASS. See walking-skeleton.md litmus — title is the JTBD,
  Thens are user observations, stakeholder-confirmable.
- **Dim 6 — Priority validation**: PASS. The WS is the single highest-value cut (the dead-URL
  fix the whole feature exists for); the security crux (#9 byte-identity, #11 race) is the
  largest risk per DESIGN's STRIDE analysis and is given three independent single-use angles.
- **Dim 7 — Observable-behavior assertions**: PASS. Every Then asserts a user-visible outcome
  ("she is signed in", "sees a set-password form", "byte-identical refusal", "inline error",
  "no session is created") or a journey-foregrounded state observable read at the port boundary
  ("invite recorded as used exactly once") — no private-field, no method-call-count, no internal
  struct assertion. The "used exactly once" / "still live and unconsumed" clauses are read via
  the policy-sanctioned read-only `db_introspect.rs` SELECT (a port-exposed observable of the
  `invites` row's public state), matching how slices 1-6 assert post-write DB row presence.
- **Dim 8 — Traceability coverage**: PASS (Check A). Every story ID (US-01/02/03) tagged on ≥1
  scenario (`@us-01`/`@us-02`/`@us-03`). Check B (environment-to-scenario): no `devops/` env
  matrix; this is a single in-process web feature with one environment (the `spawn_app` +
  per-scenario-schema harness) — the Background establishes its preconditions. HIGH waived:
  no multi-environment surface to map.
- **Dim 9 — WS boundary proof**:
  - 9a strategy declared: YES (walking-skeleton.md, Architecture-of-Reference defaults +
    policy mechanism).
  - 9b strategy-match: YES — `@real-io`, real `spawn_app` + real PG, no `@in-memory` on the WS.
  - 9c adapter integration coverage: YES — every driven adapter has a real-I/O scenario
    (adapter coverage table, zero MISSING).
  - 9d fixture tier: "if I deleted the real consume TX / real session store, would #1 still
    pass?" NO — #1 asserts the invite is consumed exactly once AND a session lands her on the
    workspace; both require the real driven adapters. Not fixture theater.
  - 9e strategy drift: no `@in-memory` anywhere → no drift.

## 3 Pillars

- **Pillar 1 (domain language + specific actions)**: PASS — Dim 3 above; concrete persona
  (Priya), concrete workspace ("Northwind"), concrete boundary ("twelve-character password",
  "one second away from expiring").
- **Pillar 2 (chained narrative)**: PASS — the journey chains: #2 `Given+When` (opened link,
  saw form) becomes the `Given` of #3, #14, #15, #16, #18 ("Priya has opened her live invite …
  and seen the set-password form") via a SHARED step-method, not copy-pasted setup. #1's
  successful accept becomes #10's `Given` ("already set her password and signed in"). #15's
  left-live invite becomes #17's `Given` ("shown an inline password error and her invite is
  still live"). Reused step text — DELIVER writes each step-method once.
- **Pillar 3 (app as in production)**: PASS — SUT built via the production composition root
  (`spawn_app` = the real `build_router`); only the clock (`expires_at`) is faked via the
  policy's existing `MockClock` seam for the expiry-boundary scenarios. No hand-rebuilt wiring.

## Mandate compliance (CM-A..H)

- **CM-A (Mandate 1 — hexagonal boundary)**: PASS. All scenarios enter through the public
  `/invites/accept` driving port over real HTTP; no internal validator/parser/repo invoked
  directly in a scenario.
- **CM-B (Mandate 2 — business language)**: PASS (Dim 3).
- **CM-C (Mandate 3 — complete journeys)**: PASS. Each scenario is trigger → processing →
  observable outcome → value (e.g. #1: opens+submits → consume+sign-in → lands signed in →
  can work in her workspace).
- **CM-D (Mandate 4 — pure-function extraction)**: N/A-at-DISTILL / noted for DELIVER. The one
  pure-function candidate is `check_password_policy(pwd) -> Result<(), PolicyError>` (D5/adr-004)
  — DESIGN already places it as a pure, unit-testable `foundry-auth` fn; DELIVER unit-tests it
  directly (no fixture). No fixture is parametrized across environments in this feature.
- **CM-E (Mandate 8 — Universe-bound assertion)**: WAIVED at LAYER 3 per Mandate 8 (state-delta
  is a layers-1-3 Python pilot; layer-3 real-adapter MAY use traditional assertions). No Rust
  `state_delta.rs` port exists; project precedent (slices 1-6 + web-provisioning) uses
  traditional assertions over port-exposed web observables. Followed.
- **CM-F (Mandate 9 — PBT mode layer-dependent)**: PASS. This is a LAYER-3 feature → example-only.
  NO `@given`/PBT machinery. The three `@property` scenarios are example-pinned (their invariant
  shape kept in the title for the crafter) — exactly the journey-feature + slice-04 convention.
- **CM-G (Mandate 10 — two-tier)**: PASS. Tier A only; Tier B correctly skipped (layer-3,
  adversarial-enumerated input space, no project `InMemoryComposition`) — rationale in
  test-scenarios.md.
- **CM-H (Mandate 11 — integration sad paths example-based)**: PASS. Every sad path (E1-E8) is a
  named example-based scenario; no PBT machinery imported at this layer.

## Pre-DELIVER fail-for-the-right-reason gate

NOT executed here (`cargo` not run per the deliverable instruction — "do not run cargo; do not
commit"). The RED-state contract is asserted by grounding, matching the
`us-mwt-web-provisioning.feature` precedent:
- Crate COMPILES — the `.feature` file is Gherkin text; it references no undefined `.rs` symbol
  and does NOT edit `acceptance.rs` (force-linking `use` list untouched). Cucumber-rs leaves
  unmatched step text as a runtime skip, NOT a compile/collection error → NOT BROKEN.
- Genuine RED is MISSING_FUNCTIONALITY: grounding confirms (grep) NO `/invites/accept` route in
  `foundry-app/src`, NO `consume_invite`/`set_first_admin_password_and_consume` in
  `foundry-store/src`, NO `check_password_policy` in `foundry-auth/src`. Every scenario fails at
  runtime because the behaviour is unimplemented, not because of a fixture/import error.
- DELIVER's RED phase runs the gate for real (`cargo test -p foundry-acceptance`) per ADR-025 D2
  and confirms the classification before GREEN.

## Verdict

Self-review APPROVED for handoff to DELIVER. 0 blocker, 0 high, 0 open ambiguity beyond the two
non-blocking DELIVER-time confirmations already recorded in DESIGN (OD-5 LAYER-1e check-arch
run; the 303 status-code mechanism in step glue). Ready for the mandatory four-reviewer final
wave gate (Eclipse / Architect / Forge / Sentinel) against the full feature-delta when the
orchestrator dispatches it.
