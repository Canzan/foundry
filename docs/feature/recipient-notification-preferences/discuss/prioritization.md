# Prioritization: recipient-notification-preferences (v1 = recipient unsubscribe)

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-01) — token + public route + `0014` table + suppression filter on `workspace_invite` | A recipient opts out of a workspace's invitations from the email, logged-out, and the next matching invite is suppressed | KPI-1 opt-out works | Carries all the uncertainty (token shape/expiry ODD-1, GET-safety ODD-2, suppression hook ODD-3, table/keying ODD-4). Nothing else can be de-risked until one suppressible event can be opted out end-to-end. |
| 1 | v1 guardrails (US-02 mandatory-never-suppressed, US-03 non-enumerable + prefetch-safe) | Security mail is always delivered; the public link leaks no existence and is prefetch-proof | KPI-2 mandatory never suppressed, KPI-3 non-enumerable/prefetch-safe | The two crux risks the moment suppression + a public token endpoint exist: withholding a security email (CRITICAL safety) and an enumeration/prefetch oracle (HIGH security). Both must ship with US-01, not after. |
| 1 | v1 completeness (US-04 member-invite) | One opt-out silences both invite events for a workspace | KPI-1 | Extends the proven mechanism to the second suppressible event so the opt-out is complete, not a half-measure. Small (reuses US-01). Closes the v1 boundary. |
| 2 | Self-serve management (US-05 status, US-06 resubscribe) | An account holder sees per-workspace status and undoes a mute themselves | KPI-4 self-serve management | Additive account-holder value over the working mechanism; new UI (a11y in scope, NFR-8). |
| 3 | Operator visibility (US-07) | Ops/compliance sees opt-out volume without PII, and confirms security is never suppressed | KPI-5 observable opt-out, 0 PII | Observability over an already-de-risked pipeline; lowest risk, last. |

## Prioritization Scores (Value × Urgency / Effort, 1–5)

| Story | Value | Urgency | Effort | Score | Notes |
|-------|-------|---------|--------|-------|-------|
| US-01 | 5 | 5 | 3 | 8.3 | Token + route + `0014` table + suppression filter + one re-routed emit. Carries ALL the uncertainty — walking-skeleton tie-break wins regardless. |
| US-02 | 5 | 5 | 1 | 25.0 | The safety invariant (mandatory > unsubscribe). Tiny surface (a bounded allow-list + a litmus) but CRITICAL — the highest-leverage guardrail. |
| US-03 | 5 | 5 | 2 | 12.5 | Non-enumerable refusal + prefetch-safe GET. Reuses shipped uniform refusal + constant-time verify; security crux of the public endpoint. |
| US-04 | 4 | 4 | 1 | 16.0 | Attach the same link to `member_invite` + add it to the allow-list. Reuses everything from US-01; completes the v1 promise. |
| US-05 | 4 | 3 | 2 | 6.0 | Signed-in per-workspace status page (own state only). New authed UI; a11y in scope. Read-only. |
| US-06 | 4 | 3 | 2 | 6.0 | CSRF-protected resubscribe over US-05's page. Idempotent, least-privilege. |
| US-07 | 3 | 2 | 1 | 6.0 | PII-free suppression count on the shipped `/metrics` seam. Observability only. |

> Tie-break (per user-story-mapping skill): Walking Skeleton > Riskiest Assumption > Highest Value.
> US-01 is the skeleton (P1). US-02 (mandatory never suppressed) and US-03 (non-enumerable/prefetch-safe) are
> the riskiest **safety/security** assumptions and must close the v1 boundary alongside US-01. US-04 completes
> the mechanism across both invite events. US-05/US-06 are the account-holder management surface; US-07 is
> observability.

## Dependency rationale (per slice)

- **US-01** depends on shipped seams only: the notifier (`notify.rs:237`), the `InviteToken` pattern
  (`foundry-auth/src/lib.rs:354-390`), the public route cluster (`lib.rs:371-374`), the migration/store pattern
  (`0002_sessions_and_reset.sql:20-28`, `insert_reset_token` `lib.rs:980`), CSRF (`csrf.rs:137`), and
  `public_url` (`main.rs:122`). Adds the one migration `0014`.
- **US-02** depends on US-01 (the suppression filter it constrains). No new persistence.
- **US-03** depends on US-01 (the token + route it hardens) + the shipped uniform refusal
  (`invites_accept.rs:332-339`) + constant-time verify (`foundry-auth/src/lib.rs:260`).
- **US-04** depends on US-01 (token/table/route/filter) + US-02 (mandatory exemption) — attaches the link to
  `member_invites.rs:204` and adds the event to the allow-list.
- **US-05** depends on US-01 (the state it reads) + session/membership lookups (`session.rs`,
  `foundry-store/src/lib.rs:1048,1955`, `find_user_by_email` `:930`).
- **US-06** depends on US-05 (the page it acts on) + US-01 (the state it clears) + CSRF (`csrf.rs:137`).
- **US-07** depends on US-01 (the suppression it counts) + the shipped metric seam (`notify.rs:39,291-297`).

## Dogfood cadence

Each slice ships a **dogfood moment** verified in one session:

| Slice | Dogfood moment |
|-------|----------------|
| US-01 | Issue a `workspace_invite` to a local mail catcher, click the unsubscribe link, confirm, re-issue the invite, watch it get suppressed. |
| US-02 | With that pair unsubscribed, trigger a password reset + a member removal; confirm both still deliver and neither is counted `suppressed`. |
| US-03 | Alter a byte of the token → uniform refusal; request a fake address → identical response; GET the real link without confirming → no row written. |
| US-04 | Unsubscribe via a `workspace_invite` link, then fire a `member_invite` for the same pair; confirm it's suppressed too. |
| US-05 | Sign in as Maria, open `/account/notifications`, see Northwind muted + the others subscribed; confirm no other user's rows appear. |
| US-06 | Click Resubscribe on Northwind, re-issue an invite, watch it deliver again; retry a forged CSRF POST → 403. |
| US-07 | After a batch of suppressions, scrape `/metrics`, read the suppressed count by event; grep for any email/token → none. |

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-01 | WS | P1 | KPI-1 | Shipped seams; new `0014` migration; ODD-1/2/3/4. |
| US-02 | v1 | P1 | KPI-2 | US-01 (suppression filter). |
| US-03 | v1 | P1 | KPI-3 | US-01; uniform refusal + constant-time verify. |
| US-04 | v1 (gate) | P1 | KPI-1 | US-01 + US-02; `member_invites.rs:204`. |
| US-05 | R2 | P2 | KPI-4 | US-01; session + membership lookups. |
| US-06 | R2 | P2 | KPI-4 | US-05 + US-01; CSRF. |
| US-07 | R3 | P3 | KPI-5 | US-01; shipped metric seam. |

## Scope Assessment (Elephant Carpaccio Gate)

**PASS — 7 stories, 1 bounded context (`foundry-app` notification/auth surface + the new `foundry-store`
unsubscribe table), estimated ~5–6 days across seven thin slices, adding ONE migration (`0014`).**

Oversized signals checked: stories **7** (≤10 OK) | bounded contexts **1** (≤3 OK — `foundry-app` web/notifier
tier + one `foundry-store` table + the shipped `foundry-auth` signing pattern) | walking-skeleton integration
points: notifier hook, `InviteToken` pattern, public route cluster, the `0014` table + store methods, CSRF,
`public_url` = ~5 reused seams + 1 new table (at the >5 line but all but one are *reused*, and the one new
persistent surface is the deliberately-scoped table) | effort **~5–6 days** (< 2 weeks) | **one** coherent
capability (recipient unsubscribe) sliced into thin per-event / per-surface increments, each dogfoodable in a
single session — no slice ships 4+ new components.

Right-sized; no split needed. The genuine scope pressure — **per-category preferences / digests / quiet-hours**
— was carved OUT and deferred **before** this map was drawn (see `wave-decisions.md` Scope Assessment); v1 is a
single **per-workspace opt-out** over the two suppressible events, with security explicitly exempt. **Note: this
feature adds the one migration `0014_notification_unsubscribes` — unlike the predecessor, which added zero.**
</content>
