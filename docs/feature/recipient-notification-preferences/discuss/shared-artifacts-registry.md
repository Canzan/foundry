# Shared Artifacts Registry — recipient-notification-preferences (v1 = recipient unsubscribe)

Every value that flows across the journey (emit → click → confirm → suppress; and status → resubscribe →
observe), its single source of truth, and its consumers. The **unsubscribe token**, the **unsubscribe state**,
and the **suppression decision/metric** are the security-and-privacy-critical ones. This extends the shipped
notifier + `InviteToken` + non-enumerable-refusal seams; the DELTA artifacts (`unsubscribe_token`,
`unsubscribe_row`, `suppression_decision`, `suppression_metric`) are the recipient-unsubscribe surface.

```yaml
shared_artifacts:
  unsubscribe_token:
    source_of_truth: "UnsubscribeToken::new(email_lower, workspace_id, SESSION_SECRET) — an HMAC-signed token binding email + workspace_id, modelled on InviteToken (foundry-auth/src/lib.rs:354-390; sign :251, constant-time verify :260). Payload analogue of invite_payload '{id}|{unix_ts}' -> '{email_lower}|{workspace_id}'. Expiry stance ODD-1."
    consumers:
      - "the unsubscribe link embedded in the two suppressible emails (workspace_invite bootstrap.rs:266, member_invite member_invites.rs:204)"
      - "the public /unsubscribe route (GET verify + POST re-verify)"
    owner: "this feature (the token) — signed with the app SESSION_SECRET like InviteToken"
    integration_risk: "HIGH (security) — the token embedded at emit MUST verify at the route, and a tampered/unknown token MUST yield the uniform non-enumerable refusal (never a differential response). Verification is constant-time; the server re-derives (email_lower, workspace_id) from the token, never trusting client input."
    validation: "A token minted for (sam.okafor@acme.example, Northwind) verifies only for that pair; flipping any byte or swapping the workspace_id yields the identical refusal as a made-up token, and records nothing (non-enumerable @property)."

  unsubscribe_row:
    source_of_truth: "notification_unsubscribes(email_lower, workspace_id, unsubscribed_at) — the NEW 0014 migration (latest shipped is 0013_issue_change_events.sql); follows the reset_tokens shape (0002_sessions_and_reset.sql:20-28). Absence of a row = SUBSCRIBED (default, BR-7). Single source of subscription state."
    consumers:
      - "the suppression filter (U4) — suppresses a suppressible event iff a row exists for the pair"
      - "the signed-in status page (S1) — Muted iff a row exists"
      - "resubscribe (S2) — CLEARS the row, returning to subscribed"
    owner: "this feature (the unsubscribe state) — Store methods insert/delete/exists on impl Store (pattern: insert_reset_token, foundry-store/src/lib.rs:980)"
    integration_risk: "HIGH — the row that suppression reads MUST be exactly what confirm wrote and what the status page shows and resubscribe clears; any divergence means a muted recipient still gets mail, or a wrong status, or a resubscribe that doesn't take. Key is (email_lower, workspace_id); email normalisation must match find_user_by_email(email_lower). Shape/keying ODD-4."
    validation: "Confirm writes exactly one row per pair (idempotent, BR-8); suppression, status, and resubscribe all read/act on that same row; resubscribe deletes it and delivery resumes."

  suppression_decision:
    source_of_truth: "derived in the delivery path: event ∈ suppressible allow-list {workspace_invite, member_invite} (BR-1) AND exists ${unsubscribe_row}(email_lower, workspace_id). MANDATORY events (password_reset, password_changed, member_removed) skip the check entirely (BR-3). Hook point (notify.rs:237 vs emit-site) is ODD-3."
    consumers:
      - "the deliver()/skip branch in the notifier (notify.rs loop :244-280)"
      - "the suppression metric (never propagated to the request; notify() stays INFALLIBLE)"
    owner: "the suppression filter (this feature) over the shipped Notifier"
    integration_risk: "HIGH (safety) — a mandatory event must NEVER be suppressed (mandatory > unsubscribe, BR-3). The suppressible set is a bounded ALLOW-LIST so a new event is delivered-by-default. A suppression-lookup error in the INFALLIBLE notify() must not worsen today's contract (ODD-3, R5)."
    validation: "For an unsubscribed pair, a suppressible event is skipped and a mandatory event is delivered; the never-suppress @property reds if any mandatory event is suppressed."

  suppression_metric:
    source_of_truth: "a suppressed outcome on foundry_notification_deliveries_total{provider,event,outcome} (notify.rs:39, 291-297) OR a sibling counter foundry_notification_suppressions_total{event} — ODD-5. Bounded-label; labels carry event (+ at most workspace), NEVER recipient email or token."
    consumers:
      - "the existing /metrics Prometheus sidecar (metrics_server)"
      - "operator/compliance opt-out-volume view (US-07) + DEVOPS alert thresholds (follow-up)"
    owner: "this feature (the observability seam) -> reuses the shipped metrics facade + sidecar"
    integration_risk: "HIGH (privacy) — a recipient email or token in a label is a PII leak (R4). Labels MUST stay bounded and PII-free; the cardinality/PII guard fails closed. The suppressed count for mandatory events is always 0 (US-02), observably."
    validation: "N suppressions -> suppressed count == N by event; a full /metrics scrape + logs contain no recipient email/token; mandatory suppressed series == 0 (no-PII @property)."

  member_identity:
    source_of_truth: "SessionUser (session.rs; carries user_id + workspace_id) -> the member's own email_lower via find_user_by_email / the users table. NEVER a client-supplied email (NFR-6)."
    consumers:
      - "the per-workspace status query (S1)"
      - "the resubscribe POST scope (S2)"
    owner: "foundry-app web tier — reuses the shipped session"
    integration_risk: "HIGH (authz) — the identity the status page reads MUST equal the identity resubscribe mutates; deriving identity from the request instead of the session would let a member view one scope and mutate another. Least-privilege: only the member's own pairs, only their workspaces (NFR-6, BR-6)."
    validation: "A request naming another user's email returns only the session member's own scope; resubscribe only ever clears a row for the session member's own (email_lower, workspace_id)."

  workspace_status_list:
    source_of_truth: "for each workspace the member belongs to (membership lookups is_team_member/is_workspace_admin, foundry-store/src/lib.rs:1048,1955): Muted iff exists ${unsubscribe_row}(email_lower, workspace_id), else Subscribed"
    consumers:
      - "the settings page render (S1)"
      - "the per-row resubscribe control (S2)"
    owner: "this feature (the signed-in surface) over shipped membership lookups"
    integration_risk: "MEDIUM — lists ONLY the member's own workspaces (no cross-recipient enumeration); status derived from the single source ${unsubscribe_row}. Multi-workspace interaction (an email in several workspaces) is ODD-7."
    validation: "The page lists exactly the member's workspaces with a status matching the unsubscribe table; muting one workspace does not change another's status (FR-9)."

  unsubscribe_link_url:
    source_of_truth: "${public_url}/unsubscribe?token=${unsubscribe_token}, where public_url = FOUNDRY_PUBLIC_URL -> AppState.public_url (main.rs:122) — the SAME host the shipped invite accept-link already uses"
    consumers:
      - "the unsubscribe link in the two suppressible emails"
      - "the public /unsubscribe route registration (lib.rs public cluster :356-409)"
    owner: "foundry-app configuration (public_url, shipped) + this feature (the /unsubscribe path)"
    integration_risk: "LOW — reuses the shipped public_url host (already exercised by invite links); the path joins the existing public token-route cluster (/invites/accept :371-374)."
    validation: "The unsubscribe link host equals the configured public_url (as the invite accept-link already does); the route resolves under the public, CSRF-screened, session-layer-covered cluster."

  csrf_token:
    source_of_truth: "the shipped double-submit CSRF token — foundry_csrf cookie issued by ensure_csrf_cookie (csrf.rs:54), validated by csrf_middleware (csrf.rs:137), enforced by the layer at lib.rs:536-539"
    consumers:
      - "the public unsubscribe CONFIRM POST (U3)"
      - "the signed-in RESUBSCRIBE POST (S2)"
    owner: "foundry-app (shipped CSRF middleware) — reused verbatim"
    integration_risk: "MEDIUM — both new state-changing POSTs MUST sit under the CSRF layer; a POST without a valid token is 403 (NFR-5). The public confirm form must be served with the CSRF cookie so the POST validates."
    validation: "A POST to /unsubscribe (confirm) or /account/notifications/resubscribe without a valid _csrf is refused 403 and changes no state; the shipped double-submit check applies unchanged."
```

## Consistency checks (for DISTILL / DELIVER)

1. Does every `${variable}` in the journey mockups have a documented source above? **Yes** — all 7 tracked
   (`unsubscribe_token`, `unsubscribe_row`, `suppression_decision`, `suppression_metric`, `member_identity`,
   `workspace_status_list`, `unsubscribe_link_url`) plus the shared `csrf_token`.
2. **Token round-trip**: the token embedded at emit == what the route verifies; a tampered token yields the
   uniform non-enumerable refusal and records nothing. (HIGH — security)
3. **State single source**: confirm writes `${unsubscribe_row}`; suppression, the status page, and resubscribe
   all act on that same row; nothing else records or reads subscription state. (HIGH)
4. **Mandatory never suppressed**: `password_reset` / `password_changed` / `member_removed` are always
   delivered, even for a fully-unsubscribed pair; suppressible set is a bounded allow-list. (HIGH — the crux)
5. **Prefetch safety**: a bare GET never writes `${unsubscribe_row}`; only a valid-token, CSRF-checked confirm
   POST does. (HIGH — security)
6. **Least-privilege**: `${member_identity}` comes from the session only; a member can never read or write
   another recipient's state. (HIGH — authz)
7. **No-PII observability**: `${suppression_metric}` carries `event` (+ at most `workspace`), never a recipient
   email or token; guard fails closed on a PII/unbounded label. (HIGH — privacy)
8. **Backwards-compat**: with the `0014` table empty, the notifier behaves exactly as today; the filter only
   removes a suppressible delivery for an unsubscribed pair. (HIGH — regression guard)
</content>
