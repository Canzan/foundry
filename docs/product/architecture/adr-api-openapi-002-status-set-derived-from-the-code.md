# ADR-API-OPENAPI-002: Each operation's status set is derived from the source by a conservative check-arch rule

## Status

Accepted (2026-10-18). Proposed by the api-openapi-document DESIGN wave. The user chose the lexical
status rule read from the code (OQ-U2 (a)), as recommended, over behavioural-only checking.

The user's 404 decision for non-GET methods on `/api/v1/openapi.json` adds two rows to the
transport table below:
- a route-scoped `.fallback(api_not_found_handler)` produces the 404 envelope instead of 405;
- the document route carries no 401 and no 500.

## Context

US-AOD-02 requires CI to fail when "an operation can answer a status the document does not list for
it, or the document lists one the operation cannot answer". Its scenario 2 plants a `409` on list
issues. DISCUSS named statuses as the walking skeleton's riskiest assumption. No Rust type carries
them:

- `ServiceError` is one flat enum (`foundry-services/src/lib.rs:541`), and every use-case returns
  `Result<_, ServiceError>`.
- `status_for` (`foundry-api/src/lib.rs:214-233`) maps the variants to `(status, code, message)`.
- The transport answers come from the handler's shape: the `MachinePrincipal` 401, axum's
  `Path`/`Json` rejections, the method router's 405, and the adapter-local 429
  (`rate_limited_response`, `:260`).

How variants arise in foundry-services today (grep, 2026-10-18):

- Every variant is constructed as a visible token (`ServiceError::NotFound`,
  `.map_err(|_| ServiceError::Internal)`, `ServiceError::Validation { code: "title_required"… }`).
- There is **no** `impl From<_> for ServiceError` and no `#[from]` on the enum.
- The `Services` facade methods each delegate to one free function (`board::list_board_issues`,
  `issues::create_issue`, `tokens::revoke_project_token`, …), which call further free functions in
  the same crate (`resolve_member_project`, `resolve_board_project`, `require_workspace_admin`,
  `resolve_token_address`, `validate_project_lane`, …).

The per-operation domain sets read from the code agree with DISCUSS's list in US-AOD-03's Technical
Notes. No `/api/v1` path reaches `Conflict`.

## Decision

Add check-arch rule **`api-openapi-statuses`**. For each `/api/v1` handler in
`crates/foundry-api/src`, it derives a status set, compares it with the document in both
directions, and names the operation, status and difference.

**Transport set, from the handler's signature and body:**

| Handler fact | Statuses and body kind |
|---|---|
| returns `Result<_, ApiError>` | 500 envelope `internal` |
| takes `MachinePrincipal` | 401 envelope `unauthorized` |
| `Path<(…)>` holds a non-`String` element (`i32`, `uuid::Uuid`) | 400 `text/plain` |
| takes `Json<T>` | 400, 413, 415, 422 `text/plain` |
| takes `State<Arc<dyn RevokeRateGuard>>` | 429 envelope `rate_limited` |
| its path registers fewer than all methods, with no method fallback | 405, no body, `Allow` header |
| its route chains `.fallback(api_not_found_handler)` (only `/api/v1/openapi.json`; user decision 2026-10-18) | 404 envelope `not_found` instead of 405 |
| the handler returns no `ApiError` and takes no extractor (the document route) | no 401 and no 500 |
| success | 200 by default; 201 or 204 when the body returns `StatusCode::CREATED` / `StatusCode::NO_CONTENT` (checked by `api-openapi-operations`) |

**Domain set, by conservative reachability.**

- Start at the `services.<method>(` call in the handler and resolve it to the `impl Services` method
  of that name.
- Walk the call graph lexically over `crates/foundry-services/src/**/*.rs`. Every `name(` or
  `module::name(` that names a crate `fn` is followed. Same-named functions are unioned, and a
  visited set stops cycles.
- Collect every `ServiceError::<Variant>` token, and every `code: "<literal>"` inside a
  `ServiceError::Validation { … }` (also through a helper fn that returns one), in each function
  reached.
- Map variants to `(status, code)` by **parsing `status_for`'s match arms**, so a remapping in
  `status_for` is caught too. `Validation` contributes one 422 envelope response whose codes are the
  collected literals.
- The domain set only grows: the walk never prunes branches.

**What "equals the document" means, per operation:**

- the set of response status keys is equal;
- each status's body kind is equal: `application/json` with `$ref: Error`, `text/plain`, or no
  content;
- for each envelope response, the keys of its `examples` map equal the derived `error.code` set.
  For a 422, that is the Validation codes; for the others, the one `status_for` or
  `rate_limited_response` code.

**Collector scope (iteration-2 review).** Tokens are collected from **both**
`crates/foundry-services/src` and `crates/foundry-api/src`:
- each operation handler's own body;
- the extractor path `MachinePrincipal::from_request_parts` → `token_auth::authenticate` /
  `verify_bearer`;
- every crate function those call.

A `ServiceError::Conflict` planted in a handler body, or a `Forbidden` planted in `token_auth`, is
therefore seen. The catch-all handler (`api_not_found_handler`, `lib.rs:349`) is the one excluded
body: it is not an operation (D15), and the document's shared `NotFound` describes it.

**Start set.** The walk starts at each handler's `services.<method>(` call **and** at the
extractor's Services calls: `MachinePrincipal` → `token_auth::authenticate` →
`Services::resolve_active_token`. Every variant reachable from the extractor is added to every
operation that takes `MachinePrincipal`. Today that set is `Unauthorized` only. A planted `Internal`
there would add 500 through the walk, not by assumption.

**Soundness clause, in the same rule. The rule fails closed: an unrecognised form is a
violation, never an empty contribution.** Each of these fails the build, naming the file and line:
- an `impl From<…> for ServiceError`, or a `#[from]` in `ServiceError`. A conversion would let a
  variant arise with no token for the walk to see;
- a handler or extractor call to a `Services` method the rule cannot resolve;
- inside a reached function:
  - **call resolution, defined (iteration-2 review):**
    - A call `name(`, `module::name(` or `.name(` is a crate call when a `fn name` is defined in
      `crates/foundry-services/src` or `crates/foundry-api/src`. It is followed, and every
      same-named definition is unioned.
    - A name with no crate definition (`map_err`, `ok_or`, `clone`, `into_iter`, …) is not a crate
      call and contributes nothing. That is sound, because no non-crate function can return a
      `ServiceError` without a `From` impl, which the first bullet forbids.
    - A call whose receiver chain begins at a `Store` value (`store.…(`, `self.store.…(`, including
      chained calls) is a store call. It is not followed, for the same reason: a store error
      reaches `ServiceError` only through an explicit `map_err`/`ok_or` token in the caller, and the
      walk collects that token;
  - a `ServiceError::Validation` whose `code` is not a string literal (shorthand `code,`, or
    `code: <expr>`), unless the expression is a call to a crate function that itself returns a
    literal-coded Validation (followed);
  - a `ServiceError` reference in any form other than `ServiceError::<Variant>`: a glob
    `use ServiceError::*`, an import alias, or `Self::<Variant>` inside the enum's impls;
- a `status_for` match arm with a guard, a wildcard `_`, or a body that is not a
  `(<int literal>, <string literal or the Validation code binding>, …)` tuple.

The walk is lexical by choice. xtask has no `syn` dependency, and every shipped check-arch rule is
lexical, so a parser would be a new dependency and a second idiom. Fail-closed is what makes the
lexical walk sound: anything it cannot read stops the build instead of passing silently.

**Reviewed exemptions.** Exemptions are a named list of `(operation, status, reason)` for statuses
the walk over-approximates. The list is empty at design time, and adding to it is a reviewed change.

**Not an operation:** `/api/v1/{*rest}` (D15). The rule skips the catch-all route and requires the
document's shared `NotFound` response to exist.

**Gold tests** (xtask unit tests on staged trees, ADR-CANZAN-THEME-003 convention), each naming the
offender:

- plant `return Err(ServiceError::Conflict)` in `resolve_board_project` → names list issues and
  `409`;
- remove `401` from one operation in the document → names it;
- add a Validation code `title_too_long` in `create_issue` → names create issue, `422` and the code;
- remap `Gone` to `404` in `status_for` → names edit comment and `410`;
- plant `impl From<StoreError> for ServiceError` → names the soundness clause;
- add a `Json<…>` parameter to the history handler → names history and the four `text/plain`
  statuses;
- fail-closed cases, each failing and naming the form: a non-literal Validation `code`; a
  `use ServiceError::*`; a guarded `status_for` arm; a handler calling a `Services` method that
  does not exist in the facade;
- a second `fn resolve_board_project` planted in another module, returning
  `ServiceError::Conflict` → names list issues and `409` (same-named definitions are unioned);
- `return Err(ApiError(ServiceError::Conflict))` planted in `list_issues_handler`'s body → names
  list issues and `409` (foundry-api is in the collector scope);
- `Err(ServiceError::Forbidden)` planted in `token_auth::authenticate` → names every bearer
  operation without a 403, and `403`;
- plant `return Err(ServiceError::Forbidden)` in `resolve_active_token` → names list issues (and
  every other bearer operation without a 403) and `403`. This variant is reachable only through the
  extractor start set. `Internal` would prove nothing here, because every `ApiError` handler
  already carries 500 as a transport status;
- a clean tree passes, and a wording-only `description` edit passes.

## Consequences

- Positive: US-AOD-02 scenario 2 fails deterministically in `smoke` and `ci`, with no database. The
  "lists one it cannot answer" direction is also checked.
- Positive: C9's messages and C7's shared `title_required` fall out of the same walk, because the
  codes come from the code.
- Negative: it is the largest new rule in this feature. It is estimated at 250-350 lines plus seven
  gold tests, about 1 day, and it is most of the slice-01 overrun against DISCUSS's 2-day estimate.
- Negative: it is lexical. A trait-object call, a closure-supplied error or a macro could produce a
  status it does not see. The fail-closed soundness clause turns every such form it can recognise
  (conversions, globs, aliases, non-literal codes, guarded arms, unresolved calls) into a build
  failure. What remains is a construct with no lexical trace at all, such as an error produced
  inside a macro expansion. The acceptance real-request check (US-AOD-03/04) is the behavioural
  backstop. The rule's own doc comment states the limit.
- Negative: over-approximation can force the document to list a status a branch can never reach
  for that operation. It surfaces as a reviewed exemption, never silently.

## Alternatives considered

- **A. Behavioural only: rely on the acceptance suite producing each status.** Rejected as the only
  net. A status nobody triggers is invisible, so scenario 2 (a new 409) passes unless someone writes
  a 409 scenario. The check also runs only in the Docker-gated lane. That is a named partial pass of
  D3. It is about 1 day cheaper, and the user may choose it (OQ-U2).
- **B. Per-operation error enums in foundry-services, with `IntoResponse` mapping checked
  exhaustively at compile time.** Rejected for this feature. It is the strongest option (Principle
  12: push the contract into the type system), but it narrows the return type of eight use-cases
  shared with the web tier, and the HTML handlers would have to adapt. That is a cross-crate
  refactor larger than this feature, and no behaviour changes for it. It is recorded as a follow-up.
- **C. A hand-maintained status table in foundry-api, checked against the document.** Rejected. It
  is a second hand-written copy of the same claim, and it checks the document against a list
  rather than against the code.

**Size of option B (estimated from the code, review M1).** The change would touch:
- the 8 use-case functions and their 8 `Services` facade methods;
- every helper they share that returns `ServiceError`: `resolve_member_project`,
  `resolve_member_issue`, `resolve_board_project`, `resolve_member_team`,
  `require_workspace_admin`, `resolve_token_address`, `validate_project_lane`;
- the web-tier handlers that call 5 of those use-cases (create issue, change state, create and
  edit comment, history) and match on `ServiceError` today.

That is about 25-30 signatures across foundry-services and foundry-app, and the HTML error mapping
would need a per-enum mapping too. Estimated at 3-4 days, against about 1 day for the rule.
