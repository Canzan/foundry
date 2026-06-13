# Acceptance Review (self-review) — web-provisioning-flow (DISTILL)

> Quinn (nw-acceptance-designer), DISTILL wave. Self-review against the AD critique dimensions
> (`nw-ad-critique-dimensions`) and the task's success criteria. Feature SSOT:
> `crates/foundry-acceptance/tests/features/us-mwt-web-provisioning.feature`.

## Critique dimensions (Dim 1–9)

| Dim | Check | Verdict |
|---|---|---|
| **1 — Happy-path bias** | error/edge ratio | **PASS** — 6/11 = 55% are `@error`/edge (≥40% mandate). |
| **2 — GWT compliance** | one When per scenario, Given context, Then outcome | **PASS** — each scenario is Given/When/Then; the dual-probe non-enumerability scenarios (#5/#6/#7/#9) use TWO `When` steps deliberately to capture the foreign-vs-control pair as ONE behaviour ("are refused identically"), matching the SHIPPED slice-02/04 idiom for byte-identity assertions — the asserted behaviour is the *comparison*, not two separate actions. |
| **3 — Business language purity** | no technical jargon in titles/steps | **PASS** — see Pillar 1 audit below. |
| **4 — Coverage completeness** | every AC / story leg has a scenario | **PASS** — US-MWT07 + US-MWT08 web legs both covered (test-scenarios.md story matrix). |
| **5 — WS user-centricity** | title = user goal; Then = observation | **PASS** — "A super-admin provisions a new isolated workspace from the browser"; Then = "exists and is isolated" + "the web page reports the new workspace and a first-admin invite link". |
| **6 — Priority validation** | addresses the largest gap | **PASS** — the non-shell-operator provisioning gap is the feature's reason to exist (architecture.md §6); the security core (non-enumerability) gets the most scenarios. |
| **7 — Observable behaviour assertions** | Then checks observable outcome, not internals | **PASS (with note)** — assertions are over rendered page/fragment substrings, HTTP refusal status + body bytes, and "workspace exists / starts empty / is unchanged" (observable tenant state via the same `db_introspect` read the SHIPPED slices use). "Exists / unchanged / starts empty" are user-observable tenant outcomes at this layer (LAYER-3, traditional assertions per Mandate 8), not private-field probes. |
| **8 — Traceability coverage** | story-to-scenario; env-to-scenario | **PASS** — every story ID covered (Check A). Check B (env matrix): no DEVOPS env matrix for this inherited feature; the single environment is the shipped testcontainers PG16 + in-process router, exercised by every scenario. |
| **9 — WS boundary proof** | strategy declared; real I/O; no @in-memory drift | **PASS** — strategy = Architecture-of-Reference real-adapter (policy row added); zero `@in-memory`; the WS deletes-real-adapter litmus holds (if the route/use-case were absent the WS reds — it is RED today, correctly). |

## The 3 Pillars

- **Pillar 1 (domain language)** — Scanned all titles + Given/When/Then steps. Zero technical jargon:
  no "HTTP", "POST" (as a verb the user performs — note: step text says "submits the provision form"
  / "posts to the legacy create-workspace path"; the latter is a deliberate domain reference to the
  *retired route* as a user-reachable address, mirroring slice-06's "/api/v1" surface references and
  slice-02's "by its real address" idiom — it names a user-reachable surface, not an implementation
  call). No "404"/"403"/"409" in step text — refusals are phrased "refused identically to a path that
  never existed" and "does not answer with the old conflict response". No "database"/"SQL"/"session
  cookie"/"CSRF token" — phrased "signed in on the web", "without a valid security token". **PASS.**
- **Pillar 2 (chained narrative)** — The grant line (#3 → #4) chains: #4's `Given` reuses #3's
  `Given + When` step vocabulary ("submits the grant form for …"). The provision line (#1 → #10 → #11)
  reuses "submits the provision form for workspace …" / "has provisioned … from the browser". The
  refusal pair (#6 signed-out → #7 signed-in non-admin) reuses "requests each /admin/instance route"
  + "requests a path that never existed" + "refused identically to the never-existed path". Step
  methods are shared, not copy-pasted fixtures. **PASS.**
- **Pillar 3 (app as in production)** — Every scenario drives the SHIPPED production composition root
  (`spawn_app()` in-process axum router over real HTTP) under the real session + real CSRF layers and
  real Postgres; no hand-rebuilt wiring. Only the shipped policy's external/non-deterministic fakes
  apply (none on this feature's assertion surface). **PASS.**

## Mandate compliance evidence

- **CM-A (Mandate 1 — hexagonal boundary)**: every scenario enters through the web driving port (the
  3 new `/admin/instance/…` routes + the retired legacy `/workspaces`) over real HTTP, never an
  internal component. The "first admin acts" leg (#11) enters through the SHIPPED
  `resolve_active_workspace` membership seam (a driving seam, as slice-06 established), NOT a direct
  store call. **No internal-component entry.**
- **CM-B (Mandate 2 — business language)**: Gherkin uses business terms only (Pillar 1 audit above);
  step glue (DELIVER's job) will delegate to the production composition root.
- **CM-C (Mandate 3 — user journeys)**: scenarios validate complete journeys with business value
  (provision a tenant, grant authority, be refused invisibly) — not isolated technical operations.
- **CM-H (Mandate 11 — integration sad paths example-based)**: #5/#6/#7/#8/#9 are named example-based
  sad paths; no PBT machinery imported at this LAYER-3 surface.
- **Mandate 9 (PBT mode layer-dependent)**: LAYER-3 throughout → example-only. No `@property`
  scenario, no `@given`/state-machine. Correct for a real-adapter feature.
- **Mandate 10 (two-tier)**: Tier A only. Tier B (state-machine PBT) NOT added — the journey is
  example-coverable and the input space is a finite enumerable matrix (3 routes × {signed-out,
  non-admin} refusal causes), not domain-rich. Adding Tier B would be over-built.

## Success-criteria self-check (from the task)

| Criterion | Verdict |
|---|---|
| Every D1–D6 exercised or explicitly noted non-testable-at-this-layer | **MET** — D1/D2/D3/D4/D5 exercised; D6 explicitly noted build-time non-testable-at-acceptance-layer (test-scenarios.md). |
| US-MWT07/08 web legs covered | **MET** — story matrix, both covered. |
| Scenarios drive the web driving port (in-process htmx router over real HTTP), not internals; @real-io real Postgres; no mocks | **MET** — feature-level `@real-io @driving_adapter`; policy row records real session/CSRF/PG; no `@in-memory`. |
| Walking skeleton first; remaining @pending | **MET** — 1 `@walking_skeleton @wiring_e2e`; 10 `@pending`. |
| Non-enumerability asserts BYTE-IDENTICAL refusal (status + body), not merely same-status (slice-04 found 4 oracles — be strict) | **MET** — #6/#7 assert "refused identically to the never-existed path" + #7 "byte-identical to the signed-out refusal"; phrasing reuses the SHIPPED slice-04 byte-identity step idiom, NOT a same-status check. |
| D5 honoured (no invite-accept sign-in scenario) | **MET** — no scenario follows the invite link; #1 asserts the link is RENDERED; #11's "first admin acts" rides `resolve_active_workspace` (the slice-06 approximation), not a live accept flow. |
| Legacy 409 route asserted RETIRED | **MET** — #9 asserts the legacy path is refused like a never-existed path and "does not answer with the old conflict response" (D3 RATIFIED RETIRE/DELETE). |
| House-style headers match slice-02/06 exemplars | **MET** — long top-of-file header: hypothesis + what disproves it; driving adapter w/ concrete routes; driven adapters (LAYER-3 @real-io); non-enumerability/refusal decision; explicit RED-state contract (crate COMPILES → not BROKEN; RED = MISSING_FUNCTIONALITY, enumerated); scope in/out; `@walking_skeleton @wiring_e2e` first, rest `@pending`. |
| Do not run cargo; do not commit | **MET** — no cargo invocation; no commit. |

## Pre-DELIVER fail-for-the-right-reason gate (deferred to DELIVER)

The gate (run the suite, classify each failure as MISSING_FUNCTIONALITY vs BROKEN) is executed in
DELIVER's RED phase per ADR-025, because running it requires the testcontainers PG16 + a cargo test
run, which this DISTILL session does not perform (task: "do not run cargo"). The RED-state contract
(test-scenarios.md) pre-classifies every scenario's expected RED cause as MISSING_FUNCTIONALITY so
DELIVER can confirm genuine RED at PREPARE/RED.

## Crate-compiles guarantee

This DISTILL session added ONLY a `.feature` file (Gherkin text) + docs + one policy-table row. It
did NOT edit `crates/foundry-acceptance/tests/acceptance.rs`, did NOT add step-definition glue, and
did NOT add any undefined-symbol reference to any `.rs`. Therefore the crate still COMPILES (the
feature is NOT BROKEN) — it is RED only at runtime once DELIVER registers steps and unskips the
walking skeleton. Step glue + the `acceptance.rs` force-link line are DELIVER's job.

## Verdict

**Self-review: APPROVED for handoff to the final wave review gate (Sentinel + the 3 wave reviewers)
and then DELIVER.** No blockers. One advisory: the dual-`When` non-enumerability scenarios (#5/#6/#7/
#9) intentionally pair a foreign/control probe in one scenario to assert the byte-identity comparison
as a single behaviour — this is the established SHIPPED slice-02/04 idiom and is the correct shape for
a non-enumerability oracle assertion (asserting the *difference is absent* requires both probes in
scope).
