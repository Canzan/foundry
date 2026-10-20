# Evolution: api-openapi-document (the JSON API describes itself at /api/v1/openapi.json, CI holds the description to the code, and /docs/api renders it with Try it out)

**Finalized**: 2026-10-20
**Commits**:
- DISCUSS through DISTILL: `a6b4095` (53 pending scenarios, 163 rows, 54 RED stubs, ADR-API-OPENAPI-001..004).
- DELIVER steps: `06978bb` (01-01), `980b869` (01-02), `215b37c` (01-03), `75bc2a9` (01-05),
  `de66f92` (02-01), `3a61e53` (UI-5 drift-driver fix), `4017d00` (02-02), `0a30e50` (03-01),
  `3ca4f6f` (03-02), `c40a60e` (03-03). 01-04 was retired as validation-only in the roadmap.
- Refactor (L1-L3): `497aec4`.
- Review fixes: `58baa41` (02-02 re-run through DES: a re-revoke keeps the first revocation
  time), `c33d641` and `a05434b` (page-rule gaps, drift normalise keywords, planted mutation
  survivors).
- Follow-up: `99330d6` (store-level re-revoke test).

Gates:
- DES integrity: "All 9 steps have complete DES traces", exit 0.
- Phase 4 review: pass 1 REJECTED (D1-D8), pass 2 APPROVED after the user's product decision.
- Mutation (per-feature): 622 mutants, 565 caught, 10 missed, 19 timeouts, 28 unviable:
  95.1% (98.3% excluding timeouts).
- Lanes: aod-01 34/34, aod-02 107/107, aod-03 22/22 (15 browser rows), 2026-10-20.
- Full Docker `xtask ci`: run 2 1780/1781, the failure the known sqlx `'\0'` flake. **No
  single fully green run yet.**

Feature dir PRESERVED. **Not pushed, not released.** The document says version 0.16.0 until
the release bump.

Sources:
- `docs/feature/api-openapi-document/feature-delta.md` (DISCUSS, DESIGN, DEVOPS, DISTILL,
  DELIVER upstream issues UI-1..UI-9 and the DELIVER closing sections)
- `docs/feature/api-openapi-document/deliver/roadmap.json` and `execution-log.json`
- `docs/feature/api-openapi-document/distill/red-classification.md`
- `docs/feature/api-openapi-document/slices/slice-0{1..3}-*.md`

## Summary

Before this feature, `/api/v1` was described only in two prose `api-contract.md` files, and
nothing stopped them drifting from the router. Now:
- every instance serves a hand-written OpenAPI 3.1.0 document at `GET /api/v1/openapi.json`,
  with no credential, compiled in with `include_bytes!` so the served bytes are the committed
  file;
- three layers hold the file to the code: check-arch (`api-openapi-operations`,
  `api-openapi-statuses`), foundry-api unit tests (`schemars` shapes, OAS 3.1 schema
  validity, `info.version` = `CARGO_PKG_VERSION`), and behavioural tests on the real router
  and Postgres;
- `cargo xtask openapi [--check]` refreshes or checks the version; the Dockerfile builder
  runs `--check`;
- `GET /docs/api` renders the document with vendored swagger-ui 5.33.3 under Foundry's first
  Content-Security-Policy, and Try it out calls the API with a pasted machine token.

## Business context

The instance operator and integrators wiring CI to Foundry had to read Rust or prose to learn
which statuses an operation answers. The job is "know exactly what the API will answer before
I call it, and trust that it is current". The document is the contract of record; the drift
check makes "current" a CI property rather than a promise. The page lets a person try a call
without a client.

## Key decisions

- **Hand-written OAS 3.1, checked against the code** (ADR-API-OPENAPI-001), not generated.
  The file can describe what the code really does (axum's text/plain 400/413/415/422, the
  empty 405, the uniform 404 catch-all) and stays reviewable.
- **Status sets derived from the code** (ADR-API-OPENAPI-002): `api-openapi-statuses` walks
  handler shapes and `ServiceError` reachability through `status_for`, with a soundness
  clause forbidding `From` conversions into `ServiceError`. The reachability walk is slightly
  wider than the ADR (it can only add statuses) and fails closed on a computed validation code
  (UI-3).
- **Vendored swagger-ui 5.33.3 in the web tier** (ADR-API-OPENAPI-003), served from
  `/static/vendor/`, with an authored initializer that pins the spec URL and turns off query
  config, the validator and persisted authorization. No CDN.
- **A route-scoped CSP on `/docs/api`** (ADR-API-OPENAPI-004): `default-src 'none';
  script-src 'self'; ...; frame-ancestors 'none'`, plus XFO DENY, nosniff, no-referrer,
  no-cache. It is the seed of a site-wide policy. check-arch `api-docs-page` forbids inline
  script, handlers, styles and `unsafe-*` in `script-src`.
- **UI-1, the method router.** On axum 0.8.9 a chained `.fallback(` adds `Allow: GET,HEAD` to
  the 404, so the route is `any(api_not_found_handler).get(openapi_document)`. Rule 1 (vii)
  allows exactly that form on that path.
- **A re-revoke keeps the first revocation time** (user product decision, 2026-10-20).
  `revoke_machine_token` updates only `WHERE revoked_at IS NULL`; the answer is still 204.
  This replaced an orchestrator decision that had changed the test to expect a re-stamp.

## Work completed

| Step | Commit | What |
|---|---|---|
| 01-01 | `06978bb` | The served document skeleton, its route and unit tests |
| 01-02 | `980b869` | check-arch `api-openapi-operations` with the pending-operations list |
| 01-03 | `215b37c` | check-arch `api-openapi-statuses` with its soundness clause |
| 01-05 | `75bc2a9` | `cargo xtask openapi [--check]`, the image-build check, RELEASING steps |
| 02-01 | `de66f92` | Issue and comment operations with every answer (GREEN FAIL first: UI-4) |
| 02-02 | `4017d00`, re-run `58baa41` | Token operations; the pending list emptied and removed (GREEN FAIL first: UI-7) |
| 03-01 | `0a30e50` | Vendored swagger-ui and the initializer |
| 03-02 | `3ca4f6f` | `/docs/api` and its content policy (driver fix UI-8 first) |
| 03-03 | `c40a60e` | Try it out with the token dialog; check-arch `api-docs-page` (RED FAIL first: UI-9) |

## Lessons

- **Other projects on the same host collide with test ports.** CI run 1 failed in
  `comment_scoping` because canzan-lift's test servers held the ports; the test passed alone.
  `lsof` before blaming Docker, and run the tag-day `ci` on a quiet machine.
- **A test expectation change is a product decision.** UI-4 (drop `position` from the
  universe) and UI-7 (expect the re-stamp) were orchestrator decisions that made the tests
  fit the code. The Phase 4 review rejected both (D1, D2) and noted no escalation record (D3).
  The user decided the product rule; the code changed, the test went back to as written.
  Route such changes to the user before the step resumes, and record the decision.
- **Driver bugs can look like product bugs.** UI-5 (cached test binary newer than a planted
  file: a false green), UI-8 (`arguments` inside a callback) and UI-9 (a selector matching the
  header cell) were all harness defects found by RED/GREEN discipline, not product defects.
- **Mutation pays for check-arch rules.** The rule gold sets missed plants (UI-2, UI-3) that
  only mutation showed; `c33d641`/`a05434b` closed them.

## Issues encountered

- UI-1 to UI-9 in `feature-delta.md` § *DELIVER / [WHY] Upstream Issues*; UI-4 and UI-7
  carry 2026-10-20 superseded notes.
- The sqlx `'\0'` pool flake under full-suite concurrency (one aod-02 row in CI run 2; 107/107
  alone).
- Recorded gaps: the CSP "refused nothing" check runs in headless Chrome only; the
  execution log has no machine-readable entry for the product decision; the store-level
  re-revoke test came after the review, in `99330d6`.

## Permanent artifacts

Already permanent; nothing migrated at finalize (single `feature-delta.md` layout, no
`design/` or `discuss/` directory):
- `docs/product/architecture/adr-api-openapi-001-hand-written-document-checked-against-the-code.md`
- `docs/product/architecture/adr-api-openapi-002-status-set-derived-from-the-code.md`
- `docs/product/architecture/adr-api-openapi-003-vendored-swagger-ui-page-in-the-web-tier.md`
- `docs/product/architecture/adr-api-openapi-004-api-page-content-security-policy.md`
- `docs/product/architecture/brief.md` § "The JSON API describes itself, and CI holds it to
  the code" (inventory marked shipped at this finalize)
- `docs/product/journeys/journey-api-openapi-document.yaml`, `docs/product/jobs.yaml`,
  `docs/product/personas/persona-instance-operator.yaml`, `docs/product/kpi-contracts.yaml`,
  `docs/product/outcomes/registry.yaml`
- `crates/foundry-api/openapi.json` (the contract itself)

## Release (not done)

1. One fully green `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on a quiet machine.
2. `cargo xtask openapi` with the 0.17.0 bump, then `smoke`.
3. Push and tag only on the user's go; dev auto-deploys, prod is held for approval.
4. DoD 5 dogfood on dev and prod: document hash against the tagged file; Try it out with a
   scratch token; no other origin in the network log.

## Follow-ups / open items

- Every `/api/v1` refusal carries the envelope (OQ-9).
- A site-wide content policy, starting from `/docs/api`'s.
- A read-one-issue route; client SDK generation when integrators ask.
