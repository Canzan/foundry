# Slice 02 — Security events are never suppressed (the mandatory invariant)

**Goal**: guarantee that `password_reset`, `password_changed`, and `member_removed` are always delivered, even
for a recipient unsubscribed in every workspace → the confirm page's promise ("you'll still receive
security-critical notifications") is literally true and regression-guarded.
**Story**: US-02.

**IN scope**
- Partition the closed `NotificationEvent` catalog (`crates/foundry-app/src/notify.rs:46-77`) into a **bounded
  suppressible allow-list** {`WorkspaceInvite`, `MemberInvite`} and a **mandatory** set {`PasswordReset`,
  `PasswordChanged`, `MemberRemoved`}.
- The suppression filter (slice 01) applies **only** to the allow-list; mandatory events **skip the unsubscribe
  lookup entirely** and always deliver (BR-3, precedence: mandatory > unsubscribe).
- A dedicated **never-suppress `@property`** litmus: for a recipient unsubscribed in every workspace they belong
  to, every mandatory event is delivered and none is counted `suppressed`; reverting the allow-list guard reds
  it.
- Acceptance: unsubscribed recipient still receives a `password_reset`, a `member_removed`, and a
  `password_changed`; no mandatory event is ever `suppressed`.

**OUT of scope**: the suppression **metric** itself (US-07 — this slice asserts "not suppressed" behaviourally
and via the property, the counter series lands in US-07); the token/route/table (US-01); `member_invite`
coverage (US-04); anything signed-in (US-05/06).

**Learning hypothesis**: disproves "a bounded suppressible **allow-list** (rather than a mandatory **deny-list**)
makes it structurally impossible to suppress a security event, so mandatory delivery is safe by construction" if
the allow-list can be bypassed, if a mandatory event's emit path can reach the suppression check, or if a future
event defaults to suppressible instead of delivered.

**Seams**: `NotificationEvent` closed enum + `as_str` + `ALL` (`crates/foundry-app/src/notify.rs:46-77`); the
mandatory emit sites `signin.rs:255` (`password_reset`), `signin.rs:360` (`password_changed`),
`member_invites.rs:292` (`member_removed`); the suppression point introduced in slice 01
(`notify.rs:237` / emit-site per ODD-3).
**Dependencies**: slice 01 (US-01) — the suppression filter this constrains. DESIGN ODD-3 (where the check
lives, so mandatory can bypass it cleanly). No new persistence, no new migration.
**Effort**: ~0.5 day (a bounded allow-list + a property litmus; tiny surface, critical guardrail).
</content>
