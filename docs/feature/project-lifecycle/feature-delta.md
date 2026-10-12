<!-- markdownlint-disable MD024 -->
# Feature Delta: project-lifecycle

A team's lead, or a workspace admin, can rename a project and delete it from
inside its team. Create and read already ship (`/team/{team_slug}/projects/new`
and the board). Today only an instance super-admin can rename a project, from
`/admin/instance/workspaces`, and nothing can delete one: a project lives until
psql removes it, and a team that holds one can never be deleted
(team-lifecycle OQ-4, TL-KPI-5). A delete here is a **soft delete**: the project
disappears for everyone, nothing under it is erased, and the instance operator
can bring it back.

## Wave: DISCUSS

Product owner: Luna (nw-product-owner) | Date: 2026-10-12 | Feature type:
user-facing | Walking skeleton: none (brownfield; every surface it extends
ships) | UX depth: lightweight, happy path plus the main refusals and races |
Research depth: comprehensive (user) | JTBD: yes, a new job | Density: lean,
Tier-1 [REF] only, `expansion_prompt = ask-intelligent` | Requirements
readiness: two decisions locked by the user (D-A, D-B), the rest explored here.

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | `job-instance-project-rename` (the super-admin's door, drift), `job-project-naming` (one rule on both doors), `job-team-lifecycle` (team create/rename/delete; its delete refuses while projects exist). None covers a team retiring its own project. A new job, `job-project-lifecycle`, is appended. |
| `docs/product/journeys/*` | ✓ | Three journeys (theme, issue-card-delete, card-drag-drop). `journey-issue-card-delete.yaml` is the closest shape (a delete with a confirm that counts). `journey-project-lifecycle.yaml` is added, because this feature has a delete and a restore whose emotional arc matters. team-lifecycle added none. |
| `docs/product/personas/persona-instance-operator.yaml` | ✓ | Priya Raman: instance operator with shell access (the restore door's user), workspace admin of Canzan Labs, member of Backend. Gains `also_referenced_by` and the job id only. Marco (`persona-team-member-foil`) is the 404 foil. |
| `docs/product/outcomes/registry.yaml` | ✓ | OUT-1 (instance project rename, slug-stable), OUT-19 (the project-name rule), OUT-20 (DB name triggers), OUT-24..26 (team lifecycle). This feature amends OUT-1 and OUT-25 and adds new rows **at DISTILL**, as team-lifecycle did. |
| `docs/product/kpi-contracts.yaml` | ✓ | `team-lifecycle` TL-KPI-5 ("admins who try to delete a team with projects hit the refusal; above 0 is evidence for project delete or move") is this feature's demand signal. A `project-lifecycle` key is appended at DISTILL. |
| `docs/feature/team-lifecycle/feature-delta.md` (all waves) | ✓ | **The format precedent.** D4 (slugs are identity), D5 (uniform 404), D8 (GET confirm + POST), D11 (tenancy from the session), D12 (copy locked), D15 (byte-equal rename writes nothing), OQ-4 (refuse a team delete while it has projects), OQ-6 (`team_lifecycle_events`), INV-TL-1 (no lost project). Followed section for section. |
| `docs/evolution/2026-10-12-team-lifecycle.md` | ✓ | Follow-up "OQ-4 (b)/(c): project delete or move" is **partly closed** here (delete; move stays out). Lesson 1 (check collision examples against the real `slugify`) applied: every slug in this document was derived by hand from `slugify`'s lowercase-and-hyphen rule. Lesson 5 (tenancy column in authz outlines) applied: the matrix names Hugo, a Globex admin. |
| `docs/evolution/2026-10-10-team-membership.md` | ✓ | D9 (managing a team grants no board access), DDD-3 (`TeamAccess::of`, now with `Administer`). Project rename and delete use `Manage`, not `Administer` (D-B). |
| `docs/feature/project-name-rule/feature-delta.md` | ✓ | D2 the rule (trim, non-empty, no refused character, at most 256, unique within the team), D11 the refusal shapes, the copy is `ProjectNameError`'s `Display`. Reused unchanged. |
| `docs/feature/instance-admin-project-rename/feature-delta.md` | ✓ | D1 display name only (slug, key prefix, issue keys unchanged), D5 uniform 404, D7 the accepted duplicate-label race. That door stays; this feature adds the team's own door beside it. |
| `docs/product/architecture/brief.md`, `adr-project-rename-001/002`, `adr-team-lifecycle-002/003`, `adr-issue-delete-002` | ✓ | "Names are labels; slugs are identity"; rename is a services use-case; delete refusal under `FOR UPDATE`; the audit table pattern; GET+POST confirm. Read for grounding, not changed. |
| DIVERGE (`docs/feature/project-lifecycle/diverge/*`), DISCOVER, `docs/product/vision.md` | ⊘ | Do not exist. Grounded in the user's locked decisions (D-A, D-B), the team-lifecycle follow-up and code reading. **Recorded risk** (see Wave Decisions), as in every precedent. |

No contradiction with prior evidence. One tension is surfaced, not resolved:
team-lifecycle's INV-TL-1 says "no project row is ever removed by a team
delete", and its D9 says a team delete never destroys a project the admin was
not shown. Once a project can be soft-deleted, a team whose only projects are
deleted must either stay undeletable or do something with those rows. That is
OQ-1.

### [REF] Lifecycle inventory (code-grounded)

| Fact | Source |
|---|---|
| `projects(id, team_id → teams ON DELETE CASCADE, workspace_id, name, slug, key_prefix ^[A-Z]{2,6}$, next_issue_number, UNIQUE(team_id, slug), UNIQUE(workspace_id, key_prefix))`. No deleted marker exists. | `0001_init.sql:51-62` |
| Everything under a project cascades from it: `issues` (0001), `comments` (0004), `issue_attachments` stored inline (0005), `issue_change_events` (0013), `lanes` (0015). | migrations |
| Soft-delete precedent: `comments.deleted_at` + `deleted_by` (0006), hidden from the live list, restored by `foundry doctor restore-comment <uuid>` (`admin_cli.rs:446`, exit 0 restored / 2 bad uuid / 3 DB / 4 not restorable). A 90-day GC purges tombstoned comments (`gc_tombstoned_comments`). | `0006`, `admin_cli.rs`, `lib.rs:3116` |
| Project rename today: `POST /admin/instance/projects/{project_id}/rename`, instance admin only, `ProjectName` rule, byte-equal is a no-op, `UPDATE projects SET name` with **no lock and no record**. | `lib.rs:4296-4345`, `foundry-services/src/projects.rs:89-127` |
| Project delete: **none** in production code (`DELETE FROM projects` appears in acceptance fixtures only). | grep |
| Team delete: `delete_team_if_empty` locks the team `FOR UPDATE`, lists `SELECT name, key_prefix FROM projects WHERE team_id = $1`, refuses on any row, else deletes and records. | `foundry-store/src/teams.rs:589, 740-785` |
| Project addresses. Web: `/team/{t}/project/{p}` (board), `/report` (+ `?format=csv`), `/issues` (create), `/issues/new`, `/search`, `/events` (SSE), `/issues/{n}`, `/issues/{n}/state`, `/edit`, `/delete`, `/comments`, `/comments/{id}` (+ `/edit`), `/lanes/{l}/delete`, `/edit`, `/insert/{side}`, `/move`. API: `/api/v1/teams/{t}/projects/{p}/issues`, `/issues/{n}`, `/history`, `/comments`, `/comments/{id}`, `/tokens`, `/tokens/{jti}`. | `foundry-app/src/lib.rs:564-710`, `foundry-api/src/lib.rs:312-338` |
| Project readers in the store that a deleted marker must reach: `list_projects_for_workspace` (dashboard, nav), `find_project_by_slug`, `find_issue_by_team_project_number`, the issue joins at `lib.rs:1973, 2251, 2315, 2381`, `lanes.rs:302, 361, 664`, `list_projects_for_instance`, `project_rename_context`, `TEAM_PROJECTS_SQL`, `export_workspace`. About 15 sites. | grep `FROM projects` / `JOIN projects` in `src` |
| The team members page has no project list today. It shows **New project** to anyone on the team, **Rename** to `Manage`, **Delete team** to `Administer`. | `templates/team_members.html` |
| `TeamAccess::of(is_admin, role)`: admin → `Administer`, lead → `Manage`, member → `View`. `can_manage` is true for admin and lead. | `foundry-core/src/team_membership.rs:52-76` |
| `team_lifecycle_events(workspace_id, team_id (no FK), team_slug, kind renamed\|deleted, actor_id NOT NULL, name, new_name, created_at)`, kept out of the per-workspace export. | `0023_team_lifecycle_events.sql` |
| Workspace roles can be changed only by hand-written SQL (no `foundry doctor` door). Found 2026-10-12 when the user's Keycloak account had to be made a workspace admin by SQL. | user report; `main.rs:800-973` lists the doctor commands |

### [REF] Persona

- **Lena Park**: lead of Backend in Canzan Labs, a workspace `member` (not an admin). The primary actor: they own Backend's projects day to day. Referenced inline, as in team-lifecycle.
- **Priya Raman** (`persona-instance-operator`): workspace admin of Canzan Labs and member of Backend, and the instance operator with shell access. They act as a workspace admin in the web stories and as the operator in the restore story.
- **Sam Okafor**: a member of Backend (not a lead). They can see Backend's projects and must not be able to rename or delete them.
- **Marco** (`persona-team-member-foil`): signed in, not on Backend. The uniform-404 foil.
- **Hugo Brandt**: workspace admin of Globex, a different workspace on the same instance. The tenancy foil (team-lifecycle lesson 5): an admin elsewhere is a stranger here.

### [REF] JTBD

**job_id: `job-project-lifecycle`** (appended to `docs/product/jobs.yaml`).

One-liner: *When a project on my team no longer fits (its name has drifted, it
was created by mistake, or its work is over), I want to rename it or delete it
myself from the team's page, so our project list matches the work we actually
do, without asking an instance admin, without psql, and without losing a single
issue if I change my mind.*

Job stories:

1. When "Identity Platform" has become the team's access-management work, I want to rename it myself, and every bookmark and every issue key (AUTH-7) should keep working.
2. When "Auth v2 Spike" was created by mistake or its work is over, I want to delete it so it stops cluttering the dashboard, the new-issue picker and the team page, and be told before I commit what disappears.
3. When I deleted a project and realise a week later that one of its issues still mattered, I want it back exactly as it was.
4. When every project of a team is retired, I want to be able to retire the team too (team-lifecycle TL-KPI-5).

Dimensions: **functional**: rename and delete a project from its team's page as
its lead or a workspace admin, under the shipped project-name rule; restore it
through the operator. **Emotional**: deleting stops being frightening, because
nothing is erased. **Social**: the team's project list says what the team
works on now, and nobody has to ask an instance admin to tidy it.

Four forces:

- **Push**: only an instance super-admin can rename a project, from an instance-wide dashboard a lead never sees. No delete exists, so mistaken and finished projects stay on every dashboard, every new-issue picker and every team forever, and a team holding one can never be deleted (TL-KPI-5).
- **Pull**: a **Rename** field and a **Delete project** link next to each project on the team's page, the way the team's own rename and delete already work.
- **Anxiety**: that a delete takes issues, comments and attachments with it for good; that a rename breaks URLs or changes issue keys; that someone without the right can delete a project; that a deleted project still answers on some forgotten URL or through the API.
- **Habit**: asking an instance admin, or psql. The product path must be safer (nothing erased, a restore exists, slugs and keys stay put), not just closer.

### [REF] Scope Assessment: PASS (5 stories, 3 contexts, about 5 days)

| Oversized signal | Fired? | Evidence |
|---|---|---|
| More than 10 user stories | No | 5 |
| More than 3 bounded contexts or modules | No (exactly 3) | the project catalogue (rename, delete, restore, every project-scoped web and API door), teams (the team delete's projects check, OQ-1), the operator CLI (`foundry doctor`) |
| Walking skeleton needs more than 5 integration points | No | No walking skeleton (brownfield). The thinnest slice (01) is one route, one store write, one template section. |
| More than 2 weeks of effort | No | about 5 days |
| Multiple independent outcomes that could ship separately | **Yes** | Rename and delete are independent outcomes |

One of five signals fires, so the verdict is **right-sized**. Rename (slice 01)
could split into its own feature with no rework. The delete-and-hide surface
(slice 02) is the largest piece, because it must close every project address,
and is kept whole on purpose (see Story Map).

### [REF] Locked Decisions

| ID | Decision | Source |
|---|---|---|
| D-A | **Project delete is a soft delete.** The project is hidden. Every address under it answers the uniform 404. Nothing (issues, comments, attachments, change events, lanes, the issue counter) is erased. An admin can restore it. Precedent: `foundry doctor restore-comment <uuid>`. | **User** |
| D-B | **Rename and delete: the team's lead(s) and workspace admins.** The same rule as team rename in v0.14.0 (`TeamAccess` `Manage`). A member, an outsider, a lead of another team, an admin of another workspace, an instance admin who is not a workspace admin and a signed-out visitor get the uniform 404. | **User** |
| D-C | **Out of scope:** moving a project between teams (OQ-4(b) of team-lifecycle), a hard purge, and a cascade delete (OQ-4(c)). | **User** |
| D1 | User-facing feature. JTBD yes. UX depth lightweight. No walking skeleton: every surface it extends ships (board, members page, `TeamAccess`, project rule, doctor CLI). | User |
| D2 | **Rename changes the name only.** `projects.slug` and `key_prefix` never change, so every URL, bookmark and issue key (AUTH-7) keeps working. There is no "change project URL" and no "change key" here. (OQ-2 asks the user to confirm.) | ADR-PROJECT-RENAME-001; team-lifecycle D4; instance-admin-project-rename D1 |
| D3 | **One project-name rule on both rename doors.** The team's new door runs the shipped `ProjectName` rule (OUT-19) with the shipped copy, the project itself excluded from uniqueness, exactly as the instance-admin door does. A byte-equal name writes nothing. A case-only change is a rename. | project-name-rule D2, D11; team-lifecycle D15 |
| D4 | **Every refused request on a new or changed route is the uniform 404**, on both verbs, byte-identical to `resource_not_found_page`: signed out, an unknown team or project slug, another workspace's team, a deleted project, and a caller without the right. A stated reason (a name refusal) is shown only to an authorized actor. | team-lifecycle D5 |
| D5 | **"Hidden" means hidden from everyone, including the person who deleted it and every admin.** After a delete, every web and API address under the project answers exactly as an address that never existed (D4, API: its shipped not-found answer). Every list drops it: the dashboard, the nav, the new-issue project picker, the team page, the instance-admin dashboard, and the team delete's project list (subject to OQ-1). The only ways to see it are the operator CLI (D10) and the database. | D-A; no new oracle |
| D6 | **"Nothing is erased" is exact.** A delete writes the project's deleted marker (who, when) and the record (OQ-3), and changes no other row: issue, comment (live or tombstoned), attachment, change event, lane, outbox row or the issue counter. | D-A |
| D7 | **Delete is a GET confirm page plus a POST, and works with scripting disabled.** The confirm page states the consequence before anything happens: the issue count, that the board, the issues and their keys stop working for everyone, and that nothing is erased and the instance operator can restore it. No typed-name confirmation (OQ-5). | ADR-ISSUE-DELETE-002; team-lifecycle D8 |
| D8 | **The controls live on the team's members page**, in a new **Projects** section. Anyone who can view the team sees each live project as "{name} · {KEY}", linking to its board. A `Manage` caller also sees a prefilled **Project name** field with **Rename**, and a **Delete project** link, per project. The board gains no new control. DESIGN may move them; the copy stays. | PO: the team page is already where the team is managed; a destructive control on the board header was rejected for lanes (board-lane-overflow-menu) |
| D9 | **Tenancy comes from the session.** A project is resolved by (acting workspace, team slug, project slug), live projects only. No workspace or project id is parsed from a web request. | team-lifecycle D11 |
| D10 | **Restore is an operator door, not a web page.** `foundry doctor list-deleted-projects` prints every deleted project; `foundry doctor restore-project <uuid>` restores one against `DATABASE_URL`, with `restore-comment`'s exit codes (0 restored, 2 invalid uuid, 3 database, 4 not restorable). A web restore for workspace admins is a follow-up (OQ-6). | D-A ("precedent: restore-comment") |
| D11 | **A restore puts back exactly what was there.** The same name, slug, key, issue numbers, issue counter, lanes, comments and attachments. Every address answers as it did before the delete. | D-A |
| D12 | **The comment tombstone GC is unchanged.** Comments a user deleted (tombstones) inside a deleted project are purged on their usual 90-day schedule; the project delete neither speeds it up nor pauses it. Live comments are never touched. | PO: the project delete erases nothing *new*; pausing GC would add a special case for no user value |
| D13 | **The per-workspace export includes deleted projects**, marked as deleted. An export is the operator's copy of the data, and D-A says nothing is erased. | PO |
| D14 | **Copy is locked** (DESIGN may change routes, not strings). See [REF] Copy. | PO, for testable ACs |
| D15 | **Routes are proposed, and DESIGN finalizes them.** `POST /team/{team_slug}/project/{project_slug}/rename`, `GET` and `POST /team/{team_slug}/project/{project_slug}/delete`. Plain forms on the shared CSRF layer. Neither collides with a shipped route. | PO, following team-lifecycle D13 |
| D16 | **The instance-admin rename door stays**, unchanged in behaviour except that it never finds a deleted project (uniform 404) and, if OQ-3 is accepted, writes the same rename record. | instance-admin-project-rename D1, D5 |
| D17 | **Notifications queued before a delete are delivered; none arise after it.** A notification already queued when the delete commits reports a change made while the project was live, so it goes out as usual (its link answers 404 until a restore). After the delete no write can reach the project, so no new notification arises. A restore does not replay anything. | PO: holding queued mail would need a new outbox state and a release-on-restore path, for a window of seconds; D6 keeps the queue untouched |

### [REF] Copy (D14)

- Members page, section heading **Projects**. Each row: "{project} · {KEY}" linking to the board. Empty: "{team} has no projects yet." Per row for `Manage`: field **Project name** (prefilled), button **Rename**, link **Delete project**.
- Renamed: "Renamed {old} to {new}." (members page notice).
- Name refusals: the shipped `ProjectNameError` `Display`: "Project name must not be empty" · "Project name must not contain control characters" · "Project name must be at most 256 characters" · "Project name must be unique within the team". Shown in that project row, with the typed text kept.
- Confirm page: heading "Delete {project}?". Then "{project} ({KEY}) has {n} issue(s). After it is deleted, nobody can open its board, its issues or its report, and links to {KEY} issues stop working." Then "Nothing is erased. The instance operator can restore {project} with every issue, comment and attachment." Buttons **Delete project** and **Cancel** (back to the members page).
- Deleted: "Deleted {project}." (members page notice).
- Team delete page, only if OQ-1 is answered (b), when the team has deleted projects and no live ones, one extra line: "{team} also has {k} deleted project(s): {list}. They are kept, but can't be restored after {team} is deleted, and their keys stay taken in {workspace}." The list reads "Prototype Graveyard (PROTO)".
- CLI `list-deleted-projects`, one `key: value` block per project, blank line between blocks, oldest deletion first:
  `project-id: {uuid}` · `workspace: {workspace}` · `team: {team_slug}` · `name: {project}` · `key: {KEY}` · `issues: {n}` · `deleted-at: {RFC 3339 UTC}` · `deleted-by: {email}`. None: `status: none`. Exit 0.
- CLI `restore-project`: stdout `project-id: {uuid}` and `status: restored`, exit 0. Exit 4 with stderr `foundry doctor restore-project: project {uuid} is not deleted — status: not restorable`, or `… not in database — status: not restorable`, or (OQ-1 (b)) `… its team {team_slug} was deleted — status: not restorable`. Exit 2 and 3 as `restore-comment`.
- Pluralization follows the shipped "1 member(s)" style.

### [REF] Open Questions (each has a recommended answer; the stories assume it)

These need the user's answer. Each names the stories it touches.

- **OQ-1: Does a soft-deleted project still block a team delete? (touches US-PL-05 only)** The options:
  - (a) **Yes, it still blocks.** The refusal names it ("Labs still has 1 project(s): Prototype Graveyard (PROTO), deleted. …"). Zero schema impact on teams, but a team can then never be deleted once it has held any project, because there is no purge (D-C). TL-KPI-5's demand stays unmet.
  - (b) **No, it no longer counts. The team delete keeps the deleted projects' rows, detached from the gone team.** Nothing is erased (D-A holds). A detached project can't be restored, because restoring needs a team and moving is out of scope (D-C): `restore-project` refuses it with exit 4 and the reason, and the data waits for a "restore into a team" follow-up. The confirm page says so before the delete. This needs `projects.team_id` to survive its team (a schema change DESIGN chooses: for example a nullable `team_id` allowed only on deleted rows).
  - (c) No, it no longer counts, and the team delete erases them. Rejected: that is the hard purge and cascade delete that D-C puts out of scope, and it breaks D-A.
  - (d) No, it no longer counts, and the deleted projects move to another team. Rejected: that is a move (D-C).

  *Recommended: (b).* It is the only option that honours D-A (nothing erased), D-C (no purge, no move) and the job's fourth story (a team whose work is all retired can be retired). The cost is a schema change on a core foreign key, an orphaned-restore gap (recorded as a follow-up), and the detached projects' keys staying taken in the workspace for good (stated on the confirm page). If the user picks (a), US-PL-05 shrinks to one refusal-copy change and slice 04 becomes half a day.
- **OQ-2: Do the slug and the key stay fixed on a project rename? (US-PL-01)** *Recommended: yes, both.* This is the shipped behaviour of the instance-admin door (D1 there), team-lifecycle D4 for teams, and the brief's "names are labels; slugs are identity". Changing the key would rewrite every issue id (AUTH-7 → ACCESS-7) in comments, commit messages, emails and bookmarks outside foundry. A "change key" or "change URL" is a separate feature if ever wanted.
- **OQ-3: Are renames, deletes and restores recorded? (US-PL-01, -02, -04)** *Recommended: yes, all three*, in an append-only `project_lifecycle_events`-style record mirroring `team_lifecycle_events`, written in the same transaction as the change, kept out of the per-workspace export:
  - renamed: actor · project · old name → new name · when;
  - deleted: actor · project · name · key · when;
  - restored: "the operator (foundry doctor)" · project · when (the CLI has no signed-in user, so the actor is recorded as the operator).

  And **both** rename doors write it: the shipped instance-admin door joins in, so "who renamed this?" has one answer whichever door was used. No UI in this feature (read with psql). The deleted marker on the project row (who, when) is required either way, because `list-deleted-projects` prints it.
- **OQ-4: Can a new project take a deleted project's name, slug or key? (US-PL-04)** *Recommended: no, not while the project is deleted.* A deleted project keeps its name (case-insensitive, in its team), its derived slug (in its team) and its key (in its workspace). A create or rename that collides gets the shipped copy ("Project name must be unique within the team", "Project key is already in use in this workspace"). This guarantees a restore can never collide (INV-PL-5), keeps every old link and issue id pointing at one project only, and costs no schema change (the rows and unique constraints already stay). The cost: someone reusing "Auth v2 Spike" must pick another name or ask for a restore. Freeing them instead needs partial unique indexes and a restore that can fail or rename on collision. Under OQ-1 (b), a detached project keeps its key in the workspace; its team's names are freed with the team.
- **OQ-5: Is a typed confirmation needed for delete? (US-PL-02)** *Recommended: no.* The GET confirm page plus POST (D7) is the house pattern for team, lane and issue deletes, which are all irreversible. A soft delete is reversible, so the bar is lower, not higher.
- **OQ-6: Is restore CLI-only in this feature? (US-PL-04)** *Recommended: yes.* D-A names `restore-comment` as the precedent, which is an operator CLI. A "Deleted projects" page with **Restore** for workspace admins is a follow-up, built if PL-KPI-6 shows restores are requested often.

**Resolved by the user:** pending. If any OQ is answered against its
recommendation, the affected ACs are edited before DESIGN. The DoR table names
the OQs each story depends on.

### [REF] Journey (lightweight: happy path plus main refusals)

Emotional arc, **from clutter to tidy, without fear**: cluttered (a dead spike
on every dashboard and picker; asking an instance admin for a rename) → in
control (the team page lists the team's projects with Rename and Delete) →
wary (a destructive control) → reassured (the confirm page says nothing is
erased and how it comes back) → tidy (the project is gone everywhere) →
relieved (the operator restores it, exactly as it was). The full journey is
`docs/product/journeys/journey-project-lifecycle.yaml`.

```text
[Where]                              [Does]                                        [Sees]                                                              [Feels]
/team/backend/members (Lena, lead)   opens the team page                            Projects: "Identity Platform · AUTH", "Auth v2 Spike · SPIKE"     in control
/team/backend/members                edits Identity Platform to                     "Renamed Identity Platform to Identity & Access."                  confident
                                     "Identity & Access", clicks Rename             /team/backend/project/auth/issues/7 still opens AUTH-7
/team/backend/members                clicks Delete project on Auth v2 Spike         /team/backend/project/auth-v2-spike/delete:                        wary
                                                                                    "Delete Auth v2 Spike? Auth v2 Spike (SPIKE) has 3 issue(s). …
                                                                                     Nothing is erased. The instance operator can restore …"           reassured
/team/backend/project/.../delete     clicks Delete project                          /team/backend/members: "Deleted Auth v2 Spike."                     tidy
/team/backend/project/auth-v2-spike  (Sam, an old bookmark)                         the uniform not-found page                                         (expected)
shell (Priya, operator)              foundry doctor list-deleted-projects           "name: Auth v2 Spike" "key: SPIKE" "issues: 3" "deleted-by: lena@…" focused
shell                                foundry doctor restore-project <uuid>          "status: restored"; the board, SPIKE-1..3 and their comments open  relieved
```

Main refusals and edge paths:

| Step | What happens | Sees |
|---|---|---|
| Sam (member) posts a rename or opens the delete page of Identity Platform | refused | the uniform 404 (D-B, D4) |
| Marco, Hugo (Globex admin), a signed-out visitor, the same | refused | the uniform 404 |
| Lena posts a rename of Reading List (Ops; Lena leads only Backend) | refused | the uniform 404 (lead of another team) |
| Lena renames Identity Platform to "auth v2 spike" while Auth v2 Spike is live, or deleted | refused, text kept | "Project name must be unique within the team" (OQ-4) |
| Lena creates a new project with key SPIKE after the delete | refused | "Project key is already in use in this workspace" (OQ-4) |
| Sam saves an edit to SPIKE-2 from a dialog opened before the delete | refused, nothing changes | the uniform 404 (INV-PL-3) |
| The SPIKE board is open in Sam's tab during the delete | next action refused | the uniform 404 |
| Priya restores a project that is not deleted | exit 4 | "… is not deleted — status: not restorable" |
| (OQ-1 b) Priya deletes team Labs whose only project, PROTO, is deleted | confirm page warns; delete succeeds | "Labs also has 1 deleted project(s): Prototype Graveyard (PROTO). They are kept, but can't be restored after Labs is deleted, and their keys stay taken in Canzan Labs." |

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| `${project}` (name) | `projects.name` | members page row, board heading, nav, dashboard, new-issue picker, confirm page, notices, instance dashboard, CLI | MEDIUM: rename must reach every consumer by reading |
| `${project_slug}` | `projects.slug`, minted once | every `/team/{t}/project/{p}/…` and `/api/v1/…/projects/{p}/…` address, bookmarks | HIGH: rename and restore must not touch it (D2, D11) |
| `${KEY}` and issue ids | `projects.key_prefix`, `issues.number` | issue keys everywhere (AUTH-7), confirm page, CLI | HIGH: never changes (D2); held while deleted (OQ-4) |
| Deleted marker (who, when) | the project row (DESIGN names the columns) | every project reader (D5), CLI list, restore | **HIGH: one missed reader is a leak (INV-PL-2)** |
| `${project_id}` | `projects.id` | CLI `list-deleted-projects` and `restore-project`, the instance-admin rename route, lifecycle records | LOW: never shown in the web UI to non-admins |
| Live project list of a team | `projects` by `team_id`, live only | members page Projects section, team delete check (OQ-1) | HIGH: the team delete's check decides at commit time |
| Issue count | `issues` by `project_id` | confirm page, CLI list | LOW: advisory |
| Acting workspace | session `SessionUser.workspace_id` | every route | HIGH: tenancy (D9) |
| Name-rule copy | `ProjectNameError` `Display` | both rename doors, create | MEDIUM: one source |
| Uniform 404 | `resource_not_found_page`; the API's not-found | every refusal and every deleted-project address | HIGH |

### [REF] Story Map and Slices

Backbone: **See the team's projects → Name one right → Retire one → Bring one
back → Retire the team**

| Slice | Stories | What it adds | Est. | Brief |
|---|---|---|---|---|
| 01 | US-PL-01 | The team page lists the team's projects. A lead or admin renames one; the slug, key and every link stay. | 1 d | `slices/slice-01-rename-project-from-its-team.md` |
| 02 | US-PL-02, US-PL-03 | A lead or admin deletes a project through a confirm page. It vanishes from every list and every web and API address answers the uniform 404, for everyone. Nothing is erased. | 2 d | `slices/slice-02-delete-project-hidden-everywhere.md` |
| 03 | US-PL-04 | The operator lists deleted projects and restores one exactly. A deleted project's name, slug and key stay taken. | 1 d | `slices/slice-03-operator-restores-a-project.md` |
| 04 | US-PL-05 | (OQ-1) A team whose projects are all deleted can be deleted; its deleted projects are kept. | 1 d | `slices/slice-04-retire-a-team-of-retired-projects.md` |

Priority by learning leverage and risk:

1. **01** first. Lowest uncertainty (the rule, the policy and the slug discipline all ship), and it lands the Projects section that 02 hangs its control on. It is dogfooded the same day: Lena-style rename of a real dev project.
2. **02** next. It carries the riskiest assumption: that **every** address of a project can be closed by one deleted marker without missing a reader (INV-PL-2). About 15 store readers and 21 routes. Learning that early costs least. It is 2 days, over the one-day taste test, **on purpose**: shipping the delete before the API and SSE doors are closed would leave a "deleted" project answering on `/api/v1`, which is a leak, and an infrastructure-only "close the other doors" slice is forbidden.
3. **03** third, and **released together with 02** (the user tags releases; slice 02 is not tagged without 03). A delete with no restore is a hard delete in practice for everyone without psql. It holds the second risk: that a restore is exact (INV-PL-4).
4. **04** last. It depends on OQ-1, touches team-lifecycle's proven delete transaction, and has value only once 02 and 03 exist.

Taste tests: no slice ships 4 or more new components. No slice is
infrastructure only. Every slice names what it disproves (see the briefs). Every
dogfood uses dev's real workspaces. No two slices differ only in scale.

### [REF] User Stories

#### US-PL-01: A team's lead renames a project from the team page, and every link and issue key keeps working

`job_id: job-project-lifecycle` | slice 01

##### Elevator Pitch

Before: "Identity Platform" became Backend's access-management work, but only an instance super-admin can rename it, from `/admin/instance/workspaces`, which Lena (Backend's lead) never sees. Lena files a request and waits.
After: on `/team/backend/members`, under **Projects**, Lena edits **Project name** to "Identity & Access" and clicks **Rename** → "Renamed Identity Platform to Identity & Access." The row reads "Identity & Access · AUTH", and `/team/backend/project/auth/issues/7` still opens AUTH-7.
Decision enabled: Lena keeps Backend's project names truthful themselves, knowing no link or issue id changes.

##### Problem

Project names drift as work changes. The only rename door is an instance-wide
super-admin dashboard, so the people who own the project (its team's lead)
cannot fix its name.

##### Who

- A team's lead, or a workspace admin | the team's members page, desktop browser | wants a project's name to say what it is now.

##### Domain Examples

1. **Lead**: Lena renames Identity Platform (AUTH, slug `auth`, minted from its original name "Auth" at creation and never re-derived) to "Identity & Access" → the row reads "Identity & Access · AUTH". `/team/backend/project/auth` and `/team/backend/project/auth/issues/7` still answer 200.
2. **Admin, another team**: Priya (workspace admin, not on Ops) renames Homelab Ops (OPS, slug `homelab-ops`) on `/team/ops/members` to "Homelab Operations" → "Renamed Homelab Ops to Homelab Operations.". The slug stays `homelab-ops`.
3. **Duplicate**: in Ops, Priya renames Reading List (READ) to "homelab operations" → "Project name must be unique within the team", the field keeps "homelab operations", nothing changes.
4. **Case-only and no-op**: "Reading list" → "Reading List" is a rename. Submitting "Reading List" unchanged writes nothing and shows no notice.
5. **Refused**: Sam (member of Backend), Marco, Hugo (Globex admin) and Lena on Ops (lead of Backend only) each post a rename → the uniform 404, nothing changes. Sam sees the Projects list with no Rename or Delete.

##### UAT Scenarios (BDD)

###### Scenario: A team's lead renames a project and every link to it keeps working

- Given Lena leads Backend, which has project Identity Platform (AUTH) at /team/backend/project/auth with issue AUTH-7
- When Lena renames Identity Platform to "Identity & Access" on Backend's page
- Then Lena sees "Renamed Identity Platform to Identity & Access." and Backend's Projects list shows "Identity & Access · AUTH"
- And /team/backend/project/auth and /team/backend/project/auth/issues/7 still open, and the issue is still AUTH-7

###### Scenario: A workspace admin renames a project of a team they are not on

- Given Priya is a workspace admin of Canzan Labs and not on Ops, which has Homelab Ops (OPS)
- When Priya renames Homelab Ops to "Homelab Operations" on Ops's page
- Then Priya sees "Renamed Homelab Ops to Homelab Operations." and the project's address is still /team/ops/project/homelab-ops

###### Scenario: A name already used in the team is refused

- Given Ops has projects Homelab Operations and Reading List
- When Priya renames Reading List to "homelab operations"
- Then Priya sees "Project name must be unique within the team", their text is kept, and Reading List keeps its name

###### Scenario: Submitting the same name changes nothing

- When Lena submits Identity & Access's rename with "Identity & Access" unchanged
- Then Backend's page re-renders with no notice, and nothing is recorded

###### Scenario: Only the team's lead or a workspace admin can rename a project

- Given Sam is a member of Backend, Marco is on no team, and Hugo is a workspace admin of Globex
- When each of them, and a signed-out visitor, submits a rename of Identity Platform
- Then every request answers the uniform not-found page, and the project keeps its name
- And Sam sees Backend's Projects list without a Rename or Delete project control

##### Acceptance Criteria

- [ ] The members page shows a Projects section listing the team's live projects as "{project} · {KEY}" linking to each board, to anyone who can view the team; "{team} has no projects yet." when there are none (scenario 1; D8).
- [ ] A `Manage` caller sees a prefilled Project name field, Rename and Delete project per project; a `View` caller sees neither (scenario 5; D-B, D8).
- [ ] Rename writes the name only, never the slug or key; every shipped address under the slug answers as before; the D14 notice is shown (scenarios 1, 2; D2).
- [ ] The shipped project-name rule and copy apply, the project itself excluded from uniqueness, the input kept, nothing written (scenario 3; D3).
- [ ] A byte-equal name writes nothing and records nothing; a case-only change is a rename (scenario 4; D3).
- [ ] Every caller without `Manage` on the project's team, an unknown or deleted project, a foreign team and a signed-out caller get the uniform 404 with zero writes (scenario 5; D4).
- [ ] (OQ-3) Each effective rename, on this door and on the instance-admin door, is on record: who, which project, old name, new name, when. A refused or byte-equal rename leaves no record.
- [ ] It works with scripting disabled.

##### Outcome KPIs

PL-KPI-1, PL-KPI-4, PL-KPI-5.

##### Technical Notes

- Reuses `ProjectName` (OUT-19), `TeamAccess::can_manage` and the rename use-case of ADR-PROJECT-RENAME-002. DESIGN decides whether both doors share one use-case (recommended: yes, so OQ-3 holds on both).
- The shipped instance-admin rename is check-then-write with no lock (its D7 residual). Whether the new door locks, as `rename_team_with_audit` does, is DESIGN's call (INV-PL-9).
- No schema change unless OQ-3 is accepted (the record).
- Verification: the record AC is checked by a read-only store query (records have no UI).

##### Size

1 day | 5 scenarios | slice 01

#### US-PL-02: A team's lead deletes a project through a confirm page, and nothing under it is erased

`job_id: job-project-lifecycle` | slice 02

##### Elevator Pitch

Before: "Auth v2 Spike" (SPIKE), created by mistake in Backend, sits on every dashboard and in every new-issue picker forever, and no one but psql can remove it. In psql a delete would cascade over its 3 issues, 2 comments and 1 attachment with no way back.
After: on `/team/backend/members`, Lena clicks **Delete project** on Auth v2 Spike → `/team/backend/project/auth-v2-spike/delete` shows "Delete Auth v2 Spike? Auth v2 Spike (SPIKE) has 3 issue(s). … Nothing is erased. The instance operator can restore Auth v2 Spike with every issue, comment and attachment." Clicking **Delete project** lands on `/team/backend/members` with "Deleted Auth v2 Spike.", and the project is gone from the list.
Decision enabled: Lena can retire dead projects without fear, because the delete is reversible and says so.

##### Problem

Mistaken and finished projects accumulate on every dashboard, picker and team
page. There is no product delete, and the only removal (psql) destroys every
issue, comment and attachment under the project.

##### Who

- A team's lead, or a workspace admin | members page, confirm page | removes a project.

##### Domain Examples

1. **Mistaken project**: Lena deletes Auth v2 Spike (SPIKE: 3 issues, 2 comments, 1 attachment `spike-notes.png`) → "Deleted Auth v2 Spike.". Backend's Projects list shows only Identity & Access. The store still holds 3 issues, 2 comments and 1 attachment for SPIKE.
2. **Admin**: Priya deletes Reading List (READ, 0 issues) on Ops → the confirm page reads "Reading List (READ) has 0 issue(s). …"; after the delete, "Deleted Reading List.".
3. **Cancel**: Lena opens the confirm page and clicks **Cancel** → back on Backend's page, nothing changes.
4. **Refused**: Sam (member), Marco, Hugo and Lena on an Ops project each open the delete page and post a delete → the uniform 404, nothing changes.
5. **Double submit**: Lena's second POST of the same delete (a double click) → the uniform 404, because the project is already gone; one delete, one record.

##### UAT Scenarios (BDD)

###### Scenario: A team's lead deletes a project after reading what happens

- Given Lena leads Backend, which has Auth v2 Spike (SPIKE) with 3 issues
- When Lena opens Delete project on Auth v2 Spike
- Then Lena reads "Delete Auth v2 Spike?", "Auth v2 Spike (SPIKE) has 3 issue(s). After it is deleted, nobody can open its board, its issues or its report, and links to SPIKE issues stop working." and "Nothing is erased. The instance operator can restore Auth v2 Spike with every issue, comment and attachment."
- And when Lena confirms, they see "Deleted Auth v2 Spike." and Backend's Projects list no longer shows it

###### Scenario: Deleting a project erases none of its work

- Given Auth v2 Spike has 3 issues, 2 comments, 1 attachment and 4 lanes
- When Lena deletes Auth v2 Spike
- Then the store still holds exactly 3 issues, 2 comments, 1 attachment and 4 lanes for it, unchanged

###### Scenario: Cancelling a delete changes nothing

- When Lena opens Delete project on Auth v2 Spike and chooses Cancel
- Then Lena is back on Backend's page and Auth v2 Spike is still listed

###### Scenario: Only the team's lead or a workspace admin can delete a project

- Given Sam is a member of Backend, Marco is on no team, and Hugo is a workspace admin of Globex
- When each of them, a signed-out visitor, and Lena on Ops's Reading List open the delete page and submit a delete
- Then every request answers the uniform not-found page, and every project still exists

##### Acceptance Criteria

- [ ] The confirm page shows the D14 heading, the issue count and both consequence lines, with Delete project and Cancel (scenario 1; D7).
- [ ] The POST marks the project deleted (who, when) and lands on the members page with "Deleted {project}." (scenario 1; D-A).
- [ ] Every issue, comment (including deleted comments awaiting cleanup), attachment, history entry and lane of the project is still there and unchanged, and the next issue number is unchanged (scenario 2; D6, INV-PL-1).
- [ ] Cancel writes nothing (scenario 3).
- [ ] Every caller without `Manage`, a foreign team, an unknown or already-deleted project and a signed-out caller get the uniform 404 on both verbs with zero writes (scenario 4; D4).
- [ ] (OQ-3) Each delete is on record: who, which project, its name and key, when. A refused or cancelled delete leaves no record.
- [ ] Both pages work with scripting disabled.

##### Outcome KPIs

PL-KPI-1, PL-KPI-2, PL-KPI-5.

##### Technical Notes

- The deleted marker mirrors `comments.deleted_at`/`deleted_by` (0006). Column names and the migration are DESIGN's.
- The confirm page's issue count is advisory (the team-lifecycle INV-TL-3 posture).
- Verification: the "nothing erased" AC is checked by before-and-after store counts of issues, comments, attachments, change events, lanes and outbox rows; the record AC by a read-only store query.

##### Size

0.5 day | 4 scenarios | slice 02

#### US-PL-03: A deleted project answers nowhere, for anyone, on any door

`job_id: job-project-lifecycle` | slice 02

##### Elevator Pitch

Before: there is no delete, so the question never arose. With a delete, any address that forgets the project is deleted keeps serving it: an old board bookmark, an issue link in an email, `/api/v1/teams/backend/projects/auth-v2-spike/issues` from a script, an open live-update stream.
After: after Lena deletes Auth v2 Spike, `/team/backend/project/auth-v2-spike`, `/team/backend/project/auth-v2-spike/issues/2`, `…/report?format=csv` and `/api/v1/teams/backend/projects/auth-v2-spike/issues` all answer 404 with the same "not found" page as `/team/backend/project/no-such-project`, for Lena, Sam and Priya alike (the API answers its own not-found body). Sam's dashboard lists "Identity & Access" only, and the new-issue dialog's project picker no longer offers "Auth v2 Spike".
Decision enabled: Lena chooses to delete instead of leaving a dead project around (or asking Priya to run psql), because a delete removes it from every view Sam and scripts use.

##### Problem

A soft delete that leaves one door open is a leak and a confusion: the project
is "deleted" but still answers, and someone keeps filing issues into it.

##### Who

- Everyone who reaches a project: team members in the browser (Sam), scripts with a machine token, the instance admin's dashboard (Priya).

##### Domain Examples

1. **Bookmarks**: Sam opens their bookmark `/team/backend/project/auth-v2-spike/issues/2` (SPIKE-2) the day after the delete → the uniform 404.
2. **API**: a script with a machine token scoped to Backend GETs `/api/v1/teams/backend/projects/auth-v2-spike/issues` → the API's not-found answer, the same as for `/api/v1/teams/backend/projects/no-such-project/issues`.
3. **Lists**: Sam's dashboard and the new-issue dialog's project picker list Identity & Access only. Priya's `/admin/instance/workspaces` no longer lists Auth v2 Spike under Canzan Labs, and a rename POST to its id answers the uniform 404.
4. **Stale tab**: Sam had SPIKE-2's edit dialog open before the delete. Sam clicks Save → the uniform 404, and SPIKE-2 is unchanged.
5. **Race**: Sam files a new issue on Auth v2 Spike at the same moment Lena deletes it → either the issue is filed and then hidden with the project (and comes back with a restore), or Sam's create answers the uniform 404. Never a 500, never a lost issue.
6. **Notifications already queued**: Sam commented on SPIKE-2 a second before the delete. The email about that comment, already queued, is still delivered (it reports a change made while the project was live); its link answers 404 until a restore. No new notification about SPIKE can arise after the delete, because every write is refused.

##### UAT Scenarios (BDD)

###### Scenario: Every address of a deleted project answers not-found, for everyone

- Given Lena has deleted Auth v2 Spike (SPIKE)
- When Lena, Sam and Priya each open its board, SPIKE-2's page, its report, its CSV export, its new-issue form and its live updates
- Then every request answers the uniform not-found page, byte-identical to an address that never existed

###### Scenario: A deleted project is not reachable through the API

- Given Lena has deleted Auth v2 Spike and a script holds a machine token for Backend
- When the script lists SPIKE's issues, reads SPIKE-2 and posts a comment on it through the API
- Then each call gets the API's not-found answer, the same as for a project that never existed, and nothing is written

###### Scenario: A deleted project disappears from every list

- Given Lena has deleted Auth v2 Spike
- When Sam opens their dashboard and the new-issue dialog, and Priya opens the instance dashboard
- Then none of them lists Auth v2 Spike, and Identity & Access is still listed

###### Scenario: A change sent from a page opened before the delete is refused

- Given Sam opened SPIKE-2's edit dialog before Lena deleted Auth v2 Spike
- When Sam saves a new title
- Then Sam gets the uniform not-found page, and SPIKE-2 is unchanged

###### Scenario: Filing an issue during a delete gets a normal answer, never a server error

- When Sam files an issue on Auth v2 Spike at the same moment Lena deletes it
- Then either Sam's issue exists, hidden with the project, or Sam's request answered the uniform not-found page
- And no request answers a server error

##### Acceptance Criteria

- [ ] Every web route under `/team/{t}/project/{p}/…` (the inventory in [REF] Lifecycle inventory) answers the uniform 404 for a deleted project, on every verb, for every actor including its deleter and workspace admins (scenario 1; D5, INV-PL-2).
- [ ] Every `/api/v1/teams/{t}/projects/{p}/…` route answers the API's shipped not-found answer for a deleted project, with zero writes (scenario 2; INV-PL-2).
- [ ] The dashboard, the nav, the new-issue project picker, the team's Projects list and the instance dashboard omit deleted projects; the instance rename of a deleted project answers the uniform 404 (scenario 3; D5, D16).
- [ ] A write racing or following the delete gets its normal outcome or the uniform 404, never a 500; nothing it wrote before the delete committed is lost (scenarios 4, 5; INV-PL-3). Over 100 repeated delete-versus-issue-create races on real Postgres: 0 responses of 500, 0 lost issues.
- [ ] An open live-update stream on a deleted project sends no further events, and a reconnect answers the uniform 404 (scenario 1).
- [ ] Notifications queued before the delete are delivered as usual; none is produced after it (example 6; D17).

##### Outcome KPIs

PL-KPI-3, PL-KPI-5.

##### Technical Notes

- About 15 store readers and 21 web plus 7 API routes reach a project (inventory). INV-PL-2 is a requirement; the mechanism (one live-project resolution seam, a view, a check-arch rule as team-lifecycle's `team-lifecycle-one-seam`) is DESIGN's.
- Whether an issue create may land in a just-deleted project (and be restored with it) or must be refused under a lock is DESIGN's call within INV-PL-3.

##### Size

1.5 days | 5 scenarios | slice 02 (the address sweep is most of slice 02's 2 days)

#### US-PL-04: The instance operator restores a deleted project exactly as it was

`job_id: job-project-lifecycle` | slice 03

##### Elevator Pitch

Before: a week after Lena deleted Auth v2 Spike, they realise SPIKE-2 still matters. Without a restore, the delete is final for everyone who cannot write SQL.
After: Priya runs `foundry doctor list-deleted-projects` → a block with `name: Auth v2 Spike`, `key: SPIKE`, `issues: 3`, `deleted-by: lena@canzan.dev` and its `project-id`. Then `foundry doctor restore-project <uuid>` → `status: restored`. `/team/backend/project/auth-v2-spike/issues/2` opens SPIKE-2 again, with its comments and attachment.
Decision enabled: Lena and Priya can delete without fear, and Priya can answer "can we get it back?" with yes.

##### Problem

A soft delete is only as safe as its restore. The operator needs to find a
deleted project and bring it back whole, without hand-written SQL.

##### Who

- Priya as the instance operator | a shell with `DATABASE_URL` | restores on a lead's request.
- Lena | asks for the restore and uses the project again.

##### Domain Examples

1. **Restore**: Priya lists the deleted projects, finds Auth v2 Spike (deleted by lena@canzan.dev), restores it → `status: restored`. Backend's page lists "Auth v2 Spike · SPIKE" again; SPIKE-1..3, 2 comments and `spike-notes.png` are back; the next issue filed is SPIKE-4.
2. **Nothing deleted**: on a fresh dev instance, `list-deleted-projects` prints `status: none`, exit 0.
3. **Not deleted**: `restore-project` on Identity & Access's id → exit 4, "… is not deleted — status: not restorable". An unknown uuid → exit 4, "… not in database …". `restore-project not-a-uuid` → exit 2.
4. **Name held**: while Auth v2 Spike is deleted, Lena tries to create "auth v2 spike" in Backend → "Project name must be unique within the team"; and a new project with key SPIKE anywhere in Canzan Labs → "Project key is already in use in this workspace" (OQ-4). So the restore never collides.
5. **Stale bookmark after restore**: Sam's old bookmark to SPIKE-2 opens again.

##### UAT Scenarios (BDD)

###### Scenario: The operator restores a deleted project with everything under it

- Given Lena deleted Auth v2 Spike (SPIKE) with 3 issues, 2 comments and 1 attachment
- When Priya runs list-deleted-projects, then restore-project with its id
- Then the list showed Auth v2 Spike, SPIKE, 3 issues and deleted-by lena@canzan.dev, and the restore printed "status: restored"
- And Backend's page lists Auth v2 Spike again, SPIKE-2 opens at its old address with its comments and attachment, and the next issue filed is SPIKE-4

###### Scenario: The operator is told when there is nothing to restore

- When Priya runs list-deleted-projects on an instance with no deleted project
- Then it prints "status: none" and exits 0

###### Scenario: Restoring a project that is not deleted is refused

- When Priya runs restore-project with Identity & Access's id, with an unknown id, and with "not-a-uuid"
- Then the first two exit 4 with "status: not restorable" and the reason, the third exits 2, and nothing changes

###### Scenario: A deleted project's name and key stay taken until it is restored

- Given Auth v2 Spike (SPIKE) in Backend is deleted
- When Lena creates a project named "auth v2 spike" in Backend, and another named "Spike Two" with key SPIKE
- Then Lena sees "Project name must be unique within the team" and "Project key is already in use in this workspace", and no project is created

##### Acceptance Criteria

- [ ] `list-deleted-projects` prints the D14 block per deleted project, oldest deletion first, or `status: none`; exit 0 (scenarios 1, 2; D10).
- [ ] `restore-project` clears the deleted marker; afterwards every address answers as before the delete, with the same name, slug, key, issue numbers, issue counter, lanes, comments and attachments (scenario 1; D11, INV-PL-4).
- [ ] Not deleted or unknown → exit 4 with the reason on stderr; invalid uuid → exit 2; database unreachable → exit 3; nothing changes (scenario 3).
- [ ] While deleted, the project's name, derived slug and key stay taken, with the shipped copy on create and rename (scenario 4; OQ-4, INV-PL-5).
- [ ] (OQ-3) Each restore is on record as done by the operator, with the project and when. A refused restore leaves no record.

##### Outcome KPIs

PL-KPI-2, PL-KPI-6.

##### Technical Notes

- Mirrors `run_restore_comment` (`admin_cli.rs:446`): live `DATABASE_URL`, thread-isolated runtime, the same exit codes.
- OQ-4 (b), freeing names on delete, would need partial unique indexes and a collision path on restore; the recommendation avoids both.

##### Size

1 day | 4 scenarios | slice 03

#### US-PL-05: A team whose projects are all deleted can itself be deleted, and its deleted projects are kept

`job_id: job-project-lifecycle` | slice 04 | **Written to OQ-1 (b).**

##### Elevator Pitch

Before: team Labs holds one finished project, Prototype Graveyard (PROTO). Today `/team/labs/delete` answers "Labs still has 1 project(s): Prototype Graveyard (PROTO). A team with projects can't be deleted." With slices 02-03 alone, deleting PROTO would not change that, because the deleted row still exists, so Labs could never go.
After: on `/team/labs/delete` Priya reads "Labs has 1 member(s) and no projects. …" plus "Labs also has 1 deleted project(s): Prototype Graveyard (PROTO). They are kept, but can't be restored after Labs is deleted, and their keys stay taken in Canzan Labs." Clicking **Delete team** → `/teams` shows "Deleted Labs.", and PROTO's issues, comments and attachments are still in the store.
Decision enabled: Priya can retire a team whose work is all retired, knowing what they give up (restoring its projects) before they commit.

##### Problem

Team-lifecycle refuses a team delete while the team has any project, because a
delete must never destroy a project. Once projects can be soft-deleted, a team
whose projects are all deleted is blocked forever unless deleted projects stop
counting, and they must survive the team.

##### Who

- Workspace admin | the team delete confirm page | retires a team.

##### Domain Examples

1. **Only deleted**: Labs has PROTO, deleted; Priya deletes Labs → "Deleted Labs."; PROTO's 12 issues are still in the store.
2. **Mixed**: Ops has Homelab Operations (live) and Reading List (deleted) → "Ops still has 1 project(s): Homelab Operations (OPS). A team with projects can't be deleted." The live project is named; the deleted one is not.
3. **Orphan restore**: after Labs is gone, Priya runs `restore-project` on PROTO's id → exit 4, "… its team labs was deleted — status: not restorable". PROTO's rows are unchanged.
4. **Race**: Priya runs `restore-project` on PROTO at the same moment a delete of Labs, confirmed earlier in a browser tab, commits → either PROTO is restored first and the delete is refused naming PROTO, or Labs is deleted first and the restore exits 4. Never a live project lost.
5. **Key held**: after Labs is deleted, a new project with key PROTO in Canzan Labs is still refused (OQ-4).

##### UAT Scenarios (BDD)

###### Scenario: A team whose only project is deleted can be deleted, and the project's work is kept

- Given team Labs has one project, Prototype Graveyard (PROTO), which is deleted and has 12 issues
- When Priya opens Delete team on Labs and confirms
- Then the page said "Labs also has 1 deleted project(s): Prototype Graveyard (PROTO). They are kept, but can't be restored after Labs is deleted, and their keys stay taken in Canzan Labs."
- And Priya sees "Deleted Labs.", and the store still holds PROTO's 12 issues unchanged

###### Scenario: Only live projects block a team delete

- Given Ops has Homelab Operations (OPS), live, and Reading List (READ), deleted
- When Priya opens Delete team on Ops
- Then Priya sees "Ops still has 1 project(s): Homelab Operations (OPS). A team with projects can't be deleted." and no Delete team button

###### Scenario: A deleted project whose team is gone cannot be restored, and is not lost

- Given Labs was deleted while Prototype Graveyard was deleted
- When Priya runs restore-project with Prototype Graveyard's id
- Then it exits 4 with "its team labs was deleted — status: not restorable", and PROTO's rows are unchanged

###### Scenario: A restore racing a team delete never loses a project

- When Priya restores Prototype Graveyard at the same moment as Labs is deleted
- Then either Prototype Graveyard is live in Labs and the delete was refused naming it, or Labs is gone and the restore exited 4
- And no project row and no issue is ever removed

##### Acceptance Criteria

- [ ] The team delete (page and POST) refuses only while the team has a **live** project, naming live ones only, decided at commit time (scenario 2; OQ-1, INV-TL-1 as amended).
- [ ] With deleted projects and no live ones, the confirm page adds the D14 line and the delete succeeds; every row of every deleted project survives (scenario 1; D-A, INV-PL-6).
- [ ] `restore-project` on a project whose team is gone exits 4 with the reason and changes nothing (scenario 3).
- [ ] Over 100 repeated restore-versus-team-delete races on real Postgres: 0 lost projects, 0 lost issues, 0 unmapped errors (scenario 4; INV-PL-6).
- [ ] The team delete record is unchanged in shape (team-lifecycle OQ-6).

##### Outcome KPIs

PL-KPI-2, PL-KPI-7.

##### Technical Notes

- Amends `delete_team_if_empty` (store `teams.rs:740`) and `TEAM_PROJECTS_SQL`, and therefore OUT-25 and ADR-TEAM-LIFECYCLE-002. `projects.team_id → teams ON DELETE CASCADE` cannot stay as is: the deleted rows must survive. The mechanism is DESIGN's.
- If OQ-1 is answered (a): this story becomes "the team delete refusal marks deleted projects as deleted", 0.5 day, 2 scenarios.

##### Size

1 day | 4 scenarios | slice 04

### [REF] System Constraints

Invariant and concurrency requirements for DESIGN. They name the risks and do
not choose mechanisms.

- **INV-PL-1 (nothing erased):** a project delete changes only the project's deleted marker and writes the record. No row of `issues`, `comments`, `issue_attachments`, `issue_change_events`, `lanes` or `outbox` under it is removed or changed, and `next_issue_number` is unchanged (D6).
- **INV-PL-2 (no door left open):** every read or write that reaches a project through a web or API address treats a deleted project exactly as one that never existed, and every list omits it. The only readers that see deleted projects are the operator CLI, the per-workspace export (D13), the uniqueness checks (OQ-4) and the team delete (OQ-1). A new project reader added later must not see deleted projects by default.
- **INV-PL-3 (no 500 from a vanishing project):** a write racing a delete (issue create, edit, state, delete; comment; attachment; lane edit, insert, move, delete; project rename) gets its normal outcome or the uniform 404, never a 500. Anything that committed before the delete is kept and comes back with a restore.
- **INV-PL-4 (exact restore):** after a restore, the project's name, slug, key, issue numbers, issue counter and every row under it equal their state at the delete, and every address answers as before.
- **INV-PL-5 (names held, OQ-4):** while a project is deleted, its case-insensitive name and derived slug stay taken in its team and its key in its workspace, so a restore can never collide.
- **INV-PL-6 (team delete, OQ-1 (b)):** a team delete commits only if the team has no live project at commit time. Its deleted projects survive with every row. A restore racing a team delete either restores first (the delete is refused, naming it) or finds the team gone (exit 4). A project delete racing a team delete in either order leaves no live project destroyed. This amends INV-TL-1 to "no **live** project row, and no row of any project, is ever removed by a team delete".
- **INV-PL-7 (audit atomicity, OQ-3):** a rename, delete or restore record commits or rolls back with its change, and a byte-equal rename writes none.
- **INV-PL-8 (authority at request time):** per request, against current rows. A stale form from a demoted lead, or from a lead removed from the team, gets the uniform 404.
- **INV-PL-9 (rename):** a rename never changes a slug or key. Concurrent renames may leave two case-insensitively equal names only if DESIGN accepts that residual, as ADR-PROJECT-RENAME-002 did; if so, record it.
- No new oracle: every refusal to an unauthorized caller, and every address of a deleted project, is byte-identical to the uniform 404 (D4, D5).

### [REF] Non-Functional Requirements

- **NFR-PL-1 (accessibility):** the Projects section, its Project name fields, Rename buttons and Delete project links, and the confirm page meet the same standard as the shipped team members page and team delete page (no regression; no new standard is introduced). Each Project name field has a visible label that names its project for assistive technology (for example "Project name, Identity Platform"); the tab order is per row (field, Rename, Delete project); **Cancel** returns to the members page with focus at the top of the page, as the team delete's Cancel does.
- **NFR-PL-2 (no slower lists):** the dashboard, the board, the members page and the instance dashboard answer no slower than before the deleted-marker filter, measured by the shipped smoke and acceptance timings (no new budget; a visible regression in the `default` lane is a defect). Homelab scale: tens of projects.
- **NFR-PL-3 (scripting disabled):** every new page and form works with scripting disabled (the shipped lane).
- **NFR-PL-4 (failure is all-or-nothing):** if the database is unreachable or a write fails during a rename, delete or restore, nothing is changed (no partial rename, no delete without its record), the web door shows the shipped error page and the CLI exits 3. No new copy.
- **NFR-PL-5 (record retention, OQ-3):** lifecycle records are kept for as long as their workspace exists, like `team_lifecycle_events` and `workspace_rename_events`. No retention period, no UI.
- **NFR-PL-6 (security):** no new oracle (D4, D5); CSRF on every new POST (the shared layer); tenancy from the session (D9); restore requires `DATABASE_URL`, i.e. operator shell access (D10).

### [REF] Outcome KPIs

Objective: a team keeps its own project list truthful and tidy, and no delete
ever loses work.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| PL-KPI-1 | Team leads and workspace admins | Rename and retire their team's projects in the product | 0 project renames requested of an instance admin and 0 hand-written `UPDATE`/`DELETE` on `projects` from the release onward | Every project rename was the instance dashboard or psql; every removal was psql | Operator report at each dogfood | North star (leading) |
| PL-KPI-2 | Project deletes | Erase nothing | 0 rows removed or changed under a deleted project; 100% of rows back after a restore (counts before delete = counts after restore) | Not applicable (no delete) | Acceptance before/after store counts; read-only counts around each dogfood | Guardrail |
| PL-KPI-3 | Deleted projects | Answer nowhere | 100% of the web and API address inventory answers the byte-identical not-found for a deleted project, for every actor | Not applicable | Acceptance outline over the inventory | Guardrail |
| PL-KPI-4 | Callers without the right | Learn nothing and change nothing | 100% of the (actor × verb × project) matrix answers byte-identical 404s with 0 rows written | Not applicable (no routes) | Acceptance authz outline | Guardrail |
| PL-KPI-5 | Renamed and restored projects | Keep every link | 100% of addresses under the slug answer the same status after a rename and after a delete-then-restore; issue keys identical | Not applicable | Acceptance scenarios | Guardrail |
| PL-KPI-6 | Restore requests | Are met by the operator door | Counted only. Above 2 in the first 30 days is evidence for a web restore for workspace admins (OQ-6) | 0 | Operator report | Leading (demand signal) |
| PL-KPI-7 | Teams whose work is all retired | Can be deleted | TL-KPI-5 refusals caused only by deleted projects: 0 | Every team that ever held a project is undeletable | Operator report; acceptance | Leading |

At homelab scale, the KPIs are verified by acceptance scenarios, read-only
store queries and the operator's report, not analytics.

### [REF] DoD

1. All UAT scenarios pass: 5 + 4 + 5 + 4 + 4 = 22. The neighbouring lanes stay green: team-lifecycle (tl-01..04, with OUT-25's amended scenarios if OQ-1 is (b)), team-membership, us-07 project create, project-name-rule, name-db-checks, instance-admin-project-rename, issue-card-delete, board-lane-*, us-10 comments (including tombstone GC), us-11 attachments, us-09 SSE, the API features, keyboard and new-issue dialog, and default.
2. The authz matrix (PL-KPI-4) covers signed out, Marco, a member, a lead of another team, the team's lead, a workspace admin, an instance admin with no workspace-admin row, Hugo (a Globex admin, tenancy column), an unknown slug, a foreign team and a deleted project, on every new route and both verbs.
3. The deleted-project inventory outline (PL-KPI-3) covers every route in [REF] Lifecycle inventory.
4. INV-PL-3 and INV-PL-6 are proven by races on real Postgres, at least 100 runs each, with 0 lost rows and 0 responses of 500.
5. Nothing is written on a refusal (row counts asserted).
6. The pages work with scripting disabled.
7. `cargo xtask check-arch` and `cargo xtask smoke` pass before each commit. `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` passes before push. Mutation kill rate is at least 80% on modified files.
8. **Dogfood on dev**, after the user releases (dev auto-deploys release tags; prod is held for approval): rename a real project and reopen an old bookmark; delete a scratch project, check its board, an issue link and an API call answer not-found, take read-only counts; restore it and recheck; (OQ-1 b) delete a scratch team whose only project is deleted.
9. Upgrade: every migration additive and rolling-deploy safe (an old replica that ignores the deleted marker would show deleted projects during a rolling deploy: DESIGN states whether that window is acceptable). Restore compatibility of old dumps is named in DESIGN (team-lifecycle lesson 3).
10. Documentation: a CHANGELOG `[Unreleased]` entry; `brief.md` gains "Project lifecycle" (DESIGN); outcomes registry rows (new, plus OUT-1 and OUT-25 amended) and the `kpi-contracts.yaml` key at DISTILL; `jobs.yaml`, the persona and the journey were done here.

### [REF] Out of Scope

- Moving a project between teams, including restoring a detached project into another team (D-C; follow-up).
- A hard purge of a deleted project, a retention period, a cascade delete (D-C).
- A web restore page, a "Deleted projects" list in the product, or a "deleted" badge anywhere (OQ-6).
- Changing a project's slug, URL or key, and redirects (OQ-2).
- A UI for the lifecycle records (OQ-3: records only).
- Notifications to the team or watchers about a rename or delete.
- Project archive (read-only but visible) as distinct from delete.
- Team-level soft delete.
- **`foundry doctor set-role`** (see Follow-ups).

### [REF] Follow-ups (recorded, not in scope)

- **A `foundry doctor set-role` door.** Workspace roles (`workspace_memberships.role`, admin or member) can be changed today only by hand-written SQL. Found 2026-10-12, when the user's Keycloak account had to be made a workspace admin of their own workspace by SQL. A doctor command (for example `foundry doctor set-role --workspace <name> --email <email> admin|member`), with `grant-super-admin`'s idempotence and the never-leaderless discipline in mind, is its own small feature.
- **Restore into a team** for a project whose team was deleted (OQ-1 (b)'s gap), when move is designed.
- **A web restore for workspace admins** if PL-KPI-6 shows demand (OQ-6).
- **A purge** of long-deleted projects, if storage ever matters (inline attachments are the heavy part).

### [REF] WS Strategy

Brownfield, no walking skeleton (user). Sessions, tenancy, the members page,
`TeamAccess`, the project-name rule, the rename use-case, boards, the API and the
doctor CLI all ship. Slice 01 extends the members page and the rename door;
slices 02-04 build on it. Every slice runs on real adapters (real Postgres, real
router, real session, the real `foundry` binary for the CLI), with no
environment-switched seam.

### [REF] Driving Ports

1. `GET /team/{team_slug}/members` (delta): the Projects section; Rename and Delete project for `Manage`.
2. `POST /team/{team_slug}/project/{project_slug}/rename` (new): US-PL-01.
3. `GET` and `POST /team/{team_slug}/project/{project_slug}/delete` (new): US-PL-02.
4. Every shipped `/team/{t}/project/{p}/…` route and every `/api/v1/teams/{t}/projects/{p}/…` route (delta): the uniform 404 for a deleted project (US-PL-03).
5. `GET /`, the nav, the new-issue project picker, `GET /admin/instance/workspaces`, `POST /admin/instance/projects/{project_id}/rename` (delta): omit or refuse deleted projects; the instance rename writes the record (OQ-3).
6. `POST /team/{team_slug}/projects` (delta): a deleted project's name and key stay taken (OQ-4).
7. `foundry doctor list-deleted-projects`, `foundry doctor restore-project <uuid>` (new): US-PL-04.
8. `GET` and `POST /team/{team_slug}/delete` (delta, OQ-1): only live projects block; deleted ones are kept (US-PL-05).

Routes are proposals (D15). The copy is locked (D14).

### [REF] Pre-requisites

- team-lifecycle has shipped (v0.14.0, live on dev and prod): `TeamAccess::Administer`, `delete_team_if_empty`, `team_lifecycle_events`, the members page's Rename and Delete team.
- These already ship: `ProjectName` and its copy, the instance-admin rename use-case, `resource_not_found_page`, the API not-found, the doctor CLI scaffold (`run_restore_comment`), CSRF and sessions, and the scripting-disabled lane.
- Dev has a scratch project and a scratch team for the dogfoods (create them with the shipped doors if absent).
- The OQs are answered before DESIGN of the slices they touch: OQ-2 and OQ-3 before slice 01; OQ-3 and OQ-5 before slice 02; OQ-4 and OQ-6 before slice 03; OQ-1 before slice 04.

### [REF] DoR Validation

| DoR item | PL-01 | PL-02 | PL-03 | PL-04 | PL-05 | Evidence |
|---|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | PASS | Drifting names with no door for the owner; dead projects everywhere; a half-hidden project; no way back; a team blocked forever |
| 2. Persona specific | PASS | PASS | PASS | PASS | PASS | Lena (lead, not admin), Priya (workspace admin and operator), Sam (member), Marco, Hugo (Globex admin) |
| 3. 3+ domain examples, real data | PASS (5) | PASS (5) | PASS (5) | PASS (5) | PASS (5) | Identity Platform/AUTH-7, Auth v2 Spike/SPIKE (3 issues, 2 comments, spike-notes.png), Homelab Ops/OPS, Reading List/READ, Labs/PROTO |
| 4. UAT 3-7 scenarios | PASS (5) | PASS (4) | PASS (5) | PASS (4) | PASS (4) | Business-outcome titles |
| 5. AC from UAT | PASS | PASS | PASS | PASS | PASS | Each AC cites its scenarios or decisions |
| 6. Right-sized | PASS | PASS | PASS | PASS | PASS | 0.5-1.5 days each, 4-5 scenarios |
| 7. Technical notes | PASS | PASS | PASS | PASS | PASS | Per story, plus INV-PL-1 to INV-PL-9 and NFR-PL-1 to NFR-PL-6 |
| 8. Dependencies tracked | PASS, conditional (OQ-2, OQ-3) | PASS, conditional (OQ-3, OQ-5) | PASS | PASS, conditional (OQ-3, OQ-4, OQ-6) | PASS, conditional (OQ-1) | Every OQ has a recommended answer, and the stories are written to it |
| 9. Outcome KPIs | PASS | PASS | PASS | PASS | PASS | PL-KPI-1 to PL-KPI-7 with targets, baselines and methods |
| job_id + Elevator Pitch | PASS | PASS | PASS | PASS | PASS | `job-project-lifecycle`. Every After line names a real route or CLI command and the exact text seen. |

DoR status: **PASSED on the recommended answers.** Item 8 is conditional on
the user's answers to OQ-1 to OQ-6 (a sequencing rule, not a block: each slice
starts DESIGN once its OQs are answered); US-PL-03 depends on none. Peer
review: see [REF] Peer Review.

### [REF] Wave Decisions

- Feature type: user-facing. No walking skeleton (brownfield). UX depth: lightweight. JTBD: a new job, `job-project-lifecycle`. Research depth: comprehensive, spent on the code-grounded inventory of every project address and reader (the riskiest assumption, INV-PL-2).
- **Risk: no DIVERGE or DISCOVER artifacts.** No `recommendation.md` or `job-analysis.md` exists. The job is grounded in the user's locked decisions, team-lifecycle's follow-up and TL-KPI-5, and code reading, not interviews. Probability medium, impact low (single-operator homelab; the user is the customer). Mitigation: every OQ is put to the user before DESIGN.
- Density: lean, `ask-intelligent`. Triggers evaluated:
  - **Cross-context complexity: fired.** Three contexts (project catalogue, teams, operator CLI). Suggests `alternatives-considered`, mainly for OQ-1.
  - **Multi-stakeholder need: fired.** Five personas (Lena, Priya as admin and as operator, Sam, Marco, Hugo). Suggests `persona-narrative`.
  - **Compliance / regulatory: fired on the term only.** ACs reference an audit record (OQ-3). No regulation is involved.
  - **AC ambiguity: not fired.** "Hidden" (D5) and "nothing erased" (D6) each have one definition, and the address inventory is listed.
  - **WS strategy = D: not fired.**
  - No expansion is rendered. All are offered at handoff.
- Upstream changes: none to DISCOVER (none exists). Partly closes team-lifecycle's follow-up "OQ-4 (b)/(c): project delete or move" (delete only). Amends, at DISTILL and only if OQ-1 is (b), team-lifecycle's INV-TL-1 and OUT-25.
- SSOT updated here: `docs/product/jobs.yaml` (`job-project-lifecycle`), `docs/product/personas/persona-instance-operator.yaml` (reference and job), `docs/product/journeys/journey-project-lifecycle.yaml` (new). The outcomes registry and `kpi-contracts.yaml` are updated at DISTILL, as team-lifecycle did.

### [REF] Peer Review

Reviewer: nw-product-owner-reviewer, 2026-10-12.

- **Iteration 1: rejected pending revisions** (1 critical, 4 high). All were edit-level and are fixed:
  - critical, no NFRs → [REF] Non-Functional Requirements added (accessibility, list latency, all-or-nothing failure, record retention, security);
  - high, KPI ids did not match → the table rows are PL-KPI-1..7, the ids the stories cite;
  - high, technical ACs (store queries, record commits) → ACs restated as outcomes; the verification method moved to Technical Notes;
  - high, US-PL-03 Elevator Pitch → the After line now shows the observable 404 page and lists, and Decision enabled names a decision;
  - high, queued notifications of a deleted project undefined → D17 and US-PL-03 example 6 and AC.
- Medium and low items also applied: US-PL-03 re-estimated at 1.5 days (US-PL-02 0.5 day; slice 02 still 2 days); the detached projects' keys staying taken is stated on the team delete page and in OQ-1 (b)'s cost; `${project_id}` added to Shared Artifacts; the `auth` slug's origin stated; US-PL-05's Before reworded; DoR item 8 worded as conditional; a scenario title reworded; the journey's sample timestamp and refusal failure modes fixed.
- Not applied: a web database-unavailable scenario per write (medium). NFR-PL-4 states the all-or-nothing requirement on the shipped error page; DISTILL may add a scenario.
- Iteration 2 was not run: no critical or high item remained after the fixes, and each fix is mechanical. Max-2-iterations rule respected.
