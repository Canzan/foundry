# DISCUSS Decisions — recipient-notification-preferences (v1 = recipient unsubscribe)

## Key Decisions

- [D1] **v1 = the recipient unsubscribe MECHANISM, proven on the invite events, security exempt.** Because the
  whole shipped catalog is transactional, v1 builds the mechanism end-to-end (token → public route → `0014`
  table → suppression filter → signed-in surface) and proves it on the two **suppressible** events
  (`workspace_invite`, `member_invite`), with the **security** events (`password_reset`, `password_changed`,
  `member_removed`) **explicitly exempt** (never suppressed). Framed honestly — not a rich preference centre.
- [D2] **Per-workspace, email-keyed opt-out.** Unsubscribe keys on `(email_lower, workspace_id)` (BR-2). A
  global opt-out is too blunt; per-category/digests/quiet-hours is too big (carved out). Email (not user id) is
  the key because **many recipients are account-less invitees** — email is what the notifier already targets
  (`Notification.recipient`, `notify.rs:117-122`) and what the two suppressible events already carry a
  `workspace_id` for.
- [D3] **Both surfaces: a logged-out signed link AND a signed-in page.** (a) A signed `UnsubscribeToken`
  (InviteToken model, `foundry-auth/src/lib.rs:354-390`) embedded in the suppressible emails lets an
  **account-less** recipient unsubscribe **logged-out**. (b) A signed-in `/account/notifications` page lets
  **account holders** review per-workspace status and resubscribe.
- [D4] **Security events are never suppressed — a bounded allow-list, not a deny-list.** The suppressible set
  is `{workspace_invite, member_invite}`; everything else is delivered by default (BR-1, BR-3). Mandatory >
  unsubscribe, regression-guarded by a never-suppress `@property` (NFR-3). This makes the confirm page's promise
  ("you'll still receive security-critical notifications") literally true.
- [D5] **Non-enumerable + prefetch-safe public endpoint.** A tampered/unknown token yields the shipped uniform
  refusal (`invites_accept.rs:332-339`) — a fixed, byte-identical response that leaks no existence (NFR-1). A
  bare GET is non-destructive; only an explicit confirm mutates state (NFR-2). Token verify is constant-time
  (`foundry-auth/src/lib.rs:260`).
- [D6] **CSRF on both state-changing POSTs; least-privilege signed-in surface.** The public confirm and the
  signed-in resubscribe both sit under the shipped CSRF middleware (`csrf.rs:137`, NFR-5). The signed-in page
  derives identity from the session (`SessionUser`) and exposes only the member's own state, scoped to their
  workspaces (NFR-6).
- [D7] **PII-free suppression observability on the shipped seam.** A suppressed delivery is counted on
  `foundry_notification_deliveries_total` (a `suppressed` outcome) or a sibling counter (ODD-5), bounded-label,
  with **no** recipient email/token in any label (NFR-4). No new dashboard infra.
- [D8] **Additive / backwards-compatible filter.** With an empty `0014` table the notifier behaves byte-for-byte
  as it does post `notification-delivery-providers`; the filter only ever removes a suppressible delivery for an
  unsubscribed pair (NFR-7).
- [D9] **This feature adds ONE migration (`0014_notification_unsubscribes`).** Unlike the predecessor (zero
  migrations), the unsubscribe state needs persistence. It follows the shipped `reset_tokens` shape
  (`0002_sessions_and_reset.sql:20-28`) and store-method pattern (`insert_reset_token`, `lib.rs:980`); latest
  shipped migration is `0013_issue_change_events.sql`.
- [D10] **New user-facing surfaces ⇒ accessibility is in scope (NFR-8).** Unlike the predecessor (no UI, NFR-7
  N/A), this feature adds a public confirm page and a signed-in settings page — WCAG 2.1 AA basics apply.
- [D11] **Repo legacy multi-file convention; no `docs/product/` SSOT; JTBD folded inline.** Three jobs (Sam,
  Maria, Olivia), four forces + ODI opportunity scores, live in `requirements.md`, not a `jobs.yaml`. Matches
  all prior features on trunk.

## Requirements Summary
- Primary need: let a recipient (often an account-less invitee) stop a workspace's suppressible notifications
  with one click from the email, per `(email_lower, workspace_id)`, safely (non-enumerable, prefetch-safe,
  security-never-suppressed) and observably (PII-free opt-out volume) — and let account holders review status +
  resubscribe.
- Walking skeleton: US-01 — the token + public route + `0014` table + suppression filter, proven on one
  suppressible event (`workspace_invite`), end-to-end (link → confirm → recorded → suppressed).
- Feature type: cross-cutting, brownfield (UI settings page + a public token route + a new store table + a
  notifier filter hook + auth/session). One bounded context (`foundry-app` web/notifier tier + one
  `foundry-store` table + the shipped `foundry-auth` signing pattern).

## Constraints Established
- Per-workspace, email-keyed opt-out `(email_lower, workspace_id)`; default (no row) = subscribed (BR-2, BR-7).
- Suppressible = {`workspace_invite`, `member_invite`}; mandatory = {`password_reset`, `password_changed`,
  `member_removed`} and **never** suppressed (BR-1, BR-3, NFR-3).
- Signed `UnsubscribeToken` (InviteToken model); tampered/unknown → uniform non-enumerable refusal; constant-time
  verify (NFR-1, BR-4).
- GET non-destructive; confirm via POST / RFC 8058 one-click (NFR-2).
- CSRF on both POSTs (NFR-5); signed-in surface least-privilege, session identity only (NFR-6, BR-6).
- PII-free suppression count, bounded-label, on the existing `/metrics` sidecar (NFR-4).
- Additive filter; empty table ⇒ delivery unchanged (NFR-7). New UI ⇒ WCAG 2.1 AA (NFR-8).
- One new migration `0014_notification_unsubscribes` (D9).

## Scope Assessment: PASS

**PASS — 7 stories, 1 bounded context, ~5–6 days across seven thin slices, adding ONE migration (`0014`).**
Right-sized; no split needed.

**Oversized→split analysis performed.** The framing "recipient notification preferences" threatened to bundle
per-category preferences, digests, and quiet-hours — each a distinct state model and user outcome that together
would push past the Elephant-Carpaccio gate (>10 stories, a second bounded context around a rich preference
schema, multiple independent outcomes). **Resolution (per the locked scope): v1 is a single per-workspace
unsubscribe over the two suppressible events, with security explicitly exempt; per-category / digests /
quiet-hours and per-channel routing are OUT OF SCOPE and deferred.** This makes v1 a thin, coherent mechanism.

Post-carve-out signals: stories **7** (≤10) | bounded contexts **1** (≤3 — `foundry-app` web/notifier +
one `foundry-store` table + the `foundry-auth` signing pattern) | walking-skeleton integration points ~5 reused
seams (notifier hook, `InviteToken` pattern, public route cluster, CSRF, `public_url`) + **1 new persistent
surface** (the `0014` table) — at the >5 line but only one is genuinely new, and it is the deliberately-scoped
opt-out table | effort **~5–6 days** (<2 weeks) | **one** coherent capability (recipient unsubscribe) sliced
into thin per-event/per-surface increments, each dogfoodable in a single session. No slice ships 4+ new
components.

**Migration note**: this feature adds the one migration `0014_notification_unsubscribes(email_lower,
workspace_id, unsubscribed_at)` — the first since the delivery-provider work (which added zero). It follows the
shipped `reset_tokens` table shape and store-method pattern; latest shipped is `0013_issue_change_events.sql`.

## Handoff to DESIGN

DISCUSS deliberately leaves the genuine architecture choices open (requirements are solution-neutral). The
solution-architect must resolve these Open Design Decisions (ODDs):

- **ODD-1 — `UnsubscribeToken` shape + expiry stance.** Payload binds `email_lower` + `workspace_id` (analogue
  of `invite_payload` `"{id}|{unix_ts}"`, `foundry-auth/src/lib.rs:388-390`). Unlike an invite, an unsubscribe
  link arguably should **not** expire (or expire very long) — decide, and whether a rotated `SESSION_SECRET`
  should invalidate old links.
- **ODD-2 — GET-safety / one-click stance.** RFC 8058 `List-Unsubscribe-Post` one-click POST vs a
  GET→confirm→POST page; and whether to emit `List-Unsubscribe` / `List-Unsubscribe-Post` **email headers**
  alongside the in-body link. Must guarantee a prefetch never mutates state (NFR-2, Risk R2).
- **ODD-3 — Where the suppression filter hooks + its fail stance.** Inside `Notifier::notify` (`notify.rs:237`,
  which needs `workspace_id` added to `Notification` — it carries none today, `:117-122`) vs at the emit sites
  (`bootstrap.rs:266`, `member_invites.rs:204`, which have `workspace_id` in scope). And: how a suppression-store
  lookup error is handled in the INFALLIBLE `notify()` — fail-open to delivering a suppressible event, or
  fail-closed to suppressing? (default: no worse than today's contract; Risk R5.)
- **ODD-4 — State table shape + email keying.** `notification_unsubscribes(email_lower, workspace_id,
  unsubscribed_at)` uniqueness + index; case-normalisation matching `find_user_by_email(email_lower)`
  (`foundry-store/src/lib.rs:930`); FK to `workspaces(id) ON DELETE CASCADE` (per `0013`); how an account-less
  invitee's email reconciles with a later account email (Risk R8).
- **ODD-5 — Suppression metric contract.** A `suppressed` outcome added to
  `foundry_notification_deliveries_total{provider,event,outcome}` (widening `DeliveryOutcome`, `notify.rs:161`,
  which also touches the register-at-0 zero-series and the cardinality guard) vs a sibling
  `foundry_notification_suppressions_total{event}`. Either way, **no recipient PII** in labels (NFR-4, Risk R4).
- **ODD-6 — Resubscribe UX for account-less recipients.** The signed-in page needs an account; a token-based
  resubscribe / an undo-on-the-confirmation-page path for account-less recipients (Risk R7).
- **ODD-7 — Multi-workspace interaction.** How per-workspace unsubscribe presents for an email in several
  workspaces — the settings page lists all the member's workspaces; muting one must not affect another (FR-9,
  Risk R6).

Handoff package: `requirements.md` (context, scope + carve-out, brownfield grounding table with real
`file:line`, inline JTBD, FR/NFR/BR, alternatives, risk table, glossary), `user-stories.md` (US-01..07 with
`job_id` + Elevator Pitch), `acceptance-criteria.md`, the journey trio (`journey-unsubscribe-visual.md`,
`.yaml`, `.feature`), `shared-artifacts-registry.md`, `story-map.md`, `prioritization.md`, `outcome-kpis.md`,
`dor-checklist.md`, and the seven slice briefs under `../slices/`.

## Upstream Changes

**None to a prior wave's assumptions.** No DISCOVER or DIVERGE artifacts exist for this feature (no
`docs/feature/recipient-notification-preferences/diverge/`); the job statements and personas were established
directly in this DISCUSS pass and folded into `requirements.md` (inline JTBD, no `docs/product/` SSOT — house
convention). This feature is the **named successor** the just-shipped `notification-delivery-providers` carved
recipient preferences out to (`docs/evolution/2026-07-11-notification-delivery-providers.md`); its scope
(operator/developer delivery abstraction only) explicitly deferred recipient preferences here. All seams cited
(the notifier `notify.rs:237`, the closed `NotificationEvent` enum `:46-77`, the two suppressible + three
mandatory emit sites, the `InviteToken` signing pattern `foundry-auth/src/lib.rs:354-390`, the uniform refusal
`invites_accept.rs:332-339`, CSRF `csrf.rs:137`, session `session.rs`, the migration/store pattern, the public
route cluster `lib.rs:371-374`, and `public_url` `main.rs:122`) are shipped and verified by `file:line` in the
grounding table. One honest note for DESIGN: the `Notification` struct carries **no `workspace_id`** today
(`notify.rs:117-122`), so a notifier-side suppression hook requires threading workspace context — surfaced as
ODD-3.

## Peer Review

- **Status**: COMPLETE (iteration 1 of max 2) — run via Task (`nw-product-owner-reviewer`, 2026-07-11).
- **Verdict**: **approved** — `critical_issues_count: 0`, `high_issues_count: 0`. All four hard gates PASS:
  DoR 9/9 on all 7 stories; JTBD traceability (every story a `job_id`, none infrastructure-only); Dimension 0
  Elevator Pitch PASS on all 7 (real entry points + concrete observable outputs); every slice carries a
  user-visible value story. Zero LeanUX anti-patterns. Security NFRs (non-enumerable token, mandatory-never-
  suppressed invariant, PII-free metrics, CSRF on both POSTs, least-privilege scope) validated as measurable
  and regression-guarded. No critical/high issues to remediate → no second iteration needed.
- **DoR gate**: PASSED. **Handoff to DESIGN (solution-architect): CLEARED.**
</content>
