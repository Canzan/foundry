# Upstream Changes — web-provisioning-flow (DESIGN findings)

> One DESIGN finding that refines an inherited assumption. It does not block DESIGN; it is recorded
> so DISTILL/DELIVER (and a future reader of the parent feature) do not rediscover or trip on it.
> Per the nw-design back-propagation contract: original quoted, new assumption + rationale stated.
> **The parent features' docs are NOT modified from this feature** — this note is the record.

## Finding 1 — The emitted `/invites/accept` invite link is a DEAD URL on BOTH surfaces (CLI and web)

**Original framing** (`docs/evolution/2026-06-12-multi-workspace-provisioning.md`, "Deferred / follow-ups"):
> "**Real invite-accept / password-set flow.** No `/invites/accept` route exists; the provisioned
> first-admin 'sign in' is proven via the shipped `resolve_active_workspace` membership seam (the
> same approximation the parent slices used for US-MWT04). A real invite-accept flow is a follow-up."

**Actual code state** (confirmed during this feature's grounding):
- `bootstrap::create_invite` (`crates/foundry-app/src/bootstrap.rs:275`) and the CLI
  `provision-workspace` (`crates/foundry-app/src/admin_cli.rs:505`) BOTH print
  `"{public_url}/invites/accept?id=<id>&sig=<sig>"`.
- `build_router` (`crates/foundry-app/src/lib.rs:234-388`) registers NO `/invites/accept` route.
- `crates/foundry-store/src/lib.rs` has NO `consume_invite` (or equivalent) function.
- `crates/foundry-app/src/signin.rs` has `submit_forgot` but NO accept / set-password handler.

⇒ The link is emitted but **points at a route that does not exist** — it is a dead URL today, for
the CLI surface as much as for the web surface this feature adds.

**New assumption** (this feature, `adr-005-invite-accept-scope.md`, D5):
The web provisioning surface will emit the SAME informational invite link the CLI does (clearly
marked as a link whose accept flow is a pending follow-up). Building the real `/invites/accept` +
password-set + `consume_invite` vertical is **OUT of this feature's v1** — it is a separate increment
that fixes both surfaces' dead link at once. **Flagged for user ratification** (the most
scope-defining open decision).

**Rationale**: the accept vertical (signed-token verification, a one-shot/expiry-enforcing
`consume_invite` store transaction, a set-password form + session establishment, CSRF + brute-force
considerations on the token) is LARGER than the web provisioning surface itself, and it is
cross-cutting (it equally fixes the CLI's dead link). Bundling it would roughly double this feature's
scope and contradict "keep the design tightly scoped to a v1 web provisioning surface." It deserves
its own increment with full Earned-Trust treatment, not a rushed half-build on a credential path.

## Impact
- No change to any inherited NFR or user story.
- This finding is NOT new drift introduced by this feature — it documents a pre-existing gap (the
  parent evolution doc already lists the accept flow as deferred) and confirms that the web surface
  does not change it. The "dead link on both surfaces" framing is the precise record.
- DELIVER should NOT assume `/invites/accept` works when testing the web success fragment; the
  fragment asserts the link is *rendered* (and marked pending), not that following it signs the admin
  in. The provisioned-admin "can act" property remains proven (as in the parent) via the
  `resolve_active_workspace` membership seam, not via a live accept flow.
- Correcting or extending the parent evolution doc (e.g. to note the CLI link is equally dead) is
  OPTIONAL and belongs to the parent feature's owner, not this feature.
</content>
