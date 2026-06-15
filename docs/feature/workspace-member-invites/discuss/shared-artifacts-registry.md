# Shared Artifacts Registry — workspace-member-invites

Every value that flows across the two-capability journey (issuance + acceptance), its single source of
truth, and its consumers. The invite token, the invites row, and the NEW user/membership created on
accept are the security-critical ones. This generalizes the shipped `invite-accept-flow` registry — the
DELTA artifacts (`invitee_email`, `user_id`, `membership`) are the new-account-creation seam.

```yaml
shared_artifacts:
  workspace_id:
    source_of_truth: "issuance: SessionUser.workspace_id (signed-in admin's active workspace, via resolve_active_workspace). acceptance: invites.workspace_id, resolved at landing via resolve_active_workspace(new user)."
    consumers:
      - "is_workspace_admin(workspace_id, user_id) issuance gate"
      - "insert_invite workspace_id (issuance)"
      - "create_member_and_consume: the new membership's workspace_id"
      - "SessionUser.workspace_id at landing; dashboard tenant scoping"
    owner: "multi-workspace-tenancy (shipped) -> this feature reads/writes membership"
    integration_risk: "HIGH — the new member must join AND land on invites.workspace_id, never another tenant. The issuance gate workspace_id is the admin's; the accept workspace_id is the invite's."
    validation: "Landed workspace_id == invites.workspace_id == the membership's workspace_id; Sam sees only that tenant."

  public_url:
    source_of_truth: "application configuration (foundry-app public_url / PUBLIC_URL env var) — AppState.public_url, the same value bootstrap::create_invite uses (bootstrap.rs:245-250)"
    consumers:
      - "issuance: format!(\"{public_url}/invites/accept?id={}&sig={}\") — the emitted link + the best-effort email body"
    owner: "foundry-app configuration — shipped, reused verbatim"
    integration_risk: "MEDIUM — the emitted link's host MUST be the public-facing domain the invitee actually receives (e.g. https://foundry.northwind.example); a misconfigured public_url emits an unreachable link. Single source: AppState.public_url, never hardcoded per-handler."
    validation: "The emitted invite_url begins with the configured public_url (trailing slash trimmed, as in create_invite); the same value is used by the shipped first-admin emit, so consistency is already exercised."

  invite_id:
    source_of_truth: "invites.id — NEW row created by store::insert_invite during issuance (uuid v7)"
    consumers:
      - "the emitted /invites/accept?id= link"
      - "GET /invites/accept query param + hidden form field + POST body"
      - "create_member_and_consume WHERE id = $1"
    owner: "issuance (writer) -> acceptance (reader/consumer)"
    integration_risk: "HIGH — the id issuance creates must be the id accept consumes; the consumed row must be the one issuance created."
    validation: "InviteToken::verify binds id||expires_at; create_member_and_consume matches on id; new membership workspace == invites.workspace_id."

  signature:
    source_of_truth: "InviteToken::new(invite_id, expires_at, SESSION_SECRET).signature — HMAC, foundry-auth (shipped, reused verbatim)"
    consumers:
      - "emitted link sig param (url-encoded)"
      - "hidden form field on the set-password page"
      - "GET + POST re-verification (defense-in-depth before the DB hit)"
    owner: "foundry-auth (InviteToken) — shipped, reused"
    integration_risk: "HIGH — tamper detection. The sig emitted on issuance == the sig rendered on GET == the sig POST re-verifies. The DB row is the primary single-use control; HMAC is defense-in-depth."
    validation: "InviteToken::verify(id, expires_at, sig, secret) returns Ok only for an untampered (id, expiry) pair."

  expires_at:
    source_of_truth: "invites.expires_at — set at issuance to now + 7 days (mirrors bootstrap::create_invite and provision_workspace)"
    consumers:
      - "bound into the HMAC payload (invite_payload)"
      - "GET liveness check (expires_at > now)"
      - "create_member_and_consume WHERE expires_at > now (race-safe expiry guard inside the tx)"
    owner: "issuance (writer) -> acceptance (reader)"
    integration_risk: "MEDIUM — expiry is bound into the signature; an attacker cannot extend it without breaking the HMAC. 7d matches the already-emitted 'valid for 7 days' promise."
    validation: "InviteToken::verify rejects a mutated expiry; the consume re-checks expiry inside the tx."

  invitee_email:
    source_of_truth: "the email the admin typed at issuance; stored in invites.invitee_email (DELTA: the first-admin flow stored the admin's own email; here it is the prospective member's)"
    consumers:
      - "email.send target (best-effort invite email)"
      - "create_member_and_consume: the NEW users row email_lower/email_display"
      - "email-already-a-user check (OD-1) inside the tx"
    owner: "issuance (writer) -> acceptance (reader, becomes the new account's identity)"
    integration_risk: "HIGH (DELTA) — this email BECOMES the new user's identity. It must be the single source for the created account; if it already maps to a user, the v1 behavior is non-enumerable refusal (OD-1)."
    validation: "After accept, the new users.email_lower == invites.invitee_email (lower-cased); exactly one user created."

  csrf_token:
    source_of_truth: "csrf::generate_token + double-submit cookie (foundry-app csrf.rs — shipped)"
    consumers:
      - "hidden _csrf field on the issuance form AND the set-password form"
      - "CSRF middleware on POST /workspace/invites AND POST /invites/accept"
    owner: "foundry-app web tier — shipped, reused"
    integration_risk: "MEDIUM — the accept page is a PUBLIC (signed-out) route; CSRF still applies to its POST. The issuance page is admin-gated; CSRF applies to its POST too. Both state-changing POSTs are protected."
    validation: "A POST (either surface) without a matching double-submit token is refused before any create/consume."

  password_hash:
    source_of_truth: "foundry_auth::hash_password (argon2id, OWASP) — shipped, reused verbatim. Validated by check_password_policy (min-12) BEFORE hashing."
    consumers: ["the NEW users row created in create_member_and_consume", "verify_password at later sign-in"]
    owner: "foundry-auth — shipped"
    integration_risk: "MEDIUM — written into a NEWLY created user row in the SAME tx as the consume + membership (DELTA: the first-admin flow updated a pre-existing row). Validation runs before the tx opens."
    validation: "After accept, verify_password(chosen_pwd, stored_hash) succeeds at sign-in for the new account."

  user_id:
    source_of_truth: "NEW users.id created inside create_member_and_consume (uuid v7) — DELTA: no pre-existing user, unlike the first-admin created_by"
    consumers: ["workspace_memberships.user_id", "invites.used_by", "SessionUser.user_id"]
    owner: "this feature (the account-creation seam)"
    integration_risk: "HIGH (DELTA) — must be created exactly once even under concurrency; tied to the consume guard so a lost race creates no orphan user."
    validation: "Concurrent accepts -> exactly one user created; the row is the one referenced by the membership and the session."

  membership:
    source_of_truth: "NEW workspace_memberships row (workspace_id, user_id, role='member') created in create_member_and_consume — mirrors the 'admin' insert in provision_workspace with role swapped to member"
    consumers: ["resolve_active_workspace", "all workspace-scoped authz (member, not admin)"]
    owner: "this feature -> multi-workspace-tenancy membership model (shipped)"
    integration_risk: "HIGH — the role MUST be 'member' (v1 scope); the workspace_id MUST equal invites.workspace_id. A wrong role or workspace is a privilege/tenant breach."
    validation: "After accept, the membership is role='member' for invites.workspace_id; the member cannot reach the admin-gated issuance surface."

  invites_row_used_at:
    source_of_truth: "invites.used_at / used_by — the single-use markers (SHIPPED columns from invite-accept-flow, reused verbatim)"
    consumers: ["create_member_and_consume guard (UPDATE ... WHERE used_at IS NULL AND expires_at > now)", "GET liveness re-render guard"]
    owner: "this feature reuses the shipped single-use semantics"
    integration_risk: "HIGH — single-use atomicity. The UPDATE's WHERE is the race guard: exactly one concurrent consumer updates 1 row, creates the user+membership, and signs in."
    validation: "Two concurrent consumes -> exactly one updates 1 row and creates one account; the other sees 0 rows -> uniform refusal."

  session:
    source_of_truth: "session layer SESSION_KEY_USER_ID -> SessionUser{user_id, workspace_id} (foundry-app — shipped, reused)"
    consumers: ["all subsequent authenticated requests", "the 303 landing on /"]
    owner: "foundry-app web tier — shipped, reused"
    integration_risk: "MEDIUM — established only after a successful create+join+consume; auto sign-in means no separate login step."
    validation: "After accept, the redirect to / is authenticated as the new member and resolves Northwind."
```

## Consistency checks (for DISTILL / DELIVER)

1. Does every `${variable}` in the TUI mockups have a documented source above? **Yes** — all 12 tracked
   (incl. `public_url`, the single source for the emitted link host).
2. The `sig` emitted by issuance == the `sig` rendered on GET == the `sig` POST re-verifies. (HIGH)
3. The consumed `invite_id` == the row `insert_invite` created at issuance. (HIGH)
4. The new account's email == `invites.invitee_email`; exactly one user created. (HIGH, DELTA)
5. The new `membership` is role `member`, for `invites.workspace_id`; landed `workspace_id` matches. (HIGH)
6. The uniform acceptance refusal leaks NONE of: workspace name, account existence, invite-state reason,
   or email-already-a-user fact. (security crux — reused invariant, extended to A-E9)
7. The issuance refusal (non-admin/signed-out) is byte-identical to a generic 404. (issuance non-enumerability)
8. create-user + member-membership insert + consume(`used_at`) are in the SAME transaction (atomicity).
