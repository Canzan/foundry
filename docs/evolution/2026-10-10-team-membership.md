# Evolution: team-membership (a way onto teams, a team that never loses its last lead, and a default team for newcomers)

**Finalized**: 2026-10-10
**Commits**:
- DISCUSS: `e4c8bcd`, `8889f63`.
- DESIGN: `59d2633`.
- DISTILL: `234eaae`.
- DELIVER roadmap: `ecd08ae`.
- DELIVER steps: `11785f0` → `16608ee` (12 DES-monitored steps, plus the removal-guard hardening
  `eda1755` between 03-01 and 03-02).

Gates:
- DES integrity: all 12 steps have complete traces.
- Roadmap review: conditionally approved. The orchestrator overruled 1 blocker and 2 highs with
  evidence: scenario ownership is by name; the tm-01 lane in 02-01 is a labelled regression check;
  and the cascade exemption is also proven directly in `team_lead_guard.rs`.
- End-of-DISTILL review approved (DEVOPS conditionally; its one high was already resolved in
  DISTILL).
- Adversarial review APPROVED: 0 blocker, 0 high, 0 medium findings; 1 low (H8, below).
- Mutation: 100% of viable mutants on every Rust production file the feature changed. SQL hand
  tables: 0021 11/11, 0022 8/9 (H8 survives).

Feature dir PRESERVED. **Nothing is deployed or pushed.** The user said no pushes or releases until
told, so the dev and prod checks this feature owes are listed under Open / deferred.

**Scope**: a workspace admin, or a team's lead, can open `/teams` and a members page per team, add
a workspace member to the team, make someone a lead or a member, and take someone off it. A team
that has a lead never loses its last one, whether through the product, two admins at once, or
psql. Removing someone from the workspace also takes them off its teams, and is refused while
they are any team's last lead. A workspace admin can pick a default team, and a newcomer who
accepts an invite or signs in with Keycloak for the first time lands on it as a member. Two
migrations (0021, 0022), no new crate, no new dependency.

## Business context

Before this feature, nothing in the product put anyone on a team. Bootstrap made the first user
lead of General, and `foundry doctor add-test-user` put a user on every team in every workspace.
Everything else was psql. fix-hide-unreachable-boards (2026-10-09) made that visible: the
operator's Keycloak-provisioned account landed on no team, and the dashboard told her to "ask a
workspace admin to add you". She is the workspace admin, and there was no way to do it. Its first
follow-up, "a way to join teams", is **closed by this feature**.

The `lead` role also meant nothing: no handler read it. Leads now manage their own team, which is
why "never leaderless" had to hold under concurrency, and for every caller.

## Key decisions

- **D2 / OQ-2 (user)**: a workspace admin manages any team in the workspace; a lead manages only
  their own team. Leads are peers: a lead may demote or remove any lead, including themselves or a
  workspace admin, unless that leaves the team leaderless (DDD-23: no special case).
- **D5 / D9 / D10**: every refusal on the new routes is byte-identical to the uniform 404. Tenancy
  comes from the session. Managing a team grants no board access to it. Instance admins get no
  implicit access (OQ-1).
- **DDD-3 (ADR-TEAM-MEMBERSHIP-003)**: one pure policy, `foundry_core::TeamAccess::of(is_admin,
  Option<TeamRole>) → {None, View, Manage}`, fed by one store read and applied only inside the
  `foundry_services::teams` use-cases. Slice 04 changed exactly one arm. check-arch
  `team-membership-one-seam` keeps `team_memberships` SQL in the store and the store's team
  writes out of the app and API crates.
- **DDD-6 (ADR-TEAM-MEMBERSHIP-001)**: never leaderless is a team-row `FOR NO KEY UPDATE` in id
  order, an exact pre-read and a typed `LastLead`, **plus** 0021's `BEFORE` row triggers taking
  the same lock and raising 23514 `team_memberships_team_keeps_a_lead` for any other caller. Team
  and workspace deletes are exempt (the team is gone). SERIALIZABLE and advisory locks were
  rejected, as in board-lane-002.
- **DDD-7**: "lead" means a lead row. 0021 deletes orphan team rows (people who already left the
  workspace) once, with a NOTICE count, before installing the triggers.
- **DDD-10 / DDD-11 (OQ-9, user)**: an add takes `FOR KEY SHARE` on the target's workspace
  membership; workspace removal takes it `FOR UPDATE` first, then the team locks, then the
  sole-lead read, then the deletes. Nobody ends up on a team of a workspace they are not in.
- **DDD-8**: the last-lead refusal is a stated reason, not a 404: 409 with the members page and the
  D15 copy, or 409 `text/plain` on `POST /workspace/members/remove`, with no `member_removed`
  notification.
- **DDD-12 (ADR-TEAM-MEMBERSHIP-002)**: `workspaces.default_team_id` with a composite FK to
  `teams (workspace_id, id)` `ON DELETE SET NULL (default_team_id)`. A cross-workspace default is
  impossible for every caller, and deleting the team clears only the default. Postgres 15 or later.
- **DDD-13 / D17**: placement is one store helper, run in a savepoint inside the invite-accept and
  first-provisioning transactions. A failure logs one ids-only warn and never fails the accept or
  the sign-in. Linking an existing account and the first-admin invite place no one; there is no
  backfill (OQ-8).

Pre-push CI: `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` passed in ONE uninterrupted run. That
covers fmt, clippy, check-arch, the release build, workspace tests (523 passed), cargo-deny, and
acceptance on all tags including browser and docker-compose: 1219/1219 scenarios across 92
features, 8285 steps.

## Steps completed (12/12, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Walking skeleton: `TeamRole`/`TeamAccess`, the facts read, `add_team_member`, `/teams`, members page, Add; the Keycloak account opens Sandbox | `11785f0` |
| 01-02 | Teams index scoped to own teams, read-only view for members, admin view grants no board, refusals uniform | `2e9a5f8` |
| 01-03 | Add list, idempotent re-add, add refusals; check-arch `team-membership-one-seam` | `3643a45` |
| 02-01 | Migration 0021 lead guard; `set_team_role`, store-proven (100-run demote race) | `021cd47` |
| 02-02 | Role changes on the members page; last lead refused (409) | `6118e23` |
| 03-01 | `remove_team_member`; `remove_workspace_member` as one guarded transaction (OQ-9) | `1ec818f` |
| — | Removal-guard hardening: guard-dropped store tests kill the `remove_team_member` survivors | `eda1755` |
| 03-02 | Remove on the members page; last lead refused | `a41fd2c` |
| 04-01 | A lead manages her own team, and only it (one policy arm) | `ea653cf` |
| 05-01 | Migration 0022, the probe line, `default_team`/`set_default_team`, invite placement | `ca34629` |
| 05-02 | Default-team setting on `/teams`; invitees land on it | `ab637e5` |
| 06-01 | First Keycloak sign-in lands the newcomer on the default team | `42096e9` |
| 06-02 | CHANGELOG, burn-down, mutation report | `16608ee` |

Escalations, all resolved through the test owner (the acceptance designer), never by the crafter
editing tests outside its step:
- `name_rule_in_database.rs` pinned `_sqlx_migrations WHERE version >= 19` to `[19, 20]`, so any
  new migration broke it (02-01 GREEN failed on it). Narrowed to `IN (19, 20)`.
- A hardening test used `DISABLE TRIGGER`, which check-arch DDD-12b bans. It now uses
  `DROP TRIGGER`.
- 0022's probe line broke `probe_schema_scoping.rs`'s hand-built healthy schemas. A
  `workspaces(id, default_team_id)` table was added to the healthy sides only.
- Mutation survivors in the 23514 → `LastLead` mapping (02-01: 6/14 at first) and in the removal
  guard (03-01: 6/11) were killed by fault-injection and guard-dropped store tests, to 100%.
- The trigger's own lock (SQL M1) was proven by a 100-run hand-typed SQL demotion race.

## Lessons

1. **A test that pins "everything from version N on" breaks on the next migration.**
   `version >= 19` was really "these two migrations"; saying `IN (19, 20)` says what it means.
   Any test that reads `_sqlx_migrations` should name its versions.
2. **A trigger backstop needs its own proof.** The store takes the team lock before the trigger
   ever runs, so no store-path test could tell whether the trigger locked. Only a hand-typed SQL
   race, bypassing the store, killed "trigger lock removed" (M1).
3. **Two guards for one rule hide each other.** The store's pre-read and the trigger both refuse
   the last lead, so with either one mutated away the behavioural tests still passed (02-01's
   23514 → `LastLead` mapping at 6/14 at first, 03-01's removal guard at 6/11). Guard-dropped
   tests (`DROP TRIGGER`, then expect the store's typed outcome) and injected refusals (the lead
   rule maps to `LastLead`; any other refusal is an error) made each guard observable on its own.
4. **A correct user outcome can hide an operator bug.** H8 (placement without
   `default_team_id IS NOT NULL`) leaves every newcomer exactly where they should be, and logs a
   placement warning for each one in a workspace with no default, which after the upgrade is
   every workspace. It is not equivalent; it needs a log-capturing test.
5. **Machine load is an environment fault, and so is another session's leak.** Another session's
   test suite (jeffbaileyblog zsh-wasm-playground pytest, `docker run --rm -it … zsh -l` never
   exits) leaked about 1,500 `zswp-oracle` containers; the user approved stopping them. Under a
   load average of 30-100, smoke and store runs needed `RUST_TEST_THREADS=1` and cargo-mutants
   needed `-j 1`. PortNotExposed and PoolTimedOut flakes on untouched tests were rerun, not
   filed.
6. **Don't run hand mutation in parallel with a lane-running crafter.** A mutated store file sat in
   the shared tree while a lane ran. Hand mutation and lanes share one working tree; serialise
   them.

## Measured KPIs

Acceptance halves are measured; every dev/prod half is owed, because nothing is deployed.

- **TM-KPI-1** (0 workspace members on no team): the walking skeleton proves the path. The dev and
  prod baselines and the D4 dogfood are owed.
- **TM-KPI-2** (100% of newcomers open a board when a default is set): tm-05 and tm-06 placement
  scenarios, plus the D17 floor (no default, deleted default). The real-newcomer dogfoods are owed.
- **TM-KPI-3** (0 leaderless teams through the product): store races, 100 runs each, for
  demote/demote, remove/demote, remove/remove and workspace-removal/demote, plus the 100-run
  add/workspace-removal race (INV-5) and the 100-run hand-typed SQL race. HTTP races ran 5 times
  each. The dev/prod leaderless baseline is owed.
- **TM-KPI-4** (byte-identical 404s, 0 rows written): every `@error @kpi` outline in tm-01..05 is
  green.
- **TM-KPI-5** (0 hand-written membership writes): owed, as an operator report after release.

## Permanent artifacts

- `docs/product/architecture/adr-team-membership-001-never-leaderless-lock-and-trigger.md`
- `docs/product/architecture/adr-team-membership-002-default-team-column-composite-fk.md`
- `docs/product/architecture/adr-team-membership-003-one-team-authority-policy.md`
- `docs/product/architecture/brief.md`: "Teams, team authority and the never-leaderless guard", now
  with a shipped component inventory
- `docs/product/jobs.yaml`: `job-team-membership`; the persona file's two-accounts characteristic
- `docs/product/outcomes/registry.yaml`: OUT-21, OUT-22 (`related: [OUT-5, OUT-21]`), OUT-23
- `docs/product/kpi-contracts.yaml`: `team-membership`, TM-KPI-1..5
- `CHANGELOG.md` `[Unreleased]`: Added, Changed, and migration notes for 0021 and 0022
- `docs/feature/team-membership/`: the full wave history, including `deliver/mutation-report.md`
  and `distill/red-classification.md`

## Open / deferred

Owed before or at release (nothing is deployed or pushed):
- **OQ-D4**: read-only orphan team-row counts on dev and prod before 0021 ships (expected 0). The
  query is in the CHANGELOG migration note.
- **The D4 dogfood on dev**: as `jeffabailey@gmail.com`, add `jeff@unintelligent-design.us` to
  General; then, signed in with Keycloak as that account, `/team/general/project/sandbox` answers
  200.
- **KPI-1 and KPI-3 baselines** (members on no team; teams with no lead) on dev and prod.
- **The slice-02 and slice-05 dogfoods** (promote and demote on General, with the sole-lead
  refusal; set General as default, invite a real address, open Sandbox), and the slice-06
  first-Keycloak-sign-in dogfood.

Follow-ups:
- **H8**: a store test that captures `tracing` output and asserts that an invite accept in a
  workspace with no default logs no warning. Route it to the acceptance designer; no capture
  harness exists in `crates/foundry-store/tests` yet.
- **Team create, rename and delete** (D14, out of scope): workspaces provisioned from
  `/admin/instance/workspaces` have zero teams, so nobody can be put on a team there.
- **A per-invite team choice** (OQ-7).
- **Show the signed-in account, and link a Keycloak identity to an existing account**
  (fix-hide-unreachable-boards' second follow-up, still open).
- **OQ-D9**: a future user hard-delete of a sole lead will be refused by the 0021 trigger until
  that feature moves leadership first.
- **Constraint names on `projects`**: still Postgres auto-generated (the project-name-rule
  follow-up).
