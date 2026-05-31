# Programmatic Foundry (Feature A) — Boundary Guard Design (US-W06)

Owner: solution-architect (Morgan). Scope: US-W06 `@infrastructure` — make the web/api boundary a
CI fact, not a review chore. **The crate split was RATIFIED 2026-05-31 (ADR-W01), which upgrades the
dependency-direction invariant from an AST convention to a Cargo-graph fact.** Three invariants to
enforce (NFR-WEB-BND-01/02):
1. **api≠HTML** — no handler in the `foundry-api` crate returns an HTML body.
2. **dependency direction** — the adapter crates (`foundry-api`, `foundry-app`) MUST NOT depend on
   `foundry-store` directly (they go through `foundry-services`); `foundry-services` MUST NOT depend
   on either adapter crate. *(Now crate-graph-enforceable — the override's payoff.)*
3. **web≠DB** — no `foundry-web` handler depends on the DB pool / runs SQL. *(This half only bites
   once Feature B introduces `foundry-web`; specified now, enforced when the target exists.)*

This embodies Core Principle 11 (enforceable architecture rules) and Principle 12c (a probe that
verifies the guard actually bites). Ratification record in `wave-decisions.md` (ADR-W06).

## Why the topology choice shapes the guard

The ratified crate split (ADR-W01) puts the JSON tier in its own crate (`foundry-api`). This makes
the **dependency-direction** invariant a crate-dependency-graph fact that `cargo-deny` can forbid
directly (e.g. `foundry-api` depending on `foundry-store` is a banned edge) — the user's stated
reason for choosing the crate split over the module option. **However, api≠HTML still needs an
AST-level check**: the crate boundary does not stop the `foundry-api` crate from importing
`axum::response::Html` and constructing an HTML body, since `foundry-api` legitimately depends on
`axum`. So the guard reasons at TWO granularities — crate-graph (dep direction) AND AST/symbol
(no `Html` inside `foundry-api`). This is the same reasoning Principle 12c records for `import-linter`
("import-graph only, no API for method/usage enforcement") — the crate graph answers "who may depend
on whom," the AST answers "what may a given crate's source construct."

## Guard mechanism (RATIFIED)

| Option | Catches api→HTML? | Catches dep-direction? | Catches web→DB? | New tooling | Fit with repo |
|---|---|---|---|---|---|
| `cargo-deny bans` only | No (a JSON crate may still construct `Html`) | **Yes** (crate-graph edge) | Partial (crate granularity) | none (already in CI) | Present; now catches dep-direction outright, blind to in-crate HTML construction |
| clippy custom lint (dylint) | Yes, but | Yes | Yes | NEW (`dylint` driver + a lint crate) | Heavyweight for a small rule set |
| **`xtask check-arch` AST walk + `cargo-deny bans` crate-graph rule** (RATIFIED) | **Yes** (AST: forbid `Html`/`text/html` constructions in the `foundry-api` crate) | **Yes** (`cargo-deny bans`: forbid `foundry-api`/`foundry-app` → `foundry-store`, and `foundry-services` → adapters) | **Yes** (`cargo-deny`: forbid `sqlx` in `foundry-web`'s tree; AST backstop) | reuses the EXISTING `xtask` + `cargo-deny` | Native — plugs into `xtask ci` + CI `lint-format` |

**Ratified: a custom `xtask check-arch` AST walk for api≠HTML, plus a real `cargo-deny bans`
crate-graph rule for dependency-direction (the crate split made this enforceable), wired into the
existing `xtask ci` and CI `lint-format` job.** The repo already has an `xtask` crate whose header
anticipates this subcommand (`xtask/src/main.rs:8`), and `cargo-deny` is already a CI job
(`.github/workflows/ci.yml` `deny`; `deny.toml` present). No new CI infrastructure, no new dependency
class. A `dylint` custom lint was rejected as disproportionate; `cargo-deny` alone is now a
first-class layer (dep-direction) but cannot see in-crate `Html` construction, so the AST layer
remains for api≠HTML.

## The three orthogonal layers (Principle 12c)

The guard is enforced by three semantically distinct checks; a single-layer bypass is caught by at
least one of the other two.

1. **Structural (AST/source walk) — `xtask check-arch`.** Walks the source tree and asserts:
   - **api≠HTML**: no symbol in the `foundry-api` crate (`crates/foundry-api/src/**`) constructs
     `axum::response::Html`, returns `Html<…>`, or sets a `text/html` content-type. (A JSON string
     field that contains sanitized HTML — e.g. `body_html` — is *not* an HTML response body and is
     explicitly allowed; the rule targets response-body/content-type construction, not string
     contents — see `api-contract.md`.)
   - **api≠ad-hoc-authz**: no `is_team_member`/`is_workspace_admin` call site appears in the
     `foundry-api` crate (authorization belongs in `foundry-services`, NFR-WEB-API-SEC-02).
   - **api≠JWT-footgun (NEW, JWT-specific)**: the JWT `Validation` in the token-auth extractor pins
     `algorithms = [EdDSA]`; the AST check asserts no `Validation` in `foundry-api`/`foundry-auth`
     permits a non-EdDSA algorithm or disables signature validation (closes the alg-confusion /
     `insecure_disable_signature_validation` footgun structurally, not just by test).
   - **web≠DB** (Feature B activation): no `sqlx::`/`PgPool`/pool-handle symbol in the (future)
     `foundry-web` crate. No target until it exists; flips on automatically.
2. **Subtype/graph — `cargo-deny bans` (now a FIRST-CLASS dependency-direction layer).** `bans`
   entries assert the crate-graph invariants the split made enforceable:
   - forbid `foundry-api` → `foundry-store` and `foundry-app` → `foundry-store` (adapters must go
     through `foundry-services`);
   - forbid `foundry-services` → `foundry-api` and `foundry-services` → `foundry-app` (no reversed
     edge);
   - (Feature B) forbid `sqlx` in `foundry-web`'s dependency tree.
   This is the layer the ratified crate split upgraded from "AST hope" to "Cargo fact."
3. **Behavioral (CI gold test) — injected-violation probe.** A scheduled CI lane checks out a
   throwaway branch that deliberately introduces violations — (a) an `Html("…")` return inside a
   `foundry-api` handler, and (b) a `foundry-api → foundry-store` dependency edge in
   `crates/foundry-api/Cargo.toml` — and asserts the guard **fails** on each (the AST layer on (a),
   `cargo-deny` on (b)). This proves the guard *bites* across BOTH layers, satisfying US-W06's
   "injected-violation test in CI proves it bites" and Principle 12's self-application.

Each layer answers a different question: (1) "does the `foundry-api` source construct HTML or pin a
bad JWT alg?" (2) "does the crate graph allow a forbidden dependency edge?" (3) "if either kind of
violation existed, would the guard catch it?"

## Wiring (no new CI infrastructure)

- Add `("xtask check-arch", vec!["run", "-p", "xtask", "--", "check-arch"])` (or a direct invocation)
  to the `xtask ci` step list (`xtask/src/main.rs` `run_ci`), positioned alongside `cargo fmt
  --check` / `cargo clippy` so a developer's local `cargo xtask ci` catches violations before push.
- Add a `cargo xtask check-arch` step to the CI **`lint-format`** job (cheap, no DB) so a violating
  PR goes red before merge (US-W06 AC: "runs in the existing CI lane; a clean boundary passes
  without manual steps").
- Extend the `bans` section of `deny.toml` with the dependency-direction rules (the api≠store and
  no-reversed-edge bans are **active from Feature A**; the `foundry-web` `sqlx` ban activates with
  Feature B).
- Add the injected-violation gold test as a separate scheduled workflow job (or a `check-arch
  --selftest` mode the `lint-format` job runs) so the guard's own correctness — across BOTH the AST
  and `cargo-deny` layers — is continuously proven.

## Acceptance mapping (US-W06)

| AC | Mechanism |
|---|---|
| CI fails when an adapter crate depends on the DB pool directly | **Layer 2 (`cargo-deny bans`): `foundry-api`/`foundry-app` → `foundry-store` is banned — active from Feature A** (the crate-split payoff). The future `foundry-web` `sqlx` ban activates with Feature B. |
| CI fails when a `foundry-api` handler returns an HTML body | Layer 1 (`xtask check-arch` AST: no `Html`/`text/html` in `crates/foundry-api/src/**`) — active from Feature A slice 1 |
| CI fails when the JWT verifier permits a non-EdDSA alg / disables signature validation | Layer 1 (AST: `Validation` must pin `[EdDSA]`) — active from Feature A (JWT-specific) |
| Guard runs in the existing CI lane; clean boundary passes without manual steps | Wired into `lint-format` + `xtask ci`; no new job required for the core check |
| Injected-violation test proves it bites | Layer 3 (scheduled gold test — injects an `Html` return AND a banned dep edge) |

## Scope notes

- api≠HTML and the **dependency-direction crate-graph rule** are enforceable and meaningful **from
  Feature A's first slice** (the `foundry-api` and `foundry-services` crates exist). The web≠DB half
  is **specified now but only bites when Feature B creates `foundry-web`** — exactly as DISCUSS
  sequenced it (`stories.md` US-W06 dependencies).
- The guard is intentionally small (the three invariants above plus the JWT-alg structural check).
  It is NOT a general architecture-fitness DSL; if more rules are needed later, `check-arch` grows a
  rule list — but Feature A ships only what the NFRs require (Principle 8).
