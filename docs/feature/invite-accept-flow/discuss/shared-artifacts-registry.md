# Shared Artifacts Registry — invite-accept-flow

Every value that flows across the accept journey, its single source of truth, and its consumers.
Untracked artifacts are the primary cause of horizontal integration failures; the invite token +
the invites row are the security-critical ones.

```yaml
shared_artifacts:
  invite_id:
    source_of_truth: "invites.id — the row PK created by store::insert_invite during provision_workspace (crates/foundry-store/src/lib.rs:491, :1254)"
    consumers:
      - "URL emitted by bootstrap::create_invite (bootstrap.rs:269) and CLI provision-workspace (admin_cli.rs:505)"
      - "GET /invites/accept query param `id`"
      - "hidden form field on the set-password page"
      - "POST /invites/accept body"
      - "consume_invite TX WHERE id = $1"
    owner: "provisioning (writer) -> invite-accept-flow (reader/consumer)"
    integration_risk: "HIGH — the id verified on GET must be the same id consumed on POST; the consumed row must be the one provisioning created."
    validation: "InviteToken::verify binds id||expires_at; consume_invite matches on id; landing workspace == invites.workspace_id."

  signature:
    source_of_truth: "InviteToken.signature — HMAC(SESSION_SECRET, invite_id||expires_at) (crates/foundry-auth/src/lib.rs:354-385)"
    consumers:
      - "URL query param `sig` (url-encoded)"
      - "hidden form field on the set-password page"
      - "POST re-verification (defense-in-depth before the DB hit)"
    owner: "foundry-auth (InviteToken) — shipped, reused verbatim"
    integration_risk: "HIGH — tamper detection. The sig rendered on GET must be the sig POST re-verifies. The DB row is the primary single-use control; HMAC is defense-in-depth (rejects obviously-tampered URLs without a DB hit)."
    validation: "InviteToken::verify(id, expires_at, sig, secret) returns Ok only for an untampered (id, expiry) pair."

  expires_at:
    source_of_truth: "invites.expires_at — set at provisioning to now + 7 days (bootstrap.rs:244, the provision tx)"
    consumers:
      - "bound into the HMAC payload (invite_payload)"
      - "GET liveness check (expires_at > now)"
      - "consume_invite WHERE expires_at > now (race-safe expiry guard)"
    owner: "provisioning (writer) -> invite-accept-flow (reader)"
    integration_risk: "MEDIUM — expiry is bound into the signature, so an attacker cannot extend it without breaking the HMAC. Default 7d is already shipped (see requirements NFR-1 for the exact-value rationale)."
    validation: "InviteToken::verify rejects a mutated expiry; consume_invite re-checks expiry inside the TX."

  workspace_name:
    source_of_truth: "workspaces row joined via invites.workspace_id"
    consumers: ["set-password page heading (context that builds trust on arrival)"]
    owner: "provisioning -> invite-accept-flow (read-only display)"
    integration_risk: "LOW — display-only; do not leak it on the refusal page (would become an enumeration oracle)."
    validation: "Shown ONLY on the set-password page for a live valid invite; NEVER on the uniform refusal page."

  csrf_token:
    source_of_truth: "csrf::generate_token + double-submit cookie (crates/foundry-app/src/csrf.rs — shipped)"
    consumers: ["hidden _csrf field on the set-password form", "CSRF middleware on POST /invites/accept"]
    owner: "foundry-app web tier — shipped, reused"
    integration_risk: "MEDIUM — the accept page is a PUBLIC (signed-out-accessible) route; CSRF still applies to its POST. The page is NOT behind the instance-admin gate (the invitee isn't signed in yet)."
    validation: "POST without a matching double-submit token is refused by the shipped middleware before any consume."

  password_hash:
    source_of_truth: "foundry_auth::hash_password (argon2id, OWASP params — crates/foundry-auth/src/lib.rs:319, shipped, reused verbatim)"
    consumers: ["users row update during consume_invite TX", "verify_password at later sign-in (signin.rs:106)"]
    owner: "foundry-auth — shipped"
    integration_risk: "MEDIUM — replaces the operator-never-sees generated hash created at provisioning; written in the SAME TX as the consume so neither happens alone."
    validation: "After accept, verify_password(chosen_pwd, stored_hash) succeeds at sign-in."

  invites_row_used_at:
    source_of_truth: "invites.used_at — the single-use marker (SEE OPEN DECISION: confirm this column exists or is added)"
    consumers: ["consume_invite single-use guard (UPDATE ... WHERE used_at IS NULL)", "GET liveness re-render guard"]
    owner: "invite-accept-flow (this feature introduces the consume semantics)"
    integration_risk: "HIGH — single-use atomicity. The UPDATE's WHERE used_at IS NULL AND expires_at > now is the race guard: exactly one concurrent consumer updates 1 row."
    validation: "Two concurrent consumes -> exactly one updates 1 row; the other sees 0 rows -> uniform refusal."

  session:
    source_of_truth: "session layer SESSION_KEY_USER_ID -> SessionUser{user_id, workspace_id} (crates/foundry-app/src/session.rs + signin.rs — shipped)"
    consumers: ["all subsequent authenticated requests", "the 302 landing on /"]
    owner: "foundry-app web tier — shipped, reused"
    integration_risk: "MEDIUM — established only after a successful consume; auto sign-in means no separate login step (user-ratified decision 3)."
    validation: "After accept, the redirect to / is authenticated and resolves the correct workspace."

  workspace_id:
    source_of_truth: "resolve_active_workspace(user) — shipped membership seam (store; used at signin.rs:149)"
    consumers: ["the SessionUser.workspace_id", "dashboard tenant scoping", "all workspace-scoped reads"]
    owner: "multi-workspace-tenancy — shipped, reused"
    integration_risk: "HIGH — the landed workspace_id MUST equal invites.workspace_id for the consumed invite. Priya must never land on another tenant."
    validation: "Landed workspace_id == invites.workspace_id; Priya sees only that tenant's data."
```

## Consistency checks (for DISTILL / DELIVER)

1. Does every `${variable}` in the TUI mockups have a documented source above? **Yes** — all 9 tracked.
2. The `sig` rendered on GET == the `sig` POST re-verifies. (invite_id + signature, risk HIGH)
3. The consumed `invite_id` == the row `insert_invite` created at provisioning. (risk HIGH)
4. The landed `workspace_id` == `invites.workspace_id`. (risk HIGH — tenant isolation)
5. The uniform refusal page leaks NONE of: `workspace_name`, account existence, invite-state reason.
6. `password_hash` write and `used_at` consume are in the SAME transaction (atomicity).
