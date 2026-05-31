# DISTILL Self-Review — Feature A "Programmatic Foundry"

Peer-review pass against `nw-ad-critique-dimensions` (Dimensions 1-9) + the 3 Pillars +
Mandates 1-11. Performed by the acceptance designer before handoff. 24 scenarios → full
review (>3, so no fast-path).

## Dimension 1 — Happy-path bias
**PASS.** 13/24 (54%) error/edge scenarios (target ≥40%). Auth (US-W05b) is 7/10 sad
paths; the guard (US-W06) is 3/4 violation paths. Every story has success + error +
boundary coverage. The `failure_modes` are derived from `auth.md`'s fail-closed
catalogue (missing/malformed/forged/expired/revoked/wrong-alg/out-of-scope), not just
inferred.

## Dimension 2 — GWT compliance
**PASS.** Every scenario is Given (context) → When (one action) → Then (observable
outcome). No multi-When scenarios. The US-W05a "same core path" scenario has two When
lines (read-as-data, then open-in-browser) — this is a deliberate comparison scenario
(observe two surfaces of one fact), acceptable per the "shared artifact" pattern; the
single behavioural assertion is "both list the same set."

## Dimension 3 — Business-language purity (Pillar 1)
**PASS (with one deliberate domain term).** A hard-jargon grep of the Gherkin (titles +
steps) found ZERO raw `JWT`/`JSON`/`HTTP`/`endpoint`/`SQL`/`crate`/`Postgres`/`bearer`/
status-numbers — they appear ONLY in the `#` provenance header comments (same convention
as the existing `us-06-signin.feature` header). De-jargoned domain phrasings used:
"machine-readable data" (not JSON), "no markup" (not HTML), "credential" (not JWT/token),
"refused as unauthenticated" (not 401), "refused as not-allowed" (not 403), "anti-forgery
token" (not CSRF). Deliberate domain terms kept: **"the API"** (the integrator persona's
own word; the feature is literally *about* the programmatic interface — DISCUSS glossary
defines "API tier" as ubiquitous language) and **"the database"** in US-W06 (the
maintainer's plain word for the persistence layer the boundary guards). Both are
defensible domain vocabulary, not implementation leakage.

## Dimension 4 — Coverage completeness
**PASS.** Every US-W05a/b/c/W06 AC maps to ≥1 scenario (coverage-matrix.md §Story→AC→
Scenario). Every Feature-A NFR maps to a scenario or a flagged DELIVER test
(contract-snapshot, secret-hygiene, one-binary-topology — the three correctly out of
acceptance scope).

## Dimension 5 — Walking-skeleton user-centricity
**PASS.** Exactly ONE `@walking_skeleton` for the feature (US-W05a *"An integrator reads
the board's issues as data"*) after demoting the W05b/W05c first scenarios to
`@slice2-entry` during this review. Litmus: title = a user goal (an integrator reads
data), not a technical flow; Then steps = consumer observations (a data list, the right
issues, no markup), not internal side effects (no "row inserted", no "route returns
200"). A non-technical stakeholder confirms "yes — an outside script can read our board."

## Dimension 6 — Priority validation
**PASS.** Slice order follows DISCUSS D2/story-map (read first = riskiest assumption
"is core presentation-neutral", then auth+writes). The walking skeleton attacks exactly
that risk. No secondary concern is addressed ahead of the headline.

## Dimension 7 — Observable-behaviour assertions
**PASS.** Every Then asserts a driving-port observable: the data list / its entries /
"no markup" (parsed from the response body), the refusal status + no-data-leak, the
created resource, the guard's pass/fail + named violation. NONE assert internal struct
fields, private state, or mock call-counts. Seeding helpers read the DB only to set up
preconditions and (for `no_second_*`-style checks) to confirm absence — never to assert
the feature's positive outcome (that comes from the port response). The false-GREEN
audit (Critical Rule 7) found and fixed two assertions that could pass for the wrong
reason (CSRF-403 collision; unknown-subcommand exit) — see red-classification.md.

## Dimension 8 — Traceability coverage
**PASS.** Check A (story→scenario): every US-W05a/b/c/W06 has `@us-w05a`/`@us-w05b`/
`@us-w05c`/`@us-w06` tags and ≥1 scenario. Check B (environment): the harness uses the
testcontainers Postgres + per-scenario schema environment (project default per the
Infrastructure Policy); the `@docker-compose` one-binary-topology environment scenario
is flagged for DEVOPS/DELIVER (gap #3).

## Dimension 9 — Walking-skeleton boundary proof
**PASS.** Strategy is recorded in the project Infrastructure Policy (the post-RETIRED
equivalent of wave-decisions WS strategy): driving = in-process axum + bearer; driven
internal = real Postgres; driven external = fixed test Ed25519 key + FakeClock. The WS
(US-W05a) carries `@real-io` and uses the real adapter (real Postgres, real router). Every
driven adapter has a `@real-io` scenario (coverage-matrix §Driven adapter coverage).
Litmus "delete the real adapter → would the WS still pass?": no — it reads real seeded
rows through the real router. No `@in-memory` on any WS.

## 3 Pillars
- **Pillar 1 (domain language)**: PASS — see Dim 3.
- **Pillar 2 (chained narrative)**: PASS — within US-W05b the auth scenarios reuse the
  shared `the admin has granted a machine credential ...` Given chain; the revoke
  scenario's Given reuses the grant step then adds `revokes that credential` (Given of N
  = Given+When of N-1, composed, not copy-pasted). US-W05c write scenarios chain off the
  same granted-write-credential Given.
- **Pillar 3 (app as in production)**: PASS — the SUT is built via the production
  composition root (`build_router` through `InProcHarness`/`spawn_app`); only the
  external/non-deterministic ports (Ed25519 key material, clock) are fixtures. No
  hand-rebuilt wiring.

## Mandates 8-11 (layered test discipline)
- **Mandate 8 (universe-bound state-delta at layers 1-3)**: these acceptance scenarios
  run at **layer 3-4** (subprocess-equivalent: real adapter + real I/O via the in-process
  binary over HTTP; the guard is a true subprocess). Per the layered-discipline table,
  layers 4+ MAY use traditional assertions, and layer 3 uses example-only. `assert_state_delta`
  is the layer 1-2 unit/in-memory contract — it belongs to DELIVER's `foundry-services`
  unit tests, not this acceptance set. **Correctly not applied here.**
- **Mandate 9 (PBT mode layer-dependent)**: PASS — these are layer 3+ scenarios, so
  EXAMPLE-ONLY. No `proptest`/PBT machinery imported in the step file. Sad paths are
  enumerated as named scenarios (Mandate 11), never generated.
- **Mandate 10 (two-tier acceptance)**: Tier A only. Tier B (state-machine PBT) is NOT
  warranted: the API journeys are 1-2 chained scenarios with config/CRUD-shaped inputs,
  not a ≥3-step domain-rich state machine. Per the skill's "skip Tier B when" criteria.
- **Mandate 11 (integration sad paths example-based)**: PASS — every sad path
  (missing/malformed/forged/expired/revoked/wrong-alg/out-of-scope/empty-title/non-author/
  three guard violations) is a named example scenario.

## Verdict
**APPROVED for handoff to DELIVER.** Two false-GREEN risks were detected by running the
suite and hardened during review (CSRF-403 collision on the authorization scenario; the
unknown-subcommand exit on the guard-violation scenarios). All 23 not-yet-implemented
scenarios fail RED for the right reason; the browser regression scenario is GREEN by
design; the existing suite stays green; the workspace is clippy- and fmt-clean.
