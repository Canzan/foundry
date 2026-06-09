# Slice 3 — Propagate isolation to the JSON /api/v1 + machine-token + sign-in/session surfaces

## Outcome
The isolation boundary holds for a machine-token bearer principal and a signed-in API caller,
including how a request picks its workspace: a token bound to A acts only on A, and a session
resolves to exactly one workspace, fail-closed.

## Learning hypothesis
**We believe** the SAME boundary proven on the web tier holds for a machine-token bearer
(workspace bound by `machine_tokens.workspace_id`) and for session resolution — **and we will
know we are right when** an Acme-bound token cannot touch Globex (refused non-enumerably), and a
session resolves to exactly one workspace (single-membership automatically, multi-membership
explicitly), refusing when none resolves.

## Riskiest assumption being validated
That a token's `workspace_id` is the authoritative acting workspace for `/api/v1` (a leaked token
is confined to its own tenant), and that session resolution yields exactly one workspace under
multi-membership. A token reaching another tenant is the highest-impact isolation failure.

## Stories
- **US-MWT03** — `/api/v1` + machine-token principals act only on the token's bound workspace;
  cross-tenant call refused non-enumerably.
- **US-MWT04** — sign-in/session resolution yields exactly one acting workspace, fail-closed;
  multi-membership selection is explicit.

## IN scope
- Resolve the acting workspace from the token binding (bearer) or session on `/api/v1`.
- Confine token list/revoke to the acting workspace; foreign jti = not-found.
- Single-membership auto-resolution; multi-membership explicit selection; fail-closed when none.
- Real Acme-bound token vs real Globex resources.

## OUT scope
- The full uniform non-enumerability matrix across all surfaces (Slice 4 hardens + proves).
- Provisioning / migration (Slices 5-6).

## Reuses (shipped — do not rebuild)
- The `MachinePrincipal` extractor, `is_workspace_admin`, the per-request `jti` denylist,
  `foundry_services::tokens` use-cases (workspace-scoped, 100%-mutation-hardened); the sign-in +
  tower-sessions machinery.

## Done when
- An Acme-bound token acts only on Acme; a Globex-targeting call is refused non-enumerably.
- Token list/revoke is confined to the acting workspace.
- A single-membership session auto-resolves; a multi-membership session resolves to one chosen
  workspace; an unresolvable session is refused.
- The shipped verify path / denylist / `iss`/`aud`/EdDSA pinning are unchanged.

## Learning hypothesis verdict shape
Confirms: token + session surfaces inherit the boundary → harden uniformly (Slice 4).
Disproves: if a token reaches another tenant or a session defaults silently → fix the resolution
contract before hardening.

## Open questions touching this slice
- **OD-2** multi-membership (drives whether a selection UX exists) — flag for user before DESIGN.

## Dependencies
- Slice 1 (resolution + coexistence); complements Slice 2 (same boundary, API surface).

## Effort estimate
~1-1.5 days (two surfaces; mostly threading the resolved workspace through shipped scoped paths).
