# DISTILL — Acceptance Review (self-review): workspace-member-invites

> Quinn (nw-acceptance-designer), self-review against the 9 critique dimensions + the mandate
> compliance checklist, before handoff to DELIVER. Fast-path NOT applicable (30 scenarios > 3) —
> full review pass.

## Dimension-by-dimension

### Dim 1 — Happy-path bias → PASS
16 of 30 scenarios are `@error` (53%, ≥40% required). Error/edge coverage: expired (13,14),
tampered (15), unknown-id (16), email-collision (17), five-arm byte-identity (18), consumed/single-use
(19), concurrency race (20), TOCTOU (21), non-admin (22), signed-out (23), CSRF both POSTs (24),
no-secret-leak (25), weak password (26), mismatched confirm (27), blank email (28).

### Dim 2 — GWT format compliance → PASS
Every scenario is Given(context)/When(single action)/Then(observable outcome). Multi-`When` only where
the action IS a chained pair central to the behavior (WS: invite then accept; 20/23/24: two racing /
two-surface submissions — the concurrency/uniformity is the behavior). No rambling (all ≤6 steps).
Background carries the single shared Given (Dana signed in as admin).

### Dim 3 — Business-language purity (Pillar 1) → PASS
Grep of scenario/step lines (excluding the header comment) for {POST,GET,HTTP,JSON,REST,API,endpoint,
database,SQL,status code,201,303,404,axum,sqlx,argon2,HMAC,csrf_middleware,insert_invite,tower} returns
only domain-vocabulary terms. "transaction guard" (scenario 21) is retained deliberately: it matches the
shipped `us-invite-accept.feature` scenario 14 title verbatim-in-spirit and names a DISCUSS-glossary
concept (the atomic single-use consume guard), not an implementation type. "security token" stands in
for CSRF; "no longer valid page" for the refusal; "shareable accept link" for the signed URL.

### Dim 4 — Coverage completeness → PASS
Every US-01..04 and every NFR-1..6 + FR-1..8 maps to ≥1 scenario (see `test-scenarios.md`
traceability tables). NFR-7 (accessibility) is non-gating per DISCUSS/DESIGN — correctly no scenario.
Every `failure_modes` entry from the visual journey (I-E1..5, A-E1..9) is covered:
I-E1→22, I-E2→23, I-E3→28, I-E4→24, I-E5→4; A-E1→14, A-E2→19, A-E3→15, A-E4→16, A-E5→26, A-E6→27,
A-E7→20, A-E8→24, A-E9→17. Zero uncovered failure modes.

### Dim 5 — Walking-skeleton user-centricity → PASS
WS title is a user goal; Given/When are user actions; Then are user observations (account exists, signed
in, member sees only Northwind, invite used once) — no internal side-effects asserted. Stakeholder-
confirmable. See `walking-skeleton.md` litmus.

### Dim 6 — Priority validation → PASS
The WS targets the highest-value cut (the conjunction of both new surfaces). The collision arm (17/18)
and concurrency (20) are prioritized because the DISCUSS+DESIGN risk table flags them HIGH-impact
(email-collision-as-500 leak; double-create under race). No secondary concern crowds out a larger gap.

### Dim 7 — Observable-behavior assertions → PASS
Then steps assert observable web/user outcomes: rendered form/fragment substrings, signed-in landing +
tenant visibility, byte-identical refusal body+status, "an account is created"/"no account is created",
"invite used exactly once", "no signature/password in logs". The DB-state assertions ("exactly one user
and one membership", "invite not consumed") are read through the read-only `db_introspect.rs` port as
the OBSERVABLE post-conditions of the use case — the established convention of every shipped
`foundry-acceptance` security scenario (slice-04, invite-accept), at LAYER 3 where the universe is the
port-exposed DB read surface, not an internal struct field. No `mock.called`, no private-field assertion.

### Dim 8 — Traceability coverage → PASS
**Check A (story→scenario)**: US-01/02/03/04 each have ≥1 tagged scenario (`@us-01`..`@us-04`). Zero
orphan stories. **Check B (environment→scenario)**: DEVOPS env file absent → default matrix; the LAYER-3
real-PG16 environment is the single target and the WS exercises it. (No multi-environment matrix applies
— single-binary single-Postgres deployment, per the shipped crate.)

### Dim 9 — Walking-skeleton boundary proof → PASS
- 9a strategy declared: yes — `docs/architecture/atdd-infrastructure-policy.md` (the two new web-surface
  rows + the `create_member_and_consume` driven-internal row) + `walking-skeleton.md`.
- 9b strategy-implementation match: WS is `@real-io` (real PG16 + real router), NOT `@in-memory`. ✓
- 9c adapter integration coverage: every driven adapter has a `@real-io` scenario (Mandate 6 table). ✓
- 9d WS fixture tier: deleting the real `create_member_and_consume` / real PG would RED the WS — it does
  not pass on a double. ✓
- 9e strategy drift: zero `@in-memory` tags on any scenario (the project has no in-memory acceptance
  tier). ✓

## Mandate compliance evidence

- **CM-A (Mandate 1 hexagonal boundary)**: scenarios enter through the two driving adapters only
  (the issuance web surface + the public accept web surface) over real HTTP; no scenario instantiates an
  internal validator/parser/entity. ✓
- **CM-B (Mandate 2 business language)**: Pillar-1 grep clean (Dim 3). ✓
- **CM-C (Mandate 3 complete journeys)**: every scenario is a full user journey (trigger → processing →
  observable outcome + value); the WS spans issue→accept→join→land. ✓
- **CM-E (Mandate 8 universe assertion)**: N/A at LAYER 3 with no Rust state_delta port — traditional
  assertions over port-exposed web/DB-read observables, per the policy + the shipped-crate convention.
  Recorded in the policy "State-delta port" note. ✓ (compliant by the layered-discipline table)
- **CM-F (Mandate 9 PBT mode by layer)**: zero PBT machinery; all LAYER-3 example-based; the 4
  `@property` scenarios are example-pinned with invariant SHAPE in the title. ✓
- **CM-G (Mandate 10 two-tier)**: Tier A only — correctly NO Tier B (no in-memory composition port; the
  feature's risk lives in real DB constraints/concurrency). ✓
- **CM-H (Mandate 11 example-based sad paths)**: every sad path (13–28) is a named example scenario; no
  PBT-generated inputs. ✓

## RED-state contract → PASS
Gherkin-only file; `acceptance.rs` untouched (force-linking intact); no undefined-symbol reference to
any `.rs` → crate COMPILES, NOT BROKEN. Genuine RED is MISSING_FUNCTIONALITY at runtime (issuance
routes/handlers, `create_member_and_consume`, the member dispatch arm, the `invite_accept_view`
extension — all absent). Per the task, `cargo` was NOT run and nothing was committed; the
fail-for-the-right-reason classification is performed at DELIVER PREPARE/RED.

## Verdict
**Self-review: APPROVED for handoff.** All 9 dimensions pass; all applicable mandates satisfied; error
ratio 53%; every story + NFR + failure-mode + decision traced; one user-centric walking skeleton; 29
`@pending` for one-at-a-time DELIVER. Open items below are non-blocking recommendations (orchestrator
auto-accepts).
