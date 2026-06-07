# Journey — Token-Management API (the integrator / automation experience)

> This is an **API / integrator journey**, not a screen journey. The "user" is an automation
> caller (a CI pipeline, a rotation script, an audit job) using `curl` / an SDK against
> `/api/v1`. The emotional arc is **integrator confidence**: predictable authz, stable
> machine-readable errors, and revoke-that-actually-works — built progressively from the safest
> read op to the rotation flow. The escalation / abuse paths are first-class, not an afterthought.
>
> Grounded in shipped code: the bearer extractor `MachinePrincipal` / `token_auth::authenticate`
> and the `status_for` JSON envelope (`crates/foundry-api/src/lib.rs`); the use-cases
> `list_tokens` / `revoke_token` (`crates/foundry-services/src/tokens.rs`). Reflects the
> RECOMMENDED authz model (option c — wave-decisions.md Q-AUTHZ): v1 exposes LIST + REVOKE to
> bearer tokens; MINT stays human-session-only (the escalation-sensitive op, deferred).

## Persona & Goal

- **Persona:** Sven Aarø (integrator) and the Acme nightly-rotation job (automation agent);
  Dana's audit pipeline (security automation).
- **Goal:** inventory and lifecycle-manage machine-token credentials **programmatically** over
  `/api/v1` — list what exists, revoke what should not, and rotate a credential hands-free —
  with the SAME security guarantees the web UI ratified, and with the privilege-escalation
  surface deliberately bounded.

## Emotional Arc

- **Start:** *cautious* — "Can a token even manage tokens? Will this let something runaway-mint?
  Is the error contract stable enough to script against?"
- **Middle:** *building confidence* — the safe read works, refusals are predictable and
  non-enumerable, revoke is immediate and verifiable.
- **End:** *confident & in control* — rotation is hands-free and provable; the escalation-sensitive
  op (mint) is explicitly NOT on this surface in v1, which the integrator reads as "they thought
  about the footgun," not as a missing feature.

## Draft Sketch (the working hypothesis)

```
[Trigger:                  [Step 1:               [Step 2:              [Step 3:              [Goal:
 automation holds a         GET list via           DELETE/POST           rotate: mint-new       inventory +
 bearer token]             bearer]                revoke {jti}]         (human/UI) then        lifecycle under
                                                                        revoke-self]           control, hands-free]
  Feels: cautious           Sees: JSON array       Sees: 204/200 +       Sees: old token        Feels: confident,
                            (no values)            next call 401         dead on next call      escalation bounded
  Authz: ??? (CRUX)         Authz: mgmt vs         Authz: same gate +    Self is a subset of
                            non-mgmt (403)         non-enum NotFound     the workspace
```

## ASCII Flow (happy path + refusal + abuse paths)

```
                          ┌─────────────────────────────────────────────────────────────┐
                          │  Authentication (SHIPPED): MachinePrincipal / token_auth      │
                          │  bearer JWT → Principal::Machine{user_id,workspace_id,jti,…}  │
                          │  fail-closed: any auth failure → IDENTICAL 401 (non-enum)     │
                          └───────────────────────────────┬─────────────────────────────┘
                                                          │ authenticated
        ┌─────────────────────────────────────────────────┼──────────────────────────────────────┐
        │                                                 │                                        │
        ▼                                                 ▼                                        ▼
 ┌──────────────┐                              ┌──────────────────┐                     ┌────────────────────┐
 │ STEP 1: LIST │                              │ STEP 2: REVOKE   │                     │ STEP 3: ROTATE     │
 │ GET …/tokens │                              │ DELETE …/{jti}   │                     │ (LIST→revoke-self) │
 └──────┬───────┘                              └────────┬─────────┘                     └─────────┬──────────┘
        │ authz: is_workspace_admin(bound user)         │ authz: same gate                        │
        │  ├─ management-capable → 200 [TokenView…]     │  ├─ mgmt + own-workspace jti → 204/200  │ mint-new is a
        │  └─ non-management     → 403 forbidden        │  ├─ non-mgmt        → 403               │ HUMAN/UI step
        │     (non-enumerable: no list leaked)          │  ├─ unknown jti     → 404 not_found ◄───┤ in v1 (MINT
        ▼                                               │  └─ OTHER workspace → 404 not_found     │ NOT on bearer
   JSON array of {jti,label,scope,expiry,               │     (NON-ENUMERABLE — same as unknown)  │ surface — see
    revoked,last_used,minted_by}  — NEVER a value       ▼                                         │ ABUSE box)
                                              revoked_at flip → SHIPPED per-request denylist
                                              refuses that jti on its VERY NEXT /api/v1 call

  ┌──────────────────────────────── ABUSE / ESCALATION PATHS (first-class) ────────────────────────────────┐
  │ A. Leaked bearer token tries to MINT → in v1 there is NO bearer mint route; the only mint surface is    │
  │    the human session UI (/admin/tokens). No mint-loop / self-replication is possible from a bearer.     │
  │ B. Leaked bearer token tries to REVOKE every other token → workspace-confined DoS; loud, reversible by  │
  │    the human admin (who can still log into /admin/tokens — no mint loop to outrace). Q-RATE-LIMIT        │
  │    guardrail throttles a revoke storm.                                                                  │
  │ C. Leaked bearer token probes another workspace's jti → 404 not_found, IDENTICAL to an unknown jti. No  │
  │    oracle: the attacker cannot tell whether the jti exists.                                             │
  │ D. Non-management bearer token hits any token route → 403 forbidden, non-enumerable (no registry leak). │
  └─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Steps

### Step 1 — LIST: "show me what exists" (the walking skeleton, safest op)

**Command (illustrative — DESIGN fixes exact path/field names):**
```
$ curl -s -H "Authorization: Bearer $FOUNDRY_TOKEN" \
       https://foundry.acme.dev/api/v1/teams/platform/projects/infra/tokens
```

**TUI / wire mockup (success — management-capable bearer):**
```
HTTP/1.1 200 OK
Content-Type: application/json

[
  {
    "jti": "${jti}",                          <-- tracked artifact (row PK, safe to show)
    "label": "ci-issue-filer",
    "scope_team_id": null,                    <-- null = workspace-wide; else a team id
    "scope_team_name": null,
    "expires_at": "2026-09-05T00:00:00Z",
    "revoked": false,
    "last_used_at": "2026-06-07T03:11:00Z",
    "minted_by": "priya@acme.dev"             <-- created_by resolved (audit; NFR-TMA-SEC-06)
  }
]
# NOTE: there is NO "value"/"token"/"secret" field. Ever. (NFR-TMA-SEC-02)
```

**Refusal (non-management bearer — non-enumerable):**
```
HTTP/1.1 403 Forbidden
{ "error": { "code": "forbidden", "message": "forbidden" } }
# The body does NOT reveal whether any tokens exist. Same shape as the issue/comment 403.
```

- **Emotional:** entry *cautious* → exit *reassured* (the read works, the contract is the same
  stable envelope, no value leaks).
- **Shared artifacts:** `${jti}`, the `status_for` envelope, the `TokenView` shape.
- **Integration checkpoint:** the LIST JSON mirrors `foundry_services::tokens::TokenView` field
  for field, MINUS nothing and PLUS nothing — and never carries a value.
- **Failure modes (for DISTILL):** missing/expired/forged bearer → identical 401; non-management
  bearer → 403 non-enumerable; cross-workspace rows never appear (workspace-scoped read);
  empty registry → `[]` (200, not 404).

### Step 2 — REVOKE: "kill this credential now"

**Command (verb is Q-REVOKE-VERB; `DELETE` shown as the default):**
```
$ curl -s -X DELETE -H "Authorization: Bearer $FOUNDRY_TOKEN" \
       https://foundry.acme.dev/api/v1/teams/platform/projects/infra/tokens/${jti}
```

**Wire mockup (success):**
```
HTTP/1.1 204 No Content
# Idempotent: re-DELETEing an already-revoked jti is also a success (mirrors the
# idempotent revoked_at re-stamp in revoke_token).
```

**Proof it worked (the next call by the revoked credential):**
```
$ curl -s -H "Authorization: Bearer $REVOKED_TOKEN" .../api/v1/.../issues
HTTP/1.1 401 Unauthorized
{ "error": { "code": "unauthorized", "message": "unauthorized" } }
# The SHIPPED per-request jti denylist refuses it on the VERY NEXT call. No cache. (NFR-TMA-SEC-05)
```

**Refusal — cross-workspace / unknown jti (NON-ENUMERABLE):**
```
HTTP/1.1 404 Not Found
{ "error": { "code": "not_found", "message": "not found" } }
# A jti in ANOTHER workspace and a jti that does not exist return the IDENTICAL 404.
# The caller can never tell whether the credential exists. (NFR-TMA-SEC-03)
```

- **Emotional:** entry *focused* → exit *relieved* (revoke is final, immediate, and provable in the
  rotation log).
- **Shared artifacts:** `${jti}`, the SHIPPED per-request denylist, the non-enumerable `NotFound`.
- **Integration checkpoint:** revoke goes through `revoke_token` unchanged (authz →
  non-enumerable workspace isolation → idempotent flip); effectiveness is the EXISTING denylist in
  `token_auth::authenticate` — no new refusal code.
- **Failure modes:** non-management bearer → 403; cross-workspace/unknown → identical 404;
  double-revoke → idempotent success; a revoke storm → Q-RATE-LIMIT guardrail.

### Step 3 — ROTATE (revoke-self): "promote the new credential, then retire me"

The core hands-free automation flow. The NEW credential is provisioned by a human via the UI in v1
(MINT is not on the bearer surface — see the abuse box), then the rotation script promotes it and
**revokes its OWN token** so the old credential cannot be used again.

**Wire flow (illustrative):**
```
# (human/UI, v1) admin mints "ci-issue-filer-2026Q3" in /admin/tokens, hands secret to the pipeline
# (automation) pipeline switches its config to the new token, verifies it works, then:
$ curl -s -X DELETE -H "Authorization: Bearer $OLD_TOKEN" \
       https://foundry.acme.dev/api/v1/teams/platform/projects/infra/tokens/${old_jti}
HTTP/1.1 204 No Content
# The script just revoked the very token it authenticated with. Its NEXT call with $OLD_TOKEN → 401.
```

- **Emotional:** entry *confident* → exit *in control* (rotation is hands-free and self-cleaning;
  the script proves the old credential is dead before relying on the new one).
- **Shared artifacts:** `${old_jti}`, the caller's own `jti` from `Principal::Machine`.
- **Integration checkpoint:** revoke-self is a SUBSET of `revoke_token` — the caller's own `jti` is
  in its own workspace, so it passes workspace isolation and flips `revoked_at` normally; the only
  special-ness is that the NEXT request by that same credential is the one that 401s.
- **Failure modes:** the script revokes-self BEFORE switching to the new token → it locks itself out
  mid-run (operational footgun, not a security one; the runbook must switch first, then revoke-self —
  this is documented as a habit-path scenario).

## Abuse / Escalation Paths (first-class — see also the escalation table in wave-decisions.md)

| # | Abuse | v1 outcome | Why bounded |
|---|-------|-----------|-------------|
| A | Leaked bearer token tries to **MINT** more tokens | **No bearer mint route exists in v1.** | Mint stays human-session-only; no self-replication / mint loop possible from a bearer credential. |
| B | Leaked bearer token **revokes every other token** (DoS) | Workspace-confined; Q-RATE-LIMIT guardrail throttles; human admin still recovers via UI | Loud, reversible, no mint loop to outrace; cross-workspace tokens unaffected. |
| C | Leaked bearer token **probes another workspace's jti** | 404, IDENTICAL to unknown jti | Non-enumerable `NotFound` (reused from `revoke_token`); no existence oracle. |
| D | **Non-management** bearer token hits any token route | 403 forbidden, non-enumerable | The authz gate (Q-AUTHZ ratified model) refuses without revealing the registry. |

## Shared Artifacts (tracked across steps)

| Artifact | Source of truth | Consumers | Integration risk |
|----------|-----------------|-----------|------------------|
| `${jti}` | `machine_tokens.jti` (PK) → `TokenView.jti` / `MintedToken.jti` | LIST response, REVOKE path param, the per-request denylist | LOW — opaque id, safe to display; the revoke key |
| JSON error envelope | `foundry_api::status_for` (`ErrorBody{code,message}`) | every refusal/validation across all token routes | HIGH — must be IDENTICAL to the shipped issue/comment envelope (no new shapes) |
| Bearer principal | `token_auth::authenticate` → `Principal::Machine{user_id,workspace_id,jti,scope_team_id}` | the authz gate on every token route | HIGH — the authz model (Q-AUTHZ) is decided here; naive wiring silently ships option (a) |
| `TokenView` list shape | `foundry_services::tokens::TokenView` (no value field) | LIST response | HIGH — must NEVER gain a value/secret field (NFR-TMA-SEC-02) |
| per-request denylist | `token_auth::authenticate` (`resolve_active_token`, SHIPPED) | proves revoke is effective on the next call | LOW — unchanged; reused as the revoke effectiveness mechanism |

## Integration Validation

- The LIST response shape equals `TokenView` minus internal types, plus zero secret fields.
- Every refusal across token routes uses `status_for` (same codes/statuses as issues/comments).
- Revoke effectiveness is the SHIPPED denylist — assert by a "next call is 401" scenario, not a new
  mechanism.
- The authz gate is decided ONCE (Q-AUTHZ) and applied uniformly to LIST + REVOKE; the walking
  skeleton proves authorized-vs-refused before any mutation ships.
