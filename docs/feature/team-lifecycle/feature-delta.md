<!-- markdownlint-disable MD024 -->
# Feature Delta: team-lifecycle

A workspace admin can create a team, rename it, and delete it, from the product.
Today the only production writer of `teams` is bootstrap (the first workspace's
"General"). Every workspace provisioned from `/admin/instance/workspaces` has
zero teams, so nobody there can be put on a team, create a project, or open a
board. This is the "team create, rename and delete" follow-up recorded by
team-membership (D14, evolution doc 2026-10-10).

## Wave: DISCUSS

Product owner: Luna (nw-product-owner) | Date: 2026-10-11 | Feature type:
user-facing (Decision 1) | Walking skeleton: brownfield, judged against the
existing structure (Decision 2, see WS Strategy) | UX depth: lightweight, happy
path plus the main refusals (Decision 3) | JTBD: yes, new job (Decision 4) |
Density: lean, Tier-1 [REF] only, `expansion_prompt = ask-intelligent` |
Requirements readiness: rough idea, explored here (user).

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/feature/team-membership/feature-delta.md` (all waves) | ✓ | D5 uniform 404, D10 tenancy from the session, D13 idempotent writes, D15 copy style, the Authorization Matrix, `TeamAccess::of`, DDD-6 to DDD-12. This feature extends that matrix and reuses its policy unchanged. |
| `docs/evolution/2026-10-10-team-membership.md` | ✓ | Its follow-up "Team create, rename and delete" is closed by this feature. OQ-D9 (user delete of a sole lead) stays open and is unrelated. |
| `docs/product/architecture/brief.md` | ✓ | "Names are labels; slugs are identity" (governs rename, D4), the project-name rule (governs the team-name rule, D6), and "Teams, team authority and the never-leaderless guard". The guard section says a future team delete meets the 0021 trigger and that the cascade exemption covers it. |
| `adr-team-membership-001/002/003` | ✓ | 001: the lead guard exempts a team that is being deleted, so a cascade is allowed. 002: `default_team_id` clears itself `ON DELETE SET NULL (default_team_id)`. 003: `TeamAccess` (admin → Manage, lead → Manage, member → View). |
| `adr-project-name-001/002` | ✓ | One value object on every door, the shared refused-character predicate, "unique" defined as a case-insensitive name or a derived slug that collides, and a fallback mint plus bounded retry for names that slugify to nothing. |
| `adr-project-rename-001/002` | ✓ | Slugs are minted once and never re-derived. Rename is a services use-case. The duplicate-label race was accepted there. |
| `adr-workspace-name-001` | ✓ | The copy is the error's `Display`. A check-arch literal rule. |
| `adr-issue-delete-001/002`, `adr-board-lane-002` | ✓ | Delete confirm is a GET+POST pair that works without scripting, and the confirm page counts consequences. The lane delete binds at confirm time, its count is advisory, and the FK guards the race. |
| `docs/product/jobs.yaml`, `personas/persona-instance-operator.yaml` | ✓ | No job covers team lifecycle. `job-team-membership` starts from an existing team. A new job, `job-team-lifecycle`, is appended. The persona gains only `also_referenced_by` and the job id. |
| House format: team-membership DISCUSS and `slices/slice-0N-*.md` | ✓ | Followed. |
| `docs/product/vision.md`, DISCOVER / DIVERGE artifacts | ⊘ | Do not exist. Grounded in the team-membership follow-up, code reading and the user's scope. This is a recorded risk, as in every precedent. |

No contradiction with prior evidence.

### [REF] Lifecycle inventory (code-grounded)

| Fact | Source |
|---|---|
| `teams(id, workspace_id, name, slug, UNIQUE(workspace_id, slug))`. There is no name uniqueness and no name rule (no `TeamName`, no 0019/0020-style trigger on `teams.name`). | `0001_init.sql`; `foundry-core/src` has `WorkspaceName`, `ProjectName`, `name_chars::is_refused_name_char` and `slugify` only |
| A team delete cascades to `projects`, and from them to `issues`, `lanes`, `issue_change_events`, `comments` and `issue_attachments` (stored inline as bytea). It also cascades to `team_memberships` (the 0021 guard is exempt) and to `machine_tokens.scope_team_id`. `workspaces.default_team_id` is set to NULL. | `0001`, `0004`, `0005`, `0007`, `0013`, `0015`, `0021`, `0022` |
| **No project delete or move exists** in production code. | grep `DELETE FROM projects`: acceptance fixtures only |
| The only production `INSERT INTO teams` is `seed_initial_workspace` (General/`general`, bootstrap user as lead). `provision_workspace` writes the workspace, admin user, admin membership and invite, and **no team**. | `foundry-store/src/lib.rs:3583`, `:2674` |
| Project create (`/team/{team_slug}/projects/new`, `POST /team/{team_slug}/projects`) is gated on being on the team. The members page has no link to it. The dashboard's empty state links to the hardcoded `/team/general/projects/new`. | `lib.rs:565-568`; `templates/team_members.html`; `templates/dashboard_root.html:14` |
| `/teams` shows the team list for an admin even when it is empty (an empty `<ul>`). Admins also see the default-team form there. | `templates/teams_index.html` |
| `workspace_rename_events` is an append-only record (actor · old → new · when), written in the rename's transaction and cascading with its workspace. | `0018_workspace_rename_events.sql` |

### [REF] Persona

- **Priya Raman** (`persona-instance-operator`): instance admin, and workspace admin of Canzan Labs. She provisioned the workspaces herself (persona file), including **Homelab**, which has zero teams today.
- **Lena Park**: lead of Backend in Canzan Labs, a workspace `member` (not an admin). She is the lead actor for rename (OQ-1). Referenced inline, as in team-membership.
- **Sam Okafor**: a Member of Backend only. He is the person whose only team disappears.
- **Marco** (`persona-team-member-foil`): signed in, no rights. He is the uniform-404 foil.

### [REF] JTBD

**job_id: `job-team-lifecycle`** (appended to `docs/product/jobs.yaml`).

One-liner: *When the way my workspace's work is split changes (a new workspace
with no teams, a team whose name no longer fits, a team created by mistake), I
want to create, rename or delete teams myself, so every workspace can reach a
board and its team list matches how we actually work, without psql and without
losing a project by accident.*

Job stories:

1. When I provision a workspace and its admin signs in to find no team, I want to create the first team, so a project and a board can exist there at all.
2. When a team's work changes ("Backend" becomes "Platform Engineering"), I want to rename it, and everyone's bookmarks and board links should keep working.
3. When a team was created by mistake ("Bakcend") or has wound down, I want to delete it, and be told exactly what goes with it, and never lose a project I didn't know about.

Dimensions: **functional**: create, rename and delete a team from `/teams` and its members page, under one name rule. **Emotional**: a provisioned workspace stops being a dead end, and deleting stops being frightening. **Social**: a team's name says what the team does today.

Four forces:

- **Push**: provisioned workspaces have zero teams and no path to one. Every team beyond bootstrap's General was created in psql, with no name rule, so a NUL, a 500-character name or a duplicate is possible there.
- **Pull**: a "New team" form, a rename field and a "Delete team" button, the way every team tool works.
- **Anxiety**: one delete cascades to every project, issue, comment and attachment of the team. A rename breaks URLs. Two teams could end up with the same name.
- **Habit**: psql works for one operator. The product path has to be safer (it never destroys an unseen project, it keeps slugs), not just prettier.

### [REF] Scope Assessment: PASS (5 stories, 3 contexts, about 4.5 days)

| Oversized signal | Fired? | Evidence |
|---|---|---|
| More than 10 user stories | No | 5 |
| More than 3 bounded contexts or modules | No (exactly 3) | teams (lifecycle and authz), the project catalogue (the delete refusal, the create race, the "New project" link), machine tokens (the cascade, stated on the confirm page) |
| Walking skeleton needs more than 5 integration points | No | Slice 01: one route pair, one store write (team plus lead row), one link on the members page, one dashboard link |
| More than 2 weeks of effort | No | about 4.5 days |
| Multiple independent outcomes that could ship separately | **Yes** | Create, rename and delete are independent outcomes |

One of five signals fires, so the verdict is **right-sized**. Every slice ships on
its own, and rename (slice 04) could split into its own feature with no rework.

### [REF] Locked Decisions

| ID | Decision | Source |
|---|---|---|
| D1 | User-facing feature. JTBD yes. UX depth lightweight. Walking skeleton brownfield: slice 01 is the thinnest end-to-end path (see WS Strategy). | User (Decisions 1-4) |
| D2 | **Create and delete are for workspace admins of the acting workspace only.** A lead or member who requests either gets the uniform 404. An instance admin gets no implicit right and acts as a workspace admin, as in team-membership OQ-1. Rename is OQ-1. | User suggestion, PO endorses |
| D3 | **The creator becomes the new team's Lead**, written atomically with the team. A team is never born leaderless, so the never-leaderless guard protects it from the first commit. The creator can create a project on it at once, because project create is gated on being on the team. Board authz is unchanged (team-membership D9). | PO: without it, an admin creates a team they cannot use, and an unled team escapes INV-1 |
| D4 | **Rename changes the name only. The slug never changes.** Every URL (`/team/{slug}/members`, `/team/{slug}/project/{p}`), bookmark and token scope keeps working. There is no "change team URL" in this feature. | Brief "Names are labels; slugs are identity"; ADR-PROJECT-RENAME-001 |
| D5 | **Every refused request on a new or changed route is the uniform 404**, on both verbs and byte-identical to `resource_not_found_page`. That covers being signed out, an unknown slug, another workspace's team, and a caller without the right. A stated reason (a name refusal, or "still has projects") is shown only to an authorized actor, who already sees the team. | Team-membership D5, DDD-8 |
| D6 | **One team-name rule** on both doors (create, rename), as a `foundry-core` value object like `WorkspaceName` and `ProjectName`. It trims, then refuses empty, then any character from the **shared** refused-character predicate, then names over the cap (OQ-2). "Unique within the workspace" means a case-insensitive name match with a sibling, or a non-empty derived slug equal to a sibling's stored slug (the project rule's definition). The copy is the error's `Display`. | ADR-PROJECT-NAME-001, ADR-WORKSPACE-NAME-001 |
| D7 | **The slug is minted once, at create, and is never empty.** It is `slugify(name)`, or the fallback (OQ-3) when that is empty. It is never re-derived. | ADR-PROJECT-NAME-002, D4 |
| D8 | **Delete is a GET confirm page plus a POST**, and works with scripting disabled. The confirm page states every consequence before anything happens: the member count, a warning if the team is the workspace default, and the count of machine tokens scoped to it. | ADR-ISSUE-DELETE-002; the lane two-fate confirm |
| D9 | **A team delete never destroys a project or issue that the admin was not shown and did not choose to destroy.** This holds under concurrency (INV-TL-1). The policy that satisfies it is OQ-4. | PO: data-loss guardrail, independent of OQ-4 |
| D10 | **After a delete:** the workspace default clears itself if it pointed at the team (schema fact), and members on no other team land on the shipped no-team copy. Scoped machine tokens are deleted with the team. The slug is free for a later create. Authz is per request, so an old bookmark to a reused slug only reaches the new team for people on it. | Schema facts; team-membership D9 and INV-6 |
| D11 | **Tenancy comes from the session.** A team is resolved by (acting workspace, slug). No workspace id is parsed from the request. | Team-membership D10 |
| D12 | **Copy is locked** (DESIGN may change routes, not strings). See [REF] Copy. | PO, for testable ACs |
| D13 | **Routes are proposed, and DESIGN finalizes them.** `POST /teams` (create), `POST /team/{team_slug}/rename`, `GET` and `POST /team/{team_slug}/delete`. All are plain forms on the shared CSRF layer. | PO, following `/team/{slug}/…` and ADR-ISSUE-DELETE-002 |
| D14 | **A new team can reach a board in one more click.** The members page shows "New project" (to the shipped `/team/{slug}/projects/new`) to anyone on the team. The dashboard's "create your first project" link targets a team the caller is on (the first by name) instead of the hardcoded `general`. | PO: in a provisioned workspace the hardcoded link is a 404. The walking skeleton needs this. |
| D15 | **A rename that changes nothing writes nothing.** A byte-equal name is not a write: no event, and a re-render with no notice. A case-only change ("ops" → "Ops") is a real rename. | ADR-WORKSPACE-NAME-001 / 0018 precedent |
| D16 | **A create is not deduplicated by name.** A double-submit of the create form gets the uniqueness refusal on the second POST. That is harmless and needs no special case. | PO |

### [REF] Copy (D12)

- Created: "Created {team}. You're its lead."
- Name refusals (`Display`): "Team name must not be empty" · "Team name must not contain control characters" · "Team name must be at most 64 characters" · "Team name must be unique within the workspace"
- Renamed: "Renamed {old} to {new}."
- Admin with no teams, on `/teams`: "This workspace has no teams yet. Create one to start a project."
- Confirm page: heading "Delete {team}?". Then "{team} has {n} member(s) and no projects. They lose access to {team}. This can't be undone." When the team is the default: "{team} is the default team for new members. After it is deleted, new members will join no team." When tokens are scoped to it: "{k} machine token(s) scoped to {team} will stop working." Buttons **Delete team** and **Cancel**.
- Refused (projects, OQ-4): "{team} still has {n} project(s): {list}. A team with projects can't be deleted." The list reads "Identity Platform (AUTH), Homelab Ops (OPS)". There is no **Delete team** button.
- Deleted: "Deleted {team}."
- Labels: the form button is **Create team** and the field is **Team name**. On the members page: **Rename**, **Delete team**, **New project**. Plurals follow the shipped "General · 1 member" style.

### [REF] Open Questions (each has a recommended answer; the stories assume it)

- **OQ-1: May a team's lead rename their own team?** *Recommended: yes.* Rename uses `TeamAccess` `Manage` (workspace admin, or lead of this team), with no new arm. Renaming is reversible and loses nothing, and leads already manage membership. Create and delete stay admin-only (D2).
- **OQ-2: The team-name length cap?** *Recommended: 64 Unicode scalars.* Team names show in lists, headings, copy and token labels. 24 (workspace) is tight for "Platform Engineering Guild", and 256 (project) is far wider than any team name. 64 makes no legacy name non-compliant, because the rule applies only to new writes.
- **OQ-3: The fallback slug for names that slugify to nothing** ("インフラ", "運用", "🚀")? *Recommended: `team`, then `team-2`, `team-3`, …, the lowest free suffix from 2*. This is the project rule's mechanism with `team` in place of the key prefix, which teams lack. A concurrent create that takes the same fallback is retried a bounded number of times and never reported as a duplicate name (ADR-PROJECT-NAME-002 decision 3). Transliteration and UUID fragments were rejected there and are rejected here.
- **OQ-4: Deleting a team that has projects.** The options:
  - (a) refuse while the team has any project;
  - (b) a confirm page listing every project and issue count, then cascade, perhaps with the name typed to confirm;
  - (c) move the projects to another team first.

  *Recommended: (a) refuse.* It is the only option with zero data-loss risk. There is no undo, trash or archive (ADR-ISSUE-DELETE-001), so a cascade would destroy every issue, comment and inline attachment of every project, on one click by one admin. (c) needs a project move, which does not exist and changes issue keys, URLs and authz (a separate feature). The cost is that a team with projects cannot be deleted from the product until a project delete or move exists, which is recorded as a follow-up. The main jobs (a typo team, a team that never started) are covered. (b) remains the designated next step if KPI-TL-5 shows real demand.
- **OQ-5: Should a provisioned workspace get a first team automatically?** *Recommended: no, not in this feature.* With create shipped, the provisioned admin sees "This workspace has no teams yet. Create one to start a project." on `/teams`, which is one path for existing and future workspaces alike. An auto-"General" names a team nobody chose, and existing zero-team workspaces would still need the create path. This can be revisited as its own decision if dogfood shows every admin creating "General" anyway.
- **OQ-6: Do create, rename and delete record an event?** *Recommended: yes for rename and delete, and no for create.* Following `workspace_rename_events`, each records one append-only row in the same transaction as the write:
  - rename: actor · old name → new name · when;
  - delete: actor · name · slug · when.

  The record must outlive the team, so it is workspace-scoped and keeps the name and slug as text. It has no UI in this feature (it is read with psql or the per-workspace export). Create is evident from the team's first lead, and it destroys nothing.
- **OQ-7: Should the database refuse a bad team name, as 0019/0020 do for workspaces and projects?** *Recommended: yes.* Apply the shared `foundry_name_rule_violation` function to `teams.name` at cap 64, on insert and on a name change only, so legacy names stay. It lands with slice 03. Seeds and fixtures that write `teams` directly (about 40 acceptance sites) use compliant names already.

**Resolved by the user (2026-10-11):** every OQ takes its recommended answer. OQ-1 (a lead may rename their own team), OQ-4 (refuse delete while the team has projects), OQ-6 (audit records for rename and delete) and OQ-7 (a DB name rule at cap 64) were answered by the user; OQ-2 (cap 64), OQ-3 (`team`, `team-2`, … fallback) and OQ-5 (no auto first team) were taken by the orchestrator on the recommendation. DoR item 8 passes for every story with no block.

**Resolved by the user:** pending. If any OQ is answered against its recommendation, the affected ACs are edited before DESIGN. Each OQ names the story it touches in the DoR table.

### [REF] Journey (lightweight: happy path plus main refusals)

Emotional arc, **from dead end to tidy**: stuck (a provisioned workspace has no
team, and "create your first project" is a 404) → in control (one form, a team
with her as lead) → productive (a project and a board on it) → confident
(renames keep every link; a delete says what goes and refuses to lose a project).

```text
[Where]                          [Does]                                     [Sees]                                                         [Feels]
/teams (Homelab, admin)          opens Teams                                "This workspace has no teams yet. Create one to start a project." stuck
/teams                           types "Ops", clicks Create team            /team/ops/members: "Created Ops. You're its lead."             in control
                                                                            "Priya Raman · priya@canzan.dev · Lead"
/team/ops/members                clicks New project                         /team/ops/projects/new (shipped form)                          productive
/team/ops/projects               creates "Homelab Ops" (OPS)                /team/ops/project/homelab-ops board (200)                      productive
/team/ops/members                renames to "Homelab Ops Team", Rename      "Renamed Ops to Homelab Ops Team." URL still /team/ops/…       confident
/team/bakcend/members            clicks Delete team                         "Delete Bakcend? Bakcend has 1 member(s) and no projects…"     wary
/team/bakcend/delete             clicks Delete team                         /teams: "Deleted Bakcend."                                     confident
```

Main refusals and edge paths:

| Step | What happens | Sees |
|---|---|---|
| Lena (lead, not admin) posts a create, or opens `/team/backend/delete` | refused | the uniform 404 (D2, D5) |
| Marco posts a rename on Backend | refused | the uniform 404 (D5) |
| Create "backend" while Backend exists | refused, form keeps the text | "Team name must be unique within the workspace" |
| Create "インフラ" | created | slug `team` (OQ-3), URL `/team/team/members` |
| Delete Backend, which has Identity Platform (AUTH) | refused, nothing changes | "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted." |
| Lena creates a project on Ops while Priya's Ops confirm page is open, then Priya confirms | refused, nothing changes | the projects refusal, naming Lena's project (INV-TL-1) |
| Delete Ops, the workspace default | confirm page warns; after delete the default is "No team" | the default warning line |

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| `${team}` (name) | `teams.name` | `/teams`, members page heading, copy, nav, token scope labels | MEDIUM: rename must reach every consumer by reading, never by caching |
| `${team_slug}` | `teams.slug`, minted once (D7) | every `/team/{slug}/…` URL, bookmarks | HIGH: rename must not touch it (D4) |
| Acting workspace | session `SessionUser.workspace_id` | every route | HIGH: the tenancy boundary (D11) |
| Project list of the team | `projects` by `team_id`, at commit time | confirm page, refusal copy, INV-TL-1 | HIGH: confirm-page counts are advisory; the commit-time check is binding |
| Default team | `workspaces.default_team_id` | `/teams` select, confirm warning, newcomer placement | MEDIUM: clears itself (schema) |
| Scoped token count | `machine_tokens.scope_team_id` | confirm page | LOW: advisory |
| Name-rule copy | the team-name error's `Display` (D6) | create and rename refusals | MEDIUM: one source, two doors |
| Uniform 404 | `resource_not_found_page` | every refusal (D5) | HIGH |

### [REF] Story Map and Slices

Backbone: **Start a team → Use it → Name it right → Retire it**

| Slice | Stories | What it adds | Est. | Brief |
|---|---|---|---|---|
| 01 (walking skeleton) | US-TL-01 | Admin creates a team on `/teams` and becomes its lead, then creates a project on it. **Unblocks provisioned workspaces.** | 1 d | `slices/slice-01-admin-creates-team.md` |
| 02 | US-TL-02, US-TL-03 | Admin deletes a team through a confirm page. A team with projects is refused, including a project created mid-confirm. | 1.5 d | `slices/slice-02-delete-team-never-loses-a-project.md` |
| 03 | US-TL-04 | The full team-name rule: control characters, the cap, the non-Latin fallback slug, the concurrent same-name create, and the DB rule (OQ-7). | 1 d | `slices/slice-03-team-name-rule-and-fallback-slug.md` |
| 04 | US-TL-05 | An admin or a lead renames a team. The slug and every link stay. | 1 d | `slices/slice-04-rename-team-keeps-its-urls.md` |

The walking skeleton covers "Start a team" and "Use it". "Name it right" and
"Retire it" are not needed for the provisioned-workspace dead end, so they stay
below the line.

Priority by learning leverage:

1. **01** first. It resolves the zero-team dead end, is dogfooded the same day on a provisioned dev workspace, and settles creator-as-lead (D3) and the D14 links.
2. **02** next. It carries the riskiest assumption: that a delete can be guaranteed never to destroy a project, including one created while the confirm page is open (INV-TL-1). Learning that early costs least. It is 1.5 days, over the one-day taste test, **on purpose**: the projects refusal is the safety net of delete and must never ship without it, and an infrastructure-only slice is forbidden.
3. **03** hardens the rule both doors share, before the second door (rename) arrives. It holds the second-riskiest assumption: that the project fallback-slug mechanism transfers to teams, which have no key.
4. **04** last. It has the lowest uncertainty (no slug change, the shipped policy), and it reuses 03's rule and parity.

Taste tests: no slice ships 4 or more new components. No slice is infrastructure
only: the race guarantee rides inside slice 02's user value. Every slice names what
it disproves (see the briefs). Every dogfood uses dev's real workspaces. No two
slices differ only in scale.

### [REF] User Stories

#### US-TL-01: A workspace admin creates a team, becomes its lead, and can start a project on it

`job_id: job-team-lifecycle` | slice 01

##### Elevator Pitch

Before: in Homelab, a workspace provisioned from `/admin/instance/workspaces`, Priya's `/teams` is an empty list, and the dashboard's "create your first project" link answers 404. Nothing in the product creates a team.
After: on `/teams`, type "Ops" in **Team name** and click **Create team** → `/team/ops/members` shows "Created Ops. You're its lead." and "Priya Raman · priya@canzan.dev · Lead". Then **New project** → create "Homelab Ops" (OPS) → `/team/ops/project/homelab-ops` shows the board.
Decision enabled: Priya can organise Homelab's work into teams and start boards there without psql.

##### Problem

Every provisioned workspace has zero teams. Its admin cannot put anyone on a team
or create a project, and the dashboard advertises a project link that 404s.

##### Who

- Workspace admin | `/teams`, desktop browser | wants a first or an additional team.

##### Domain Examples

1. **First team (dogfood)**: in Homelab (zero teams), Priya creates "Ops" → slug `ops`, she is Lead, `/teams` lists "Ops · 1 member", and she creates "Homelab Ops" (OPS) from **New project**.
2. **Second team**: in Canzan Labs, Priya creates "Design" next to General, Backend and Ops. She becomes Design's lead, and Lena (not on it) does not see Design on her `/teams`.
3. **Duplicate**: Priya types "backend" in Canzan Labs → "Team name must be unique within the workspace", and the field keeps "backend".
4. **Empty**: "   " → "Team name must not be empty".
5. **Refused**: Lena (lead of Backend, not admin) posts a create → uniform 404, and no team is written. A non-admin sees no create form on `/teams`.

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin creates the first team of a workspace and leads it

- Given Priya is workspace admin of Homelab, which has no teams
- When she opens Teams, enters "Ops" and creates it
- Then she sees "Created Ops. You're its lead." on Ops's members page, listing "Priya Raman · priya@canzan.dev · Lead"
- And Teams lists "Ops · 1 member"

###### Scenario: The new team's lead starts a project and opens its board

- Given Priya has just created Ops in Homelab
- When she follows New project and creates "Homelab Ops" with key OPS
- Then the Homelab Ops board opens, and her dashboard lists "OPS Homelab Ops"

###### Scenario: A name already used in the workspace is refused

- Given Canzan Labs has team Backend
- When Priya creates a team named "backend"
- Then she sees "Team name must be unique within the workspace", her text is kept, and Canzan Labs still has one team called Backend

###### Scenario: An admin of a workspace with no teams is told how to start

- Given Homelab has no teams
- When Priya opens Teams
- Then she sees "This workspace has no teams yet. Create one to start a project." and the create form

###### Scenario: Only a workspace admin can create a team

- Given Lena leads Backend but is not a workspace admin
- When Lena submits a team create, and Marco does too
- Then both answer the uniform not-found page, and no team is created

##### Acceptance Criteria

- [ ] A workspace admin's create writes one team and one Lead row for the creator together, and lands on the new members page with the D12 copy (scenario 1; D3).
- [ ] The members page shows **New project** to anyone on the team. The dashboard's empty-state project link targets a team the caller is on, and never a team they are not on (scenario 2; D14).
- [ ] An empty name or a non-unique name (case-insensitive name, or a colliding derived slug) is refused with the D12 copy, the input is kept, and nothing is written (scenario 3; D6).
- [ ] An admin on a workspace with no teams sees the empty-state copy and the form. A non-admin sees no form (scenario 4).
- [ ] A non-admin, signed-out or instance-admin-only caller gets the uniform 404 with zero rows written (scenario 5; D2, D5).
- [ ] It works with scripting disabled.

##### Outcome KPIs

KPI-TL-1, KPI-TL-2, KPI-TL-4.

##### Technical Notes

- A new `foundry-core` team-name value object, sharing `is_refused_name_char`. Slice 01 needs at least the empty and unique arms. DESIGN may land every arm here, and slice 03 then proves the rest.
- The slug is `slugify(name)`. An empty slugify result is refused in slice 01 ("must not be empty" is wrong copy for it, so DESIGN may simply land OQ-3 here).
- INV-TL-7 (team and lead row are atomic). No schema change is expected.

##### Size

1 day | 5 scenarios | slice 01

#### US-TL-02: A workspace admin deletes a team after a confirm page that says exactly what goes

`job_id: job-team-lifecycle` | slice 02

##### Elevator Pitch

Before: a team created by mistake ("Bakcend") stays in every list forever, unless someone runs psql, and in psql nothing says what a delete would cascade to.
After: on `/team/bakcend/members`, click **Delete team** → `/team/bakcend/delete` shows "Delete Bakcend? Bakcend has 1 member(s) and no projects. They lose access to Bakcend. This can't be undone." Click **Delete team** → `/teams` shows "Deleted Bakcend.", and Bakcend is gone from the list.
Decision enabled: Priya can tidy away dead teams, knowing beforehand who and what is affected.

##### Problem

Mistaken and wound-down teams accumulate. The only removal is psql, which
cascades silently to projects, issues, memberships and tokens.

##### Who

- Workspace admin | members page, confirm page | removes a team.
- Sam Okafor | a member whose only team is deleted.

##### Domain Examples

1. **Typo team**: Priya deletes "Bakcend" (1 member, Sam; no projects) → "Deleted Bakcend.". Sam's next dashboard shows "You're not on any team yet — ask a workspace admin to add you."
2. **Default team**: Ops is Canzan Labs's default. Its confirm page adds "Ops is the default team for new members. After it is deleted, new members will join no team." After the delete, `/teams` shows "No team" selected.
3. **Scoped token**: Design has 1 machine token scoped to it. The confirm page adds "1 machine token(s) scoped to Design will stop working."
4. **Cancel**: **Cancel** returns to the members page, and nothing changes.
5. **Refused**: Lena (lead of Backend) opens `/team/backend/delete` → uniform 404. Marco posts a delete → uniform 404.

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin deletes a team that has no projects

- Given Canzan Labs has team Bakcend with member Sam Okafor and no projects
- When Priya opens Delete team on Bakcend, reads "Delete Bakcend? Bakcend has 1 member(s) and no projects. They lose access to Bakcend. This can't be undone." and confirms
- Then she sees "Deleted Bakcend." and Teams no longer lists Bakcend

###### Scenario: A member whose only team is deleted lands on the no-team page

- Given Sam Okafor's only team was Bakcend, and Priya has deleted it
- When Sam opens his dashboard
- Then he sees "You're not on any team yet — ask a workspace admin to add you."

###### Scenario: Deleting the default team warns first and leaves no default

- Given Ops is Canzan Labs's default team and has no projects
- When Priya opens Delete team on Ops
- Then the page also says "Ops is the default team for new members. After it is deleted, new members will join no team."
- And after she confirms, the default team on Teams is "No team"

###### Scenario: Only a workspace admin can delete a team

- Given Lena leads Backend and is not a workspace admin
- When Lena opens Backend's delete page and submits a delete, and Marco submits one too
- Then every request answers the uniform not-found page, and Backend still exists

##### Acceptance Criteria

- [ ] The confirm page shows the D12 heading, the member count, the default warning (only when it is the default) and the scoped-token line (only when there are such tokens). It has **Delete team** and **Cancel** (scenarios 1, 3; D8).
- [ ] The POST deletes the team, its memberships and its scoped tokens, then shows "Deleted {team}." on `/teams` (scenario 1; D10).
- [ ] Members on no other team see the shipped no-team copy. Members still on other teams keep them (scenario 2).
- [ ] A deleted default reads as "No team" afterwards (scenario 3; D10).
- [ ] A non-admin or signed-out caller, an unknown slug or a foreign team gets the uniform 404 on both verbs, and nothing is deleted (scenario 4; D2, D5).
- [ ] (OQ-6) One delete record (actor · name · slug · when) commits with the delete. Checked by a read-only store query.
- [ ] Both pages work with scripting disabled.

##### Outcome KPIs

KPI-TL-3, KPI-TL-4, KPI-TL-5.

##### Technical Notes

- The cascade is allowed by 0021's exemption for a team being deleted (INV-TL-8). Its regression guard stays green.
- The confirm page's member and token counts are advisory (as the lane delete's card count is). The project check is binding (US-TL-03).

##### Size

1 day | 4 scenarios | slice 02

#### US-TL-03: A team that has projects is never deleted, even if a project appears while the confirm page is open

`job_id: job-team-lifecycle` | slice 02

##### Elevator Pitch

Before: a team delete in psql silently destroys every project, issue, comment and attachment of the team. There is no undo.
After: on `/team/backend/delete`, Priya sees "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted." and there is no **Delete team** button. A confirm sent from a page opened before Lena created a project gets the same refusal, and nothing is deleted.
Decision enabled: Priya can delete teams without fearing she will lose a board, and she knows which projects keep a team alive.

##### Problem

Delete is the one irreversible lifecycle action, and the schema cascades from a
team to all its work. No project delete or move exists to empty a team first.

##### Who

- Workspace admin | confirm page | must never lose work by accident.

##### Domain Examples

1. **Has projects**: Backend has Identity Platform (AUTH) → refusal naming AUTH, and no Delete button.
2. **Two projects**: Ops has Homelab Ops (OPS) and Reading List (READ) → "Ops still has 2 project(s): Homelab Ops (OPS), Reading List (READ). A team with projects can't be deleted."
3. **Stale confirm**: Priya opens Design's confirm page (no projects). Lena then creates "Brand Refresh" (BRAND) on Design. Priya confirms → the refusal naming BRAND, and Design and BRAND are intact.
4. **Race**: Priya's delete and Lena's project create on Design commit at the same moment → either the project exists and Design is refused, or Design is deleted and Lena's create answers the uniform 404. A project is never created and then destroyed.

##### UAT Scenarios (BDD)

###### Scenario: A team with a project shows why it cannot be deleted

- Given Backend has project Identity Platform (AUTH)
- When Priya opens Delete team on Backend
- Then she sees "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted." and no Delete team button

###### Scenario: A confirm sent from a page opened before a project appeared is refused

- Given Priya opened Design's delete page while Design had no projects
- And Lena has since created project Brand Refresh (BRAND) on Design
- When Priya confirms the delete
- Then she sees "Design still has 1 project(s): Brand Refresh (BRAND). A team with projects can't be deleted." and Design and Brand Refresh still exist

###### Scenario: A delete racing a project create never destroys the project

- Given Design has no projects
- When Priya deletes Design at the same moment as Lena creates Brand Refresh on it
- Then either Design still exists with Brand Refresh, or Design is gone and Lena's create answered the uniform not-found page
- And no project row is ever destroyed by the delete

##### Acceptance Criteria

- [ ] The confirm page and the POST both refuse while the team has any project, with the D12 copy listing every project as "Name (KEY)", and nothing changes (scenarios 1, 2; OQ-4, D9).
- [ ] The refusal is decided at commit time, not from the page (scenario 2; INV-TL-1).
- [ ] Over 100 repeated delete-versus-create races on real Postgres: 0 projects destroyed, 0 responses of 500 (scenario 3; INV-TL-1, INV-TL-2).
- [ ] The refusal is shown only to a workspace admin. Everyone else gets D5's 404.

##### Outcome KPIs

KPI-TL-3.

##### Technical Notes

- INV-TL-1 and INV-TL-2 are requirements for DESIGN. The mechanism is DESIGN's choice (lane-002's FK guard and team-membership's row lock are the house precedents).
- The status for the refusal (409, as DDD-8) is DESIGN's choice.

##### Size

0.5 day | 3 scenarios | slice 02

#### US-TL-04: Every team name follows one rule, and a name in any script gets a working URL

`job_id: job-team-lifecycle` | slice 03

##### Elevator Pitch

Before: a team name typed in psql can hold a NUL, a bidi override or 500 characters, and a name like "インフラ" would mint an empty, unreachable slug.
After: on `/teams`, **Create team** with "インフラ" → `/team/team/members` shows "Created インフラ. You're its lead."; with "Platform\u{202E}Ops" → "Team name must not contain control characters"; with 65 characters → "Team name must be at most 64 characters".
Decision enabled: Priya can name teams in any language and trust that every team has a working page and an honest name.

##### Problem

Without one rule, team names drift from the workspace and project rules, and
non-Latin names make teams unreachable.

##### Who

- Workspace admin | `/teams` create form (and the rename field, slice 04).

##### Domain Examples

1. **Non-Latin**: "インフラ" → slug `team`. Then "運用" → slug `team-2` (OQ-3).
2. **Control**: "Platform" followed by U+202E and "Ops" → refused with the control copy. So is a pasted tab.
3. **Cap**: a 64-character name is created, and a 65-character one is refused (OQ-2).
4. **Collision by slug**: with Backend existing, "Back-End" (which slugifies to `backend`) → "Team name must be unique within the workspace".
5. **Race**: two admins create "Infra" at the same moment → one is created, and the other sees the uniqueness copy, never a 500. Two concurrent "インフラ" creates both succeed, as `team` and `team-2`.

##### UAT Scenarios (BDD)

###### Scenario: A name with no Latin letters gets a working team page

- Given Canzan Labs has no team with slug "team"
- When Priya creates "インフラ", then "運用"
- Then their members pages are /team/team/members and /team/team-2/members, and each opens

###### Scenario: Control characters and over-long names are refused

- When Priya creates a name containing U+202E, and a 65-character name
- Then she sees "Team name must not contain control characters" and "Team name must be at most 64 characters", and no team is created

###### Scenario: A name whose URL would collide is refused

- Given Backend exists
- When Priya creates "Back-End"
- Then she sees "Team name must be unique within the workspace"

###### Scenario: Two admins creating the same name at once get one team

- When two workspace admins create "Infra" at the same moment
- Then exactly one Infra team exists, and the other admin sees "Team name must be unique within the workspace"

###### Scenario: The database refuses a bad team name from any caller

- When a team name containing a NUL is written directly in psql
- Then the write is refused by the team-name rule (OQ-7)

##### Acceptance Criteria

- [ ] Every arm of D6 gives its D12 copy on create, with the shared predicate (a core property test shows the same verdict as the workspace and project rules) (scenarios 2, 3).
- [ ] An empty slugify result mints `team`, then `team-N` with the lowest free N from 2, never `""` (scenario 1; OQ-3).
- [ ] Concurrent same-name creates give one team and one uniqueness refusal. Concurrent fallback creates both succeed. 0 responses of 500 over 100 runs (scenario 4; INV-TL-4).
- [ ] (OQ-7) A new or changed `teams.name` that breaks a pure arm is refused by the database. Legacy rows are untouched (scenario 5).

##### Outcome KPIs

KPI-TL-6.

##### Technical Notes

- This mirrors ADR-PROJECT-NAME-001/002 and name-db-checks (0019/0020). A migration is needed only for OQ-7.

##### Size

1 day | 5 scenarios | slice 03

#### US-TL-05: A team is renamed, and every link to it keeps working

`job_id: job-team-lifecycle` | slice 04

##### Elevator Pitch

Before: Backend became the platform team, but its name can only change in psql, and the board links people bookmarked depend on whether psql also touched the slug.
After: on `/team/backend/members`, edit **Team name** to "Platform Engineering" and click **Rename** → "Renamed Backend to Platform Engineering." The heading changes, and `/team/backend/project/auth` still opens the Identity Platform board.
Decision enabled: Priya (or Lena, Backend's lead) can keep team names truthful without breaking anyone's links.

##### Problem

Team names go stale as work shifts, and a rename must never become a URL
migration.

##### Who

- Workspace admin, or the team's lead (OQ-1) | members page.

##### Domain Examples

1. **Admin**: Priya renames Backend to "Platform Engineering". `/teams` lists "Platform Engineering · 3 members" at `/team/backend/members`.
2. **Lead**: Lena renames Backend herself → the same result (OQ-1).
3. **Case-only**: "ops" → "Ops" → "Renamed ops to Ops." (D15).
4. **No-op**: submitting "Backend" unchanged → no write, no event, no notice (D15).
5. **Collision**: renaming Ops to "general" → "Team name must be unique within the workspace". After General is renamed "Platform", creating "General" is still refused, because its derived slug `general` is taken (D6, accepted).
6. **Refused**: Sam (member) and Marco post a rename → uniform 404.

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin renames a team and its links keep working

- Given Backend has project Identity Platform at /team/backend/project/auth
- When Priya renames Backend to "Platform Engineering"
- Then she sees "Renamed Backend to Platform Engineering." and Teams lists "Platform Engineering"
- And /team/backend/members and /team/backend/project/auth still open

###### Scenario: A team's lead renames their own team

- Given Lena leads Backend and is not a workspace admin
- When Lena renames Backend to "Platform Engineering"
- Then she sees "Renamed Backend to Platform Engineering."

###### Scenario: A rename to a name already in use is refused

- When Priya renames Ops to "general"
- Then she sees "Team name must be unique within the workspace" and Ops keeps its name

###### Scenario: Submitting the same name changes nothing

- When Priya submits Backend's rename form with "Backend" unchanged
- Then the page re-renders with no notice, and nothing is recorded

###### Scenario: Someone without the right cannot rename a team

- When Sam Okafor (a member of Backend) and Marco submit a rename of Backend
- Then both answer the uniform not-found page, and Backend keeps its name

##### Acceptance Criteria

- [ ] Rename writes the name only, never the slug, and shows the D12 copy. Every shipped URL under the slug answers as before (scenario 1; D4).
- [ ] Rename runs the same rule as create, with the same copy (the team itself is excluded from the uniqueness check) (scenario 3; D6).
- [ ] A workspace admin, or this team's lead, may rename (OQ-1). A member, an outsider or a signed-out caller gets the uniform 404 (scenarios 2, 5; D5).
- [ ] A byte-equal name writes nothing. A case-only change is a rename (scenario 4; D15).
- [ ] (OQ-6) One rename record (actor · old → new · when) commits with the rename. Checked by a read-only store query.

##### Outcome KPIs

KPI-TL-2, KPI-TL-4, KPI-TL-6.

##### Technical Notes

- Rename uses `TeamAccess` `Manage` with no new arm (ADR-TEAM-MEMBERSHIP-003). INV-TL-5 and INV-TL-6.

##### Size

1 day | 5 scenarios | slice 04

### [REF] System Constraints

These are invariant and concurrency requirements for DESIGN. They name the risks and
do not choose mechanisms.

- **INV-TL-1 (no lost project):** a team delete commits only if the team has no project at commit time. A project create racing it either commits first, and the delete is refused with the projects copy, or finds the team gone and gets the uniform 404. No project row is ever removed by a team delete (D9).
- **INV-TL-2 (no 500 from a vanished team):** any write racing a delete gets the team's normal outcome or the uniform 404, never a 500. That covers an add, role or remove, the default-team set, a token mint scoped to the team, a rename, and a project create. A newcomer accept or a Keycloak first sign-in racing the delete of the default team still succeeds (team-membership D17).
- **INV-TL-3 (confirm-time truth):** the confirm page's counts are advisory. The commit-time state decides (the lane-delete precedent).
- **INV-TL-4 (create uniqueness under concurrency):** concurrent creates of the same name, or of names with the same derived slug, give exactly one team, and the loser gets the uniqueness copy. Concurrent fallback-slug creates both succeed with distinct slugs (bounded retry, ADR-PROJECT-NAME-002).
- **INV-TL-5 (rename races):** a rename can never change or corrupt a slug. Whether two concurrent renames, or a rename and a create, may leave two teams with case-insensitively equal names is DESIGN's call. ADR-PROJECT-RENAME-002 accepted that residual for projects. If DESIGN accepts it, record it.
- **INV-TL-6 (authority at request time):** per request, against current rows. A stale form from a demoted lead or a removed admin gets the 404.
- **INV-TL-7 (born led):** a team row and its creator's Lead row commit together. No reader ever sees the new team without a lead.
- **INV-TL-8 (lead guard compatibility):** a team delete is never refused by 0021's lead guard (its cascade exemption). A rename touches no `team_memberships` row.
- **INV-TL-9 (audit atomicity, if OQ-6):** a rename or delete record commits or rolls back with its write, and survives the team's deletion.
- No new oracle: every refusal to an unauthorized caller is byte-identical to the uniform 404 (D5).

### [REF] Outcome KPIs

Objective: every workspace can reach a board, team names stay truthful, and no
team delete ever loses work.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| TL-1 | Workspaces with at least one member | Have at least one team | 0 zero-team workspaces on dev 1 day after the slice-01 dogfood (prod after approval) | dev and prod: every workspace provisioned from `/admin/instance/workspaces` (counted at the slice-01 dogfood) | Read-only query: workspaces with no `teams` row | North star |
| TL-2 | The operator | Creates and renames teams without psql | 0 hand-written `INSERT`/`UPDATE` on `teams` from the release onward | Every team beyond bootstrap's General was psql | Operator report at each dogfood | Leading |
| TL-3 | Team deletes | Destroy no project | 0 projects removed by a team delete. 0 lost projects and 0 responses of 500 over 100 delete-versus-create races | Not applicable (no delete path exists) | Race scenarios on real Postgres, plus a read-only project count before and after each dogfood delete | Guardrail |
| TL-4 | Callers without the right | Learn nothing and change nothing | 100% of the (actor × verb × team) matrix answers byte-identical 404s with 0 rows written | Not applicable (no routes) | Acceptance authz outline | Guardrail |
| TL-5 | Admins who try to delete a team with projects | Hit the refusal | Counted only, with no target. If it is above 0 in the first 30 days, it is evidence for a project delete or move feature (OQ-4) | 0 | Operator report | Leading (demand signal) |
| TL-6 | Renamed teams | Keep every link working | 100% of shipped URLs under the slug answer the same status after a rename. 100% verdict parity of create and rename on the rule | Not applicable | Acceptance scenarios, plus a core property test | Guardrail |

At homelab scale, the KPIs are verified by acceptance scenarios, read-only
store queries and the operator's own report, not analytics.

### [REF] DoD

1. All UAT scenarios pass: 5 + 4 + 3 + 5 + 5 = 22. The neighbouring lanes stay green: team-membership (tm-01..06), us-07 project create, project-name-rule, name-db-checks, member invites, keycloak-sso (46), fix-hide-unreachable-boards, dashboard-enhancements, machine-token-admin, and default.
2. The authz matrix (KPI-TL-4) covers being signed out, Marco, a plain member, a lead (allowed to rename if OQ-1 is accepted; refused to create and delete), a workspace admin, an instance admin with no workspace-admin row, an unknown slug and a foreign team, on every new route and both verbs.
3. INV-TL-1, -2 and -4 are proven by races on real Postgres, at least 100 runs each, with 0 lost projects and 0 responses of 500.
4. Nothing is written on a refusal (row counts asserted).
5. The pages work with scripting disabled.
6. `cargo xtask check-arch` and `cargo xtask smoke` pass before each commit. `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` passes before push. Mutation kill rate is at least 80% on modified files.
7. **Dogfood on dev**, after the user releases (dev auto-deploys release tags; prod is held for approval). In a provisioned workspace with zero teams, create a team and a project and open its board. Rename a team and re-open an old bookmark. Delete a typo team, and see a team with a project refused. Record KPI-TL-1 baselines (read-only) on dev and prod.
8. Upgrade: an additive migration only, if OQ-6 or OQ-7 needs one. It is rolling-deploy safe, and the CHANGELOG migration notes say so.
9. Documentation: a CHANGELOG `[Unreleased]` entry. `brief.md`'s Teams section gains "Team lifecycle" (DESIGN). An outcomes registry row is added at DISTILL. `jobs.yaml` and the persona were done here.

### [REF] Out of Scope

- Project delete, archive, or move between teams. A cascade delete of a team with projects (OQ-4 (b) and (c) are follow-ups).
- Changing a team's slug or URL, and redirects from old slugs (D4).
- Undo, trash or archive for teams.
- Auto-creating a first team on provisioning or backfilling zero-team workspaces (OQ-5).
- Team management by an instance admin who is not a workspace admin. A `/api/v1` or `foundry doctor` team command.
- A UI for rename or delete records (OQ-6 records only).
- Notifications to members of a renamed or deleted team.
- Team descriptions, avatars and settings beyond the name.
- Workspace delete.

### [REF] WS Strategy

Brownfield (Decision 2, judged against the structure). Sessions, tenancy, `/teams`,
members pages, `TeamAccess`, project create and boards already ship. The missing
piece is any product path into `teams`, so slice 01 is the walking skeleton:
`/teams` → **Create team** → members page as Lead → **New project** → board. It runs
on real adapters only (real Postgres, real router, real session), with no
configurable or environment-switched seam (not strategy D). Slices 02-04 extend it.

### [REF] Driving Ports

1. `GET /teams` (delta): the create form and the empty-state copy for a workspace admin.
2. `POST /teams` (new): create (US-TL-01, US-TL-04).
3. `GET /team/{team_slug}/members` (delta): the **Rename** field and button for Manage, **Delete team** for a workspace admin, and **New project** for anyone on the team.
4. `POST /team/{team_slug}/rename` (new): US-TL-05.
5. `GET /team/{team_slug}/delete` (new): the confirm page, or the projects refusal.
6. `POST /team/{team_slug}/delete` (new): delete, or the projects refusal.
7. `GET /` (delta): the empty-state project link targets a team the caller is on (D14).
8. `GET /team/{team_slug}/projects/new` and `POST /team/{team_slug}/projects` (unchanged): the walking skeleton's second half and the INV-TL-1 race partner.
9. `GET /team/{team_slug}/project/{project_slug}` (unchanged): the rename bookmark check.

Routes are proposals (D13). The copy is locked (D12).

### [REF] Pre-requisites

- team-membership has shipped (v0.13.0: `/teams`, members pages, `TeamAccess`, the 0021 guard with its cascade exemption, and 0022's self-clearing default). It is live on dev, and prod is awaiting approval.
- These already ship: `slugify`, `name_chars::is_refused_name_char`, the `ProjectName`/`mint_project_slug` precedent, `foundry_name_rule_violation` (0019), project create, `resource_not_found_page`, CSRF and sessions, and the scripting-disabled lane.
- Dev has at least one workspace provisioned from `/admin/instance/workspaces` with zero teams for the slice-01 dogfood. If it has none, Priya provisions "Homelab" there first.
- The OQs must be answered before DESIGN of the slices they touch: OQ-2, OQ-3 and OQ-7 before slice 03 (OQ-3 before slice 01 if DESIGN lands the fallback there); OQ-4 before slice 02; OQ-1 before slice 04; OQ-6 before slices 02 and 04. OQ-5 blocks nothing.

### [REF] DoR Validation

| DoR item | TL-01 | TL-02 | TL-03 | TL-04 | TL-05 | Evidence |
|---|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | PASS | Zero-team provisioned workspaces; typo teams; cascading psql delete; drifting names; stale names |
| 2. Persona specific | PASS | PASS | PASS | PASS | PASS | Priya (instance admin and workspace admin), Lena (lead, not admin), Sam (only team), Marco |
| 3. 3+ domain examples, real data | PASS (5) | PASS (5) | PASS (4) | PASS (5) | PASS (6) | Homelab/Ops/OPS, Canzan Labs Backend/AUTH, Bakcend, インフラ/運用, Brand Refresh (BRAND) |
| 4. UAT 3-7 scenarios | PASS (5) | PASS (4) | PASS (3) | PASS (5) | PASS (5) | Business-outcome titles |
| 5. AC from UAT | PASS | PASS | PASS | PASS | PASS | Each AC cites its scenarios or decisions |
| 6. Right-sized | PASS | PASS | PASS | PASS | PASS | 0.5-1 day each, 3-5 scenarios |
| 7. Technical notes | PASS | PASS | PASS | PASS | PASS | Per story, plus INV-TL-1 to INV-TL-9 |
| 8. Dependencies tracked | PASS (OQ-3) | PASS (OQ-6) | PASS (OQ-4) | PASS (OQ-2, OQ-3, OQ-7) | PASS (OQ-1, OQ-6) | Every OQ has a recommended answer, and the stories are written to it |
| 9. Outcome KPIs | PASS | PASS | PASS | PASS | PASS | KPI-TL-1 to TL-6 with targets, baselines and methods |
| job_id + Elevator Pitch | PASS | PASS | PASS | PASS | PASS | `job-team-lifecycle`. Every After line names a real route or UI action and the exact text seen. |

DoR status: **PASSED on the recommended answers**. Item 8 passes with a block
until the user answers OQ-1 to OQ-4, OQ-6 and OQ-7 (OQ-5 blocks nothing). Per-wave
peer review was not invoked. The consolidated review runs at the end of DISTILL.

### [REF] Wave Decisions

- Feature type: user-facing. Walking skeleton: slice 01, brownfield. UX depth: lightweight. JTBD: a new job, `job-team-lifecycle`.
- Density: lean, `ask-intelligent`. Triggers evaluated:
  - **Cross-context complexity: fired.** Three bounded contexts: teams, the project catalogue (the delete refusal and race, the New project link), and machine tokens (the cascade). Suggests `alternatives-considered`, mainly for OQ-4.
  - **Multi-stakeholder need: fired.** Four personas across the stories (Priya, Lena, Sam, Marco). Suggests `persona-narrative`.
  - **Compliance / regulatory: fired.** The US-TL-02 and US-TL-05 ACs reference an audit record (OQ-6). No regulation is involved, only the term. Suggests `journey-deep-dive` (user-facing).
  - **AC ambiguity: not fired.** The uniqueness AC shared by US-TL-01, -04 and -05 has one definition (D6) and one copy string. The surprising case (a renamed General still blocks a new "General") is written down as an example.
  - **WS strategy = D: not fired.**
  - No expansion is rendered. All are offered at handoff.
- Upstream changes: none to DISCOVER (none exists). This closes team-membership's "Team create, rename and delete" follow-up (D14).
