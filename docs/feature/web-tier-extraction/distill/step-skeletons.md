# Step Definition Skeletons — Feature A "Programmatic Foundry"

The cucumber-rs step phrases (the contract between DISTILL and DELIVER) and the
precise list of what DELIVER must wire to flip each scenario GREEN. All step bodies
are IMPLEMENTED in `crates/foundry-acceptance/src/steps/feature_a_programmatic.rs`
(they issue real HTTP / spawn the real subprocess and assert the outcome) — DELIVER
does NOT rewrite the steps; it implements the PRODUCTION code that makes the
assertions pass.

## Step phrase inventory (declared in `feature_a_programmatic.rs`)

Reused from existing step files (NOT redeclared — cucumber-rs phrases are global):
- `a workspace "<name>" exists with admin "<email>"`           (us_06_signin.rs)
- `a member "<email>" belongs to the team "<team>"`             (us_07_project_create.rs)
- `a project "<name>" with key prefix "<KEY>" exists in the "<team>" team` (us_08_file_issue.rs)
- `<Who> is signed in`                                          (us_07_project_create.rs)

New Feature-A phrases (this file's contract):

| Kind | Phrase (regex-trimmed) | Used by |
|---|---|---|
| Given | `the "<P>" project has issue <K>-<N> titled "<T>" (in progress\|in the backlog)` | W05a/b/c |
| Given | `the "<P>" project has no issues` | W05a |
| Given | `the team "<team>" owns a project "<P>" with key prefix "<KEY>"` | W05b scope |
| Given | `the "<P>" project has a comment by <Who> on issue <K>-<N>` | W05c |
| Given | `the admin has granted a machine credential for "<label>" bound to <Who>` | W05b |
| Given | `the admin has granted a machine credential for "<label>" bound to <Who> with write access to "<P>"` | W05c |
| Given | `the admin has granted a machine credential bound to <Who> scoped to the "<team>" team` | W05b |
| Given | `the admin granted a machine credential bound to <Who> that has since expired` | W05b |
| Given | `the admin has granted a second machine credential bound to a member who is not the comment's author and not an admin` | W05c |
| Given | `the admin revokes that credential` | W05b |
| Given | `a caller holds a credential the workspace never issued` | W05b |
| Given | `a caller holds a credential signed with an algorithm the server does not accept` | W05b |
| Given | `a caller presents no valid credential` | W05a |
| Given | `a member account for "<email>" with password "<pw>"` | W05b regression |
| Given | `Mei is watching the "<P>" board in real time` | W05c |
| Given | `the project tree has no boundary violations` / `a copy of the tree in which ...` | W06 |
| When | `<Who> requests the "<P>" board's issues as machine-readable data` (+ read/reads variants) | W05a/b |
| When | `the machine requests the "<P>" board's issues with that credential` (+ variants) | W05b |
| When | `the machine files an issue titled "<T>" through the API` / `with an empty title` | W05c |
| When | `the machine moves <K>-<N> to "<state>" through the API` | W05c |
| When | `the machine posts a comment on <K>-<N> containing a script tag and a "javascript:" link through the API` | W05c |
| When | `that machine edits Mei's comment through the API` | W05c |
| When | `Mei signs in through the browser with her email and password` | W05b regression |
| When | `the maintainer runs the boundary check` / `on that copy` | W06 |
| Then | `the answer is a data list containing <K>-<N> and <K>-<N>` / `each entry carries ...` / `... reported in progress and ... backlog` / `the answer contains no markup` / `an empty data list` / `reported as successful` / `both list exactly the same set of issues` / `no issue data is returned` | W05a |
| Then | `the request is authenticated as the machine` / `the board's issues are returned as data` / `the request succeeds` / `refused as unauthenticated` / `refused as not-allowed` / `she receives a session cookie as before` / `... still requires an anti-forgery token ...` | W05b |
| Then | `a new issue is created with the next sequential key` / `... returned as data including its key and state` / `... starts in the backlog` / `appears on Mei's board` / `same core path ...` / `... state becomes in progress` / `the updated issue is returned as data` / `the comment is stored with the dangerous content removed` / `... matches what a browser-posted comment ... would store` / `rejected for a missing title` / `rejection reason matches the browser's "Title is required" rule` / `rejection ... no markup` / `the write is refused as not-allowed` / `the comment is left unchanged` | W05c |
| Then | `the check passes` / `the check fails` / `it names the handler that builds a page` / `it names the forbidden dependency` / `it reports the credential verifier no longer pins the single allowed algorithm` | W06 |

## What DELIVER must wire (the scaffold → GREEN plan)

The RED scaffolds compile and the scenarios reach their assertions, failing for the
RIGHT reason (see `red-classification.md`). To flip each group GREEN, DELIVER must:

### A. The `foundry-services` crate (ADR-W04/W07) — replace the `panic!`s
- Add `foundry-store` + `foundry-auth` dependencies to `crates/foundry-services/Cargo.toml`.
- Lift the use-case orchestration out of the `foundry-app` handlers (a PURE
  move-and-call, keeping HTML responses byte-identical — NFR-WEB-COMPAT-02):
  - `board::list_board_issues(&Store, &Principal, team_slug, project_slug) -> Result<Vec<BoardIssue>, ServiceError>` (reuses `Store::list_issues_by_project` + `is_team_member`).
  - `issues::create_issue` (reuses `insert_issue_with_outbox` + title validation).
  - `issues::change_issue_state` (reuses `update_issue_state_with_outbox` + the lifted `normalize_state`, DD10).
  - `comments::create_comment` (calls `render_comment_markdown` in core then `insert_comment_with_outbox`).
  - `comments::edit_comment` (authorship authz: author-or-admin; reuses `update_comment_with_outbox`).
- Rewire the existing `foundry-app` HTML handlers to call these (the regression net is
  the existing suite — it must stay green).

### B. The `foundry-api` crate (ADR-W01) — replace the `panic!`s + add axum
- Add `axum` + `foundry-store` (for the `machine_tokens` repo via the extractor) to
  `crates/foundry-api/Cargo.toml`. **Do NOT add a direct `foundry-store` write
  dependency for the use-cases** — those go through `foundry-services` (the cargo-deny
  rule, US-W06). The denylist read in `token_auth` is the one allowed store touch; if
  cargo-deny forbids it, route it through a `foundry-services` auth helper.
- Implement the route handlers (`routes::*`) as axum handlers calling `foundry_services::*`.
- Implement `token_auth::verify_bearer` per `auth.md`: parse `Bearer`, verify EdDSA
  with `algorithms = [EdDSA]` pinned, validate `exp`, check the `jti` denylist; build
  `Principal::Machine`. All failure paths → `ServiceError::Unauthorized` (non-enumerable).
- Implement `impl IntoResponse for ApiError` mapping each `ServiceError` to its
  `(status, envelope code, message)` per `error-and-observability.md` — JSON only.
- Export `pub fn routes(state) -> axum::Router` (the `/api/v1` sub-router), and an
  axum `FromRequestParts` extractor wrapping `verify_bearer`.

### C. Composition wiring (foundry-app) — the harness blast radius
Per `architecture.md` §Composition & harness blast radius:
1. Add `AppState.machine_token_verifier: Arc<MachineTokenVerifier>`, built at boot from
   `MACHINE_TOKEN_PUBLIC_KEYS` exactly as `session_secret` is built from `SESSION_SECRET`.
2. Update the ~4 acceptance `AppState` construction sites (`harness.rs:267`,
   `multi_replica_harness.rs:236,387`, `steps/us_03_backup_restore.rs:980`) + `main.rs`
   to set a FIXED test Ed25519 keypair (like the fixed `session_secret` test value).
   **This is the change that lets the W05b/W05c scenarios mint+verify real tokens.**
3. In `build_router`, `.merge(foundry_api::routes(state.clone()))` for the `/api/v1`
   group, mounted OUTSIDE the CSRF + session tower layers (auth.md §Coexistence). This
   is also what turns the W05c `403 CSRF token missing` RED into a real `201`/`422`.
4. Add `foundry-auth::MachineToken` (EdDSA mint/verify) + the `jsonwebtoken = "9"` dep
   (workspace + foundry-auth Cargo.toml).
5. Add migration `0007_machine_tokens.sql` (registry + `jti` denylist) + the `Store`
   repo methods (`insert/find_by_jti/revoke/list/touch`) + extend `Store::probe`.

### D. The harness credential helpers (DELIVER replaces the DISTILL placeholders)
The `admin has granted a machine credential ...` Givens currently store a placeholder
string in `world.fa_credential`. DELIVER replaces those bodies to actually mint a real
JWT via the admin issuance use-case (or directly via `MachineTokenSigner` in the test
harness with the fixed test key) and store the compact JWT. The `When` steps already
send `Authorization: Bearer <world.fa_credential>` — no change needed there. The
expired / forged / wrong-alg Givens mint (or hand-craft) the corresponding bad token.

### E. Boundary guard wiring (US-W06)
1. Implement `cargo xtask check-arch` (the AST source walk: no `Html`/`text/html` in
   `crates/foundry-api/src/**`; no `is_team_member`/`is_workspace_admin` there; the JWT
   `Validation` pins `[EdDSA]`). Wire into `xtask ci` + CI `lint-format`.
2. Add the `cargo-deny bans` crate-graph rules (adapter crates must not depend on
   `foundry-store`; no reversed `foundry-services → adapter` edge) to `deny.toml`.
3. Extend the acceptance harness so the `a copy of the tree in which ...` Givens
   actually materialise a throwaway tree copy with the planted violation, and the
   `When the maintainer runs the boundary check on that copy` runs `check-arch` against
   THAT copy. (The DISTILL scaffold runs `check-arch` against the real tree and records
   the exit/stderr; DELIVER adds the copy-with-violation staging — mirrors how
   `support::test_migration` stages a per-scenario tree for US-04.) Until then the
   violation scenarios fail RED on "the guard never ran" (the hardened assertion).

## Standing rules carried into DELIVER
- Every When step already drives the REAL `/api/v1` protocol (HTTP) or the REAL guard
  (subprocess) — no step calls a service function directly (Mandate 1 / Pillar 3).
- The `/api/v1` numbers (200/201/401/403/404/410/422) ARE the observable contract
  (api-contract.md), not implementation leakage — they belong in the Gherkin (Pillar 1
  exception, same as the existing suite's 4xx lines).
- The browser regression scenario (`browser sign-in path is unchanged`) is GREEN today
  and must STAY green — it is the live proof the credential surface did not touch the
  session/CSRF path. No `@skip`. (Foundry uses RED scaffolds + lane tags as the
  one-at-a-time mechanism, NOT per-scenario skip markers — matching the existing suite.)
