# Feature Delta — keycloak-sso

> DISCUSS wave, lean density (`~/.nwave/global-config.json`: `lean` + `ask-intelligent`).
> Adds a native OpenID Connect relying-party sign-in path to foundry so an operator
> can reach their board with the Keycloak identity they already use for the rest of
> the cluster. Strictly ADDITIVE: the local password path, the bootstrap claim, and
> the invite-accept flow are unchanged and remain the break-glass route.
>
> Cross-repo: the Keycloak client, the tofu variables, and the cluster e2e test land
> in `jeffbailey/homelab` (`infrastructure/modules/keycloak/`, `infrastructure/modules/foundry/`).
> Tracked here as a pre-requisite, designed there.

## Wave: DISCUSS

### [REF] Persona ID

`operator` — the single person who runs this cluster and this tracker. Already
authenticates to Grafana, Portainer, ArgoCD and Element through the same Keycloak
realm (federated from LLDAP). Reads "sign in" as "the button the other services
have". Research depth: lightweight (one persona, happy path plus the
security-relevant error paths) — the user research is the operator.

### [REF] JTBD one-liner

When I open foundry to file or triage an issue, I want to sign in with the Keycloak
identity I already use elsewhere on the cluster, so I can get to my board without a
password that exists only here. (`job-sso-signin` — `docs/product/jobs.yaml`.)

Job dimensions — functional: reach an authenticated session with no foundry-specific
credential. Emotional: foundry stops being the odd one out. Social: one identity
across the cluster, so issue and comment authorship matches everything else.

Four forces — **push**: foundry holds the only private password on the cluster, and
it is the one most likely to need an email reset at the worst moment. **pull**: one
click from `/sign-in` to a signed-in board, with the same session semantics the
password path already produces. **anxiety**: being locked out of my own tracker when
Keycloak or LLDAP is down, or after a cluster rebuild misconfigures OIDC. **habit**:
typing a foundry password — which is why the SSO path must be additive, never a
migration that removes the fallback.

The anxiety force is what makes D2 and D3 below non-negotiable rather than merely
convenient: an SSO design that can strand the operator has not done the job.

### [REF] Locked decisions

| ID | Decision | Verdict | Rationale |
|----|----------|---------|-----------|
| D1 | Native OIDC relying party inside foundry, NOT an oauth2-proxy front | LOCKED | `infrastructure/modules/foundry/main.tf:10` records that foundry "speaks no OIDC, so it is NOT fronted by oauth2-proxy". A proxy would still leave foundry's own session layer behind it, so the operator authenticates twice unless foundry learns to trust a forwarded header — which is app code anyway, just less honest about it. Native RP puts the real identity in the session, so issue/comment authorship is correct and `/api/v1` + SSE keep working unchanged. |
| D2 | Local password sign-in is RETAINED | LOCKED | Keycloak and LLDAP run on the same cluster as foundry. An SSO-only tracker is unreachable exactly when the cluster is broken and the operator most needs to read the issue describing how to fix it. |
| D3 | Keycloak sign-in links to an EXISTING foundry user by verified email; unknown email is refused | SUPERSEDED 2026-09-27 (see D3a) | No auto-provisioning. `users.email_lower` is already `UNIQUE`, so the match is unambiguous and "two users share an email" is structurally impossible. Keeps `users.password_hash NOT NULL` valid — no migration. Keeps the invite flow as the gate on who is in the tracker, so a realm federating all of LLDAP cannot silently populate foundry. |
| D3a | 2026-09-27 — role-gated, opt-in provisioning on first Keycloak sign-in | LOCKED | Supersedes the "provision nothing" half of D3 (the linking half stands unchanged). With `FOUNDRY_OIDC_PROVISION_ROLE` unset or blank, behaviour is exactly D3. With it set, a verified email with no foundry account whose ID token carries that Keycloak realm role (`realm_access.roles`) gets a `member` account in the instance's original workspace (oldest by creation), display name from `name` → `preferred_username` → email local-part, and NO password (migration `0016` makes `users.password_hash` nullable; the password door refuses a NULL hash through the same timing-equalised path as an unknown email). Missing role → the generic refusal, logged as `identity lacks provision role`. The realm role, not LLDAP membership, is the gate, so federating the directory still populates nothing on its own. Open follow-up: revoking the role does not touch an existing account. |
| D4 | `POST /sign-out` clears the foundry session only — no RP-initiated logout | LOCKED | Matches how the rest of the cluster behaves. Ending the Keycloak SSO session would also sign the operator out of Grafana, Portainer and ArgoCD, which is surprising rather than correct. |
| D5 | The bootstrap-claim and invite-accept flows are untouched | LOCKED | They are how a foundry user comes to exist at all; D3 makes SSO depend on them. A fresh cluster must still be claimable with no Keycloak in play. |
| D6 | SSO is OFF unless configured; foundry starts and serves normally with no OIDC settings | LOCKED | `cargo xtask ci`, `cargo xtask smoke` and a contributor's `run.sh` must not require a Keycloak. Absent config means the "Sign in with Keycloak" affordance is not rendered and the OIDC routes refuse — not a boot failure. |
| D7 | An OIDC refusal is observationally identical to a bad-password refusal | LOCKED | `signin.rs` already returns `GENERIC_SIGNIN_ERROR` and runs `known_bad_hash()` so timing does not leak whether an email is registered (`us-06-timing-symmetry-redesign`). A new sign-in path that answers "no such account" differently would re-open the enumeration oracle that feature closed. |

### [REF] Scope Assessment: PASS

Elephant Carpaccio early gate, run before journey investment. Oversized signals
checked: 5 user stories (threshold >10) — no. Modules touched: `foundry-app`,
`foundry-auth`, plus the homelab tofu/Keycloak wiring (threshold >3 bounded
contexts) — borderline at 3, counted as ONE signal. Walking-skeleton integration
points: Keycloak, the web tier, the session layer = 3 (threshold >5) — no. Effort
estimate ~3 days (threshold >2 weeks) — no. Multiple independent shippable
outcomes — no; auto-provisioning and realm-role→`instance_admins` mapping were
explicitly scoped OUT, leaving one outcome.

One signal of five. Right-sized; no split proposed.

### [REF] Journey (happy path)

| # | Step | Observable output | Emotional state |
|---|------|-------------------|-----------------|
| 1 | Operator opens `https://foundry.<domain>/sign-in` | Existing email/password form, plus a new "Sign in with Keycloak" control | Neutral — recognises the affordance from Grafana |
| 2 | Clicks it | 302 to the Keycloak authorization endpoint | Confidence rising — this is the familiar login page |
| 3 | Authenticates at Keycloak (or is already signed in) | 302 back to `/auth/oidc/callback` | Confident |
| 4 | foundry validates the ID token and links the verified email to a foundry user | 303 to `/`, session cookie set | Confident — arrived |
| 5 | Board renders | Dashboard, authored-by identity matches the Keycloak account | Confident — same person everywhere |

Confidence builds monotonically and never dips; the only downward transitions live
on the refusal paths below, all of which land back on step 1 with a message rather
than an error page.

Shared artifacts across steps — each has one source: `state` (minted at step 2,
verified at step 4, source: the short-lived OIDC cookie), `nonce` (same lifetime,
source: same cookie, compared against the ID token claim), PKCE `code_verifier`
(same), `email` + `email_verified` (source: the validated ID token, never a request
parameter), `SessionUser { user_id, workspace_id }` (source: `users` +
`resolve_active_workspace`, identical to the password path).

Error paths, all landing on step 1 with `GENERIC_SIGNIN_ERROR` per D7: unverified
email; email matching no foundry user; `state` absent or mismatched; `nonce`
mismatched; ID token signature, `iss`, `aud` or `exp` invalid; token endpoint
unreachable; user belongs to no workspace (the existing fail-closed branch).

### [REF] User stories with elevator pitches

All stories trace to `job-sso-signin`.

---

**US-01 — Sign in to foundry with Keycloak** (`job_id: job-sso-signin`) `@walking_skeleton`

As the operator, I want a "Sign in with Keycloak" control on `/sign-in`, so that I
reach my board using the identity I already hold.

#### Elevator Pitch
Before: I cannot reach foundry without typing a password that exists only in foundry.
After: run `click "Sign in with Keycloak" on https://foundry.<domain>/sign-in` → sees `the Keycloak login page, then my foundry dashboard at / with my own name on it`
Decision enabled: I decide whether foundry has joined the cluster's single sign-on, by arriving at my board without a foundry password.

Acceptance criteria:
- AC-1.1 With OIDC configured, `GET /sign-in` renders a control whose href is `/auth/oidc/start`.
- AC-1.2 `GET /auth/oidc/start` responds 302 to the configured Keycloak authorization endpoint, carrying `client_id`, `redirect_uri`, `scope=openid email profile`, a `state`, a `nonce`, and PKCE `code_challenge`; `state`, `nonce` and `code_verifier` are stored in a short-lived, `HttpOnly`, `SameSite=Lax` cookie.
- AC-1.3 `GET /auth/oidc/callback` with a valid `code` and matching `state` exchanges the code server-to-server, validates the ID token (signature via the issuer's JWKS, `iss`, `aud`, `exp`, `nonce`), and reads `email` + `email_verified`.
- AC-1.4 A verified email matching `users.email_lower` establishes a session by the SAME path the password flow uses — `resolve_active_workspace` then `session.insert(SESSION_KEY_USER_ID, SessionUser { user_id, workspace_id })` — and responds 303 to `/`.
- AC-1.5 The resulting session is indistinguishable from a password sign-in: the dashboard renders, `/api/v1` and SSE work, and issues created afterwards are authored by that user.
- AC-1.6 The one-time OIDC cookie is cleared on both success and refusal.

---

**US-02 — A Keycloak identity with no foundry account is refused, non-enumerably** (`job_id: job-sso-signin`)

As the operator, I want SSO to refuse an identity that has no foundry account, so
that federating my whole directory into Keycloak does not silently populate my tracker.

#### Elevator Pitch
Before: I cannot tell whether enabling SSO would let every LLDAP user into my tracker.
After: run `complete the Keycloak sign-in as a realm user with no foundry account` → sees `the foundry sign-in page again with the same generic message a wrong password produces`
Decision enabled: I decide that invites remain the only way into foundry, having watched a valid Keycloak login fail to create an account.

Acceptance criteria:
- AC-2.1 A validated ID token whose `email` matches no `users.email_lower` row creates no user, no membership, and no session.
- AC-2.2 The response is 401 with `GENERIC_SIGNIN_ERROR` — byte-identical to the bad-password refusal (D7).
- AC-2.3 A validated ID token with `email_verified` absent or false is refused identically, even when the email DOES match a foundry user.
- AC-2.4 A user who exists but belongs to no workspace hits the existing fail-closed branch and is refused identically.
- AC-2.5 No refusal branch reveals, in body, status, or header, whether the email is registered.

---

**US-03 — A tampered or replayed callback is refused** (`job_id: job-sso-signin`)

As the operator, I want the callback to reject anything it did not itself initiate,
so that a link someone sends me cannot sign me in as them or them as me.

#### Elevator Pitch
Before: I cannot tell whether foundry's SSO callback is safe to expose on the public internet.
After: run `curl "https://foundry.<domain>/auth/oidc/callback?code=x&state=y"` → sees `HTTP 401 and the generic sign-in page, with no session cookie set`
Decision enabled: I decide the callback is safe to publish through the tunnel, having watched a hand-crafted request be refused.

Acceptance criteria:
- AC-3.1 A callback with no OIDC cookie is refused (no `state` to compare against).
- AC-3.2 A callback whose `state` does not match the cookie is refused.
- AC-3.3 An ID token whose `nonce` does not match the cookie is refused.
- AC-3.4 An ID token failing signature, `iss`, `aud`, or `exp` validation is refused.
- AC-3.5 Replaying a previously consumed callback is refused (the cookie is single-use, cleared per AC-1.6).
- AC-3.6 A token-endpoint failure or timeout is refused, not 500, and is logged.
- AC-3.7 Every refusal in this story returns the same status and body as AC-2.2.

---

**US-04 — Password sign-in and bootstrap claim still work with SSO enabled** (`job_id: job-sso-signin`)

As the operator, I want the local password path intact, so that a Keycloak or LLDAP
outage cannot lock me out of the tracker that documents how to fix it.

#### Elevator Pitch
Before: I cannot adopt SSO without risking being locked out of my own issue tracker during a cluster outage.
After: run `sign in at https://foundry.<domain>/sign-in with my foundry email and password while OIDC is configured` → sees `my dashboard, exactly as before SSO existed`
Decision enabled: I decide it is safe to enable SSO in production, having confirmed the break-glass path still opens.

Acceptance criteria:
- AC-4.1 With OIDC configured, `POST /sign-in` with valid local credentials succeeds unchanged.
- AC-4.2 The existing `us_06_signin` acceptance scenarios, including the timing-symmetry oracle, remain green.
- AC-4.3 `/bootstrap` and `/invites/accept` are unchanged and reachable with no Keycloak available.
- AC-4.4 A user who signed in via Keycloak can subsequently sign in with their local password, and the reverse — the paths do not disturb each other's session or credential state.

---

**US-05 — foundry runs normally with no OIDC configuration** (`job_id: job-sso-signin`)

As a contributor, I want foundry to start and serve without Keycloak settings, so
that the CI gate and a local `run.sh` do not require an identity provider.

#### Elevator Pitch
Before: I cannot run foundry locally or in CI if it demands OIDC configuration at boot.
After: run `./run.sh with no OIDC environment variables set` → sees `foundry serving, and /sign-in rendering the password form with no "Sign in with Keycloak" control`
Decision enabled: I decide whether I need a Keycloak to contribute, by watching foundry come up without one.

Acceptance criteria:
- AC-5.1 With OIDC unset, foundry boots and `/healthz` + `/readyz` answer as today.
- AC-5.2 `/sign-in` renders no Keycloak control when OIDC is unconfigured.
- AC-5.3 `/auth/oidc/start` and `/auth/oidc/callback` refuse when OIDC is unconfigured, with the same generic response as AC-2.2 — not a 500 and not a stack trace.
- AC-5.4 `cargo xtask ci` and `cargo xtask smoke` pass with no Keycloak reachable.
- AC-5.5 Partial configuration (issuer set, secret missing) is a startup refusal with a named error, not a half-enabled flow.

---

**US-06 — A cluster identity holding the provision role gets a foundry account** (`job_id: job-sso-signin`) — *Addendum 2026-09-27 (D3a); supersedes nothing above, extends US-02*

As the operator, I want a Keycloak realm role to decide who may enter foundry on
their first sign-in, so that contributors I have already vetted in the cluster do
not each need an invite.

#### Elevator Pitch
Before: I cannot let a vetted cluster user into foundry without sending them an invite and waiting for them to accept it.
After: run `grant "foundry-user" to a realm user, then sign in as them through "Sign in with Keycloak"` → sees `the foundry dashboard greeting them by name, as an ordinary member of the original workspace`
Decision enabled: I decide whether the realm role, rather than invites, is how contributors join, having watched a role holder arrive and a non-holder be refused.

Acceptance criteria:
- AC-6.1 With `FOUNDRY_OIDC_PROVISION_ROLE` unset or blank, an identity with no foundry account is refused exactly as AC-2.1/AC-2.2, even when its ID token carries a realm role.
- AC-6.2 With it set, a verified email with no foundry account whose ID token lists that role in `realm_access.roles` gets one `member` account in the instance's original (oldest by creation) workspace, with no password, and is signed in by the same path as AC-1.4.
- AC-6.3 The display name is the first of `name` → `preferred_username` → email local-part that is non-blank and at most 64 characters; a longer candidate is skipped, never truncated (OD-7).
- AC-6.4 A missing role, an unconfirmed email, or an instance with no workspace yet is refused byte-identically to AC-2.2 and creates no account (D7). A missing role is logged `identity lacks provision role`.
- AC-6.5 An identity whose email matches an existing account (in any letter case) is linked, never duplicated; that account's display name, password and membership are untouched, whatever roles the token carries.
- AC-6.6 The password door refuses a provisioned (password-less) account with the same status, body and timing as an unknown email.
- AC-6.7 A provisioned member may set a local password through forgot-password and then use the password door (OD-8). The forgot-password answer is identical whether the account has a password, has none, or does not exist.
- AC-6.8 *Pinned, OD-10 open:* an account provisioned earlier keeps signing in through Keycloak after the role is withdrawn, and no second account is created. This records today's deliberate behaviour, not a decision on revocation. **[Superseded 2026-10-04 by AC-7.1 and AC-7.2 (US-07, D3b): OD-10 is resolved, and a provisioned account without the role is now refused. The wording above is kept as recorded.]**

### [REF] Definition of Done

1. All ACs above are green in the acceptance suite at layer 3 (real axum via `build_router`, real Postgres via testcontainers).
2. `cargo xtask smoke` green before each commit; `cargo xtask ci` green before push (foundry AGENTS.md pre-push gate).
3. The check-arch boundary guard passes — no new dependency-direction violation from `foundry-app` into `foundry-auth`.
4. `cargo deny check` passes for any new OIDC dependency.
5. No new dead code; superseded paths deleted outright (foundry AGENTS.md "Dead code").
6. Session establishment is shared with the password path, not duplicated.
7. The homelab side applies cleanly: `make plan` zero-diff after `make apply`, Keycloak client present in the realm import, not hand-created in the admin UI.
8. End-to-end verified on the real cluster: a browser round trip from `https://foundry.<domain>/sign-in` to a signed-in dashboard.
9. `docs/product/jobs.yaml` and this file reflect what shipped; `CHANGELOG.md` updated.

### [REF] Out of scope

- Auto-provisioning foundry users from Keycloak (D3). Revisit if contributors outgrow invites.
- Mapping a Keycloak realm role onto `instance_admins`. Admin grants stay manual.
- RP-initiated (single) logout (D4).
- Replacing local passwords, or migrating existing users to SSO-only.
- OIDC for `/api/v1` machine tokens — those keep their Ed25519 JWT path.
- Any other identity provider. The implementation should be generic OIDC, but only Keycloak is specified, configured, and tested.

### [REF] Walking Skeleton strategy

Strategy **D (configurable)** in the skill's taxonomy, expressed in this repo's
convention: the OIDC path is env-switched (D6), so the skeleton must demonstrate
BOTH the configured round trip and the unconfigured no-op.

The skeleton is US-01 end to end: a real browser-equivalent request to `/sign-in`,
following the control to a REAL Keycloak, back through `/auth/oidc/callback`, to a
rendered dashboard — driven through the production `build_router` composition root,
against a real Postgres. Per the ATDD infrastructure policy (`docs/architecture/atdd-infrastructure-policy.md`),
the driving port (HTTP) is real via `spawn` and the driven-internal store is real via
testcontainers; Keycloak is a driven-EXTERNAL port, so slice 01 must settle whether
it is real (a Keycloak testcontainer) or a fake issuer — see SPIKE-0.

Litmus test (Mandate 5 / Dim 5): a non-technical stakeholder reads it as "the
operator clicks Sign in with Keycloak and lands on their board." Yes.

### [REF] Driving ports

| Driving port | Protocol | Status | Stories |
|---|---|---|---|
| `GET /sign-in` (Keycloak control rendered) | HTTP GET | shipped, extended | US-01, US-05 |
| `GET /auth/oidc/start` | HTTP GET → 302 | NEW | US-01, US-05 |
| `GET /auth/oidc/callback` | HTTP GET → 303 / 401 | NEW | US-01, US-02, US-03, US-05 |
| `POST /sign-in` (local password) | HTTP POST | shipped, regression only | US-04 |
| `POST /sign-out` | HTTP POST | shipped, unchanged (D4) | US-04 |
| `GET /bootstrap`, `GET|POST /invites/accept` | HTTP | shipped, regression only | US-04 |

Both new routes mount alongside `/sign-in` under the existing `csrf_middleware` +
`session_layer`, since the visitor is signed OUT when they start. The callback is a
GET, so `state` — not the double-submit CSRF cookie — is its request-forgery defence.

### [REF] Outcome KPIs

| KPI | Target | Measurement |
|---|---|---|
| Sign-ins requiring a foundry-specific password | 0 in normal operation | Operator's own usage over the two weeks after rollout; local password reserved for break-glass |
| Steps from `/sign-in` to a signed-in board | ≤ 2 (click, authenticate) | Counted in the US-01 acceptance scenario |
| Accounts auto-created by SSO | exactly 0 | `SELECT count(*) FROM users` unchanged across a refused unknown-email sign-in (AC-2.1) |
| Enumeration signal from the new path | 0 distinguishable responses | AC-2.5 / AC-3.7 assert byte-identical status and body across all refusal branches |
| Break-glass availability | 100% | AC-4.1/4.3 green with Keycloak unreachable |
| CI independence from Keycloak | `cargo xtask ci` green with no IdP | AC-5.4 |

### [REF] Pre-requisites

- **SPIKE-0** (slice 01, timeboxed): choose the OIDC mechanism — the `openidconnect` crate versus a thin flow over the already-present `reqwest` (rustls, json) plus `jsonwebtoken` (already a `foundry-auth` dependency) with JWKS fetch and caching. `cargo deny` is a CI gate, so dependency weight is a real constraint. Also settle whether the acceptance harness hosts a Keycloak testcontainer or a minimal fake issuer.
- **Cross-repo, `jeffbailey/homelab`** — designed there, not here: a `foundry` OIDC client in the Keycloak realm import under `infrastructure/modules/keycloak/`, carrying `defaultClientScopes = ["roles","web-origins","acr","basic","profile","email","groups"]` as that module's AGENTS.md requires; redirect URI `https://foundry.<domain>/auth/oidc/callback`; new tofu variables on `infrastructure/modules/foundry/` for issuer, client id and client secret, threaded into the Deployment's env; and an e2e test beside `tests/e2e/test_portainer_keycloak_sso.py`.
- **Deployment**: foundry's image pin is per-cluster (`services-state.<domain>.tfvars`), so shipping this means a Forgejo Actions build and a pin bump, not just `make apply`.
- `users.email_lower` is `UNIQUE` and `password_hash` is `NOT NULL` — D3 is chosen so neither needs a migration. Any drift toward auto-provisioning reopens migration `0015`.

### [REF] Story map and slice order

Backbone (operator activities, left to right): **arrive at foundry → prove who I am
→ get scoped to my workspace → work the board**. This feature touches only "prove
who I am"; the other three are shipped and must stay untouched.

| Slice | Ships | Stories | Effort | Brief |
|---|---|---|---|---|
| P0 (precursor commit, not a slice) | Extract the shared session-establish helper out of `submit_signin` | — (`@infrastructure`) | ~1h | in slice 01 brief |
| 01 | The Keycloak round trip, plus the unconfigured no-op | US-01, US-05 | ~1 day + 2h SPIKE | `slices/slice-01-keycloak-round-trip.md` |
| 02 | Every refusal, non-enumerably; password path regression-guarded | US-02, US-03, US-04 | ~0.5 day | `slices/slice-02-refusals.md` |
| 03 | Keycloak realm client, tofu wiring, production e2e | US-01 re-asserted on the cluster | ~0.5 day | `slices/slice-03-cluster-wiring.md` |

Order rationale: slice 01 first because it carries all the uncertainty — if the
session seam is not credential-agnostic, slices 02 and 03 are wasted, and finding
that out on day one is the cheapest possible failure. Slice 02 second because
refusals are branches on 01's handler and cost nothing once the flow exists, while
shipping 01 to production without them would publish an unhardened callback. Slice
03 last because it is the only slice that cannot be rolled back with a code revert
alone — it touches a live Keycloak realm and a per-cluster image pin.

Carpaccio taste tests: **thin** — pass for 02 and 03; slice 01 brushes the "4+ new
components" test and is documented as an accepted exception in its brief (a partial
OIDC flow has no end-to-end value). **Abstraction first** — pass: the shared session
helper ships as the P0 precursor commit rather than being invented inside a slice.
**Disproves a pre-commitment** — pass: each slice names one. **Production data** —
pass: real Postgres via testcontainers throughout, and slice 03 runs against the
live cluster. **No duplicate-by-scale slices** — pass. **Slice composition gate** —
pass: every slice carries at least one user-visible value story; the only
`@infrastructure` work (P0) is a precursor commit, not a shipped slice.

### [REF] Definition of Ready validation

| # | DoR item | Verdict | Evidence |
|---|---|---|---|
| 1 | Business value articulated | PASS | `job-sso-signin` with all four forces; the anxiety force is what forces D2/D3 |
| 2 | User stories in LeanUX format with elevator pitches | PASS | US-01…US-05, each with Before / After / Decision-enabled |
| 3 | Acceptance criteria testable and unambiguous | PASS | 27 ACs, each naming an observable HTTP status, body, redirect, or row count |
| 4 | Dependencies identified | PASS | SPIKE-0, the P0 precursor, and the cross-repo homelab work, all in Pre-requisites |
| 5 | Job traceability | PASS | All five stories carry `job_id: job-sso-signin`; no `infrastructure-only` escape used |
| 6 | Sized and sliced | PASS | Scope Assessment PASS (1 of 5 oversized signals); three slices, each ≤1 day |
| 7 | Outcome KPIs measurable | PASS | Six KPIs, each with a numeric target and a stated measurement |
| 8 | Out-of-scope explicit | PASS | Six named non-goals, each with the decision that excluded it |
| 9 | Technical feasibility grounded | PASS | Read against the real tree: `reqwest` and `jsonwebtoken` already present, `users.email_lower` UNIQUE, `password_hash` NOT NULL, `resolve_active_workspace` fail-closed, `GENERIC_SIGNIN_ERROR` + `known_bad_hash()` already the non-enumerable posture |

Requirements completeness: **0.96** (27 of 28 identified behaviours have a bound AC;
the one unbound is the OIDC mechanism itself, deliberately deferred to SPIKE-0 as
OD-1 rather than guessed at here).

### [WHY] Alternatives considered

Tier-2 expansion, rendered on request. Triggered by cross-context complexity (four
distinct technologies: Rust/axum, OIDC/Keycloak, PostgreSQL, OpenTofu/Kubernetes)
and by WS strategy D (configurable/env-switched). One row per locked decision, plus
the mechanism choice deferred to SPIKE-0.

#### D1 — Native OIDC relying party

| Alternative | Verdict | Why |
|---|---|---|
| **Native RP inside foundry** | CHOSEN | The identity lands in `SessionUser`, so issue authorship, `/api/v1` and SSE are correct with no further work. One flow to reason about, and the refusal semantics are foundry's own. |
| oauth2-proxy in front | REJECTED | foundry has its own session layer, so the operator authenticates twice — once at the proxy, once at foundry — unless foundry learns to trust a forwarded header, which is app code anyway. It also forces every non-browser client (`/api/v1`, SSE) through a proxy that does not speak machine tokens. |
| oauth2-proxy + trusted `X-Forwarded-User` | REJECTED | The same app-code cost as native RP, minus the ID-token validation, plus a header the app must never accept from anywhere but the proxy. A misrouted ingress becomes total auth bypass. Strictly worse than doing it properly. |
| Keycloak as the only user store (no foundry `users` rows) | REJECTED | `users.id` is a foreign key from issues, comments, memberships, and `instance_admins`. Removing the local row is a schema rewrite of the whole domain, not an auth change. |

#### D2 — Local password retained

| Alternative | Verdict | Why |
|---|---|---|
| **Keep local passwords indefinitely** | CHOSEN | Keycloak and LLDAP run on the same cluster as foundry. SSO-only makes the tracker unreachable exactly when the cluster is broken — and the issue describing how to fix it lives in that tracker. |
| SSO-only once it works | REJECTED | Removes the fallback at the moment of highest dependence. Also strands the `invites/accept` set-password flow, which has no Keycloak equivalent. |
| SSO-only with one break-glass admin | REJECTED | Better than pure SSO-only, but it concentrates recovery on one credential that is used rarely enough to rot unnoticed. A password path exercised by nobody is a password path that fails when tried. |

#### D3 — Link to existing users by verified email

| Alternative | Verdict | Why |
|---|---|---|
| **Invited/existing users only** | CHOSEN | `users.email_lower` is already `UNIQUE`, so the match is unambiguous. `password_hash NOT NULL` stays valid — no migration. Invites remain the gate on who is in the tracker. |
| Auto-provision any realm user | REJECTED | The Keycloak realm federates LLDAP, so everyone in the directory would silently gain a foundry account and appear in assignee pickers. Needs migration `0015` to make `password_hash` nullable, and makes the invite flow vestigial. |
| Auto-provision gated on a realm role | REJECTED (for now) — CHOSEN 2026-09-27 as D3a, opt-in via `FOUNDRY_OIDC_PROVISION_ROLE`; role revocation for existing accounts remains open | Genuinely reasonable, and the natural next step if contributors outgrow invites. Rejected here only because it adds role-claim plumbing plus an unanswered question — what happens to an existing foundry user when the role is revoked? Deleting their account orphans authorship; leaving it makes the gate cosmetic. Not worth answering before anyone needs it. |
| Match on Keycloak `sub` rather than email | REJECTED | `sub` is stable and email is not, which argues for it — but no foundry user has a `sub` until their first SSO sign-in, so the *first* match must be by email regardless. Storing `sub` afterwards is a sensible hardening once the flow exists; it is not needed to make it work. |

#### D4 — Local logout only

| Alternative | Verdict | Why |
|---|---|---|
| **Clear the foundry session only** | CHOSEN | Matches Grafana, Portainer and ArgoCD on this cluster. No new endpoint, no new scenario. |
| RP-initiated logout to Keycloak | REJECTED | Signing out of foundry would sign the operator out of every other service in the realm. Correct by the letter of the spec, surprising in practice. |
| Both, behind separate controls | REJECTED | The right answer for a shared machine, and cheap to add later. Rejected now as an extra affordance and an extra scenario for a single-operator tracker on trusted devices. |

#### D5 — Bootstrap and invite flows untouched

| Alternative | Verdict | Why |
|---|---|---|
| **Leave both alone** | CHOSEN | They are how a foundry user comes to exist, and D3 makes SSO depend on that. A fresh cluster must be claimable with no Keycloak running. |
| Route bootstrap through Keycloak | REJECTED | Circular: bootstrap runs before any user exists, and the Keycloak client for foundry is created by the same `make apply` that deploys foundry. A first-boot ordering dependency on an IdP is a bootstrap that fails on a fresh cluster. |

#### D6 — SSO off unless configured

| Alternative | Verdict | Why |
|---|---|---|
| **Runtime env switch, absent = off** | CHOSEN | `cargo xtask ci` and a contributor's `run.sh` must not require an IdP. Absent config renders no control and refuses the routes — not a boot failure. |
| Required configuration (fail to start) | REJECTED | Turns every local checkout and every CI run into a Keycloak dependency. Also makes the homelab deploy ordering brittle: foundry could not start before its Keycloak client existed. |
| Compile-time cargo feature | REJECTED | Means two binaries. The cluster image would carry the feature and CI would not, so the CI-green/production-identical invariant that `cargo xtask ci` exists to protect would be broken by construction. |
| Partial config tolerated (issuer set, secret missing) | REJECTED | Explicitly refused at startup (AC-5.5). A half-enabled auth flow is the worst of both — it renders the control and then fails at the callback, which reads as a broken deploy rather than an unconfigured one. |

#### D7 — Refusals are observationally identical

| Alternative | Verdict | Why |
|---|---|---|
| **One generic refusal for every branch** | CHOSEN | `signin.rs` already returns `GENERIC_SIGNIN_ERROR` and burns a `known_bad_hash()` compare so timing does not leak registration. A second sign-in path answering differently reopens the oracle `us-06-timing-symmetry-redesign` and `bootstrap-claim-enumeration-oracle` closed. |
| Specific messages ("no foundry account for this identity") | REJECTED | Much kinder, and genuinely tempting since the visitor already proved control of the email at Keycloak. But the callback is reachable by anyone who can drive a browser to Keycloak, so a specific message turns foundry into an account-existence oracle for the whole realm. |
| Specific message only after a successful token validation | REJECTED | Narrows the oracle without closing it: the attacker just completes a real Keycloak login first. Complexity for a partial fix. |

#### OD-1 — OIDC mechanism (SPIKE-0 decides)

| Option | Pull | Push |
|---|---|---|
| `openidconnect` crate | Discovery, JWKS rotation, PKCE and ID-token validation are all solved and audited. Least room for a subtle validation bug — the class of bug that matters most here. | Large transitive tree behind a `cargo deny check` gate; a licence or advisory hit fails CI on a schedule nobody controls. |
| Thin flow over `reqwest` + `jsonwebtoken` | Both are already workspace dependencies (`reqwest` with rustls-tls + json; `jsonwebtoken` already in `foundry-auth`), so the dependency delta is zero and `cargo deny` risk does not move. | Hand-rolled JWKS fetch, caching, key rotation and claim validation. Every one of those is a place to get security subtly wrong, and the acceptance suite only catches what it thinks to assert. |

The tie-breaker is not size but blast radius: a dependency problem fails the build
loudly, a validation problem fails silently and authenticates the wrong person.
SPIKE-0 should weigh it that way rather than by dependency count.

### Open decisions for DESIGN

- **OD-1** OIDC mechanism and its dependency footprint (SPIKE-0 resolves).
- **OD-2** Where the one-time `state`/`nonce`/`code_verifier` live — a dedicated signed cookie versus the existing `tower-sessions` store keyed pre-authentication. The ACs pin the observable behaviour, not the carrier.
- **OD-3** Exact route paths. Pinned as `/auth/oidc/start` and `/auth/oidc/callback`; if DESIGN moves them, the Keycloak client's redirect URI in homelab moves in the same change.
- **OD-4** Whether the Keycloak control is a link or a form POST. ACs assert the href target, so either passes.

## Wave: DESIGN

Scope: application / components (Decision 0). Mode: propose (Decision 1). Density
lean; `nwave-ai outcomes check-delta` exits 0 (registry empty — no collisions).
Paradigm is not newly chosen: the codebase is already a ports-and-adapters modular
monolith with trait-injected effects (`Arc<dyn Clock>`, `Notifier`, `Store`), and
this feature follows it. Nothing written to `CLAUDE.md`.

### [REF] Decisions

| ID | Decision | Verdict |
|---|---|---|
| DDD-1 | A new `crates/foundry-oidc` owns the OIDC protocol (discovery, JWKS, PKCE, token exchange, ID-token validation); `foundry-app` owns the two handlers and the wiring | LOCKED |
| DDD-2 | Thin implementation over the ALREADY-PRESENT `reqwest` (rustls-tls, json) and `jsonwebtoken` 9.3.1. Zero new runtime dependencies | LOCKED |
| DDD-3 | `xtask check-arch` gains `check_oidc_alg_pin`, scanning `crates/foundry-oidc/src` and requiring `algorithms = [RS256]` with no other token | LOCKED |
| DDD-4 | `AppState.oidc: Option<Arc<OidcProvider>>`, mirroring `machine_token_signer` exactly | LOCKED |
| DDD-5 | The one-time `state`/`nonce`/PKCE `code_verifier` ride in a dedicated HMAC-signed cookie, NOT a pre-auth `tower-sessions` row | LOCKED |
| DDD-6 | The session-establish tail of `submit_signin` is extracted to `signin::establish_session` and called by both paths | LOCKED |
| DDD-7 | Routes pinned at `GET /auth/oidc/start` and `GET /auth/oidc/callback`, mounted beside `/sign-in` under `csrf_middleware` + `session_layer` | LOCKED |
| DDD-8 | The Keycloak control is a plain `<a href="/auth/oidc/start">`, not a form POST | LOCKED |
| DDD-9 | Boot validates configuration SHAPE only. Discovery and JWKS are fetched lazily on first use; network failure refuses the sign-in, never the boot | LOCKED |
| DDD-10 | JWKS is cached with a TTL and refreshed on unknown `kid`, rate-limited to one refresh per interval | LOCKED |
| DDD-11 | One `oidc_refusal()` function produces every refusal, delegating to the same `GENERIC_SIGNIN_ERROR` + `render_signin_form` the password path uses | LOCKED |
| DDD-12 | Acceptance drives a FAKE issuer with a fixed RSA test keypair; the real Keycloak is exercised only by slice 03's cluster e2e | LOCKED |

Rationale for the three that are not obvious:

**DDD-1/DDD-3 — crate placement is a security decision, not a tidiness one.**
`xtask/src/check_arch.rs::check_jwt_alg_pin` scans `crates/foundry-auth/src`, fires on
any file constructing a `jsonwebtoken::Validation`, and demands an `algorithms` list
containing `EdDSA` and **no other algorithm token** — `RS256` is in its explicit
reject list. Keycloak signs ID tokens RS256, so OIDC validation inside `foundry-auth`
fails `cargo xtask ci` by construction. Widening that guard to tolerate RS256 would
loosen the pin protecting machine tokens, and `pins_algorithms_to_eddsa` only inspects
the FIRST `algorithms` list in a file, so a per-file rule cannot express "EdDSA here,
RS256 there". A separate crate keeps the two credential classes under two independent,
per-class pins. The reason to hand-roll rather than adopt `openidconnect` follows from
the same fact: a guard can only pin an algorithm it can see in first-party source.

**DDD-5 — the state carrier must not write to the database.** `/auth/oidc/start` is
reachable by anyone, signed out. Minting a `tower-sessions` row per click is an
unauthenticated, unbounded INSERT on a public endpoint — a disk-fill vector that also
pollutes the session table. An HMAC-signed cookie over the existing `SESSION_SECRET`
is stateless, self-expiring, and uses the shipped `foundry_auth::sign`/`verify`
primitives that `InviteToken` and `UnsubscribeToken` already use for exactly this
shape. Single-use is enforced by clearing the cookie on both outcomes (AC-1.6).

**DDD-9 — foundry must start when Keycloak is down.** D2 exists because Keycloak,
LLDAP and foundry share a cluster: the tracker has to open when the cluster is broken.
A boot-time discovery fetch would invert that, making foundry's readiness depend on
the IdP it exists to survive. So boot checks only that the issuer URL parses, the
client id and secret are non-empty, and the redirect URL is absolute — a partial or
malformed config is `health.startup.refused` (AC-5.5, the shipped
`MACHINE_TOKEN_SIGNING_KEY` pattern at `main.rs:204`), while an unreachable Keycloak
is a refused sign-in attempt (AC-3.6) and nothing more.

### [REF] Reuse Analysis

| Existing component | File | Overlap | Decision | Justification |
|---|---|---|---|---|
| `submit_signin` session tail | `crates/foundry-app/src/signin.rs:150-198` | Establishing a session from a resolved user | **EXTEND** (extract `establish_session`) | ~20 LOC extraction versus duplicating `resolve_active_workspace`, the fail-closed no-workspace branch, the `SessionUser` insert and the 303. Duplication would let the two paths drift on the fail-closed branch — the branch AC-2.4 depends on |
| `GENERIC_SIGNIN_ERROR` + `render_signin_form` | `crates/foundry-app/src/signin.rs:543` | Rendering a refusal | **EXTEND** (call verbatim) | D7 requires byte-identical refusals; a second renderer guarantees eventual drift |
| `foundry_auth::sign` / `verify` (HMAC over `SESSION_SECRET`) | `crates/foundry-auth/src/lib.rs:251,260` | Signing a short-lived opaque blob | **EXTEND** | `InviteToken` and `UnsubscribeToken` already use these primitives for the same shape; the OIDC state cookie is a third instance, not a new mechanism |
| `MachineTokenVerifier` | `crates/foundry-auth/src/lib.rs:146` | Verifying a JWT | **CREATE NEW** (`foundry-oidc`) | Different algorithm class (RS256 vs EdDSA), different key source (remote rotating JWKS vs static PEM set), different trust model (external issuer vs self-issued). Hard evidence, not preference: co-locating them fails `check_jwt_alg_pin`, which rejects any non-EdDSA token in `foundry-auth`'s `algorithms` list |
| `machine_token_signer: Option<Arc<..>>` boot wiring | `crates/foundry-app/src/main.rs:204` | Optional-feature boot with a refusal probe | **EXTEND** (reuse the pattern) | Same `Option` shape, same `health.startup.refused` event, same metrics counter. AC-5.5 asks for behaviour this file already implements |
| `check_jwt_alg_pin` | `xtask/src/check_arch.rs:455` | Build-time algorithm pinning | **EXTEND** (add sibling scanner) | ~30 LOC sibling over a second directory, versus generalising one function with per-crate config. Two independent pins fail independently, which is the point |
| `spawn_app()` in-process router | `foundry_app::test_support` | Acceptance driving port | **EXTEND** | ATDD policy names it the HTTP driving mechanism; no new harness |
| `resolve_active_workspace` | `foundry-store` | Scoping a session to a workspace | **EXTEND** (verbatim) | Reached through `establish_session`; the fail-closed branch is reused, not reimplemented |
| `csrf_middleware` + `session_layer` in `build_router` | `crates/foundry-app/src/lib.rs:368` | Route mounting | **EXTEND** | Both new routes mount on the existing signed-out-accessible layer, exactly as `/sign-in` and `/invites/accept` do |

One CREATE NEW, carrying build-guard evidence. Zero unjustified.

### [REF] Component decomposition

| Component | Path | Change |
|---|---|---|
| `foundry-oidc` crate | `crates/foundry-oidc/` | **NEW** — `OidcConfig` (shape-validated at boot), `OidcProvider` (lazy discovery + cached JWKS), `AuthRequest` (state/nonce/PKCE minting), `IdTokenClaims` + RS256-pinned validation, `OidcError` |
| OIDC handlers | `crates/foundry-app/src/oidc.rs` | **NEW** — `start`, `callback`, `oidc_refusal`, state-cookie encode/decode over `foundry_auth::sign`/`verify` |
| Session establish | `crates/foundry-app/src/signin.rs` | **EXTRACT** — `establish_session(state, session, user) -> Response`, called by `submit_signin` and by `callback` (P0 precursor commit) |
| Sign-in template | `crates/foundry-app/src/signin.rs::render_signin_form` | **EDIT** — render the `<a href="/auth/oidc/start">` control only when `state.oidc.is_some()` |
| Composition root | `crates/foundry-app/src/main.rs` | **EDIT** — build `Option<Arc<OidcProvider>>` from env; refuse to start on partial config |
| App state | `crates/foundry-app/src/lib.rs::AppState` | **EDIT** — add `pub oidc: Option<Arc<foundry_oidc::OidcProvider>>` |
| Router | `crates/foundry-app/src/lib.rs::build_router` | **EDIT** — two route mounts beside `/sign-in` |
| Boundary guard | `xtask/src/check_arch.rs` | **EDIT** — add `check_oidc_alg_pin` |
| Dependency direction | `deny.toml` | **EDIT** — ban `foundry-oidc` with `wrappers = ["foundry-app", "foundry-acceptance"]`, mirroring the `foundry-api` rule so nothing else can reach the protocol crate |

### [REF] Driving ports

| Port | Handler | Method | Stories |
|---|---|---|---|
| `/auth/oidc/start` | `oidc::start` | GET → 302 | US-01, US-05 |
| `/auth/oidc/callback` | `oidc::callback` | GET → 303 or 401 | US-01, US-02, US-03, US-05 |
| `/sign-in` (control rendered) | `signin::show_form` | GET | US-01, US-05 |
| `/sign-in` (password) | `signin::submit_signin` | POST | US-04, regression |

### [REF] Driven ports and adapters

| Driven port | Class | Production adapter | Acceptance treatment |
|---|---|---|---|
| Keycloak discovery + JWKS | external, non-deterministic | `reqwest::Client` GET, cached | **Fake issuer** with a FIXED RSA test keypair — real RS256 crypto, fixture key material (the shipped `MachineTokenSigner` pattern) |
| Keycloak token endpoint | external, non-deterministic | `reqwest::Client` POST (client secret, PKCE verifier) | Same fake issuer; unreachable/slow arms drive AC-3.6 |
| `users` lookup by `email_lower` | internal | `Store` | **Real** Postgres, testcontainers + per-scenario schema |
| Session store | internal | `tower-sessions-sqlx-store` | **Real**, same schema |
| Clock (`exp`, cookie TTL, JWKS TTL) | external, non-deterministic | `Arc<dyn Clock>` | Shipped `MockClock` |

Per the ATDD infrastructure policy's Architecture of Reference: driven-internal is
real, driven-external/non-deterministic is faked. Keycloak is squarely the latter, so
no `@docker-compose` Keycloak container is added — that lane costs 30–60s per
scenario and would buy coverage slice 03 already provides against the real thing.

### [REF] Technology choices

| Concern | Choice | Note |
|---|---|---|
| Language / edition | Rust 1.88, edition 2021 | Workspace-pinned; unchanged |
| HTTP client | `reqwest` 0.12 (rustls-tls, json) | **Already a workspace dependency** |
| JWT validation | `jsonwebtoken` 9.3.1 | **Already a dependency.** `DecodingKey::from_jwk(&Jwk)` and the `jwk` module cover JWKS→key directly — verified against the published source, so the JWKS path needs no `rsa`/`base64` addition |
| PKCE `code_challenge` | `sha2` + `base64` | Both already workspace dependencies (`foundry-auth` uses them) |
| Randomness (state, nonce, verifier) | `rand` | Already a dependency |
| State carrier | `foundry_auth::sign`/`verify` HMAC over `SESSION_SECRET` | Shipped primitive |

**Net new runtime dependencies: zero.** This is the whole reason DDD-2 survives the
`cargo deny check` gate without a licence or advisory review.

### [REF] C4 — System Context

```mermaid
C4Context
  title System Context — foundry with Keycloak SSO
  Person(operator, "Operator", "Runs the cluster; files and triages issues")
  System(foundry, "foundry", "Issue tracker and project board")
  System_Ext(keycloak, "Keycloak", "OIDC provider for the cluster, federated from LLDAP")
  System_Ext(browser, "Browser", "Carries the operator between foundry and Keycloak")
  Rel(operator, browser, "Uses")
  Rel(browser, foundry, "Signs in, works the board", "HTTPS")
  Rel(browser, keycloak, "Authenticates", "HTTPS / OIDC authorization code + PKCE")
  Rel(foundry, keycloak, "Discovery, JWKS, token exchange", "HTTPS, server-to-server")
```

### [REF] C4 — Container

```mermaid
C4Container
  title Container — OIDC sign-in path
  Person(operator, "Operator")
  System_Ext(keycloak, "Keycloak", "External OIDC provider")
  Container_Boundary(f, "foundry") {
    Container(app, "foundry-app", "Rust / axum", "Handlers, composition root, session + CSRF layers")
    Container(oidc, "foundry-oidc", "Rust", "NEW — discovery, JWKS cache, PKCE, RS256-pinned ID-token validation")
    Container(auth, "foundry-auth", "Rust", "Password hashing, HMAC sign/verify, EdDSA machine tokens")
    Container(store, "foundry-store", "Rust / sqlx", "users, sessions, workspaces")
  }
  ContainerDb(pg, "PostgreSQL", "Postgres 16", "users.email_lower UNIQUE, tower_sessions")
  Rel(operator, app, "GET /sign-in, follows the Keycloak control", "HTTPS")
  Rel(app, oidc, "Mint auth request; validate callback")
  Rel(oidc, keycloak, "Discovery, JWKS, token exchange", "HTTPS")
  Rel(app, auth, "HMAC-sign the one-time state cookie")
  Rel(app, store, "Look up user by email_lower; establish session")
  Rel(store, pg, "sqlx")
```

`foundry-oidc` depends on neither `foundry-store` nor `foundry-auth` — it is pure
protocol, taking claims in and handing validated claims out. The link from an
identity to a `users` row happens in `foundry-app`, which is where the tenancy rules
already live. `deny.toml` enforces the direction.

### [REF] Open questions deferred to DISTILL / DELIVER

- **OQ-1** JWKS cache TTL and the refresh-on-unknown-`kid` rate limit. Behaviour is pinned (a rotated key must not require a restart); the numbers are a DELIVER tuning choice.
- **OQ-2** Whether the fake issuer is a `wiremock`-style in-process HTTP server or a hand-rolled axum app in the acceptance crate. `wiremock` would be a new dev-dependency and must clear `cargo deny`; an axum fake reuses what is there. DISTILL decides when it writes the harness.
- **OQ-3** Whether `id_token`'s `sub` is persisted on first link, as hardening against email change. Out of scope for this feature's ACs; recorded because DISCUSS surfaced it and the column would be a migration.
- **OQ-4** Exact `state` cookie TTL. Pinned as "short"; 10 minutes is the working assumption, matching typical authorization-code lifetimes.

### [WHY] Trade-off analysis

Tier-2 expansion, rendered on request. Quality attributes ranked for THIS feature,
then what each locked decision bought and what it paid.

#### Attribute priority

| Rank | Attribute | Why it ranks here |
|---|---|---|
| 1 | Availability of the operator's own access | Straight from the JTBD anxiety force. foundry holds the issue describing how to fix the cluster; an auth design that can strand the operator has failed the job regardless of its other merits |
| 2 | Auditability of credential verification | The failure mode that matters is algorithm confusion — it authenticates the wrong person and emits no signal. Everything about DDD-1/2/3 is buying visibility into that class |
| 3 | Confidentiality (non-enumerability) | The callback is publicly reachable, so a chatty refusal turns foundry into an account-existence oracle for the whole Keycloak realm |
| 4 | Testability / CI independence | `cargo xtask ci` is the pre-push gate; a gate that needs an IdP is a gate that gets skipped |
| 5 | Maintainability | Real, and the thing this design spends most freely |
| 6 | Time to market | ~2 days either way; not a differentiator |
| 7 | Performance / scalability | One operator, occasional sign-in. Effectively irrelevant, and saying so prevents it being smuggled in as a tie-breaker |

#### What each decision bought and paid

| Decision | Bought | Paid | Mitigation |
|---|---|---|---|
| DDD-1/2/3 — own crate, hand-rolled, second guard | **Auditability (2)**: the RS256 pin sits in first-party source where `check-arch` can enforce it, and the EdDSA pin is untouched. Zero dependency delta, so `cargo deny` risk does not move | **Maintainability (5)**: JWKS fetch, cache, and key rotation are hand-written. This is the single most likely place in the feature for a real bug | The guard fails the build on a lost pin; acceptance mints wrong-`alg`, wrong-`kid` and wrong-key tokens. Neither catches a rotation-logic bug, which is why OQ-1's behaviour (a rotated key must not require a restart) is pinned even though its numbers are not |
| DDD-9 — lazy discovery | **Availability (1)**: foundry starts and serves when Keycloak is down, which is the whole point of keeping local passwords | Diagnosability of a *semantically* wrong config. Shape validation catches an unparseable issuer, not an issuer pointing at the wrong realm — that stays invisible until someone tries to sign in | See the residue below |
| DDD-5 — signed cookie | **Availability (1)** and replica-independence: a start on one replica and a callback on another both work. No unauthenticated DB write on a public endpoint | Revocability: an outstanding state cookie cannot be invalidated server-side before its TTL | Short TTL, cleared on both outcomes. See the replay residue below |
| DDD-11 — one refusal function | **Confidentiality (3)**: the two sign-in paths physically cannot drift on refusal shape | A caller debugging a failed SSO login gets no detail from the response | Detail goes to `tracing` at the server, which is where it belongs |
| DDD-12 — fake issuer | **Testability (4)**: the suite stays fast and IdP-free; no 30–60s `@docker-compose` lane | Fidelity: real Keycloak claim shapes and quirks are not exercised until slice 03 | Slice 03's cluster e2e is the contract test. This is the standard driven-external trade the ATDD policy already makes for SMTP |
| DDD-4 — `Option<OidcProvider>` | **Testability (4)** and a clean off state | An extra `Option` unwrap on every OIDC path | Mirrors the shipped `machine_token_signer`; no new idiom to learn |

#### Two residues worth naming

**AC-3.5's replay protection does not come from where it looks like it does.** Clearing
the cookie stops a replay only if the client cooperates by discarding it. A client that
*retains* the cookie and replays the callback is actually refused because the
authorization code is single-use **at Keycloak** — the second exchange fails at the
token endpoint. The `nonce` comparison is a second, independent layer. DELIVER must not
implement AC-3.5 as "we cleared the cookie, therefore replay is impossible": the
scenario should assert refusal against a genuinely replayed code, which exercises the
mechanism that actually provides the property.

**A shape-valid but wrong issuer is undetectable until first sign-in.** DDD-9 trades
this away deliberately to buy availability, and the trade is right. But the failure
lands on the operator mid-sign-in with a generic error (DDD-11), which is the least
diagnosable moment possible. The proportionate mitigation is not to move the check to
boot — that would undo DDD-9 — but to make it *available on demand*: a
`foundry doctor oidc-check` subcommand that resolves discovery, fetches JWKS, and
reports what it found. There is precedent for exactly this shape
(`foundry doctor provision-workspace`, `foundry doctor backup-verify`), including its
driving-port treatment in the ATDD policy. Not in scope for this feature — recorded
here so it is a considered omission rather than an oversight.

### Changed Assumptions

**Original (DISCUSS, `feature-delta.md` § [WHY] Alternatives considered, OD-1):**

> "The tie-breaker is not size but blast radius: a dependency problem fails the build
> loudly, a validation problem fails silently and authenticates the wrong person.
> SPIKE-0 should weigh it that way rather than by dependency count."

That reasoning stands, but it was written without knowledge of
`xtask/src/check_arch.rs::check_jwt_alg_pin`. **New assumption:** the same
blast-radius test now favours the hand-rolled flow, because foundry owns a mechanism
that converts the silent failure class (algorithm confusion) into a loud build-time
failure — and that mechanism can only inspect first-party source. Adopting
`openidconnect` would move ID-token validation somewhere no `check-arch` rule can
reach. The conclusion inverted; the criterion did not.

**Consequence for SPIKE-0:** its first question is answered by this wave and the
2-hour timebox drops to the second question only, which DDD-12 also answers by
policy. SPIKE-0 is therefore **closed before it ran**; slice 01's brief should be read
with OQ-2 as its only remaining unknown.

**Original (DISCUSS, US-03):**

> "AC-3.5 Replaying a previously consumed callback is refused (the cookie is
> single-use, cleared per AC-1.6)."

The parenthetical names the wrong mechanism. Clearing the cookie stops a replay only
if the client cooperates by discarding it. **New assumption:** a retained-cookie replay
is refused because the authorization code is single-use **at Keycloak** — the second
token exchange fails — with the `nonce` comparison as an independent second layer. The
AC's observable outcome is unchanged and needs no rewrite; what changes is that DELIVER
must not implement it as "cookie cleared, therefore replay impossible", and DISTILL's
scenario must replay a genuine code so it exercises the mechanism that actually holds.
Surfaced by the [WHY] trade-off analysis below.

### [REF] D3a addendum (2026-09-27)

Addendum, not a rewrite: DDD-1..DDD-12 stand. D3a (DISCUSS, 2026-09-27) supersedes
only the "provision nothing" half of D3 and OUT-3; this records how the design absorbs
it. Design note only — no code shape is prescribed beyond the seams named.

| ID | Decision | Verdict |
|---|---|---|
| DDD-13 | Migration `0016_nullable_password_hash.sql`: `ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL`. Additive, no backfill, no default; every existing row keeps its hash. `users.email_lower UNIQUE` is untouched and remains the one-account-per-address guarantee | LOCKED |
| DDD-14 | `foundry-oidc` extracts, never decides. `IdTokenClaims` gains `realm_access.roles` (Keycloak's realm-role mapper shape), `name`, `preferred_username`, all `serde(default)`, so a token without them validates exactly as today. `IdentityClaims` exposes them. `OidcConfig.provision_role` (blank = `None`) is carried through `OidcProvider` read-only. The crate still depends on neither `foundry-store` nor `foundry-auth` (C4 direction unchanged) | LOCKED |
| DDD-15 | "Find-or-provision" is owned by `foundry-app::oidc::callback`, the same place the link happens today. Order after the unchanged exchange + `email_verified` checks: (1) existing account by `email_lower` → link and sign in, role IGNORED (D3's linking half, unchanged); (2) no account and `provision_role` is `None` → refuse `no foundry account for this identity` (exactly D3); (3) role not in `realm_access.roles` (exact, case-sensitive match) → refuse `identity lacks provision role`; (4) no workspace exists → refuse `no workspace to provision into`; (5) provision, then `signin::establish_session` — the SAME DDD-6 seam, so the fail-closed branch and session shape are shared | LOCKED |
| DDD-16 | Provisioning is one store operation (user row with `password_hash = NULL` + one `member` membership) in a single transaction, `ON CONFLICT (email_lower) DO NOTHING` then re-read, so two concurrent first sign-ins produce one account and both sign in. It never grants `admin`, never touches an existing row | LOCKED |
| DDD-17 | Workspace: the instance's ORIGINAL workspace, `ORDER BY created_at, id LIMIT 1`. Not "the only" (0009 made multi-workspace legal) and not "newest" (the existing `ORDER BY id DESC` read). An unclaimed instance (no workspace) refuses, so provisioning can never pre-empt the bootstrap claim, whose "claimed" test is "a workspace exists" (D5 untouched) | LOCKED |
| DDD-18 | Display name: first candidate of `name` → `preferred_username` → email local-part that is non-blank (trimmed) and at most 64 characters (`users.display_name CHECK (length BETWEEN 1 AND 64)`). A longer candidate is SKIPPED, never truncated (OD-7, decided 2026-09-27 review). A local-part over 64 characters is unreachable in practice (RFC 5321 caps it at 64 octets) | LOCKED |
| DDD-19 | The password door treats a NULL hash exactly as an unknown email: `submit_signin` runs `verify_password` against `known_bad_hash()` and records the failed attempt, so status, body (D7/DDD-11) and wall-clock match the unknown-address arm. No early return on `None` — that would be a timing oracle for "this address has an SSO-only account" | LOCKED |
| DDD-20 | Logging rides the existing `refuse()` → `tracing::info!(reason = ..., "oidc sign-in refused")` line; D3a adds the reasons `identity lacks provision role` and `no workspace to provision into`. A successful provision logs once at `info` (`"oidc identity provisioned"`, user id + workspace id; no claims dumped). Nothing is added to the response | LOCKED |
| DDD-21 | Forgot-password / reset for a NULL-hash account (OD-8, decided: allowed). `submit_forgot` needs no logic change: it looks the user up by email, inserts a reset token and notifies, and ALWAYS renders the same `ForgotSentPage`, so the answer is identical for an account with a hash, without one, or none at all. `submit_reset` needs no logic change: `reset_password_and_consume` is an `UPDATE users SET password_hash = $2`, which fills a NULL as readily as it replaces a hash. The only required change is DDD-13's type change: while `UserRow.password_hash` is `String`, decoding a NULL row fails, `find_user_by_email` errs, and `submit_forgot`'s `if let Ok(Some(user))` silently sends nothing — non-enumerable, but the provisioned member never gets a link. Scenario 10 catches that | LOCKED |
| DDD-22 | Role withdrawal (OD-10 stays OPEN). DDD-15 step (1) links an existing account before the role is consulted, so an account provisioned earlier keeps signing in after the role is removed, and no second account is created. Scenario 11 PINS this as today's deliberate behaviour so a revocation design has to change a named scenario, not discover an unpinned one | PINNED. **SUPERSEDED 2026-10-04 by DDD-28** (D3b resolves OD-10: a provisioned account without the role is refused at step (1d); scenario 11 rewritten to AC-7.1/AC-7.2) |

**DDD-13 addendum (2026-09-27 review).** Migration `0016` is one-way once any
NULL-hash row exists: restoring `NOT NULL` fails while a provisioned account has no
password. Reverting D3a is therefore a FORWARD migration — first delete those
accounts or assign them hashes (e.g. force a reset) — never a down migration.

**Consequences.** `UserRow.password_hash` becomes `Option<String>` in `foundry-store`;
every reader (≈50 references across `signin`, `reset_password`, `invites_accept`,
`bootstrap`, `admin_cli`, `foundry-services`) must handle `None` explicitly — the
compiler enumerates them, which is the reason to model it as `Option` rather than a
sentinel string. Role revocation remains open (D3a): an existing account is linked
on every later sign-in regardless of the role, by DDD-15 step (1).

## Wave: DISTILL

Reconciliation gate: **passed — 0 contradictions.** DISCUSS D1–D7 and DESIGN
DDD-1–DDD-12 were checked pairwise; the two places DESIGN revised DISCUSS are
recorded in § Changed Assumptions (the OD-1 conclusion inverting, and AC-3.5's
mechanism), and both are revisions with an audit trail, not contradictions.
DEVOPS artifacts are absent → **WARN**, default environment matrix applied (per the
graceful-degradation matrix); nothing in this feature needs an environment axis
beyond what the shipped harness provides. Tier A only.

### [REF] Scenario list with tags

`.feature` SSOT: `crates/foundry-acceptance/tests/features/keycloak-sso.feature`
(23 scenarios, all `@pending`).

| # | Scenario | Tags | ACs |
|---|---|---|---|
| 1 | The operator signs in with their cluster identity and reaches their board | `@us-01 @walking_skeleton @driving_port @real-io` | 1.2, 1.3, 1.4 |
| 2 | The sign-in page offers the cluster identity when it is available | `@us-01 @driving_port @real-io` | 1.1 |
| 3 | Each sign-in attempt carries a fresh single-use challenge | `@us-01 @driving_port @real-io` | 1.2 |
| 4 | A cluster identity grants exactly what a password grants | `@us-01 @driving_port @real-io` | 1.5 |
| 5 | The challenge is discarded once the sign-in finishes | `@us-01 @real-io` | 1.6 |
| 6 | An identity with no foundry account is turned away | `@us-02 @error @security @driving_port @real-io` | 2.1, 2.2 |
| 7 | An unconfirmed address is turned away even when it matches an account | `@us-02 @error @security @driving_port @real-io` | 2.3 |
| 8 | A person who belongs to no workspace is turned away | `@us-02 @error @security @driving_port @real-io` | 2.4 |
| 9 | An arrival nobody started is refused | `@us-03 @error @security @driving_port @real-io` | 3.1 |
| 10 | An arrival that does not match the challenge it answers is refused | `@us-03 @error @security @driving_port @real-io` | 3.2 |
| 11 | An identity answering a stale challenge is refused | `@us-03 @error @security @driving_port @real-io` | 3.3 |
| 12 | An identity signed by an unknown key is refused | `@us-03 @error @security @driving_port @real-io` | 3.4 |
| 13 | An identity vouched for by a different provider is refused | `@us-03 @error @security @driving_port @real-io` | 3.4 |
| 14 | An identity that has already expired is refused | `@us-03 @error @security @driving_port @real-io` | 3.4 |
| 15 | Replaying a completed sign-in is refused | `@us-03 @error @security @driving_port @real-io` | 3.5 |
| 16 | An unreachable provider refuses the sign-in rather than breaking | `@us-03 @error @driving_port @real-io` | 3.6 |
| 17 | Every refusal looks identical, whoever is refused | `@us-02 @us-03 @security @driving_port @real-io` | 2.2, 2.5, 3.7 |
| 18 | The password door still opens while cluster identity is available | `@us-04 @driving_port @real-io` | 4.1 |
| 19 | A fresh instance can still be claimed with the provider unreachable | `@us-04 @driving_port @real-io` | 4.3 |
| 20 | Either door leads to the same person | `@us-04 @driving_port @real-io` | 4.4 |
| 21 | With no provider configured foundry serves as it always did | `@us-05 @driving_port @real-io` | 5.1, 5.2 |
| 22 | Asking for cluster identity when none is configured is refused | `@us-05 @error @driving_port @real-io` | 5.3 |
| 23 | A half-configured provider stops foundry from starting | `@us-05 @error` | 5.5 |

Error/edge ratio: 14 of 23 carry `@error` or `@security` = **61%** (target ≥ 40%).
Exactly one `@walking_skeleton` (scenario 1) closes the loop sign-in page → provider
→ arrival → board through the production composition root.

**Two ACs are suite-level properties, not scenarios, and are deliberately not in the
table above.** AC-4.2 (the shipped `us_06_signin` scenarios including the
timing-symmetry oracle stay green) is satisfied by the existing suite continuing to
pass, and asserting it as a new scenario would duplicate it. AC-5.4 (`cargo xtask ci`
green with no Keycloak reachable) is satisfied structurally: the issuer double is
in-process, so no lane in the suite can reach a real IdP. Both are named in the DoD.

### [REF] Walking Skeleton strategy

Per the Architecture of Reference, not a per-feature A/B/C/D negotiation: driving
ports are real (HTTP through the production `build_router` via
`foundry_app::test_support::spawn_app()`), driven-internal is real (shared
testcontainers Postgres 16, per-scenario `CREATE SCHEMA` rotation), and
driven-external / non-deterministic is faked (the identity provider, the clock).

Litmus test (Mandate 5 / Dim 5): a non-technical stakeholder reads scenario 1 as "the
operator signs in with the login they already use and lands on their board." Yes.

### [REF] Driving-adapter coverage

| Driving adapter (DESIGN entry point) | Protocol | Scenario(s) |
|---|---|---|
| `GET /sign-in` (control rendered / absent) | HTTP GET | 1 (WS), 2, 21 |
| `GET /auth/oidc/start` (NEW) | HTTP GET → 302 | 1 (WS), 3, 16, 22 |
| `GET /auth/oidc/callback` (NEW) | HTTP GET → 303 / 401 | 1 (WS), 5–17, 22 |
| `POST /sign-in` (shipped, regression) | HTTP POST | 17, 18, 20 |
| `POST /sign-out` (shipped, unchanged) | HTTP POST | 20 |
| `GET|POST /bootstrap` (shipped, regression) | HTTP | 19 |
| `GET /healthz`, `GET /readyz` (shipped) | HTTP GET | 21 |
| Binary startup (composition root) | process launch | 23 |

Zero uncovered entry points. Every one is invoked via its real protocol against
`build_router` — or, for scenario 23, by launching the real binary — never by calling
a service function directly (RCA-fix P1).

### [REF] Adapter (driven) coverage

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Identity provider discovery + JWKS | YES (against the double) | 1, 12, 13 — the double serves real discovery and JWKS documents over a real socket; the production `reqwest` client fetches them for real |
| Identity provider token exchange | YES (against the double) | 1, 15, 16 — a genuine POST over the loopback socket; 16 binds and drops it to produce a real connection failure |
| `Store` user lookup by `email_lower` | YES | Real testcontainers Postgres, per-scenario schema (all scenarios); read directly at the store boundary in the refusal Thens |
| `tower-sessions` store | YES | Real, same schema — scenarios 1, 4, 15, 20 read the session row |
| Clock (`exp`, challenge TTL) | Faked (shipped `MockClock`) | 14 mints an already-lapsed identity by advancing the shared clock seam |

No driven adapter is left without a real-I/O scenario. The identity provider is faked
per the Architecture of Reference's driven-external row — but the fake is a real HTTP
listener, so the production adapter's wire behaviour (headers, body, TLS-off loopback,
timeouts) is genuinely exercised, exactly as `webhook_receiver.rs` does for the
webhook channel.

### [REF] Scaffolds (RED-ready, Mandate 7)

Per THIS project's convention, **no new production panic-stub is committed.** The
shipped `build_router`, `signin.rs`, and templates are left untouched, and the new
endpoints are referenced ONLY as HTTP path string literals. The step module therefore
COMPILES against current production (no ImportError-class BROKEN); an unskipped
scenario fails at an assertion — 404 where a 302 was expected, absent control, absent
session — which is RED. DELIVER mounts the routes and turns each GREEN, Outside-In.

| Artifact | Kind | Status |
|---|---|---|
| `crates/foundry-acceptance/tests/features/keycloak-sso.feature` | Tier-A Gherkin, 23 scenarios, all `@pending` | **created** |
| `crates/foundry-acceptance/src/support/oidc_issuer.rs` | In-process axum identity-provider double, fixed RSA test keypair, 6 mint variants | **created** |
| `crates/foundry-acceptance/src/steps/feature_keycloak_sso.rs` | Step defs, 43 step phrases | **created** |
| `crates/foundry-acceptance/src/support/mod.rs` (`pub mod oidc_issuer;`) | Module registration | **edited** |
| `crates/foundry-acceptance/src/lib.rs` (`pub mod feature_keycloak_sso;`) | Module registration | **edited** |
| `crates/foundry-acceptance/tests/acceptance.rs` (force-link `use`) | Link registration | **edited** |
| `crates/foundry-acceptance/src/world.rs` (16 `kc_*` fields) | Per-scenario state | **edited** |

Verified, not assumed: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
clean (the gate `cargo xtask smoke` runs), and two unit tests in `oidc_issuer.rs` pass —
they parse and sign with both fixture keypairs and prove the published JWKS verifies what
the double signs, so a fixture typo fails at unit-test speed rather than mid-scenario. A
script check confirms none of the 43 step phrases collides with another step module
(cucumber-rs matches step text globally and panics at runtime on a duplicate — a class the
compiler cannot catch).

**Known RED gap, deliberate.** `InProcHarness` has no `spawn_with_oidc` constructor,
because `AppState` has no `oidc` field yet. The "foundry is connected to the cluster
identity provider" Given therefore starts the double and records its issuer URL, but
cannot yet point foundry at it. Every OIDC scenario consequently fails on the 404 from
the unmounted route — RED for MISSING_FUNCTIONALITY, which is the right failure for the
right reason. Four Thens that need a working federated session to observe anything
(`the issue is recorded as authored by them`, `their original session is untouched`,
`a wrong password is answered identically too`, `it refuses to start and names the
missing credential`) `panic!` with a message naming what DELIVER must wire. This is
documented here rather than left for a crafter to discover.

DELIVER production targets (not scaffolded here): the P0 and P1 precursor commits,
then `oidc::start` + `oidc::callback`, the `AppState.oidc` field, the composition-root
wiring, the sign-in control, and the two `build_router` mounts.

### [REF] Test placement

`crates/foundry-acceptance/tests/features/<name>.feature` +
`crates/foundry-acceptance/src/steps/feature_<name>.rs`, registered in
`src/lib.rs` and force-linked in `tests/acceptance.rs` — the established Rust/cucumber
precedent in this repo (mirrors `notification-preferences-ui` and
`workspace-member-invites`). The polyglot matrix's Rust row
(`<feature>_scenarios.rs` + `<feature>_specifications.rs`) does NOT apply: this host
uses the cucumber-rs `.feature` + step-module idiom, which is the stronger local
precedent.

The identity-provider double lands in `src/support/`, beside `webhook_receiver.rs` and
`round_robin_proxy.rs`, and must carry the repo's Extension Justification header block
(`WHY-NEW-FILE` / `CLOSEST-EXISTING` / `EXTENSION-COST` / `PARALLEL-RATIONALE`).

**OQ-2 resolved: a hand-rolled axum double, not `wiremock`.** Two reasons, the second
decisive. First, `wiremock` is a new dev-dependency that must clear the
`cargo deny check` gate, where the repo already has two in-process axum doubles doing
this exact job. Second, `wiremock` is built for *canned* responses, and this double
must serve *computed* ones — a correctly RS256-signed identity minted per scenario,
plus deliberate variants (unpublished key, foreign issuer, lapsed validity, stale
challenge). That is a signing fixture with an HTTP surface, not a stub server.

### [REF] Layered-test discipline notes

- **Tier A only (Mandate 10).** Tier B (state-machine PBT over an in-memory
  composition) is skipped: the journey is a 3-step chain, but the input space is not
  domain-rich — identities are structured tokens with a fixed claim set, not free
  text, dates, or IDs drawn from a large space. Modelling it as a state machine would
  restate the example scenarios with more machinery.
- **Mandate 9.** Every scenario runs at layer 3+ (real axum through the production
  composition root, real Postgres), so all are example-based; no `@property` tags, no
  Hypothesis-equivalent generation.
- **Mandate 11.** The 14 sad paths are named example-based scenarios, never
  PBT-generated.
- **Mandate 8.** Universe-bound `assert_state_delta` is the Python pilot contract;
  this Rust host asserts the equivalent observable universe directly — rendered page
  content, HTTP status, `Set-Cookie` presence, and store reads through the read-only
  `db_introspect.rs` helper — never internal struct fields.
- Scenario 17 is the one that would most tempt a property test ("all refusals are
  identical"). It stays an enumerated example because the enumeration IS the
  specification: a new refusal branch added later must be added to that list
  deliberately, which a generator would silently absorb.

### [REF] Pre-requisites

- **DESIGN driving ports**: `/auth/oidc/start` and `/auth/oidc/callback` mount under
  the shipped `csrf_middleware` + `session_layer` in `build_router`.
- **Precursor commits** P0 (`signin::establish_session` extraction) and P1
  (`crates/foundry-oidc` + `check_oidc_alg_pin` + the `deny.toml` ban) land before
  slice 01, so the step module has stable production names to reference.
- **DEVOPS environment**: the shipped in-process axum harness plus a testcontainers
  Postgres 16, per the Project Infrastructure Policy. **No new environment matrix and
  no new `@docker-compose` lane** — the identity provider is in-process.
- **Fixture**: a fixed RSA test keypair (PEM for the double's signer, matching JWK for
  its published key set), committed as test fixture material exactly as the
  machine-token test keypair is.

### [REF] Outcomes to register

`nwave-ai outcomes register` is **broken in this install** — it resolves
`docs/product/outcomes/schema.json` relative to its own site-packages
(`~/.local/share/uv/tools/nwave-ai/lib/python3.12/site-packages/docs/...`) rather than
the project, and every invocation dies with `FileNotFoundError`. It did create
`docs/product/outcomes/registry.yaml` with `schema_version: "0.1"` and an empty
`outcomes: []`. The rows below are NOT hand-written into that file: the schema it
would be validated against is the missing file, so hand-authored rows could be
malformed in a way nothing here can detect. They are recorded instead, ready to
register once the CLI works.

| ID | Kind | Input shape | Output shape | Keywords |
|---|---|---|---|---|
| OUT-1 | operation | Signed-out `GET /auth/oidc/start` | 302 to the provider's authorization endpoint plus a single-use signed challenge cookie | oidc, sso, start, authorization, pkce |
| OUT-2 | operation | `GET /auth/oidc/callback` with `code` and `state` | 303 to `/` with an established session, or a 401 generic refusal | oidc, sso, callback, session, signin |
| OUT-3 | invariant | Any validated federated identity | No `users` row is ever created by the federated path | oidc, provisioning, invite, users |
| OUT-4 | invariant | Any failed sign-in, federated or password | Byte-identical status and body across every refusal branch | enumeration, refusal, signin, uniform |
| OUT-5 | specification | An ID token presented at the callback | Accepted only when RS256-signed by a published provider key with matching `iss`, `aud`, `exp`, `nonce` | jwt, rs256, algorithm, pin, validation |

`nwave-ai outcomes check-delta` exits 0 (nothing to collide with yet).

### Open decisions for DELIVER

- **OD-5** Where the RSA test fixture lives — inline `const` in `oidc_issuer.rs` versus
  a file under `tests/fixtures/`. The machine-token precedent uses a `test_keys` module
  gated behind the `test-support` feature; following it is the obvious default.
- **OD-6** Whether scenario 23 launches the real binary via `assert_cmd` (the
  `admin_cli` precedent) or asserts the composition-root refusal in-process. The
  scenario is written to the observable outcome — "refuses to start and names the
  missing credential" — so either satisfies it, but `assert_cmd` exercises the actual
  operator-visible failure.

### [REF] D3a increment (2026-09-27)

Rigor profile `adr-025-scaffolded-red`. Increment on top of the delivered base
feature (c755003); `keycloak-sso.feature` is not rewritten. Revised the same day
after review (see § Review 2026-09-27).

**Reconciliation: passed — 0 contradictions.** D3a supersedes the "provision
nothing" half of D3 with an audit trail (DISCUSS D3 row marked SUPERSEDED; D3
alternatives table amended; US-06 added as an addendum), so it is a revision, not a
contradiction. D3's linking half, D5 (bootstrap/invite untouched — DDD-17 refuses on
an unclaimed instance) and D7 (non-enumerable refusals — DDD-19/DDD-20/DDD-21 route
every new refusal and the reset answer through the shipped uniform paths) all hold.
DDD-6 (`establish_session`) and DDD-11 (one refusal function) are reused verbatim.
DDD-12 (fake issuer) extended with realm roles and profile claims. DEVOPS still
absent → WARN, default matrix. Tier A only; all scenarios layer 3+ (example-based,
Mandate 9/11).

`.feature` SSOT: `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature`
(11 scenarios / 15 examples, all `@pending`, all `@keycloak-sso @keycloak-sso-provisioning @us-06`).

| # | Scenario | Tags | ACs | What it proves |
|---|---|---|---|---|
| 1 | A newcomer holding the provision role is given an account and signed in | `@us-06 @driving_port @real-io` | 6.2 | Provision + sign-in through `establish_session`; greeted by `name`; exactly one `member` membership, in the ORIGINAL workspace although a newer one exists (DDD-16/17) |
| 2 | A newcomer the provider gives no usable full name is still greeted by a name (3 examples) | `@us-06 @driving_port @real-io` | 6.3 | DDD-18: no `name` → `preferred_username`; neither → local-part; a 68-character `name` falls through to `preferred_username`, not truncated (OD-7) |
| 3 | A newcomer without the provision role is turned away (2 examples: other role / no roles) | `@us-06 @error @security @driving_port @real-io` | 6.4 | DDD-15 step 3; refusal byte-identical to a real wrong-password answer (D7); no account |
| 4 | With provisioning switched off a newcomer is turned away even holding the role | `@us-06 @error @security @driving_port @real-io` | 6.1 | Role unset = exactly D3 |
| 5 | A newcomer whose address the provider has not confirmed is turned away | `@us-06 @error @security @driving_port @real-io` | 6.4 | `email_verified` precedes provisioning |
| 6 | Before anyone has claimed the instance a newcomer holding the role is turned away | `@us-06 @error @security @driving_port @real-io` | 6.4 | DDD-17: no workspace → refuse; the bootstrap claim cannot be pre-empted |
| 7 | A member who already has an account keeps it when they hold the provision role | `@us-06 @driving_port @real-io` | 6.5 | DDD-15 step 1: linked across address case, not duplicated; display name and password untouched |
| 8 | A provisioned account has no password to sign in with (2 examples: empty / a guess) | `@us-06 @error @security @driving_port @real-io` | 6.6 | DDD-13/19: NULL hash refused byte-identically to an unknown address |
| 9 | The password form takes as long to refuse a provisioned account as an unknown address | `@us-06 @error @security @driving_port @real-io` | 6.6 | DDD-19 timing: median within 150ms over 7 interleaved pairs (the `us-06-signin` oracle and budget) |
| 10 | A provisioned member can choose a password of their own through a reset | `@us-06 @driving_port @real-io` | 6.7 | DDD-21: forgot-password → emailed link → new password → password door → board. Reuses the shipped `a visitor submits the forgot-password form with email "…"` step (`us_06_signin.rs`) |
| 11 | A member provisioned earlier still signs in after the provision role is withdrawn | `@us-06 @driving_port @real-io` | 6.8 | DDD-22: PINS today's behaviour while OD-10 stays open; no second account |

Error/edge ratio: 6 of 11 scenarios (8 of 15 examples) = **55%** (target ≥ 40%).

AC-6.7's "the forgot-password answer is identical whatever the account state" is not a
separate scenario: `submit_forgot` renders one static `ForgotSentPage` unconditionally
(DDD-21), so the property is structural, and the shipped `us-06-signin` unknown-email
scenario already exercises the no-account arm. AC-6.4's log line has no acceptance
observable (the harness captures no tracing output); DELIVER covers it at unit level.
Blank `FOUNDRY_OIDC_PROVISION_ROLE` = off (AC-6.1) is a config-parsing rule
(`OidcConfig::with_provision_role`), specified at layer 1 in `foundry-oidc`.

"Turned away exactly as a wrong password is" is asserted byte-for-byte against a live
`POST /sign-in` for an unknown address, CSRF token masked — not by searching for the
refusal copy.

**Scaffolds.** No production stub (repo convention, as for the base feature): the
step module compiles against current production and fails at assertions.

| Artifact | Status |
|---|---|
| `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature` | created |
| `crates/foundry-acceptance/src/steps/feature_keycloak_sso_provisioning.rs` | created (25 step phrases; no collision with any other module; one shipped phrase reused) |
| `crates/foundry-acceptance/src/support/harness.rs` — `InProcHarness::spawn_with_oidc` | edited (closes the base DISTILL's "known RED gap") |
| `crates/foundry-acceptance/src/support/oidc_issuer.rs` — `will_grant_realm_roles`, `will_name`, `realm_access`/`name`/`preferred_username` claims | edited |
| `crates/foundry-acceptance/src/world.rs` (5 `kc_*` fields), `src/lib.rs`, `tests/acceptance.rs`, `Cargo.toml` (`foundry-oidc` path dep) | edited |
| `crates/foundry-oidc/src/lib.rs` — `OidcConfig.provision_role` + `with_provision_role` | edited (config parsing only) |
| `docs/architecture/atdd-infrastructure-policy.md` — identity-provider fake row | appended (missing since the base feature) |

`cargo check -p foundry-acceptance --tests` and
`cargo clippy -p foundry-acceptance --tests -- -D warnings` clean.

**Pre-requisites.** Migration `0016` (DDD-13, one-way — see addendum);
`UserRow.password_hash: Option<String>` with the NULL-hash refusal on the
known-bad-hash path (DDD-19); claim extraction in `foundry-oidc` (DDD-14);
find-or-provision in `oidc::callback` (DDD-15..18).

**RED classification.** Command (after `cargo test -p foundry-acceptance --test
acceptance --no-run` to warm the binary; `@pending` stripped from this one file with
`sed -i '' 's/ @pending$//'`, file restored from a copy afterwards and the 11 tags
re-counted):
`FOUNDRY_ACCEPTANCE_TAGS=keycloak-sso-provisioning cargo test -p foundry-acceptance --test acceptance`
→ 15 examples, 9 failed, 6 passed, 0 parse errors, 0 undefined steps. All 9 failures
carry the identical message.

| # | Result | Class | Failing step and message |
|---|---|---|---|
| 1 | FAIL | MISSING_FUNCTIONALITY | `Then the newcomer arrives signed in to the board` — `expected a redirect onto the board; got 401 Unauthorized` (body: the generic "Invalid email or password" page; left 401, right 303) |
| 2a, 2b, 2c | FAIL | MISSING_FUNCTIONALITY | same step, same message |
| 3a, 3b, 4, 5, 6, 7 | PASS | GUARD (green by inheritance) | — |
| 8a, 8b, 9, 10, 11 | FAIL | MISSING_FUNCTIONALITY | `And the newcomer has been given an account through the identity provider` — same message |

BROKEN: 0. False-GREEN: 0. The six GUARDs pin behaviour D3a must PRESERVE, which the
link-only callback already exhibits; no fixture does the feature's work. They are not
vacuous: scenario 7 passes the full start → authorize → exchange → session → board
round-trip against the same double, so the 401s above are genuine "no foundry account"
refusals, and 3–6 pass the byte-identical comparison, proving the D7 check works.
Un-pend them together with scenario 1 (repo precedent: "green-by-inheritance" in
`feature_mwt_slice_06_provision_and_prove.rs`).

Scenarios 10 and 11 fail at the shared provisioning Given, so their later steps never
ran. The reset steps were therefore proven separately with a throwaway scenario
(deleted afterwards): an existing member ran the reused forgot-password step → the
reset-link step → the password door → board greeting, all green. Scenario 10's steps
are therefore not hiding a BROKEN behind the Given.

**Outcomes** (recorded only; the `nwave-ai outcomes` CLI is still broken per the note
above).

| ID | Kind | Change |
|---|---|---|
| OUT-3 | invariant | Becomes CONDITIONAL: "no `users` row is created by the federated path **unless `FOUNDRY_OIDC_PROVISION_ROLE` is set and the verified identity carries that realm role**" |
| OUT-6 (proposed) | operation | Input: validated federated identity, no account, provision role held, a workspace exists. Output: one `users` row with NULL `password_hash` + one `member` membership in the oldest workspace, then an established session. Keywords: oidc, provisioning, realm-role, member, keycloak |

### Review 2026-09-27

| Reviewer | Verdict |
|---|---|
| Acceptance designer reviewer | NEEDS_REVISION |
| Solution architect reviewer | NEEDS_REVISION |

| Finding | Disposition | Reason / where |
|---|---|---|
| No story or ACs to trace D3a scenarios to | ACCEPTED | US-06 (AC-6.1..6.8) added to DISCUSS as a dated addendum; every scenario tagged `@us-06` and mapped above |
| Reset path for a NULL-hash account undesigned (OD-8) | ACCEPTED | DDD-21; scenario 10 |
| Over-64-character display name undecided (OD-7) | ACCEPTED | DDD-18 (skip, never truncate); scenario 2 third example |
| Role withdrawal behaviour unpinned (OD-10) | ACCEPTED | DDD-22; scenario 11 pins today's behaviour, OD-10 stays open |
| Migration 0016 rollback unstated | ACCEPTED | DDD-13 addendum: one-way; revert is a forward migration |
| Seed scenario 8's provisioned account by SQL | REJECTED | State is built through the real sign-in, per precedent; a SQL fixture would do the feature's work (fixture theater) and could pass without provisioning |
| Replace DB-read Thens with HTTP observations | REJECTED | Account and membership existence has no HTTP observable that does not itself need the feature; DB reads in Thens only follow the base feature and `us-06-signin` |
| Tag the timing scenario `@flaky` | REJECTED | Same interleaved-median oracle and budget as the shipped `us-06-signin` timing scenario, which is not tagged |
| Add a concurrent-first-sign-in scenario | REJECTED | DDD-16's `ON CONFLICT` race is a store-level property; a parallel HTTP race in the acceptance lane would be nondeterministic. Unit/integration test in DELIVER |

### Open decisions for DELIVER (D3a)

- **OD-7** — RESOLVED 2026-09-27: skip, never truncate (DDD-18, AC-6.3).
- **OD-8** — RESOLVED 2026-09-27: reset allowed (DDD-21, AC-6.7).
- **OD-9** — RESOLVED 2026-09-27 (user confirmed): exact, case-sensitive match against
  `realm_access.roles` only; `resource_access` client roles never count.
- **OD-10** Role revocation for existing accounts — still OPEN; today's behaviour is
  pinned by scenario 11 (DDD-22, AC-6.8).
- **OD-11** The base `keycloak-sso.feature` is still wholly `@pending` although
  delivered in c755003; `spawn_with_oidc` now exists, so its "known RED gap" is closed
  and those scenarios can be un-pended.

## Wave: DELIVER

> **D3a increment, 2026-09-27.** Apex (@nw-platform-architect), DELIVER finalize. The
> base feature (`c755003`, v0.4.0) has no DELIVER section. This section covers the D3a
> increment only and supersedes nothing above; the DISTILL open decisions it resolves
> are marked here, not edited in place. **Sources:** `deliver/roadmap.json`,
> `deliver/execution-log.json` and `deliver/mutation/mutation-report.md`.
> Evolution archive: `docs/evolution/2026-09-27-keycloak-sso.md`.

### [REF] Implementation Summary

There are 4 roadmap steps in 2 phases, and all are GREEN:
- **Phase 01, foundations:** 01-01 and 01-02, independent of each other, with no scenario
  un-pended.
- **Phase 02, US-06:** 02-01, then 02-02.

The increment ran in no-commit mode, so every COMMIT phase is `APPROVED_SKIP`.

- **Migration `0016`.** `password_hash` becomes nullable: additive, no backfill,
  one-way (DDD-13 and its addendum).
- **The store.** `UserRow.password_hash: Option<String>`, and
  `Store::provision_federated_member`: one transaction, the ORIGINAL workspace, `ON
  CONFLICT (email_lower) DO NOTHING` then re-read, `member` only. It returns
  `Created`, `Existing` or `NoWorkspace` (DDD-16/17).
- **`signin::verify_against`.** One verifier serves the password door and
  change-password reauth. A missing hash verifies against `known_bad_hash()` and
  answers `false` (DDD-19).
- **`foundry-oidc`.** It now reads `realm_access.roles`, `name` and
  `preferred_username`, all `serde(default)`. `has_realm_role` is exact and
  realm-only (OD-9). `with_provision_role` treats blank as `None`, and
  `OidcProvider::provision_role()` exposes it (DDD-14).
- **`oidc::callback`.** Find-or-provision in DDD-15 order, through the helpers
  `provision`, `judge_newcomer`, `greeting_name` and `log_provisioned`. It adds the
  refusal reasons `identity lacks provision role` and `no workspace to provision into`
  (DDD-20).
- **Opt-in.** `FOUNDRY_OIDC_PROVISION_ROLE`: unset or blank = off = exactly D3.

**Open decisions at close:**
- **OD-7**, **OD-8** and **OD-9**: resolved, as recorded in DISTILL.
- **OD-10**: **OPEN**. Scenario 11 pins today's behaviour.
- **OD-11**: **OPEN**. The 23 base scenarios are still `@pending`.

### [REF] Per-step outcome

| Step | RED | GREEN |
|---|---|---|
| 01-01 Accounts may exist without a password | Integration RED (RED_UNIT `NOT_APPLICABLE`). The store answered 500 on a NULL-hash decode error, and forgot-password silently sent nothing. The timing assertion was mutation-checked | Migration 0016, `Option` hash end to end, `verify_against` |
| 01-02 foundry-oidc exposes realm roles and profile names | RED_UNIT: 3 failing on assertion, 8 passing. RED_ACCEPTANCE `NOT_APPLICABLE` | foundry-oidc 11/11; workspace excluding acceptance 297 passed; us-06 30/30 |
| 02-01 OIDC callback provisions a member holding the provision role | RED_ACCEPTANCE 6/10: scenario 1 and three scenario-2 examples failed with 401, expected 303. RED_UNIT: 3 `oidc` unit tests and 3 of 4 store tests failed on assertion | Scenarios 1-7 (10 examples) green; us-06 40/40; default lane 645/645 (a us-09 sqlx `'\0'` flake, green on rerun); workspace 304 |
| 02-02 Provisioned accounts at the password door, reset, and role withdrawal | Scenarios 8-11 (5 examples) green on un-pend; their RED was observed in 01-01's integration tests. RED_UNIT `NOT_APPLICABLE` | All 11 scenarios / 15 examples green |

**A DISTILL defect was fixed in 02-01.** The outline placeholder `<full name>` was never
substituted, because cucumber-rs forbids spaces in outline headers. The header is now
`full_name`. All three examples had failed upstream at the 401, which masked it.

**Final counts:**
- provisioning: 15/15 examples;
- `us-06`: 45/45;
- default lane: 650/650;
- workspace: 304 passed, 0 failed.

The feature file has no `@pending` tag. Its one `@pending` string is the header comment
at line 19.

### [REF] Refactor

Phase 3 was a separate L1–L4 pass in DES orchestrator mode, with no step entry in the log.
Behaviour was unchanged. Afterwards: 304 workspace tests, provisioning 15/15 and `us-06`
45/45 passed, and clippy, fmt and check-arch were clean.
- **L2:** `verify_against` replaces the duplicated stored-hash / known-bad-hash block in
  `submit_signin` and `submit_change_password`.
- **L1:**
  - `Refusal` → `ProvisionFailure` in `oidc.rs`.
  - Step-module cookie constants.
  - The `cookie_value`, `insert_workspace` and `describe_newcomer` helpers.
  - The store-test helpers `migrated_store` and `account_of`.
  - `assert_eq!` in place of `assert!(matches!)`.
  - The stale feature-file header was fixed.
- **Kept on purpose:** `provision()` keeps 4 parameters, and `spawn_inner` keeps 5 (that
  long parameter list predates D3a).
- **Phase 5 test code:** `post_form_as` and `password_hash_of` in
  `password_less_account_doors.rs`.

### [REF] Review

| Review | Verdict |
|---|---|
| DISTILL (acceptance designer + solution architect) | NEEDS_REVISION ×2. 5 findings accepted and 4 rejected; see § Review 2026-09-27 |
| Roadmap | APPROVED |
| Phase 4 adversarial review | APPROVED, no defects |

### [REF] Mutation

The strategy is per-feature, gate ≥80%, run with cargo-mutants 25.3.1 `--in-diff` over
the 4 production files. Full detail is in `deliver/mutation/mutation-report.md`.

| | |
|---|---|
| Viable | 25: 33 generated, 8 unviable |
| Kill rate | **96.0%** (24/25). **PASS**. Package tests alone: 84.0% (21/25). 3 kills came from the acceptance lane, applied by hand in an `rsync` copy |
| Survivor | `OidcConfig::from_env → Ok(None)`, the untested humble env adapter. No test pins the name `FOUNDRY_OIDC_PROVISION_ROLE`. A typo fails safe: link-only |
| H1: password-less account passes change-password reauth | Survived, then **closed** by the new test `a_signed_in_password_less_account_cannot_set_a_password_by_changing_it` |
| H2: race loser (`Existing`) refused | Survived; **open**. The store half is covered; the app half needs a deterministic interleaving |
| H4: provision into the newest workspace | Killed |

### [REF] Integrity

`des-verify-integrity`: "All 4 steps have complete DES traces" (26 events).

### [REF] Quality gate

`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: **RED, 854/855 scenarios (5971/5972 steps), 2026-09-27; the one failure is not D3a.** `us-03-backup-restore` "`foundry doctor backup-verify` ... reports row counts" printed an empty `row-counts:` block. `count_rows` (`admin_cli.rs:1634`) shells out to a bare `psql`, but this host has no Postgres client. `439ee6f` (2026-09-04) routed `pg_restore` through a container shim (`FOUNDRY_PG_RESTORE`) but not `psql`, and `count_rows` swallows every error as "table not present". It failed 3/3 in isolation with `FOUNDRY_ACCEPTANCE_TAGS=us-03-cli`. D3a touches neither `admin_cli.rs` nor the backup steps. The user chose to commit D3a and fix the `psql` seam as a separate bugfix.

Known infrastructure flakes were seen during the wave: the sqlx `Protocol('\0')` flake,
a testcontainers port-fetch failure, and `Connection reset by peer`. None was counted as
a result, and no mutation kill relied on one.

### [REF] Closing notes

- **Outcomes.** OUT-3 (now conditional) and the proposed OUT-6 stay recorded in DISTILL
  only, and `docs/product/outcomes/registry.yaml` is not edited.
  - This feature's DISTILL declined to hand-write rows while the `nwave-ai outcomes`
    CLI was broken.
  - The registry has since used OUT-1..OUT-16 for other features, so this feature's
    OUT-1..6 **collide by ID**. They need new IDs when they are registered.
- **Architecture SSOT.** `docs/product/architecture/brief.md` § "Federated identity is
  additive, never a migration" gains a dated D3a addendum. Its body is left as written.
- **Rollback.** Migration 0016 is one-way once a NULL-hash row exists. Reverting the
  code is safe with the role unset, which is link-only, exactly D3. Reverting the
  schema is a forward migration (DDD-13 addendum).
- **Workspace kept.** `docs/feature/keycloak-sso/`, including `deliver/`, is kept, as
  earlier features did.
- **Follow-ups:**
  - OD-10: role revocation.
  - OD-11: un-pend the 23 base `keycloak-sso.feature` scenarios. The shipped flow has no
    acceptance coverage.
  - A test for the `FOUNDRY_OIDC_PROVISION_ROLE` env name, through a pure lookup seam
    in `from_env`.
  - The concurrent-loser (`Existing`) path, end to end (H2).
  - Change-password copy for password-less accounts.
  - The empty-greeting-name refusal, which DDD-18 calls "unreachable" and nothing
    exercises.
  - Whether to keep the identity-provider row that the D3a DISTILL added to
    `docs/architecture/atdd-infrastructure-policy.md`.

## Wave: DELIVER

> **Phase 03 increment (OD-11), 2026-10-03.** Apex (@nw-platform-architect), DELIVER
> finalize. This section covers roadmap phase 03 only (steps 03-01..03-06) and
> supersedes nothing above; the D3a DELIVER section stays as written, including its
> "OD-11: OPEN" line. **Sources:** `deliver/roadmap.json` (phase 03),
> `deliver/execution-log.json` and the step commits. Evolution archive:
> `docs/evolution/2026-09-27-keycloak-sso.md` § "2026-10-03 increment: OD-11".

### [REF] Implementation Summary

Phase 03 un-pends the 23 base `keycloak-sso.feature` scenarios against the flow
shipped in `c755003` (v0.4.0). There are 6 roadmap steps, run in order, all GREEN.
The roadmap phase was approved 2026-10-03 by nw-acceptance-designer-reviewer
(`e13940a`).

- **No production code changed.** Every base scenario passed against the shipped
  flow once the harness drove it for real. The diff `e13940a..4e6a1f2` touches only
  `crates/foundry-acceptance/`.
- **The KNOWN RED GAP harness is gone.** "foundry is connected to the cluster
  identity provider" starts the issuer double and spawns foundry with
  `InProcHarness::spawn_with_oidc`, provision role unset (link-only, D3 exactly).
- **The round-trip is real:** start redirect, the double's `/authorize` (records the
  nonce), callback with the redirect's state and the sealed challenge cookie. Cookies
  are carried by hand, because foundry's cookies are `Secure` and the lane speaks
  plain HTTP over loopback.
- **Flag-only Whens are real HTTP:** the password door (`GET`/`POST /sign-in`), the
  bootstrap claim (`POST /bootstrap` with a minted token, provider shut down),
  `POST /sign-out`, and for scenario 23 the real `foundry` binary launched
  half-configured.
- **Placeholder `panic!` Thens are gone.** Several were strengthened past what the
  first draft checked:
  - 03-01: "different challenge" compares state and nonce, not the whole redirect
    (PKCE kept the redirect differing under F2).
  - 03-02: the refusal Then asserts no session cookie and compares status 401 and
    the CSRF-masked body with a real wrong-password answer (it used to check two
    forbidden phrases, which let F8 through).
  - 03-04: the refusal sweep also collects no-account, no-workspace, never-begun and
    wrong-state arrivals (the no-workspace branch renders outside `refuse()`).
  - 03-05: "same person" compares user ids, read from the session row before
    sign-out (a greeting check alone passed under F21).
- **`0c2553d` (F10).** "An arrival nobody started is refused" now arrives as a
  forgery that would otherwise succeed: a confirmed account with a workspace, a
  provider that vouches for it, and a code from an authorization request begun at
  the provider directly. Only the missing challenge refuses it.
- **`4e6a1f2` refactor (test code only, oracles unchanged).** Shared round-trip,
  password door and protocol constants moved into `pub(crate)` items of
  `feature_keycloak_sso_provisioning`; `feature_keycloak_sso.rs` 1212 → 1088 lines;
  World `kc_*` fields 23 → 18; truthful step-module header.

**Scenario 19 landing (user decision 2026-10-03, recorded in 03-05's notes).** The
shared Then derives the door from world state and requires a 303 to that door's
documented landing exactly: `/` for the password and cluster-identity doors,
`/dashboard` for the bootstrap claim. For the claim it follows `/dashboard` with the
session cookie and then checks that `/` greets the claimant. `/dashboard` is never
accepted for the password or SSO doors (fault F-landing). The reviewer confirmed the
decision does not weaken scenarios 1, 18 or 20.

**Open decisions at close:**
- **OD-11** — RESOLVED 2026-10-03: all 23 base scenarios run against the shipped
  flow; `keycloak-sso.feature` has no `@pending` tag (03-06, `5c8491b`).
- **OD-10** — still OPEN; provisioning scenario 11 pins today's behaviour.

### [REF] Files modified

| File | Change |
|---|---|
| `crates/foundry-acceptance/src/steps/feature_keycloak_sso.rs` | Real harness, round-trip, Whens and Thens; refactored in `4e6a1f2` |
| `crates/foundry-acceptance/src/steps/feature_keycloak_sso_provisioning.rs` | Round-trip split into begin/finish helpers; shared `pub(crate)` helpers and constants (behaviour unchanged) |
| `crates/foundry-acceptance/src/world.rs` | `kc_federated_user_id`, `kc_startup`; five write-only `kc_*` fields dropped |
| `crates/foundry-acceptance/tests/features/keycloak-sso.feature` | `@pending` removed from 23 tag lines; Gherkin otherwise byte-identical to DISTILL's |

### [REF] Scenarios green

| Step | Commit | Scenarios (un-pended) | First run |
|---|---|---|---|
| 03-01 | `29a7326` | Walking skeleton; offer when available; fresh single-use challenge; grants exactly what a password grants; challenge discarded (5) | All green |
| 03-02 | `72a76f3` | No foundry account; unconfirmed address; no workspace (3) | No-account RED on the harness ("they authenticate" assumed a begun sign-in), not the product; the others green |
| 03-03 | `26ecdd9` | Nobody started; mismatched challenge; stale challenge; unknown key; different provider; expired (6) | All green (the mismatched-challenge step was reworked before its first run to carry the genuine cookie) |
| 03-04 | `b6689e9` | Replay; unreachable provider; every refusal identical (3) | All green |
| 03-05 | `39863a6` | Password door open; claim with provider unreachable; either door same person (3) | All green |
| 03-06 | `5c8491b` | No provider serves as before; asking when none configured; half-configured stops startup (3) | Half-configured RED on the placeholder `panic!` (harness), then green against the real binary |

### [REF] Per-step outcome

| Step | Lanes at the step |
|---|---|
| 03-01 | keycloak-sso 20/20 (5 base + 15 provisioning); provisioning 15/15; check-arch, smoke, clippy green |
| 03-02 | keycloak-sso tag 23/23; provisioning 15/15; check-arch, smoke green |
| 03-03 | keycloak-sso 29/29; provisioning 15/15; check-arch, smoke green |
| 03-04 | keycloak-sso 32/32; provisioning 15/15; check-arch, smoke green |
| 03-05 | keycloak-sso 35/35; provisioning 15/15; us-06 45/45; check-arch green; smoke green on rerun (first run: foundry-services testcontainers SSLRequest flake, crate 7/7 alone) |
| 03-06 | keycloak-sso 38/38; provisioning 15/15; default lane 677/677; check-arch green; smoke workspace-test gate failed only on foundry-store testcontainers start-up flakes (`PortNotExposed` / `PoolTimedOut`), foundry-store 25/25 binaries alone |
| `0c2553d`, `4e6a1f2` | keycloak-sso 38/38; provisioning 15/15; us-06 45/45; fmt, clippy `-D warnings`, check-arch pass |

### [REF] Quality gates

**Named faults: 25/25 killed (F1–F25).** Each was seeded alone, restored and
`cmp`-verified against HEAD.

| Faults | Scenario group | Note |
|---|---|---|
| F1–F5 | US-01 (03-01) | F2 (fixed state/nonce) survived the whole-`Location` oracle; killed after the Then compared state and nonce |
| F6–F9 | US-02 (03-02) | F8 (reason-naming refusal) survived the forbidden-phrase oracle; killed after the Then compared with a real wrong-password answer |
| F10–F15 | US-03 (03-03) | F10 (missing challenge cookie trusted) survived 29/29 at 03-03: the world held no account, so the forgery was refused downstream. Killed after `0c2553d` |
| F16–F18 | Replay, unreachable, identical refusals (03-04) | F18b (no-workspace → 403) survived the first sweep; killed after the When collected the no-workspace arrival |
| F19–F21 | US-04 (03-05) | F21 survived a greeting-only check in drafting; the user-id compare kills it |
| F22–F25 | US-05 (03-06) | Binary rebuilt for F24/F25 and after restore |

Variants also killed: F16b (`cycle_id()` rotate on refusal), F17 (`expect()` panic on
discovery failure), F-landing (password door → `/dashboard`).

- **Mutation (cargo-mutants):** not run. No production file changed in the
  increment, so the per-feature `--in-diff` scope is empty; the named faults stand in
  for it against the shipped production files.
- **Adversarial review** (nw-software-crafter-reviewer): APPROVED, zero defects.
- **Integrity:** `des-verify-integrity`: all 10 steps complete.
- **Full CI gate:** **GREEN on 4e6a1f2 (2026-10-03): exit 0, all gates green; acceptance (all tags) 903/903 scenarios, 6268/6268 steps, browser lane run** (`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`).

**Security follow-up (production, not changed here).** Without a challenge the
expected nonce is `""`, and an ID token with no nonce deserialises as `""`
(`serde(default)`), so the nonce comparison passes. The challenge cookie is the sole
guard against code injection; F10's scenario now pins that guard. Suggested
hardening: refuse an empty expected nonce in `foundry-oidc`, with a scenario.

### [REF] Pre-requisites

- `c755003` (v0.4.0): the shipped base flow; `InProcHarness::spawn_with_oidc` and
  `AppState.oidc` (used by the D3a provisioning lane).
- D3a (2026-09-27): the provisioning step module whose round-trip, password door and
  constants phase 03 shares.
- A warm `foundry` binary for scenario 23 (subprocess); rebuild before the lane.
- Base scenarios run with `FOUNDRY_OIDC_PROVISION_ROLE` unset (D3 exactly).

**Left as-is (DISTILL-owned).** The `keycloak-sso.feature` header (line 11) still says
it "provisions nothing (D3)", which is true only with `FOUNDRY_OIDC_PROVISION_ROLE`
unset. Outcome rows stay unregistered (see the D3a closing notes); no
`docs/product/kpi-contracts.yaml` or `docs/product/outcomes/registry.yaml` entry
exists for keycloak-sso.

**No migration.** The feature uses the single `feature-delta.md` layout; there are no
`design/`, `distill/walking-skeleton.md` or `discuss/journey-*` files to move.

## Wave: DISCUSS

> **OD-10 increment, 2026-10-04 — role withdrawal for provisioned accounts.** Lean,
> lightweight, cross-cutting (auth), brownfield: no walking skeleton. Appends to the
> DISCUSS above and edits none of it. **OD-10 is RESOLVED 2026-10-04** by the user
> decision recorded as D3b. AC-6.8 and DDD-22 are **superseded** by D3b. DISTILL
> must rewrite `keycloak-sso-provisioning.feature` scenario 11 ("A member provisioned
> earlier still signs in after the provision role is withdrawn"). The scenario is a
> pin, and it now pins the opposite outcome.

### [REF] Persona

The user is `persona-instance-operator` (Priya Raman, from
`docs/product/personas/persona-instance-operator.yaml`). She grants the Keycloak
realm role `foundry-user`, and from 2026-10-04 she also takes it away. The account
affected is a provisioned member, Nia Newcomer (`nia.newcomer@example.test`). The
provisioning scenarios already use Nia as a fixture identity, not as a researched
persona, so she is no new persona. Pat Operator is the control case: he was invited
and has a password, so this increment must leave him alone.

### [REF] JTBD one-liner

When I take the `foundry-user` role away from someone in Keycloak, I want that to stop
them signing in to foundry through Keycloak, so I can use the realm role as the one
gate both into foundry and back out of it (`job_id: job-sso-signin`). This extends
the job's **anxiety** force. Today the gate opens but never closes, so the operator
cannot trust "who holds the role" as an answer to "who can get in". No new job;
`docs/product/jobs.yaml` is **not** edited.

### [REF] Locked decisions

| ID | Decision | Verdict | Rationale |
|----|----------|---------|-----------|
| D3b | 2026-10-04: an account CREATED BY role provisioning (D3a) must hold the provision realm role at EVERY Keycloak sign-in. If it does not, the sign-in gets the generic refusal (D7). Accounts that existed before, or were linked or invited normally, are unaffected: they keep D3's link-only behaviour | LOCKED (user) | Resolves OD-10 and supersedes AC-6.8 / DDD-22. The role was the reason the account exists, so it stays the condition for using the Keycloak door. Accounts that came in another way never depended on the role, so withdrawing it means nothing for them. |
| D8 | Withdrawal deletes nothing: the account, its membership, display name, password state, and every issue and comment it authored stay intact | LOCKED (user) | Authorship has a foreign key to `users.id`. D3's [WHY] already recorded that deleting the account orphans authorship. Re-granting the role must restore access with no repair step. |
| D9 | foundry must know which accounts were provisioned. The record is written when the account is created, and nothing later removes it: not a reset, not a password change, not a sign-in | LOCKED (requirement only; the schema belongs to DESIGN) | Today a provisioned account can only be recognised by `password_hash IS NULL`. OD-8 (reset allowed) erases that, so a reset would turn a provisioned account into one that looks invited. |
| D10 | Revocation applies at the NEXT Keycloak sign-in. An existing foundry session lives until it expires or the member signs out | LOCKED | foundry learns roles only from an ID token, at sign-in. Revoking in real time would need back-channel logout or role polling against Keycloak, and both are out of scope. |
| D11 | The role checked is the one `FOUNDRY_OIDC_PROVISION_ROLE` names at sign-in time, not the role that was configured when the account was provisioned. The match is the same exact, case-sensitive, `realm_access.roles`-only match as OD-9 | LOCKED (derived from D3b's "the provision realm role") | One rule and one setting. If the operator renames the role, every provisioned member needs the new one. |
| D12 | The refusal is logged on the existing `oidc sign-in refused` line, with its own reason, separate from a newcomer's `identity lacks provision role`. Proposed wording: `provisioned account lacks provision role`. DESIGN may reword it. The response carries nothing extra | LOCKED | The operator needs the log to tell "a stranger without the role" from "a member whose role I removed". Under D7 the response must not tell them apart. |

Rejected alternatives (user, 2026-10-04):

| Alternative | Why rejected |
|---|---|
| Keep today's behaviour (AC-6.8: link regardless of role) | Leaves the role gate cosmetic after the first sign-in. This is the residue D3's [WHY] named. |
| Require the role for EVERY SSO sign-in while provisioning is on | Would lock out the operator and invited members who were never given the role. That breaks D3's linking half and the anxiety force. |
| Remove membership when the role is withdrawn | Destructive and one-way. The member could not come back on re-grant without a manual repair, and the change would reach into team and workspace state well beyond authentication. |

### [REF] User story

**US-07: Withdrawing the provision role closes foundry's Keycloak door to a provisioned member** (`job_id: job-sso-signin`). Addendum 2026-10-04 (D3b). Supersedes AC-6.8.

As the operator, I want removing the `foundry-user` realm role to stop a provisioned
member signing in through Keycloak, so that the role I used to let them in is also
how I take that access away.

#### Elevator Pitch

Before: Priya removes `foundry-user` from Nia in Keycloak, but Nia still lands on the board through "Sign in with Keycloak". Revoking the role does nothing in foundry.
After: Nia clicks "Sign in with Keycloak" at `https://foundry.<domain>/sign-in`, which goes through `/auth/oidc/start`, and is sent back to the sign-in page with the same generic message a wrong password gets. Once Priya re-grants the role, Nia's next sign-in lands on her board as the same account.
Decision enabled: Priya decides she can manage foundry access from Keycloak alone, with the realm role as the gate in both directions, because she has watched a withdrawn role close the door and a re-grant open it again.

#### Acceptance criteria

All ACs below assume `FOUNDRY_OIDC_PROVISION_ROLE=foundry-user` unless stated.

- AC-7.1: Nia's account was created by provisioning. If her ID token no longer lists `foundry-user` in `realm_access.roles`, the Keycloak sign-in is refused with no session cookie, and the response is byte-identical to a wrong-password refusal (status 401 plus the CSRF-masked body, D7).
- AC-7.2: After that refusal, Nia still has exactly one account and one `member` membership in the original workspace. Her display name and her authored issues and comments are unchanged (D8).
- AC-7.3: Once the role is granted again, Nia's next Keycloak sign-in lands on the board as the same account (same user id), and no second account is created.
- AC-7.4: Pat Operator was invited and has a password. His Keycloak sign-in without `foundry-user` still links and lands on the board, exactly as before this change (D3b).
- AC-7.5: Nia's account is still treated as provisioned after she sets a password through forgot-password and reset (OD-8). A Keycloak sign-in without the role is still refused (D9).
- AC-7.6: The refusal in AC-7.1 is logged on the `oidc sign-in refused` line with a reason distinct from `identity lacks provision role`. The response does not include that reason (D12).
- AC-7.7: A session Nia established before the role was withdrawn keeps working until it expires or she signs out (D10). This is pinned as deliberate behaviour, not as a gap.
- AC-7.8 (*provisional, OD-12*): Nia's password door is not affected by the role. If she has set a password, she can still sign in with it after the role is withdrawn. If she has not, the door refuses her as it does today (AC-6.6).
- AC-7.9 (*provisional, OD-13*): With `FOUNDRY_OIDC_PROVISION_ROLE` unset, Nia's Keycloak sign-in links and lands on the board whatever roles she holds. This is exactly D3, as AC-6.1 promises for "unset".

### [REF] Definition of Done

1. AC-7.1 to AC-7.7 are green at layer 3 (real axum via `build_router`, real Postgres). AC-7.6 is checked at unit level, because the lane captures no tracing (D3a precedent).
2. AC-7.8 and AC-7.9 are green as confirmed, or rewritten if the user overturns OD-12 or OD-13.
3. Scenario 11 is rewritten to AC-7.1 and AC-7.2, not deleted. Scenarios 1 to 10 and the 23 base scenarios stay green unchanged.
4. Accounts provisioned before this change are marked by the backfill rule chosen for OD-14, and that rule has its own test.
5. `cargo xtask ci` is green (with `FOUNDRY_XTASK_INCLUDE_DOCKER=1`). Per-feature mutation is at least 80% over the files changed.
6. AC-6.8 and DDD-22 are marked superseded by later dated sections, not edited in place. `CHANGELOG.md` is updated.

### [REF] Out of scope

- Ending live sessions when a role is withdrawn: no back-channel logout and no role polling (D10).
- Gating the password door on a Keycloak role (see OD-12). foundry cannot ask Keycloak at password sign-in without breaking D2.
- Deleting accounts, removing memberships, or reassigning authorship (D8).
- Role checks for accounts that were not provisioned (D3b).
- A UI or CLI for viewing or editing an account's provisioned marker.
- Client roles in `resource_access` (OD-9 stands).

### [REF] Driving ports

| Port | Change |
|---|---|
| `GET /auth/oidc/callback` | CHANGED: a provisioned account now has to pass a role check before it is linked (DDD-15 step 1 splits) |
| `GET /auth/oidc/start` | Unchanged; it is the entry point for the Elevator Pitch |
| `POST /sign-in` (password) | Unchanged, if OD-12 is confirmed. Regression only |
| `POST /forgot-password`, reset | Unchanged. AC-7.5 only depends on the provisioned marker surviving a reset |

### [REF] Pre-requisites

- D3a as shipped: `Store::provision_federated_member`, the DDD-15 order in `oidc::callback`, and migration `0016`.
- A provisioned marker on the account (D9). DESIGN owns the schema. It is a new forward migration, plus the OD-14 backfill.
- The provider double in the acceptance lane can already withdraw a role. The step "the identity provider no longer grants the newcomer…" exists for scenario 11.
- Production context: prod is still on v0.6.0, and nobody has checked whether `FOUNDRY_OIDC_PROVISION_ROLE` is set there. That decides how much the OD-14 backfill matters.

### [REF] Outcome KPIs

| KPI | Target | Measurement |
|---|---|---|
| Keycloak sign-ins that get in on a provisioned account with the role withdrawn | 0 | AC-7.1 scenario. `provisioned account lacks provision role` log lines with no session created |
| Non-provisioned accounts refused because of this change | 0 | AC-7.4 scenario. Base scenarios still 23/23 and provisioning scenarios 1–10 still green |
| Manual steps to restore a re-granted member | 0 | AC-7.3: same user id, no operator action in foundry |

### [REF] Slice

`slices/slice-04-role-withdrawal.md`: a single slice, about 1 day.

### Open decisions for DESIGN / the user

- **OD-10**: RESOLVED 2026-10-04 (D3b, D8). It supersedes AC-6.8 and DDD-22.
- **OD-12: does withdrawing the role also close the password door?** *Recommendation: no.* The role gates only the Keycloak door, and the password door stays the local break-glass route (D2). foundry cannot check a Keycloak role at password sign-in without calling Keycloak, which would break D2's availability. A "seen refused" flag would not help either, because the member just uses the password door and never triggers it. **Residue the user must accept:** forgot-password reaches provisioned accounts (OD-8), so a member whose role was withdrawn can reset a password and get in through the password door. Under this recommendation, revocation is complete only if the operator also removes the membership locally. That route ships today (`member_invites::submit_remove_member`), and with no workspace the member is refused fail-closed. The user should confirm this explicitly. The alternative is to refuse forgot-password and reset for a provisioned account that has no password. That narrows the hole but does not close it for a member who already set one.
- **OD-13: provisioning switched OFF later, with the role variable unset.** *Recommendation: link-only.* A provisioned account signs in like any linked account, exactly D3. AC-6.1 promises that "unset = exactly D3", and the D3a DELIVER rollback note depends on it ("reverting the code with the role unset is link-only"). **Risk:** an operator who unsets the variable just to "stop new accounts" also reopens the Keycloak door to every member whose role was withdrawn. The alternative is to refuse provisioned accounts whenever provisioning is off. That is safer, but it locks out the members with no warning on an innocent config change. The operator docs should state the consequence whichever way this goes.
- **OD-14: accounts provisioned before this change have no marker.** *Recommendation:* the forward migration backfills the marker on every row with `password_hash IS NULL`. Bootstrap and invite-accept always set a password, so a NULL hash means provisioned. **Risk:** an account that was provisioned and then reset its password before the migration looks invited, so it would keep link-only access. The `oidc identity provisioned` info log lines carry the user id and can find those accounts by hand. If provisioning was never enabled in production (to be checked; prod is on v0.6.0), the backfill is a no-op and the risk is zero.
- **OD-15 (DESIGN):** where the provenance check goes in the DDD-15 order. The requirement only says that a provisioned account is checked before the link in step (1), and that every other account is unaffected. Timing does not need to match: the OIDC path has no timing AC today, and both arms do the same account lookup.

### [REF] Open-question resolutions (user, 2026-10-04)

- **OD-12 — RESOLVED:** the password door stays open. The role gates only the Keycloak
  door, and the password door remains the local break-glass route (D2). AC-7.8 becomes
  firm. Withdrawing the role alone does **not** fully revoke a provisioned member, who can
  still set a password through forgot-password. Full revocation also requires removing
  their workspace membership in foundry. The operator docs must say so.
- **OD-13 — RESOLVED:** link-only, exactly D3. With `FOUNDRY_OIDC_PROVISION_ROLE` unset,
  a provisioned account signs in through Keycloak like any linked account. AC-7.9 becomes
  firm. Risk accepted: unsetting the variable also reopens the Keycloak door to members
  whose role was withdrawn. The operator docs must say so.
- **OD-14 — RESOLVED:** the migration backfills the provisioned marker onto every account
  that has no password. The bootstrap claim and invite-accept always set one. Accepted
  risk: an account that was provisioned and then reset before the migration stays
  link-only. The `oidc identity provisioned` log lines can find such accounts by hand.
- **D11 (confirmed):** the role checked is whatever `FOUNDRY_OIDC_PROVISION_ROLE` names
  at sign-in time, matched exactly against realm roles only (OD-9).
- **OD-15** (where the check sits in the DDD-15 order) remains a DESIGN decision.

## Wave: DESIGN

> **OD-10 increment, 2026-10-04 — role withdrawal (D3b).** Scope: application /
> components. Mode: propose. Density: lean, Tier-1 [REF] only. Appends to the DESIGN
> above and edits none of it. DDD-1..DDD-21 stand. **DDD-22 is SUPERSEDED by DDD-28**
> (AC-6.8 was superseded in DISCUSS by AC-7.1/AC-7.2). Paradigm unchanged
> (object-oriented, trait-injected effects). No ADR: every decision is an extension of
> DDD-13..DDD-20 inside an existing component; none changes a crate boundary, a
> dependency, or a security pin.

### [REF] Decisions

| ID | Decision | Verdict |
|---|---|---|
| DDD-23 | **Provenance is `users.provisioned_at TIMESTAMPTZ NULL`.** Non-NULL means "this account was created by role provisioning". Rejected: `provisioned_by_oidc BOOLEAN NOT NULL DEFAULT false` (see trade-off below) | LOCKED |
| DDD-24 | **Migration `0017_users_provisioned_at.sql`**, forward-only, one file, one transaction: (1) `ALTER TABLE users ADD COLUMN provisioned_at TIMESTAMPTZ NULL`, with no default and no index; (2) the OD-14 backfill `UPDATE users SET provisioned_at = created_at WHERE password_hash IS NULL AND provisioned_at IS NULL`. `created_at` is the true provisioning moment for such a row, so the backfilled value is accurate, not a placeholder | LOCKED |
| DDD-25 | **`Store::provision_federated_member` writes the marker in its existing INSERT**, so it lands in the same transaction as the user row and the membership. On the `Created` path only. The `ON CONFLICT (email_lower) DO NOTHING` re-read path (`Existing`) writes nothing; DDD-16's "never touches an existing row" stands. The timestamp comes from the caller's injected `Clock` (a `now` parameter, the `reset_password_and_consume` convention), not from SQL `now()`. `FederatedProvisionOutcome` is unchanged | LOCKED |
| DDD-26 | **Read model: `UserRow` gains `provisioned: bool`**, computed in SQL as `provisioned_at IS NOT NULL OR password_hash IS NULL`, in both queries that build `UserRow` (`find_user_by_email`, `find_user_by_id`). The `OR password_hash IS NULL` arm is the OD-14 rule applied at read time. It closes the rolling-deploy window: a v0.6.2 replica can still provision, without the marker, after 0017 has run (DDD-24 consequences) | LOCKED |
| DDD-27 | **Nothing clears the marker (D9).** The only code that writes `provisioned_at` is the DDD-25 INSERT plus the 0017 backfill. The three password writers set `password_hash` only: `reset_password_and_consume` (`foundry-store/src/lib.rs:1255`), `update_user_password` (`:2764`) and `set_first_admin_password_and_consume` (`:382`). Sign-in writes nothing to `users`. So AC-7.5 holds by construction and is pinned behaviourally (AC-7.5 scenario plus a store test on `update_user_password`) | LOCKED |
| DDD-28 | **OD-15: the check sits inside DDD-15 step (1), right after `find_user_by_email` returns `Some`, before `establish_session`.** Step (1) splits: (1a) not provisioned → link, role ignored (D3, unchanged; AC-7.4); (1b) provisioned, `provision_role` is `None` → link (OD-13, AC-7.9); (1c) provisioned, role held → link (AC-7.3); (1d) provisioned, role not held → refuse. Everything before step (1) keeps its order: challenge cookie, `state`, `code`, exchange + RS256 + nonce, then `email_verified`. Steps (2)–(5) are unchanged. The `Existing` outcome of step (5) is NOT re-judged, because that identity has just passed `judge_newcomer` and so holds the role. **Supersedes DDD-22** | LOCKED |
| DDD-29 | **The verdict is a pure function, a sibling of `judge_newcomer`:** `judge_returning(provisioned, provision_role, identity) -> Result<(), &'static str>`. Contract shape: pure, return-only, no I/O. It is table-tested exactly like `judge_newcomer`. The callback stays the imperative shell. The role match reuses `IdentityClaims::has_realm_role` (exact, case-sensitive, realm-only: D11 / OD-9) and reads the role from `provider.provision_role()` at sign-in time (D11) | LOCKED |
| DDD-30 | **DDD-20's reason list gains `provisioned account lacks provision role`** (D12's wording, kept). It is a new `const` beside `LACKS_PROVISION_ROLE`, and it goes through the existing single `refuse()`, so status, body and the cleared challenge cookie are the shipped D7 refusal. Nothing is added to the response. No new log line on success | LOCKED |
| DDD-31 | **Password door untouched (OD-12).** `submit_signin` ignores `UserRow.provisioned`. DDD-19 (a NULL hash is treated as an unknown address) stands. `submit_forgot` / `submit_reset` need no change | LOCKED |
| DDD-32 | **`Store::probe` asserts that `users.provisioned_at` exists in `current_schema()`**, beside the shipped 0006/0007 column checks. `find_user_by_email` serves BOTH doors. A binary that boots against a pre-0017 schema would 500 every password sign-in as well as every Keycloak one. The probe makes that substrate lie a `/readyz` refusal instead (Earned Trust) | LOCKED |
| DDD-33 | **Operator documentation goes in `CHANGELOG.md` `[Unreleased]`**, because no OIDC operator page exists in this repo: `FOUNDRY_OIDC_PROVISION_ROLE` is documented only there and in this brief. The entry has a `### Changed` item (withdrawal closes the Keycloak door at next sign-in; live sessions run to expiry) that states both accepted residues in plain words. OD-12: full revocation also needs removing the workspace membership, because forgot-password still reaches the account. OD-13: unsetting the variable reopens the Keycloak door to withdrawn members. It also has a `### Migration notes` item for 0017 (DDD-24 consequences). The brief gets a dated "planned" note now; DELIVER finalize rewrites it as shipped | LOCKED |

**DDD-23 trade-off: timestamp vs boolean.**

| | `provisioned_at TIMESTAMPTZ NULL` (chosen) | `provisioned_by_oidc BOOLEAN NOT NULL DEFAULT false` |
|---|---|---|
| ADD COLUMN cost | Catalog-only, instant | Catalog-only, instant (PG11+ fast default) |
| Old binary (v0.6.2) inserting a row | Gets NULL = "not provisioned" | Gets false = "not provisioned". Same exposure |
| Carries *when* | Yes. Backfill uses `created_at`, so an operator can line it up against `oidc identity provisioned` log lines (the OD-14 hand-search) | No |
| Schema idiom | Matches `used_at`, `revoked_at`, `notified_at`, `deleted_at` (nullable event timestamps) | No precedent for a provenance flag |
| Tri-state risk | None in practice. NULL is the one "no" value, and the column is never written back to NULL (DDD-27) | None |

The boolean buys nothing the timestamp lacks. The timestamp buys audit context for free.

**DDD-24 consequences (NFR-MIG-03).**
- *Runtime.* The ADD COLUMN is catalog-only. The backfill is one sequential scan of `users` (there is no index on `password_hash`) that updates only NULL-hash rows. On a 10k-user database that is milliseconds. On prod it updates zero rows unless provisioning was ever enabled (prod is on v0.6.0, where provisioning does not exist). The ACCESS EXCLUSIVE lock from the ALTER is held for the length of the transaction, i.e. that scan.
- *Forward-compatible with v0.6.2 (safe for rolling deploys).* Every `users` INSERT in v0.6.2 names its columns, and nothing selects `*` from `users`, so the old binary neither sees nor breaks on the column. **Window:** while old and new replicas coexist after 0017 has run, an old replica can provision an account with `password_hash = NULL` and `provisioned_at = NULL`. DDD-26's read predicate treats that account as provisioned, so D3b holds for it. The residue is an account provisioned in that window that ALSO resets its password before the marker is ever written. It is the same accepted-risk class as OD-14, and it is narrower.
- *Rollback to v0.6.2.* Safe: the column is ignored and no down migration is needed. While on v0.6.2, D3b is not enforced. Accounts provisioned during the rollback are covered on return by DDD-26, subject to the same residue.
- *Transactional.* sqlx applies the file in one transaction under the shipped `MIGRATION_LOCK_ID` advisory lock. A failure leaves 0016 state intact. There is no `CONCURRENTLY` and no partial state.

**OD-14 assumption verified against the code.** Every `users` INSERT outside
provisioning binds a non-optional `&str` hash: `create_member_and_consume`
(`foundry-store/src/lib.rs:455`), `create_initial_workspace` (`:621`),
`claim_bootstrap_and_create_workspace` (`:714`), `provision_workspace` (`:2538`) and
`create_user` / `doctor add-test-user` (`:2791`). The only literal `NULL` is
`provision_federated_member` (`:523`), and no `UPDATE` sets the hash to NULL. So a NULL
hash means provisioned, as DISCUSS assumed. No contradiction.

### [REF] Reuse Analysis

| Existing component | File | Overlap | Decision | Justification |
|---|---|---|---|---|
| `judge_newcomer` | `crates/foundry-app/src/oidc.rs:273` | Role verdict for an identity | **EXTEND** (sibling pure fn, same shape, same table-test idiom) | It cannot simply be reused: its `None` role answers `NO_ACCOUNT`, while a returning provisioned account with `None` must LINK (OD-13). The sibling shares `has_realm_role` and the refusal-const pattern |
| `refuse()` + DDD-20 reason consts | `oidc.rs:60`, `:223-225` | Logged generic refusal | **EXTEND** (one new const) | D7 / D12 require the single shipped refusal path |
| `IdentityClaims::has_realm_role` | `crates/foundry-oidc/src/lib.rs:239` | Exact realm-role match | **EXTEND** (call verbatim) | Already pins OD-9 / D11 semantics, with unit tests |
| `provider.provision_role()` | `foundry-oidc/src/lib.rs:322` | Role named at sign-in time | **EXTEND** (call verbatim) | D11 "whatever the env names at sign-in" is exactly what it returns |
| `Store::provision_federated_member` | `foundry-store/src/lib.rs:504` | Creating the provisioned row | **EXTEND** (one more column in the INSERT, `now` parameter) | Same transaction, by construction |
| `Store::find_user_by_email` / `find_user_by_id` + `UserRow` | `foundry-store/src/lib.rs:1122`, `:2742`, `:3265` | Account lookup in callback step (1) | **EXTEND** (one computed column, one field) | Step (1) already does this lookup. A second query would add a round-trip and a place for the two to drift |
| `Store::probe` column assertions | `foundry-store/src/lib.rs:220-263` | Substrate check for required columns | **EXTEND** (one more assertion) | Precedent: the 0006 and 0007/0008 checks |
| Migration idiom | `crates/foundry-store/migrations/0016_nullable_password_hash.sql` | Additive `users` change | **EXTEND** (new numbered file `0017`) | Forward-only numbered migrations are the shipped mechanism |
| Acceptance step "the identity provider no longer grants the newcomer…" | `crates/foundry-acceptance/src/steps/feature_keycloak_sso_provisioning.rs` | Withdrawing the role in the provider double | **EXTEND** (reuse) | Already drives scenario 11 |

Zero CREATE NEW, apart from the migration file, which is the mechanism and not a component.

### [REF] Component decomposition

| Component | Path | Change |
|---|---|---|
| Migration 0017 | `crates/foundry-store/migrations/0017_users_provisioned_at.sql` | **NEW**: ADD COLUMN + OD-14 backfill (DDD-24) |
| Provisioning write | `crates/foundry-store/src/lib.rs::provision_federated_member` | **EDIT**: set `provisioned_at` in the INSERT; `now` parameter (DDD-25) |
| User read model | `crates/foundry-store/src/lib.rs::{UserRow, find_user_by_email, find_user_by_id}` | **EDIT**: `provisioned: bool` computed in SQL (DDD-26) |
| Substrate probe | `crates/foundry-store/src/lib.rs::probe` | **EDIT**: assert `users.provisioned_at` (DDD-32) |
| Callback step (1) | `crates/foundry-app/src/oidc.rs::callback` | **EDIT**: call `judge_returning` on `Ok(Some(u))`; refuse on `Err` (DDD-28) |
| Returning-account verdict | `crates/foundry-app/src/oidc.rs::judge_returning` + reason const | **NEW fn in existing module** (DDD-29/30) |
| Password door, forgot, reset, change-password, doctor set-password | `crates/foundry-app/src/signin.rs`, `reset_password.rs`, `admin_cli.rs` | **NO CHANGE**: regression only (DDD-27/31) |
| Provisioning feature | `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature` | **DISTILL**: rewrite scenario 11; add AC-7.2..7.9 scenarios |
| Operator docs | `CHANGELOG.md` `[Unreleased]` | **EDIT** at DELIVER (DDD-33) |
| Architecture SSOT | `docs/product/architecture/brief.md` | Planned note now; shipped wording at DELIVER finalize |

### [REF] Driving ports

| Port | Handler | Change |
|---|---|---|
| `GET /auth/oidc/callback` | `oidc::callback` | CHANGED: step (1) splits per DDD-28 |
| `GET /auth/oidc/start` | `oidc::start` | Unchanged |
| `POST /sign-in` | `signin::submit_signin` | Unchanged (OD-12). Regression for AC-7.8 |
| `POST /forgot-password`, reset form | `signin::submit_forgot` / `reset_password::submit_reset` | Unchanged. AC-7.5 depends on DDD-27 |
| Change password (signed in), `foundry doctor` set-password | `signin.rs:356`, `admin_cli.rs:1673` (both via `update_user_password`) | Unchanged. Neither touches the marker (DDD-27) |

### [REF] Driven ports and adapters

| Driven port | Class | Production adapter | Change | Acceptance treatment |
|---|---|---|---|---|
| `users` lookup by `email_lower` (now with `provisioned`) | internal | `Store` / Postgres | EDIT (DDD-26) | **Real** Postgres, per-scenario schema |
| Provisioning transaction | internal | `Store::provision_federated_member` | EDIT (DDD-25) | **Real** |
| Keycloak token exchange (roles in ID token) | external | `foundry-oidc` over `reqwest` | Unchanged | Shipped fake issuer. The withdraw/re-grant steps already exist |
| Clock (`provisioned_at`) | external, non-deterministic | `Arc<dyn Clock>` | Newly passed to the store call | Shipped `MockClock` / `SystemClock` |
| Schema substrate | internal | `Store::probe` | EDIT (DDD-32) | Store test against a schema missing the column |

No external integration is added or changed, so there are no new contract-test annotations.
Slice 03's cluster e2e against the real Keycloak remains the contract test for role claims.

### [REF] Technology choices

None new. Rust 1.88 / edition 2021, sqlx + Postgres 16, `tracing`; every one is already
in the workspace. Net dependency delta: zero. `cargo deny` is unaffected.

### [REF] C4

No new diagram. The container view (brief.md, and the DESIGN [REF] C4 — Container
above) is unchanged: the edit stays inside `foundry-app` → `foundry-store` →
PostgreSQL, along the existing "Look up user by email_lower" relation. The callback
order is the DDD-28 table.

### [REF] Open questions deferred to DISTILL / DELIVER

- **OQ-5 (DISTILL): AC-7.6 at unit level.** `refuse()` needs an `AppState`. Recommended: assert that `judge_returning` yields the new const and that it differs from `LACKS_PROVISION_ROLE`. The log wiring is the shipped `refuse()` line. Alternatively, capture `tracing` around `refuse()` with a minimal state. DISTILL picks.
- **OQ-6 (DISTILL): the OD-14 backfill test (DoD 4).** It needs a schema at 0016 with mixed rows, then 0017 applied. The precedent is `feature_mwt_slice_05_migration_guarantee` and `run_migrator_timed`. It could be a store integration test rather than a Gherkin scenario.
- **OQ-7 (DISTILL): the DDD-26 rolling-window arm** (NULL hash, NULL marker, still provisioned) needs its own store-level example, or mutation of the `OR` survives.
- **OQ-8 (DELIVER): an optional `xtask check-arch` scanner** that fails the build if `provisioned_at` appears in an `UPDATE` anywhere in `crates/`. It would enforce D9 structurally rather than only behaviourally. Recommended but not required; DDD-27's tests are the floor.
- **OQ-9 (outside this repo):** the deploy repository's environment docs for `FOUNDRY_OIDC_PROVISION_ROLE` should carry the OD-12/OD-13 consequences too, if that repo documents the variable.
- **OQ-10 (operator, before release):** check whether prod ever set `FOUNDRY_OIDC_PROVISION_ROLE`. It sizes the backfill (expected: zero rows) and goes in the 0017 migration note.

## Wave: DISTILL

> **OD-10 increment, 2026-10-04 — role withdrawal (US-07, D3b).** Rigor profile
> `adr-025-scaffolded-red`. Lean, Tier-1 [REF] only. Appends to the DISTILL above and
> edits none of it. Rewrites provisioning scenario 11 (the AC-6.8 / DDD-22 pin, both
> superseded) and adds scenarios 12–18 to the same `.feature`. Nothing is committed by
> DISTILL; DELIVER commits on GREEN.

### [REF] Reconciliation

**Passed — 0 contradictions.** D3b is implemented step by step in DDD-28: (1a) not
provisioned → link (D3b's "unaffected"), (1b) role unset → link (OD-13), (1c) role
held → link, (1d) role missing → refuse. D8 maps to DDD-28 ("refuse", no delete). D9
maps to DDD-23/25/27, D10 to "no session revocation" (nothing in DESIGN), D11 to
DDD-29 (`provider.provision_role()` at sign-in, `has_realm_role`), and D12 to DDD-30.
OD-12 maps to DDD-31 and OD-14 to the DDD-24 backfill and its DDD-26 read arm. DEVOPS
is still absent, so this is a WARN and the default matrix applies. Tier A only: every
scenario is layer 3 (real axum via `build_router`, real Postgres, the shipped
`OidcProvider` against the RS256 double), so they are example-based (Mandates 9 and
11). Language: Rust (cucumber-rs + `cargo test`). There is no Python state-delta port,
and this project's precedent is direct assertions at layer 3+.

### [REF] Scenario list with tags

`.feature` SSOT: `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature`.
Every row below carries `@keycloak-sso @keycloak-sso-provisioning` (feature tags),
`@us-07 @driving_port @real-io`, and `@pending`.

| # | Scenario | Extra tags | AC | Oracle |
|---|---|---|---|---|
| 11 | A provisioned member whose provision role is withdrawn is turned away and keeps everything they had (**rewritten**, was the AC-6.8 pin) | `@error @security` | 7.1, 7.2 | The refusal is byte-identical (CSRF masked) to a live wrong-password answer, with no session cookie. Then exactly one `users` row; exactly one `member` membership in the original workspace; `display_name` = "Nia Newcomer"; the issue she filed and the comment she made through her own session are still hers and not tombstoned |
| 12 | A provisioned member left holding only a different role is turned away (2 examples: `some-other-role`, `Foundry-User`) | `@error @security` | 7.1 (D11 / OD-9 boundary) | Byte-identical refusal; one account |
| 13 | A member turned away after the withdrawal is let back in as the same account once the role is granted again | — | 7.3 | The Given composes 11's When and refusal (Pillar 2). The sign-in then lands on `/` with a session. That session's user id equals the id recorded when provisioning created the account. One account |
| 14 | A provisioned member who chose a password through a reset is still turned away without the role | `@error @security` | 7.5 | Reset reuses scenario 10's real forgot-password → emailed link → reset form. Byte-identical refusal; one account |
| 15 | A provisioned member who chose a password can still use it after the role is withdrawn | — | 7.8 (OD-12) | The password door lands on `/` and the greeting is "Nia Newcomer" |
| 16 | A provisioned member already signed in keeps working after the role is withdrawn | — | 7.7 (D10) | The pre-withdrawal session still opens the board, greeted by name |
| 17 | An invited member without the provision role still signs in through the identity provider | — | 7.4 (D3b control) | Pat has a password account and no realm roles; he links, lands, is greeted "Pat Operator", and has one account |
| 18 | With provisioning switched off a provisioned member signs in whatever roles they hold | — | 7.9 (OD-13) | Role unset; the account came from the store's real `provision_federated_member`; he links, lands and is greeted, with one account |

Examples: 9 across 8 scenarios. Error/security examples: 11, 12a, 12b and 14, i.e.
**4 of 9 = 44%** (target ≥ 40%). The base feature is not touched, and scenarios 1–10
are unchanged.

AC-7.6 (a distinct log reason with nothing added to the response) is split by layer.
"Nothing added to the response" is the byte-identical comparison in 11, 12 and 14.
The distinct reason is unit-level (OQ-5, below), because the lane captures no
tracing (D3a precedent).

### [REF] RED classification (fail-for-the-right-reason gate)

Procedure (D3a precedent): `cargo test -p foundry-acceptance --test acceptance
--no-run` to warm the binary. The `.feature` was copied, then
`sed -i '' 's/ @pending$//'` was run on it, then
`FOUNDRY_ACCEPTANCE_TAGS=keycloak-sso-provisioning timeout 900 cargo test -p
foundry-acceptance --test acceptance`, then the file was restored from the copy. The
restore was verified with `cmp` (identical), and the tag-line diff against the
pre-DISTILL file showed only scenario 11's line changed plus seven added. Result:
**23 examples, 18 passed, 5 failed; 0 parsing errors, 0 undefined steps, 0 hook
errors.**

| # | Result | Class | Failing step — message |
|---|---|---|---|
| 11 | FAIL | MISSING_FUNCTIONALITY | `Then the newcomer is turned away exactly as a wrong password is`: "a refusal must not establish a session". The withdrawn member arrives signed in, which is today's link-only step (1) |
| 12a, 12b | FAIL | MISSING_FUNCTIONALITY | Same step, same message |
| 13 | FAIL | MISSING_FUNCTIONALITY | `And the newcomer has been turned away through the identity provider` (the Given that composes 11), same message |
| 14 | FAIL | MISSING_FUNCTIONALITY | `Then … turned away …`, same message |
| 15, 16, 17, 18 | PASS | GREEN_ALREADY (guards) | — |
| 1–10 (14 examples) | PASS | unchanged | — |

**BROKEN: 0.** The GREEN_ALREADY guards are legitimate. D3b must PRESERVE the
behaviour they pin (the password door, live sessions, non-provisioned accounts, and
role-unset link-only), and the shipped callback already shows it. They are not
vacuous: each one fails if DELIVER over-applies the check (see named faults below).
Un-pend them together with 11.

The steps that 11 and 13 never reach were proven by a throwaway probe scenario
(`@distill-probe`, deleted afterwards). It ran provision → file an issue and comment
→ sign in again with the role → `same account`, `exactly one account`, `ordinary
member of the original workspace`, `name and the work they authored are unchanged`:
12/12 steps green. So 11's and 13's later Thens hide no BROKEN.

### [REF] Scaffolds (RED-ready, Mandate 7)

No production stub. Each scaffold is test code that compiles against today's
production and is `#[ignore]`d. Each stands in for the API it awaits with a local shim
that `panic!`s with `SCAFFOLD: … -- RED scaffold`, so un-ignoring it gives a RED
assertion-class failure, never a compile error. `grep -rn "SCAFFOLD" crates/` finds
them, and DELIVER leaves zero behind.

| Artifact | What it specifies | How it stays compiled / ignored | `--include-ignored` today |
|---|---|---|---|
| `crates/foundry-app/src/oidc.rs` `tests::a_returning_account_is_refused_only_when_provisioned_and_the_role_is_missing` (OQ-5, AC-7.6) | 10-row table over (provisioned, role, held): not provisioned → `Ok` ×3 (incl. role set and roles missing); provisioned + role `None` → `Ok` ×2; provisioned + role held → `Ok` ×2; provisioned + role missing / other / `Foundry-User` → `Err("provisioned account lacks provision role")` ×3; plus `assert_ne!` against `LACKS_PROVISION_ROLE` (D12) | Calls `judge_returning_scaffold` (a `#[cfg(test)]` shim with the DDD-29 signature) and a test-local reason literal. `#[ignore = "DISTILL scaffold (US-07, DDD-29): …"]`. **DELIVER:** delete the shim and the literal, call `judge_returning` and the new const, un-ignore | FAILED — `SCAFFOLD: judge_returning (DDD-29) not yet implemented` |
| `crates/foundry-store/tests/users_provisioned_at.rs` `the_upgrade_marks_exactly_the_password_less_accounts_as_provisioned_when_they_were_created` (OQ-6, DoD 4) | The schema is staged at 0016 from a temp copy of the production migrations (run by the real `run_migrations_from_dir`). Mixed rows go in: password-less ×2 (different `created_at`), password ×1, "reset before the upgrade" ×1. After applying 0017, only the password-less rows are marked, with `provisioned_at = created_at`; the reset row stays unmarked (OD-14's accepted residue); a re-run changes nothing | New column read through SQL only. An entry guard asserts the 0017 file exists. `#[ignore]` with reason | FAILED — `SCAFFOLD: migration 0017_users_provisioned_at.sql (DDD-24) not yet written` |
| same file, `an_account_is_read_as_provisioned_when_marked_or_password_less` (OQ-7, DDD-26) | By email AND by id: no password + no marker (the rolling-deploy window) → provisioned; marker + password → provisioned; password + no marker → not provisioned | Reads `UserRow.provisioned` through the `provisioned(&UserRow)` shim. `#[ignore]` | FAILED — same guard |
| same file, `no_password_write_clears_the_provisioned_marker` (DDD-25/27, AC-7.5 at the store) | `provision_federated_member` sets the marker. `reset_password_and_consume` and `update_user_password` leave it unchanged, and the row still reads provisioned | Same guard and shim. **DELIVER:** pass `now` once DDD-25 adds it | FAILED — same guard |

The 0016 staging and mixed-row insert path was proven against a real container by a
throwaway probe test (deleted). The schema stood up at 0016, had no `provisioned_at`,
and `created_at` round-tripped. So OQ-6's harness is not a hidden BROKEN.

Test-support edits (no behaviour change; the keycloak-sso lane stays green):
`feature_keycloak_sso.rs` gains `seed_team_project` (extracted from
`seed_operator_project`, so the operator and the newcomer share one seeder), and
`csrf_for_session` and `session_user_id` become `pub(crate)`.
`us_06_signin.rs::submit_forgot_password` becomes `pub(crate)` so 14 and 15 reuse
scenario 10's step rather than copy it. `newcomer_already_provisioned` now also
records the session's user id (`kc_federated_user_id`) for AC-7.3.

New step phrases (8, all in `feature_keycloak_sso_provisioning.rs`, under the "US-07"
banner; no collision, since the 23-example run reported no ambiguity):
`the newcomer has filed an issue and commented on it`;
`the identity provider now grants the newcomer only the "…" realm role`;
`the newcomer has been turned away through the identity provider`;
`the newcomer has chosen the password "…" through a reset`;
`the newcomer was given an account while provisioning was still switched on`;
`the newcomer comes back to the board in the session they already have`;
`the newcomer's name and the work they authored are unchanged`;
`the newcomer is signed in as the same account they were given`.
Step reuse across the file is 139 step lines over 34 phrases ≈ **4.1×** (informational).

### [REF] Test placement

| Layer | Where | Precedent |
|---|---|---|
| Acceptance (layer 3) | `keycloak-sso-provisioning.feature` scenarios 11–18 | Same file, harness and double as D3a. US-07 is a D3a addendum on the same driving port |
| Unit (layer 1) | `foundry-app/src/oidc.rs` `#[cfg(test)] mod tests` | `judge_newcomer`'s table test sits there. DDD-29 makes `judge_returning` its sibling |
| Store integration | `foundry-store/tests/users_provisioned_at.rs` (new file; WHY-NEW-FILE header) | `nullable_password_hash.rs` (0016), `instance_admins_migration.rs` (`run_migrations_from_dir`) |

### [REF] Driving-port coverage

| Port | Scenarios |
|---|---|
| `GET /auth/oidc/start` + `GET /auth/oidc/callback` (CHANGED, DDD-28) | 11, 12, 13, 14 (refuse arm 1d); 13 (1c); 17 (1a); 18 (1b) |
| `POST /sign-in` (unchanged, OD-12) | 15; baseline for every byte-identical refusal |
| `POST /forgot-password` + reset link (unchanged, DDD-27) | 14, 15 (Given) |
| Board `GET /` with an existing session (D10) | 16 |
| Issue + comment posts (authorship, D8) | 11 (Given) |

Driven: `users` lookup/provisioning is real Postgres in every scenario. The roles in
the ID token come from the shipped fake issuer (policy row from D3a; no new row).
`Clock` is not observed at acceptance; `provisioned_at` is pinned at store level.

### [REF] Named faults DELIVER must kill

| Fault | Killed by |
|---|---|
| Role check skipped for provisioned accounts (step 1 still links unconditionally) | 11, 12a/b, 13, 14; unit rows 8–10 |
| Check applied to non-provisioned accounts (invited/linked refused without role) | 17; unit rows 1–3 |
| Role unset treated as refuse for provisioned accounts | 18; unit rows 4–5 |
| Role match not exact (case-insensitive, or client roles count) | 12b (`Foundry-User`); unit row 10 |
| Role held but still refused (re-grant does not restore) / a second account created | 13; unit rows 6–7 |
| Marker cleared by reset or password change | 14; store `no_password_write_clears_the_provisioned_marker` |
| Refusal reason leaks to the response / response differs from wrong password | 11, 12, 14 (byte-identical, CSRF masked) |
| Reason reuses `LACKS_PROVISION_ROLE` (operator cannot tell withdrawn from stranger) | unit `assert_ne!` |
| Refusal deletes or alters account, membership, name, issues or comments | 11's four Thens |
| Live session ended by the change | 16 |
| Password door gated on the role | 15 |
| Backfill marks password accounts, or uses `now()` instead of `created_at` | store OQ-6 test |
| DDD-26 `OR password_hash IS NULL` dropped (window account reads as invited) | store OQ-7 test (window row, by email and by id) |
| Marker not written by the provisioning INSERT | store DDD-25/27 test; 14 end-to-end |

### [REF] Pre-requisites

- D3a as shipped (0016, `provision_federated_member`, DDD-15 order) — present.
- DELIVER builds: migration 0017 (DDD-24), the `provision_federated_member` `now`
  parameter (DDD-25; this changes the call in step
  `the newcomer was given an account while provisioning was still switched on`
  and in the DDD-25/27 store test), `UserRow.provisioned` (DDD-26), the `probe`
  column assertion (DDD-32), and `judge_returning` plus its const and the
  callback split (DDD-28..30).
- Lanes after DISTILL (with `@pending` restored): `FOUNDRY_ACCEPTANCE_TAGS=keycloak-sso`
  → 37/37. That is the former 38 minus scenario 11, which is now `@pending`;
  nothing new runs. `keycloak-sso-provisioning` → 14/14 examples (scenarios 1–10);
  11–18 skipped. `cargo clippy -p foundry-acceptance -p foundry-app -p foundry-store
  --all-targets -D warnings`, `cargo fmt --check` and `cargo xtask check-arch` are
  all clean. `foundry-store --test users_provisioned_at`: 3 ignored.

### Open items for DELIVER / the orchestrator

- **Un-pend 11–18 together** once the callback split lands, with 15–18 as guards.
  Un-ignore the four scaffolds as their APIs appear and delete each shim.
- **DDD-32 probe test** (a schema missing `provisioned_at` refuses `/readyz`) is not
  scaffolded here. It follows the shipped `probe_schema_scoping.rs` pattern and is
  DELIVER's to add alongside the probe edit.
- **OQ-8** (a check-arch scanner forbidding an `UPDATE` of `provisioned_at`) is
  optional and left to DELIVER. The store test above is the floor.
- **D11's "renamed role" case** (provisioned under one role name, checked against a
  renamed one) has no acceptance scenario, because the harness cannot restart foundry
  over the same schema with a new config. It is covered structurally:
  `judge_returning` takes the role read at sign-in, and unit rows 8–10 vary the held
  set against the configured name.
- **AC-6.8 and DDD-22** must still be marked superseded by dated text (DoD 6), and
  `CHANGELOG.md` updated (DDD-33). Both are DELIVER's job.
- The end-of-DISTILL consolidated four-reviewer gate is run by the orchestrator.

### [REF] End-of-DISTILL consolidated review (2026-10-04, OD-10 increment)

Three reviewers ran in parallel. No DEVOPS wave ran, so there is no platform reviewer;
this follows the card-pointer-drag and board-lane-reorder precedent.

| Reviewer | Wave | Verdict |
|---|---|---|
| nw-product-owner-reviewer | DISCUSS (US-07, AC-7.1..7.9, slice-04) | APPROVED: DoR 9/9; every AC-7.x mapped (AC-7.6 at unit level); 0 antipatterns |
| nw-solution-architect-reviewer | DESIGN (DDD-23..33) | APPROVED: check placement, DDD-26 OR safety (every non-provisioning INSERT binds a hash), 0017 rolling-deploy and rollback safety, D7 preserved |
| nw-acceptance-designer-reviewer | DISTILL (scenarios 11–18, 4 scaffolds) | APPROVED: coverage, scenario 11 rewrite, RED classification, oracles, scaffolds compile and are ignored |

**Findings:** 0 blockers, 0 high, 0 medium. The DISTILL reviewer called scenario 18 RED.
DISTILL's own gate run measured it GREEN_ALREADY, as a guard, and the measured
classification stands.

## Wave: DELIVER

> **OD-10 increment (US-07, D3b), 2026-10-04.** Apex (@nw-platform-architect), DELIVER
> finalize. This section covers roadmap phase 04 only (steps 04-01..04-03, approved
> `0ea878f`) and supersedes nothing above; the earlier DELIVER sections stay as
> written, including their "OD-10: OPEN" lines. **US-07 is DELIVERED.** **Sources:**
> `deliver/roadmap.json` (phase 04), `deliver/execution-log.json` and the step
> commits. Evolution archive: `docs/evolution/2026-09-27-keycloak-sso.md`
> § "2026-10-04 increment: OD-10 / US-07".

### [REF] Implementation Summary

A provisioned account must now hold the provision realm role at every Keycloak
sign-in (D3b). Without it, the sign-in gets the shipped generic refusal. Three
roadmap steps, run in order, all GREEN; one harness fix, one test tightening and one
refactor followed.

- **04-01 `fdf3346` (store).** Migration `0017_users_provisioned_at.sql` adds
  `users.provisioned_at TIMESTAMPTZ NULL` and backfills `provisioned_at = created_at`
  onto every password-less row (DDD-23/24, OD-14). `UserRow.provisioned` is computed
  in SQL as `provisioned_at IS NOT NULL OR password_hash IS NULL` from one shared
  projection behind `find_user_by_email` and `find_user_by_id` (DDD-26).
  `provision_federated_member` takes the caller's `now` and writes it in the
  provisioning INSERT only; the `ON CONFLICT` re-read writes nothing (DDD-25/27).
  `Store::probe` refuses a schema without `users.provisioned_at` (DDD-32), with a
  probe test in `probe_schema_scoping.rs`. The four DISTILL store scaffolds are
  un-ignored and their shims gone.
- **04-02 `25c6ab3` (oidc).** Callback step (1) splits per DDD-28. The pure
  `judge_returning(provisioned, provision_role, identity)` decides for an existing
  account: not provisioned links (1a), provisioned with the role unset links (1b,
  OD-13), provisioned with the role held exactly links (1c), otherwise refuse (1d)
  through the single `refuse()` with the new reason
  `PROVISIONED_LACKS_PROVISION_ROLE` = `provisioned account lacks provision role`,
  distinct from `LACKS_PROVISION_ROLE` (DDD-29/30, D11, D12). Earlier checks keep their
  order; steps (2)–(5) and the `Existing` outcome are unchanged. The unit scaffold is
  un-ignored (10 rows plus `assert_ne!`). Scenarios 11–14 and 17–18 un-pended.
- **24a209d (harness fix, test code only).** 04-01 regressed 4 default-lane scenarios
  in `us-mwt-slice-05`: that harness stops the schema at 0011 to prove the upgrade
  byte-for-byte, then read through the current `Store` sign-in API, which now
  projects `provisioned_at` → Postgres 42703. Fix: upgrade through the latest
  migration before any current-Store read. The 0011 byte-for-byte proofs are
  unchanged. Production is unaffected (`Store::probe` refuses a pre-0017 schema).
- **04-03 `44262c8`.** Guards 15 and 16 un-pended; the feature has no `@pending`
  line. `CHANGELOG.md` `[Unreleased]` gains a `### Changed` entry (both accepted
  residues, OD-12 and OD-13, in plain words) and `### Migration notes` for 0017,
  including the OQ-10 check (DDD-33). AC-6.8 and DDD-22 marked superseded with dated
  text, original wording intact (DoD 6).
- **`a901f19` (test).** `no_password_write_clears_the_provisioned_marker` now
  provisions at a fixed instant and asserts the stored marker equals it (truncated
  to microseconds), so an INSERT using SQL `now()` fails. Closes a gap 04-01
  reported.
- **`182b0a8` (refactor, L1–L2, behaviour unchanged).** One migration-copy loop in
  `test_migration.rs` (3 → 2 loops); one 0017-presence guard in
  `users_provisioned_at.rs` (3 → 1); truthful reason doc in `oidc.rs` and US-07 step
  header. Left as-is, with reasons: `judge_newcomer` / `judge_returning` (merging
  would blur two reasons and the DDD-28 order), the `probe` counts, migration 0017.

**Open decisions at close:**
- **OD-10** — RESOLVED 2026-10-04 (D3b) and DELIVERED: scenario 11 now pins the
  refusal (AC-7.1/AC-7.2).
- **OD-12, OD-13** — residues accepted by the user and stated in `CHANGELOG.md`.
- **OQ-8** (check-arch rule against an `UPDATE` of `provisioned_at`) — not done;
  optional. The store test is the floor.
- **OQ-10** (operator: did production ever set `FOUNDRY_OIDC_PROVISION_ROLE`?) — open;
  the 0017 migration note tells operators to check.

### [REF] Files modified

| File | Change |
|---|---|
| `crates/foundry-store/migrations/0017_users_provisioned_at.sql` | NEW: ADD COLUMN + OD-14 backfill (04-01) |
| `crates/foundry-store/src/lib.rs` | `UserRow.provisioned`, shared projection, `provision_federated_member(now)`, `probe` column check (04-01); return-direct reads (182b0a8) |
| `crates/foundry-store/tests/users_provisioned_at.rs` | Three scaffolds live (04-01); clock pinned (a901f19); one guard helper (182b0a8) |
| `crates/foundry-store/tests/probe_schema_scoping.rs` | DDD-32 probe test (04-01) |
| `crates/foundry-store/tests/provision_federated_member.rs` | `now` argument (04-01) |
| `crates/foundry-app/src/oidc.rs` | `state.clock.now()` to provisioning (04-01); `judge_returning`, new const, callback step-1 split, unit table live (04-02); reason doc (182b0a8) |
| `crates/foundry-acceptance/src/steps/feature_keycloak_sso_provisioning.rs` | Scenario-18 Given passes the harness clock (04-01); step header (182b0a8) |
| `crates/foundry-acceptance/tests/features/keycloak-sso-provisioning.feature` | `@pending` removed from 11–18 (04-02, 04-03); Gherkin otherwise DISTILL's |
| `crates/foundry-acceptance/src/steps/feature_mwt_slice_05_migration_guarantee.rs` | Upgrade to latest before current-Store reads (24a209d) |
| `crates/foundry-acceptance/src/support/test_migration.rs` | Post-0011 staging, `copy_production_migrations_into` (24a209d); one copy loop (182b0a8) |
| `CHANGELOG.md` | `[Unreleased]` Changed + Migration notes (04-03) |
| `docs/feature/keycloak-sso/feature-delta.md` | AC-6.8 / DDD-22 superseded (04-03); this section |

### [REF] Scenarios green

| # | Scenario (AC) | Step | First run |
|---|---|---|---|
| 11 | Withdrawn role turned away, keeps everything (7.1, 7.2) | 04-02 | RED for the business reason ("a refusal must not establish a session") → GREEN |
| 12a, 12b | Only a different role: `some-other-role`, `Foundry-User` (7.1, D11) | 04-02 | RED, same reason → GREEN |
| 13 | Re-grant lets the same account back in (7.3) | 04-02 | RED in its composed Given → GREEN |
| 14 | Reset does not lift the refusal (7.5) | 04-02 | RED, same reason → GREEN |
| 17 | Invited member without the role links (7.4) | 04-02 | GREEN_ALREADY (guard, arm 1a) |
| 18 | Provisioning off: provisioned member links (7.9) | 04-02 | GREEN_ALREADY (guard, arm 1b) |
| 15 | Password door still open after withdrawal (7.8) | 04-03 | GREEN_ALREADY (guard) |
| 16 | Live session keeps working (7.7) | 04-03 | GREEN_ALREADY (guard) |

`keycloak-sso-provisioning.feature`: 18 scenarios / 23 examples, no `@pending`.
AC-7.6 is unit-level: `judge_returning` yields the new const, `assert_ne!` against
`LACKS_PROVISION_ROLE`.

### [REF] Per-step outcome

| Step | Lanes at the step |
|---|---|
| 04-01 `fdf3346` | foundry-store 82/82 (users_provisioned_at 3/3, provision_federated_member 4/4, probe_schema_scoping 2/2); keycloak-sso 37/37; provisioning 14/14; us-06 44/44; fmt, clippy `-D warnings`, check-arch pass. **Default lane not run** — see Lessons |
| 04-02 `25c6ab3` | foundry-app 79/79 (lib); provisioning 21/21 (15, 16 pending); keycloak-sso 44/44; us-06 44/44; fmt, clippy, check-arch pass |
| 04-03 (first GREEN) | provisioning 23/23; keycloak-sso 46/46; us-06 44/44; **default lane 681/685** — the 4 failures were 04-01's `provisioned_at` regression in `us-mwt-slice-05`. COMMIT blocked (`BLOCKED_BY_DEPENDENCY`), harness fix routed to acceptance-designer |
| `24a209d` | mwt-slice-05 6/6 (42/42 steps); default lane run twice, 684/685 each — one different scenario each run (navigation-bar, then member-invites), the known sqlx `'\0'` flake, both features green alone |
| 04-03 `44262c8` | As above, GREEN; committed |
| `a901f19`, `182b0a8` | foundry-store 82/82; foundry-app 89/89 (lib 79/79); keycloak-sso 46/46; provisioning 23/23; us-06 44/44; mwt-slice-05 6/6; us-blm-01 6/6; fmt, clippy, check-arch pass |

### [REF] DoD check (DISCUSS OD-10 DoD, 2026-10-04)

| # | DoD item | Status | Evidence |
|---|---|---|---|
| 1 | AC-7.1..7.7 green at layer 3; AC-7.6 at unit level | MET | Scenarios 11–14, 16, 17 (layer 3); unit table + `assert_ne!` in `oidc.rs` |
| 2 | AC-7.8 and AC-7.9 green as confirmed | MET | Scenarios 15, 18 (OD-12, OD-13 confirmed by the user) |
| 3 | Scenario 11 rewritten, not deleted; 1–10 and the 23 base scenarios unchanged and green | MET | keycloak-sso 46/46 (23 base + 23 provisioning examples) |
| 4 | Pre-change accounts marked by the OD-14 backfill rule, with its own test | MET | `users_provisioned_at.rs` upgrade test (0016 → 0017, mixed rows, re-run idempotent) |
| 5 | `cargo xtask ci` green with Docker; per-feature mutation ≥ 80% over changed files | **MET** | CI 911/911 on `182b0a8`. Mutation: 12/12 viable mutants killed (100%) |
| 6 | AC-6.8 and DDD-22 superseded by dated text; `CHANGELOG.md` updated | MET | `44262c8` |

### [REF] Demo evidence

Auth/HTTP feature with no new UI: the demonstration is the layer-3 scenario runs
(real axum via `build_router`, real Postgres, the shipped `OidcProvider` against the
RS256 issuer double), not a live walk-through. The Elevator Pitch maps to scenario 11
(Nia withdrawn → byte-identical refusal, account and work intact) and scenario 13
(re-grant → same user id, one account). Pat's control case is scenario 17. A sign-in
against the real cluster Keycloak was not run; production is on v0.6.0.

### [REF] Quality gates

**Named faults: 20/20 killed in phase 04**, each seeded alone, restored and
`cmp`-verified; plus the marker-clock fault, killed after `a901f19`.

| Step | Killed | Faults |
|---|---|---|
| 04-01 | 7/7 | Backfill uses `now()`; backfill marks password accounts; DDD-26 `OR` arm dropped; marker not written on insert; reset clears marker; password change clears marker; probe silent pre-0017 |
| 04-02 | 11/11 | Check skipped at the call site; check applied to non-provisioned; role unset refuses; case-insensitive match; held role still refused; reason leaked as a 403; reason reuses `LACKS_PROVISION_ROLE`; refusal renames / drops membership / deletes comments; reset clears `provisioned_at`. F7 killed by the unit test only |
| 04-03 | 2/2 | Password door gated on the role (15, and 10); live session ended for a provisioned account (16, and others) |
| `a901f19` | 1/1 | Provisioning INSERT uses SQL `now()` instead of the caller's clock (survived 04-01's store test) |

- **Mutation (cargo-mutants, per-feature ≥ 80%):** **PASS**. cargo-mutants 25.3.1 `--in-diff` over the phase-04 diff of `oidc.rs` and the store's `lib.rs`: 16 generated, 4 unviable, 12 viable, **12/12 killed (100%)**. The package tests kill 10/12 (83.3%) and the acceptance re-check kills the 2 whole-function handler mutants. See `deliver/mutation/mutation-report-od10.md`.
- **Adversarial review:** **APPROVED**, no defects (nw-software-crafter-reviewer, 2026-10-04)
- **Integrity:** `des-verify-integrity`: all 13 steps complete.
- **Full CI gate** (`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on `182b0a8`):
  **GREEN**: exit 0, all gates, 911/911 scenarios and 6339/6339 steps, browser lane run (2026-10-04)

### [REF] Pre-requisites

- D3a as shipped: migration `0016`, `provision_federated_member`, the DDD-15 order.
- Phase 03 (v0.6.1): the base round-trip harness the provisioning lane shares.
- `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false` for release builds on this host;
  a warm acceptance binary; every lane run bounded with `timeout`.
- **Before release (operator, OQ-10):** check whether production ever set
  `FOUNDRY_OIDC_PROVISION_ROLE`. If not, 0017's backfill marks zero rows. Production
  is on v0.6.0, unrelated to this increment.

**No migration of artifacts.** Single `feature-delta.md` layout; nothing to move.
Outcome rows stay unregistered: no keycloak-sso entry exists in
`docs/product/kpi-contracts.yaml` or `docs/product/outcomes/registry.yaml`.

## Wave: DELIVER / [REF] OQ-8 addendum (2026-10-04)

**OQ-8 — RESOLVED** in phase 05, step 05-01 (`87d1ee8`). `cargo xtask check-arch`
now carries the `provisioned-marker` rule. It fails the build, naming `file:line`
and D9, when any SQL `UPDATE` assigns `users.provisioned_at` anywhere except
`crates/foundry-store/migrations/0017_users_provisioned_at.sql`:

- **Scanned:** string literals in `.rs` files under `crates/`, excluding each
  crate's `tests/`, and every `*.sql` migration.
- **Not flagged:** comments; the provisioning INSERT; reads such as `IS NOT NULL`
  and `WHERE` filters.
- **Wider than asked:** `ON CONFLICT DO UPDATE SET provisioned_at` is flagged too.

D9 is now enforced structurally as well as by DDD-27's store tests.

**Tests:** 9 gold tests, with the aggregation test extended; `cargo test -p xtask`
43/43; check-arch PASSED on the tree.

**Seeded faults, 4 of 4 killed:** exemption widened, comment stripping removed,
multi-line matching removed, rule not wired in.

**Known limit:** an UPDATE split across separately concatenated Rust literals is not
matched (scope: one SQL string).

`des-verify-integrity`: all 14 steps complete.

### [REF] OQ-10 — RESOLVED (2026-10-04, checked on production)

Production does set the provision role: the `foundry` ConfigMap on the k3s-canzan-core
cluster (`foundry.jeffbailey.us`) has `FOUNDRY_OIDC_PROVISION_ROLE = foundry-member`, wired
by the homelab Terraform. **Provisioning never created an account there**, so migration
0017's backfill marked zero rows. Evidence, from a read-only query on CNPG `pg-1`, database
`foundry`, migrations through 17 applied:

- 1 user in total, with a password and `provisioned_at IS NULL`.
- That user was created 2026-08-22, before provisioning existed (D3a shipped in v0.6.0 on
  2026-10-03), so it cannot have been provisioned and then reset.
- 0 password-less accounts, 0 accounts marked provisioned.
- 0 `oidc identity provisioned` log lines (the pod started 2026-10-04 20:22Z, so the log
  window is short; the database facts above are conclusive regardless).

From now on, any account created by Keycloak sign-in on production is provisioned, and needs
`foundry-member` at every Keycloak sign-in (D3b).
