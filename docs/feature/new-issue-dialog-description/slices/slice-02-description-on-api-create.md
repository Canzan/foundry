# Slice 02 — Description on the API create endpoint

**Goal**: `POST /api/v1/teams/{team}/projects/{project}/issues` with `{"title":…,"description":…}` → `201` and
the description persists, restoring web/API rule-parity.

**Story**: US-02. **Depends on**: slice 01 (the service param must exist).

**IN scope**
- `CreateIssueRequest` gains `#[serde(default)] description: String` (ODD-2).
- `create_issue_handler` (`foundry-api/src/lib.rs:378`) passes it to the shared service instead of empty.
- Tests: create-with-description → `201` + read-back equality; create-**without**-description → `201`, empty
  `description_md` (existing-client compatibility); rule-parity assertion that web and API land the same
  normalized value.

**OUT of scope**
- Whether `IssueJson` echoes the description in the `201` body — the shipped body is `{key, number, title,
  state}`; widening the response is an api-contract change. **DESIGN decides**; default is NO change, since
  AC-02.2 is satisfied by the subsequent-read equality the contract already requires.
- The length bound + its 422 (slice 03).

**Learning hypothesis**: disproves **"one service serves both callers"** (NFR-WEB-API-CON-02) if the API path
needs its own validation, normalization, or error shape to accept a description. Confirms it if the diff is a
struct field and an argument.

**Acceptance**: `discuss/acceptance-criteria.md` US-02 (AC-02.1-.3; AC-02.4 lands with slice 03).

**Seams**: `create_issue_handler` + `CreateIssueRequest` (`foundry-api/src/lib.rs:378`); `EditIssueForm`'s
`#[serde(default)]` (`issues.rs:269`) as the shape to mirror; `Services::create_issue`
(`services/src/lib.rs:57`); `docs/…/api-contract.md` §Create-issue.

**Watch items**
- Existing clients omitting `description` must be unaffected — assert it, don't assume it.
- NFR-WEB-API-CON-02: the returned representation must equal a subsequent read. The handler already echoes the
  service-trimmed title for exactly this reason; description must follow the same discipline.

**Effort**: ~2 h. **Reference class**: the shipped `title` handling in the same handler.
