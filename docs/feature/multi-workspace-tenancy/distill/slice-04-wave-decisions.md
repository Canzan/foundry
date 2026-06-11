# Multi-Workspace Tenancy — Slice 4 (Cross-Tenant Non-Enumerability Hardening) DISTILL Wave Decisions

> Sentinel (nw-acceptance-designer), DISTILL wave, SLICE 4 ONLY (cross-tenant
> non-enumerability HARDENING + the adversarial matrix across ALL surfaces,
> US-MWT05). Legacy per-feature layout; trunk-based (commit to `main`, no
> branch/PR). Scenarios for Slices 1-3, 5-6 are NOT authored here. The scoping is
> ALREADY enforced (slices 2-3); slice 4 UNIFIES + PROVES the refusal is
> observationally identical everywhere. Slices 2-3 (per-surface uniform-404
> proofs) are the dependencies this slice unifies — referenced, not re-authored.

## Reconciliation HARD GATE result

**Reconciliation passed — 0 contradictions.**

Read + reconciled:
- `discuss/wave-decisions.md` (DM2 isolation fail-closed + non-enumerable at the
  scoping seam; OD-1 shared-schema; OD-2 multi-membership RATIFIED; the risk
  register entry "A 404-vs-403 (or timing/error-shape) difference reveals that
  B's resource exists" mitigated by NFR-MWT-SEC-02 + US-MWT05).
- `discuss/nfrs.md` — **NFR-MWT-SEC-02** (cross-tenant refusal is NON-ENUMERABLE;
  measurable = byte-equivalent status + body to a never-existed id, no
  403-vs-404 oracle, no field leak; verify = adversarial US-MWT05 scenarios on
  every surface + an API-contract check on the refusal envelope).
- `discuss/stories.md` — **US-MWT05** (another tenant's existence + resources are
  invisible on EVERY surface; 4 ACs: foreign ≡ missing on every surface; no
  403-vs-404 oracle; adversarial coverage of web reads/writes + admin actions +
  `/api/v1` reads + token revoke; REAL Acme/Globex fixtures).
- `slices/slice-04-non-enumerability-hardening.md` (the slice contract — the
  "Done when": foreign-id ≡ missing-id on every surface, no 403-vs-404 oracle,
  the adversarial matrix passes against real A/B fixtures).
- `design/adr-003-non-enumerability-contract.md` (option (b): generalise the
  shipped `find_*_in_workspace → None` idiom as the SINGLE refusal pattern; web
  = 404 page, API = shipped `status_for` 404 envelope, foreign-jti revoke =
  shipped non-enumerable `NotFound`; timing equivalence is STRUCTURAL — same
  `WHERE id AND workspace_id` query, not a constant-time hack; cross-tenant
  RESOURCE refusals NEVER 403; intra-workspace authz failures keep their shipped
  shape — boundary clause).
- `distill/slice-02-wave-decisions.md` + `distill/slice-03-wave-decisions.md`
  (the already-proven per-surface uniform-404s — see the audit below).

| DISCUSS / DESIGN decision | Position | Slice-4 relevance | Verdict |
|---|---|---|---|
| DM2 / NFR-MWT-SEC-02 non-enumerable refusal at the scoping seam | ADR-003 (b): `find_*_in_workspace → None`; web 404 page; API 404 envelope; never 403 cross-tenant | the WHOLE matrix (foreign ≡ missing, no oracle) | consistent |
| OD-MWT-D6 cross-tenant refusal status (per-surface) | web = 404 page; API = JSON 404 envelope (RESOLVED slices 2-3) | every matrix cell asserts that exact uniform 404 | consistent — carried |
| NFR-MWT-SEC-04 per-tenant authority does not cross | shipped `is_workspace_admin`; cross-tenant admin reach collapses to 404 (`admin_tokens.rs:48`) | scenario 8 (admin no-403) | consistent |
| ADR-003 boundary clause: this governs CROSS-tenant RESOURCE refusals only | intra-workspace authz keeps shipped shape | the matrix is scoped to CROSS-tenant reaches only | consistent |
| ADR-003 timing is STRUCTURAL (same query), not constant-time | asserted structurally, not by wall-clock | the timing-handling decision below | consistent |
| Carried invariant: shipped verify path + sign-in error unchanged | ADR-003 boundary clause | OUT of this matrix (regression-guarded in slice 3 sc 10) | consistent |

No DISCUSS↔DESIGN↔DEVOPS opposition. Gate passed.

## THE COMPLETENESS AUDIT — every tenant-scoped surface → covered-in-slice-N or GAP

Enumerated from the live route surfaces:
- `crates/foundry-app/src/lib.rs` `build_router` (web htmx tier) +
  `crates/foundry-app/src/attachments.rs` `build_routes` (merged attachment routes).
- `crates/foundry-api/src/lib.rs` `api_routes` (the JSON `/api/v1`).

A surface is **tenant-scoped** iff it reads/writes a `workspace_id`-bearing
resource (team / project / issue / comment / attachment / machine_token) by an
id/slug a cross-tenant actor could craft. Instance-global, public, and
session-only surfaces (sign-in, bootstrap, dashboard root, sign-out, forgot-
password, `/workspace/switch`, `/keyboard-help`, `/healthz`, `/readyz`,
`/static`, `/metrics`) are NOT tenant-scoped — no foreign-resource reach exists,
so they carry no existence oracle and are correctly out of the matrix.

### Web htmx tier (`foundry-app`)

| Web surface (route) | Tenant-scoped? | Refusal idiom | Uniform-404 proven in | Slice-4 matrix cell |
|---|---|---|---|---|
| `GET /team/{t}/project/{p}` (board) | yes | `find_team_by_slug(ws,..)`→`find_project_by_slug(ws,..)`→None | slice 2 (sc 4) | **2** (regression/completeness) |
| `GET /team/{t}/project/{p}/issues/{n}` (issue detail) | yes | same scoping chain → 404 page | slice 2 (sc 5) | **1 (WS)** |
| `POST /team/{t}/project/{p}/issues` (file issue) | yes | same chain → 404 page; no foreign row | slice 2 (sc 6) | **3** |
| `POST /team/{t}/project/{p}/issues/{n}/comments` (comment) | yes | same chain → `resource_not_found_page` 404 | **GAP** (slice-2 noted shared chain, not individually exercised) | **4** (closes the gap) |
| `POST /team/{t}/project/{p}/issues/{n}/state` (state change) | yes | `change_issue_state`→`NotFound`→`resolve_not_found_page` 404 | **GAP** | **5** (closes the gap) |
| `POST /team/{t}/project/{p}/issues/{n}/attachments` (upload) | yes | same chain → 404; no foreign row | **GAP** | **6** (closes the gap) |
| `GET /team/{t}/project/{p}/issues/{n}/attachments/{id}` (download) | yes | `find_attachment_for_requester(id, ws)`→None (the CANONICAL idiom) | **GAP** | **7** (closes the gap) |
| `POST /admin/tokens/{jti}/revoke` (admin revoke) | yes | non-admin/missing/foreign jti collapse to 404 (`admin_tokens.rs:48`) | slice 2 (sc 7) | **8** (matrix: + never-existed comparator, no-403) |
| `POST /admin/tokens` (mint), `GET /admin/tokens` (list) | session-workspace-implicit | acts on the SESSION's workspace; no foreign-id input to craft | n/a (no cross-tenant reach surface) | not in matrix — see note |
| `GET /team/{t}/project/{p}/issues/{n}/comments/{cid}` + `/edit`, `PATCH`/`DELETE` (comment edit/delete) | yes | same project-scoping chain → 404; author-only authz is INTRA-workspace | slice-5 (US-10) shipped; cross-tenant 404 via shared chain | **DEFERRED** — see residual R1 |
| `GET /team/{t}/project/{p}/issues/new`, `/search`, `/events` (SSE), `/projects/new`, `POST /projects` | yes | same `find_team_by_slug(ws,..)` chain → 404 | slice-1/2 board/issue chain (shared seam) | **DEFERRED** — see residual R1 |

### JSON `/api/v1` (`foundry-api`)

| API surface (route) | Tenant-scoped? | Refusal idiom | Uniform-404 proven in | Slice-4 matrix cell |
|---|---|---|---|---|
| `GET .../issues` (list) | yes | `list_board_issues` scoped by `principal.workspace_id()` → 404 | slice 1 (read) + slice 3 (sc 3) | **12** (oracle-hunt API leg) |
| `POST .../issues` (create) | yes | `create_issue` scoped → 404; no foreign row | slice 3 (sc 4) | covered slice 3; not re-authored |
| `PATCH .../issues/{n}` (state change) | yes | `change_issue_state` → `NotFound` → `status_for` 404 | **GAP** | **9** (closes the gap) |
| `POST .../issues/{n}/comments` (comment) | yes | `create_comment` → `NotFound` → 404 envelope | **GAP** | **10** (closes the gap) |
| `PATCH .../issues/{n}/comments/{cid}` (comment edit) | yes | scoped chain → 404; author-only authz intra-workspace | shipped (web-tier-extraction); cross-tenant via shared chain | **DEFERRED** — see residual R1 |
| `GET .../tokens` (list) | yes | `list_tokens(principal.workspace_id())` confines | slice 3 (sc 5, residual closure) | covered slice 3; not re-authored |
| `DELETE .../tokens/{jti}` (revoke) | yes | `revoke_token`: `row.workspace_id != principal ⇒ NotFound` | slice 3 (sc 6) | **11** (matrix: token-surface cell + still-active) |

### Audit verdict — every surface accounted for; NO existence oracle found

- **No GAP is a real oracle.** Every "GAP" row is a surface whose uniform-404
  was implied by the shared `find_team_by_slug(workspace_id,..) →
  find_project_by_slug(workspace_id,..)` scoping chain (or
  `find_attachment_for_requester`, or `revoke_token`'s workspace check), but had
  not been INDIVIDUALLY exercised by a slice-2/3 scenario. Slice 4's matrix
  cells (4-7, 9-11) close those gaps by asserting foreign-id ≡ never-existed-id
  directly on each. None reveals a 403-vs-404, body-echo, or shape oracle in the
  shipped code — they are green-by-inheritance behind the `0002` gate.
- **No `distill/slice-04-upstream-issues.md` is written** — the audit found no
  real existence oracle. If, at DELIVER RED→GREEN, any matrix cell reds for a
  REAL oracle (a 403 where a 404 is required, a body echoing the foreign id/slug,
  or a status/shape diff), THAT is the upstream finding and the file is authored
  then (the `__SCAFFOLD__` step + `@error` cell already reds on it for the right
  reason). See "Fail-for-the-right-reason" below.
- **Adversarial completeness critic** ("what surface did we NOT cover, what
  oracle remains?"): the only uncovered tenant-scoped surfaces are the comment
  edit/delete, the keyboard/search/SSE/project-create reads, and the API comment
  edit — all of which (a) share the SAME workspace-scoping chain proven by the
  cells in this matrix, so cross-tenant reach is the SAME 404 by construction,
  and (b) carry an INTRA-workspace authz dimension (author-only edit; team
  membership) governed by ADR-003's boundary clause, NOT the cross-tenant matrix.
  They are documented as residual **R1** (deliberately deferred, not a gap),
  because asserting them adds rows that exercise the identical seam without a new
  oracle class. The matrix covers one representative of EVERY distinct refusal
  idiom (project-scoping chain, attachment idiom, admin-revoke collapse,
  token-revoke workspace check, API 404 envelope).

## Oracle-hunt assertions (the heart of the matrix)

Every matrix cell asserts the FOREIGN reach and the NEVER-EXISTED reach are
observationally identical, and the cross-surface oracle-hunt scenario asserts the
no-403 / all-404 invariant:

1. **Status identity + 404, never 403.** Each cell: the foreign-resource refusal
   status `== 404` AND `== the never-existed refusal status`. The dedicated
   oracle-hunt scenario (12) gathers a web + an API cross-tenant refusal and
   asserts `no cross-tenant refusal is 403` and `every cross-tenant refusal is a
   non-enumerable 404` (the explicit no-403 turn).
2. **Body identity (byte-equal).** Each cell: the foreign refusal body `==` the
   never-existed refusal body (the web 404 page; the API `status_for` JSON
   envelope). A differing body shape would be a shape oracle.
3. **No id/slug echo.** `the web/API refusal reveals no foreign identifier`
   asserts the refusal body does NOT contain the foreign issue key (`GLOBEX-`),
   the foreign project/team slug (`core`/`platform`), the foreign workspace name
   (`Globex`), the foreign attachment id, or the foreign filename — checked
   UNCONDITIONALLY (the known Globex identifiers are always forbidden, so the
   assertion is never vacuous even when a reused slice-2 read When produced the
   refusal).
4. **No foreign mutation (write cells).** `no comment was created in "Globex"` /
   `no attachment was created in "Globex"` / `no "Globex" issue changed state` /
   `the "Globex" token "globex-ci" remains active` — workspace-scoped DB count /
   state snapshots prove the refused cross-tenant WRITE left the foreign
   workspace untouched (a write that 404s but still mutates would be a covert
   leak).

### Timing — handled STRUCTURALLY, not by wall-clock (ADR-003)

ADR-003 makes the foreign-id and missing-id paths timing-equal BY CONSTRUCTION:
both execute the SAME `WHERE id AND workspace_id` query and return `None` on the
same branch — there is no extra branch that could leak timing. Slice 4 asserts
this STRUCTURALLY: status + body identity ⇒ the same `None`/`NotFound` code path
was taken ⇒ the same timing profile. **No wall-clock timing scenario is
authored** — a latency comparison at layer 3 under `@all` parallelism is flaky
(the slice-2/3 docs and the shipped US-06 sign-in timing-symmetry scenario both
document `spawn_blocking`-pool contention making single-sample timing
unreliable). **Residual R2** records the genuine (accepted) concern: the
structural argument is sound for the query path, but a future change that adds a
data-dependent branch AFTER the scoped lookup (e.g. rendering more for a found
foreign row before refusing) could reintroduce a timing oracle — the structural
assertion (body identity) catches that class, since a divergent post-lookup
branch would also diverge the body.

## Tier classification

**Tier A only.** LAYER 3 (real Postgres via testcontainers + per-scenario schema
+ real HTTP via the in-process `InProcHarness`/`reqwest`, under the production
session + double-submit CSRF layers for the web cells and the real EdDSA
`MachinePrincipal` verifier for the API cells). Per Mandates 9 + 11:
example-based; every adversarial path enumerated explicitly; NO PBT machinery.
Per Mandate 10: Tier B (state-machine PBT, in-memory doubles) is NOT added — the
matrix runs at layer 3 with real I/O and the input space is fixed Acme/Globex
personas (not domain-rich), so Tier A examples cover it; there is no ≥3-scenario
chained journey (each cell is an independent foreign-vs-missing comparison). Per
Mandate 8: layer-3 uses traditional assertions over port-exposed observables
(HTTP refusal status + byte-identical body; refusal-body substring absence;
post-write workspace-scoped DB row/state) per the Layered Test Discipline table
(matching slices 1-3; no `state_delta.rs` Rust port exists — Python is the
canonical pilot).

## Scenario list + tags

File: `crates/foundry-acceptance/tests/features/us-mwt-slice-04-non-enumerability.feature`

| # | Scenario | Surface (matrix cell) | Tags | Active/Pending |
|---|----------|----------------------|------|----------------|
| 1 | A foreign web issue and a never-existed web issue are indistinguishable | web issue-detail READ | `@walking_skeleton @wiring_e2e @error` | **active** |
| 2 | A foreign web board and a never-existed web board are indistinguishable | web board READ | `@error @pending` | pending |
| 3 | A foreign web issue-create and a never-existed are indistinguishable | web file-issue WRITE | `@error @pending` | pending |
| 4 | A foreign web comment and a never-existed are indistinguishable | web comment WRITE (GAP closed) | `@error @pending` | pending |
| 5 | A foreign web state-change and a never-existed are indistinguishable | web state WRITE (GAP closed) | `@error @pending` | pending |
| 6 | A foreign web attachment-upload and a never-existed are indistinguishable | web upload WRITE (GAP closed) | `@error @pending` | pending |
| 7 | A foreign web attachment-download and a never-existed are indistinguishable | web download READ (`find_attachment_for_requester`, GAP closed) | `@error @pending` | pending |
| 8 | A foreign web admin revoke and a never-existed revoke are indistinguishable | web admin action (no-403) | `@error @pending` | pending |
| 9 | A foreign API state-change and a never-existed are indistinguishable | API state PATCH (GAP closed) | `@error @pending` | pending |
| 10 | A foreign API comment and a never-existed are indistinguishable | API comment WRITE (GAP closed) | `@error @pending` | pending |
| 11 | A foreign API token revoke and a never-existed are indistinguishable | API token revoke (still-active) | `@error @pending` | pending |
| 12 | No cross-tenant refusal anywhere is an existence-revealing 403 | cross-surface ORACLE HUNT (web + API) | `@error @pending` | pending |

Feature-level tags: `@multi-workspace-tenancy @mwt-slice-04 @real-io @driving_adapter @us-mwt05`.

- **Error/adversarial ratio**: 12 of 12 = **100%** (this is a hardening +
  adversarial-matrix slice — every scenario is an evil-user / oracle-hunt cell;
  far exceeds the 40% bar).
- **Story coverage**: US-MWT05 — all four ACs covered: foreign ≡ missing on
  EVERY surface (1-11); no 403-vs-404 oracle anywhere (8, 12 + each cell's `404
  not 403`); adversarial coverage of web reads/writes + admin actions + `/api/v1`
  reads + token revoke (1-12); REAL Acme/Globex fixtures (every Background). No
  synthetic uuids — the never-existed comparators are absurd-but-real-shape ids
  under the acting workspace's own real routes (existence is the only variable).
- **Walking skeleton**: exactly ONE (`@walking_skeleton`, scenario 1) —
  demo-able: "a member of Acme reaching a real Globex issue is indistinguishable
  from reaching an issue that never existed." Active (un-skipped RED).

### Active-vs-@pending choice (deliberate)

Scenario 1 is ACTIVE (`@walking_skeleton`); scenarios 2-12 are `@pending`,
matching the one-at-a-time DELIVER cadence slices 1-3 established (one scenario
per RED→GREEN→COMMIT cycle; `@pending` is excluded by BOTH the default and `@all`
lanes per `tests/acceptance.rs`). **Why mostly pending despite being
green-by-inheritance:** even though the shipped scoping makes each cell
green-by-inheritance once `0002` ships, the matrix is large (12 cells) and
DELIVER unskips deliberately so a cell that DOES surface a real oracle is
isolated to its own RED→diagnose→(fix-or-flag) cycle rather than hidden in a bulk
run. The walking skeleton stays active to prove the wiring + the central claim
end-to-end immediately.

## Adapter coverage table (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Web project-scoping chain (`find_team_by_slug(ws,..)`→`find_project_by_slug(ws,..)`) | YES | 1, 2, 4, 5, 6, 12 |
| Web file-issue WRITE scoped by acting `workspace_id` | YES | 3 (+ no-foreign-row count) |
| Web comment WRITE scoped (`comments` insert under the scoped chain) | YES | 4 (+ no-foreign-comment count) |
| Web state-change WRITE (`change_issue_state` scoped) | YES | 5 (+ foreign-state-unchanged snapshot) |
| Web attachment UPLOAD scoped (`insert_attachment` under the scoped chain) | YES | 6 (+ no-foreign-attachment count) |
| Web attachment DOWNLOAD (`find_attachment_for_requester(id, ws)` — the canonical idiom) | YES | 7 |
| Web admin revoke non-enumerable 404 (`is_workspace_admin` + `admin_tokens` collapse) | YES | 8 |
| API issue READ scoped (`list_board_issues`) | YES | 12 (API leg) |
| API state-change PATCH scoped (`change_issue_state` → 404 envelope) | YES | 9 (+ foreign-state-unchanged) |
| API comment WRITE scoped (`create_comment` → 404 envelope) | YES | 10 (+ no-foreign-comment count) |
| API token REVOKE cross-tenant `NotFound` (`revoke_token`) | YES | 11 (+ Globex token still active) |
| tower-sessions Postgres store + double-submit CSRF (real signed-in cookie) | YES | 1-8 (every signed-in web cell) |
| `machine_tokens` registry + EdDSA verify (`token.workspace_id` resolution) | YES | 9-12 (real Acme bearer) |
| htmx web driving adapter (`foundry-app` over real HTTP) | YES | 1-8, 12 (RCA-fix P1 — real signed-in HTTP) |
| JSON API driving adapter (`foundry-api` over real HTTP) | YES | 9-12 (real bearer HTTP) |
| The `0002` forward-only migration (drop `uniq_one_workspace`) | YES | Background of every scenario (second workspace insert) |

Zero `NO — MISSING` rows. All driven adapters in slice-4 scope are exercised with
real I/O. Mechanism per the Project Infrastructure Policy
(`docs/architecture/atdd-infrastructure-policy.md`) — all ports already recorded
(HTTP API via `spawn_app`/`reqwest`; PgPool via testcontainers + per-scenario
schema; tower-sessions Postgres store; EdDSA fixed test keypair; multipart upload
via `reqwest::multipart`). **No policy rows added this run** (`--policy=inherit`,
every slice-4 port present).

## NEW steps/fixtures vs reused

**NEW (slice-4 phrases — globally-unique cucumber-rs step text), in
`src/steps/feature_mwt_slice_04_non_enumerability.rs`:**
- Given `the "<ws>" project "<p>" issue <KEY> has an attachment` — seeds a REAL
  `issue_attachments` row in the named workspace (the foreign download target;
  the canonical `find_attachment_for_requester` idiom).
- When (web writes): `the member comments on issue <KEY> in the "<ws>" project
  "<p>" on the web` (+ `… on an issue that never existed …`); `the member
  changes the state of issue <KEY> …` (+ `… an issue that never existed …`); `the
  member uploads a file to issue <KEY> …` (+ `… an issue that never existed …`).
- When (web reads/admin): `the member downloads the "<ws>" attachment on the web`
  (+ `… an attachment that never existed …`); `the "<ws>" admin tries to revoke a
  credential that never existed on the web` (the never-existed comparator for the
  reused slice-2 foreign-revoke When).
- When (API writes): `the Acme-bound credential changes the state of issue <KEY>
  in the "<p>" project over the API by its real address` (+ `… an issue that
  never existed …`); `the Acme-bound credential comments on issue <KEY> …` (+ `…
  an issue that never existed …`).
- When (oracle-hunt): `the member probes the "<ws>" issue <KEY> in project "<p>"
  across the web and the API` — gathers BOTH a web and an API cross-tenant
  refusal into the no-403/all-404 accumulator in ONE step.
- Then (oracle-hunt + no-mutation): `the web refusal reveals no foreign
  identifier`; `the API refusal reveals no foreign identifier`; `no comment was
  created in "<ws>"`; `no attachment was created in "<ws>"`; `no "<ws>" issue
  changed state`; `no cross-tenant refusal in this scenario is a 403`; `every
  cross-tenant refusal in this scenario is a non-enumerable 404`.
- World fields added to `src/world.rs` (9 `mwt4_*` slots):
  `mwt4_first_refusal_body`, `mwt4_first_refusal_status`,
  `mwt4_foreign_identifiers`, `mwt4_refusal_statuses`,
  `mwt4_attachment_id_by_label`, `mwt4_foreign_comment_count_before`,
  `mwt4_foreign_attachment_count_before`, `mwt4_foreign_issue_state_before`.

**REUSED (registered step text bound by exact regex match, NOT re-declared —
cucumber-rs requires globally-unique step text):**
- The two-workspace SEED Background — slice-1's `workspace "…" exists with admin
  "…"`, `"…" has a member … with project … prefix …`, `the "…" project "…" has
  issues …-1 and …-2`; the bearer-bind `a machine credential is bound to "…" in
  workspace "…"`.
- Slice-2 web steps: `"<email>" is signed in on the web acting on workspace
  "<ws>"`; the foreign-vs-missing board/issue/file Whens (scenarios 1-3 reuse
  them verbatim); `the "<ws>" workspace has an admin credential "<label>"` + `the
  "<ws-A>" admin tries to revoke the "<ws-B>" credential "<label>" on the web`
  (scenario 8); **`the two web responses are refused identically`** (reads
  `mwt2_first_refusal_*` vs `mwt2_last_*` — slice-4's NEW web Whens populate
  EXACTLY those slots, so the slice-2 assertion is reused verbatim); `no "<ws>"
  data appears on the web`; `no "<ws>" membership or credential is changed`.
- Slice-3 API steps: `a managed token "<label>" exists in workspace "<ws>"`; the
  token-revoke Whens (`… revokes the "<ws>" token "<label>" over the API` + `… a
  token id that exists nowhere over the API`); **`the two API responses are
  refused identically`** (reads `mwt3_first_refusal_*` vs `mwt_last_*` — slice-4's
  API Whens populate those); `the two API revoke responses are refused
  identically as not found`; `the "<ws>" token "<label>" remains active`.

Deliberately NOT reused: a fresh harness reset on subsequent steps (would discard
the first workspace — same hazard slices 1-3 document). Slice-4 owns a
self-contained `sign_in_cookie` + `web_get` (mirrors slice-2) so it does not
depend on slice-2's private helpers; cucumber-rs binds reused step TEXT, the
helper FUNCTIONS are module-private and intentionally duplicated minimally.

## Scaffold inventory (Mandate 7 / RED-ready)

- `.feature`: `crates/foundry-acceptance/tests/features/us-mwt-slice-04-non-enumerability.feature`
  (12 scenarios; #1 active `@walking_skeleton`, #2-12 `@pending` per one-at-a-time).
- Steps: `crates/foundry-acceptance/src/steps/feature_mwt_slice_04_non_enumerability.rs`
  (carries the `__SCAFFOLD__` / `SCAFFOLD: true` marker per Mandate 7;
  registered in `src/lib.rs` `pub mod steps {…}` + force-linked via
  `use … as _feature_mwt_s04` in `tests/acceptance.rs`).
- World fields added to `src/world.rs` (9 `mwt4_*` slots).
- **No production-source scaffold stub needed**: the production surface this
  slice requires (the `0002` migration dropping `uniq_one_workspace`, shared with
  slices 1-3) is a DELIVER-authored migration file, not a Rust module that
  step-defs import; the scoping/refusal idioms (`find_*_in_workspace`,
  `find_attachment_for_requester`, `revoke_token`'s workspace check, the web 404
  page, the API `status_for` envelope) are SHIPPED. So there is no module to
  stub. The step file calls REAL HTTP against the in-process harness — RED comes
  from the Background's second-workspace insert failing on `uniq_one_workspace`,
  not from a stub. The crate COMPILES clean, so the test is RED-not-BROKEN by
  construction.

**Gates run this slice:**
- `cargo test -p foundry-acceptance --no-run` → Finished (compiles,
  RED-not-BROKEN).
- `cargo fmt --all -- --check` → clean (exit 0).
- `cargo clippy --all-targets --release -- -D warnings` → clean (exit 0, 0
  warnings — full-workspace lint per the verify-with-full-workspace-lint
  discipline).

## Test placement + precedent

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/*.rs` — matches
EVERY prior feature (us-w05a, us-tma, us-mt0x, us-0x, us-mwt-slice-01/02/03).
Feature filename `us-mwt-slice-04-non-enumerability.feature` + step module
`feature_mwt_slice_04_non_enumerability.rs` mirror the slice-named slice-1/2/3
precedent.

## Pre-requisites — what DELIVER must build vs what is green-by-inheritance

**DELIVER must build (the single RED edge, shared with slices 1-3):**
1. **`0002_multi_workspace.sql`** (ADR-006): `DROP INDEX uniq_one_workspace;` so
   the Background's second `workspaces` row can exist. If slices 1-3 already
   shipped `0002`, this is satisfied and EVERY slice-4 cell is green-by-
   inheritance.

**Green-by-inheritance (shipped — slice 4 PROVES uniformity under two real tenants):**
2. The web ADR-005 membership resolution + `/workspace/switch` (shipped by slices
   1-2) — scopes the signed-in member to Acme so each cross-tenant reach
   collapses to the shipped `find_*_in_workspace → None` 404.
3. The shipped project-scoping chain (`find_team_by_slug(ws,..)` →
   `find_project_by_slug(ws,..)`), `find_attachment_for_requester(id, ws)`,
   `change_issue_state`, `create_comment`, `insert_attachment`, the web 404
   page (`resource_not_found_page`), the API `status_for` 404 envelope,
   `revoke_token`'s `row.workspace_id != principal ⇒ NotFound`, and the
   `admin_tokens` non-enumerable 404 collapse — all SHIPPED; the matrix asserts
   they refuse foreign ≡ missing UNIFORMLY.

**Flagged real-oracle gaps:** NONE. The audit found no surface with a 403-vs-404,
body-echo, or shape oracle. No `distill/slice-04-upstream-issues.md` is written.
(If a DELIVER RED reveals one, author it then.)

## Fail-for-the-right-reason expectation (per scenario)

The crate compiles clean (no import/collection error → not BROKEN). At runtime
against the real testcontainers PG16, every scenario reds for
MISSING_FUNCTIONALITY (the `0002` guard drop), then — once `0002` ships — proves
the shipped uniform refusal under two real tenants:

| # | RED cause (the genuine missing functionality), then the green-once-`0002` proof |
|---|---|
| 1 | Background's 2nd `INSERT INTO workspaces` fails on `uniq_one_workspace`; once 2 coexist + Acme-scoped, the foreign Globex issue must 404 byte-identically to a never-existed issue, body echoing no `GLOBEX-`/`Globex`/`core` — shipped `find_*_in_workspace → None` + 404 page. |
| 2 | Same Background red; then foreign board ≡ never-existed board (same chain). |
| 3 | Same; then foreign file-issue write ≡ never-existed write, 404 + no Globex row. |
| 4 | Same; then foreign comment write ≡ never-existed, 404 + no Globex comment (the GAP-closing cell — shared scoping chain proven directly). |
| 5 | Same; then foreign state-change ≡ never-existed, 404 + Globex issue state unchanged. |
| 6 | Same; then foreign upload ≡ never-existed, 404 + no Globex attachment. |
| 7 | Same; then foreign attachment download ≡ never-existed via `find_attachment_for_requester(id, ws)` → None (the canonical idiom proven under two real tenants). |
| 8 | Same; then an Acme admin revoking a Globex credential is a non-enumerable 404 (NOT a 403) ≡ never-existed credential, Globex credential unchanged. |
| 9 | Same; then foreign API state PATCH ≡ never-existed, `status_for` 404 envelope + Globex issue unchanged. |
| 10 | Same; then foreign API comment ≡ never-existed, 404 envelope + no Globex comment. |
| 11 | Same; then foreign API token revoke ≡ never-existed jti, 404 + Globex token still active. |
| 12 | Same; then BOTH a web and an API cross-tenant refusal are 404 (never 403) — the explicit no-403 oracle-hunt across two surfaces. |

This is the RED-phase entry signal DELIVER reads at PREPARE (ADR-025 D2). No
scenario reds for a fixture/import/setup reason — the genuine missing
functionality is the `0002` guard drop (shared with slices 1-3); everything else
is the shipped non-enumerable boundary PROVEN UNIFORM across the full surface
matrix under real two-workspace fixtures. A cell that reds AFTER `0002` ships
(e.g. a 403, a body echo, a shape diff) is a REAL oracle finding → flag in
`distill/slice-04-upstream-issues.md` for DELIVER.

## Residuals (deliberate, not gaps)

- **R1 — comment edit/delete, keyboard/search/SSE/project-create reads, API
  comment edit are NOT individually in the matrix.** They share the SAME
  workspace-scoping chain proven by cells 4-5/9-10, so cross-tenant reach is the
  SAME 404 by construction, AND they carry an INTRA-workspace authz dimension
  (author-only edit; team membership) governed by ADR-003's boundary clause, not
  the cross-tenant matrix. Adding them exercises the identical seam without a new
  oracle class. Accepted as covered-by-shared-seam.
- **R2 — timing is asserted structurally, not by wall-clock.** ADR-003's
  same-query timing equivalence is sound for the scoped lookup; a future change
  adding a data-dependent branch AFTER the lookup could reintroduce a timing
  oracle, but the structural body-identity assertion catches that class (a
  divergent post-lookup branch diverges the body). A flaky layer-3 wall-clock
  scenario is deliberately NOT authored (US-06 + slices 2-3 document the
  `spawn_blocking` contention that makes such timing unreliable under `@all`).

## Scope confirmation

**SLICE 4 ONLY.** Cross-tenant non-enumerability HARDENING + the adversarial
matrix across ALL surfaces (US-MWT05): web reads (board, issue detail, attachment
download), web writes (file-issue, comment, state-change, attachment upload), web
admin action (revoke), API reads (issue list), API writes (state PATCH, comment),
API token revoke — each asserting foreign-id ≡ never-existed-id with no
status/body/shape oracle, plus the explicit no-403 oracle-hunt. Migration-as-
guarantee (Slice 5) and provisioning (Slice 6) are explicitly OUT — NOT authored
here. Slices 1-3 are NOT re-authored — their steps are reused; cells 1-3, 8, 11,
12 consolidate the slice-2/3 surfaces into the unified matrix for completeness /
regression.

## Upstream issues

**None.** The completeness audit found NO existence oracle on any tenant-scoped
surface: every "GAP" is a surface whose uniform-404 was implied by a shared
shipped scoping idiom and is now asserted directly by a matrix cell; no surface
exposes a 403-vs-404, body-echo, or shape difference for cross-tenant access. No
`distill/slice-04-upstream-issues.md` is written. (If a DELIVER RED surfaces a
real oracle on any cell, that finding + the offending surface + the recommended
fix are authored into that file at DELIVER time.)
