# Acceptance Criteria — recipient-notification-preferences (v1 = recipient unsubscribe)

All criteria are observable and testable. Given/When/Then scenarios live in `journey-unsubscribe.feature` and
per-story in `user-stories.md`; this file is the consolidated, traceable AC index for DISTILL. Every AC traces
to a functional requirement (FR-1..10) or a non-functional requirement (NFR-1..8) and a business rule
(BR-1..8) in `requirements.md`, and every story traces to an outcome KPI in `outcome-kpis.md`.

## US-01 — Stop a workspace's invite emails with one click (Walking Skeleton)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-01.1 | The `workspace_invite` email carries a signed unsubscribe link binding recipient email + workspace, usable logged-out | FR-1, NFR-1 |
| AC-01.2 | Confirming the link records an opt-out for `(email_lower, workspace_id)` and shows a confirmation naming the workspace + reassuring about security mail | FR-2 |
| AC-01.3 | After unsubscribe, a subsequent `workspace_invite` to that pair is not delivered by any provider (suppressed) | FR-3 |
| AC-01.4 | An unsubscribe in one workspace does not suppress deliveries in another for the same email | FR-9, BR-2 |
| AC-01.5 | Unsubscribing an already-unsubscribed pair is an idempotent no-op success — no duplicate row, no error | BR-8 |
| AC-01.6 | With no unsubscribe rows, `workspace_invite` delivery is unchanged from today | NFR-7 |

## US-02 — Never lose a security-critical notification, even when unsubscribed

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-02.1 | `password_reset`, `password_changed`, `member_removed` are delivered regardless of any unsubscribe state | FR-4, BR-3 |
| AC-02.2 | A mandatory event is never counted `suppressed` under any unsubscribe configuration | NFR-3 |
| AC-02.3 | The suppressible set is a bounded allow-list ({`workspace_invite`, `member_invite`}); a new event is not suppressible by default | BR-1 |
| AC-02.4 | A regression that suppressed a mandatory event reds a dedicated never-suppress `@property` litmus | NFR-3 |
| AC-02.5 | The confirmation page's promise ("you'll still receive security-critical notifications") is literally true | FR-2, FR-4 |

## US-03 — A tampered or unknown link is safely refused without leaking who exists

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-03.1 | A tampered / malformed / unknown token returns the uniform non-enumerable refusal (fixed status + byte-identical body) and records nothing | NFR-1, FR-5, BR-4 |
| AC-03.2 | The refusal response is indistinguishable between a real recipient and a non-existent address | NFR-1 |
| AC-03.3 | Token verification uses a constant-time comparison (reusing `foundry-auth`'s `verify`) | NFR-1 |
| AC-03.4 | A bare GET of a valid unsubscribe URL creates no unsubscribe row; only an explicit confirm mutates state | NFR-2 |
| AC-03.5 | No unsubscribe token or recipient email appears in logs or error output for a refused request | NFR-1, NFR-4 |

## US-04 — The same one-click unsubscribe works for member-invite emails

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-04.1 | The `member_invite` email carries the same signed unsubscribe link (recipient + workspace) as `workspace_invite` | FR-1 |
| AC-04.2 | `member_invite` is in the suppressible allow-list; it is suppressed for an unsubscribed `(email, workspace)` | FR-3, BR-1 |
| AC-04.3 | A single opt-out row suppresses both suppressible events for that pair (one confirm covers both) | FR-3, BR-2 |
| AC-04.4 | Mandatory events remain delivered after an unsubscribe made via a `member_invite` link | FR-4, NFR-3 |
| AC-04.5 | With no unsubscribe rows, `member_invite` delivery is unchanged from today | NFR-7 |

## US-05 — See my per-workspace notification status when signed in

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-05.1 | `GET /account/notifications` lists each workspace the signed-in member belongs to with a Subscribed/Muted status | FR-6 |
| AC-05.2 | Status reflects whether `(member's email_lower, workspace_id)` has an unsubscribe row | FR-6, BR-7 |
| AC-05.3 | Only the signed-in member's own workspaces and own status are shown; identity is from the session, not client input | NFR-6, BR-6 |
| AC-05.4 | The page requires an authenticated session and is reachable from the account/nav surface | NFR-6 |
| AC-05.5 | The page meets WCAG 2.1 AA basics (labelled controls, status as text, keyboard-navigable) | NFR-8 |

## US-06 — Resubscribe a workspace I previously muted

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-06.1 | `POST /account/notifications/resubscribe` clears the unsubscribe row for `(member's email_lower, workspace_id)` and shows Subscribed | FR-7, BR-7 |
| AC-06.2 | After resubscribe, a subsequent suppressible notification to that pair is delivered again | FR-3, FR-7 |
| AC-06.3 | Resubscribing an already-subscribed pair is an idempotent no-op success | BR-8 |
| AC-06.4 | The POST is CSRF-protected; a request without a valid token is rejected `403` and changes no state | NFR-5, BR-5 |
| AC-06.5 | A member can resubscribe only their own pairs, scoped to workspaces they belong to | NFR-6, BR-6 |

## US-07 — See how much notification opt-out is happening, without exposing who

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-07.1 | A suppressed suppressible-delivery increments a bounded-label suppression count on the existing `/metrics` sidecar | FR-8, NFR-4 |
| AC-07.2 | Suppression labels carry `event` (∈ bounded catalog) and at most `workspace` — never recipient email or token | NFR-4 |
| AC-07.3 | A full `/metrics` scrape and the delivery logs contain no recipient PII for a suppressed delivery | NFR-4 |
| AC-07.4 | The suppressed count for every mandatory event is always 0 (US-02 invariant, observable) | NFR-3, NFR-4 |
| AC-07.5 | A label/cardinality guard fails closed on an unbounded or PII label | NFR-4 |

## Property-shaped criteria (tag `@property` for DISTILL)

- **@property mandatory never suppressed**: for a recipient unsubscribed in every workspace they belong to, every
  `password_reset` / `password_changed` / `member_removed` is delivered and none is counted `suppressed`;
  reverting the allow-list guard reds the litmus. (AC-02.1, AC-02.2, AC-02.4, AC-04.4, AC-07.4)
- **@property non-enumerable + no-mutate refusal**: a tampered / unknown token, and a valid token that is only
  fetched (not confirmed), all leave state unchanged and return responses that reveal no existence; invalid
  responses are byte-identical between a real and a non-existent address. (AC-03.1, AC-03.2, AC-03.4)
- **@property prefetch safety**: a bare GET of a valid unsubscribe URL records no opt-out; only an explicit
  confirm (POST) mutates state. (AC-03.4)
- **@property least-privilege**: a signed-in member can only ever read or write their own subscription state,
  scoped to workspaces they belong to, with identity from the session. (AC-05.3, AC-06.5)
- **@property no-PII observability**: across any suppression, no recipient email or token appears in any metric
  label or log line, and the label domains stay bounded. (AC-07.2, AC-07.3, AC-07.5)
- **@property additive backwards-compat**: with an empty unsubscribe table, every existing delivery flow behaves
  byte-for-byte as before this feature. (AC-01.6, AC-04.5)

## Traceability

Every AC above maps to a functional (FR-1..10) or non-functional (NFR-1..8) requirement and a business rule
(BR-1..8) in `requirements.md`, and every story traces to an outcome KPI in `outcome-kpis.md`. No orphan AC.
The v1 boundary (US-01..US-04) is fully covered by AC-01.\*, AC-02.\*, AC-03.\*, AC-04.\*; US-05..US-07 add the
signed-in management surface and operator visibility over the same guarantees (non-enumerability, mandatory
exemption, PII-free observability, least-privilege).
</content>
