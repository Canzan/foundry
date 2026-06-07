# Slice 3 — Take it back (revocation)

## Outcome
A workspace admin or security reviewer revokes a token from its list row and it is refused on
its very next `/api/v1` call.

## Learning hypothesis
**We believe** an in-product revoke (over the shipped `revoke_machine_token` + per-request
denylist) gives reviewers an immediate, trustworthy kill switch — **and we will know we are
right when** a revoke is followed by a refused next API call within one request, with no DB
access needed.

## Riskiest assumption being validated
Low crypto risk (the refusal mechanism is shipped and tested). The validation is the UX +
the trust property: revoke is immediate, final, idempotent, and workspace-bounded.

## Stories
- **US-MT03** — revoke a token; refused on next API use; idempotent; workspace-bounded.

## Reuses (shipped)
- `revoke_machine_token(jti)` → `SET revoked_at = now()` (foundry-store).
- The per-request `jti` denylist (`find_machine_token_by_jti(jti).revoked_at IS NULL`) — the
  refusal already works; no new verify code.
- `is_workspace_admin(...)` for the gate.

## Done when
- An admin revokes any token in their workspace from its row, after a clear
  immediate-and-irreversible warning.
- The revoked token's next API call is refused; the row survives and shows Revoked.
- Revoke is idempotent (already-revoked → no-op success).
- Revoking a token outside the acting workspace is refused non-enumerably; that token is
  untouched.

## Key risks / guardrails
- Belief that revoke is effective when it is not → NFR-MT-SEC-05 (next-request refusal).
- Cross-workspace revoke → NFR-MT-SEC-03 / NFR-MT-REL-03.
- Idempotency / concurrent revoke → NFR-MT-REL-02.

## Open questions touching this slice
- **Q6** surface (web button vs `DELETE`/`POST .../revoke`) — surface-neutral; DESIGN picks.

## Depends on
- Slice 2 (revoke is triggered from the list row).
