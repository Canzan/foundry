# DISTILL — Walking Skeleton: workspace-member-invites

> The ONE demo-able end-to-end scenario, green-before-handoff target for DELIVER. All other 29
> scenarios are `@pending`, unskipped one-at-a-time in the DELIVER RED→GREEN→COMMIT cycle.

## The walking skeleton

```gherkin
@walking_skeleton @wiring_e2e @us-01 @us-02
Scenario: An admin invites a teammate who creates an account and joins as a member
  Given Dana Reyes is signed in as an admin of the "Northwind" workspace   # (Background)
  When Dana invites "sam.okafor@northwind.example" to "Northwind"
  And Sam opens his invite link and sets a password meeting the strength policy
  Then a new account is created for "sam.okafor@northwind.example"
  And Sam is signed in on the "Northwind" workspace without a separate login step
  And Sam is a member of "Northwind" and sees no data from any other workspace
  And his invite is recorded as used exactly once
```

## Why this is the right walking skeleton

It chains BOTH genuinely-new surfaces into the single thinnest cut of observable user value, demo-able
to a non-technical stakeholder: "an admin invites a teammate, and the teammate — who had no account —
joins and is working in the workspace one step later." It is the conjunction of US-01 (issuance) and
US-02 (account-creating accept), the two halves the DISCUSS named the walking-skeleton halves.

**Litmus test (Mandate 5 / Dim 5):**
1. Title is a user goal ("invites a teammate who creates an account and joins"), not a technical flow. ✓
2. Given/When are user actions (Dana invites; Sam opens his link and sets a password), not system-state
   setup. ✓
3. Then are user observations (a new account exists for Sam; Sam is signed in; Sam is a member seeing
   only Northwind; the invite is used once) — not internal side effects (no "row inserted", no
   "status 303", no "tx committed"). ✓
4. A non-technical stakeholder confirms "yes — that is exactly what we need: an admin can bring a
   teammate in, and the teammate ends up working in the workspace." ✓

## What it proves wired (end-to-end, real I/O)

The thinnest slice that forces the entire NEW vertical to exist and connect:
- the NEW `member_invites.rs` issuance handlers + the two `/workspace/invites` routes on the SHARED
  layer (admin-gated INSIDE the handler by the SHIPPED `is_workspace_admin`),
- the SHIPPED `insert_invite` (created_by = the admin) + `InviteToken::new` emitting the real link,
- the SHIPPED accept GET rendering the set-password form ("join as a member"),
- the EXTENDED `invite_accept_view` surfacing `invitee_email` + `created_by` for the kind discriminator,
- the member arm of the accept DISPATCH (D3) routing a no-existing-user invite to the NEW tx,
- the NEW `Store::create_member_and_consume` one-TX: guarded-UPDATE consume → INSERT user → INSERT
  `member` membership → set `used_by` → COMMIT,
- the SHIPPED `hash_password` + `check_password_policy`, session auto-sign-in, and
  `resolve_active_workspace` landing,
all over the real session + double-submit CSRF + testcontainers PG16 machinery — NO mocks.

## Strategy (Architecture of Reference + Project Infrastructure Policy)

Per `docs/architecture/atdd-infrastructure-policy.md`: driving ports = the in-process axum router via
`spawn_app` over real HTTP (issuance + accept surfaces); driven-internal ports = real testcontainers
PG16 (invites/users/memberships/sessions) + real CSRF + real `InviteToken`/`hash_password`; driven
external/non-deterministic = the best-effort email seam (output-captured) + a seeded clock for expiry
windows. This replaces the retired per-feature A/B/C/D choice — the treatment is structural (the shipped
crate's established LAYER-3 real-IO convention) and the mechanism is recorded once in the policy.

## Tag / harness behavior

- The walking skeleton carries `@walking_skeleton @wiring_e2e` and is NOT `@pending` → it runs in the
  default lane AND the `@all` lane (per `acceptance.rs`).
- All 29 other scenarios carry `@pending` → excluded in EVERY lane (default + `@all`) until DELIVER
  unskips one per RED→GREEN→COMMIT cycle.
- Feature tags `@workspace-member-invites @real-io @driving_adapter` mark the whole file as LAYER-3
  real-adapter web-driving scenarios.

## DELIVER unskip order (recommended)

1 (WS, green first) → 2,3,4,5 (issuance) → 6,7 (accept GET non-committal) → 8,9,10,11 (member accept +
landing + privilege) → 12 (first-admin regression guard) → 14,15,16 (refusal arms) → 17,18 (collision +
five-arm byte-identity) → 19,20,21 (single-use / concurrency / TOCTOU) → 13 (expiry boundary) →
22,23,24,25 (issuance non-enumerability + CSRF + no-leak) → 26,27,28,29,30 (inline recovery). Security
@property scenarios (18,20,23,25) land after their constituent example scenarios so the byte-identity /
single-create / no-leak invariants are asserted once the arms exist.
