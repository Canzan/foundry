# ADR-API-OPENAPI-001: The OpenAPI document is a hand-written committed file, served as compiled-in bytes, and the code is checked against it

## Status

Accepted (2026-10-18). Proposed by the api-openapi-document DESIGN wave. The user chose option
(b), a hand-written document checked against the code, as recommended (OQ-U1).

The user also decided that non-GET methods on `/api/v1/openapi.json` answer the uniform 404
envelope through a method fallback scoped to that route, rather than axum's 405. The route is
`get(openapi_document).fallback(api_not_found_handler)` (feature-delta DDD-4, Changed
Assumption 1).

## Context

D1 asks for `GET /api/v1/openapi.json`, unauthenticated, whose bytes equal a committed file. D3 asks
for a CI failure when the document drifts from the code in route, method, path parameters, status
codes, request and response shapes, or the error envelope. DISCUSS left the mechanism to DESIGN:
generated from the code, hand-written with a conformance check, or a mix.

Code facts that decide it (all re-read 2026-10-18):

- **The wire structs are already adapter-local.** `IssueJson`, `CreateIssueRequest`,
  `PatchIssueRequest`, `CommentBodyRequest`, `CommentJson`, `IssueHistoryJson`, `TokenJson`,
  `ErrorBody` and `ErrorDetail` live in `crates/foundry-api/src/lib.rs:62-208`. foundry-services
  returns neutral types (`BoardIssue`, `CommentView`, `TokenView`). No option leaks serde or wire
  concerns into foundry-core or foundry-services.
- **Statuses are not visible in any type.** Handlers return `Result<_, ApiError>`. `ApiError` wraps
  the flat `ServiceError` (`foundry-services/src/lib.rs:541`), mapped once by `status_for`
  (`foundry-api/src/lib.rs:214`). The 401 comes from the `MachinePrincipal` extractor, the 429 from
  `rate_limited_response`, and the 400/413/415/422 text/plain answers from axum's `Path` and `Json`
  rejections. The 405 comes from the method router and the 404 from the `/api/v1/{*rest}`
  catch-all. No generator, utoipa included, derives any of these. Each one must be declared by
  hand and checked against the code by some other means.
- **axum's router cannot be introspected.** `Router` exposes no route table, so a check of "every
  route is documented" must read the source (check-arch already parses `.route(` blocks for
  `check_api_no_mint_route`, `check_arch.rs:267`) or probe the router.
- **The release image builds from `crates/` only.** The Dockerfile copies `Cargo.*`, `crates` and
  `xtask` (`Dockerfile:53-55`) and nothing under `docs/`.
- **`deny.toml` bans `foundry-api` everywhere except `foundry-app`.** xtask therefore cannot link
  foundry-api to call a generator.
- **The crates are versioned together** (RELEASING.md step 3), so foundry-api's `CARGO_PKG_VERSION`
  equals the footer's `views::RELEASE_VERSION`.

Quality drivers, in order: integrity of the contract (D3), maintainability for a team of one, no
production dependency growth, and operational simplicity (no build step, no network at build time,
SC-4 and SC-7).

## Decision

1. **The document is data.** `crates/foundry-api/openapi.json` is hand-written OpenAPI 3.1.0
   (pretty-printed, two-space indent, trailing newline). Every byte of it is reviewed in a diff.
2. **The served bytes are the committed bytes by construction.** foundry-api compiles the file in
   with `include_bytes!` and answers `GET /api/v1/openapi.json` with exactly those bytes. Nothing is
   generated at startup or at build time. There is no second copy (SC-6).
3. **The code is checked against the document in three orthogonal layers** (Earned Trust
   principle 13). Each layer answers a question the others cannot:
   - **Structural, check-arch** (`xtask`, reads source and the JSON, no build):
     - `api-openapi-operations`: the (path, method) set registered in `foundry_api::routes()`
       equals the document's operations. Path parameter names and types come from each handler's
       `Path<(…)>` tuple. The request body type comes from `Json<T>`. The success status and body
       type come from the handler.
     - `api-openapi-statuses`: per operation, the statuses, body kinds and envelope codes the code
       can produce equal the documented ones (ADR-API-OPENAPI-002).
   - **Type-derived, foundry-api unit test:** each wire struct's JSON Schema, as derived by
     `schemars` from the struct and its serde attributes, equals its component schema in the
     document. The test catches a field that is added, removed, renamed, retyped or made nullable,
     and a change to the `Error` envelope. `schemars` is a **dev-dependency only**, derived through
     `#[cfg_attr(test, derive(schemars::JsonSchema))]`, so the production binary gains no crate.
   - **Behavioural, real router:**
     - A `crates/foundry-app/tests/openapi_conformance.rs` test runs on real Postgres
       (testcontainers, the `api_comment_scoping.rs` idiom) inside `cargo test --workspace`. It
       runs in `cargo xtask smoke` and `cargo xtask ci` when Docker is available, and fails loudly
       (it does not skip) without it. The authoritative gate is a local
       `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`. GitHub CI gives a check-arch verdict only:
       it never reaches these tests or the foundry-api unit tests (DEVOPS environment matrix).
       It checks:
       - the served bytes equal the file, with any credential or none (D8);
       - every documented operation answers 401 without a token;
       - other methods on each path answer 405 with `Allow`;
       - an unrouted path answers the 404 envelope;
       - with a real token, the text/plain 400, 413, 415 and 422 answers carry the content type
         the document declares.
     - The acceptance suite (US-AOD-03/04) produces each deterministic domain status with real
       fixtures and validates the bodies against the document.
4. **"A standard validator accepts it"** means the document validates against the official OAS 3.1
   JSON Schema, iteration `https://spec.openapis.org/oas/3.1/schema/2025-11-23`. The schema is
   vendored as a test fixture in `crates/foundry-api/tests/fixtures/`, with its sha256 pinned in the
   test. The validator is the `jsonschema` crate (MIT, 0.59.x, draft 2020-12) as a dev-dependency
   with default features off, so it makes no network fetch. Schema Objects are not validated against
   the OAS dialect here, because the type-derived layer already compares them with `schemars`
   output.
5. **`info.version` is a literal in the file.** A unit test asserts it equals foundry-api's
   `CARGO_PKG_VERSION`. On failure the test names both values and the refresh command. The one
   refresh command is **`cargo xtask openapi`**: it rewrites `info.version` to the version in
   `crates/foundry-api/Cargo.toml` and leaves every other byte unchanged.

   **`cargo xtask openapi --check`** (added by DEVOPS, R2) is the read-only mode. It reads the
   `Cargo.toml` version and `info.version`, writes nothing, exits 0 when they are equal, and
   otherwise exits non-zero, naming both values and the fix (`cargo xtask openapi`). The file's
   bytes are unchanged after a failing run. These two modes are the command's whole surface.

   **One signature everywhere:** `cargo xtask openapi [--check] [--root <dir>]`. `--root` is
   optional. Like `check-arch --root` (`xtask/src/check_arch.rs` `parse_root`), it defaults to the
   workspace root inferred from xtask's `CARGO_MANIFEST_DIR`. The Dockerfile builder (`WORKDIR
   /work`, the copied repo root) runs it without `--root`. Tests on a staged copy pass `--root`.
   (Amended 2026-10-19. The first text said rewriting was "the only thing the command does".)
6. **Release flow** (amended 2026-10-19 with what DEVOPS verified):
   - **Step 1** becomes `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`. With Docker forced on, the
     acceptance and browser lanes cannot be skipped silently.
   - **Step 3** gains `cargo xtask openapi`, then `cargo xtask smoke`, after the
     `crates/*/Cargo.toml` bump. Step 4's `git add` gains `crates/foundry-api/openapi.json`.
   - **Why step 3 is needed.** No pipeline after the commit runs the version test:
     - `release.yml` (GHCR) and Forgejo's `build-and-publish.yml` (zot, which deploys) run no tests;
     - a tag alone builds the image and rolls it to dev;
     - GitHub CI never reaches the foundry-api unit tests.

     A stale `info.version` therefore reaches the tag, the image and dev unless the release commit
     catches it.
   - **The guards are two:**
     - the release-commit `smoke` run (RELEASING step 3, gate R1);
     - the Dockerfile builder gate. The builder stage runs `cargo xtask openapi --check` before
       building `foundry` (gate R2, a DELIVER edit), so no image builds from a stale document.

     **Until R2 lands, the guard is local discipline only.**

## Consequences

- Positive: every drift class D3 names has a mechanical check that runs in `cargo xtask smoke` and
  `cargo xtask ci` when Docker is available. Without Docker, those runs fail loudly; GitHub CI
  gives a check-arch verdict only. Statuses are covered, which no generator offers. Coverage per class, with its
  limits, is the table in the feature-delta DESIGN Quality Attributes. The one known gap is a status
  produced with no lexical trace (inside a macro expansion), which only the acceptance suite sees.
  Response headers are not a D3 class; the two the document promises (`Location`, `Allow`) are
  checked by the conformance test.
- Positive: zero new production crates. The new crates are dev-only: `schemars` (MIT) and
  `jsonschema` (MIT), plus the `schemars_derive` proc-macro. xtask gains an edge to `serde_json`, a
  workspace crate already in the graph. Both licences are on `deny.toml`'s allow list.
- Positive: axum's text/plain refusals, the empty 405, the catch-all and the open `state` string
  are written exactly as they behave, with no generator workaround (C3, C4, C6).
- Positive: the bytes change only when a person changes them, so a dependency bump never reformats
  the contract.
- Negative: a shape change is edited twice, once in Rust and once in the JSON. The check names the
  operation, status and field to edit. The API has changed rarely (8 operations since 2026-06), so
  the cost is small and visible.
- Negative: the structural layer is lexical. It reads source text, so a sufficiently unusual
  refactor (a handler built by a macro, routes assembled in a loop) can blind it. The behavioural
  layer is the backstop. Every structural rule ships with injected-violation gold tests (the
  ADR-CANZAN-THEME-003 convention).
- Negative: the release commit has one more step. If the step is forgotten but `smoke` runs, it
  fails locally, before the tag. If both are skipped, only the Dockerfile `--check` (R2) stops the
  image. Before R2 lands, nothing does: the publish workflows run no tests, and a tag deploys to
  dev. (Amended 2026-10-19 after DEVOPS. The first text said such a tag "is caught only by main's
  CI after the tag is pushed", which is not true.)
- Negative, mitigated: a Dockerfile line is not exercised by `smoke` or `ci`, so R2 could be
  dropped or reordered silently. DELIVER adds an xtask unit test asserting that the builder stage
  runs `openapi --check` before `cargo build … --bin foundry`. It lands with the Dockerfile edit
  (slice 01 step 7, DISTILL roadmap step 01-05, with R2).

## Alternatives considered

- **A. utoipa 6.0 (MIT/Apache-2.0, 3.1 by default, utoipa-axum 0.3 on axum ^0.8.4): derive
  `ToSchema` on the wire structs, `#[utoipa::path]` on the handlers, and generate the file.**
  Rejected.
  - It derives shapes only. Statuses stay hand-declared inside macro annotations, so a status
    check is still needed, and check-arch would have to parse utoipa's macro DSL instead of JSON.
  - Using utoipa-axum to single-source the routes would move the methods into attributes and blind
    `check_api_no_mint_route`'s `.route(` block parser (SC-2 forbids weakening it). Without
    utoipa-axum, the route check is still needed.
  - utoipa and utoipa-gen become production dependencies.
  - Generation needs a foundry-api binary or test that xtask cannot link (deny ban), and a utoipa
    bump can reformat the bytes.
  - `utoipa-swagger-ui` downloads the swagger-ui zip at build time, which SC-4 forbids. It would
    not be used.
- **B. Hybrid: hand-written paths plus `schemars`-generated component schemas, merged by a
  generator.** Rejected. It keeps every check of the chosen design and adds a generator and a
  merge step. The merge must run in a foundry-api binary or test, because xtask cannot link
  foundry-api. Comparing `schemars` output to the committed components gives the same guarantee
  with no generator.
- **C. Hand-written file checked only by the acceptance suite's real responses.** Rejected as the
  only net:
  - it runs in the Docker-gated lane that `xtask ci` skips silently (memory: xtask ci skips the
    browser lane), which breaks US-AOD-02's last AC;
  - response validation cannot see a request field the code stopped reading;
  - it cannot see a status no scenario triggers.
  It is kept as the behavioural complement.
- **D. Committed copy at `docs/api/openapi.json`.** Rejected. The image build does not copy
  `docs/`, so `include_bytes!` would break the release build, and the crate would reach outside
  its own directory. The two api-contract.md amendments and brief.md point readers at the file and
  at the served URL instead.
- **E. Generate at runtime and test that the result equals the file.** Rejected. Two copies with a
  test between them are weaker than one copy. `include_bytes!` makes served = committed true by
  construction (OQ-6).
