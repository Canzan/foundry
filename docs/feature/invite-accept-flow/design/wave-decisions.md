# DESIGN Decisions — invite-accept-flow

> **STATUS: IMPLEMENTED / SHIPPED (finalized 2026-06-14).** All decisions D1–D7 are IMPLEMENTED and
> shipped to `main` via the 18 DES-monitored TDD steps (`770626d`→`bfb7e79`) + review-remediation
> `b8b9b06`. `@all` acceptance 303/303 scenarios (2460/2460 steps) green; fmt + release clippy
> `-D warnings` clean; `cargo xtask check-arch` PASSED (D7 confirmed — no new allow-list line);
> adversarial review APPROVED (no Testing Theater); scoped mutation 100% (4/4) on
> `check_password_policy`. Zero new crate, zero migration. OD-1..OD-5 ratified by user 2026-06-14.
> This CLOSES the dead `/invites/accept` URL flagged by the two prior provisioning features. See
> `docs/evolution/2026-06-14-invite-accept-flow.md`.
>
> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode.
> The deferred `/invites/accept` credential-establishment vertical — the single highest-value
> follow-up that `web-provisioning-flow` ADR-005 / D5 ratified OUT of its v1 (the emitted invite link
> is a DEAD URL on BOTH the CLI and web surfaces today). This feature makes the link LIVE.
> Requirements are COMPLETE (DoR 27/27, DISCUSS is the SSOT — see `../discuss/`).
> Paradigm is ESTABLISHED and NOT re-decided: Rust, modular monolith, ports-and-adapters,
> functional-core / imperative-shell. Legacy per-feature layout. Trunk-based.

## Headline finding (overturns the task's OD-1 premise — read first)

**The `invites` table ALREADY HAS the single-use marker.** `0001_init.sql:93-102` defines:

```sql
CREATE TABLE invites (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invitee_email   TEXT,
    created_by      UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,            -- ⇐ THE single-use marker, already shipped
    used_by         UUID REFERENCES users(id),  -- ⇐ who consumed it, already shipped
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

The task brief, `requirements.md`, and `shared-artifacts-registry.md` all recorded the observed columns
as `id, workspace_id, invitee_email, created_by, expires_at` with **NO** `used_at` — a grounding miss.
The truth: `used_at` + `used_by` shipped in the **very first** migration. `insert_invite`
(`store/lib.rs:491`) and the provision tx (`:1254`) simply do not write them — they DEFAULT to NULL,
which is exactly the "unconsumed" state the consume guard needs.

**Consequence: NO migration is needed.** OD-1 collapses from "add a column + write a consume fn" to
just "write a consume fn" against an already-correct schema. The race-safe guarded-UPDATE idiom is
ALSO already shipped and proven — `claim_bootstrap_token` (`store/lib.rs:258-276`) is the exact
pattern (`UPDATE … SET used_at = $2 WHERE … AND used_at IS NULL AND expires_at > $2 RETURNING …`).
This feature is **even more reuse-heavy than expected**: 0 migrations, 1 new store fn that mirrors a
shipped one, 2 thin handlers, 1 template, 1 tiny password-policy check.

## Reading checklist

- ✓ `../discuss/requirements.md` (FR-1..5, NFR-1..6, BR-1..4 — the SSOT; note the two "findings DESIGN must resolve")
- ✓ `../discuss/user-stories.md` (US-01 accept+sign-in walking skeleton, US-02 refuse-invalid-safely, US-03 password-mistake recovery)
- ✓ `../discuss/acceptance-criteria.md` (AC-01.1..03.4 + the 3 @property criteria)
- ✓ `../discuss/journey-invite-accept-visual.md` (emotional arc; sad paths E1-E8; the uniform-refusal copy)
- ✓ `../discuss/shared-artifacts-registry.md` (9 tracked artifacts; the invite_id/sig/used_at integration risks)
- ✓ `docs/evolution/2026-06-13-web-provisioning-flow.md` (this is its D5 deferred follow-up) + `2026-06-12-multi-workspace-provisioning.md`
- ✓ `crates/foundry-store/migrations/0001_init.sql:93-102` (**`invites` table — `used_at`+`used_by` ALREADY PRESENT**) — latest migration is `0011`
- ✓ `crates/foundry-store/src/lib.rs:258-276` (`claim_bootstrap_token` — the SHIPPED atomic guarded-UPDATE single-use idiom to MIRROR)
- ✓ `crates/foundry-store/src/lib.rs:491-511` (`insert_invite` — writes no consumed-marker; the row the consume tx targets)
- ✓ `crates/foundry-store/src/lib.rs:1216-1266` (`provision_workspace` tx — first-admin user_id is BOTH the user PK AND the invite's `created_by`; this is the join key for the password write)
- ✓ `crates/foundry-store/src/lib.rs:434-451` (`resolve_active_workspace` — REUSE for the landing)
- ✓ `crates/foundry-auth/src/lib.rs:319-347` (`hash_password` / `verify_password`, argon2id OWASP — REUSE verbatim)
- ✓ `crates/foundry-auth/src/lib.rs:354-390` (`InviteToken::verify` — HMAC binds `invite_id||expires_at` — REUSE verbatim)
- ✓ `crates/foundry-app/src/bootstrap.rs:100-209` (the bootstrap claim flow — its `124-139` is the ENUMERATION ORACLE this feature must NOT replicate; its `190-208` is the session-establish + 303 idiom to MIRROR)
- ✓ `crates/foundry-app/src/bootstrap.rs:340-346` (`resource_not_found_page()` — the SHIPPED uniform-404 refusal shape)
- ✓ `crates/foundry-app/src/signin.rs:90-209` (sign-in POST — resolve_active_workspace + session.insert + redirect; the auth-handler shape to MIRROR)
- ✓ `crates/foundry-app/src/signin.rs:285-328` (`ensure_csrf_cookie` + `render_signin_form` — **the public-route CSRF-cookie-on-GET pattern**, the seam for OD/NFR-6)
- ✓ `crates/foundry-app/src/csrf.rs:57-173` (`csrf_middleware`, `is_safe_method`, `is_exempt_path` — double-submit; the POST mounts UNDER it)
- ✓ `crates/foundry-app/src/lib.rs:234-393` (`build_router` — the public-layer mount point; confirmed NO `/invites/accept` route exists)
- ✓ `xtask/src/check_arch.rs:387-402` (LAYER-1e `is_tenant_scoping_allowlisted` — the allow-list the new file may owe)

## Key Decisions (DDD-numbered)

| # | Decision | Rationale | ADR |
|---|---|---|---|
| **D1** | **NO migration.** Reuse the already-shipped `invites.used_at` / `used_by` columns (`0001_init.sql:93-102`) as the single-use marker. Add ONE new store fn `consume_invite(invite_id, user_id, now)` doing an atomic guarded UPDATE `SET used_at = now(), used_by = $user WHERE id = $1 AND used_at IS NULL AND expires_at > now() RETURNING workspace_id, created_by`, mirroring the shipped `claim_bootstrap_token`. | The schema is ALREADY correct (headline finding). `claim_bootstrap_token` proves the exact race-safe idiom in-tree. 0-rows ⇒ refuse. Forward-only, ZERO schema change. | adr-001 |
| **D2** | **One TX = password write + consume.** `set_first_admin_password_and_consume(invite_id, password_hash, now)`: BEGIN; guarded-UPDATE the invite (D1); if 0 rows → ROLLBACK → refuse; else write `password_hash` onto `users WHERE id = invites.created_by` (the first-admin); COMMIT. Neither effect happens without the other (BR-3, NFR-2). | The first-admin user_id == `invites.created_by` (proven in the provision tx). The consume's `RETURNING created_by` names the exact user row to update — no extra lookup, no second query to race. | adr-001 |
| **D3** | **Non-enumerable refusal = collapse all four reasons to ONE byte-identical page (body + status).** Both GET (form-vs-refusal) and POST refusal arms return a single `invite_refusal_page()` — a NEW public-route analogue of `resource_not_found_page()` carrying the journey's "This invite is no longer valid… ask your instance administrator to re-issue" copy, at a SINGLE fixed status (200 OK). Reasons differ ONLY in `tracing` keyed on `invite_id`, never in the body/status (NFR-3, NFR-5). | A 410-vs-404 split would itself be an oracle (NFR-3 forbids it). 200-with-refusal-body avoids even a status oracle and is honest for a public page. Deliberately DIVERGES from `bootstrap.rs:124-139` (distinct Used/Expired/Unknown messages — the leak recorded in `upstream-changes.md`). | adr-002 |
| **D4** | **Public-POST CSRF = issue a CSRF cookie on the GET accept page** (reuse `ensure_csrf_cookie`, the SHIPPED sign-in pattern). The POST mounts UNDER the shipped `csrf_middleware`; the GET sets the double-submit cookie + renders the matching hidden `_csrf` field. NO new middleware, NO `is_exempt_path` entry. | The double-submit middleware needs only a cookie + matching token — NOT a session. `signin.rs:287` already mints the CSRF cookie on a signed-out GET; the accept page copies that idiom verbatim. The `/bootstrap` exemption is NOT reused (it relied on a single-use URL token; the accept POST keeps real double-submit). | adr-003 |
| **D5** | **Password policy = a small reusable `check_password_policy(pwd) -> Result<(), PolicyError>` (min 12 chars, length-first per NIST 800-63B), placed in `foundry-auth`** beside `hash_password` so a future app-wide rollout (sign-up, reset, bootstrap) imports the SAME check. Applied BEFORE the consume tx; a violation re-renders the form inline with the invite UNTOUCHED (FR-5, NFR-4, US-03). | Net-new policy (foundry enforces none today). Co-locating with the hashing primitive in `foundry-auth` makes it the obvious shared home; the accept handler is its first caller, bootstrap/signin can adopt it later WITHOUT moving it. OD-2 ratifies the threshold (proposed 12). | adr-004 |
| **D6** | **Token verify (GET, non-committal) vs DB liveness vs consume (POST, TOCTOU-safe).** GET: `InviteToken::verify(id, expires_at, sig)` (HMAC, no DB) → read invite row → render form ONLY if `used_at IS NULL AND expires_at > now`; NO mutation. POST: re-verify HMAC (defense-in-depth, rejects tampered URLs without a DB hit) → validate password (D5) → run the consume TX (D2) whose guarded-UPDATE re-checks liveness+expiry atomically. The GET liveness check is advisory; the TX guard is authoritative. | Closes the TOCTOU gap the journey calls out (E7): a link cannot be consumed in the GET→POST window because the TX guard is the sole source of truth. GET's check is purely to avoid showing a form for a dead link. `expires_at` is HMAC-bound so it cannot be extended by tampering. | adr-001/002 |
| **D7** | **LAYER-1e allow-list = NO new line needed (confirm).** The accept handler reads an invite by id and writes a credential, but it does NOT name a literal/parsed `workspace_id` the way provisioning does — the workspace is RESOLVED post-consume via `resolve_active_workspace`, exactly as `signin.rs` (already allow-listed) does. If `check_arch` flags the new file, add `invites_accept` to `is_tenant_scoping_allowlisted` (one line), inheriting the `signin` rationale. | The detector trips on a handler that handles a literal workspace id outside the resolution seam. This handler uses the resolution seam itself. Provisional verdict: NO line; DELIVER confirms against the real `check_arch` run (the cheap, reversible fallback is the one-line add). | adr-005 |

## Architecture Summary

- **Pattern**: modular monolith + ports-and-adapters (inherited, in force). The accept flow is a NEW
  **driving adapter** (`foundry-app/src/invites_accept.rs`, two handlers) over a NEW thin **driven**
  store seam (`consume_invite` / `set_first_admin_password_and_consume`) and three SHIPPED driven
  ports reused verbatim (`InviteToken::verify`, `hash_password`, `resolve_active_workspace`, the
  session + CSRF layers). The genuinely-new BACKEND work is the single-statement consume guard + the
  one-TX password-write-and-consume — small, but real (unlike the prior two thin-adapter features).
- **Paradigm**: Rust, composition-over-inheritance, functional-core / imperative-shell — UNCHANGED.
- **Key components** (see `architecture.md` C4 + component diagram):
  - `invites_accept.rs` — `show_accept_form` (GET) + `submit_accept` (POST), the public driving adapter (NEW).
  - 1-2 Askama templates: `invite_accept.html` (set-password form) + the shared/new `invite_refusal.html` (NEW).
  - `Store::consume_invite` + `Store::set_first_admin_password_and_consume` (NEW driven seam, mirrors `claim_bootstrap_token`).
  - `foundry_auth::check_password_policy` (NEW, tiny, shared-home).
  - two `.route("/invites/accept", get(...).post(...))` lines in `build_router` on the PUBLIC layer (EXTEND).
  - Everything else — `InviteToken::verify`, `hash_password`, session, CSRF, `resolve_active_workspace`,
    `resource_not_found_page` shape — is SHIPPED and REUSED.

## Reuse Analysis (verdict: 8 REUSE/EXTEND · 4 CREATE-NEW · 0 RETIRE · **0 MIGRATION**)

| # | Component | File | Decision | Justification |
|---|---|---|---|---|
| 1 | `invites.used_at` / `used_by` columns | `migrations/0001_init.sql:99-100` | **REUSE (verbatim)** | The single-use marker ALREADY EXISTS — no migration (headline finding). |
| 2 | `claim_bootstrap_token` guarded-UPDATE idiom | `store/lib.rs:258-276` | **REUSE (shape)** | The exact race-safe `SET used_at WHERE used_at IS NULL AND expires_at > now RETURNING` pattern `consume_invite` mirrors. |
| 3 | `InviteToken::verify` (HMAC id‖expires_at) | `foundry-auth/lib.rs:377` | **REUSE (verbatim)** | GET + POST signature verification; defense-in-depth before any DB hit. |
| 4 | `hash_password` (argon2id, OWASP) | `foundry-auth/lib.rs:319` | **REUSE (verbatim)** | Writes the chosen password; same primitive as bootstrap/signin. |
| 5 | `resolve_active_workspace` | `store/lib.rs:434` | **REUSE (verbatim)** | Resolves the landing workspace post-consume; identical to `signin.rs:149`. |
| 6 | session insert + 303 redirect idiom | `bootstrap.rs:190-208`, `signin.rs:169` | **REUSE (shape)** | `session.insert(SESSION_KEY_USER_ID, SessionUser{..})` then `SEE_OTHER` → `/`. Auto sign-in (decision 3). |
| 7 | `csrf_middleware` + `ensure_csrf_cookie` | `csrf.rs:96`, `signin.rs:287` | **REUSE (verbatim + shape)** | Public-GET mints the CSRF cookie; POST mounts under the shipped double-submit layer (D4, NFR-6). |
| 8 | `resource_not_found_page()` refusal shape | `bootstrap.rs:340` | **REUSE (shape)** | The uniform-refusal template `invite_refusal_page()` mirrors (single fixed body+status), D3. |
| 9 | `build_router` route registration | `lib.rs:264-386` | **EXTEND** | Add `.route("/invites/accept", get().post())` on the PUBLIC layer (NOT the instance-admin gate). |
| 10 | `Store::consume_invite` + `set_first_admin_password_and_consume` | — (do not exist) | **CREATE NEW (driven)** | One guarded-UPDATE + one TX wrapping it with the password write (D1/D2). Mirrors #2. |
| 11 | `invites_accept.rs` adapter (2 handlers) + templates | — (do not exist) | **CREATE NEW (driving)** | The public GET/POST accept vertical + set-password + refusal templates. |
| 12 | `foundry_auth::check_password_policy` (min 12) | — (does not exist) | **CREATE NEW (tiny, shared)** | Net-new app-wide length-first policy with a reusable home (D5, NFR-4). |
| 13 | LAYER-1e allow-list line | `check_arch.rs:387` | **CONFIRM (likely no-op)** | Provisional: NO line (uses the resolution seam like `signin`); one-line fallback if flagged (D7). |

## Technology Stack

- **Rust** (inherited): axum, askama, tower_sessions, the shipped `csrf_middleware`, sqlx,
  `foundry-auth` (`InviteToken`, `hash_password`). **ZERO new crates.**
- **PostgreSQL** (one instance, inherited): **ZERO migration** — `invites.used_at`/`used_by` shipped in `0001`.
- **Enforcement**: `cargo xtask check-arch` (inherited; likely ZERO new allow-list line, D7).
- **OSS-first / license**: all inherited deps; no proprietary; no new dependency to license.

## Constraints honored

- ONE binary · ONE Postgres · NO Redis · NO Node · NO CDN · **ZERO new crates** · **ZERO migration**.
- The accept page is a PUBLIC (signed-out-accessible) route — NOT behind the instance-admin gate.
- The state-changing POST is CSRF-protected by the SHIPPED double-submit layer (D4, NFR-6).
- Refusals are **non-enumerable**: byte-identical body AND status across expired/used/tampered/unknown (D3, NFR-3).
- No `sig` and no password in logs; refusal/consume `tracing` keys on `invite_id` only (NFR-5).
- The `foundry-acceptance` suite green-before stays green-after.

## Earned-Trust (probe-don't-assume) commitments for DISTILL/DELIVER

- **Single-use under concurrency PROBED**: two concurrent POSTs for one live invite → exactly one
  consume updates 1 row, the other sees 0 rows → uniform refusal; `used_at` set exactly once. The
  guarded-UPDATE (not a read-then-write) is the race oracle (NFR-2, @property, AC-02.6).
- **Non-enumerability PROBED**: a falsifiability litmus REDs if any refusal arm's body OR status
  diverges across {expired, used, tampered, unknown-id} — the revert-reds-it pattern the shipped
  instance-admin 404 uses (NFR-3, @property, AC-02.1/02.2).
- **TOCTOU PROBED**: a link consumed between GET and POST is refused by the TX guard (the GET liveness
  check is advisory only); expiry enforced on GET AND inside the TX (AC-02.7, D6).
- **CSRF PROBED**: a POST with a missing/mismatched `_csrf` is refused (403) by the shipped middleware
  before any consume or password write (NFR-6, AC-02.8).
- **No-secret-leakage PROBED**: a log scan after a full accept + refusal cycle contains no `sig` and
  no password (NFR-5, @property, AC-02.9).
- **Recoverability PROBED**: a weak/mismatched password re-renders inline with the invite UNCONSUMED;
  a valid retry on the SAME invite completes (FR-5, US-03, AC-03.1/03.4) — the policy check runs
  BEFORE the consume TX opens.
- **Tenant landing PROBED**: the landed `workspace_id` == `invites.workspace_id` for the consumed
  invite (via `resolve_active_workspace`); the admin sees only that tenant's data (FR-3, AC-01.4).

## Open decisions — RATIFIED by user 2026-06-14 (before DISTILL)

| # | Decision | Ratified outcome | Status |
|---|---|---|---|
| **OD-1** | Single-use marker + consume shape. | **NO migration**; reuse shipped `invites.used_at`/`used_by` (NOT `consumed_at`); `consume_invite` guarded-UPDATE `SET used_at = now(), used_by = $user WHERE id = $1 AND used_at IS NULL AND expires_at > now() RETURNING workspace_id, created_by`, mirroring `claim_bootstrap_token`; password write + consume in ONE TX. | **RATIFIED** — reuse shipped `used_at`/`used_by`; no migration. |
| **OD-2** | Password-strength threshold. | **Min 12 chars, length-first (NIST 800-63B), no composition rule**; reusable `foundry_auth::check_password_policy` (D5). | **RATIFIED** — user approved introducing the min-12 policy in this feature (net-new app-wide enforcement). |
| **OD-3** | Refusal status code. | **200 OK** with the uniform refusal body (all four invalid reasons byte-identical; a valid invite still renders the form — not an oracle, the holder already knows it is valid). | **RATIFIED** — 200 OK confirmed. |
| **OD-4** | Promote `job_id: claim-my-account-and-sign-in` to `docs/product/jobs.yaml`. | Documentation formality, not behavior; defer. | **DEFERRED** — non-blocking doc cleanup. |
| **OD-5** | LAYER-1e allow-list line for `invites_accept`. | **NO line** (uses the resolution seam like `signin`); one-line fallback if `check_arch` flags it at DELIVER (D7). | **RESOLVED provisionally** — DELIVER confirms against the real run. |

## Peer review (Atlas, nw-solution-architect-reviewer) — iteration 1

**Status: conditionally_approved · 0 critical · 2 high — both HIGH issues RESOLVED in this pass.**

| # | Issue (HIGH) | Resolution |
|---|---|---|
| 1 | AC-01.3 says "302" but design says "303 SEE_OTHER" — divergence. | Grounded: ALL shipped web POST→redirects use `SEE_OTHER` (303) — `bootstrap.rs:208`, `signin.rs:185/198`, etc. 303 is the grounded-correct PRG value; recorded as `upstream-changes.md` Finding 3 (DISCUSS AC wording correction; behavior unchanged). Design uses 303. |
| 2 | Performance omitted from the ISO 25010 quality section. | Added a Performance subsection to `architecture.md`: no v1 performance NFR (single-TX, once-per-workspace); `hash_password` already on `spawn_blocking`. |

DELIVER-time confirmation (non-blocking, per ADR-005/D7): run `cargo xtask check-arch` on the new
`invites_accept.rs`; add one allow-list line only if flagged (reversible fallback). No critical/bias/
feasibility issues found; reviewer noted the reuse-heavy verdict as a strength (no resume-driven bias).

## Upstream Changes

See `upstream-changes.md` — three findings: (1) the `invites.used_at`/`used_by` columns already exist
(grounding correction to `requirements.md` + `shared-artifacts-registry.md` — NO migration; recorded,
parent DISCUSS docs NOT modified); (2) the `bootstrap.rs:124-139` claim flow leaks distinct
expired/used/not-found messages (an enumeration oracle) — recorded as a SECURITY FOLLOW-UP, explicitly
OUT of scope here, bootstrap NOT modified.
