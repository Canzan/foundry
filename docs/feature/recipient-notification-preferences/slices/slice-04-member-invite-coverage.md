# Slice 04 — The same one-click unsubscribe covers `member_invite` (closes the v1 boundary)

**Goal**: extend the proven mechanism to the second suppressible event so one opt-out silences **both** invite
emails for a workspace → the recipient's unsubscribe is complete, not a half-measure.
**Story**: US-04.

**IN scope**
- Attach the same signed `UnsubscribeToken` unsubscribe link to the `member_invite` email
  (`crates/foundry-app/src/member_invites.rs:204`), bound to the recipient email + that `workspace_id` (both in
  scope at the emit site).
- Add `MemberInvite` (`notify.rs:49`) to the suppressible allow-list so the notifier suppresses it for an
  unsubscribed `(email_lower, workspace_id)` exactly as `workspace_invite` (slice 01).
- A **single** opt-out row covers **both** suppressible events for the pair (one confirm suffices).
- Acceptance: unsubscribed-from-Northwind Sam re-added → `member_invite` suppressed; a `member_invite` as first
  contact carries its own link and mutes both events; a `member_removed` still delivers (US-02 holds).

**OUT of scope**: any new token/table/route (all reused from US-01); the signed-in surfaces (US-05/06); the
suppression metric (US-07). No new migration.

**Learning hypothesis**: disproves "the unsubscribe mechanism is a general per-workspace opt-out (not a
one-event special-case) — adding a suppressible event is just attaching the link + extending the bounded
allow-list, one opt-out covering both" if `member_invite`'s emit path can't carry the token the same way, or if
one opt-out row fails to cover both events for the pair.

**Seams**: emit site `member_invites.rs:204` (`submit_invite`, built `:198-203`); `NotificationEvent::MemberInvite`
(`notify.rs:49`); the suppressible allow-list + suppression point from slices 01–02; the token/route/table from
slice 01; the mandatory exemption from slice 02 (`member_removed` at `member_invites.rs:292`).
**Dependencies**: slice 01 (US-01, token/table/route/filter) + slice 02 (US-02, mandatory exemption). Reuses
everything; adds no new component. Closes the **v1 boundary** (US-01..US-04).
**Effort**: ~0.5 day (attach the link + one allow-list entry; the mechanism already exists).
</content>
