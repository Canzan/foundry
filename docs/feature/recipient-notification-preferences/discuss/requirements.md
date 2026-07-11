# Requirements — recipient-notification-preferences (v1 = recipient unsubscribe)

## Context

The just-shipped `notification-delivery-providers` feature made Foundry's notifications actually deliver:
one emitted notification now fans out to every configured provider (log / SMTP / webhook / hosted email API),
best-effort and observable (`docs/evolution/2026-07-11-notification-delivery-providers.md`). That feature
**deliberately carved out recipient preferences** into this named successor and built the delivery mechanism so
preferences would have something concrete to route over.

Now that notifications reliably arrive, a new problem appears for the **person on the receiving end**. Today the
whole catalog is transactional and there is **no way to opt out of anything**: a recipient who keeps getting
workspace-invite reminders (`workspace_invite`) or member-invite emails (`member_invite`) they didn't ask for
has no lever to quiet them — and many recipients are **invitees with no account yet**, identified only by their
email address, so an account-gated preference screen would not even reach them.

This feature builds the **recipient unsubscribe mechanism end-to-end** and proves it on the two suppressible
events (the invites), while holding the **security-critical events exempt**. A suppressible notification carries
a **signed unsubscribe link** that works **logged-out**; one click (plus a confirm) records a per-workspace
opt-out keyed on `(email_lower, workspace_id)`; the notifier then **suppresses future suppressible notifications**
to that pair — but **always delivers** the security-sensitive ones (`password_reset`, `password_changed`,
`member_removed`). Account holders additionally get a **signed-in settings page** to see their per-workspace
subscription status and **resubscribe**.

The value is three-sided:

1. **Recipients** ("Sam the invitee/member") can stop non-essential workspace notifications with **one click from
   the email**, without an account, **without losing security-critical alerts**.
2. **Account-holding members** ("Maria") get a settings page to see and manage their per-workspace subscription
   status and resubscribe.
3. **Operators / compliance** ("Olivia") get unsubscribe **honored** (deliverability / list-hygiene) and a
   **measurable guarantee** that security events are never suppressed — visible as suppression volume without
   exposing who opted out.

> **Honest framing.** Because today's entire catalog is transactional, v1 is really building the unsubscribe
> **mechanism** — token, state, public route, suppression filter, signed-in surface — and **proving it on the
> invite events**, with the security events **explicitly exempt**. It is not a rich per-category preference
> centre. That richer surface (per-category, digests, quiet-hours) is out of scope (see Scope).

## Scope (v1 boundary = slices 01–04; 05–07 fast-follow in this feature)

- **In scope**:
  - A **signed `UnsubscribeToken`** binding `email` + `workspace_id` (modelled on the shipped `InviteToken`
    HMAC pattern), embedded as an **unsubscribe link** in the two **suppressible** notifications
    (`workspace_invite`, `member_invite`) — working **logged-out**.
  - A **public unsubscribe route** (token-verified) that records a per-workspace opt-out and shows a
    confirmation; a **tampered / invalid / unknown token** yields the **uniform non-enumerable refusal**.
  - A **new store table** for unsubscribe state keyed on `(email_lower, workspace_id)` — **the one migration
    this feature adds** (`0014`).
  - A **suppression filter** in the notifier delivery path so a future **suppressible** notification to an
    unsubscribed `(email, workspace)` is **not delivered**; **mandatory security events are always delivered**.
  - **Suppression observability** — a suppressed delivery is counted, with **no recipient PII** in labels.
  - A **signed-in notification settings page** showing the member's **per-workspace** subscription status
    (least-privilege: only their own), plus a **resubscribe** control (CSRF-protected POST).
- **Out of scope** (deliberate carve-outs, deferred):
  - **Per-category / per-event-type preferences** (choose which kinds of notification to receive), **digests**,
    and **quiet-hours** — explicitly deferred; v1 is a single **per-workspace** opt-out over the suppressible
    events only.
  - **Per-channel routing preferences** (recipient chooses email vs chat) — that is delivery-provider config,
    owned by the predecessor feature; recipients do not choose channels here.
  - Making any **currently-mandatory** event suppressible — the security events stay exempt by design (BR-3).
  - A **preference API / bulk admin management** of other people's subscriptions — a member manages only their
    own state (NFR-6).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse / Role |
|------|----------|--------------|
| `Notifier::notify(&self, notification)` — **INFALLIBLE** concurrent `JoinSet` fan-out, per-provider timeout, best-effort isolated | `crates/foundry-app/src/notify.rs:237` (loop `:244-280`, `deliver()` at `:252`) | **Where the suppression filter hooks** — a suppressible notification to an unsubscribed `(email, workspace)` must not reach `deliver()`. Hook point (notifier vs emit-site) is **ODD-3**. |
| `NotificationEvent` **closed** enum (`PasswordReset`, `WorkspaceInvite`, `MemberInvite`, `MemberRemoved`, `PasswordChanged`) + `as_str` + `ALL` | `crates/foundry-app/src/notify.rs:46-77` | The catalog we partition into **suppressible** {`WorkspaceInvite`,`MemberInvite`} vs **mandatory** {`PasswordReset`,`PasswordChanged`,`MemberRemoved`} (BR-1). |
| `Notification { event, recipient, subject, body }` (recipient is a bare email string; **not `Debug`**) | `crates/foundry-app/src/notify.rs:117-122` | The recipient email is the natural unsubscribe key half; the notification carries no `workspace_id` today — supplying workspace context to the filter is **ODD-3**. |
| Delivery metric `foundry_notification_deliveries_total{provider,event,outcome}` + `DeliveryOutcome{Delivered,Failed}` | `crates/foundry-app/src/notify.rs:39, 161-176, 291-297` | The observability seam a **suppression count** extends — a `suppressed` outcome or a sibling counter (**ODD-5**), bounded-label per ADR (no PII). |
| **Suppressible emit site** — `workspace_invite` (bootstrap invite) | `crates/foundry-app/src/bootstrap.rs:266` (built `:260-265`, `create_invite`) | Carries the **unsubscribe link** (US-01); `workspace_id` is in scope here. |
| **Suppressible emit site** — `member_invite` | `crates/foundry-app/src/member_invites.rs:204` (built `:198-203`, `submit_invite`) | Carries the **unsubscribe link** (US-04); `workspace_id` in scope. |
| **Mandatory emit sites** — `password_reset`, `password_changed` | `crates/foundry-app/src/signin.rs:255`, `:360` | **Never suppressed** (NFR-3, BR-3) — no unsubscribe link, always delivered. |
| **Mandatory emit site** — `member_removed` | `crates/foundry-app/src/member_invites.rs:292` (built `:286-291`) | **Never suppressed** — a person must always learn they were removed (NFR-3). |
| `InviteToken` — HMAC-SHA256 over `invite_payload = "{id}|{unix_ts}"`, signed with `SESSION_SECRET`; `new()` / `verify()`; constant-time compare | `crates/foundry-auth/src/lib.rs:354-390` (`sign` `:251`, `verify` `:260`, `HmacSha256` `:28`) | **The exact model for `UnsubscribeToken`** binding `"{email_lower}|{workspace_id}"`. Expiry stance is **ODD-1**. |
| Uniform **non-enumerable refusal** — fixed **200 OK**, byte-identical body for every invalid reason (expired / used / tampered / unknown) | `crates/foundry-app/src/invites_accept.rs:332-339` (`invite_refusal_page`) | The model for a **tampered/unknown unsubscribe token** response — leaks nothing about whether an email/workspace/account exists (NFR-1, FR-5). |
| CSRF double-submit middleware (`foundry_csrf` cookie vs `_csrf`/header, constant-time, 403 on mismatch); GET issues the cookie | `crates/foundry-app/src/csrf.rs:137` (`csrf_middleware`), issue `:54` (`ensure_csrf_cookie`), layer `lib.rs:536-539` | Protects **both** state-changing POSTs — the public unsubscribe **confirm** and the signed-in **resubscribe** (NFR-5). |
| Session — tower-sessions over Postgres; `foundry_session` cookie; `SessionUser{user_id, workspace_id}`; `Session` extractor | `crates/foundry-app/src/session.rs:69, 59, 64`; `SessionUser` in `bootstrap.rs` | Authenticates the **signed-in** settings + resubscribe surface (US-05/US-06); the public unsubscribe path needs **no** session. |
| Store lookups — `find_user_by_email(email_lower)`, `resolve_active_workspace(user_id)`, workspace membership (`is_workspace_admin`, `is_team_member`) | `crates/foundry-store/src/lib.rs:930, 804, 1955, 1048` | Resolve the signed-in member's workspaces + own email for the per-workspace status page (US-05), least-privilege scoped (NFR-6). |
| Store + **migration** pattern — `sqlx::migrate!("./migrations")`; token-table precedent `reset_tokens` (id PK, FK `ON DELETE CASCADE`, `expires_at`, `created_at`); `insert_reset_token(...)` method-on-`Store` | migrations dir `crates/foundry-store/migrations/` (**latest `0013_issue_change_events.sql`**); `foundry-store/src/lib.rs:186`, `:980`; `0002_sessions_and_reset.sql:20-28` | The **new `0014_notification_unsubscribes.sql`** table + its `Store` methods follow this shape. Table shape + email keying is **ODD-4**. |
| Public token-route cluster — `/invites/accept` (GET show + POST submit), `/forgot-password`; router `build_router`; uniform 404 fallback | `crates/foundry-app/src/lib.rs:371-374, 406-409, 302, 535` | Where the **public `/unsubscribe`** GET+POST registers — same public, CSRF-screened, session-layer-covered cluster as `/invites/accept`. |
| Account/settings authed routes — `/account/password` (GET+POST) | `crates/foundry-app/src/lib.rs:415-418` | The **neighbour** where the signed-in `/account/notifications` page registers. |
| Nav rail — `NavSection{Home,Board}`, `NavContext`, footer threads `is_instance_admin` + `csrf` | `crates/foundry-app/src/nav.rs:16, 29, 35-37` | Where a footer/menu link to the settings page surfaces (a template + `NavContext` change; `NavSection::Home`). |
| `public_url` (`FOUNDRY_PUBLIC_URL` → `AppState.public_url`) — the link host the shipped invite emails already use | `crates/foundry-app/src/main.rs:122` | The unsubscribe link host = the same `public_url` the invite links already embed (consistency already exercised). |

### The genuinely-new surface (DESIGN owns the exact shapes)

Everything else is a thin adapter over shipped seams. Only four pieces are genuinely new, each isolated behind
an open decision so requirements stay solution-neutral:

1. **The `UnsubscribeToken`** — an HMAC-signed token binding `email_lower` + `workspace_id`, a direct analogue
   of `InviteToken`. Its **expiry stance** (an unsubscribe link arguably should not expire, or expire long) is
   **ODD-1**; its **GET-safety / one-click-POST (RFC 8058)** stance is **ODD-2**.
2. **The unsubscribe state** — a new `(email_lower, workspace_id, unsubscribed_at)` table (`0014`). Its exact
   shape and **how account-less invitee emails are keyed/normalised** is **ODD-4**.
3. **The suppression filter** — where the notifier (or emit site) checks unsubscribe state before delivering a
   **suppressible** event, and how it obtains `workspace_id` (the `Notification` carries none today). **ODD-3.**
4. **The suppression metric** — a `suppressed` outcome on the existing counter vs a sibling counter, **without
   recipient PII** in labels. **ODD-5.**

> This feature is overwhelmingly an **EXTENSION** of the shipped notifier, its emit sites, the `InviteToken`
> signing pattern, the non-enumerable refusal, and the CSRF/session seams — reuse-over-reinvent is the
> deliberate choice. The **one** genuinely new persistent surface (the unsubscribe table + its migration) is
> named and grounded; the genuine architecture choices are flagged as ODDs.

## Jobs To Be Done (inline — no `docs/product/` SSOT in this repo)

This repo deliberately does not use a `docs/product/` SSOT; JTBD is folded in here (house convention). Three
jobs drive the feature. Every user story carries a `job_id` referencing one of them.

### JOB-1 `stop-unwanted-workspace-notifications` — Sam the recipient (invitee / member)

> **When** I keep getting workspace notifications I didn't ask for, **I want to** stop them with one click from
> the email, **so I can** quiet the noise while still getting security-critical alerts.

- **Functional**: stop the non-essential workspace emails for this workspace, from the email itself, with no
  account and no support ticket.
- **Emotional**: relief that the noise stops; reassurance that clicking won't cost him a password-reset or a
  "you were removed" email; trust that the link is safe to click.
- **Social**: not the person who "missed the memo" because he nuked all mail with an aggressive inbox filter;
  he opted out cleanly and precisely.
- **Four forces**:
  - **Push**: repeated `workspace_invite` / `member_invite` reminders he never asked for; his only lever today
    is a blunt inbox filter that would also bury security mail.
  - **Pull**: a one-click, per-workspace unsubscribe link right in the email that stops *only* the noise.
  - **Anxiety**: "If I click this, will I stop getting security alerts too? Is this link even legit — or will it
    tell some attacker my address is live?"
  - **Habit**: he's used to the "Unsubscribe" link every other service puts in the footer; he expects one here.
- **Opportunity score (ODI)**: Importance 8, Satisfaction 1 → **Opportunity = 8 + (8−1) = 15 (very high)** —
  there is **no** opt-out today, so satisfaction is floor.

### JOB-2 `manage-my-notification-subscription` — Maria the account-holding member

> **When** I want to check or change whether a workspace can email me, **I want to** open a settings page that
> shows my per-workspace subscription status with a resubscribe control, **so I can** stay in control without
> digging through old emails for a link.

- **Functional**: see, per workspace she belongs to, whether she is subscribed or muted, and flip it back on.
- **Emotional**: confidence and control — no guessing whether an earlier unsubscribe is still in effect.
- **Social**: manages her own comms like a competent adult; doesn't have to ask an admin to "turn my emails
  back on."
- **Four forces**:
  - **Push**: she unsubscribed once (or wonders if she did) and now has no way to check or undo it.
  - **Pull**: a clear per-workspace status list with a one-click resubscribe.
  - **Anxiety**: "Will I accidentally change someone else's settings, or expose them?"
  - **Habit**: she expects a "Notifications" area under account settings, like every other app.
- **Opportunity score (ODI)**: Importance 6, Satisfaction 1 → **Opportunity = 6 + (6−1) = 11 (high)**.

### JOB-3 `honor-unsubscribe-and-preserve-security` — Ops / Compliance Olivia (secondary)

> **When** recipients opt out, **I want** their unsubscribe honored and the volume visible, **while** knowing
> security-critical notifications are never suppressed, **so I can** keep good list hygiene without ever
> withholding a password-reset or removal notice.

- **Functional**: unsubscribe is actually enforced (suppressed count is real), security events are provably
  never dropped, and opt-out volume is observable without exposing who.
- **Emotional**: assurance / low compliance anxiety.
- **Social**: the operator who runs a clean, compliant, trustworthy deployment.
- **Four forces**: **Push** — no opt-out today is a deliverability/compliance liability. **Pull** — enforced
  opt-out + a suppression metric. **Anxiety** — "could this ever swallow a security email?" **Habit** — she
  watches `/metrics`, not a bespoke dashboard.
- **Opportunity score (ODI)**: Importance 7, Satisfaction 2 → **Opportunity = 7 + (7−2) = 12 (high)**.

## Functional requirements

- **FR-1** Each **suppressible** notification (`workspace_invite`, `member_invite`) carries a **signed
  unsubscribe link** that works **logged-out** — a token binding the recipient `email` + the `workspace_id`
  (shape/expiry per ODD-1). Mandatory events carry **no** such link.
- **FR-2** Following the link and **confirming** records a per-workspace opt-out for `(email_lower,
  workspace_id)` and shows the recipient a **confirmation** ("You won't receive further invitations from
  {workspace}. You'll still receive security-critical notifications.").
- **FR-3** Once a `(email_lower, workspace_id)` is unsubscribed, **future suppressible notifications to that
  pair are not delivered** through any provider (the delivery is suppressed before `deliver()`, hook per ODD-3).
- **FR-4** The **mandatory security events** (`password_reset`, `password_changed`, `member_removed`) are
  **always delivered** to the recipient **regardless** of unsubscribe state (BR-3, NFR-3).
- **FR-5** A **tampered, malformed, expired (if expiry applies), or unknown** unsubscribe token yields the
  **uniform non-enumerable refusal** (fixed status + body, à la `invite_refusal_page`) — it never reveals
  whether the email, workspace, or account exists, and never records an opt-out (FR-2 requires a valid token).
- **FR-6** A **signed-in account holder** can view a **notification settings page** listing, for each workspace
  they belong to, whether they are **subscribed** or **muted** — **their own** state only (NFR-6).
- **FR-7** A signed-in account holder can **resubscribe** a previously-muted workspace from that page via a
  **CSRF-protected** POST, returning that `(email, workspace)` to the subscribed (default) state.
- **FR-8** A **suppressed** delivery is **observable** as a count (metric/log), keyed on `event` (+ workspace at
  most) but **never** on the recipient address or token, so operators can see opt-out volume without PII (ODD-5).
- **FR-9** Unsubscribe granularity is **per `(email_lower, workspace_id)`**: a recipient muted in one workspace
  **still receives** suppressible notifications from a **different** workspace (BR-2).
- **FR-10** This feature introduces **one store migration** (`0014_notification_unsubscribes*`) for the
  unsubscribe state; the **default** state (no row) is **subscribed** (opt-out model, BR-7).

## Non-Functional Requirements (security & operability — first-class)

### NFR-1 — Non-enumerable, tamper-proof token
The unsubscribe link is **HMAC-signed** binding `email_lower` + `workspace_id` (the `InviteToken` model,
`foundry-auth/src/lib.rs:354-390`), verified with a **constant-time** compare. A tampered / malformed / unknown
token yields the **uniform non-enumerable refusal** (`invites_accept.rs:332-339` model): a fixed status and a
byte-identical body for **every** invalid reason, leaking nothing about whether the email/workspace/account
exists. No opt-out is recorded for an invalid token.
- **Measurable**: flipping any byte of a valid token, or substituting a different `workspace_id`, yields the
  identical refusal page as an entirely made-up token; **no** unsubscribe row is written; the response is
  indistinguishable (status + body) across all invalid reasons.

### NFR-2 — Prefetch / one-click safety (GET must not silently mutate)
Email clients and security scanners **prefetch** links. A bare **GET** of the unsubscribe URL must **not**
silently record an opt-out; state changes only on an explicit **confirm** — either a GET → confirmation-page →
**POST**, or an **RFC 8058 one-click POST** (`List-Unsubscribe-Post`) issued by the mail client (stance is
ODD-2). Whatever the choice, an automated prefetch of the raw link leaves subscription state unchanged.
- **Measurable**: an automated **GET** of a valid unsubscribe URL (simulating a scanner prefetch) does **not**
  create an unsubscribe row; only the confirm action (POST) does. Reverting this reds the litmus.

### NFR-3 — Mandatory-security invariant (never-suppress), regression-guarded
`password_reset`, `password_changed`, and `member_removed` are **always** delivered, even when the recipient is
unsubscribed for that workspace. This is a hard invariant with **precedence**: mandatory > unsubscribe (BR-3).
- **Measurable**: with `(sam.okafor@acme.example, "Northwind")` unsubscribed, a `member_removed` and a
  `password_reset` for Sam both still deliver (counted `delivered`, **not** `suppressed`); a regression that
  suppressed a mandatory event reds a dedicated `@property` litmus.

### NFR-4 — Suppression observability without PII
A suppressed suppressible-delivery is **counted** (a `suppressed` outcome on the delivery counter, or a sibling
`foundry_notification_suppressions_total{event}` — ODD-5), bounded-label, on the existing `/metrics` sidecar.
Labels carry `event` (∈ the bounded catalog) and at most `workspace`; **never** the recipient email, token, or
any PII.
- **Measurable**: after N suppressed invites, the suppression count == N with the correct `event` label, and a
  `/metrics` scrape contains **no** recipient email or token value; the cardinality/label guard fails closed on
  an unbounded or PII label.

### NFR-5 — CSRF on both state-changing POSTs
Both the **public unsubscribe confirm** and the **signed-in resubscribe** are POSTs mounted under the shipped
CSRF middleware (`csrf.rs:137`, layer `lib.rs:536-539`); a request with a missing/mismatched `_csrf` token is
rejected `403` (constant-time compare). The public confirm form is served with a CSRF cookie
(`ensure_csrf_cookie`, `csrf.rs:54`).
- **Measurable**: a POST to either endpoint without a valid CSRF token is refused `403` and changes no state;
  the shipped double-submit check (`csrf.rs`) applies unchanged.

### NFR-6 — Least-privilege on the signed-in page
A signed-in member sees and manages **only their own** subscription state, scoped to the **workspaces they
belong to**. There is **no** way to view or change another recipient's subscription, and **no** cross-recipient
enumeration. The page derives the acting identity from the **session** (`SessionUser`, `session.rs`), never from
a client-supplied email.
- **Measurable**: the settings page for Maria lists only workspaces Maria is a member of and only Maria's own
  status; a crafted request naming another user's email returns her own scope (or a uniform refusal), never
  another recipient's state.

### NFR-7 — Backwards-compatibility (additive filter)
For a **subscribed** recipient, and for **every** mandatory event, delivery behaviour is **byte-for-byte
unchanged** from today — the suppression filter is **purely additive** and only ever *removes* a suppressible
delivery for an *unsubscribed* pair. With no unsubscribe rows, the notifier behaves exactly as it does post
`notification-delivery-providers`.
- **Measurable**: with an empty unsubscribe table, every existing delivery scenario (invite + reset + removal
  fan-out) passes unchanged; the only new behaviour appears once a row exists.

### NFR-8 — Accessibility of the new web surfaces (WCAG 2.1 AA)
Unlike the predecessor (which added **no** UI), this feature adds **two new user-facing HTML surfaces** — the
public unsubscribe **confirmation page** and the signed-in **notification settings page** — so **accessibility
applies**. Both use semantic HTML, labelled form controls, a keyboard-operable resubscribe/confirm control with
a visible focus indicator, and text (not colour alone) to convey subscription status.
- **Measurable**: the confirm and settings pages pass an automated a11y check (labels present, controls
  keyboard-reachable, status conveyed as text); the resubscribe control is operable without a pointer.

## Business rules

- **BR-1** The catalog is partitioned: **suppressible** = {`workspace_invite`, `member_invite`}; **mandatory**
  = {`password_reset`, `password_changed`, `member_removed`}. Only suppressible events carry an unsubscribe link
  and can be suppressed.
- **BR-2** Unsubscribe state keys on **`(email_lower, workspace_id)`** — per recipient-email, per workspace.
  Email is normalised to lower-case (matching `find_user_by_email(email_lower)`).
- **BR-3** **Mandatory > unsubscribe.** A mandatory event is **never** suppressed, even for an unsubscribed
  pair. Suppression applies **only** to suppressible events.
- **BR-4** An **invalid / tampered / unknown / (expired)** token is refused **uniformly and non-enumerably**;
  it records **no** state change.
- **BR-5** A subscription **state change** requires either a **valid signed token** (public unsubscribe) or an
  **authenticated session + CSRF** (signed-in resubscribe). No state change is possible without one of these.
- **BR-6** A member may view and change **only their own** subscription state, scoped to workspaces they belong
  to (least-privilege; identity from session, not client input).
- **BR-7** The **default** state is **subscribed** — absence of an unsubscribe row means subscribed. Unsubscribe
  is opt-out; resubscribe clears the opt-out.
- **BR-8** Unsubscribe is **idempotent**: unsubscribing an already-unsubscribed pair is a no-op success;
  resubscribing an already-subscribed pair is a no-op success. Neither leaks prior state.

## Alternatives considered (constraint rationale)

- **Per-workspace opt-out** (vs a single global unsubscribe, vs per-category preferences now): chose
  per-workspace. A global opt-out is too blunt (a recipient in two workspaces wants to mute only the noisy one);
  full per-category/digest/quiet-hours is a much larger surface that would blow the Elephant-Carpaccio gate.
  Per-workspace is the smallest unit that matches how the noise actually arrives (workspace invites), and it is
  the natural key given the two suppressible events both carry `workspace_id`.
- **Email-keyed state** (vs user-id-keyed): chose `email_lower` because **many recipients are invitees with no
  account yet** — a user-id key can't represent them. Email is what the notifier already targets
  (`Notification.recipient`) and what invites are addressed to. (Case-normalisation + the account-vs-email
  reconciliation is ODD-4.)
- **Signed token + non-enumerable refusal** (vs an authenticated-only unsubscribe): chose a signed public token
  because the recipient **has no account** and must act **from the email, logged-out**. The `InviteToken`
  pattern already solves exactly this (act on a workspace resource from a link with no session), and its uniform
  refusal already solves non-enumerability — reuse both.
- **Suppress in the notifier / emit path** (vs filtering at each provider): chose a single suppression point so
  the rule is enforced **once** for all providers and can't be forgotten in a new provider. Whether it sits in
  `Notifier::notify` (needs workspace context added to `Notification`) or at the emit sites is ODD-3.
- **Security events exempt** (vs letting recipients mute everything): chose to keep `password_reset` /
  `password_changed` / `member_removed` **always-on**. Suppressing a "your password changed" or "you were
  removed" notice is a security and safety hazard; the invariant is regression-guarded (NFR-3). This is the
  honest v1 stance: the mechanism is general, but only the invites are opted-in to suppression.
- **One-click-safe confirm** (vs a destructive GET): a raw destructive GET would let an email client's prefetch
  silently unsubscribe people. Either a GET→confirm→POST or an RFC 8058 one-click POST avoids that (ODD-2).

## Risk assessment (surfaced, not managed)

| # | Risk | Category | Probability | Impact | Mitigation |
|---|------|----------|-------------|--------|------------|
| R1 | A **mandatory** event is accidentally suppressed (a security email withheld) | Security/Safety | Low | **Critical** | NFR-3 + BR-3 (mandatory > unsubscribe); dedicated never-suppress `@property` litmus (US-02); suppressible set is a bounded allow-list, not a deny-list. |
| R2 | A **destructive GET** lets a mail-client/scanner **prefetch** silently unsubscribe people | Security | Medium | High | NFR-2 (GET non-destructive; confirm via POST or RFC 8058 one-click); ODD-2; prefetch-safety litmus (US-03). |
| R3 | The token **leaks existence** (different response for known vs unknown email/workspace) | Security/Privacy | Medium | High | NFR-1 + BR-4; reuse the uniform 200-OK non-enumerable refusal (`invites_accept.rs:332-339`); indistinguishable-response litmus (US-03). |
| R4 | The **suppression metric leaks PII** (recipient email in a label) | Privacy | Medium | High | NFR-4 (labels bounded to `event`/`workspace`, never recipient); ODD-5; label/cardinality guard fails closed. |
| R5 | Adding a **store lookup in the delivery path** stalls or fails the INFALLIBLE `notify()` | Technical | Medium | Medium | Hook point + failure stance is ODD-3; suppression check must preserve the best-effort/infallible contract (a lookup error must **fail open to delivering** a suppressible event? or closed? — DESIGN decides, defaulting to *not* worse than today). |
| R6 | An **email in multiple workspaces** gets muted in the wrong scope, or the settings page mis-lists | Technical | Low | Medium | BR-2 (key is `(email_lower, workspace_id)`); FR-9 (muting A doesn't affect B); ODD-7 (multi-workspace interaction); per-workspace status test. |
| R7 | **Account-less invitee** can't resubscribe (signed-in page needs an account) | UX | Medium | Low | ODD-6 (resubscribe UX — token-based resubscribe / undo on the confirmation page for account-less recipients); documented as a known v1 edge. |
| R8 | The **new migration** (`0014`) or email-normalisation is inconsistent with `find_user_by_email(email_lower)` | Technical | Low | Medium | ODD-4 (table shape + email keying mirrors `reset_tokens` + `email_lower` normalisation); grounded in the shipped migration/store pattern. |

## Glossary (ubiquitous language)

- **Recipient** — a person a notification is addressed to, identified by **email** (may be an **invitee with no
  account**).
- **Suppressible event** — a notification a recipient may opt out of: `workspace_invite`, `member_invite`.
- **Mandatory event** — a security-critical notification that is **never** suppressed: `password_reset`,
  `password_changed`, `member_removed`.
- **Unsubscribe** — recording a per-workspace opt-out for `(email_lower, workspace_id)`.
- **Resubscribe** — clearing that opt-out (returning to the subscribed default).
- **Subscribed (default)** — the absence of an unsubscribe row; suppressible events deliver normally.
- **UnsubscribeToken** — an HMAC-signed token binding `email` + `workspace_id`, embedded in the unsubscribe link
  (modelled on `InviteToken`).
- **Non-enumerable refusal** — a fixed, byte-identical response to any invalid token that reveals nothing about
  existence (modelled on `invite_refusal_page`).
- **Suppression** — the notifier not delivering a suppressible event to an unsubscribed pair.
- **Suppression count** — the PII-free metric recording how many suppressible deliveries were suppressed.
- **Per-workspace granularity** — an opt-out covers exactly one `(recipient-email, workspace)` pair.
</content>
</invoke>
