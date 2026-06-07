# Upstream Issues — machine-token-admin-ux (DISTILL)

Gaps DISTILL surfaced while making the DISCUSS/DESIGN scenarios executable.

## UI-1 — Single-workspace schema constraint blocks REAL cross-workspace fixtures (LOW impact, modelling note)

**Where**: `crates/foundry-store/migrations/0001_init.sql`:
```sql
CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true));
```
plus `machine_tokens.workspace_id UUID NOT NULL REFERENCES workspaces(id)` and
`scope_team_id UUID REFERENCES teams(id)` (0007).

**The gap**: slice-1 Foundry is a **single-workspace** product (the unique index
permits exactly one `workspaces` row per database). Several DISCUSS scenarios and
DESIGN NFRs are framed cross-workspace and name a second workspace "Globex":
- US-MT02 scenario 2 — "a reviewer of one workspace cannot see another
  workspace's tokens".
- US-MT03 scenario 3 / NFR-MT-REL-03 — "an admin cannot revoke a token outside
  their workspace … the Globex token remains active".
- US-MT04 scenario 3 — "a scope that is not part of the workspace is refused".
- US-MT05 scenario 3 — "an admin of one workspace cannot manage another
  workspace's tokens".

A REAL second workspace (and a `machine_tokens`/`teams` row owned by it) is
**structurally impossible to seed** in slice 1: a second `workspaces` row
violates `uniq_one_workspace`, and a foreign `workspace_id`/`scope_team_id`
violates the FK.

**How DISTILL modelled it (faithful, not a workaround)**: the cross-workspace
evil-user paths are exercised with a SYNTHETIC `jti`/team uuid that the acting
workspace did not issue / does not own. From the acting admin's side this is
**observably identical** to a foreign-workspace target: the service's
`find_machine_token_by_jti(jti)` returns `None` (or a row whose
`workspace_id != principal.workspace_id()` once a multi-workspace world exists),
yielding the SAME non-enumerable `404`. The behaviour under test — the
non-enumerable refusal + "the acting admin's revoke does not touch a token it
cannot see" — is faithfully covered. List isolation (US-MT02) is exercised at the
read boundary: the list must contain EXACTLY the acting workspace's rows (the
`list_machine_tokens(workspace_id)` filter), never more.

**Impact**: LOW. The SECURITY guarantee (non-enumerability, workspace-scoped
reads, scope-team-in-workspace validation) is verifiable today. What is NOT
exercised end-to-end is "two concurrently-existing workspaces with a real
boundary between them" — which the product does not support yet anyway.

**Recommendation for DELIVER / a future multi-workspace feature**: when Foundry
gains multi-workspace support (the `uniq_one_workspace` index is dropped),
PROMOTE these synthetic-jti scenarios to seed a REAL second workspace + a real
foreign token, and assert the foreign row is untouched by lookup (not just
`None`). The DESIGN contract (`token-admin-services.md` `revoke_token` step 2:
`row.workspace_id != principal.workspace_id()`) already anticipates the real
multi-workspace check — the service code path is correct; only the test fixture
is bounded by today's schema. No DESIGN change is required for slice 1.

**Resolution status**: NOT a blocker. No prior-wave document is wrong; this is a
modelling constraint of the shipped schema, documented so DELIVER (and the future
multi-workspace feature) inherit the context. No `created_by` / signer / mint /
revoke contract is affected.
