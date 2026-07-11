# Story Map: recipient-notification-preferences (v1 = recipient unsubscribe)

## Users: Sam Okafor (recipient / account-less invitee) and Maria Santos (account-holding member); Ops/Compliance Olivia (secondary)
## Goal: let a recipient stop a workspace's suppressible notifications (workspace_invite, member_invite) with one click from the email, keyed per (email_lower, workspace_id), while security-critical events are never suppressed — and let an account holder review per-workspace status and resubscribe

## Scope (v1 = slices 01–04; 05–07 fast-follow in this feature)

**In scope**: a signed `UnsubscribeToken` (InviteToken model) in the two suppressible emails, a public
token-verified unsubscribe route (non-destructive GET + CSRF confirm POST + non-enumerable refusal), a new
`0014_notification_unsubscribes` table, a notifier suppression filter (mandatory events never suppressed),
PII-free suppression observability, and a signed-in per-workspace status + resubscribe page.
**Out of scope** (explicit): **per-category / per-event-type preferences, digests, quiet-hours** (deferred);
**per-channel routing** (owned by `notification-delivery-providers`); making any **mandatory** event
suppressible; **bulk/admin** management of other recipients' subscriptions.

## Backbone

| Carry the link (emit) | Recipient acts from the email (logged-out) | Record the opt-out | Suppress future noise (deliver) | Manage when signed in |
|-----------------------|--------------------------------------------|--------------------|---------------------------------|-----------------------|
| Suppressible emails (`workspace_invite`, `member_invite`) carry a signed `UnsubscribeToken` link; mandatory emails carry none | Recipient opens the link (non-destructive GET), sees a confirm page that promises security mail still arrives | Confirm (CSRF POST) verifies the token + writes `(email_lower, workspace_id)` to the `0014` table | Notifier suppresses future suppressible events to the pair; **security events always delivered** | Account holder sees per-workspace status + resubscribes (CSRF POST) |
| Token binds recipient email + workspace (both in scope at emit) | Tampered/unknown token → uniform non-enumerable refusal; prefetch-safe | Idempotent; single source of subscription state | Bounded allow-list; additive (empty table = today) | Least-privilege: own state only; operator sees PII-free opt-out volume |

---

### Walking Skeleton (the thinnest end-to-end unsubscribe loop on ONE suppressible event)

The single minimum task from each backbone activity that makes the end-to-end opt-out work, on **one** event
(`workspace_invite`):

- **Carry the link**: the `workspace_invite` email (`bootstrap.rs:266`) embeds a signed `UnsubscribeToken`
  link bound to `(recipient email, workspace_id)`.
- **Recipient acts**: a public `/unsubscribe` GET renders a non-destructive confirm page (prefetch-safe).
- **Record the opt-out**: a CSRF-checked confirm POST verifies the token and writes one row to the new
  `0014_notification_unsubscribes` table (idempotent).
- **Suppress future noise**: the notifier suppresses the next `workspace_invite` to that pair (the filter +
  suppression point exist even for one event).
- **(Security exempt)**: mandatory events skip the check — proven in the very next slice (US-02).

This is **US-01**. It carries the whole feature's uncertainty: the **token shape/expiry** (ODD-1), the
**GET-safety/one-click stance** (ODD-2), the **suppression hook** (ODD-3), and the **table shape/email keying**
(ODD-4). Everything after it hardens, extends, or surfaces this proven loop.

### Release 1 (v1): "A safe, honest per-workspace unsubscribe on the invite events" — the v1 boundary

- **US-01 Stop a workspace's invite emails with one click** — the token + public route + `0014` table +
  suppression filter, proven on `workspace_invite` (walking skeleton).
- **US-02 Never lose a security-critical notification** — the mandatory-exemption invariant (mandatory >
  unsubscribe), regression-guarded. Makes the confirm page's promise literally true.
- **US-03 A tampered/unknown link is safely refused** — the non-enumerable refusal + prefetch-safety hardening
  (defeats enumeration + prefetch).
- **US-04 The same unsubscribe works for member-invite emails** — extends the mechanism to the second
  suppressible event; one opt-out covers both.
- Target outcome: a recipient can silence a workspace's invitation noise from the email, with **no** account,
  **without** losing security mail, and the link is **safe** to click and to sit in an inbox. KPIs: KPI-1
  (opt-out works), KPI-2 (mandatory never suppressed — guardrail), KPI-3 (non-enumerable/prefetch-safe —
  guardrail).

### Release 2: "Self-serve subscription management for account holders"

- **US-05 See my per-workspace notification status when signed in** — the signed-in status page (own state
  only).
- **US-06 Resubscribe a workspace I previously muted** — the CSRF-protected resubscribe.
- Target outcome: an account holder can see and control which workspaces can email them, any time, without
  hunting for an old link. KPI: KPI-4 (self-serve status + resubscribe adoption).

### Release 3: "Operator/compliance visibility"

- **US-07 See how much opt-out is happening, without exposing who** — PII-free suppression observability on
  `/metrics`.
- Target outcome: operators can monitor opt-out volume and prove suppression is enforced (and security never
  suppressed) without seeing recipient PII. KPI: KPI-5 (observable opt-out volume, 0 PII).

---

## Priority Rationale

1. **US-01 (Walking Skeleton, P1)** — carries all the feature's uncertainty (token shape/expiry ODD-1,
   GET-safety ODD-2, suppression hook ODD-3, table/keying ODD-4). Until one suppressible notification can be
   opted out end-to-end (link → confirm → recorded → suppressed), nothing else can be de-risked. Highest
   learning leverage; it is the reason to build first.
2. **US-02 (Mandatory never suppressed, P1/v1 — guardrail)** — the **safety crux**. The moment suppression
   exists (US-01), the risk that a `password_reset` or `member_removed` gets withheld is live and **critical**.
   Sequenced immediately after the skeleton because it *constrains* the filter US-01 introduces and makes the
   confirm page's promise honest. Shipping suppression without this would be shipping the risk.
3. **US-03 (Non-enumerable + prefetch-safe, P1/v1 — guardrail)** — the **security crux** of a public
   token endpoint: no existence leak, no prefetch mutation. Reuses the shipped uniform refusal + constant-time
   verify, but must be proven against the adversary before the link goes into real inboxes. v1 boundary.
4. **US-04 (Second suppressible event, P1/v1)** — extends the proven mechanism to `member_invite` so the
   opt-out is complete, not a half-measure. Small (reuses everything from US-01), but closes the v1 promise
   ("silence the workspace's invitation noise" — both invite events). Depends on US-01 + US-02.
5. **US-05 (Signed-in status, P2)** — the account-holder management surface; additive value once the mechanism
   works. New user-facing page (a11y in scope, NFR-8). Read-only; depends on the US-01 state.
6. **US-06 (Resubscribe, P2)** — completes account-holder control (undo a mute). CSRF-protected mutation over
   US-05's page. Depends on US-05 + US-01.
7. **US-07 (Operator visibility, P3)** — surfaces opt-out volume for ops/compliance over an already-de-risked
   pipeline; lowest risk (observability only), so last. Depends on US-01 (the suppression it counts).

All seven stories trace to outcome KPIs (no orphans). Slicing is by **user outcome** (opt out / keep security
mail / trust the link / cover both invites / see my status / undo a mute / see the volume), NOT by technical
layer — each slice is a thin end-to-end increment verifiable in a single dogfood session. The v1 boundary
(US-01..US-04) is the minimum that delivers a **safe, honest** per-workspace unsubscribe on the invite events.
</content>
