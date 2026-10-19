# RED classification — api-openapi-document

Gate output of the pre-DELIVER *fail-for-the-right-reason* check (ADR-025 scaffolded RED, rigor
profile `adr-025-scaffolded-red`). DELIVER reads this file at the start of each step to confirm the
RED it un-pends is genuine.

## Acceptance (`@api-openapi-document`, four feature files)

**Run** (2026-10-19): `foundry` built and warmed (`cargo build -p foundry-app --bin foundry`, then
`target/debug/foundry --help` three times; no scenario of this feature runs the binary, the HTTP
lane is in-process and the drift lane runs `xtask` and `cargo`). `@pending` was stripped from the
four `api-openapi-document-0*.feature` files only (a `perl` over the working files, byte-exact
copies kept in the scratchpad), then
`FOUNDRY_ACCEPTANCE_TAGS=api-openapi-document timeout 3000 cargo test -p foundry-acceptance --test acceptance`.
An EXIT trap restored the files byte for byte (`cmp` clean on every run). Docker was up, so the
`@needs-browser` scenarios ran in the real browser lane (`selenium/standalone-chrome`), not only
the HTTP lane. No canzan-lift port collision was seen (its containers were running throughout).

**Result** (re-run 2026-10-19 after the round-2 review fixes, all four files lifted together): 4
features, **53 scenario declarations, 163 rows** with outline rows expanded; **163 RED
(MISSING_FUNCTIONALITY), 0 GREEN_ALREADY, 0 BROKEN**; 0 parsing errors, 0 hook errors, 0 undefined
or ambiguous steps (the lane runs `fail_on_skipped`). 993 of 1 156 steps passed before each row's
first missing behaviour, including every new `Given <precondition>` (AUTH-7 moved, the review lane,
the four comment setups, the spent revoke budget) and the page's expired and forged tokens.

The four rows added by the review: `create an issue in a way the address does not offer` (405) and
`read the API description in a way the address does not offer` (404), both passing their status
assertion and stopping at the missing description; the page's expired and forged tokens (401),
stopping at the missing page.

**Verdict: PASS.** Zero BROKEN, zero false GREEN.

### Where each row stops

| Stops at | Rows | Why it is the missing functionality |
|---|---|---|
| "the API description is not served at /api/v1/openapi.json" (404, the catch-all) | 129 | No route serves the description (DDD-3/4). Every reading scenario, every matrix row, the D7/D8/two-instance rows and every "still served" control stop here |
| "the API page never showed its operations" (`/docs/api` is the uniform not-found page) | 15 | No page (DDD-15/16) |
| "there is no committed API description at …/crates/foundry-api/openapi.json" | 6 | The committed file is DELIVER's (DDD-2): the reword, un-document, bump (×2 Givens), refresh and `--check` plants need it |
| "the drift check passed over a planted drift" | 5 | No rule and no unit test exist, so both commands pass on any tree: field (×2), 409, new operation, removed operation |
| "the drift check does not report that it ran api-openapi-operations" | 1 | The clean-tree control: the PASSED line names no description rule yet |
| The page's HTTP-lane checks (no CSP header, `/docs/api` and the vendored files 404, no `<noscript>` copy, no framing refusal, no load-failure message, not the API page) | 7 | Same: page, policy layer and vendored files absent |

### Matrix rows proved their statuses against today's code

The matrix Then asserts the real status **before** it reads the description. All 73 matrix rows
(slices 01 and 02) passed that assertion and stopped only at the description: every row's status in
the `.feature` file is what the shipped router answers today, including 413 (a body over 2 MB),
415, the text/plain 400 and 422, 405 on every path, 403 (member token; another author's comment),
410 (deleted comment), 404 (another issue's comment; scoped token; unknown project) and 429 (the
twenty-first revoke inside the guardrail window). That is DISCUSS's slice-02 learning hypothesis
("the code's answers can be listed completely") holding for every deterministic answer before a line
of the document is written.

### Unverified at DISTILL: the universe-delta Thens

The Mandate 8 oracles of scenarios 9, 10 and 33-36 ("nothing in Canzan Labs changed", "Canzan Labs
changed only as that answer reports") **never ran**: every one of those rows stops one step earlier,
at "the API description is not served". They cannot be exercised before the description exists, so
they are **unverified at DISTILL and first exercised in DELIVER** (steps 01-01 and 02-01/02-02). If a
success row's declared delta is wrong (for example, a create also writing a row the universe
includes), that is a test fix in DELIVER, not a product finding. The same holds for the matrix
oracle's body judgement against a real description; the oracle itself (`jsonschema`, draft 2020-12,
components in the root resource) is proven on a fixture by `oracle_self_check` (2 tests, green).

### Defects found and fixed during classification (not in the final run)

- **BROKEN, fixed:** the outline column `a write` (a space in a placeholder name) was not
  substituted, so "Sven sends <a write> …" matched no step (4 rows). Renamed to `write`.
- **Wrong-reason failure, fixed:** the drift lane's staged tree first copied only the crates and
  manifests, so check-arch's shipped `publish-stamp` rule failed on the missing `Dockerfile`,
  `.github` and `.forgejo`. The "drift check fails" rows were therefore failing for the wrong reason.
  The tree now copies the whole repository minus `target`, `.git` and tool caches; the drift rows
  now pass the check on today's tree and are RED on "passed over a planted drift".
- **False GREEN, fixed:** "The API page loads nothing from another site" passed over the 404 page
  (a missing page requests nothing from anywhere). The own-origin oracle now also requires the
  page's own files and the description among the requests (a control); the row is RED.

### Defects fixed after the round-2 review (Sentinel)

- Hidden setup moved out of the When into explicit `Given <precondition>` rows (H3); the When is one
  request and snapshots after setup.
- "scratch-rotation is revoked" is judged by listing tokens over HTTP, not by SQL (H4).
- The drift lock is held for the whole scenario (H5).
- The token-value check scans the whole Token schema and both token operations for every minted JWT
  and the `eyJ` prefix (H6).
- The matrix oracle is `jsonschema` 0.59, as DESIGN locks; the subset checker is retired.

### Harness facts observed in the browser lane

- chromedriver's `goog/cdp/execute` endpoint is reachable through the Selenium router:
  `Network.setBlockedURLs` was accepted, so "the description cannot be loaded" is mechanically sound.
- The browser log capability records the 404 of `/docs/api` ("Failed to load resource"), so the
  content-policy and framing oracles read a live recorder, not an empty one.
- Not yet observable (no page exists): swagger-ui's selectors, the Authorize plugin's label, the
  `curl` block and the `?url=` / `?configUrl=` behaviour. These stop at the page and are first
  exercised at slice 03; a selector mismatch then is a harness fix, not a product finding.

## Non-acceptance seams (pending panic stubs, project-lifecycle `project_lifecycle_copy.rs` idiom)

| File | Tests | Ignored now | `--ignored` |
|---|---|---|---|
| `xtask/src/check_arch.rs` `api_openapi_check_arch_tests` | 41 (rule 1: 12, rule 2: 14 + the AC 8 `ci_and_smoke_run_the_drift_check`, `xtask openapi`/`--check`: 5, rule 3: 9) | 41 | 41 RED (scaffold panic), 0 BROKEN |
| `crates/foundry-app/tests/openapi_conformance.rs` | 8 | 8 | 8 RED |
| `crates/foundry-app/tests/api_docs_page.rs` | 5 | 5 | 5 RED |

Each stub's `#[ignore = "pending: api-openapi-document NN-NN — …"]` names its DELIVER step. The
foundry-api unit tests (`src/openapi.rs`) are not stubbed: their module does not exist and they need
foundry-api's own `schemars`/`jsonschema` dev-dependencies, which DDD-12a gates to DELIVER (see
feature-delta DISTILL, Test placement).

## Elevator pitch → scenario names (verb continuity, deliberate waiver)

The pitches name URLs and statuses on purpose (an HTTP API whose integrator branches on them; the
token-management-api precedent). The scenario names carry the same user-valued outcome:

| Story | Elevator pitch "After" (essence) | Scenario names carrying it |
|---|---|---|
| US-AOD-01 | `curl …/api/v1/openapi.json` with no token returns the 3.1 description, the release version, `/` as server | An integrator downloads the API description from the instance without a token (1); The description an instance serves is the one committed in the repository (2); A bad or stale credential does not change the answer (3) |
| US-AOD-02 | adding `priority` makes `cargo xtask ci` fail naming the operation, status and field | A field the description lacks fails the build (12); An answer the description does not list fails the build (13); An operation missing from the description fails the build (14) |
| US-AOD-03 | the description lists every issue and comment answer, text/plain refusals, an open `state` | Every operation is described with every answer it can give (22); Malformed requests are described as they really answer (28); An issue's state is described as one of its project's lanes (27); A refused issue or comment call answers the way the description says (34) |
| US-AOD-04 | token operations described, no create, the scheme names /admin/tokens | The description offers no way to create a token, and says where one comes from (31); The place the description sends people to mint a token exists (32); A refused token call answers the way the description says (36) |
| US-AOD-05 | `/docs/api` shows "Foundry API v…", every operation, nothing from another origin | An integrator reads every API operation in a page on their own instance (37); The API page loads nothing from another site (38) |
| US-AOD-06 | paste the token, Try it out → 200 with AUTH-7 and the `curl` line; reload forgets it | An integrator lists a project's issues from the API page with a pasted token (46); A pasted token is forgotten on reload and never stored (51) |

## Burn-down

- `grep -c '^\s*@pending' crates/foundry-acceptance/tests/features/api-openapi-document-0*.feature`
  = 10 + 11 + 15 + 17 = 53 now (served, drift, every-answer, page), 0 at the end.
- `grep -c '^    pending_gold!(' xtask/src/check_arch.rs` = 41, and
  `grep -c 'pending: api-openapi-document' crates/foundry-app/tests/{openapi_conformance,api_docs_page}.rs`
  = 8 + 5: 54 stubs now, 0 at the end (the `pending_gold!` macro is deleted with the last one).
- Every `aod-0N` lane run DELIVER reports must show a non-zero scenario count: an all-`@pending`
  slice selects 0 scenarios and exits green.
