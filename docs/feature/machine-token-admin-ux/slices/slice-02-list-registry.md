# Slice 2 — See what exists (the registry)

## Outcome
A workspace admin or security reviewer sees every machine token issued in the workspace —
label, scope, expiry, status — newest first, without touching Postgres.

## Learning hypothesis
**We believe** giving reviewers an in-product, workspace-scoped list (over the shipped
`list_machine_tokens`) replaces DB queries for credential audit — **and we will know we are
right when** the listed set equals the issued set and reviewers stop querying the table.

## Riskiest assumption being validated
Low risk — a pure read over a shipped function. The validation is mostly UX: the list is
legible, workspace-isolated, and never leaks a token value.

## Stories
- **US-MT02** — list the workspace's tokens (label, scope, expiry, status), newest first.

## Reuses (shipped)
- `list_machine_tokens(workspace_id)` (foundry-store, newest first).
- `is_workspace_admin(...)` for the gate.

## Done when
- The surface lists the workspace's tokens, newest first, with label/scope/expiry/status.
- No token value appears anywhere in the list.
- The list is workspace-isolated (other workspaces' tokens never appear).
- An empty workspace shows an inviting empty state.

## Key risks / guardrails
- Cross-workspace leakage → NFR-MT-REL-03 / NFR-MT-SEC-03.
- Accidental value exposure in a detail view → NFR-MT-SEC-02.

## Open questions touching this slice
- **Q6** surface (web table vs `GET` JSON array) — surface-neutral; DESIGN picks.

## Depends on
- Slice 1 (there must be tokens to list).
