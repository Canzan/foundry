# Upstream Changes — invite-accept-flow (DESIGN findings)

> Two DESIGN findings that refine inherited assumptions. They do not block DESIGN; they are recorded so
> DISTILL/DELIVER (and future readers) do not rediscover or trip on them. Per the nw-design
> back-propagation contract: original quoted, new assumption + rationale stated. **No parent or DISCUSS
> docs are modified from this feature** — this note is the record.

## Finding 1 — The `invites` single-use columns ALREADY EXIST (no migration; grounding correction)

**Original framing** (`../discuss/requirements.md` "grounding table" + the "Two grounded findings"
section, and `../discuss/shared-artifacts-registry.md` `invites_row_used_at`):
> "Columns today: `id, workspace_id, invitee_email, created_by, expires_at`."
> "**No `used_at` / single-use column observed** on the `invites` table … The single-use guarantee
> (NFR-2) requires a consumed marker with an atomic guarded UPDATE. **Open decision OD-1** — confirm/add
> the column."
> (`shared-artifacts-registry.md`): "`invites.used_at` — the single-use marker (SEE OPEN DECISION:
> confirm this column exists or is added)."

**Actual code state** (confirmed during this feature's grounding):
`crates/foundry-store/migrations/0001_init.sql:93-102`:
```sql
CREATE TABLE invites (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invitee_email   TEXT,
    created_by      UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,            -- the single-use marker — PRESENT since 0001
    used_by         UUID REFERENCES users(id),  -- audit: who consumed — PRESENT since 0001
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```
`insert_invite` (`store/lib.rs:491`) and the provision tx (`:1254`) do not WRITE `used_at`/`used_by`
(they default NULL = unconsumed), which is why a column-list inspection of the INSERT statement read as
"no used_at". The schema itself has had the marker since the first migration.

**New assumption** (this feature, `adr-001-single-use-consume-tx.md`, D1):
**NO migration.** Reuse the shipped `used_at` (marker) + `used_by` (audit) columns. The new
`consume_invite` guarded-UPDATE mirrors the shipped `claim_bootstrap_token` (`store/lib.rs:258-276`)
verbatim in shape. OD-1 collapses from "add a column + write a fn" to "write a fn against the existing
schema."

**Rationale**: the column exists; adding a second differently-named one (`consumed_at`) would be
redundant and orphan the shipped `used_by`. Latest migration is `0011`; this feature adds none.

**Impact**: no behavior change to requirements; the single-use GUARANTEE is unchanged. The "is a
migration needed?" risk in `requirements.md`'s risk table resolves to **NO**. DISTILL/DELIVER should
target the existing columns. The DISCUSS docs are NOT edited (their solution-neutral wording — "the
invite is recorded as used exactly once" — remains correct; only the grounding annotation was off).

## Finding 2 — The bootstrap claim flow is an enumeration oracle (SECURITY follow-up, OUT of scope here)

**Original code** (`crates/foundry-app/src/bootstrap.rs:124-139`, the bootstrap CLAIM flow):
```rust
return match status {
    BootstrapTokenStatus::AlreadyUsed => invalid_page(
        StatusCode::GONE, "Link already used",
        "This bootstrap link has already been used to claim the workspace.",
    ),
    BootstrapTokenStatus::Expired => invalid_page(
        StatusCode::GONE, "Link expired",
        "This bootstrap link has expired. Ask the operator to generate a new one.",
    ),
    _ => invalid_page(
        StatusCode::GONE, "Link not found",
        "This bootstrap link is not recognised.",
    ),
};
```

**The leak**: these three arms return DISTINCT bodies (and the same 410 status). A prober can tell
whether a bootstrap token id existed and its state (used vs expired vs never-existed) — an **existence
oracle** (STRIDE: Information Disclosure; OWASP A01/A07-adjacent). This is the exact anti-pattern NFR-3
forbids for the invite-accept flow.

**This feature's stance** (`adr-002-non-enumerable-refusal.md`, D3):
The invite-accept flow deliberately does the OPPOSITE — ONE byte-identical refusal (body + status)
across {expired, used, tampered, unknown-id}, reasons recorded only in `tracing`. The accept flow does
NOT replicate the bootstrap leak.

**Out of scope here / NOT modified**: this feature does **NOT** change `bootstrap.rs`. Fixing the
bootstrap claim flow to a uniform refusal is a **separate security follow-up** (it touches a different,
shipped, working flow with its own tests + non-enumerability considerations; bundling it would pull
cross-cutting scope onto this feature, exactly as `web-provisioning-flow` ADR-005 reasoned about
bundling). Recorded here for the future owner.

**Recommended follow-up** (for a future feature, not this one):
- Collapse `bootstrap.rs:124-139` to a single uniform refusal (drop `bootstrap_token_status`'s
  user-visible branching; keep it for `tracing` only), mirroring `adr-002` here.
- Add a revert-reds-it byte-identity litmus to the bootstrap claim flow.
- Consider whether the 410-vs-other status is itself an oracle on that surface.

## Finding 3 — AC-01.3 says "302" redirect; the grounded-correct value is "303 SEE_OTHER"

**Original** (`../discuss/acceptance-criteria.md` AC-01.3, and `../discuss/user-stories.md` US-01 AC):
> "POST with a valid password consumes the invite, writes the argon2id hash, establishes a session,
> **302-redirect to `/`**."

**Actual code convention** (confirmed during grounding): **every** shipped web-tier POST→redirect uses
`StatusCode::SEE_OTHER` (HTTP 303), not 302 — `bootstrap.rs:208` (the closest analogue: claim → set
session → 303 → /dashboard), `signin.rs:185`/`:198`, `session.rs:152`/`:190`, `projects.rs:330`,
`issues.rs:221`, `comments.rs:593`, `keyboard.rs:331`, `attachments.rs:343`. 303 is the correct
Post/Redirect/Get status (RFC 7231 §6.4.4): it forces the follow-up to be a GET, preventing a form
re-POST on refresh. 302's method-preservation ambiguity is exactly what 303 fixes for a PRG flow.

**New assumption** (this feature, `architecture.md` § "POST /invites/accept", D6/D2): the accept POST
redirects with **303 SEE_OTHER → `/`**, matching the universal shipped convention and the auto-sign-in
PRG pattern. The behavioral intent of AC-01.3 ("redirect to `/`, no second login") is UNCHANGED — only
the precise status code is corrected from 302 to 303.

**Rationale**: aligning to the shipped `SEE_OTHER` convention is the grounded, consistent choice; "302"
in the DISCUSS AC reads as a loose synonym for "redirect", not a deliberate divergence from the
codebase. The AC's observable behavior (lands on `/`, signed in, no separate login) holds either way.

**Impact**: DISTILL should author the acceptance scenario asserting **303** (or, equivalently, "redirects
to `/`" without pinning 302 specifically). Editing the DISCUSS AC-01.3 wording from "302" to "303" is
OPTIONAL and belongs to that artifact's owner; this note is the record so DELIVER does not assert a 302
that the shipped convention (and this design) does not produce.

## Impact summary
- No change to any inherited NFR or user story (behavioral intent preserved across all three findings).
- Finding 1 resolves OD-1's "migration?" to NO (no drift introduced — a pre-existing schema fact the
  DISCUSS grounding annotation missed).
- Finding 2 is a pre-existing security debt in a DIFFERENT flow, surfaced and recorded, explicitly
  deferred. The invite-accept flow's own non-enumerability is fully designed (adr-002).
- Finding 3 corrects AC-01.3's redirect status (302 → 303 SEE_OTHER) to the shipped convention; the
  observable "lands on `/` signed in" behavior is unchanged.
- Correcting the parent/DISCUSS docs is OPTIONAL and belongs to their owners, not this feature.
