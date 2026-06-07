# Story Map: machine-token-admin-ux

## User: Priya Nandakumar (workspace admin); with Marco (integration owner) and Dana (security reviewer) as secondary
## Goal: Grant an integration programmatic access, hand over the secret safely, see what credentials exist, and revoke any that should no longer work — all from the product.

## Backbone

| Issue a token | See the secret once | Use it (API) | Audit what exists | Revoke when stale |
|---------------|---------------------|--------------|-------------------|-------------------|
| Mint via admin action | Show value once | (SHIPPED verify path) | List workspace tokens | Flip `revoked_at` |
| Pick label/scope/expiry | "copy now or lose it" | denylist + touch last_used | Status active/revoked | Refused next request |
| `MachineTokenSigner::mint` | never re-shown | — | "minted by {admin}" | idempotent revoke |

> The "Use it (API)" activity is SHIPPED (Feature A: `MachineTokenVerifier`, the per-request
> `jti` denylist, `touch_machine_token_last_used`). This feature adds the issue/see/audit/
> revoke surface around it. The backbone still lists it so the end-to-end flow is honest.

---

### Walking Skeleton (Slice 1) — mint ONE token end-to-end, value shown once

The thinnest end-to-end slice that proves the riskiest, highest-value path: server-side
Ed25519 signing surfaced safely through the admin surface, with the one-time secret display.

- **US-MT00** (`@infrastructure`, folded) — put a `MachineTokenSigner` live in `AppState`
  (the security-posture change, DM1/Q1) + the forward-only `created_by` migration (DM4).
  Never ships standalone; it is the substrate US-MT01 stands on.
- **US-MT01** — admin mints a token from the admin surface and sees its value EXACTLY ONCE;
  metadata (incl. `created_by`) persists, the secret does not.

This skeleton deliberately defers list, revoke, scope/expiry choice, and audit polish to
later slices — but it touches every backbone activity that is NEW (issue → see once), and
the SHIPPED verify path makes "use it" already true, so a minted token genuinely works.

### Release Slice 2: outcome — "a reviewer can see what programmatic access exists"

Targets **mt-job-2** (visibility half) / KPI: admin/reviewer can enumerate the workspace's
credentials without DB access.

- **US-MT02** — admin/reviewer lists the workspace's issued tokens (label, scope, expiry,
  status), newest first, via `list_machine_tokens`. No secret shown.

### Release Slice 3: outcome — "a reviewer can shut down a risky credential and it dies immediately"

Targets **mt-job-2** (control half) / KPI: a revoked token is refused on its next API call.

- **US-MT03** — admin/reviewer revokes a token (`revoke_machine_token`); the SHIPPED
  per-request denylist refuses it on the next `/api/v1` call. Idempotent.

### Release Slice 4: outcome — "issuance is least-privilege, bounded, attributable, and admin-only"

Targets **mt-job-3** (least privilege) + **mt-job-4** (audit) + the **mt-job-1** authz
boundary / KPI: tokens are issued with explicit scope + expiry within server bounds, every
row attributes its issuer, and only admins can use the surface.

- **US-MT04** — admin chooses scope (workspace vs team via `scope_team_id`) + expiry within
  server-enforced bounds (default + max cap) at mint time.
- **US-MT05** — only workspace admins can mint/list/revoke (`is_workspace_admin`);
  non-admins get a non-enumerable refusal.
- **US-MT06** — the list view shows "minted by {admin}" (`created_by`) + "last used"
  (`last_used_at`) for audit.

> **Note on US-MT04/05 vs the skeleton**: the walking skeleton (US-MT01) mints with a SAFE
> DEFAULT scope + default TTL and is admin-gated from day one (US-MT05's check is reused by
> US-MT01 — you cannot expose a mint surface without authz). US-MT04 adds the admin's
> *choice* of scope/expiry; US-MT05 hardens + explicitly tests the boundary. This avoids
> shipping an un-gated mint in Slice 1 while keeping the skeleton thin.

## Priority Rationale

| Priority | Release | Target Outcome | Job/KPI | Rationale |
|----------|---------|---------------|---------|-----------|
| 1 | Walking Skeleton (US-MT00 + US-MT01) | Mint one token end-to-end, secret shown once | mt-job-1 | **Riskiest + highest value.** Validates the signing-key-in-AppState posture (Q1) and the one-time-display UX — the two genuinely new, genuinely risky things. Everything else is registry reads/flags over shipped store fns. Tie-break: Walking Skeleton first. |
| 2 | Slice 2 (US-MT02) | See what exists | mt-job-2 (visibility) | A minted credential you cannot see is half a feature; list is a pure read over `list_machine_tokens`, low effort, high control-value. Comes before revoke because you revoke from the list. |
| 3 | Slice 3 (US-MT03) | Shut down a risky credential | mt-job-2 (control) | Revocation is the security payoff; it's a flag-flip over the SHIPPED denylist (the refusal mechanism already works), so effort is low and the value (immediate kill switch) is high. Depends on US-MT02 (revoke from the list). |
| 4 | Slice 4 (US-MT04/05/06) | Least-privilege, bounded, attributable, admin-only | mt-job-3 + mt-job-4 + mt-job-1 authz | Hardening + governance. Lower urgency than getting a working, listable, revocable token; each is incremental over the skeleton (scope/expiry choice, explicit authz tests, audit columns in the view). US-MT05's authz is *reused* by the skeleton, so its dedicated story is the hardening + adversarial test, not net-new gating. |

Value x Urgency / Effort favors the skeleton first (riskiest assumption: the signing-key
posture), then visibility, then the kill switch, then governance polish — exactly the order
that derisks the feature fastest while keeping every slice independently demonstrable.
