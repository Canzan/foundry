# Slice 01: The instance serves its API description, and CI refuses one that lies

Stories: US-AOD-01, US-AOD-02 | Estimate: 2 days | job_id: `job-api-machine-readable-contract` | Walking skeleton

## Goal

`GET /api/v1/openapi.json` answers, with no token, an OpenAPI document that is
byte-identical to the committed file and fully describes one operation, listing
a project's issues, together with the `machineToken` scheme and the `Error`
envelope. CI fails when that operation's path, method, parameters, statuses or
shapes stop matching the code, or when the served copy and the committed copy
differ.

## IN

- `GET /api/v1/openapi.json`: public, evaluates no credential (D8), same bytes for every caller on a release (D7), describes itself (OQ-10).
- OpenAPI 3.1 (OQ-5), `servers: [{"url": "/"}]` (OQ-7), `info.version` = the footer's release version (OQ-8), `info.title` "Foundry API v1".
- `GET /api/v1/teams/{team_slug}/projects/{project_slug}/issues`, fully described: `200` `[Issue]`, `401`, the uniform `404`, `500`, the `405` for other methods on the path.
- The `machineToken` scheme and the `Error` schema with the D14 copy.
- The committed file and the rule that keeps it equal to the served bytes (OQ-6).
- The drift check in `cargo xtask ci`, with gold tests, and D13's "not yet documented" list holding the other 7 operations.

## OUT

- The other 7 operations' descriptions (slice 02). The UI page (slice 03).
- Any change to an existing `/api/v1` answer (D6).

## Learning Hypothesis

- **Disproves if it fails**: that one mechanism (generated, or hand-written plus a conformance test, whichever DESIGN picks) can catch drift in **statuses** as well as shapes, with no false alarm on a wording-only edit. If it can only check shapes, D3 is only partly met, and the user decides before 7 more operations are documented.
- **Confirms if it succeeds**: that the served-equals-committed rule survives a release bump without manual care beyond one command.

## Acceptance Criteria

- [ ] `curl https://foundry.unintelligent-design.us/api/v1/openapi.json` with no token → 200 JSON a standard validator accepts; `info.version` equals the footer.
- [ ] The served bytes equal the committed file; dev and prod on the same release give equal `shasum -a 256`.
- [ ] An expired token, a forged one and a session cookie get the same 200 and bytes.
- [ ] Adding `priority` to the issue answer, or a 409 path to list issues, fails `cargo xtask ci`, naming the operation, status and field. A wording-only edit passes.
- [ ] A new `/api/v1` operation that is neither documented nor on D13's list fails CI.
- [ ] Every existing API acceptance scenario is green and unchanged.

## Dependencies

- Shipped: `/api/v1`, `routes()`, `status_for`, `views::RELEASE_VERSION`.
- OQ-1, OQ-5, OQ-6, OQ-7, OQ-8, OQ-10 answered (recommended answers assumed).
- Reference class: token-management-api slice 03 (contract and boundary hardening), about 1.5 days, plus a new CI check.
