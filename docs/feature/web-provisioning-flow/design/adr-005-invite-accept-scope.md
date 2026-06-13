# ADR-005 — Invite-accept / first-admin onboarding scope

## Status
**IMPLEMENTED / SHIPPED** (ratified OUT-of-v1 2026-06-13; finalized 2026-06-13). DESIGN wave,
Propose mode. Option (a) chosen: invite-accept stays OUT of v1. The web success fragment surfaces
the same informational invite link the CLI emits; the `/invites/accept` + password-set +
consume-invite vertical remains the highest-value deferred follow-up (the link is dead on both CLI
and web today). See `docs/evolution/2026-06-13-web-provisioning-flow.md`.

## Context
When a super-admin provisions a workspace, the use-case seeds a first admin and mints a signed
invite link. Both the CLI and (under ADR-001) the web success fragment surface that link:
`{public_url}/invites/accept?id=<invite_id>&sig=<signature>`. The genuinely-open question for this
feature is: **does the first admin's invite link actually work — i.e. does this feature also build
the `/invites/accept` (password-set) route — or is that a further follow-up?**

Grounding (read the code) — the decisive finding (G7):
- The invite link is printed in TWO places: `bootstrap::create_invite` (`bootstrap.rs:275`) and the
  CLI `provision-workspace` (`admin_cli.rs:505`).
- **There is NO route registered for `/invites/accept`** — `build_router` (`lib.rs:234-388`) has no
  such entry. There is **no `consume_invite` store function** (`store/lib.rs` has none). There is
  **no password-set / accept handler** (`signin.rs` has `submit_forgot` but no accept path).
- The provisioned admin row is created with a generated password hash the operator never sees; the
  ONLY way that admin could sign in is through an accept-and-set-password flow that does not exist.
- The parent evolution doc confirms this is a known deferred follow-up: *"Real invite-accept /
  password-set flow. No `/invites/accept` route exists; the provisioned first-admin 'sign in' is
  proven via the shipped `resolve_active_workspace` membership seam (the same approximation the
  parent slices used) … A real invite-accept flow is a follow-up."*

So the emitted link is a **dead URL today**, for BOTH the CLI and the web surface. Building it is a
NEW VERTICAL: a GET `/invites/accept` (verify the signed `InviteToken`, render a set-password form),
a POST that sets the password + consumes the invite + creates/activates the session, and a new
`consume_invite` store transaction (mark invite used, attach the password, enforce expiry/one-shot).

## Options considered
- **(a) Invite-accept is OUT of this v1 — a further follow-up (RECOMMENDED).** This feature ships the
  web provisioning + grant surface only. The success fragment shows the same informational invite link
  the CLI prints (with a note that the accept flow is pending), keeping the web and CLI surfaces at
  parity. The accept vertical is its own increment.
- **(b) Bundle invite-accept INTO this feature.** Build `/invites/accept` + password-set + the
  consume-invite store tx here, so provisioned admins can truly sign in. Maximises end-to-end value,
  but: it is a LARGER vertical than the admin surface itself (new GET+POST routes, signed-token
  verification, a new store transaction with expiry/one-shot semantics, session establishment, CSRF on
  the set-password POST, and its own non-enumerability + brute-force considerations on the token).
  Doubling the feature's scope contradicts "keep the design tightly scoped to a v1 web provisioning
  surface." It also affects the CLI surface (whose link is equally dead), pulling cross-cutting scope.
- **(c) A reduced accept stub** (accept route that just activates the session without a real
  password-set, mirroring the parent's `resolve_active_workspace` approximation). Rejected: a
  half-built auth path is worse than none — it invites a security-sensitive shortcut on the
  credential-establishment path, exactly where Earned Trust demands the full, probed flow, not an
  approximation.

## Decision
**(a) Invite-accept is OUT of this v1 — recommended as a further follow-up.** This feature ships the
web provisioning + grant surface; the success fragment surfaces the same signed invite link the CLI
emits (clearly marked as a link whose accept flow is a pending follow-up, so the operator is not
misled). The real `/invites/accept` + password-set + `consume_invite` vertical is a separate
increment that fixes BOTH the CLI's and the web's dead link at once.

**This is the single decision most likely to change the feature's size**, so it is flagged for
explicit user ratification. If the user wants provisioned admins to sign in end-to-end within THIS
feature, option (b) is chosen and the scope, the component set (a new auth vertical), and the test
surface (token verification, expiry/one-shot, set-password) all expand materially.

## Consequences
- **Positive (of a)**: the feature stays tightly scoped to the web provisioning surface; CLI and web
  remain at parity (both emit the same link); the accept vertical is designed properly as its own
  increment rather than rushed in; no security-sensitive credential path is half-built.
- **Negative (of a)**: a provisioned admin still cannot sign in via the link in v1 (same limitation
  as the shipped CLI — not a regression). The operator must understand the link is pending until the
  accept flow ships (the success fragment says so).
- **Security**: keeping the credential-establishment path out (rather than stubbed) avoids a
  half-built auth shortcut; the full accept flow, when built, gets the Earned-Trust treatment
  (signed-token verification, one-shot consume, expiry, brute-force resistance) as a first-class
  design.

## Relationship
Confirms the parent evolution doc's deferred "Real invite-accept / password-set flow" follow-up and
makes the scope ruling explicit. Recorded (without modifying parent docs) in `upstream-changes.md`:
the emitted `/invites/accept` link has no route behind it on either surface today.
</content>
