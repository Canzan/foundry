# Non-Functional Requirements — Machine-Token Admin UX

> SECURITY-heavy by nature: this feature turns the running process into a credential ISSUER
> and surfaces a one-time secret. Every NFR is testable. IDs are referenced from `stories.md`
> and `wave-decisions.md`. Solution-neutral: these fix the constraints + observable
> properties; DESIGN picks mechanisms.

## Security

### NFR-MT-SEC-01 — Token value shown exactly once, never persisted
The minted token value (the `SecretString` from `MachineTokenSigner::mint`) is displayed on
exactly one surface (the issuance screen / the `POST` response body) and is NEVER written to
the database, a log, an error message, or any retrievable store.
- **Measurable**: a DB scan of `machine_tokens` finds no column carrying a token/secret/hash
  (matches the shipped table). A log scan during an issuance acceptance run finds no token
  value substring. The minted JWT remains a `SecretString` (no `Debug`/`Display`).
- **Verify**: acceptance asserts the value appears once at issuance and nowhere afterward;
  static check that no `machine_tokens` migration adds a secret column.

### NFR-MT-SEC-02 — Token value never re-displayed
No screen or endpoint re-displays a token value after issuance. Every list/detail surface
exposes only `jti` + metadata.
- **Measurable**: `GET .../tokens` (and the web list/detail) contain no `token` field/column;
  there is no "reveal" endpoint.
- **Verify**: acceptance asserts the value is absent from the list and from any per-token
  detail view; an API contract check asserts no response after the `201` carries `token`.

### NFR-MT-SEC-03 — Issuance is workspace-admin-only, non-enumerable
Mint, list, and revoke require `is_workspace_admin(workspace_id, acting_user)`. Non-admins
and cross-workspace actors are refused without revealing the surface or token existence.
- **Measurable**: 100% of non-admin and cross-workspace attempts are refused; the refusal
  body/status does not differ in a way that confirms existence.
- **Verify**: adversarial acceptance scenarios (US-MT05) for non-admin and cross-workspace
  mint/list/revoke.

### NFR-MT-SEC-04 — Signing-key posture is explicit and bounded (Q1/DM1)
Minting requires a `MachineTokenSigner` (the Ed25519 PRIVATE key) live in `AppState`. This is
a deliberate posture change from verifier-only. A binary without a signer offers no mint
surface and cannot issue.
- **Measurable**: an issuer-configured binary can mint; a verifier-only binary cannot and
  reports "issuing not enabled" (not a 500). The boot key self-test still passes on issuer
  binaries.
- **Verify**: acceptance scenarios (US-MT00/US-MT01) for both binary configurations. **DESIGN
  owns** the at-rest key mechanism, the guard, and whether issuer capability is a separate
  binary/config mode — and must document the threat delta vs verifier-only.

### NFR-MT-SEC-05 — Revocation effective on the next request
A revoked token (`revoked_at` set) is refused on its very next `/api/v1` request via the
SHIPPED per-request `jti` denylist; there is no token cache that keeps a revoked credential
alive.
- **Measurable**: revoke → the next API call with that token is refused (one-request latency).
- **Verify**: acceptance scenario (US-MT03) revokes then asserts the next call is refused.

### NFR-MT-SEC-06 — Issuance is attributable (audit trail)
Every token minted after this feature records `created_by` (the acting admin) and surfaces it
in the list, alongside `last_used_at`.
- **Measurable**: 100% of post-feature `machine_tokens` rows have non-NULL `created_by`; the
  list shows issuer + last-used.
- **Verify**: row-level check + acceptance scenario (US-MT06).

### NFR-MT-SEC-07 — Issuance surface preserves the browser auth/CSRF/session contract
If the admin surface is the web UI, it preserves the existing contract unchanged: double-
submit `foundry_csrf` cookie + `_csrf`/`HX-CSRF`, tower-sessions Postgres store, 30-day
cookie attrs, argon2id sign-in, brute-force delay, non-enumerable sign-in error.
- **Measurable**: existing browser-auth acceptance scenarios stay green; the mint/revoke POSTs
  carry CSRF protection.
- **Verify**: the `foundry-acceptance` browser-auth suite remains green; new mint/revoke POST
  scenarios assert CSRF enforcement.

## Reliability / Correctness

### NFR-MT-REL-01 — Mint is all-or-nothing (no partial-token leak)
A render or persistence failure during issuance never produces a half-emitted page or
response that leaks a partial token value.
- **Measurable**: an injected render/persist failure at issuance yields a clean error, never a
  partial token.
- **Verify**: acceptance scenario with an injected issuance failure (mirrors the existing
  `force_board_render_failure` test seam pattern).

### NFR-MT-REL-02 — Revoke is idempotent
Revoking an already-revoked token is a no-op success; concurrent revokes converge to Revoked.
- **Verify**: acceptance scenario (US-MT03) double-revoke + a concurrent-revoke check.

### NFR-MT-REL-03 — Registry reads are workspace-isolated
`list_machine_tokens` and any per-token lookup return only the acting workspace's rows.
- **Verify**: cross-workspace acceptance scenario (US-MT02/US-MT05).

## Performance

### NFR-MT-PERF-01 — Issuance and revocation are interactive
A mint completes (sign + persist + render the one-time value) and a revoke completes within an
interactive budget consistent with the existing web tier (target ≤200 ms server-side,
excluding network), with no regression to the SHIPPED per-request verify path.
- **Measurable**: server-side mint/revoke handler time ≤200 ms p95 in the acceptance harness;
  verify-path latency unchanged.
- **Verify**: timing assertions in the issuance/revoke scenarios; the existing verify-path
  benchmarks stay within budget.

## Data Integrity

### NFR-MT-DATA-01 — Forward-only `created_by` migration, no rewrite of history
The `created_by` column is added forward-only (ADR-003), nullable, referencing `users(id)`;
pre-existing rows back-fill NULL. No prior migration is edited.
- **Verify**: migration review + a scenario asserting a pre-feature row shows an unknown
  issuer while new rows carry the admin.

### NFR-MT-DATA-02 — No secret column ever added
No migration in this feature adds a token/secret/hash column to `machine_tokens`.
- **Verify**: static review of the feature's migrations.

## Invariants (carried, must not regress)

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
- The SHIPPED verify path + per-request `jti` denylist + `iss`/`aud`/EdDSA pinning are
  unchanged; this feature adds issuance/registry surface around them, not new verify logic.
- `MachineTokenSigner::mint` and the `machine_tokens` repo are reused, not reimplemented.
- The `foundry-acceptance` suite green before this feature stays green after.
