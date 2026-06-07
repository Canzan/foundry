# No-Mint Boundary — keeping MINT off the API (DESIGN)

> Ratified Q-AUTHZ option (c): a machine-token bearer may LIST + REVOKE, but **NEVER MINT**. This
> document specifies how that invariant is held — and made *enforceable*, not merely conventional.

## The risk this defends (recap)

A bearer that can MINT is a credential printing press: a single leaked token mints N fresh
admin-bound children, each independent, so revoking the leaked one does nothing (the mint loop /
self-replication — `wave-decisions.md` escalation table). The defense is to remove the mint surface
from the bearer channel entirely. Mint stays human-session-only (`/admin/tokens`, session+CSRF).

## Two layers of "no mint"

### Layer A — no route (the cleanest defense; the default)

The simplest, strongest control is **structural absence**: the `routes<S>()` router adds ONLY
`GET .../tokens` and `DELETE .../tokens/{jti}`. There is **no `POST .../tokens` route** and no
shared handler that could mint. A `POST` to `…/tokens` therefore returns axum's method-not-allowed
(**405**) or not-found (**404**) by construction — there is no code path from a bearer to
`Services::mint_token`. US-TMA05 asserts this as a route-surface scenario.

This is sufficient on its own for correctness today. But absence is fragile to *future* edits: a
later contributor adding token routes could innocently wire `POST .../tokens -> mint_token`
(`Services::mint_token` already EXISTS — it is reachable, just not routed). Convention does not stop
that; a guard does.

### Layer B — a `check-arch` no-mint guard rule (PROPOSED — recommend ADOPT)

> **Decision proposed: ADD a LAYER-1d AST rule to `xtask/src/check_arch.rs` asserting no MINT route
> is exposed on `/api/v1`.** Awaiting ratification.

`check_arch.rs` already AST-walks `crates/foundry-api/src` for three orthogonal rules (api≠HTML,
api≠ad-hoc-authz, JWT alg pin). A no-mint rule is a natural fourth detector (`check_api_no_mint_route`)
in the SAME source-walk, costing one function + one test pair. It answers a question none of the
existing rules do: *"does the bearer adapter expose a mint surface?"*

**What it asserts (proposed detector logic):**

- In `foundry-api/src`, **no line both names a token route literal and calls/registers `mint`**.
  Concretely, flag any source line that contains `mint_token` (a call to the mint use-case from the
  JSON adapter) — `Services::mint_token` must never appear in `foundry-api`. This is the precise,
  low-false-positive signal: the adapter cannot mint without naming `mint_token`, and today it never
  does. (Mirrors the existing `is_workspace_admin`/`is_team_member` substring detector in
  `check_api_no_adhoc_authz` — same shape, same robustness, same `strip_comment` handling so doc
  prose mentioning "mint" is not flagged.)
- Optionally also flag a `post(` registration on a `…/tokens` route literal in `foundry-api`, to
  catch a mint route wired to a *differently-named* handler. The `mint_token` substring rule is the
  load-bearing one; the `post(.../tokens)` check is a belt-and-braces sibling.

**Why this is the right enforcement (Principle 11 + 12):** the no-mint property IS an architectural
rule, and "architecture rules without enforcement erode." A leaked-token printing press is precisely
the kind of catastrophic regression a single careless edit could reintroduce. The guard turns the
ratified authz decision into a build-time invariant — a future edit that re-adds a bearer mint path
**cannot merge green**. It is the cheapest possible insurance against the highest-impact risk in the
feature.

**Self-application (Principle 12c):** the existing boundary-guard already has a gold-test that drives
the guard binary against a planted-violation tree (`check-arch --root <copy>`). The no-mint rule
inherits that harness: plant a `foundry-api` source line that calls `Services::mint_token` (or
registers `post(.../tokens)`), assert the guard NAMES it and exits non-zero — proving the guard
actually bites, not just claims to.

## Why NOT a 403 shared handler

An alternative is a single `POST .../tokens` handler that always returns 403 (signalling "mint is
disabled here"). **Rejected:** it (a) adds a real code path toward the mint use-case that a future
edit could "complete", (b) is an existence oracle — a 403 confirms a mint surface conceptually
exists, whereas 404/405 from absence reveals nothing, and (c) is strictly more code than no route.
Structural absence (Layer A) + the guard (Layer B) is simpler AND safer.

## Verify

- **US-TMA05 route-surface scenario** — `POST /api/v1/.../tokens` returns 404/405 (no mint route).
- **`cargo xtask check-arch`** (with the proposed rule) — fails if `mint_token` appears in
  `foundry-api` or a `post(.../tokens)` route is registered there.
- **Gold test** — planted mint violation makes the guard exit non-zero and name the file/line.
- **`Services::mint_token` stays callable from `/admin/tokens` only** — the human channel is
  unaffected; mint is not removed from the system, only kept off the bearer surface.
</content>
