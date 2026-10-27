# ADR-EIP-007: Project emails are queued deliveries sent through a narrow EmailTransport port that the shipped SMTP and email-API providers also implement, with access, mute and choice checked at send time and a per-choice signed stop link that supports one-click

## Status

Accepted (2026-10-27, by the user at the end of external-integration-plugins DELIVER).
Implemented (finalized 2026-10-27 at `8c681b2`; not yet released). DD-2 was amended on
2026-10-27 (FU-11, fixed in `c7694b2`). Departures from this record are listed in `docs/evolution/2026-10-27-external-integration-plugins.md` (Key decisions).

Proposed (2026-10-21, external-integration-plugins DESIGN). Deliverable B (slice 05).

## Context

- US-EIP-10 / -11; OD-8 (one email per event), OD-10 (nothing on by default), OD-11 (never about
  one's own action), D-9 (`/account/settings` hosts the section; the workspace mute is the master
  switch), D-14 (project emails suppressible, security emails never), SC-5 (access at send time),
  OD-7 (queued and retried, not the in-request `notify()`).
- Shipped facts: `Notifier::notify` is infallible and returns nothing; `SmtpProvider::deliver` and
  `EmailApiProvider::deliver` return `Result<(), DeliveryError>` with a transient/permanent class; the
  `Notification` struct carries no headers. `UnsubscribeToken` (foundry-auth) is an HMAC over
  `unsub|v1|{email_lower}|{workspace_id}` with `SESSION_SECRET`. **No `List-Unsubscribe` header is
  sent today, and `POST /unsubscribe` is CSRF-checked**, so a mail client's RFC 8058 one-click POST
  (no cookie, body `List-Unsubscribe=One-Click`) cannot succeed against it (Changed Assumption DA-1).

## Options considered

1. **Send through `Notifier::notify`** with a new event. Rejected: no outcome, so no retry or history;
   it would also fan out to every configured provider, including the env `WebhookProvider`, which
   would then receive project mail it has never carried (SC-7).
2. **Call `NotificationProvider::deliver` directly** on the email-capable providers. Workable for the
   outcome, but the `Notification` shape has no headers for one-click, and widening it changes the
   shipped port.
3. **A narrow `EmailTransport` port (chosen):** `send(OutboundEmail) -> Result<(), DeliveryError>`,
   where `OutboundEmail` carries recipient, subject, text body and an optional unsubscribe pair
   (`List-Unsubscribe` URL, `List-Unsubscribe-Post`). `SmtpProvider` and `EmailApiProvider` implement
   it over their existing transport objects; their `NotificationProvider` impls are unchanged.

## Decision

1. **Choices:** `integration_email_choices (user_id, project_id, event, scope)`; `scope` is `all` or
   `mine` ("issues I created or commented on": the member authored the issue or has a comment on it
   that is not deleted, US-EIP-10 AC). Saving a choice requires the member to see the project
   (`TeamAccess::can_view`) and an email transport to exist (else K-13 and nothing saved).
2. **Dispatch** creates one email delivery per (member, event) for members with a matching choice in the
   event's project, excluding the actor (OD-11). Scope and every other condition are re-checked at send.
3. **Send-time checks**, in order, each ending the delivery as `skipped` with a reason: the choice still
   exists; the member can still view the project; the workspace is not muted for the member's address
   (`is_unsubscribed`, the shipped table); the scope still matches; the issue or comment still exists.
   Only then is the email composed (D-3, Summary with comment text for New comment) and sent.
4. **Transport selection:** the composition root wires the first configured `smtp` or `email_api`
   provider from `NOTIFICATION_PROVIDERS` as the `EmailTransport`. None configured → the email kind is
   unavailable and the settings section shows K-13. `log` is accepted in test builds only.
5. **Stop link:** `ProjectEmailStopToken` in `foundry-auth`, HMAC over
   `stop|v1|{user_id}|{project_id}|{event}` with `SESSION_SECRET`, the `UnsubscribeToken` shape. No
   expiry (DD-7): an unsubscribe link must keep working for as long as the email exists.
   - `GET /email/stop?t=…&sig=…`: verify, render the confirm (prefetch-safe, changes nothing);
   - `POST /email/stop`: browser confirm, CSRF-checked, deletes that one choice;
   - `POST /email/stop/one-click?t=…&sig=…`: RFC 8058 target, CSRF-exempt (signature-authenticated,
     idempotent, can only switch one choice off), answers 200 with no body;
   - every project email carries `List-Unsubscribe: <{one-click URL}>` and
     `List-Unsubscribe-Post: List-Unsubscribe=One-Click`, plus the K-12 footer links;
   - an invalid signature gets the shipped uniform refusal page.
6. **Mute stays the master switch** and security emails are untouched: project email never passes
   through the suppression gate, and the account path is not modified.

## Consequences

- Positive: project email gets the same retry, history and outcome model as chat routes, with no
  change to the five account notifications or `NOTIFICATION_PROVIDERS` (SC-7).
- Positive: one-click unsubscribe actually works for project email; the shipped workspace-level
  `/unsubscribe` is unchanged (giving it one-click headers is a separate follow-up).
- Negative: two new CSRF exemptions in total for the feature (this one and the Slack events endpoint);
  a unit test pins the exact exempt set.
- Negative: an SMTP relay that accepts and later bounces is reported "Delivered"; bounce handling is
  out of scope.
- Negative: `SESSION_SECRET` rotation invalidates outstanding stop links (the shipped unsubscribe
  links share the property).
