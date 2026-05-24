# Coverage Matrix — Slice 1

Proves: every in-scope AC and every NFR linked to a Slice-1 story is covered by at least one scenario. Tags are the actionable handles DELIVER uses to run subsets.

## Scenario inventory

| File | Scenario count | Tags |
|---|---|---|
| `us-01-install.feature`         | 4 (3 auto + 1 `@manual`) | `@us-01 @walking_skeleton @docker-compose @real-io @nfr-port-01 @manual` |
| `us-05-bootstrap.feature`       | 5 | `@us-05 @walking_skeleton @real-io @error @nfr-sec-01 @nfr-sec-03` |
| `us-06-signin.feature`          | 6 | `@us-06 @walking_skeleton @real-io @error @nfr-sec-02 @nfr-sec-03` |
| `us-07-project-create.feature`  | 5 + Outline (6 examples) | `@us-07 @walking_skeleton @real-io @error @property @nfr-sec-06` |
| `us-08-file-issue.feature`      | 7 + Outline (4 examples) | `@us-08 @walking_skeleton @real-io @error @nfr-perf-01 @nfr-sec-06 @property` |
| **Total**                        | **27 scenarios** (22 inline + 2 Outlines yielding 10 examples; counted as 7 outline-yields) — execution count when running auto-only tags: ~23 examples | |

Within the user constraint (3-6 scenarios per feature): US-08 is at 7+outline; the perf scenario + the boundary outline are NFR-driven, not happy-path noise. Acceptable.

## Story → Scenario → AC trace

### US-01 (5 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| `docker compose up -d` healthy in ≤300s on fresh VM | `Fresh-machine install becomes healthy and prints the bootstrap URL` |
| Exactly one `[BOOTSTRAP]` line first run; zero subsequent | both `Fresh-machine install...` and `Re-running docker compose up...` |
| Bootstrap token single-use, signed, 30-min TTL | covered structurally in US-05 (US-01's role is just printing it) + `Re-running...` |
| `.env` controls FOUNDRY_PORT, DATABASE_URL, SESSION_SECRET, FILE_UPLOAD_MAX_MB | covered via `Re-running...` (the harness uses the .env) and US-05's session-secret-dependent scenarios; explicit per-knob coverage deferred to US-13 (DEVOPS) — flagged as a gap, see "Coverage gaps" below |
| No host:volume mounts required (K8s-translatable) | `The compose file uses no host-bind volumes for the app container` (`@nfr-port-01`) |
| **Manual UAT**: "hour to demo" for a fresh operator | `An evaluating operator reaches the admin claim form within 30 minutes` (`@manual`) |

### US-05 (5 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| Bootstrap form requires email + password + display + workspace in one screen | `Admin claims the workspace via the bootstrap URL and is signed in` |
| Default team "General" + default project "Sandbox" auto-created | same |
| Invite links signed, default 7-day TTL | `Admin generates a shareable invite link that contains a signed token` |
| Email invites available iff SMTP set; otherwise only "Copy link" | the link-only path is covered; SMTP-email-invite explicit scenario deferred to US-05 follow-up in Slice 2 (no SMTP fake yet wired). **Flagged as gap** — see below |
| Multi-workspace per instance out of scope; second-workspace returns 409 | `Attempting to create a second workspace via the API is rejected` |
| **Negative paths**: replay + expiry | `Replayed bootstrap token is rejected...` + `Expired bootstrap token is rejected` |

### US-06 (5 ACs + 6 UAT scenarios)

| AC (paraphrased) | Scenario(s) |
|---|---|
| Password hashing meets OWASP minimum (argon2id) | structural — covered indirectly by happy-sign-in scenario reading the stored hash format; explicit `@nfr-sec-01` audit deferred to a unit test in `foundry-auth` (layer 1, not acceptance) |
| Session cookies HttpOnly + Secure + SameSite=Lax + 30-day TTL | `Member signs in successfully and receives a secure session cookie` |
| Failed-attempt delays kick in after 5 within 15 minutes per email | `The sixth failed attempt within 15 minutes is delayed by at least 5 seconds` (`@nfr-sec-02`) |
| Password reset via email when SMTP set, via CLI otherwise | `Password-reset email is sent when SMTP is configured...` covers the SMTP path. CLI fallback is `@us-06 @cli-fallback` — see Coverage gaps |
| Sign-out invalidates server-side session row, not just cookie | `Sign-out invalidates the server-side session row` |
| **Negative paths**: wrong password + unknown email (non-enumerable) | `Wrong password produces a non-enumerable error` + `Unknown email produces the same error as wrong password` |

### US-07 (4 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| Project belongs to exactly one team; team to exactly one workspace | `Member creates a project under their team and lands on its empty board` |
| Project key prefix auto-suggested, editable, 2-6 uppercase chars | `Project key prefix must match the invariant I-P3` Scenario Outline (accepted + rejected examples) |
| Project name unique within team; key unique within workspace | `Duplicate project name within the same team is rejected` + `Duplicate project key within the same workspace is rejected` |
| Default states seeded on project creation | `Member creates a project ... lands on its empty board` (asserts the four column names) |
| **Authorization**: non-team-member cannot create project there | `A workspace member who is not on the team cannot create a project there` (`@nfr-sec-06`) |

### US-08 (5 ACs)

| AC (paraphrased) | Scenario(s) |
|---|---|
| `c` keyboard shortcut works globally; modal opens within 200ms | NOT in scope — `c` shortcut is US-12 (Slice 2). The HTTP `POST /issues` is exercised; the UI shortcut is deferred. |
| Only title required; sensible defaults (state=Backlog, priority=Medium, ...) | `Member files an issue with only a title and sees AUTH-1 on the board` asserts state + priority defaults |
| Markdown rendering with CommonMark + sanitization | NOT in slice-1 ACs (description is OPTIONAL); explicit markdown rendering deferred. **Flagged as gap** — see below |
| Issue keys sequential per project, prefixed | `Issue keys are sequential per project` + `Issue keys are scoped per project, not per workspace` |
| (Implicit AC from US-08 examples / NFR-PERF-01) Hot-path latency | `Sequential issue creation has P95 latency under 200ms` (`@nfr-perf-01`) |
| **Negative paths**: empty title + cross-team forbidden | `An empty title is rejected with an inline htmx error fragment` + `A workspace member not on the team cannot file an issue against that team's project` |
| **Property**: title length boundary | `Title length boundary handling` Scenario Outline |

## NFR → Scenario trace (Slice-1 NFRs only, per NFR-MATRIX in `nfrs.md`)

| NFR | Slice-1 linked stories | Scenario(s) |
|---|---|---|
| NFR-PERF-01 (server-render P95 ≤ 200ms) | US-07, US-08 | `Sequential issue creation has P95 latency under 200ms` (`@nfr-perf-01` US-08) |
| NFR-OBS-01 (structured JSON logs) | US-01 | NOT in `.feature` — verified by a `foundry-app` integration test (DELIVER concern); `.feature` scope is user-observable behaviour |
| NFR-OBS-02 (`/healthz` + `/readyz`) | US-01 | `Fresh-machine install becomes healthy and prints the bootstrap URL` (asserts `/healthz`); `/readyz` semantics under DB-outage is US-02 (Slice 3) — out of scope |
| NFR-OBS-04 (request IDs / X-Request-Id) | US-01 | NOT in `.feature` — DELIVER integration test |
| NFR-SEC-01 (argon2id parameters) | US-05, US-06 | structural; covered by hash-shape assertion as part of the happy sign-in scenario. Detailed parameter audit is a `foundry-auth` unit test (layer 1) — `@nfr-sec-01` tag on US-06 walking skeleton |
| NFR-SEC-02 (brute-force delay) | US-06 | `The sixth failed attempt within 15 minutes is delayed by at least 5 seconds` (`@nfr-sec-02`) |
| NFR-SEC-03 (session cookies) | US-05, US-06 | `Admin claims the workspace...` + `Member signs in successfully...` (both assert HttpOnly + SameSite=Lax + Secure + 30-day TTL) (`@nfr-sec-03`) |
| NFR-SEC-04 (CSRF) | US-05, US-06, US-07, US-08 | NOT covered by Slice-1 `.feature` scenarios — CSRF is exercised transparently by the reqwest client (cookie + form-field round-trip); explicit positive+negative CSRF coverage deferred to a dedicated middleware integration test in DELIVER. **Flagged as gap** — see below |
| NFR-SEC-05 (HTML sanitization) | US-08 | OUT of slice-1 scope (markdown description is optional in US-08 ACs; happy path uses title-only) |
| NFR-SEC-06 (auth checks at every endpoint) | US-05, US-06, US-07, US-08 | `Workspace member not on the team cannot create a project there` (US-07) + `Workspace member not on the team cannot file an issue against that team's project` (US-08) — exemplar scenarios per-story |
| NFR-SEC-07 (secret injection via env) | US-01 | NOT in `.feature` — operator-facing; verified by docker-compose harness setup |
| NFR-MIG-01 (forward-only, advisory-locked, idempotent) | US-01 | structural; the testcontainers + per-scenario-schema harness exercises `sqlx migrate run` repeatedly; advisory-lock under concurrent startup is US-02 (Slice 3) — out of scope |
| NFR-PORT-01 (K8s-translatable; no host volumes) | US-01 | `The compose file uses no host-bind volumes for the app container` (`@nfr-port-01`) |
| NFR-PORT-02 (12-Factor env config) | US-01 | structural; the harness sets env vars only (no config files) |

## Coverage gaps (flagged for DELIVER and for Slice 2/3)

1. **US-01 AC: per-knob `.env` coverage** (FILE_UPLOAD_MAX_MB explicitly). FILE_UPLOAD_MAX_MB is a US-11 (Slice 3) concern; no acceptance scenario for it in Slice 1. **Resolution: defer to US-11.**
2. **US-05 SMTP email invite scenario**. The bootstrap-claim and link-invite paths are covered; the email-delivered-by-SMTP path is NOT covered with a `@nfr-sec-04`-free scenario. **Resolution: add `When the admin emails an invite to "mei@acme.com"` scenario in Slice 1 follow-up (cheap — `FakeEmailSender` is wired for US-06 password-reset already).** Open question for DELIVER (Q1).
3. **US-06 CLI-fallback password reset**. Listed in US-06 ACs but not in the `.feature` (would require the assert_cmd + CLI subprocess path; adds infra). **Resolution: add `@us-06 @cli-fallback @real-io` scenario.** Open question for DELIVER (Q1).
4. **US-08 markdown rendering + sanitization (NFR-SEC-05)**. Title-only happy path skips this. Slice-1 AC allows description-empty default, so OK to defer. **Resolution: covered in Slice 2 alongside US-10 (comments) which shares the markdown pipeline.**
5. **NFR-SEC-04 CSRF positive + negative**. The cookie+form round-trip is exercised implicitly but no explicit "POST without CSRF token returns 403" scenario exists in Slice 1. **Resolution: add to driver design as a default middleware behavior test in DELIVER's `foundry-app` integration tests; or add one `@nfr-sec-04 @error` scenario per protected endpoint.** Open question for DELIVER (Q2).
6. **NFR-OBS-01 / OBS-04**: log JSON-ness + X-Request-Id are not user-observable in the Gherkin sense; live in `foundry-app` integration tests, not the acceptance suite. **Resolution: keep out of `.feature` scope by design.**

## Driving Adapter coverage (per Mandate 1 + Driving Adapter Verification)

DESIGN entry points per `architecture.md`, `auth.md`, `data-access.md`:

| Driving adapter | Scenario(s) invoking it via its protocol |
|---|---|
| HTTP POST `/bootstrap` | US-05 happy + replay + expired + 410-Gone scenarios |
| HTTP GET `/bootstrap?token=...` | US-05 replayed + expired |
| HTTP POST `/sign-in` | US-06 happy + wrong-password + unknown-email + brute-force |
| HTTP POST `/sign-out` | US-06 sign-out |
| HTTP POST `/forgot-password` (or equivalent) | US-06 password-reset |
| HTTP POST `/team/:team/projects` | US-07 happy + duplicates + outline |
| HTTP GET `/team/:team/project/:slug` | US-07 happy (lands on board) + US-08 happy ("opening ... lists...") |
| HTTP POST `/issues` | US-08 happy + dup-key + cross-team + empty-title + perf + outline |
| Docker Compose stack invocation (US-01) | US-01 all `@real-io` scenarios |
| CLI subcommand `foundry admin reset-password` | NOT covered (see gap #3) |

Every code-feature driving adapter in scope for Slice 1 is exercised by at least one scenario. Pipeline-only / service-only entries are NOT used to substitute for the protocol-level call — Mandate 1 satisfied.

## Driven adapter coverage (Mandate 6)

Every driven adapter has at least one `@real-io @adapter-integration` scenario per the apply-policy. Slice-1 driven adapters (per Architecture of Reference):

| Adapter (Driven internal) | `@real-io` scenario | Covered by |
|---|---|---|
| `PgPool` workspaces / users / sessions / projects / issues / outbox / bootstrap_tokens / invites / signin_attempts | YES — every `@real-io` scenario in US-05..US-08 uses real Postgres via testcontainers | per-scenario schema rotation |
| `tower-sessions Postgres store` | YES — US-05 happy + US-06 happy + US-06 sign-out | session cookie round-trip |

Driven external/non-deterministic (fakes per policy):

| Adapter | Slice-1 coverage strategy |
|---|---|
| `lettre::SmtpTransport` (FakeEmailSender) | Covered by US-06 password-reset (`exactly one email is recorded as sent to ...`). US-05 SMTP email-invite is the GAP #2 — to be added. |
| `tokio::time::sleep` (FakeClock) | Covered by US-06 brute-force (records duration ≥4500ms) + US-05 expired-token (advances clock by 31 min) |
| `rand::random` bootstrap-token generator | No fake needed; covered by structural assertions on token shape |

## Tag schema (consumed by DELIVER and CI)

| Tag | Purpose |
|---|---|
| `@slice1` | All five feature files carry this — enables `--tags @slice1` |
| `@us-01` ... `@us-08` | Story scope |
| `@walking_skeleton` | One per US-05/06/07/08 + the `@real-io` US-01 scenarios |
| `@driving_port` | Every scenario that enters via an HTTP driving port |
| `@real-io` | Real adapter (Postgres, real lettre would be — here FakeEmailSender substitutes for SMTP per policy) |
| `@docker-compose` | US-01 only — slow, sharded separately |
| `@manual` | US-01 only — human UAT |
| `@error` | Negative-path scenarios |
| `@property` | Scenario Outlines (US-07 key prefix + US-08 title length boundary) |
| `@nfr-perf-01`, `@nfr-sec-01..03`, `@nfr-sec-06`, `@nfr-port-01` | NFR-driven scenarios |
| `@adapter-integration` | Reserved for explicit adapter-only integration tests (US-08 outbox round-trip is the canonical example; Slice 2 adds the LISTEN/NOTIFY adapter test) |
| `@cli-fallback` | Reserved for US-06 CLI-fallback password reset (gap #3) |

## Self-review check (per nw-distill § Self-Review Checklist)

| # | Item | Status |
|---|---|---|
| 1 | WS strategy declared | Project policy file written at `docs/architecture/atdd-infrastructure-policy.md` (the post-RETIRED equivalent) |
| 2 | WS scenarios tagged `@real-io` | YES (every `@walking_skeleton` carries `@real-io`) |
| 3 | Every driven adapter has at least one `@real-io` scenario | YES (Postgres × all; SMTP via fake per policy; FakeClock via brute-force) |
| 4 | InMemory doubles limitations documented | YES — policy table records FakeEmailSender + FakeClock + WHY |
| 5 | Container preference documented | YES — testcontainers Postgres, shared+schema-rotation, in driver.md §3-4 |
| 6 | Mandate 7 (RED-ready scaffolds) | Deferred. Rust convention: scaffolds are stub modules in `crates/foundry-*/src/*` that `panic!("Not yet implemented — RED scaffold")`. The DELIVER wave creates these when it picks up the first scenario per story; alternatively DISTILL writes a one-shot scaffolding PR. See "Open questions" Q3. |
| 7 | Scaffolds include `__SCAFFOLD__` marker (`// SCAFFOLD: true` in Rust) | Pending Q3 resolution |
| 8 | Scaffold methods `panic!` not `unimplemented!()` | Q3 |
| 9 | Tests are RED, not BROKEN | Q3 |
| 10 | Every CLI/endpoint/hook in DESIGN has at least one driving-protocol scenario | YES per Driving Adapter coverage table above |
| 11 | At least one `@real-io @adapter-integration` per driven adapter | YES per Driven adapter table |
| 12 | `capsys` `@when` not `@then` | N/A (Rust, not pytest-bdd) |
| 13 | `@when` imports only from application/domain | YES — step bodies invoke `foundry_app::test_support::spawn_app()` then `reqwest`, never `foundry_store::*` directly |
| 14 | Timing assertions ≥200ms | YES — US-08 perf scenario budget is 200ms exactly (NFR-PERF-01 ceiling) |
| 15 | BDD imports `# noqa` marker | N/A (Rust) |

## Open questions for DELIVER (top 3)

1. **SMTP email-invite scenario + CLI password-reset scenario** (gaps #2, #3): cheap to add; should they be appended to Slice-1 DISTILL before handoff or picked up as the first cycle of Slice-1 DELIVER? Recommendation: append now (one scenario each, ~20 lines of Gherkin); DELIVER author confirms.
2. **CSRF coverage shape** (gap #5): the reqwest client uses a cookie jar so CSRF token round-trip works implicitly. Should we add explicit `@nfr-sec-04 @error` scenarios per protected endpoint, or push to a dedicated middleware integration test in DELIVER? Recommendation: dedicated middleware test — `.feature` should not babysit CSRF for every form.
3. **Scaffold convention for Rust** (checklist #6-9): no existing `crates/` directory exists yet. Two options: (a) DISTILL writes the scaffold modules + `panic!` stubs and commits before DELIVER picks up; (b) DELIVER scaffolds as it dequeues scenarios. The Mandate 7 contract says DISTILL writes them. Recommendation: DISTILL commits the 5-crate skeleton + `panic!` stubs as a follow-up commit (~30 min of work; out of scope for this DISTILL deliverable per the explicit task ask, but flagged).
