# Evolution: team-lifecycle (create, rename and delete a team from the product, and a delete that never loses a project)

**Finalized**: 2026-10-12
**Commits**:
- DISCUSS: `0b961ea`.
- DESIGN: `332408f`.
- DISTILL: `a878fa3`.
- DELIVER roadmap: `b791946`.
- DELIVER steps: `f14e979` → `a76ace2` (13 DES-monitored steps; 01-03 and 03-02 each have a
  second commit after an escalation).
- Test-owner hardening: `5cc220e` (before 04-04), `f4d5179` (after 04-04).

Gates:
- DES integrity: all 13 steps complete.
- End-of-DISTILL review: four reviewers approved (the platform reviewer on cycle 2).
- Roadmap review: approved, 0 findings.
- Adversarial review: APPROVED, 0 blockers. The orchestrator checked the delete lock
  (`FOR UPDATE`, store `teams.rs:748`) and the rename lock (`FOR NO KEY UPDATE`, with the record
  stamped by `clock_timestamp()`) against the code.
- Mutation: store `teams.rs` is at least 80% once the two guards that `f4d5179` covers are
  counted. Its hand bite proofs show each guard mutant failing the new tests. Every other Rust
  file the feature changed passes. The SQL hand tables are 4/4 non-equivalent (0023 and delete),
  6/6 (0024), 8/8 (rename), 5/5 (delete door) and 4/4 (rename refusal). Three waivers, below.
- Full CI: **owed** (see Open / deferred). It was not green in one uninterrupted run.

Feature dir PRESERVED. **Nothing has been pushed, released or deployed** (`main` is 41 commits
ahead of `origin/main`). Two things are owed before release: one uninterrupted Docker-inclusive
CI run, and a final store mutation pass.

**Scope**: a workspace admin can create a team on `/teams` and becomes its lead. A workspace admin
or the team's lead can rename it, and only the name changes: the slug, every URL, every bookmark
and every token scope stay the same. A workspace admin can delete a team through a confirm page
that says what goes with it (members, the default-team setting, scoped machine tokens). A team
with any project is refused, including a project created while the confirm page was open. Team
names follow one rule on both doors and in the database (cap 64). A name with no Latin letters
gets the address `team`, `team-2`, and so on. Renames and deletes leave an append-only record.
Two migrations (0023, 0024). No new crate and no new dependency.

## Business context

Before this feature, bootstrap's "General" was the only team the product ever wrote. Every
workspace provisioned from `/admin/instance/workspaces` had zero teams. Nobody there could be put
on a team, create a project or open a board, and the dashboard's "create your first project"
link pointed at `/team/general/…`, a 404 in those workspaces. Every other team was psql, with no
name rule. Now the provisioned admin reads "This workspace has no teams yet. Create one to start a
project." and is one form away from a board.

This **closes** two items:
- team-membership's follow-up "Team create, rename and delete" (D14, evolution doc 2026-10-10);
- the zero-teams dead end of provisioned workspaces that the same doc recorded.

fix-hide-unreachable-boards' remaining follow-up (show the signed-in account, link a Keycloak
identity) is unrelated and stays open. Neither earlier evolution doc is edited. The closure is
recorded here.

## Key decisions

- **D2 / D3 / OQ-1 (user)**: only a workspace admin creates or deletes. The creator becomes the
  lead in the same transaction (INV-TL-7, born led). A lead may rename their own team. Every
  refused caller, on both verbs, gets the byte-identical uniform 404.
- **OQ-4 (user) / DDD-7 / DDD-8 (ADR-TEAM-LIFECYCLE-002)**: a delete is refused while the team
  has any project. `delete_team_if_empty` locks the team row `FOR UPDATE`, which conflicts with
  the FK `KEY SHARE` that every project insert takes. Only then does it list the projects, in a
  new statement, delete, and record. `FOR NO KEY UPDATE` and a single `DELETE … WHERE NOT EXISTS`
  are pinned negative controls, and each still loses a project. A writer that waits behind the
  delete maps its 23503 by constraint name: a project create, an add or a default set gets the
  404, and a token mint gets the shipped 422.
- **OQ-6 (user) / DDD-11 / DDD-13 (ADR-TEAM-LIFECYCLE-003)**: renames and deletes are audited in
  `team_lifecycle_events` (0023). Rows are written in the same transaction, keyed by workspace,
  with no FK to `teams`, so a record outlives its team. A rename takes `FOR NO KEY UPDATE` and
  records the old name it read under the lock. A byte-equal rename writes nothing.
- **OQ-7 (user) / OQ-2 / DDD-6**: the database refuses a bad new team name at cap 64 (0024: two
  triggers over 0019's unchanged verdict function, plus one HINT arm in the shared trigger
  function). Legacy names are never scanned.
- **Keep 0023's FKs (user, 2026-10-11)**: `team_lifecycle_events` references `workspaces` and
  `users`, so `pg_restore --clean` of a pre-0023 dump over an upgraded database fails. The user
  kept the FKs. The boot that upgrades through 0023 logs one warning (an upgrade only, never a
  fresh install; `upgrade_warnings.rs`). The CHANGELOG says to restore old dumps into a fresh
  database. The user said to delete the old backups. Which ones is confirmed at release, and
  nothing has been deleted yet.
- **DDD-2 / DDD-3 / OQ-3 (ADR-TEAM-LIFECYCLE-001)**: `foundry_core::TeamName` is the third name
  rule. It shares one refused-character predicate and one uniqueness helper with `ProjectName`.
  `mint_team_slug` gives `slugify(name)`, or else the lowest free of `team`, `team-2`, …. The slug
  is minted once in `teams::create_team`, with a bounded retry (3) on a fallback clash only.
- **DDD-14**: rename uniqueness is check-then-write. A duplicate *label* under a concurrent rename
  or create is an accepted residual (the ADR-PROJECT-RENAME-002 posture). The slug cannot be
  corrupted.
- **DDD-15 (ADR-TEAM-LIFECYCLE-004)**: `TeamAccess` gains `Administer`. `can_manage` is unchanged,
  and `can_delete` is for admins only. No shipped behaviour changes.
- **OQ-5 (orchestrator, on the recommendation)**: provisioning gets no automatic first team.
- **DDD-19**: check-arch `team-lifecycle-one-seam` keeps `teams` and `team_lifecycle_events`
  writes in the store and the three store writers out of the app and API. `team-name-one-source`
  keeps the `"Team name must` copy in `foundry-core`.

Bug found and fixed in DELIVER (04-01): rename records were stamped with `now()`, which is the
transaction start, so concurrent renames recorded out of order. They now use
`clock_timestamp()`. Hand mutation R7 (`clock_timestamp()` → `now()`) is killed by the 100-run
chained-old-names race.

## Steps completed (13/13, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Walking skeleton: `TeamName`, `mint_team_slug`, `create_team_with_lead`, `teams::create_team`, `POST /teams`, the members page's New project link; Priya creates Homelab's first team and opens its board | `f14e979` |
| 01-02 | The dashboard's "create your first project" link names a team the caller is on (`first_team_slug`) | `23d2174` |
| 01-03 | Create refusals; check-arch `team-name-one-source` and `team-lifecycle-one-seam` | `25857a7` |
| 01-03 | Un-pended the create-door admin matrix and the cross-workspace create (scenario 9a) | `e44a2da` |
| 02-01 | Migration 0023, the probe line, `delete_team_if_empty`, the 23503 mappings, the 0023 upgrade warning; store-proven (100-run race) | `d803331` |
| 02-02 | `TeamAccess::Administer`; the delete page and the delete for a workspace admin; the delete door's refusal matrix | `3e43e21` |
| 02-03 | The confirm page's default-team and scoped-token lines; what a delete takes with it; stale pages | `f8f8f7f` |
| 02-04 | A team with projects is refused (200 page, or 409 at commit); a project create on a vanished team is the uniform 404 | `7730426` |
| 03-01 | Migration 0024: the database refuses a bad new team name under its named arm | `48c1045` |
| 03-02 | The rule at the create door: the fallback address and the create races (30, 32, 35-38) | `e3ac58d` |
| 03-02 | Un-pended 31, 33 and 34; zero `@pending` in file 03 | `c30622a` |
| 04-01 | `rename_team_with_audit`, store-proven; the last store scaffold removed; `clock_timestamp()` fix | `30e043b` |
| 04-02 | Rename on the members page for an admin and the team's lead; links keep working | `cd52dd5` |
| 04-03 | Rename refusals in the rule's own words with the text kept; the 404 matrix; a demoted lead's stale page; CSRF | `b36a8c9` |
| 04-04 | CHANGELOG, burn-down (0 `@pending`, 0 pending ignores, no team-lifecycle `SCAFFOLD`), mutation report | `a76ace2` |
| — | Test owner: the `first_team_slug` tie-break and the `is_fk_violation` gap closed; past-tense DISTILL comments | `5cc220e` |
| — | Test owner: an add or a default set refused for another reason is an error, not `TeamGone` | `f4d5179` |

Escalations, all resolved through the test owner (the acceptance designer), never by a crafter
editing tests outside its step:
1. **A false "Back-End" slug premise (01-01).** `slugify` keeps hyphens, so "Back-End" derives
   `back-end` and is unique. The collision example became "Backend!" everywhere (DISTILL
   correction 5).
2. **The Hugo row in create scenario 9 was an acceptance-test defect (01-03).** Under D11, his
   create lands in Globex. The row was replaced by isolation scenario 9a (DISTILL correction 6).
3. **The scenario-24 harness before-image gap** : fixed by the test owner. The roadmap
   records the scenario's move between steps (`8362368`).
4. **The restore test broken by 0023's FKs (02-01).** This led to the user's decision to keep the
   FKs, warn on upgrade and document the fresh-database restore (above).
5. **The name-db-checks restore step pinned four triggers (03-01).** Extended to six.
6. **Three tl-03 harness defects (03-02):**
   - `page_at` reused a create's re-render served at `/teams`;
   - NUL was compared as U+FFFD;
   - the `[64×a]` name mark was expanded wrongly.
7. **Mutation gaps closed:**
   - `MintedSlug::as_str`;
   - the store create-path survivors;
   - the `first_team_slug` tie-break;
   - `is_fk_violation`;
   - the add and default-set constraint-name guards.

## Lessons

1. **A worked example can carry a false premise about a shipped function.** "Back-End collides
   with Backend" read as obvious and was wrong: `slugify` keeps hyphens. The crafter's GREEN
   failure caught it, not review. Collision examples should be checked against the real
   `slugify` (or a one-line unit test) when they are written in DISCUSS or DISTILL.
2. **`now()` is not a clock in a transaction.** It is the transaction's start, so two
   overlapping renames recorded out of commit order. An append-only record that claims an order
   needs `clock_timestamp()`, and a race test that checks the chain, which is what found it.
3. **A new FK can break restoring a backup.** 0023's references to `workspaces` and `users` made
   `pg_restore --clean` of a pre-0023 dump fail over an upgraded database. Restore compatibility
   is a migration property. Name it in the design's rolling-deploy section, not only in the
   rollback notes.
4. **Tests that pin "all the triggers" break on the next rule.** name-db-checks pinned four
   triggers, as team-membership's `version >= 19` pinned two migrations. Both had to grow. A pin
   should name its members, and the delivery that adds a member owns updating it.
5. **Authorization outlines need a tenancy column.** Hugo, an admin elsewhere, is not a refused
   caller of a session-scoped create. He acts in his own workspace. A refusal matrix row for a
   session-scoped verb has to say whose workspace it is.
6. **Machine load and Docker port publishing are environment faults.** The canzan-lift session's
   mutation runs kept the load average at 30-70. Docker Desktop's port publishing became
   intermittent (a fresh `docker run -P` reported no port), and a restart (user-approved,
   2026-10-12) only partly helped. PortNotExposed and PoolTimedOut failures in an unmutated
   baseline and in untouched tests are not regressions. They also make cargo-mutants' "caught"
   untrustworthy, so the closing re-run was stopped rather than reported.
7. **Don't run a hand-mutation agent and a lane-running crafter at once.** They share one working
   tree (team-membership lesson 6, repeated here). Serialise them.

## Measured KPIs

The acceptance and store halves are measured. Every dev and prod half is owed, because nothing is
deployed.

- **TL-KPI-1** (0 zero-team workspaces): the walking skeleton (scenario 1) creates Homelab's first
  team and opens a board, and scenario 3 checks the dashboard link. The dev and prod baselines and
  the slice-01 dogfood are owed.
- **TL-KPI-2** (0 hand-written `teams` writes): the create and rename `@kpi` scenarios are green.
  The metric itself is an operator report after release, so it is owed.
- **TL-KPI-3** (0 projects lost to a team delete):
  - store: the 100-run delete-versus-project-create race (0 lost projects, 0 unmapped errors), and
    both negative controls still lose a project;
  - hand mutations H1-H3 and H5 killed;
  - HTTP: scenarios 25-29, with the race run 5 times.

  The dogfood's before-and-after project counts are owed.
- **TL-KPI-4** (byte-identical 404s, 0 rows written): every `@error @kpi` outline in tl-01, tl-02
  and tl-04 is green, each with a control proving the hidden team or door exists.
- **TL-KPI-5** (refusals of a delete with projects, the demand signal): owed, as an operator
  count over the first 30 days after release.
- **TL-KPI-6** (renames keep links; one verdict on both doors):
  - scenario 42: the members page and the board answer 200 at the old address, with the slug
    unchanged;
  - the tl-03 and tl-04 rule outlines give the same copy on both doors;
  - the core property tests (TeamName's control verdict equals ProjectName's and WorkspaceName's;
    create and rename parity);
  - 0024's S1-S6 killed.

  The slice-04 bookmark dogfood is owed.

## Permanent artifacts

- `docs/product/architecture/adr-team-lifecycle-001-team-name-rule-and-create-use-case.md`
- `docs/product/architecture/adr-team-lifecycle-002-delete-refuses-while-projects-exist.md`
- `docs/product/architecture/adr-team-lifecycle-003-team-lifecycle-events-and-rename-transaction.md`
- `docs/product/architecture/adr-team-lifecycle-004-team-access-administer-arm.md`
- `docs/product/architecture/brief.md`: "Team lifecycle: create, rename, delete", now with a
  shipped component inventory
- `docs/product/jobs.yaml`: `job-team-lifecycle`
- `docs/product/outcomes/registry.yaml`: OUT-24, OUT-25, OUT-26; OUT-20 amended
- `docs/product/kpi-contracts.yaml`: `team-lifecycle`, TL-KPI-1..6
- `CHANGELOG.md` `[Unreleased]`: Added, Changed, and migration notes for 0023 (including the
  restore caveat) and 0024 (including OQ-D6's read-only counts: dev and prod each hold 1 team,
  with 0 rule breaks and 0 case-insensitive duplicates)
- `docs/feature/team-lifecycle/`: the full wave history, including `deliver/mutation-report.md`,
  `deliver/finalize-owed.md` and `distill/red-classification.md`

The ADRs already live in `docs/product/architecture/` (house layout), and the lean layout has no
`design/`, `discuss/journey-*` or `distill/walking-skeleton.md` files, so nothing else needed
migrating.

## Open / deferred

Owed before or at release:
- **One uninterrupted `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`.** At 04-04, run 1 reached
  acceptance with 1313/1314 passing. The one failure was a browser WaitTimeout that passed when
  run alone. After the Docker Desktop restart, later runs failed only at Postgres connect
  (PortNotExposed / PoolTimedOut), in an unmutated baseline and in untouched tests.
- **A final `cargo mutants` pass** on store `teams.rs` and `lib.rs`, confirming the kills in
  `5cc220e` and `f4d5179`, and on the four services files. `deliver/mutation-report.md` still
  shows the two guard survivors as open. They were closed afterwards by `f4d5179`, whose hand
  bite proofs are recorded in `deliver/finalize-owed.md`.
- **The old-backup deletion decision**: which pre-0023 backups to delete is confirmed with the
  user at release. Nothing has been deleted.
- **Dogfoods on dev after release** (prod after approval):
  - slice 01: in a zero-team provisioned workspace, create Ops, then Homelab Ops (OPS), and open
    its board;
  - slice 02: delete a typo team, and see a team with a project refused, with project counts the
    same before and after;
  - slice 04: rename a team and reopen a bookmark taken before the rename.
- **KPI-TL baselines (read-only, dev and prod)**: workspaces with zero teams (TL-KPI-1), and
  project counts around each dogfood delete (TL-KPI-3).

Follow-ups:
- **The `team_not_found_page` copies** in `crates/foundry-app/src/keyboard.rs` and
  `crates/foundry-app/src/comments.rs` still answer a separate "Team not found" page. Project
  create now uses the uniform 404; these should follow.
- **OQ-D4**: `admin_tokens::resolve_scope` resolves a typed team *name* with
  `find_team_by_slug(slugify(raw))`. After a rename, or for a fallback-slug team, a typed name no
  longer resolves. The UI picker submits ids, so no shipped flow breaks.
- **A tracing-capture test harness** to kill W1 (`is_name_rule_refusal`, log-only) and
  team-membership's H8.
- **Delete records still take `created_at DEFAULT now()`** (the transaction start). A delete that
  waited on a concurrent rename's lock can therefore record an earlier time than that rename's
  `clock_timestamp()` record. Nothing reads the order yet. If an audit view ever does, stamp
  deletes with `clock_timestamp()` too (found at finalize; low).
- **OQ-4 (b)/(c)**: project delete or move, and a cascade delete of a team with projects. TL-KPI-5
  is the demand signal.
- Still open from team-membership: a per-invite team choice (OQ-7), OQ-D9 (a user hard-delete of
  a sole lead), and the auto-generated constraint names on `projects`.

Waivers (mutation):
- **W1** `is_name_rule_refusal`: log-only.
- **W2** `Store::migrate -> Ok(())`: killed only by smoke.
- **W3**: the locked read and `DELETE` merged into one statement is equivalent once the row is
  locked `FOR UPDATE`.
