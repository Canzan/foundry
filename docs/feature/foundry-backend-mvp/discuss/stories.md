<!-- markdownlint-disable MD024 -->
# Foundry Backend MVP — User Stories

## Scope Assessment

**Status**: PASS — right-sized MVP slice.

- 13 user stories grouped into 3 personas (Operator, Admin/User, Contributor).
- 1 bounded context (the monolith — single axum binary, single Postgres).
- Walking skeleton: US-01 + US-05 + US-06 + US-07 + US-08 (an operator can install, an admin can bootstrap, a user can sign in, create a project, and file an issue).
- Estimated effort: 8-12 weeks for a 2-Rust-developer team. Aligns with the recommendation's "ship something credible in 8-12 weeks" claim.
- Decisions locked in DIVERGE — no re-derivation: D1 Boring Monolith, AGPLv3, Postgres-for-everything, htmx 2.x + alpine.js, SSE, bytea uploads, OIDC deferred to v0.3.

## System Constraints (cross-cutting)

These apply to every story in this MVP. They are not duplicated into each story's AC; instead, NFRs in `nfrs.md` capture their measurable form.

- **Single binary**: One Rust `foundry` binary; no separate worker/scheduler containers in MVP.
- **Postgres-only state**: Sessions, queue (outbox), pubsub (LISTEN/NOTIFY), data, and file attachments all live in Postgres.
- **Stateless app tier**: Any replica must be able to serve any request; no sticky sessions; no in-memory session state.
- **AGPLv3 license**: All dependencies must be license-compatible (MIT, Apache-2.0, BSD-family, MPL-2.0 acceptable; AGPL or GPL transitively forbidden for non-app deps unless this very app remains AGPL).
- **No build-time secrets**: All secrets injected via env at container start; the Docker image is identical across deployments.
- **K8s-translatable**: docker-compose files use no host-bind-mount-only features that would block a future K8s manifest port.

---

## Glossary (Ubiquitous Language)

- **Workspace**: Top-level tenant boundary for a self-hosted Foundry instance. Usually one per company/team.
- **Team**: A sub-group within a workspace (e.g., "Backend", "Mobile") that owns projects and issues.
- **Project**: A named collection of issues belonging to a team.
- **Issue**: The unit of work — has title, markdown description, assignee, labels, state, priority, comments, and attachments.
- **State**: The workflow position of an issue (default: `backlog`, `todo`, `in-progress`, `done`, `cancelled`).
- **Operator**: The human running `docker compose up` and responsible for keeping Foundry alive on their infra.
- **Admin**: The first user in a workspace; can invite teammates and configure the workspace.
- **Member**: Any non-admin user belonging to a workspace.
- **Contributor**: A developer who clones the Foundry repo to modify Foundry itself.

---

## US-01: Operator installs Foundry in under an hour

- **job_id**: jtbd-outcome-1 (Minimize time to stand up a working issue tracker)

### Story

As a self-hosting **operator** (Devansh, an SRE at a 12-person SaaS startup), I want to spin up Foundry on a fresh VM with `docker compose up`, so that I can evaluate it against Linear before committing my team to the migration.

### Elevator Pitch

- **Before**: Devansh reads the README, edits a sample `.env`, and runs `docker compose up`.
- **After**: He browses to `http://localhost:3000`, sees the Foundry landing page, and the container logs print a one-line admin bootstrap URL with a token.
- **Decision enabled**: Devansh decides whether to demo Foundry to his team this afternoon (yes/no based on the first-hour experience).

### Problem

Devansh has 60 minutes to evaluate self-hosted issue trackers before his lunch meeting. He has Docker installed but no Rust toolchain, no Postgres knowledge beyond "it's a database", and no patience for a five-page bootstrapping doc. Most OSS trackers cost him 2-4 hours of debugging before he sees a UI — by which point he gave up and put Linear back on the agenda.

### Who

- **Devansh Iyer**, SRE at a Series-A startup. Comfortable with Docker, suspicious of YAML longer than 50 lines.
- **Context**: Fresh Ubuntu 22.04 VM with Docker 24+ installed. No prior Foundry knowledge.
- **Motivation**: Replace Linear before next renewal in Q3.

### Solution

A two-container docker-compose (foundry + postgres) that:

1. Pulls pre-built images from a public registry (or builds locally in <5 min for contributors).
2. Runs Postgres migrations idempotently on first startup.
3. Prints an admin bootstrap URL (signed, single-use, 30-minute TTL) to stdout.
4. Serves a landing page at port 3000 indicating "Foundry is running; visit /bootstrap to claim admin."

### Domain Examples

#### 1. Happy path — fresh VM, default config

Devansh on a fresh Ubuntu 22.04 VM at 14:02 runs:

```bash
git clone https://github.com/foundry-project/foundry.git
cd foundry
cp .env.example .env
docker compose up -d
```

At 14:04, `docker compose logs foundry` prints `[BOOTSTRAP] Visit http://localhost:3000/bootstrap?token=8f3a...`. He clicks it, sees the admin-claim form, and proceeds with US-05. **Total elapsed: 4 minutes.**

#### 2. Edge case — port 3000 already in use

Mei Chen has Grafana on port 3000 already. She edits `.env` to set `FOUNDRY_PORT=3030` and re-runs `docker compose up -d`. Foundry binds 3030, logs print `http://localhost:3030/bootstrap?token=...`, and there is no port conflict.

#### 3. Error path — Postgres image fails to pull

Hiroshi's corporate proxy blocks docker.io. `docker compose up` fails with `pull access denied`. The error is from Docker, not Foundry; the Foundry docs FAQ entry `Pulling images behind a proxy` explains how to retag images from a private registry. Foundry's own output is silent until images are available — no half-started state.

### UAT Scenarios

```gherkin
Scenario: Fresh-machine install completes in under five minutes
  Given Devansh has a fresh Ubuntu 22.04 VM with Docker 24+ installed
  And the Foundry repository is cloned to /home/devansh/foundry
  And the .env file is copied from .env.example with defaults
  When Devansh runs "docker compose up -d" from the repo root
  Then within 300 seconds the foundry container reports healthy on the /healthz endpoint
  And the postgres container reports healthy
  And no manual database initialization step is required

Scenario: Bootstrap URL is discoverable from logs
  Given the foundry container has started for the first time against an empty database
  When Devansh runs "docker compose logs foundry"
  Then the output contains a single line starting with "[BOOTSTRAP]"
  And the line contains a fully-qualified URL with a token query parameter
  And the URL is valid for 30 minutes from log emission

Scenario: Bootstrap URL is one-shot
  Given Devansh has visited the bootstrap URL and created the admin account
  When Devansh visits the same bootstrap URL again
  Then he sees a page stating "This bootstrap link has already been used"
  And the page links him to the regular sign-in flow

Scenario: Port override via environment variable
  Given Devansh sets FOUNDRY_PORT=3030 in .env
  When he runs "docker compose up -d"
  Then Foundry binds to port 3030 on the host
  And the bootstrap URL printed to logs uses port 3030

Scenario: Re-running docker compose up is idempotent
  Given Foundry is already running with an active workspace and 3 issues
  When Devansh runs "docker compose up -d" a second time
  Then no data is lost
  And no new bootstrap URL is printed (admin already exists)
  And all running container IDs are unchanged
```

### Acceptance Criteria

- [ ] `docker compose up -d` brings Foundry to a healthy state in under 300 seconds on a fresh VM.
- [ ] Container logs contain exactly one `[BOOTSTRAP]` line on first run, zero on subsequent runs.
- [ ] Bootstrap token is single-use, signed, and expires after 30 minutes.
- [ ] `.env` controls FOUNDRY_PORT, DATABASE_URL, SESSION_SECRET, FILE_UPLOAD_MAX_MB.
- [ ] No `host:` volume mounts required for app to function (K8s-translatable).

### Outcome KPIs

- **Who**: New self-hosting operators evaluating Foundry.
- **Does what**: Reach a working Foundry instance with admin URL visible.
- **By how much**: 80% of operators complete install in ≤10 minutes (P80 setup time).
- **Measured by**: Anonymous opt-in install telemetry (event: `bootstrap_url_visited` minus `compose_started`); fallback: GitHub issues tagged `install-failure` count.
- **Baseline**: 0 (no install pipeline exists today).

### Technical Notes

- Image must include all needed binaries (no runtime `cargo install`).
- Migrations: `sqlx migrate run` invoked on startup; safe under concurrent replica startup (see NFR-MIG-01).
- Bootstrap token: HMAC of (UUID + expiry) signed with SESSION_SECRET; stored single-use in a `bootstrap_tokens` table.

### Size

**M** (3 days, 5 scenarios). One developer; touches: Dockerfile, docker-compose.yml, migration runner, bootstrap module.

### Dependencies

None — this is the walking skeleton entry point.

---

## US-02: Operator scales to multiple replicas

- **job_id**: jtbd-outcome-6 (Minimize operability tax for multi-replica deployments)

### Story

As an **operator** running Foundry for a team that's grown to 80 people, I want to run N foundry replicas behind a load balancer with no sticky-session requirement, so that I can deploy zero-downtime updates and survive single-replica failures.

### Elevator Pitch

- **Before**: Devansh sets `FOUNDRY_REPLICAS=3` in the override compose file or scales via `docker compose up -d --scale foundry=3`.
- **After**: He hits the load balancer URL from three browsers, each picks a different replica, all show the same workspace state, and killing any one replica does not log anyone out.
- **Decision enabled**: Devansh decides Foundry is production-ready for his 80-person org.

### Problem

Linear-quality teams expect zero-downtime updates. Most OSS trackers require sticky sessions or in-process state that breaks under multi-replica deployments — turning the "self-host" promise into "single-point-of-failure-self-host." Devansh needs to know on day one whether Foundry supports the production posture he requires.

### Who

- **Devansh Iyer**, same operator from US-01, now operating Foundry at scale.
- **Context**: 3-replica deployment behind nginx or Traefik; round-robin LB.
- **Motivation**: Survive single-replica failure without user-visible logouts.

### Solution

- Sessions stored in Postgres (`tower-sessions-sqlx-store`); any replica can validate any cookie.
- SSE connections are stateless: each replica subscribes to Postgres `LISTEN issue_updates`; cross-replica fanout happens through Postgres.
- No in-memory rate limiting in MVP (deferred); rate limits, when added, will use Postgres.

### Domain Examples

#### 1. Happy path — rolling restart

Devansh runs Foundry as 3 replicas. He pulls a new image tag and runs `docker compose up -d` again with `--no-deps foundry`. Docker rolls them one at a time. Mei is editing an issue when replica 2 restarts — her next save lands on replica 1, succeeds, and she never sees an error.

#### 2. Edge case — replica dies mid-SSE

Hiroshi's browser is connected to replica 3 receiving SSE updates. Replica 3 OOMs. His browser's EventSource auto-reconnects to replica 1 within 5 seconds, and Postgres LISTEN/NOTIFY resumes pushing him updates. He sees a brief "Reconnecting…" banner; no manual refresh required.

#### 3. Error path — Postgres unreachable

All replicas lose Postgres connectivity. Each replica's `/readyz` endpoint flips to failing within 10 seconds. The load balancer (or K8s probe) pulls them from rotation. Users see a maintenance page served by the LB, not Foundry's error page.

### UAT Scenarios

```gherkin
Scenario: Session survives replica switch
  Given Foundry is running with 3 replicas behind a round-robin load balancer
  And Mei has signed in and her browser is currently routed to replica 2
  When the load balancer next routes Mei to replica 1
  Then Mei remains signed in
  And no re-authentication prompt appears

Scenario: SSE auto-reconnects to a healthy replica
  Given Hiroshi's browser is receiving SSE issue updates from replica 3
  When replica 3 stops responding
  Then within 10 seconds Hiroshi's browser reconnects to a different replica
  And the new SSE stream resumes delivering issue updates within 5 seconds

Scenario: Readyz drains traffic during DB outage
  Given Foundry has 3 replicas connected to Postgres
  When Postgres becomes unreachable from all replicas
  Then within 10 seconds all replicas return HTTP 503 from /readyz
  And the load balancer removes them from rotation

Scenario: Rolling restart preserves in-flight realtime subscriptions
  Given 3 replicas are running and users are connected via SSE to each
  When replicas restart one at a time with at least 30 seconds between each
  Then no user loses more than one SSE reconnect cycle
```

### Acceptance Criteria

- [ ] No sticky-session requirement: any cookie validates on any replica.
- [ ] `/readyz` returns 503 within 10 seconds of losing Postgres connectivity.
- [ ] SSE reconnect resumes user-visible updates within 15 seconds of replica failure.
- [ ] Three replicas + 1 LB is the documented production reference topology.

### Outcome KPIs

- **Who**: Operators running ≥2 replicas in production.
- **Does what**: Complete a rolling restart with zero user-visible logouts.
- **By how much**: 95% of rolling restarts cause no auth re-prompts (sampled across operator-reported restarts).
- **Measured by**: Survey of operators on `#foundry-operators` chat + optional `replica_restart_events` opt-in metric.
- **Baseline**: 0 (no multi-replica path tested today).

### Technical Notes

- Postgres connection pool sized via `DATABASE_MAX_CONNECTIONS` env (default 10 per replica).
- LISTEN/NOTIFY uses one dedicated listener task per replica; broadcast to local SSE subscribers via tokio::sync::broadcast.
- No replica IDs needed — replicas are interchangeable.

### Size

**M** (3 days). Touches: session store wiring, healthcheck endpoints, SSE module, docs.

### Dependencies

- US-01 (install must work first).
- US-09 (SSE realtime exists to be tested).

---

## US-03: Operator backs up and restores

- **job_id**: jtbd-outcome-2 (Maximize data sovereignty — no extraction tax)

### Story

As an **operator**, I want a single `pg_dump` to produce a complete Foundry backup including issue attachments, so that I can confidently restore on another machine without losing any user data.

### Elevator Pitch

- **Before**: Devansh runs `docker compose exec postgres pg_dump -U foundry foundry > backup.sql`.
- **After**: He spins up a new VM, restores with `psql < backup.sql`, runs `docker compose up`, signs in with his existing credentials, and sees every issue + attachment intact.
- **Decision enabled**: Devansh decides Foundry's data-ownership claim is real and not a marketing line.

### Problem

The JTBD names data sovereignty as outcome #2. SaaS trackers offer "export" that loses workflow state, custom fields, and attachments. If Foundry's backup story requires three separate commands (db + files + uploads + secrets), it inherits the same fragility. The promise is: one dump = everything.

### Who

- **Devansh Iyer**, same operator, now thinking about disaster recovery.
- **Context**: Existing Foundry instance with ~500 issues, ~50 attachments, 12 active users.
- **Motivation**: Sleep at night knowing data loss is recoverable.

### Solution

- All data — including issue attachments — lives in Postgres tables (attachments as `bytea`).
- `pg_dump` is the entire backup story.
- Sessions are restored too (users stay logged in across restore).
- A `foundry doctor backup-verify <file>` CLI subcommand checks dump integrity and reports row counts.

### Domain Examples

#### 1. Happy path — daily backup, restore on test VM

Devansh runs nightly `pg_dump foundry > /backups/foundry-$(date +%F).sql.gz`. Today's dump is 240 MB (mostly attachment bytea). He restores it on a test VM, signs in as himself, and verifies issue #FOUNDRY-127 still has its three PDF attachments.

#### 2. Edge case — partial restore mid-write

The dump is taken at 14:00. Hiroshi commented at 14:00:02. The comment is missing from the dump (consistent point-in-time snapshot). On restore, Hiroshi sees his comment is gone but receives no error; he re-comments and life continues. Documented behavior, not a bug.

#### 3. Error path — restore on different Postgres major version

Devansh restores a PG16 dump onto PG15. `psql` errors on a `bigint identity` feature. The Foundry FAQ links this to the `pg_dump --compatible-version` flag. Foundry's docs specify "use the same major Postgres version for restores."

### UAT Scenarios

```gherkin
Scenario: Single pg_dump captures all Foundry state including attachments
  Given Foundry has 100 issues, 20 of which have file attachments totalling 50 MB
  When Devansh runs "pg_dump -U foundry -d foundry -F c -f backup.dump"
  Then the dump file contains all issue rows
  And the dump file contains all attachment bytea data
  And no Foundry data lives outside the Postgres database

Scenario: Restore on a fresh machine reproduces the full system
  Given a backup.dump file from a Foundry instance with 100 issues
  And a fresh Postgres instance running on a new VM
  When Devansh runs "pg_restore -d foundry backup.dump" and starts Foundry pointing at it
  Then all 100 issues are visible in the UI
  And all 20 attachments download with byte-identical content to the original
  And all user sessions older than the backup time still validate (or are gracefully expired)

Scenario: Backup verification subcommand reports row counts
  Given Devansh has a backup.dump file
  When Devansh runs "foundry doctor backup-verify backup.dump"
  Then output contains row counts for each Foundry table
  And output reports total attachment bytes
  And exit code is 0 on success, non-zero on detected corruption
```

### Acceptance Criteria

- [ ] No Foundry state lives outside the Postgres database (verifiable by checking the running container has no `/data` volume).
- [ ] A `pg_restore` on the dump alone, on a fresh Postgres of the same major version, produces an identical functional system.
- [ ] `foundry doctor backup-verify` is provided and exits non-zero on integrity violations.

### Outcome KPIs

- **Who**: Operators who run backup verification.
- **Does what**: Successfully restore from a backup at least once during evaluation.
- **By how much**: 100% of operators who follow the documented restore procedure see all original data.
- **Measured by**: Documented in operator survey; FAQ-tagged GitHub issues for restore failures.
- **Baseline**: N/A (new capability).

### Technical Notes

- Attachments stored as `bytea` in `issue_attachments` table; max size capped per NFR-PERF-02.
- Logout-on-restore is acceptable if session data carries over but secret rotation invalidates them — document either choice clearly.

### Size

**S** (1-2 days). Mostly documentation and the verify subcommand; the architecture (Postgres-for-everything) already delivers most of this story.

### Dependencies

- US-01 (install completed).
- US-11 (attachments exist).

---

## US-04: Operator upgrades in place

- **job_id**: jtbd-outcome-7 (Minimize upgrade-without-breakage friction)

### Story

As an **operator**, I want to bump the Foundry image tag, run migrations idempotently, and have zero data loss, so that I can keep up with releases without scheduled maintenance windows.

### Elevator Pitch

- **Before**: Devansh edits `docker-compose.yml` to change `image: foundry:0.2.1` to `foundry:0.3.0` and runs `docker compose pull && docker compose up -d`.
- **After**: Migrations run, Foundry comes back healthy, all issues and attachments remain intact, and no users are logged out unless the release notes explicitly say so.
- **Decision enabled**: Devansh decides to track Foundry's release cadence (monthly) instead of pinning forever.

### Problem

Upgrading OSS infra in production is high-risk: irreversible migrations, secret-format changes, schema rewrites that break running workers. If Foundry's upgrade story is brittle, operators pin to v0.1 forever, miss security fixes, and eventually leave. The promise is: forward-only SQL, idempotent migrations, no manual steps.

### Who

- **Devansh Iyer**, same operator, now 3 months in.
- **Context**: Foundry v0.2.1 in production with 12 users and 800 issues; v0.3.0 just released.
- **Motivation**: Apply the upgrade in a 10-minute window without paging anyone.

### Solution

- `sqlx migrate run` invoked at startup; migrations are forward-only SQL files, numbered, idempotent (idempotent meaning: re-running them on an already-migrated DB is a no-op, not a crash).
- Each migration is wrapped in a transaction where possible (some Postgres operations cannot be transactional — these are explicitly noted).
- Replicas race on the migration table; the first to acquire the advisory lock runs migrations, others wait and skip.
- Release notes call out any breaking change explicitly (e.g., "v0.3.0: sessions are invalidated; users must re-sign-in").

### Domain Examples

#### 1. Happy path — minor version bump, no breaking changes

v0.2.1 → v0.2.2. Devansh bumps the tag. The 3 replicas start in sequence. Replica 1 acquires the advisory lock, runs one new migration (adds a column with a default), commits. Replicas 2 and 3 see the migration is applied and skip. No user is logged out.

#### 2. Edge case — concurrent replica startup races for migration lock

3 replicas start simultaneously. All 3 attempt `pg_advisory_lock(MIGRATION_LOCK_ID)`. Replica 1 wins, runs migrations (4 seconds). Replicas 2 and 3 block on the advisory lock. When Replica 1 commits and releases, Replicas 2 and 3 acquire, observe schema is current, release, and continue startup.

#### 3. Error path — migration fails mid-way

v0.3.0 → v0.3.1 includes a migration that adds a NOT NULL column without default; existing rows can't backfill. The migration errors; the transaction rolls back; the replica fails health checks and exits. Devansh sees the error in logs, fixes the migration file (or pins back to v0.3.0), and re-deploys. **No partial schema state was left behind.**

### UAT Scenarios

```gherkin
Scenario: Minor version upgrade preserves all data
  Given Foundry v0.2.1 is running with 800 issues and 50 attachments
  When Devansh changes the image tag to v0.2.2 and runs "docker compose up -d"
  Then within 60 seconds Foundry v0.2.2 reports healthy
  And all 800 issues are present with unchanged content
  And all 50 attachments download with byte-identical content

Scenario: Migrations are idempotent under concurrent replica startup
  Given 3 replicas of Foundry start simultaneously against a Postgres needing one new migration
  When the replicas race to apply the migration
  Then exactly one replica runs the migration
  And the other two replicas wait for the advisory lock then proceed without error
  And no migration is applied twice

Scenario: Failed migration leaves the database in the pre-migration state
  Given Foundry is about to apply a migration that will fail
  When the migration runs and errors
  Then the transaction is rolled back
  And the schema version in the migrations table is unchanged
  And the replica exits with non-zero status visible to the operator
```

### Acceptance Criteria

- [ ] Migrations are forward-only SQL files, numbered, in `migrations/` directory.
- [ ] Multi-replica startup uses Postgres advisory locks to serialize migration runs.
- [ ] Failed migrations roll back; no partial schema state.
- [ ] Release notes contain a migration impact summary for every release.

### Outcome KPIs

- **Who**: Operators upgrading between Foundry versions.
- **Does what**: Complete a version upgrade with no data loss and no downtime exceeding the rolling-restart window.
- **By how much**: 95% of minor-version upgrades complete with zero user-visible disruption.
- **Measured by**: Optional operator survey post-upgrade + GitHub issue tag `upgrade-failure` count.
- **Baseline**: N/A.

### Technical Notes

- `sqlx-cli` is the migration tool of record.
- Advisory lock ID is a fixed integer constant in the codebase.
- Postgres operations that cannot be transactional (e.g., `CREATE INDEX CONCURRENTLY`) are noted in a header comment in the migration file.

### Size

**M** (2-3 days). Touches: migration runner, advisory locks, docs.

### Dependencies

- US-01 (initial install exists to be upgraded).

---

## US-05: Admin bootstraps a workspace and invites teammates

- **job_id**: jtbd-outcome-1 (Minimize time to stand up a working issue tracker — extends "stood up" to "team is using it")

### Story

As the **first user (admin)** in a freshly-installed Foundry, I want to claim my admin account, name my workspace, and invite teammates via email or shareable link, so that my team can start using Foundry within the same hour as installation.

### Elevator Pitch

- **Before**: Devansh visits the bootstrap URL from US-01, sets an email + password, names the workspace "Acme Eng".
- **After**: He sees the workspace dashboard with a "Members (1)" panel and a "Generate invite link" button; clicking it copies a link to clipboard he can paste into Slack.
- **Decision enabled**: Devansh decides whether Foundry is ready to onboard his teammates today (yes — invite link ready in 60 seconds).

### Problem

Most OSS trackers force admins through a multi-page setup wizard (LDAP config, SMTP config, branding config, default workflow config). For the indie segment, this is unnecessary friction. Admins want one form, then "show me the app" and "show me how to invite people."

### Who

- **Devansh Iyer**, just claimed admin via US-01.
- **Context**: Empty Foundry instance, single bootstrap admin.
- **Motivation**: Get teammates in before lunch ends.

### Solution

A 2-screen bootstrap flow:

1. **Claim**: email + password + display name + workspace name. Single form, single submit.
2. **Invite**: post-bootstrap landing has a clear "Invite teammates" action that generates a shareable link (expires after 7 days) OR sends an email invite if SMTP is configured.

Default team named "General" is auto-created. Default project "Sandbox" is auto-created.

### Domain Examples

#### 1. Happy path — invite link

Devansh claims admin (`devansh@acme.com` / "Acme Eng" workspace). The post-claim page has a "Copy invite link" button. He clicks it, pastes into Slack #engineering, and 6 teammates click the link, set passwords, and appear in his Members panel within 20 minutes.

#### 2. Edge case — invite link expired

Mei's link from 8 days ago is expired. She visits it and sees "This invite has expired. Ask Devansh for a new one." No partial signup state is created.

#### 3. Error path — duplicate workspace name in same instance

Foundry MVP supports ONE workspace per instance (multi-workspace is deferred). If someone tries to create a second workspace via API, they get 409 Conflict with a clear message.

### UAT Scenarios

```gherkin
Scenario: First-run admin claim creates workspace and admin user
  Given a fresh Foundry instance with no users and a valid bootstrap token URL
  When Devansh visits the bootstrap URL
  And submits email "devansh@acme.com", password "correct horse battery staple", display name "Devansh", workspace name "Acme Eng"
  Then Devansh sees the workspace dashboard for "Acme Eng"
  And Devansh is signed in as the workspace's only admin
  And a default team "General" exists
  And a default project "Sandbox" exists in the General team

Scenario: Admin generates a shareable invite link
  Given Devansh is signed in as admin of "Acme Eng"
  When Devansh clicks "Invite teammates" and selects "Copy link"
  Then a URL is copied to clipboard
  And the URL contains a signed invite token
  And the URL is valid for 7 days from generation

Scenario: Invitee signs up via shared link
  Given an unexpired invite link for the "Acme Eng" workspace
  When Mei (mei@acme.com) visits the link and submits email, password, display name
  Then Mei is created as a member of "Acme Eng"
  And Mei is signed in
  And Mei sees the same default team and project as Devansh

Scenario: Expired invite link rejects signup
  Given an invite link generated 8 days ago with a 7-day TTL
  When Mei visits the link
  Then Mei sees "This invite has expired"
  And no user account is created
  And Mei is offered a link to the regular sign-in page

Scenario: Email invite (if SMTP configured)
  Given SMTP env vars are set in .env (SMTP_HOST, SMTP_USER, SMTP_PASS, SMTP_FROM)
  And Devansh enters "mei@acme.com" in the Invite Teammates form and clicks "Send invite"
  When Foundry processes the invite
  Then an email is delivered to mei@acme.com with subject "Devansh invited you to Acme Eng on Foundry"
  And the email body contains a signed invite link
```

### Acceptance Criteria

- [ ] Bootstrap form requires email, password, display name, workspace name in a single screen.
- [ ] Default team "General" and default project "Sandbox" are auto-created.
- [ ] Invite links carry a signed token, default 7-day TTL.
- [ ] If SMTP env vars set, email invites are also available; otherwise the UI shows only "Copy link".
- [ ] Multi-workspace per instance is explicitly out of scope; second-workspace creation returns 409.

### Outcome KPIs

- **Who**: First-time admins claiming a fresh Foundry instance.
- **Does what**: Successfully invite at least one teammate within the same session.
- **By how much**: 70% of bootstrapped instances see a second-member signup within 24 hours.
- **Measured by**: Opt-in instance telemetry (`workspace_member_count` distribution at 24h).
- **Baseline**: N/A.

### Technical Notes

- Password hashing: argon2id with reasonable 2026 cost parameters (m=64MiB, t=3, p=1).
- Invite tokens: HMAC of (workspace_id + invitee_email_or_null + expiry) signed with SESSION_SECRET; not stored unless email invite (then linked to invite_id).
- Email delivery: lettre crate with SMTP transport; no in-app SMTP server.

### Size

**M** (3 days). UI form, invite token module, email integration, default workspace seeding.

### Dependencies

- US-01 (bootstrap URL exists).
- US-06 (sign-in flow exists for invitee signup).

---

## US-06: User signs in with email and password

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — daily-usable, low-friction sign-in is part of the Linear-feel promise)

### Story

As a **member** of a Foundry workspace, I want to sign in with my email and password, so that I can return to Foundry tomorrow and see my work without going through the invite flow again.

### Elevator Pitch

- **Before**: Mei visits `https://foundry.acme.com/sign-in`, types `mei@acme.com` and her password.
- **After**: She lands on her workspace dashboard with her recent issues visible.
- **Decision enabled**: Mei decides Foundry is daily-usable (low-friction login).

### Problem

OIDC SSO is deferred to v0.3 per DIVERGE decision. The MVP must ship a competent password auth flow because the indie segment ("first 100 self-hosters") will use this exclusively. Auth flows are where security bugs cluster — argon2id, brute-force lockout, secure cookies, and password reset must all be done correctly even though they're not differentiators.

### Who

- **Mei Chen**, member of Acme Eng workspace.
- **Context**: Returning user, second visit. Cookie from yesterday may or may not be valid.
- **Motivation**: Get back to her open issues.

### Solution

- Sign-in form: email + password. Submit → server validates with argon2id verify → sets HttpOnly Secure SameSite=Lax session cookie with 30-day TTL.
- Sign-out: button in user menu; clears server-side session row.
- Password reset: "Forgot password?" link → if SMTP configured, emails a single-use reset link with 1-hour TTL; if not configured, admins can reset via `foundry admin reset-password <email>` CLI subcommand.
- Brute-force protection: after 5 failed attempts on the same email within 15 minutes, the next attempt is delayed by 5 seconds (per-request artificial delay, no lockout — see NFR-SEC-02 for rationale).

### Domain Examples

#### 1. Happy path — sign in with valid credentials

Mei visits `/sign-in`, enters `mei@acme.com` + her password, clicks Sign In. Browser receives `foundry_session=...` cookie. She lands on `/`. Total time: 800ms.

#### 2. Edge case — password reset via email

Mei forgot her password. She clicks "Forgot password?" → enters `mei@acme.com` → sees "If that email is on file, a reset link has been sent." She receives an email with a reset link, clicks it, enters a new password, and is signed in.

#### 3. Error path — wrong password

Hiroshi mistypes his password. The page shows "Invalid email or password" (same message for wrong-email and wrong-password to avoid user enumeration). After 5 attempts, the 6th submit takes 5 seconds.

### UAT Scenarios

```gherkin
Scenario: Member signs in successfully
  Given Mei has a member account in Acme Eng workspace
  And Mei is at the sign-in page with no current session
  When Mei submits her email and correct password
  Then Mei is redirected to the workspace dashboard
  And Mei's browser holds an HttpOnly Secure SameSite=Lax session cookie
  And the session is valid for 30 days

Scenario: Invalid credentials produce non-enumerable error
  Given Hiroshi attempts to sign in with email "not-a-user@acme.com"
  When Hiroshi submits any password
  Then the page displays "Invalid email or password"
  And the same message appears for a real user with a wrong password

Scenario: Brute-force protection delays repeated failures
  Given Mei has failed sign-in 5 times in the last 15 minutes for "mei@acme.com"
  When Mei submits a 6th attempt
  Then the response is delayed by approximately 5 seconds
  And the response otherwise behaves the same as the first attempt

Scenario: Password reset via email link
  Given SMTP is configured and Mei has forgotten her password
  When Mei clicks "Forgot password?" and submits "mei@acme.com"
  Then the response is "If that email is on file, a reset link has been sent" regardless of whether the email exists
  And if mei@acme.com exists, an email with a reset link arrives
  And the reset link is valid for 1 hour and single-use

Scenario: Password reset via CLI when SMTP unconfigured
  Given SMTP is not configured and Mei is locked out
  When Devansh (admin) runs "docker compose exec foundry foundry admin reset-password mei@acme.com"
  Then a new temporary password is generated and printed to stdout
  And Mei can sign in with the temp password and is forced to change it

Scenario: Sign-out clears the server-side session
  Given Mei is signed in with an active session
  When Mei clicks "Sign out"
  Then the server-side session row is deleted
  And Mei's browser cookie is cleared
  And presenting the old cookie to a protected endpoint returns 401
```

### Acceptance Criteria

- [ ] Password hashing uses parameters at or above current OWASP-recommended levels; parameters are reviewed annually and updated if OWASP recommendations change.
- [ ] Session cookies are HttpOnly, Secure (when behind HTTPS), SameSite=Lax, 30-day TTL.
- [ ] Failed-attempt delays kick in after 5 within 15 minutes per email.
- [ ] Password reset works via email when SMTP set, via CLI otherwise.
- [ ] Sign-out invalidates the server-side session row, not just the cookie.

### Outcome KPIs

- **Who**: Returning Foundry users.
- **Does what**: Successfully sign in on the first attempt.
- **By how much**: 95% first-attempt success rate among non-forgotten passwords.
- **Measured by**: Opt-in metric `sign_in_attempts_to_success` distribution.
- **Baseline**: N/A.

### Technical Notes

- argon2 crate (Rust) for hashing; `secrecy` crate to prevent password material in logs.
- Initial argon2id parameters at release time: `m=64MiB, t=3, p=1` (meets OWASP 2026 recommended minimum). Encoded in a single constant; bumped via PR when OWASP guidance changes. See NFR-SEC-01 for the parameter contract and review cadence.
- Session table: `(id TEXT PRIMARY KEY, user_id UUID, expires_at TIMESTAMPTZ, data BYTEA)` per tower-sessions schema.
- Reset-token table: `(token_hash BYTEA PRIMARY KEY, user_id UUID, expires_at TIMESTAMPTZ, used_at TIMESTAMPTZ NULL)`.

### Size

**M** (3 days). Form, session module, password reset (email + CLI), brute-force delay.

### Dependencies

- US-05 (users exist to sign in).

---

## US-07: User creates and views a project under a team

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — team→project→issue hierarchy that matches Linear's mental model)

### Story

As a **member**, I want to create a new project under one of my teams and view its issue list, so that I have a place to file the issues my team is about to work on.

### Elevator Pitch

- **Before**: Mei clicks the "+" next to "Projects" in the sidebar under the "Backend" team, types "Auth v2", clicks Create.
- **After**: The sidebar updates to include "Auth v2"; the main pane shows an empty Auth v2 board with state columns (Backlog / Todo / In-Progress / Done) and a "New issue" button.
- **Decision enabled**: Mei decides Foundry's project model fits her mental model (Linear-like: teams own projects, projects own issues).

### Problem

Foundry's hierarchy must match Linear's enough that Linear-trained users don't fight it. Teams own projects; projects own issues. Anything else (e.g., flat-issue-list with tags-only) breaks the Linear-feel promise.

### Who

- **Mei Chen**, member of Acme Eng, on the "Backend" team.
- **Context**: Existing workspace with one team and one project; she wants a new project.
- **Motivation**: File next sprint's work somewhere recognizable.

### Solution

- Team CRUD (admin-only for create/delete).
- Project CRUD under a team (any member of the team can create).
- Project list view with state columns and a project-scoped "New issue" button.

### Domain Examples

#### 1. Happy path — member creates project

Mei is on team "Backend". She clicks the "+" next to "Projects" in the sidebar. A modal asks for name + key prefix (auto-suggested "AUTH"). She types "Auth v2", key prefix "AUTH". Submit → project page loads, sidebar updates, URL is `/team/backend/project/auth-v2`.

#### 2. Edge case — duplicate project name within same team

A second project named "Auth v2" in the same team produces an inline error "Project name must be unique within this team." Project key uniqueness is also enforced (different error message).

#### 3. Error path — non-member tries to create project in team they don't belong to

Hiroshi is on team "Mobile" only. He hits the URL `/team/backend/projects/new` directly. Server returns 403 Forbidden with a page explaining "You're not a member of the Backend team."

### UAT Scenarios

```gherkin
Scenario: Member creates a project in their team
  Given Mei is signed in and is a member of the "Backend" team in workspace "Acme Eng"
  When Mei creates a project named "Auth v2" with key prefix "AUTH"
  Then the project appears in the sidebar under Backend
  And navigating to the project shows an empty board with columns Backlog, Todo, In-Progress, Done
  And the URL is "/team/backend/project/auth-v2"

Scenario: Project name uniqueness within a team
  Given a project named "Auth v2" already exists in the Backend team
  When Mei attempts to create another project named "Auth v2" in Backend
  Then the form shows an inline error indicating the name must be unique within the team
  And no second project is created

Scenario: Same name allowed across different teams
  Given a project named "Onboarding" exists in the Backend team
  When Mei creates a project named "Onboarding" in the Mobile team
  Then both projects exist and are addressable by their team slug

Scenario: Non-team-member cannot create project in that team
  Given Hiroshi is a workspace member but not a member of the Backend team
  When Hiroshi attempts to create a project in Backend
  Then he receives an HTTP 403 response with a clear explanation
  And no project is created

Scenario: Admin can create teams
  Given Devansh is workspace admin and "Mobile" team does not exist
  When Devansh creates a team named "Mobile"
  Then the team appears in the workspace sidebar
  And Devansh is auto-added as a member
```

### Acceptance Criteria

- [ ] Project belongs to exactly one team; team belongs to exactly one workspace.
- [ ] Project key prefix is auto-suggested from name, editable, 2-6 uppercase chars.
- [ ] Project name unique within team; project key unique within workspace.
- [ ] Default states (Backlog / Todo / In-Progress / Done / Cancelled) seeded on project creation; not editable in MVP.

### Outcome KPIs

- **Who**: First-week Foundry users.
- **Does what**: Create at least one project beyond the auto-seeded "Sandbox".
- **By how much**: 80% of workspaces have ≥2 projects within 7 days.
- **Measured by**: Opt-in instance metric `projects_per_workspace` distribution.
- **Baseline**: N/A.

### Technical Notes

- Issue state list is fixed in MVP; custom workflows deferred to v0.4.
- Routes: `GET /team/:team_slug/projects/new`, `POST /team/:team_slug/projects`, `GET /team/:team_slug/project/:project_slug`.

### Size

**M** (2-3 days). Team CRUD, project CRUD, board view scaffolding.

### Dependencies

- US-05 (workspace + default team exist).
- US-06 (sign-in).

---

## US-08: User files an issue

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — the JTBD-critical hot path)

### Story

As a **member**, I want to file an issue with a title, markdown description, assignee, labels, state, and priority, so that I can capture work in the same shape Linear uses and not lose context.

### Elevator Pitch

- **Before**: Mei hits `c` from anywhere in the app, the issue-create modal pops, she types the title, hits Tab into description, types markdown, sets assignee, hits Cmd-Enter.
- **After**: The new issue `AUTH-1` appears in the project board's Backlog column; closing the modal returns Mei to wherever she was.
- **Decision enabled**: Mei decides Foundry's issue-create speed matches Linear's (the JTBD-critical moment).

### Problem

The hot path of the JTBD: the most-frequent user action is "I had a thought, I need to file it before I lose it." Linear's speed here is famous and is *the* feature OSS alternatives miss. If `c` doesn't open a modal in <100ms with focus already in the title field, Foundry has failed the speed promise.

### Who

- **Mei Chen**, member of Acme Eng, on Backend team.
- **Context**: Browsing the Backend / Auth v2 project board.
- **Motivation**: Capture a thought before it evaporates.

### Solution

- Global `c` keyboard shortcut opens issue-create modal from anywhere.
- Modal pre-fills the current project context (if user is on a project page); otherwise asks.
- Title is the only required field; everything else (description, assignee, labels, state, priority) defaults sanely.
- Cmd/Ctrl-Enter submits; Esc cancels; submit closes modal and shows brief inline toast "Created AUTH-1" linking to the new issue.

### Domain Examples

#### 1. Happy path — fast issue creation with title only

Mei hits `c` while looking at the Auth v2 board. Modal appears in 80ms with title field focused. She types "Refresh token rotation broken on Safari", hits Cmd-Enter. Modal closes; toast appears bottom-right "Created AUTH-1"; the issue appears in the Backlog column. Total elapsed: under 5 seconds.

#### 2. Edge case — full-detail issue with description, assignee, labels

Hiroshi opens the modal, types title "Implement OIDC support for v0.3", hits Tab into description, types 3 paragraphs of markdown, types `@` and selects Devansh as assignee, types `#tech-debt #v0.3` to set labels, changes priority to High, presses Cmd-Enter.

#### 3. Error path — submit with empty title

Mei accidentally hits Cmd-Enter with no title. The form shows inline "Title is required" without closing the modal; her cursor is in the title field.

### UAT Scenarios

```gherkin
Scenario: Quick issue creation with just a title
  Given Mei is viewing the Auth v2 project board
  When Mei presses the keyboard shortcut "c"
  And the issue-create modal opens within 200ms with focus in the title field
  And Mei types "Refresh token rotation broken on Safari"
  And Mei presses Cmd-Enter
  Then the modal closes
  And a new issue with key "AUTH-1" appears in the Backlog column
  And a toast notification confirms creation

Scenario: Issue with full detail
  Given Mei is creating an issue
  When Mei sets title, description in markdown, assignee, labels, priority "High"
  And Mei submits
  Then the new issue has all submitted fields persisted
  And opening the issue page displays the markdown rendered as HTML

Scenario: Empty title is rejected
  Given the issue-create modal is open with an empty title field
  When Mei submits the form
  Then an inline error "Title is required" is shown
  And the modal remains open
  And focus returns to the title field

Scenario: Markdown rendering matches CommonMark
  Given Mei creates an issue with description "**bold** and *italic* and a [link](https://example.com) and ```code```"
  When the issue page renders
  Then bold, italic, link, and inline-code render correctly per CommonMark
  And any embedded HTML or javascript: URLs are sanitized

Scenario: Assignee dropdown shows team members
  Given Mei is creating an issue on a Backend project
  When Mei opens the Assignee dropdown
  Then it lists all members of the Backend team
  And it does not list members of other teams unless the workspace setting "Allow cross-team assignment" is enabled (deferred)

Scenario: Issue keys are sequential per project
  Given the Auth v2 project has issues AUTH-1 through AUTH-5
  When Mei creates a new issue
  Then the new issue's key is "AUTH-6"
```

### Acceptance Criteria

- [ ] `c` keyboard shortcut works globally (any signed-in page).
- [ ] Modal opens within 200ms; title field is focused.
- [ ] Only title is required; defaults are: state=Backlog, priority=Medium, assignee=null, labels=[], description=empty.
- [ ] Markdown rendering uses CommonMark; raw HTML and dangerous URLs sanitized.
- [ ] Issue keys are sequential per project, prefixed with the project key.

### Outcome KPIs

- **Who**: Daily-active Foundry users.
- **Does what**: File issues quickly (Linear-parity).
- **By how much**: Median issue-create time (modal open to submit) ≤8 seconds for title-only issues.
- **Measured by**: Client-side timing telemetry (opt-in), event `issue_created` with `time_in_modal_ms` field.
- **Baseline**: Linear users report ~5s median (anecdotal); 8s is the target.

### Technical Notes

- Markdown: `pulldown-cmark` for rendering; `ammonia` for HTML sanitization.
- Issue keys allocated via Postgres sequence per project (`projects.next_issue_number`) inside the transaction.
- Modal is htmx-driven: `hx-get="/issues/new?project=AUTH"` returns the modal HTML; submit is `hx-post="/issues"`.

### Size

**M** (3 days). Issue model, create endpoint, modal UI, markdown rendering, keyboard shortcut handler.

### Dependencies

- US-07 (projects + teams exist).

---

## US-09: User sees realtime updates

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — Linear-feel realtime)

### Story

As a **member** viewing an issue board, I want to see updates when teammates change issues on the same project, so that I have current information without manually refreshing.

### Elevator Pitch

- **Before**: Mei has the Auth v2 board open. Hiroshi drags AUTH-3 from Todo to In-Progress on his screen.
- **After**: Mei's board re-renders AUTH-3 in the In-Progress column within 1 second; a brief glow highlights the moved card.
- **Decision enabled**: Mei decides Foundry feels "live" the way Linear does.

### Problem

Without realtime, multi-user issue tracking devolves into "did you see I moved that?" Slack pings. The JTBD's "Linear-feel" promise lives or dies here. Polling every 5 seconds is the boring fallback; SSE is the choice because it's the simplest realtime that scales without operating a separate broker.

### Who

- **Mei Chen**, member viewing an issue board while Hiroshi edits.
- **Context**: Two browser tabs from different users on the same project page.
- **Motivation**: See teammates' changes immediately.

### Solution

- Each project board page opens an SSE connection to `/projects/:id/events`.
- When an issue is created/updated/deleted/moved, the writing replica calls `pg_notify('issue_events', payload)`.
- All replicas LISTEN; on notification, each replica fans out to local SSE subscribers whose subscription matches the event's project.
- Client-side htmx swap re-renders the affected card.

### Domain Examples

#### 1. Happy path — state change visible across users

Mei and Hiroshi both have Auth v2 open. Hiroshi changes AUTH-3 state from Todo to In-Progress. Mei sees the card move within 1 second, accompanied by a subtle highlight.

#### 2. Edge case — issue created while viewer is on board

Mei has the board open. Devansh creates AUTH-6. The new card appears in Backlog on Mei's screen within 1 second, no refresh.

#### 3. Error path — SSE connection drops

Mei's network blinks. EventSource auto-reconnects in <5 seconds. While disconnected, she may miss events; on reconnect, a "Reconnected — refresh for the latest" toast appears (MVP does NOT replay missed events; that's v0.4).

### UAT Scenarios

```gherkin
Scenario: Realtime update across two users on the same project board
  Given Mei and Hiroshi both have the Auth v2 board open in separate browsers
  And both are connected via SSE to /projects/auth-v2/events
  When Hiroshi changes the state of issue AUTH-3 from Todo to In-Progress
  Then within 1 second Mei sees AUTH-3 rendered in the In-Progress column

Scenario: New issue appears in real time
  Given Mei has the Auth v2 board open
  When Devansh creates a new issue AUTH-7 in the Auth v2 project
  Then within 1 second AUTH-7 appears in Mei's Backlog column

Scenario: Updates are scoped to the project being viewed
  Given Mei is viewing Auth v2 board only
  When Hiroshi updates an issue in the unrelated "Mobile-App" project
  Then Mei's board does NOT re-render
  And no SSE event for that update is delivered to Mei

Scenario: SSE auto-reconnects after network blip
  Given Mei's browser has an active SSE connection
  When the network drops for 3 seconds and recovers
  Then the browser EventSource automatically reconnects within 10 seconds
  And subsequent events are delivered normally

Scenario: Stale-state warning on reconnect
  Given Mei was disconnected for 30 seconds
  When the SSE reconnects
  Then a non-blocking toast appears stating "Reconnected — some events may have been missed"
  And the toast offers a "Refresh" action
```

### Acceptance Criteria

- [ ] Issue create/update/delete trigger `pg_notify`.
- [ ] Each app replica LISTENs on a single channel; per-client filtering happens in-process.
- [ ] Median client-to-client latency ≤1 second under nominal load (NFR-PERF-03).
- [ ] No event replay on reconnect in MVP (documented limitation).

### Outcome KPIs

- **Who**: Active users on a board where teammates are editing.
- **Does what**: Observe teammates' updates without manual refresh.
- **By how much**: 99% of board edits propagate to all connected viewers within 2 seconds.
- **Measured by**: Synthetic test in CI + opt-in real-user timing.
- **Baseline**: 0 (no realtime today).

### Technical Notes

- `pg_notify` payload is the JSON `{event_type, project_id, issue_id, timestamp}` (≤8KB Postgres limit).
- Server-side filter by project_id; never send cross-project events to clients.
- htmx `sse-swap` or vanilla EventSource + alpine-managed DOM update — choose one in DESIGN wave.

### Size

**M** (3 days). Outbox + pg_notify, SSE endpoint, broadcast fanout, client wiring.

### Dependencies

- US-08 (issues exist to be updated).

---

## US-10: User comments on an issue

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — in-issue discussion as a Linear-feel staple)

### Story

As a **member**, I want to comment on an issue with markdown, and have my comment appear immediately to other viewers, so that issue discussion happens in the same place as the work.

### Elevator Pitch

- **Before**: Mei opens AUTH-3, scrolls to the comment box, types "Looked into this — the root cause is the Set-Cookie SameSite default change.", clicks Comment.
- **After**: Her comment appears at the bottom of the thread; Hiroshi (also viewing) sees it within 1 second.
- **Decision enabled**: Mei decides discussion belongs in Foundry, not in Slack.

### Problem

Linear's "discussion at the issue" model wins because it co-locates context. Foundry must match: markdown comments, realtime delivery, edit/delete by author, no nested threads in MVP (keep it simple).

### Who

- **Mei Chen**, viewing an issue.
- **Context**: AUTH-3 has 2 existing comments.
- **Motivation**: Add information without leaving Foundry.

### Solution

- Comment composer below issue with markdown support.
- Comments displayed chronologically (oldest first).
- Author can edit/delete their own comments; admin can delete any.
- New comments fire via the same SSE channel as US-09; viewers see them appear without refresh.

### Domain Examples

#### 1. Happy path — comment posted, others see it

Mei comments on AUTH-3. Hiroshi (with the same issue open) sees her comment appear within 1 second.

#### 2. Edge case — author edits own comment

Mei realizes she typo'd. She clicks "Edit" on her own comment, fixes, saves. Hiroshi sees the updated text within 1 second; a subtle "edited" label appears.

#### 3. Error path — non-author tries to edit someone else's comment

Hiroshi inspects Mei's comment's HTML and tries to POST an edit. Server returns 403 Forbidden.

### UAT Scenarios

```gherkin
Scenario: Comment posted with markdown rendering
  Given Mei is viewing issue AUTH-3
  When Mei submits a comment with markdown "**bold** and `code`"
  Then the comment appears in the issue thread with bold and inline-code rendered
  And the comment's author is Mei
  And the comment's timestamp is the current time

Scenario: Comment appears in realtime to other viewers
  Given Hiroshi has AUTH-3 open at the same time as Mei
  When Mei submits a comment
  Then within 1 second Hiroshi sees the new comment appended to the thread

Scenario: Comment author can edit own comment
  Given Mei has previously posted a comment on AUTH-3
  When Mei edits the comment
  Then the updated text replaces the original in the thread
  And an "edited" indicator appears next to the comment timestamp
  And the edit is visible in realtime to other viewers

Scenario: Non-author cannot edit comment
  Given Hiroshi is not the author of a comment by Mei
  When Hiroshi attempts to POST an edit to that comment endpoint
  Then the server returns HTTP 403
  And the original comment text is unchanged

Scenario: Admin can delete any comment
  Given Devansh is workspace admin and a comment by Mei exists
  When Devansh deletes the comment
  Then the comment is removed from the thread
  And remaining viewers see it disappear within 1 second

Scenario: Comment author can delete own comment
  Given Mei has a comment on AUTH-3
  When Mei clicks "Delete" on her comment and confirms
  Then the comment is removed
  And other viewers see it disappear within 1 second
```

### Acceptance Criteria

- [ ] Markdown rendered with CommonMark, sanitized.
- [ ] Author can edit/delete own comments; admin can delete any.
- [ ] Realtime delivery via the same SSE channel as US-09; latency ≤1 second median.
- [ ] No nested threads in MVP (deferred).

### Outcome KPIs

- **Who**: Active workspace members on issues with multiple participants.
- **Does what**: Discuss in-Foundry instead of pinging chat.
- **By how much**: 50% of issues with >1 assignee touch have at least one in-Foundry comment.
- **Measured by**: Opt-in instance metric `issues_with_comments_ratio`.
- **Baseline**: 0 (no comments today).

### Technical Notes

- Comment table: `(id UUID, issue_id UUID FK, author_id UUID FK, body_md TEXT, created_at, updated_at, deleted_at NULL)`.
- Soft-delete preserves history; UI hides soft-deleted comments.
- Comment-related events use the same pg_notify channel as issue events with `event_type=comment_*`.

### Size

**S-M** (2 days). Comment CRUD + reuse of SSE channel from US-09.

### Dependencies

- US-08 (issues exist).
- US-09 (SSE channel exists).

---

## US-11: User attaches a file to an issue

- **job_id**: jtbd-outcome-2 (Maximize data sovereignty — attachments live in Postgres so pg_dump captures them, preserving the single-file backup story)

### Story

As a **member**, I want to attach a file (screenshot, log, PDF) to an issue, so that I can communicate context that doesn't fit in markdown.

### Elevator Pitch

- **Before**: Mei drags a screenshot from her desktop onto the issue's description area.
- **After**: An upload indicator appears, completes in a few seconds, and a thumbnail/link to the file appears inline; she can click it to download.
- **Decision enabled**: Mei decides Foundry can replace the "screenshot in Slack" workflow.

### Problem

Per DIVERGE decision: attachments live in Postgres `bytea`. This is the simplest backup story (one pg_dump = everything) and meets the indie-segment ceiling. The size cap must be sensible — too small and it's useless, too large and bytea breaks.

### Who

- **Mei Chen**, attaching a screenshot to an issue.
- **Context**: Reporting a UI bug; a picture is worth 1000 words.
- **Motivation**: Communicate visual context.

### Solution

- Drag-and-drop OR file picker on the issue page.
- Upload as `multipart/form-data` to `/issues/:id/attachments`; server stores in `issue_attachments` table with bytea content.
- Server returns the attachment URL; client appends a link/preview inline.
- Default cap: 10 MB per file (env-configurable via `FILE_UPLOAD_MAX_MB`; suggested max env value: 50 MB based on bytea/TOAST sensible ceiling).

### Domain Examples

#### 1. Happy path — screenshot under cap

Mei attaches a 2.3 MB PNG to AUTH-3. Upload completes in ~1 second on local network. Thumbnail appears inline.

#### 2. Edge case — large PDF near cap

Hiroshi attaches a 9.8 MB PDF (debug logs). Upload completes in ~3 seconds; appears as a link with size shown.

#### 3. Error path — file exceeds cap

Mei tries to attach a 25 MB video; default cap is 10 MB. Server rejects with HTTP 413 Payload Too Large; UI shows "File exceeds the 10 MB limit. Ask your admin to raise FILE_UPLOAD_MAX_MB."

### UAT Scenarios

```gherkin
Scenario: User uploads a file under the configured cap
  Given Mei is viewing issue AUTH-3
  And FILE_UPLOAD_MAX_MB is set to 10
  When Mei attaches a 2 MB PNG screenshot
  Then the upload completes within 5 seconds on a 100Mbps connection
  And a link or thumbnail appears in the issue body or attachment panel
  And clicking the link downloads the original file byte-identical to the upload

Scenario: File exceeding the cap is rejected
  Given FILE_UPLOAD_MAX_MB is set to 10
  When Mei attempts to upload a 25 MB file
  Then the server returns HTTP 413 Payload Too Large
  And the UI shows an explanatory message including the configured limit
  And no row is created in issue_attachments

Scenario: Attachment download preserves filename and content-type
  Given an attachment "design.pdf" of type application/pdf exists on AUTH-3
  When Hiroshi clicks the attachment link
  Then the browser downloads the file with filename "design.pdf"
  And the Content-Type response header is application/pdf

Scenario: Attachment is included in pg_dump
  Given AUTH-3 has a 5 MB attachment
  When Devansh runs pg_dump
  Then the dump file contains the bytea content for the attachment
  And restoring the dump reproduces the attachment byte-identically

Scenario: Attachment deleted with the parent issue
  Given AUTH-3 has 2 attachments
  When AUTH-3 is deleted
  Then both attachment rows are deleted (CASCADE)
```

### Acceptance Criteria

- [ ] Attachments stored as bytea in `issue_attachments` table.
- [ ] `FILE_UPLOAD_MAX_MB` env var controls per-file cap; default 10, recommended max 50.
- [ ] Download streams from Postgres bytea; no separate object store in MVP.
- [ ] CASCADE delete with parent issue.
- [ ] Content-Type sniffed and stored; sent on download.

### Outcome KPIs

- **Who**: Members reporting bugs or sharing context.
- **Does what**: Attach files to issues.
- **By how much**: 30% of issues have ≥1 attachment by the end of month 1.
- **Measured by**: Opt-in instance metric `issues_with_attachments_ratio`.
- **Baseline**: 0.

### Technical Notes

- bytea reads use streaming (sqlx supports this); do not load whole file into memory.
- For files >5 MB consider chunked HTTP encoding to keep memory bounded.
- Future: env-flag-controlled S3 backend for files >50 MB (post-MVP).

### Size

**M** (3 days). Upload endpoint, download endpoint, UI integration, size cap enforcement.

### Dependencies

- US-08 (issues exist to attach to).

---

## US-12: User navigates with keyboard shortcuts

- **job_id**: jtbd-outcome-4 (Maximize Linear-feel interaction speed — keyboard-driven flow is *the* Linear differentiator)

### Story

As a **member**, I want core keyboard shortcuts (`c` create, `/` search, `j/k` navigate list) to feel like Linear's, so that I don't lose muscle memory after switching from Linear.

### Elevator Pitch

- **Before**: Mei hits `?` from anywhere to see the shortcut help; she sees `c`, `/`, `j`, `k`, `Esc`, `Cmd-Enter` listed with their actions.
- **After**: She uses `c` to create, `/` to search by issue key or title, `j/k` to move selection on a list, `Enter` to open the selected issue. She never touches the mouse for the next hour.
- **Decision enabled**: Mei decides Foundry's keyboard story is competent enough to stop reaching for the mouse.

### Problem

Keyboard-driven flow is the headline difference between Linear and JIRA/Trello. Foundry's MVP must ship the table-stakes shortcuts even if it can't ship Linear's full command palette in v1.

### Who

- **Mei Chen**, keyboard-heavy power user.
- **Context**: Any Foundry page.
- **Motivation**: Move faster than mouse-driven users.

### Solution

- Global shortcuts: `c` create, `/` search, `?` help, `g+i` go-to-inbox (deferred labeling), `Esc` close modals.
- List shortcuts: `j` next, `k` prev, `Enter` open selected, `x` toggle select (multi-select deferred).
- Implemented in alpine.js with conflict avoidance for text inputs (ignore shortcuts when an input is focused).

### Domain Examples

#### 1. Happy path — issue list keyboard navigation

Mei is on the Backlog column of Auth v2. She presses `j` four times to move the selection down four issues, `Enter` to open the selected issue.

#### 2. Edge case — shortcut suppressed while typing

Mei is in the issue search box typing "c" as the first letter of a query. The `c` does NOT trigger create-modal because the focus is on a text input.

#### 3. Error path — unknown shortcut

Mei hits `q` (no binding). Nothing happens. No error. `?` still shows the available shortcuts.

### UAT Scenarios

```gherkin
Scenario: Help modal lists shortcuts
  Given Mei is on any Foundry page
  When Mei presses "?"
  Then a help modal opens listing at least: c (create), / (search), j (next), k (prev), Enter (open), Esc (close), ? (this help)

Scenario: c opens the issue-create modal globally
  Given Mei is on the workspace home page
  When Mei presses "c"
  Then the issue-create modal opens within 200ms
  And focus is in the title field

Scenario: j/k navigate an issue list
  Given Mei is viewing the Backlog column with 5 issues, none selected
  When Mei presses "j"
  Then the first issue becomes the selected (focused) row
  When Mei presses "j" twice more
  Then the third issue is selected
  When Mei presses "k"
  Then selection moves back to the second issue
  When Mei presses Enter
  Then the selected issue's detail page opens

Scenario: Shortcuts suppressed while typing in inputs
  Given Mei has the issue-create modal open with focus in the title field
  When Mei types "c" as part of the title
  Then a single "c" is appended to the title
  And no new create modal is triggered

Scenario: Esc closes modals
  Given any modal is open
  When Mei presses Esc
  Then the modal closes
  And focus returns to the element that was focused before the modal opened

Scenario: / opens search and focuses query input
  Given Mei is on any Foundry page
  When Mei presses "/"
  Then a search input appears (modal or pinned)
  And focus is in the search input
  And typing "AUTH-3" or "broken Safari" returns matching issues
```

### Acceptance Criteria

- [ ] Shortcuts: `c`, `/`, `?`, `j`, `k`, `Enter`, `Esc` work as specified.
- [ ] Shortcuts are suppressed when focus is on an editable input.
- [ ] Help modal (`?`) lists every shortcut and is the discoverability mechanism.
- [ ] Search supports issue key (`AUTH-3`) and full-text title/description match (simple ILIKE for MVP; Postgres FTS deferred).

### Outcome KPIs

- **Who**: Daily-active members who used Linear before.
- **Does what**: Use keyboard shortcuts at least once per session.
- **By how much**: 60% of sessions invoke ≥1 shortcut.
- **Measured by**: Client-side event `shortcut_invoked`.
- **Baseline**: 0.

### Technical Notes

- Alpine.js `x-on:keydown.window` with input-focus suppression check.
- Search query goes to `GET /search?q=...`; for MVP a simple SQL ILIKE is sufficient — Postgres FTS deferred.

### Size

**M** (2-3 days). Shortcut wiring, help modal, search backend.

### Dependencies

- US-08 (issues to create/search).

---

## US-13: Contributor clones, runs, and ships a change

- **job_id**: jtbd-outcome-3 (Minimize contributor time-to-meaningful-change)

### Story

As a **contributor** (Jamal, a Rust developer who wants to add a label-colour-picker), I want the README to walk me from `git clone` to a green local test run in 10 minutes on a fresh laptop, so that I can make a meaningful change to Foundry on day one.

### Elevator Pitch

- **Before**: Jamal clones the repo and reads `README.md`. The quickstart section has 5 commands (`cargo install sqlx-cli`, `cargo build`, `docker compose up postgres`, `sqlx migrate run`, `cargo test`).
- **After**: He runs all 5 in sequence; tests pass; he edits one line in `src/templates/index.html`, sees the change at `localhost:3000` after a recompile.
- **Decision enabled**: Jamal decides Foundry is contributor-friendly and submits a PR.

### Problem

JTBD outcome #3: a single Rust dev should be productive in a day. Most OSS trackers fail this because their dev environment requires Redis, S3-compatible storage, a JS toolchain, and 30 minutes of yak-shaving. Foundry's stack (Postgres + cargo) makes this achievable; the README must document it cleanly.

### Who

- **Jamal Okafor**, Rust developer at a different company, drawn by the AGPLv3 community.
- **Context**: Fresh MacBook with Homebrew and Rust toolchain installed.
- **Motivation**: Contribute a feature.

### Solution

- README quickstart section with exactly the 5 commands above.
- Required Postgres can run via `docker compose up postgres` (no full app needed for tests).
- `cargo test` runs unit + integration tests against the Postgres container.
- Dev hot-reload via `cargo watch` documented but optional.

### Domain Examples

#### 1. Happy path — fresh MacBook

Jamal on macOS 14 with Rust 1.83 and Docker Desktop. Clone, build, test in 9 minutes 12 seconds. Tests pass.

#### 2. Edge case — Linux without Docker

Sasha runs Arch with podman, no Docker. The README mentions a `podman-compose` recipe as an alternative; she follows it and reaches green tests in 12 minutes.

#### 3. Error path — outdated Rust toolchain

Pat has Rust 1.75 installed. `cargo build` fails with a clear "this crate requires Rust ≥ 1.83". README's prerequisites section names the minimum version.

### UAT Scenarios

```gherkin
Scenario: New contributor reaches green tests in ≤10 minutes
  Given Jamal has a fresh MacBook with Rust 1.83 and Docker Desktop installed
  When Jamal follows the README quickstart from "git clone" to "cargo test"
  Then within 10 minutes, "cargo test" reports all tests passing
  And no manual configuration outside the documented commands was required

Scenario: Quickstart commands are documented and exhaustive
  Given the README has a "Quickstart" section
  Then the section lists every prerequisite, every command, and the expected output
  And nothing is "left as an exercise to the reader"

Scenario: Visible change after one-line edit
  Given Jamal has the dev server running locally
  When Jamal changes the workspace dashboard's heading text in src/templates/dashboard.html
  And Jamal triggers a recompile (cargo run or cargo watch)
  Then Jamal sees the new heading text at http://localhost:3000 within 30 seconds

Scenario: Outdated Rust toolchain produces an actionable error
  Given Jamal has Rust 1.75 (below required 1.83)
  When Jamal runs "cargo build"
  Then the build error contains a clear message about the required minimum Rust version
  And the README lists Rust 1.83 as a prerequisite

Scenario: Integration tests against ephemeral Postgres pass
  Given the test suite uses a Postgres container started by docker compose
  When Jamal runs "cargo test"
  Then the test runner spins up (or assumes) the Postgres test container
  And no test depends on external services beyond Postgres
```

### Acceptance Criteria

- [ ] README "Quickstart" section walks from clone to green tests in 5 commands.
- [ ] No Redis, no S3, no Node toolchain required for the dev loop.
- [ ] Minimum Rust version pinned in `rust-toolchain.toml` and called out in README.
- [ ] Hot-reload path documented (`cargo watch -x run`).
- [ ] CI pipeline reproduces the same quickstart end-to-end on each PR.

### Outcome KPIs

- **Who**: New contributors arriving at the GitHub repo.
- **Does what**: Reach green tests + understand the codebase in one session.
- **By how much**: 50% of first-time clones produce a green local test run within 30 minutes (self-report via opt-in onboarding survey).
- **Measured by**: GitHub action "first PR open time" + onboarding survey.
- **Baseline**: 0.

### Technical Notes

- `sqlx-cli` is a build prerequisite (used for migration files at build time too if `sqlx::query!` macros are used).
- Test fixtures isolate per-test schema via `BEGIN ... ROLLBACK` or per-test schemas.

### Size

**S** (1-2 days). Primarily README + CI pipeline; the underlying stack already supports this.

### Dependencies

- US-01 (basic install works).
- All other stories' code must compile and test.

---

## Story Summary Table

| ID | Title | Persona | Size | Walking-Skeleton? | Outcome KPI category |
|----|-------|---------|------|-------------------|----------------------|
| US-01 | Install in under an hour | Operator | M | YES | Setup time |
| US-02 | Scale to multi-replica | Operator | M | No | Resilience |
| US-03 | Backup + restore | Operator | S | No | Data sovereignty |
| US-04 | Upgrade in place | Operator | M | No | Operability |
| US-05 | Bootstrap workspace + invite | Admin | M | YES | Activation |
| US-06 | Sign in with email + password | User | M | YES | Re-engagement |
| US-07 | Create + view project | User | M | YES | Activation |
| US-08 | File an issue | User | M | YES | Linear-feel speed |
| US-09 | Realtime updates | User | M | No | Linear-feel realtime |
| US-10 | Comment on issue | User | S-M | No | Discussion co-location |
| US-11 | Attach file | User | M | No | Context completeness |
| US-12 | Keyboard navigation | User | M | No | Linear-feel speed |
| US-13 | Contributor onboarding | Contributor | S | No | Contributor productivity |

**Total**: 13 stories, sizing histogram: S = 2, S-M = 1, M = 10, L = 0. Aggregate effort ≈ 28-35 dev-days, fits the 8-12 week target for a 2-developer team.

**Walking skeleton** (minimum demonstrable end-to-end): US-01 + US-05 + US-06 + US-07 + US-08. Five stories, ~13 days.
