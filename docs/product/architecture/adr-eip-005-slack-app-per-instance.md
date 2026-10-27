# ADR-EIP-005: Each instance runs its own Slack app from a Foundry-rendered manifest; install is a session-bound OAuth v2 flow; uninstall is learnt from refusals and from signed Slack events

## Status

Accepted (2026-10-27, by the user at the end of external-integration-plugins DELIVER).
Implemented (finalized 2026-10-27 at `8c681b2`; not yet released). DD-2 was amended on
2026-10-27 (FU-11, fixed in `c7694b2`). Departures from this record are listed in `docs/evolution/2026-10-27-external-integration-plugins.md` (Key decisions).

Proposed (2026-10-21, external-integration-plugins DESIGN). Builds on OD-3 (accepted: operator
creates the app from a Foundry manifest; client id, client secret and signing secret are
environment settings validated at boot).

## Context

- Slack's approval returns to one fixed redirect address per app, so every self-hosted instance
  needs its own Slack app (R2). Copy K-2 points the admin at "How to set up Slack".
- US-EIP-01: the return must refuse a forged, replayed or over-10-minute `state` (K-3); only a
  workspace admin reaches the page or the return (uniform 404 otherwise).
- US-EIP-06: an uninstalled app or revoked token must pause routes on the first refusal and email
  admins within 5 minutes; reconnecting to a different Slack workspace changes nothing.
- US-EIP-04 / R5: user text must never ping.
- Session cookies are `SameSite=Lax` (`session.rs`), so the session survives Slack's top-level GET
  redirect back. `FOUNDRY_PUBLIC_URL` already exists in `main.rs`.
- The OIDC flow carries its state in a signed cookie (ADR-OIDC-002) because its start is
  unauthenticated; the Slack install always starts signed in.
- D-10 / Out of Scope: no inbound Slack features (commands, unfurls, buttons).

## Options considered

- **State carrier:** (a) the signed-in `tower-sessions` session (chosen: bound to the session by
  construction, single-use by take-and-remove, no new cookie); (b) the ADR-OIDC-002 signed cookie
  (works, but adds a second carrier for a flow that never starts signed out); (c) a database table
  (unnecessary).
- **Uninstall detection:** (a) refusals only (`token_revoked`, `invalid_auth`, `account_inactive`
  on the next post); (b) refusals plus Slack's Events API `app_uninstalled` and `tokens_revoked`
  posted to a signed endpoint (chosen, DD-1); (c) periodic `auth.test` polling (adds traffic for
  nothing the events do not already give).
- **Public channels without an invite:** (a) request `chat:write.public` (chosen, DD-3); (b) make
  leads `/invite @Foundry` to every channel; (c) `channels:join` and auto-join (writes a join message
  into the channel and joins silently, a surprise for the channel).

## Decision

1. **Operator settings:** `SLACK_CLIENT_ID`, `SLACK_CLIENT_SECRET`, `SLACK_SIGNING_SECRET`: all three
   or none; a partial set is `health.startup.refused` (the OIDC and machine-key precedent). Slack is
   offered only when all three and `FOUNDRY_SECRETS_KEY` are set (ADR-EIP-004); otherwise
   `/workspace/integrations` shows K-2. `SLACK_API_BASE_URL` and `SLACK_AUTHORIZE_URL` (defaults
   `https://slack.com/api`, `https://slack.com/oauth/v2/authorize`) are operator settings documented
   as test seams; acceptance points them at the HTTP double (SC-10). The Slack client is built with
   `redirect::Policy::none()`. It may use the environment's HTTP proxy: its host is
   operator-fixed, never user-supplied, so a proxy opens no SSRF path. Boot probes configuration **shape only**; readiness never depends on Slack (the
   ADR-OIDC-003 posture).
2. **Manifest:** `GET /workspace/integrations/slack/setup` (workspace admins; the K-2 link) renders
   the Slack app manifest with this instance's redirect URL
   (`{FOUNDRY_PUBLIC_URL}/workspace/integrations/slack/return`) and events URL filled in, the bot
   scopes, and the four setup steps. The same manifest template is the operator documentation's
   source, so the two cannot drift (DD-9).
3. **Bot scopes (least for slices 01-03):** `chat:write`, `chat:write.public`, `channels:read`,
   `groups:read`. No user-token scopes, no history or message reading.
4. **Install:** `GET /workspace/integrations/slack/install` (admin) stores
   `{state (32 random bytes), workspace_id, issued_at}` in the session and redirects to Slack.
   `GET /workspace/integrations/slack/return` (admin, else uniform 404) takes the state out of the
   session (single use), compares in constant time, checks the active workspace equals the stored
   one and `issued_at` is under 10 minutes; any failure or `error=access_denied` shows K-3 and stores
   nothing. On success it exchanges the code (`oauth.v2.access`), reads team id, team name, bot user
   id and app id, and **persists `app_id` and `bot_user_id` on the connection** (columns of
   `integration_connections`). It calls `auth.test` for the domain, seals the bot token (AAD bound to the connection
   row) and writes the connection plus an audit record in one transaction.
5. **Reconnect:** the same flow; the returned team id must equal the stored one, else nothing is
   written and the US-EIP-06 "different Slack workspace" message is shown.
6. **Events endpoint:** `POST /integrations/slack/events`, public, CSRF-exempt (one new entry in
   `is_exempt_path`), verified before parsing: `X-Slack-Request-Timestamp` within 5 minutes and
   `v0=HMAC-SHA256(signing_secret, "v0:{ts}:{raw body}")` compared in constant time; anything else
   answers 401 with no body. It handles `url_verification`, `app_uninstalled` and `tokens_revoked`
   only. It resolves the affected connections by `external_id = team_id` of the envelope, and
   keeps only those whose stored `app_id` equals the envelope's `api_app_id`, so an uninstall from one Slack workspace never touches
   a Foundry workspace connected to another; no match answers 200 and does nothing. Matches go
   through the same "connection lost" transition a refusal causes; unknown event types answer 200
   and do nothing. Slack retries and duplicate events are idempotent, because the transition is a
   compare-and-set from `connected`. Gold tests: two Foundry workspaces on two Slack teams,
   uninstall one; a duplicated event; a stale timestamp; a bad signature.
7. **Disconnect:** calls `apps.uninstall` with the client credentials and the token, then forgets the
   sealed token whatever Slack answers, marks the connection `disconnected`, cancels pending
   deliveries (D-8: a disconnect queues nothing) and audits. The confirm counts routes and projects.
8. **Delivery:** `chat.postMessage` with the ADR-EIP-001 Slack encoding, `unfurl_links: false`,
   `unfurl_media: false`. The classifier reads the JSON `ok` field, never the HTTP status alone
   (Slack answers 200 with `ok: false`). The classifier uses closed lists:
   - `ratelimited` and HTTP 429 → `Transient` with `Retry-After`;
   - `channel_not_found`, `is_archived`, `not_in_channel` → `RouteRefused`;
   - `token_revoked`, `invalid_auth`, `account_inactive`, `not_authed` → `ConnectionRefused`;
   - everything else → `Transient`, bounded by the 24-hour give-up: 5xx, timeouts, non-JSON
     answers, and any other `ok: false` code, including `internal_error`, `fatal_error`,
     `service_unavailable`, `request_timeout` and codes Slack adds later.
   An unrecognised fault therefore never parks a route. DELIVER verifies the code lists against
   Slack's published method error tables and records them as fixtures.
9. **Connection lost** is one compare-and-set from `connected` to `lost`; the replica that wins it
   sends the new suppressible account notification (D-15) to each workspace admin through the shipped
   `Notifier` and the K-10 banner appears. Routes read "Paused: connection lost" by derivation.

## Consequences

- Positive: an operator sets three variables once; admins never copy a secret (EIP-KPI-2).
- Positive: an uninstall is visible within seconds when Slack can reach the instance, and on the
  next delivery when it cannot (dev and prod are public; a private instance falls back to refusals).
- Negative: one public, CSRF-exempt POST endpoint, defended by the signing secret alone; it changes
  only the connection's state, never content.
- Negative: the admin alert rides the best-effort `Notifier` (ADR-007), so a relay outage at that
  moment loses it; the banner and route statuses remain.
- Negative: `chat:write.public` lets the app post to any public channel; posting still happens only
  where a Manage user created a route.
- Contract tests: the Slack double is scripted from recorded Slack answers (`ok: false` codes, 429
  with `Retry-After`, an HTML 503 from a proxy); platform-architect should add a consumer-driven
  contract (Pact or a recorded-fixture suite) for `oauth.v2.access`, `auth.test`,
  `conversations.list`, `conversations.info`, `chat.postMessage` and `apps.uninstall`.
