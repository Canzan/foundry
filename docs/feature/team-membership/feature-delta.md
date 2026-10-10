<!-- markdownlint-disable MD024 -->
# Feature Delta: team-membership

A workspace admin, or a team's lead, can see who is on a team and put a workspace
member on it, change a member's role, or take someone off it. A team can never
be left without a lead. A workspace can also name a default team, so people who
join through an invite or a Keycloak sign-in land on a team and can open its
boards. Today nothing in the product adds anyone to a team. This is the "way to
join teams" follow-up recorded by fix-hide-unreachable-boards (2026-10-09).

## Wave: DISCUSS

Product owner: Luna (nw-product-owner) | Date: 2026-10-09 | Feature type:
user-facing (Decision 1) | Walking skeleton: brownfield, judged against the
existing structure (Decision 2, see WS Strategy) | UX depth: lightweight, happy
path and main refusals (Decision 3) | JTBD: yes, new job (Decision 4) | Density:
lean, Tier-1 [REF] only, `expansion_prompt = ask-intelligent`.

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/product/jobs.yaml` | ✓ | No job covers team membership. `job-sso-signin` is the closest: it gets the operator signed in, and this job gets her to the board once she is. A new job, `job-team-membership`, is appended (see JTBD). |
| `docs/product/personas/persona-instance-operator.yaml` | ✓ | Priya Raman is reused as the workspace admin. Marco is reused as the authz foil. One evidenced characteristic is added (two accounts since SSO), plus `also_referenced_by` and `pains_addressed_to_date`. |
| `docs/product/journeys/*.yaml` (theme-adoption, issue-card-delete, card-drag-drop) | ✓ | Format skimmed. No SSOT journey is created: UX depth is lightweight, the same as the name-rule precedents. |
| `docs/product/architecture/brief.md` | ✓ | `establish_session` resolves the acting workspace and fails closed with none. OIDC provisioning creates one password-less `member` in the instance's original workspace (keycloak-sso D3a). The brief does not describe teams or team authz. Code is the source for those (below). |
| `docs/evolution/2026-10-09-fix-hide-unreachable-boards.md` | ✓ | This is the motivating incident. Its Follow-ups name "a workspace-admin 'add to team' action, or a default-team policy". This feature does both. Its second follow-up (link a Keycloak identity to an existing account) stays out of scope. |
| `docs/feature/name-db-checks/feature-delta.md`, `docs/feature/project-name-rule/feature-delta.md` | ✓ | House format, tone and level of detail. |
| `docs/product/vision.md`, `docs/project-brief.md`, `docs/stakeholders.yaml` | ⊘ | Do not exist. |
| DISCOVER / DIVERGE artifacts | ⊘ | None. Grounded in the incident, code reading and the user's decisions. This is a recorded risk, as in every precedent. |

No contradiction with prior evidence.

### [REF] Membership inventory (code-grounded)

| Fact | Source |
|---|---|
| `team_memberships(team_id, user_id, role IN ('lead','member'), PK(team_id, user_id))`, `teams(…, UNIQUE(workspace_id, slug))`, `workspace_memberships(…, role IN ('admin','member'))` | `crates/foundry-store/migrations/0001_init.sql` |
| The only production writers of `team_memberships`: bootstrap (`seed_initial_workspace`, the first user becomes `lead` of team General/`general`) and `foundry doctor add-test-user` (`Store::grant_all_memberships`, every workspace and team, as `member`) | `foundry-store/src/lib.rs:3489`, `:2937` |
| Invite-accept (`create_member_and_consume`) and Keycloak provisioning (`provision_federated_member`, oldest workspace) write a workspace membership only | `lib.rs:454`, `:548`; `oidc.rs:259` |
| **`provision_workspace` creates no team.** A workspace provisioned from `/admin/instance/workspaces` has zero teams, so nobody can be put on a team there and no project can be created. Out of scope (no team create, D14), but it bounds the default-team questions. | `lib.rs:2644` |
| `POST /workspace/members/remove` deletes the `workspace_memberships` row only. The person's `team_memberships` rows stay, including a `lead` row. | `member_invites.rs:248`, `lib.rs:2967` |
| The board and every project route are gated on `Store::is_team_member` within the acting workspace. A refusal is the uniform 404 (`resource_not_found_page`). | `lib.rs:1569`; routes `lib.rs:563-709` |
| The `lead` role grants nothing today. No handler reads it. | grep: no reader |
| Workspace-admin surfaces gate on `is_workspace_admin` and resolve the workspace from the session. A refusal is the uniform 404. | `member_invites.rs:310-340` |
| The dashboard shows "You're not on any team yet — ask a workspace admin to add you." when `is_on_any_team` is false. The quick actions are shown to everyone, and the targets gate server-side. | `templates/dashboard_root.html:12, 29-41` |

### [REF] Persona

- **Priya Raman** (`persona-instance-operator`) is workspace admin of Canzan Labs and a member of team Backend there (per the persona file). Since Keycloak provisioning she has **two accounts**: her bootstrap password account and a provisioned SSO account with a different email. In the dogfood, the bootstrap account is `jeffabailey@gmail.com` and the SSO account is `jeff@unintelligent-design.us` on dev. Both can show the same display name.
- **Lena Park**: lead of team Backend, a workspace `member` (not admin). She is the team-lead actor (US-TM-05). She is referenced inline, as prior features did, and is not added to the personas file (no evidence beyond her role).
- **Tomás Rivera**: a newcomer joining Canzan Labs through an invite (US-TM-06) or through Keycloak (US-TM-07).
- **Marco** (`persona-team-member-foil`): signed in, wrong team or no rights. He is the uniform-404 foil.

### [REF] JTBD

**job_id: `job-team-membership`** (appended to `docs/product/jobs.yaml`).

One-liner: *When someone in my workspace cannot open a team's boards, I want to see
who is on the team and put them on it, or have newcomers land on a team
automatically, so everyone reaches the boards they belong on without me
opening psql or running a test-user command.*

Job stories:

1. When my own SSO account shows "You're not on any team yet", I want to put it on team General from the product, so I can open Sandbox with it.
2. When a teammate joins Backend's work, I want to add them to Backend myself as its lead, without waiting for a workspace admin.
3. When the people leading a team change, I want to promote or demote leads and remove people, and trust that the team is never left with nobody in charge.
4. When I invite someone, or they sign in with Keycloak for the first time, I want them to land on our default team, so their first page is a board, not a dead end.

Dimensions: **functional**: put a workspace member on a team, change their role, or take them off, from one page per team, and default newcomers onto a team. **Emotional**: stop being told to "ask a workspace admin" when you are the workspace admin. **Social**: a teammate's first sign-in shows them the team's boards, not a polite refusal.

Four forces:

- **Push**: no product path writes `team_memberships`. The incident left a provisioned account on no team. The dashboard now says "ask a workspace admin to add you", but no admin can. The workarounds are psql, or `foundry doctor add-test-user`, which puts a user on every team in every workspace.
- **Pull**: a members list per team, with an Add control, the way every team tool works.
- **Anxiety**: leaking which teams exist to people outside them. Locking a team out of its own management by removing or demoting its last lead. Two leads demoting each other at once.
- **Habit**: `add-test-user` and psql work for a single operator. The product path must be scoped (one team, one person) and safer, not just prettier.

### [REF] Scope Assessment: PASS (7 stories, 3 contexts, about 5.5 days), with a separable half

| Oversized signal | Fired? | Evidence |
|---|---|---|
| More than 10 user stories | No | 7 |
| More than 3 bounded contexts or modules | No (exactly 3) | team membership and authz (new pages), invite onboarding (`invites_accept`), federated provisioning (`oidc` + `provision_federated_member`) |
| Walking skeleton needs more than 5 integration points | No | Slice 01 touches a route pair, two store reads, one store write and the dashboard link |
| More than 2 weeks of effort | No | about 5.5 days |
| Multiple independent outcomes that could ship separately | **Yes** | Managing membership (slices 01-04) and defaulting newcomers (slices 05-06) are independent outcomes |

One of five signals fires, so the verdict is **right-sized**. The default-team half
(slices 05-06) is genuinely separable: it depends only on slice 01. If the user
prefers, it splits into its own feature with no rework (OQ-10). Either way, every
slice ships alone.

### [REF] Locked Decisions

| ID | Decision | Source |
|---|---|---|
| D1 | User-facing feature. JTBD yes. UX depth lightweight. Walking skeleton judged brownfield: slice 01 is the thinnest end-to-end path over the existing stack (see WS Strategy). | User (Decisions 1-4) |
| D2 | **Who may add.** A workspace admin may add any member of the acting workspace to any team in it. A team lead may add workspace members to their own team. Nobody else may. | User |
| D3 | **In scope:** add, remove, change role (lead ⇄ member), a members page per team, and a default team for new members (invites and Keycloak). | User |
| D4 | **Dogfood.** On dev, signed in as `jeffabailey@gmail.com` (workspace admin, lead of General), add `jeff@unintelligent-design.us` to team General through the product. Then, signed in with Keycloak as that account, `/team/general/project/sandbox` answers 200 with the Sandbox board. It answers 404 today. | User |
| D5 | **Every refused request on the new team routes is the uniform 404**, on both verbs and byte-identical to `resource_not_found_page`. This covers being signed out, an unknown team slug, a team in another workspace, a caller without the right, and a target user who is unknown or not a member of the acting workspace. A refusal never says whether the team or the person exists. | Repo convention (lane routes, `/workspace/invites`) |
| D6 | **Never leaderless.** No write made through this feature may leave a team that has a lead with none. Removing or demoting the last lead is refused **with a stated reason**, not a 404 (the actor is authorized and already sees the team), and nothing changes. This must hold under concurrent requests (System Constraints, INV-1 to INV-6). | User |
| D7 | **Only current workspace members can be added.** Bringing in a new person stays with `/workspace/invites`. A person is always added as **Member**. Making them a lead is a separate role change. | PO: keeps add a one-choice action; promote is deliberate |
| D8 | **People are shown by display name and email** on every row, every candidate in the Add list, and every message. The incident's two accounts can share a display name. Only the email tells them apart. | PO, evidence from the incident |
| D9 | **Board authz is unchanged.** Team membership is still the only gate for boards. Managing a team (as workspace admin) grants no board access to it. | PO: one authz model, the one fix-hide-unreachable-boards made visible |
| D10 | **Tenancy comes from the session.** The team is resolved by (acting workspace, slug), exactly as the board does it. No workspace id is parsed from the request. | Repo convention (LAYER-1e) |
| D11 | **Who sees the members page.** A workspace admin sees it for every team in the workspace, and a team member (lead or member) sees it for their own team. Only a workspace admin or the team's lead sees the controls (Add, role change, Remove). A plain member sees the list read-only. Everyone else gets D5's 404. | PO: teammates already see each other's names on issues, so the list leaks nothing new to them |
| D12 | **One teams index, `/teams`**, linked as "Teams" from the dashboard quick actions. A workspace admin sees every team in the acting workspace. Anyone else sees only their own teams, and a member on no team sees today's no-team copy. | PO: the admin needs a way in. Without it, D4 depends on typing a URL. |
| D13 | **Writes are idempotent and need no confirm dialog.** Re-adding someone already on the team, or setting the role they already hold, changes nothing and says so. Remove and role change are single POSTs that re-render the page with the result. Both are reversible by the same actor, or by a workspace admin. | PO: reversible, low-blast-radius actions |
| D14 | **No team create, rename or delete. No team names.** The teams that exist today are the ones managed. | User (name rules out of scope) |
| D15 | **Copy is locked** (DESIGN may change routes, not strings). The strings are: "Added {name} ({email}) to {team} as a member." · "{name} ({email}) is already on {team}." · "{team} needs at least one lead. Make someone else a lead first." · "Everyone in this workspace is already on {team}." · "{name} ({email}) is now a lead of {team}." / "… is now a member of {team}." · "Removed {name} ({email}) from {team}." · "New members will join {team}." / "New members join no team." Role labels are "Lead" and "Member". | PO, for testable ACs |
| D16 | **Routes are proposed, and DESIGN finalizes them:** `GET /teams`, `GET /team/{team_slug}/members`, `POST /team/{team_slug}/members` (add), `POST /team/{team_slug}/members/{user_id}/role`, `POST /team/{team_slug}/members/{user_id}/remove`, and `POST /teams/default` (slice 05). All take double-submit `_csrf` and sit on the shared layer. They work with scripting disabled (plain forms; htmx optional). | PO, following the `/team/{slug}/…` convention |
| D17 | **Default-team writes are best-effort for the newcomer.** An invite accept or a Keycloak sign-in is never refused, and never fails, because of the default team. At worst the newcomer lands on no team, which is today's behaviour. | PO: the default is a convenience, and sign-in is critical |

### [REF] Open Questions (for the user; each has a recommended answer)

- **OQ-1: Are instance admins covered?** *Recommended: no.* An instance admin manages team membership by being a workspace admin of that workspace, as with `/workspace/invites` today. An implicit instance-admin bypass would make a workspace's team pages reachable to someone with no row in that workspace, which is a new cross-tenant path. The dogfood is unaffected, because the bootstrap account is also workspace admin.
- **OQ-2: Who may remove and change roles?** *Recommended: the same actors as add (D2).* A workspace admin may act on any team, and a lead on their own team. A lead may demote or remove another lead, or themselves, because leads are peers, and D6 protects the last one. Stories US-TM-03 to US-TM-05 are written to this answer.
- **OQ-3: Where is the default team configured?** *Recommended: on `/teams`.* One select, "Default team for new members", visible and usable by a workspace admin only (D5 for anyone else), listing the workspace's teams plus "No team".
- **OQ-4: One default per workspace?** *Recommended: yes, at most one per workspace, and unset on upgrade.* Unset means behaviour is unchanged. The dogfood sets General on dev. (Auto-defaulting the bootstrap team was considered. It is rejected because it silently changes who can see boards on upgrade.)
- **OQ-5: Do invites and Keycloak provisioning apply it equally?** *Recommended: yes.* Both create a workspace membership today, and both add the newcomer to the default team as **Member**, in the same transaction as the workspace membership. A Keycloak sign-in that links an existing account creates no membership, so it adds no team. Keycloak provisioning targets the original (oldest) workspace, so only that workspace's default applies to it.
- **OQ-6: What if the default team is deleted?** *Recommended: the setting clears itself, and newcomers land on no team (today's copy).* A newcomer is never refused (D17). There is no team delete today, so this guards a future feature and hand-written SQL. A default that points at another workspace's team must be impossible.
- **OQ-7: Can an invite pick a specific team instead?** *Recommended: not in this feature.* Invites use the workspace default. A per-invite team changes the invite record, the invite form and the accept path. That is a follow-up if newcomers often need a non-default team (KPI-2 shows it).
- **OQ-8: Does setting a default backfill existing no-team members?** *Recommended: no.* Those members are added on the members page. A backfill would grant board access to people nobody chose.
- **OQ-9: Workspace removal and teams.** `POST /workspace/members/remove` leaves the person's team rows, including a `lead` row. That can strip a team of its only real lead, outside D6. *Recommended:* removing someone from a workspace also removes their team memberships in it. It is refused, with D15-style copy naming the team ("{name} ({email}) is the last lead of {team}. Make someone else a lead first."), when they are any team's last lead. If accepted, this goes in slice 03 as US-TM-04 AC 6. If declined, members pages list only current workspace members, and DESIGN defines "lead" for INV-1 as "a lead who is still a workspace member".
- **OQ-10: Keep the default team in this feature, or split it out?** *Recommended: keep.* You asked for it, slices 05-06 depend only on slice 01, and they ship separately anyway. Splitting means renaming them into a new feature id and changes nothing else.

**Resolved by the user (2026-10-09):** every OQ takes its recommended answer.
- OQ-1: instance admins are not covered implicitly; they act as workspace admins (orchestrator, on the recommendation).
- OQ-2: leads are peers. A lead may demote or remove any lead on their team, including themselves, unless that leaves the team leaderless.
- OQ-3 to OQ-8: accepted as written (one default per workspace on `/teams`, unset on upgrade; invite and Keycloak provisioning apply it equally as Member; a deleted default clears itself; no per-invite team; no backfill).
- OQ-9: accepted. Workspace removal also removes the person's team rows and is refused when they are any team's last lead; it lands in slice 03 (US-TM-04 AC 6).
- OQ-10: keep the default team in this feature.

So slices 05-06 are unblocked, and DoR item 8 passes for US-TM-06 and US-TM-07.

### [REF] Journey (lightweight: happy path plus main refusals)

Emotional arc, **from locked out to in charge**: stuck and faintly absurd ("ask a
workspace admin", and she is one) → oriented (the Teams page shows General and
who is on it) → in control (one Add, a sentence naming exactly who was added) →
relieved (the SSO account opens Sandbox).

```text
[Where]                          [Does]                                       [Sees]                                                         [Feels]
/ (SSO account)                  signs in with Keycloak                       "You're not on any team yet — ask a workspace admin..."        stuck
/ (bootstrap account)            clicks Teams                                 /teams: "General · 1 member"                                   oriented
/team/general/members            reads the list                               "Jeff Bailey · jeffabailey@gmail.com · Lead"                   oriented
                                 picks "Jeff Bailey — jeff@unintelligent-     "Added Jeff Bailey (jeff@unintelligent-design.us) to           in control
                                 design.us" in Add a member, clicks Add       General as a member." + a new "Member" row
/ (SSO account)                  reloads                                      Your projects: SBX Sandbox                                     relieved
/team/general/project/sandbox    opens it                                     the Sandbox board (200)                                        relieved
```

Main refusals and edge paths:

| Step | What happens | Sees |
|---|---|---|
| Marco opens `/team/general/members` (not admin, not on General) | refused | the uniform 404 (D5) |
| Lena (Backend lead) posts an add to `/team/general/members` | refused | the uniform 404 (D2, D5) |
| Forged `user_id` of someone outside Canzan Labs | refused | the uniform 404 (D5, D7) |
| Everyone is already on the team | no candidates | "Everyone in this workspace is already on General." |
| Stale page in a second tab re-adds the same person | no change | "Jeff Bailey (jeff@unintelligent-design.us) is already on General." |
| Demote or remove the only lead | refused, nothing changes | "General needs at least one lead. Make someone else a lead first." |

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| `${team_slug}`, `${team}` (name) | `teams.slug`, `teams.name`, resolved by (acting workspace, slug) | URLs, headings, D15 copy | LOW: shipped |
| `${name}`, `${email}` | `users.display_name`, `users.email_display` | rows, Add list, D15 copy | MEDIUM: both are needed (D8) |
| Role label ("Lead" / "Member") | `team_memberships.role` | rows, role control, D15 copy | LOW |
| Add candidates | members of the acting workspace minus the team's members | Add list | MEDIUM: must be computed in the acting workspace only (D10) |
| Acting workspace | session (`SessionUser.workspace_id`) | every new route | HIGH: the tenancy boundary |
| Default team (slice 05) | one per-workspace setting (DESIGN picks the shape) | `/teams`, invite accept, Keycloak provisioning | HIGH: must belong to the same workspace (OQ-6) |
| Uniform 404 | `resource_not_found_page` | every refusal (D5) | HIGH: byte-identity is the non-enumeration guarantee |
| No-team copy | `dashboard_root.html` | dashboard, `/teams` for a member on no team | LOW: unchanged string |

### [REF] Story Map and Slices

Backbone: **Find the team → See who is on it → Change who is on it → Change who leads it → Default newcomers**

| Slice | Stories | What it adds | Est. | Brief |
|---|---|---|---|---|
| 01 (walking skeleton) | US-TM-01, US-TM-02 | `/teams` index, members page, workspace-admin Add. **Resolves the incident.** | 1 d | `slices/slice-01-members-page-and-admin-add.md` |
| 02 | US-TM-03 | Workspace admin promotes and demotes. Last-lead demote is refused under concurrency. | 1 d | `slices/slice-02-change-role-never-leaderless.md` |
| 03 | US-TM-04 | Workspace admin removes someone. Last-lead removal is refused (and workspace removal, if OQ-9). | 0.5-1 d | `slices/slice-03-remove-from-team.md` |
| 04 | US-TM-05 | A team lead does add, role change and remove on their own team only. | 0.5-1 d | `slices/slice-04-team-lead-manages-own-team.md` |
| 05 | US-TM-06 | Workspace admin picks a default team, and an invitee who accepts lands on it. | 1 d | `slices/slice-05-default-team-for-invites.md` |
| 06 | US-TM-07 | A Keycloak-provisioned newcomer lands on the default team. | 0.5 d | `slices/slice-06-default-team-for-keycloak.md` |

The walking skeleton (slice 01) covers the first three backbone activities.
"Change who leads it" and "Default newcomers" are not needed for the incident's
end-to-end flow, so they stay below the line.

Priority by learning leverage:

1. **01** first. It is the incident, it is dogfooded the same day (D4), and it settles the new authz surface (a workspace admin acting on a team they are not on) that every later slice reuses.
2. **02** next. It carries the riskiest assumption: that "never leaderless" can be guaranteed under concurrent demotes (INV-1 to INV-3). Learning that early costs least, and 03 and 04 reuse the mechanism.
3. **03** reuses 02's invariant for a second verb. It is the natural place for OQ-9.
4. **04** widens the actor set from workspace admin to team lead for three verbs that already ship. That is low uncertainty, but it is what makes the `lead` role mean something.
5. **05 and 06** come last. They are blocked on OQ-3 to OQ-8 and depend only on 01, so they can move earlier, or split out (OQ-10), the moment those questions are answered. 05 comes before 06 because it adds the setting, and 06 only reads it.

Taste tests: no slice ships 4 or more new components. No slice is infrastructure
only: the invariant rides in slice 02's user value. Every slice names what it
disproves (see the briefs). Every dogfood uses the dev instance's real accounts.
No two slices differ only in scale (invite and Keycloak are different doors with
different transactions).

### [REF] User Stories

#### US-TM-01: A workspace admin sees every team and who is on it, with their roles

`job_id: job-team-membership` | slice 01

##### Elevator Pitch

Before: a workspace admin has no page that says which teams exist or who is on them. The only answer is psql.
After: on `/`, click **Teams** → `/teams` shows "General · 1 member". Click General → `/team/general/members` shows "Jeff Bailey · jeffabailey@gmail.com · Lead".
Decision enabled: who is missing from the team and needs adding.

##### Problem

Priya's SSO account is told to "ask a workspace admin", and she is the workspace
admin, but she has no page that shows teams or their members.

##### Who

- Workspace admin | dashboard, desktop browser | wants to see who can open a team's boards.
- Team member (lead or member) | wants to see who else is on their team.

##### Domain Examples

1. **Admin, dogfood**: `/teams` on dev lists "General · 1 member". `/team/general/members` lists "Jeff Bailey · jeffabailey@gmail.com · Lead".
2. **Admin not on the team**: Priya (admin of Canzan Labs, on Backend only) opens `/team/ops/members` and sees Ops's rows, but still gets the 404 on Ops's boards (D9).
3. **Plain member**: Lena's teammate Sam Okafor (Backend, Member) sees `/teams` listing only "Backend · 3 members", and the Backend list with no Add, role or Remove controls.
4. **Refused**: Marco (Canzan Labs member, not on Backend) gets the uniform 404 from `/team/backend/members`. So does `/team/no-such-team/members` and a team slug from another workspace.
5. **No team**: the SSO account on no team opens `/teams` and sees "You're not on any team yet — ask a workspace admin to add you."

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin finds a team and sees its members and roles

- Given Priya is workspace admin of Canzan Labs, where team General has lead "Jeff Bailey" (jeffabailey@gmail.com)
- When Priya follows Teams from the dashboard and opens General
- Then she sees "General · 1 member" on the teams page
- And the members page shows "Jeff Bailey · jeffabailey@gmail.com · Lead"

###### Scenario: A workspace admin sees the members of a team she is not on, but still cannot open its boards

- Given Priya is workspace admin and is not on team Ops
- When she opens Ops's members page, then an Ops board
- Then she sees Ops's members, and the board answers the uniform not-found page

###### Scenario: A team member sees their own team's members without management controls

- Given Sam Okafor is a Member of Backend and not a workspace admin
- When Sam opens Teams, then Backend's members page
- Then the teams page lists only Backend, and the members page shows no Add, role or Remove control

###### Scenario: People outside a team cannot learn that it exists

- Given Marco is a Canzan Labs member who is not on Backend
- When Marco opens Backend's members page, an unknown team's members page, and another workspace's team's members page
- Then all three answer the same uniform not-found page

##### Acceptance Criteria

- [ ] The dashboard shows a "Teams" quick action. `/teams` lists every team of the acting workspace for a workspace admin, and only the caller's teams otherwise, each with its member count (scenarios 1, 3; D12).
- [ ] The members page lists each member as display name · email · role, with leads first and then by name (scenario 1; D8).
- [ ] A workspace admin reaches any team's members page in the workspace. That gives no board access (scenario 2; D9, D11).
- [ ] A plain member sees the list read-only. No management control is rendered (scenario 3; D11).
- [ ] Signed out, not on the team and not admin, unknown slug, or another workspace's team: the answer is byte-identical to the uniform 404 (scenario 4; D5).

##### Outcome KPIs

KPI-4.

##### Technical Notes

- Reads only. No schema change. The team is resolved by (session workspace, slug) (D10).

##### Size

0.5 day | 4 scenarios | slice 01

#### US-TM-02: A workspace admin puts a workspace member on a team, and that member can open the team's boards

`job_id: job-team-membership` | slice 01

##### Elevator Pitch

Before: Jeff's SSO account `jeff@unintelligent-design.us` sees "You're not on any team yet — ask a workspace admin to add you.", and `/team/general/project/sandbox` answers 404. No admin can fix it in the product.
After: as workspace admin on `/team/general/members`, choose "Jeff Bailey — jeff@unintelligent-design.us" under **Add a member** and click **Add** → see "Added Jeff Bailey (jeff@unintelligent-design.us) to General as a member." Signed in with Keycloak as that account, `/team/general/project/sandbox` shows the Sandbox board.
Decision enabled: the person can now work on the team's boards, so nobody needs psql or `foundry doctor add-test-user`.

##### Problem

Invite-accept and Keycloak provisioning add only a workspace membership. The
only tools that put someone on a team are psql and a test-user command that puts
them on every team.

##### Who

- Workspace admin | members page | puts one person on one team.

##### Domain Examples

1. **Dogfood (D4)**: on dev, `jeffabailey@gmail.com` adds `jeff@unintelligent-design.us` to General. That account's dashboard then lists "SBX Sandbox", and the board opens.
2. **Same display name**: the Add list shows "Jeff Bailey — jeff@unintelligent-design.us". The bootstrap "Jeff Bailey — jeffabailey@gmail.com" is not offered, because it is already on General (D8).
3. **Nobody to add**: on Backend, where all 4 Canzan Labs members already are, the Add list is replaced by "Everyone in this workspace is already on Backend."
4. **Stale tab**: Priya adds Tomás to Backend in one tab, then clicks Add for Tomás again in an older tab → "Tomás Rivera (tomas.rivera@canzan.dev) is already on Backend." One row.
5. **Forged**: a POST naming a Globex user's id (not in Canzan Labs) gets the uniform 404, and no row is written (D5, D7).

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin adds a member to a team, and the member can open the team's board

- Given "jeff@unintelligent-design.us" is a Canzan Labs member on no team, and General has project Sandbox
- When the workspace admin adds "Jeff Bailey — jeff@unintelligent-design.us" to General
- Then she sees "Added Jeff Bailey (jeff@unintelligent-design.us) to General as a member."
- And the members page shows that account as Member
- And signed in as that account, the dashboard lists Sandbox and its board opens

###### Scenario: Only workspace members who are not on the team are offered

- Given General has "jeffabailey@gmail.com" as lead, and the workspace also has "jeff@unintelligent-design.us"
- When the workspace admin opens General's members page
- Then Add a member offers "Jeff Bailey — jeff@unintelligent-design.us" and nobody already on General

###### Scenario: Adding someone already on the team changes nothing and says so

- Given Tomás Rivera is already on Backend
- When the workspace admin submits Tomás again from an older page
- Then she sees "Tomás Rivera (tomas.rivera@canzan.dev) is already on Backend." and Backend still has one row for him

###### Scenario: Someone outside the workspace cannot be added

- When the workspace admin submits an add to Backend naming a user who is not a Canzan Labs member
- Then the answer is the uniform not-found page and Backend's members are unchanged

###### Scenario: A non-admin cannot add anyone to a team they do not lead

- Given Marco is a Canzan Labs member who is not a workspace admin and does not lead General
- When Marco submits an add to General
- Then the answer is the uniform not-found page and nothing changes

##### Acceptance Criteria

- [ ] A workspace admin's Add writes one `member` row and shows the D15 copy. The added person's dashboard lists the team's projects, and its board answers 200 (scenario 1; D4).
- [ ] The Add list is exactly (acting-workspace members) minus (team members), each shown as name — email. When it is empty, the D15 copy replaces it (scenario 2; D7, D8).
- [ ] A re-add is idempotent and shows the "already on" copy (scenario 3; D13).
- [ ] A target outside the acting workspace, an unknown id, or a caller who is neither admin nor this team's lead gets the uniform 404, with zero rows written (scenarios 4, 5; D5).
- [ ] Without scripting, Add is a plain form POST followed by a re-rendered page.

##### Outcome KPIs

KPI-1, KPI-4, KPI-5.

##### Technical Notes

- No schema change. CSRF on the shared layer. Authz is evaluated at request time, never from the rendered page (INV-6).
- INV-5: the add commits only if the target is still a workspace member.

##### Size

0.5 day | 5 scenarios | slice 01

#### US-TM-03: A workspace admin changes who leads a team, and a team can never lose its last lead

`job_id: job-team-membership` | slice 02

##### Elevator Pitch

Before: the `lead` role exists only in the database. Nothing in the product shows it, sets it or protects it.
After: on `/team/backend/members`, click **Make lead** on "Sam Okafor · sam.okafor@canzan.dev · Member" → see "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend." Later, **Make member** on Sam, by then the only lead, shows "Backend needs at least one lead. Make someone else a lead first."
Decision enabled: hand a team's leadership to someone else, safely, in two clicks.

##### Problem

When a team's leadership changes, the only path is psql, and nothing stops a
team from ending up with no lead.

##### Who

- Workspace admin | members page | reassigns leadership.

##### Domain Examples

1. **Promote**: Sam (Member) → "Make lead" → "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend." Backend now has Lena and Sam as leads.
2. **Hand over**: Lena is then demoted → "Lena Park (lena.park@canzan.dev) is now a member of Backend." Sam is the only lead.
3. **Last lead**: demoting Sam, now the only lead → "Backend needs at least one lead. Make someone else a lead first." Nothing changes.
4. **Concurrent**: Lena and Sam are the only two leads, and two requests demote each of them at the same instant. Exactly one succeeds. The other gets the last-lead copy. Backend ends with exactly one lead.
5. **No-op**: "Make lead" on someone who already is a lead → "… is now a lead of Backend." with no write (D13).

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin promotes a member to lead

- Given Backend has lead Lena Park and member Sam Okafor
- When the workspace admin makes Sam a lead
- Then she sees "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend." and both are listed as Lead

###### Scenario: A workspace admin hands leadership over by demoting the previous lead

- Given Backend has leads Lena Park and Sam Okafor
- When the workspace admin makes Lena a member
- Then Lena is listed as Member and Sam as Lead

###### Scenario: The last lead cannot be demoted

- Given Sam Okafor is Backend's only lead
- When the workspace admin makes Sam a member
- Then she sees "Backend needs at least one lead. Make someone else a lead first." and Sam is still Lead

###### Scenario: Two leads demoted at the same time leave the team with one lead

- Given Lena Park and Sam Okafor are Backend's only leads
- When both are demoted at the same moment
- Then exactly one demotion succeeds, the other shows the last-lead message, and Backend has exactly one lead

###### Scenario: Someone without the right cannot change a role

- When Marco submits a role change on Backend
- Then the answer is the uniform not-found page and no role changes

##### Acceptance Criteria

- [ ] Promote and demote write the role and show the D15 copy. A same-role submit writes nothing (scenarios 1, 2; D13).
- [ ] A demote that would leave a team that has a lead with none is refused with the D15 copy and writes nothing (scenario 3; D6).
- [ ] Under concurrent demotes of the last two leads, the team ends with exactly one lead, over 100 repeated races (scenario 4; INV-1, INV-2).
- [ ] An unauthorized caller gets the uniform 404 (scenario 5; D5).

##### Outcome KPIs

KPI-3, KPI-4.

##### Technical Notes

- INV-1 to INV-4 are requirements for DESIGN. The mechanism is DESIGN's choice. The race scenario must run against real Postgres.

##### Size

1 day | 5 scenarios | slice 02

#### US-TM-04: A workspace admin takes someone off a team, without leaving it leaderless

`job_id: job-team-membership` | slice 03

##### Elevator Pitch

Before: someone who left a team's work keeps its board access forever, unless psql removes it.
After: on `/team/backend/members`, click **Remove** on "Tomás Rivera · tomas.rivera@canzan.dev · Member" → see "Removed Tomás Rivera (tomas.rivera@canzan.dev) from Backend." Tomás's next request to `/team/backend/project/auth` answers the uniform 404.
Decision enabled: board access matches who actually works on the team.

##### Problem

Removal is the other half of add. Without it, every add is permanent, and it
is the second way, after a demote, that a team can lose its last lead.

##### Who

- Workspace admin | members page | removes access.

##### Domain Examples

1. **Remove member**: Tomás is removed from Backend. His dashboard drops "AUTH Identity Platform", and the AUTH board answers 404.
2. **Remove one of two leads**: Lena is removed. Sam remains the only lead.
3. **Last lead**: removing Sam, the only lead → "Backend needs at least one lead. Make someone else a lead first."
4. **Concurrent**: removing Lena and demoting Sam at the same instant, when they are the only leads → exactly one succeeds.
5. **Last team**: removing Tomás from his only team → his dashboard shows "You're not on any team yet — ask a workspace admin to add you."

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin removes a member, who loses access to the team's boards

- Given Tomás Rivera is a member of Backend, which has project Identity Platform (AUTH)
- When the workspace admin removes Tomás from Backend
- Then she sees "Removed Tomás Rivera (tomas.rivera@canzan.dev) from Backend."
- And Tomás's dashboard no longer lists AUTH, and the AUTH board answers the uniform not-found page

###### Scenario: The last lead cannot be removed

- Given Sam Okafor is Backend's only lead
- When the workspace admin removes Sam
- Then she sees "Backend needs at least one lead. Make someone else a lead first." and Sam is still on Backend as Lead

###### Scenario: A removal and a demotion at the same time leave the team with a lead

- Given Lena Park and Sam Okafor are Backend's only leads
- When Lena is removed and Sam is demoted at the same moment
- Then exactly one of the two succeeds, and Backend has exactly one lead

###### Scenario: Someone without the right cannot remove anyone

- When Marco submits a removal from Backend
- Then the answer is the uniform not-found page and Backend is unchanged

##### Acceptance Criteria

- [ ] Remove deletes the row and shows the D15 copy. The removed person's next board request is the uniform 404 (scenario 1).
- [ ] Removing the last lead is refused with the D15 copy, and nothing changes (scenario 2; D6).
- [ ] A concurrent removal and demotion of the last two leads ends with exactly one lead (scenario 3; INV-1, INV-3).
- [ ] An unauthorized caller gets the uniform 404 (scenario 4; D5).
- [ ] Removing someone not on the team (stale page) changes nothing and re-renders the list (D13).
- [ ] (OQ-9 accepted) `POST /workspace/members/remove` also removes the person's team rows in that workspace, and is refused with the OQ-9 copy when they are any team's last lead.

##### Outcome KPIs

KPI-3, KPI-4.

##### Technical Notes

- Reuses slice 02's invariant mechanism. OQ-9 widens the scope to the shipped workspace-removal route.

##### Size

0.5-1 day | 4 scenarios | slice 03

#### US-TM-05: A team lead manages their own team, and only their own team

`job_id: job-team-membership` | slice 04

##### Elevator Pitch

Before: Lena leads Backend in name only. Adding a teammate means asking Priya, the workspace admin.
After: as Lena on `/team/backend/members`, choose "Tomás Rivera — tomas.rivera@canzan.dev" and click **Add** → see "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member." The same request to `/team/general/members` answers the uniform 404.
Decision enabled: Lena staffs her own team without waiting for an admin.

##### Problem

The `lead` role grants nothing today. A lead cannot shape their own team.

##### Who

- Team lead, not a workspace admin | their team's members page.

##### Domain Examples

1. **Add**: Lena adds Tomás to Backend.
2. **Promote and step down**: Lena makes Sam lead, then makes herself a member → "Lena Park (lena.park@canzan.dev) is now a member of Backend." Her controls disappear on the next page.
3. **Other team**: Lena opens `/team/general/members` (she is not on General) → uniform 404. A POST there → uniform 404.
4. **Member of another team, not its lead**: Lena is a Member of Ops. On `/team/ops/members` she sees the list read-only, and a POST there gets the uniform 404.
5. **Demoted mid-session**: after Priya demotes Lena, Lena's open page posts an add → uniform 404 (INV-6).

##### UAT Scenarios (BDD)

###### Scenario: A team lead adds a workspace member to their own team

- Given Lena Park leads Backend and is not a workspace admin
- When Lena adds "Tomás Rivera — tomas.rivera@canzan.dev" to Backend
- Then she sees "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member."

###### Scenario: A team lead hands over leadership and steps down

- Given Lena Park and Sam Okafor are Backend members and Lena is its only lead
- When Lena makes Sam a lead, then makes herself a member
- Then Sam is the only lead and Lena sees Backend's members without management controls

###### Scenario: A team lead cannot manage a team they do not lead

- Given Lena leads Backend, is a Member of Ops, and is not on General
- When Lena submits an add, a role change and a removal on Ops and on General
- Then every request answers the uniform not-found page and nothing changes

###### Scenario: A lead who was demoted cannot act from a page opened earlier

- Given Lena had Backend's members page open as a lead, and has since been made a member
- When Lena submits an add from that page
- Then the answer is the uniform not-found page and nothing changes

##### Acceptance Criteria

- [ ] A lead can add, change roles and remove on their own team, under the same D6 rule and D15 copy as a workspace admin (scenarios 1, 2; D2, OQ-2).
- [ ] A lead acting on any other team gets the uniform 404, whether or not they are a member of it (scenario 3; D5, D11).
- [ ] Authority is checked at request time (scenario 4; INV-6).

##### Outcome KPIs

KPI-4, KPI-5.

##### Technical Notes

- This widens the authz predicate of slices 01-03 from "workspace admin" to "workspace admin or lead of this team". It adds no new route.

##### Size

0.5-1 day | 4 scenarios | slice 04

#### US-TM-06: A workspace admin picks a default team, and an invited newcomer lands on it

`job_id: job-team-membership` | slice 05

##### Elevator Pitch

Before: every invitee's first page says "You're not on any team yet — ask a workspace admin to add you."
After: on `/teams`, set **Default team for new members** to General and click **Save** → see "New members will join General.". Tomás accepts his invite at `/invites/accept` → his first dashboard lists "SBX Sandbox".
Decision enabled: invite people without a follow-up trip to the members page.

##### Problem

Every newcomer needs a second, manual admin action before foundry is useful
to them. The incident shows that it is easily missed.

##### Who

- Workspace admin | teams page | sets policy once.
- Newcomer | accepts an invite | expects to land on a board.

##### Domain Examples

1. **Set**: Priya sets General → "New members will join General."
2. **Invite**: Tomás accepts the invite to Canzan Labs → he is a Member of General, and his dashboard lists Sandbox.
3. **Unset**: with "No team" → "New members join no team." Tomás then lands on today's no-team copy (OQ-4).
4. **Default gone**: if the default team no longer exists, Tomás still gets his account and lands on no team. The accept never fails (D17, OQ-6).
5. **Refused**: Lena (lead, not admin) submits the default-team form → uniform 404. A team id from Globex → uniform 404.

##### UAT Scenarios (BDD)

###### Scenario: A workspace admin sets the default team

- When Priya sets General as Canzan Labs's default team
- Then she sees "New members will join General." and the teams page shows General selected

###### Scenario: An invited newcomer lands on the default team

- Given General is Canzan Labs's default team, with project Sandbox
- When Tomás Rivera accepts his invite to Canzan Labs
- Then Tomás is a Member of General and his first dashboard lists Sandbox

###### Scenario: Without a default team, a newcomer lands as today

- Given Canzan Labs has no default team
- When Tomás accepts his invite
- Then he is on no team and sees "You're not on any team yet — ask a workspace admin to add you."

###### Scenario: A missing default team never blocks joining

- Given Canzan Labs's default team no longer exists
- When Tomás accepts his invite
- Then his account is created, he is signed in, and he is on no team

###### Scenario: Only a workspace admin can set the default, and only to a team of the workspace

- When Lena submits the default-team form, and Priya submits a Globex team as the default
- Then both answer the uniform not-found page and the default is unchanged

##### Acceptance Criteria

- [ ] A workspace admin sets or clears the default from `/teams`, with the D15 copy (scenario 1; OQ-3, OQ-4).
- [ ] Invite accept adds the newcomer to the default team as Member, atomically with the workspace membership (scenario 2; OQ-5).
- [ ] With no default, or a missing default, the behaviour is today's, and accept never fails because of it (scenarios 3, 4; D17, OQ-6).
- [ ] A non-admin, or a team outside the workspace, gets the uniform 404 (scenario 5; D5).
- [ ] Existing no-team members are not backfilled (OQ-8).

##### Outcome KPIs

KPI-1, KPI-2.

##### Technical Notes

- Needs one per-workspace setting (a schema change). DESIGN owns the shape and the "default deleted" behaviour (OQ-6). Rolling deploy: an older replica ignores the setting, so a newcomer there lands on no team, which is today's behaviour.
- OQ-3 to OQ-8 resolved to the recommended answers (2026-10-09); the ACs stand.

##### Size

1 day | 5 scenarios | slice 05

#### US-TM-07: A newcomer provisioned by a Keycloak sign-in lands on the default team

`job_id: job-team-membership` | slice 06

##### Elevator Pitch

Before: a first Keycloak sign-in with the provision role creates a workspace member on no team. This is exactly the incident.
After: with General as default, a first sign-in through **Sign in with Keycloak** as `tomas.rivera@canzan.dev` → the dashboard lists "SBX Sandbox", and `/team/general/project/sandbox` shows the board.
Decision enabled: turn on Keycloak provisioning for the team without a manual follow-up per person.

##### Problem

Keycloak provisioning is the door the incident came through. Slice 05 covers
invites, and this slice covers this door.

##### Who

- Newcomer with the Keycloak provision role | first sign-in.

##### Domain Examples

1. **Provisioned**: Tomás's first Keycloak sign-in creates his account in Canzan Labs (the original workspace) and puts him on General as Member.
2. **Linked, not provisioned**: Priya's existing bootstrap account signs in through Keycloak with the same email → it links as today, and no team row is added (OQ-5).
3. **No default**: provisioned onto no team, as today.
4. **Default gone**: provisioned, signed in, on no team. Never refused (D17).

##### UAT Scenarios (BDD)

###### Scenario: A newcomer's first Keycloak sign-in lands them on the default team

- Given General is the default team of the original workspace, and Keycloak provisioning is on
- When Tomás signs in with Keycloak for the first time
- Then Tomás is a Member of General and the Sandbox board opens

###### Scenario: Linking an existing account adds no team

- Given Priya's existing account is not on Ops, and Ops is the default
- When Priya signs in with Keycloak using the same email
- Then she is signed in and is still not on Ops

###### Scenario: A missing or unset default never blocks a first sign-in

- Given the original workspace has no default team, or its default team no longer exists
- When Tomás signs in with Keycloak for the first time
- Then he is provisioned and signed in, on no team

##### Acceptance Criteria

- [ ] Provisioning adds the default-team Member row atomically with the workspace membership (scenario 1; OQ-5).
- [ ] Linking an existing account writes no team row (scenario 2).
- [ ] An unset or missing default never refuses or fails the sign-in (scenario 3; D17). The keycloak-sso lane (46) stays green.

##### Outcome KPIs

KPI-1, KPI-2.

##### Technical Notes

- Inside `provision_federated_member`'s transaction. The keycloak-sso acceptance lane uses its Keycloak harness.
- OQ-5 and OQ-6 resolved to the recommended answers (2026-10-09).

##### Size

0.5 day | 3 scenarios | slice 06

### [REF] System Constraints

Invariant and concurrency requirements for DESIGN. They name the risks and do not
choose mechanisms.

- **INV-1 (never leaderless):** after any committed write through this feature (demote, remove, and workspace removal if OQ-9 is accepted), every team that had at least one lead still has at least one. A team that already has no lead (legacy, or made in psql) is not refused for unrelated writes, and promoting someone there is allowed.
- **INV-2 (mutual demotion):** two leads demoting each other, or both being demoted by anyone, at the same time must not both succeed when they are the last two. A read-check-then-write without serialization fails this.
- **INV-3 (mixed verbs):** a concurrent remove and demote, or two removes, on the last two leads are under the same guarantee.
- **INV-4 (self-action):** a sole lead demoting or removing themselves is the same refusal. Self-action is not exempt.
- **INV-5 (add vs workspace removal):** an add commits only if the target is still a member of the acting workspace at commit time. Nobody may end up on a team of a workspace they are not in.
- **INV-6 (authority at request time):** authz is evaluated per request against current rows. A rendered control proves nothing (a stale page from a demoted lead gets a 404).
- **INV-7 (default team in the same workspace):** a workspace's default team is always one of its own teams or none, including after a team is deleted by any means.
- **INV-8 (store level):** INV-1 must hold for any future caller of the membership writes, not only these handlers. DESIGN decides whether the database or one service seam enforces it.
- No new oracle: every refusal is byte-identical to the uniform 404 (D5). The last-lead refusal is shown only to an authorized actor.
- No schema change in slices 01-04, unless DESIGN's INV-1 mechanism needs one.

### [REF] Outcome KPIs

Objective: every workspace member who should reach a team's boards can, without
psql or a test-user command, and no team is ever left without a lead.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Workspace members (in workspaces that have at least one team) | Are on at least one team | 0 members on no team on dev and prod, 1 day after slice 01 dogfood (and stays 0 for newcomers once slice 05-06 defaults are set) | dev: at least 1 (`jeff@unintelligent-design.us`). prod: unknown, counted at slice-01 dogfood | Read-only query: workspace members with no `team_memberships` row in that workspace | North star |
| 2 | Newcomers (invite or Keycloak provisioning) when a default is set | Open a board on their first session | 100% | 0%: no path exists | Acceptance scenarios (US-TM-06/07), plus dogfood with one real newcomer on dev | Leading |
| 3 | Teams | Never lose their last lead through the product | 0 teams with no lead made by a feature write; 0 double-success in 100 repeated concurrent races | Leaderless teams today: counted at dogfood (expected 0) | Read-only query `teams` without a `lead` row, plus the race scenarios | Guardrail |
| 4 | Callers without the right | Learn nothing and change nothing | 100% of the (actor × verb × team) authz matrix answers byte-identical 404s with 0 rows written | Not applicable (no routes) | Acceptance authz outline | Guardrail |
| 5 | The operator | Manages membership without psql or `add-test-user` | 0 hand-written `team_memberships` writes from the release onward | Every membership change since bootstrap was psql or `add-test-user` | Operator report at each dogfood | Leading |

At homelab scale, the KPIs are verified by acceptance scenarios and read-only
store queries, not analytics.

### [REF] DoD

1. All UAT scenarios pass: 4 + 5 + 5 + 4 + 4 + 5 + 3 = 30. The neighbouring lanes stay green: fix-hide-unreachable-boards, dashboard-enhancements, navigation-bar, member invites (30 + 18), keycloak-sso (46), us-07, mwt slices, and default.
2. The authz matrix (KPI-4) covers signed-out users, Marco, a lead of another team, a plain member, a workspace admin, an unknown slug and a foreign team, on every new route, both verbs. Every refusal is byte-identical to the uniform 404.
3. INV-1 to INV-6: race scenarios on real Postgres, repeated at least 100 times, with 0 leaderless outcomes.
4. Nothing is written on a refusal (row counts asserted).
5. The pages work with scripting disabled (the scripting-disabled lane).
6. `cargo xtask check-arch` and `cargo xtask smoke` pass before each commit. `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` passes before push. Mutation kill rate is at least 80% on modified files.
7. **Dogfood (D4)**, on dev (it auto-deploys release tags), the same day: add `jeff@unintelligent-design.us` to General as `jeffabailey@gmail.com`. Then the Keycloak session's `/team/general/project/sandbox` answers 200, and the dashboard lists Sandbox. Record the KPI-1 and KPI-3 baselines with read-only queries on dev and prod. Prod is verified after approval.
8. Upgrade: slices 01-04 need no migration. Slice 05's migration is additive, rolling-deploy safe (an older replica ignores the default), and its CHANGELOG migration notes say so.
9. Documentation: a CHANGELOG `[Unreleased]` entry for Teams, the members page and the default team. `brief.md` gains a "Teams and team authz" paragraph (DESIGN). An outcomes registry row is added at DISTILL. `jobs.yaml` and the persona were done here. The fix-hide-unreachable-boards evolution doc's first follow-up is marked closed in this feature's evolution doc.

### [REF] Out of Scope

- Team create, rename, delete, and team name rules (D14). Workspaces provisioned with zero teams stay without teams.
- Self-service join, leave or request-to-join. Inviting directly onto a specific team (OQ-7).
- Linking a Keycloak identity to an existing account, and showing the signed-in account (the incident's second follow-up).
- Instance-admin team management (OQ-1). An `/api/v1` or `foundry doctor` membership command (`add-test-user` is unchanged).
- Workspace role changes (admin ⇄ member).
- Notifications or email to the person added or removed. A membership audit trail.
- Backfilling existing no-team members onto a default (OQ-8).
- Granting board access through workspace admin (D9).
- Changing the dashboard's no-team copy, or its hardcoded `/team/general/projects/new` link.

### [REF] WS Strategy

Brownfield (Decision 2, judged against the structure). Sessions, tenancy, teams,
the board gate and the uniform 404 already ship. What is missing is any path into
`team_memberships`, so slice 01 is the walking skeleton. It is the thinnest
end-to-end path: dashboard → `/teams` → members page → Add → the member opens the
board. It runs on real adapters only (real Postgres, real router, real session),
with no configurable or environment-switched seam (not strategy D). Slices 02-06
extend it.

### [REF] Driving Ports

1. `GET /teams` (new): the teams index (D12). In slice 05, it also holds the default-team form.
2. `GET /team/{team_slug}/members` (new): the members page (D11).
3. `POST /team/{team_slug}/members` (new): add (US-TM-02, US-TM-05).
4. `POST /team/{team_slug}/members/{user_id}/role` (new): promote and demote (US-TM-03).
5. `POST /team/{team_slug}/members/{user_id}/remove` (new): remove (US-TM-04).
6. `POST /teams/default` (new, slice 05): set or clear the default team.
7. `GET /` (delta): a "Teams" quick action.
8. `POST /invites/accept` (delta, slice 05) and the Keycloak callback `oidc::CALLBACK_PATH` (delta, slice 06): default-team placement.
9. `POST /workspace/members/remove` (delta, only if OQ-9 is accepted).
10. `GET /team/{team_slug}/project/{project_slug}` (unchanged route): now reachable for added members. This is the dogfood check.

Routes are proposals (D16). The copy is locked (D15).

### [REF] Pre-requisites

- v0.12.0 (fix-hide-unreachable-boards) has shipped: the membership-filtered project list, `is_on_any_team`, and the no-team copy.
- These already ship: `is_workspace_admin`, `is_team_member`, `find_team_by_slug`, `resource_not_found_page`, CSRF and sessions, and the scripting-disabled lane.
- Dev has `FOUNDRY_OIDC_PROVISION_ROLE` set, and the SSO account exists there as a workspace member on no team (the incident state, which the D4 dogfood needs).
- OQ-1 and OQ-2 are answered before slice 01 and slice 02 DESIGN respectively (the stories assume the recommended answers). OQ-9 must be answered before slice 03, and OQ-3 to OQ-8 before slice 05.

### [REF] DoR Validation

| DoR item | 01 | 02 | 03 | 04 | 05 | 06 | 07 | Evidence |
|---|---|---|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | PASS | PASS | PASS | Each Problem section: "ask a workspace admin" with no admin path; permanent adds; lead means nothing; newcomers dead-end |
| 2. Persona specific | PASS | PASS | PASS | PASS | PASS | PASS | PASS | Priya (two accounts), Lena (lead, not admin), Sam, Tomás, Marco |
| 3. 3+ domain examples, real data | PASS (5) | PASS (5) | PASS (5) | PASS (5) | PASS (5) | PASS (5) | PASS (4) | dev accounts `jeffabailey@gmail.com` and `jeff@unintelligent-design.us`, General/Sandbox, Canzan Labs Backend/AUTH |
| 4. UAT 3-7 scenarios | PASS (4) | PASS (5) | PASS (5) | PASS (4) | PASS (4) | PASS (5) | PASS (3) | Business-outcome titles |
| 5. AC from UAT | PASS | PASS | PASS | PASS | PASS | PASS | PASS | Each AC cites its scenarios or decisions |
| 6. Right-sized | PASS | PASS | PASS | PASS | PASS | PASS | PASS | 0.5-1 day each, 3-5 scenarios |
| 7. Technical notes | PASS | PASS | PASS | PASS | PASS | PASS | PASS | Per story, plus INV-1 to INV-8 |
| 8. Dependencies tracked | PASS | PASS | PASS (OQ-2) | PASS (OQ-2, OQ-9) | PASS (OQ-2) | **PASS with a block** (OQ-3 to OQ-8) | **PASS with a block** (OQ-5, OQ-6) | All open questions listed with recommended answers |
| 9. Outcome KPIs | PASS | PASS | PASS | PASS | PASS | PASS | PASS | KPI-1 to KPI-5 with targets, baselines and methods |
| job_id + Elevator Pitch | PASS | PASS | PASS | PASS | PASS | PASS | PASS | `job-team-membership`. Every After line names a real route or UI action and the exact text seen. |

DoR status: **PASSED for slices 01-04**, which are ready for DESIGN on the
recommended answers to OQ-1, OQ-2 and OQ-9. **Slices 05-06 pass with a block**
until OQ-3 to OQ-8 are answered. Per-wave peer review was not invoked. The
consolidated review runs at the end of DISTILL.

### [REF] Wave Decisions

- Feature type: user-facing. Walking skeleton: slice 01, brownfield. UX depth: lightweight. JTBD: a new job, `job-team-membership`.
- Density: lean, `ask-intelligent`. Triggers evaluated:
  - **Cross-context complexity: fired.** Three bounded contexts (team membership and authz, invite onboarding, federated provisioning). Suggests `alternatives-considered`.
  - **Multi-stakeholder need: fired.** Five personas across the stories (Priya, Lena, Sam, Tomás, Marco). Suggests `persona-narrative`.
  - **AC ambiguity: not fired.** The last-lead AC shared by US-TM-03, -04 and -05 has one locked copy string and one invariant. The default-team ACs are written to the recommended answers and blocked on the OQs, rather than left ambiguous.
  - **Compliance / regulatory: not fired.** No regulatory terms appear, and the audit trail is explicitly out of scope.
  - **WS strategy = D: not fired.**
  - Neither expansion is rendered. Both are offered at handoff.
- Upstream changes: none to DISCOVER (none exists). This closes the first follow-up of `docs/evolution/2026-10-09-fix-hide-unreachable-boards.md`.
