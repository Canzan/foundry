# Out of Scope — Machine-Token Admin UX

> What this feature deliberately does NOT do, with the reason and where it goes instead.
> Keeps the walking skeleton thin and the scope right-sized (see `wave-decisions.md` Phase 2).

| # | Out of scope | Reason | Where it goes |
|---|--------------|--------|---------------|
| 1 | **OAuth / OIDC / third-party identity** | Machine tokens are self-contained Ed25519 JWTs verified offline; bolting on an OAuth/OIDC flow is a different feature and a different threat model. | Future feature if external IdP integration is ever wanted. |
| 2 | **Per-endpoint ACLs / fine-grained permission scopes beyond the existing model** | The claim set supports `scope: Option<Uuid>` (workspace vs team) and the principal `sub` bounds authorization. A richer read/write or per-route permission grammar is a larger design. | Flagged in Q3 as a possible v2; v1 reuses the existing workspace/team scope. |
| 3 | **Key rotation UI** | Rotating the `MACHINE_TOKEN_SIGNING_KEY` / public-key set is an operator concern; the verifier already supports an overlapping key SET for rotation, but a *UI* for rotation is out of scope unless trivial. | **DECISION: OUT** for v1. Rotation stays an operator/env + DESIGN concern; the verifier's existing multi-key support already covers the crypto. |
| 4 | **Editing an issued token's scope/expiry after the fact** | A token is immutable once signed (the claims are baked into the JWT). "Change scope" = revoke + reissue. | Revoke (US-MT03) + reissue (US-MT01/MT04) is the supported path; no in-place edit. |
| 5 | **Recovering / re-displaying a lost token value** | The value is shown once by design (NFR-MT-SEC-01/02); the registry stores no secret. Recovery would defeat the security model. | Revoke the lost `jti` + mint a new token (covered as the anxiety path in US-MT01). |
| 6 | **Automatic expiry sweep / GC of dead rows as a user-facing feature** | The table comment notes an expiry sweep can prune dead rows; that is operational housekeeping, not an admin UX. | Operator/DEVOPS concern; the registry surviving revocation until GC is an invariant, not a UI. |
| 7 | **Usage analytics / per-token request metrics dashboards** | `last_used_at` (surfaced in US-MT06) is enough for staleness triage; full per-token request analytics is a separate observability feature. | Future feature; DEVOPS owns deeper instrumentation if wanted. |
| 8 | **Non-admin self-service token issuance** | v1 restricts issuance to workspace admins (US-MT05, Q4). A "members can mint their own tokens" model is a different authz design. | Possible future feature pending Q4 revisiting. |
| 9 | **Choosing the signing-key-at-rest mechanism / building issuer-mode infrastructure** | DISCUSS captures the requirement + risk (Q1/DM1); the at-rest mechanism, the key guard, and whether issuer capability is a separate binary/config mode are DESIGN, not DISCUSS. | DESIGN wave (solution-architect). |
| 10 | **Picking the surface (web UI vs JSON API vs both) and its concrete shape** | The stories name concrete candidate entry points but stay surface-neutral; the choice is DESIGN (Q6). | DESIGN wave; the journey + stories give both candidates. |
