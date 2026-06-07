# Token-Management API — Pre-DELIVER RED Classification

> Pre-DELIVER fail-for-the-right-reason gate. Captured from a real default-lane
> run (`cargo test -p foundry-acceptance --test acceptance`) on the DISTILL
> scaffold. DELIVER reads this at PREPARE/RED to confirm RED is genuine.
>
> Suite result: `220 scenarios (204 passed, 16 failed)` / `1750 steps
> (1734 passed, 16 failed)`. The 16 failures are EXACTLY the 16 active
> token-management scenarios. Every Given/When step in those scenarios PASSED
> (✔) — the failure is the Then assertion firing against a route that 404s. Test
> crate compiles clean (`cargo check -p foundry-acceptance --tests` green), so
> there is no IMPORT_ERROR / BROKEN class anywhere.

| # | Scenario | RED class | Failure signal |
|---|---|---|---|
| 1 | An audit pipeline lists the workspace's tokens as data (WS) | MISSING_FUNCTIONALITY | `expected HTTP 200, got 404` (list route absent) |
| 2 | An empty registry answers with an empty list | MISSING_FUNCTIONALITY | `expected HTTP 200, got 404` |
| 3 | A non-management caller is refused without leaking | MISSING_FUNCTIONALITY | `expected HTTP 403, got 404` |
| 4 | A request with no credential is refused | MISSING_FUNCTIONALITY | `expected HTTP 401, got 404` |
| 5 | The list never exposes a token value | MISSING_FUNCTIONALITY | `expected HTTP 200, got 404` |
| 6 | A rotation job revokes; dead on next call | MISSING_FUNCTIONALITY | `expected HTTP 204, got 404` (delete route absent) |
| 7 | Revoking an already-revoked credential is harmless | MISSING_FUNCTIONALITY | `expected HTTP 204, got 404` |
| 8 | Revoking from another workspace reveals nothing | MISSING_FUNCTIONALITY | `expected HTTP 404` shape — refusal-identity assert vs absent route |
| 9 | A non-management caller cannot revoke | MISSING_FUNCTIONALITY | `expected HTTP 403, got 404` |
| 10 | A rotation job retires its own credential | MISSING_FUNCTIONALITY | `expected HTTP 204, got 404` |
| 11 | Re-running rotation against a retired credential | MISSING_FUNCTIONALITY | `expected HTTP 204, got 404` |
| 12 | A listed token reflects its revocation on next read | MISSING_FUNCTIONALITY | `the revoke before the re-list did not return 204` (delete route absent) |
| 13 | Every refusal carries a stable code | MISSING_FUNCTIONALITY | `expected HTTP 403, got 404` |
| 14 | Cross-workspace and unknown ids indistinguishable | MISSING_FUNCTIONALITY | non-enumerability assert vs absent route |
| 15 | An invalid/revoked credential refused identically | MISSING_FUNCTIONALITY | `expected HTTP 401, got 404` |
| 16 | A disallowed-algorithm credential is refused | MISSING_FUNCTIONALITY | `expected HTTP 401, got 404` |
| 17 | There is no programmatic mint surface | **GREEN ALREADY** | `✔` — POST returns 404 (structural absence is true TODAY; no-mint holds by construction) |
| 18 | A burst beyond the guardrail is throttled | `@pending` (NOT RUN in default/all lanes after the lane fix) | mechanism OPEN (OD-TMA-1) — scaffold; `statuses: [404 ×30]` when forced |

## Notes for DELIVER

- **No BROKEN/IMPORT_ERROR class present.** The 16 RED scenarios fail at the
  `Then` assertion because the `/api/v1/.../tokens` routes are not yet merged into
  `build_router`; the harness, the real EdDSA bearer minting, and the real
  Postgres seeding all succeed (every Given/When = ✔). This is the clean RED the
  gate requires.
- **Scenario #17 is already GREEN** — the no-mint boundary holds by structural
  absence today (a POST to the tokens collection 404s because there is no route).
  DELIVER must KEEP it green: adding the two token routes must NOT add a `POST
  .../tokens` route, and the proposed check-arch LAYER-1d no-mint rule
  (DD-TMA-04) enforces that at build time. When DELIVER adds the GET/DELETE
  routes, re-run #17 to confirm the POST still 404/405s.
- **Scenario #18 is `@pending`** — excluded from the default AND `@all` lanes by
  the `acceptance.rs` lane fix (the `!has("pending")` clauses). DELIVER unskips it
  once the rate-guardrail mechanism (OD-TMA-1/1b/5) is ratified and the bucket +
  test-only clock-advance affordance are wired. See `upstream-issues.md`.
- **DELIVER's job per ADR-025**: this DISTILL authored all ATs as RED scaffolds.
  DELIVER does NOT re-author them — it unskips (none here are `@skip`; they run
  RED) and merges `foundry_api::routes(state)` with the GET list + DELETE revoke
  handlers, the `TokenJson` shape (DD-TMA-02), the 204 + idempotency (DD-TMA-03),
  the no-mint check-arch rule (DD-TMA-04), and the rate guardrail (DD-TMA-05,
  after OD-TMA-1 ratification) — then the 16 flip GREEN.
