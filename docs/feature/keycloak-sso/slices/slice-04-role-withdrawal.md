# Slice 04: Withdrawing the provision role closes the Keycloak door

## Goal
A member whom role provisioning let in (D3a) is refused at the next "Sign in with
Keycloak" once the operator withdraws `foundry-user`. Their account and authorship
stay intact, and a re-grant lets them back in. Accounts that were invited or already
existed are unaffected. Story: US-07 (D3b, OD-10 resolved 2026-10-04).

## IN scope
- foundry records, permanently, which accounts provisioning created (D9). DESIGN
  owns the schema; it is a forward migration.
- A backfill for accounts provisioned before this change (OD-14; the recommendation
  is `password_hash IS NULL`).
- A role check for provisioned accounts in `oidc::callback` before the link. Every
  other account still links whatever its roles (D3b).
- The refusal goes through the shipped `refuse()`. Its log reason is distinct from
  `identity lacks provision role` (D12). The response is the generic refusal (D7).
- DISTILL rewrites provisioning scenario 11 to refusal plus intact account, and adds
  scenarios for re-grant, an invited member without the role, and a provisioned
  member after a password reset.

## OUT of scope
- Ending live sessions (D10: revocation applies at the next Keycloak sign-in).
- Gating the password door (OD-12, which recommends leaving it open).
- Deleting accounts or removing memberships (D8).
- Any UI or CLI that shows the provisioned marker.
- Client roles (OD-9 stands).

## Learning hypothesis
**Disproves if it fails:** that adding provenance to the account is enough to tell
"provisioned" from "invited" and keep that distinction for good. If a reset, a
password change or the backfill can blur it, D3b cannot be enforced without a wider
identity model, such as storing the Keycloak `sub` (OQ-3).
**Confirms if it succeeds:** the realm role becomes a working gate in both directions
for Keycloak sign-in, and nothing changes for the operator or invited members.

## Acceptance criteria
All with `FOUNDRY_OIDC_PROVISION_ROLE=foundry-user`:
- AC-7.1: provisioned Nia without the role is refused with no session cookie,
  byte-identically to a wrong password (401 plus the CSRF-masked body).
- AC-7.2: after the refusal she still has one account and one `member` membership,
  with the same display name and the same authored issues and comments.
- AC-7.3: after a re-grant, her next sign-in lands on the board as the same user id,
  and no second account is created.
- AC-7.4: invited Pat Operator without the role still links and lands on the board.
- AC-7.5: Nia is still refused without the role after setting a password through
  forgot-password and reset.
- AC-7.6: the refusal is logged with a distinct reason, checked at unit level, and
  nothing is added to the response.
- AC-7.7: a session established before the withdrawal keeps working until it expires
  or she signs out.
- AC-7.8 and AC-7.9 are provisional, pending OD-12 (password door unaffected) and
  OD-13 (role variable unset means link-only, exactly D3).

## Supersedes
AC-6.8 and DDD-22. Provisioning scenario 11 ("A member provisioned earlier still
signs in after the provision role is withdrawn") is rewritten, not deleted.

## Dependencies
- D3a as shipped: migration `0016`, `Store::provision_federated_member` and the
  DDD-15 order.
- The shipped provider-double step "the identity provider no longer grants the
  newcomer the … realm role".
- OD-12, OD-13 and OD-14 answered by the user before DISTILL finalises AC-7.8 and
  AC-7.9, and before the backfill is written.
- Whether production has provisioning enabled. Prod is on v0.6.0; check before
  sizing the backfill risk.

## Effort
About 1 day. One migration plus backfill, one new branch in the callback, one log
reason, and four or five scenarios. Reference class: the D3a 02-02 step, which also
changed callback ordering and added scenarios on the same harness.

## Dogfood moment
On dev: grant `foundry-user` to a test identity and sign it in. Then withdraw the
role and sign in again; you should get the generic refusal and see the distinct log
reason. Re-grant the role, and the same account is back on the board.
