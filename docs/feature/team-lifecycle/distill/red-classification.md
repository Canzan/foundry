# RED classification — team-lifecycle

Gate output of the pre-DELIVER *fail-for-the-right-reason* check (ADR-025
scaffolded RED, rigor profile `adr-025-scaffolded-red`). DELIVER reads this file
at the start of each step to confirm the RED it un-pends is genuine.

## Acceptance (`@team-lifecycle`, four feature files)

**Run** (2026-10-11): `@pending` stripped from the four
`team-lifecycle-0*.feature` files only (a `sed` over copies kept in the
scratchpad), `foundry` rebuilt and warmed, then
`FOUNDRY_ACCEPTANCE_TAGS=team-lifecycle cargo test -p foundry-acceptance --test acceptance`.
The files were then restored byte for byte from the copies (`cmp` clean; 53 of 53
scenario `@pending` tags back: `grep -cE '^\s*@.*@pending'` per file = 11, 18, 12,
12).

**Result**: 4 features, **95 scenarios** (53 scenario declarations, outline rows
expanded), 536 steps; **92 RED (MISSING_FUNCTIONALITY), 3 GREEN_ALREADY guards,
0 BROKEN**; 0 parsing errors, 0 hook errors, 0 ambiguous or unmatched steps.
444 steps passed: every Background (the roster, Globex, the instance admin),
every Homelab seed and workspace switch, every universe capture, every
byte-identical refusal that the shipped router already gives, every CSRF 403 and
every operator statement ran.

**Verdict: PASS.** Zero BROKEN, zero false GREEN.

### Why no refusal passes for the wrong reason

None of the new routes exists. `POST /teams` answers 405 today (the `/teams`
route has only a GET), and `GET`/`POST /team/{slug}/delete` and
`POST /team/{slug}/rename` fall through to the router's uniform 404, so several
refusal scenarios already get the byte-identical 404 they assert. Each refusal
scenario therefore also asserts a control that proves the hidden door or team
exists: "Priya's teams page still offers creating a team", "Priya still finds
Bakcend's delete page", "Hugo still finds Payments's delete page", "Priya still
finds Backend's rename form", "the same create / delete / rename with the page's
token goes through". The controls are what is RED today; they turn green only
when the route serves the authorized caller, at which point the 404 means
something. This is team-membership's idiom (no 501 stubs at DISTILL).

### By failure point

| Where it stops | Rows | Classification |
|---|---|---|
| `/teams` offers no create form (`When … creates the team … from the teams page`, `Given … has created the team …`) | 19 | MISSING_FUNCTIONALITY |
| `POST /teams` answers 405, not the uniform 404 (`Then the answer is the page that never existed`) | 6 | MISSING_FUNCTIONALITY |
| `/teams` lacks the no-teams-yet copy for Homelab's admin (walking skeleton) | 1 | MISSING_FUNCTIONALITY |
| The members page has no **Delete team** link (`… follows Delete team …`, `Given … opened …'s delete page earlier`, `Given … has deleted …`) | 12 | MISSING_FUNCTIONALITY |
| A refusal's control: the hidden team's delete page answers 404 | 13 | MISSING_FUNCTIONALITY |
| The members page has no Rename form (`When … renames …`, `Given … has renamed …`, `submits … rename form unchanged`) | 11 | MISSING_FUNCTIONALITY |
| A refusal's control: the hidden team's Rename form is absent | 7 | MISSING_FUNCTIONALITY |
| The members page offers no **New project** / **Rename** (control outlines) | 5 | MISSING_FUNCTIONALITY |
| The dashboard's first-project link goes to `/team/general/projects/new` (404 for Tomás, D14) | 1 | MISSING_FUNCTIONALITY |
| A control write with the token: `POST /teams` 405, `/team/bakcend/delete` 404, `/team/backend/rename` 404 | 3 | MISSING_FUNCTIONALITY |
| The control "Priya's teams page still offers creating a team" | 1 | MISSING_FUNCTIONALITY |
| A delete of a team with a project answers 404 instead of the 409 refusal | 1 | MISSING_FUNCTIONALITY |
| HTTP races, run 1: the delete answers 404; both creates answer 405; no team created | 4 | MISSING_FUNCTIONALITY |
| The database stores a hand-typed bad team name (no 0024 triggers yet) | 7 | MISSING_FUNCTIONALITY |
| Passes | 3 | GREEN_ALREADY (guards, below) |

### Per scenario

| # | File | Scenario | Rows | RED |
|---|---|---|---|---|
| 1 | 01 | **WS** Priya starts Homelab's first team, leads it, and opens its first board | 1 | MF: `/teams` in Homelab has no "This workspace has no teams yet…" copy (Homelab seeded, the switch went through, Teams followed from the dashboard) |
| 2 | 01 | A workspace admin adds a team next to the others and leads it, and only its people see it | 1 | MF: no create form |
| 3 | 01 | A new team's lead is sent from the dashboard to that team's project form | 1 | MF: no create form (Given) |
| 4 | 01 | A member of a team with no projects is sent to that team's project form | 1 | MF: the link opens `/team/general/projects/new`, 404 for Tomás |
| 5 | 01 | Everyone on a team is offered New project, and an admin who is not on it is not | 4 | MF ×3: no **New project** link; GREEN_ALREADY ×1 (Jeff, an admin not on Backend, is offered none) |
| 6 | 01 | A name already used in the workspace is refused, and the text is kept | 2 | MF: no create form |
| 7 | 01 | An empty name is refused | 1 | MF: no create form |
| 8 | 01 | Sending the create form a second time makes no second team | 1 | MF: no create form (Given) |
| 9 | 01 | Only a workspace admin can create a team | 6 | MF: `POST /teams` 405, not the uniform 404 |
| 10 | 01 | An instance admin who is not a workspace admin is not offered the form either | 1 | MF: control (Ines sees no form already) |
| 11 | 01 | A create sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |
| 12 | 02 | The delete page says exactly what goes with the team, and changes nothing | 1 | MF: no **Delete team** link |
| 13 | 02 | A workspace admin deletes a team that has no projects | 1 | MF: no **Delete team** link (Given) |
| 14 | 02 | A member whose only team was deleted lands on the no-team page… | 1 | MF: no **Delete team** link (Given) |
| 15 | 02 | Deleting the default team warns first and leaves no default | 1 | MF: no **Delete team** link (the default was set through the shipped form) |
| 16 | 02 | A team's machine tokens are named on the delete page and go with the team | 1 | MF: no **Delete team** link (the scoped token was seeded) |
| 17 | 02 | Cancel goes back to the team, and nothing changes | 1 | MF: no **Delete team** link |
| 18 | 02 | After a team is deleted its name and address can be used again… | 1 | MF: no **Delete team** link (Given) |
| 19 | 02 | Only a workspace admin may open a team's delete page | 6 | MF: control (refusal already byte-identical) |
| 20 | 02 | Only a workspace admin may delete a team | 6 | MF: control (refusal already byte-identical) |
| 21 | 02 | Another workspace's team and a team that does not exist look the same from Canzan Labs | 1 | MF: control (Hugo's Payments delete page 404) |
| 22 | 02 | A delete sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |
| 23 | 02 | A confirm sent again for a team already deleted finds nothing | 1 | MF: no **Delete team** link (Given) |
| 24 | 02 | An add sent from a members page opened before the team was deleted finds nothing | 1 | MF: no **Delete team** link (Given; the members page opened earlier) |
| 25 | 02 | A team with a project shows why it cannot be deleted | 1 | MF: no **Delete team** link |
| 26 | 02 | Every project of the team is named, by name and key | 1 | MF: no **Delete team** link (two projects seeded) |
| 27 | 02 | A delete sent for a team that has a project is refused, and nothing is lost | 1 | MF: 404 instead of the 409 refusal |
| 28 | 02 | A confirm sent from a page opened before a project appeared is refused | 1 | MF: no **Delete team** link (Given) |
| 29 | 02 | A delete racing a project create never destroys the project | 1 | MF: run 1, the delete answers 404 |
| 30 | 03 | A name with no Latin letters gets a working team page | 1 | MF: no create form |
| 31 | 03 | A second name with no Latin letters gets the next free address | 1 | MF: no create form (Given) |
| 32 | 03 | The fallback address is the lowest free one | 1 | MF: no create form (legacy `team`, `team-3` seeded) |
| 33 | 03 | A name that breaks the rule is refused with its reason, and nothing is created | 6 | MF: no create form |
| 34 | 03 | A name of exactly 64 characters is accepted | 2 | MF: no create form |
| 35 | 03 | A name whose address or letters would collide is refused | 3 | MF: no create form |
| 36 | 03 | Two admins creating the same name at once get one team | 1 | MF: run 1, both 405, no team |
| 37 | 03 | Two admins creating the same name with no Latin letters at once get one team | 1 | MF: run 1, both 405, no team |
| 38 | 03 | Two admins creating different names with no Latin letters at once both get a team | 1 | MF: run 1, both 405 |
| 39 | 03 | A team name typed at the database prompt that breaks the rule is refused under the rule's name | 5 | MF: stored (no 0024 trigger) |
| 40 | 03 | A team renamed at the database prompt to a name that breaks the rule is refused | 2 | MF: stored (no 0024 trigger) |
| 41 | 03 | A team name the app would accept is stored at the database prompt exactly as typed | 1 | GREEN_ALREADY (guard: never stricter than `TeamName`) |
| 42 | 04 | A workspace admin renames a team and every link to it keeps working | 1 | MF: no Rename form |
| 43 | 04 | A team's lead renames her own team | 1 | MF: no Rename form |
| 44 | 04 | A change of letter case is a real rename, and a team never collides with itself | 1 | MF: no Rename form |
| 45 | 04 | Submitting the same name changes nothing and records nothing | 1 | MF: no Rename form |
| 46 | 04 | A rename to a name already in use is refused, and the team keeps its name | 2 | MF: no Rename form |
| 47 | 04 | A rename that breaks the rule is refused with the same words as a create | 3 | MF: no Rename form |
| 48 | 04 | A renamed team's old address still holds its old name's place | 1 | MF: no Rename form (Given) |
| 49 | 04 | Rename is offered to the team's lead and to a workspace admin; Delete team only to the admin | 3 | MF ×2: no Rename; GREEN_ALREADY ×1 (Sam, a member, is offered neither) |
| 50 | 04 | Someone without the right cannot rename a team | 6 | MF: control (refusal already byte-identical) |
| 51 | 04 | A rename aimed at another workspace's team or at a team that does not exist is refused the same way | 1 | MF: control |
| 52 | 04 | A lead who has since been made a member cannot rename from a page she opened as a lead | 1 | MF: no Rename form on the page she opened (the two shipped role changes went through) |
| 53 | 04 | A rename sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |

### The three GREEN_ALREADY guards

They assert what must stay true while the feature lands, and must stay green at
every step: an admin who is not on a team is never offered **New project**
(DDD-21, project create would 404 for her); a plain member is offered neither
Rename nor Delete team (ADR-004); the database never refuses a name the app
accepts (64 CJK scalars, ADR-NAME-DB-001's "never stricter").

## Store and use-case gold tests

**Run** (2026-10-11):
`RUST_TEST_THREADS=2 cargo test --no-fail-fast -p foundry-store --test team_create --test team_delete --test team_rename --test team_lifecycle_migrations -- --include-ignored`
and `cargo test -p foundry-services --test team_lifecycle_use_cases -- --include-ignored`.

| File | Tests | GREEN now (not ignored) | RED (ignored) | How each RED fails | BROKEN |
|---|---|---|---|---|---|
| `foundry-store/tests/team_create.rs` | 5 | 0 | 5 (01-01) | scaffold panic `create_team_with_lead` | 0 |
| `foundry-store/tests/team_delete.rs` | 17 | 7: four substrate facts (`FOR UPDATE` blocks a project insert; `FOR NO KEY UPDATE` does not; a read after the lock wait sees the committed project; the four FK names on 23503), the hand cascade (sole lead, tokens, default only), **negative control (a)** and **negative control (b)** | 10 (02-01) | 8 at the 0023 RED gate; 2 at the `TeamGone` assertion (a project create, an add and a default set racing a delete answer an unmapped database error today) | 0 |
| `foundry-store/tests/team_rename.rs` | 6 | 0 | 6 (04-01) | the 0023 RED gate | 0 |
| `foundry-store/tests/team_lifecycle_migrations.rs` | 7 | 1: Postgres refuses U+0000 in `text` with 22021 | 6 (02-01: 0023 ×2, probe; 03-01: 0024 ×3) | the migration-file RED gate | 0 |
| `foundry-services/tests/team_lifecycle_use_cases.rs` | 7 | 0 | 7 (01-01 ×2, 03-01 ×4, 02-01 ×1) | 6 scaffold panic `teams::create_team`; the token mint racing a delete answers `Err(Internal)` (a 500) where INV-TL-2 wants the shipped 422 | 0 |
| **All** | **42** | **8** | **34** | | **0** |

**Both negative controls lose the project on Postgres 16**, exactly as
ADR-TEAM-LIFECYCLE-002 decision 3 argues: (a) a project committed between a
`FOR NO KEY UPDATE` check and the delete is cascaded; (b) a single
`DELETE … WHERE NOT EXISTS (projects)` that waited on the create's `KEY SHARE`
deletes the team and the RI cascade deletes the committed project. They are
GREEN substrate facts, not pending tests: if either ever stops losing the
project, the server's semantics moved and the ADR must be re-argued.

The token mint is a real RED, not a scaffold: today a mint scoped to a team
whose delete commits under it answers `ServiceError::Internal`. DESIGN DDD-9
maps 23503 `machine_tokens_scope_team_id_fkey` to the shipped
`Validation{scope_team_not_in_workspace}`.

## Burn-down

- `grep -cE '^\s*@.*@pending' crates/foundry-acceptance/tests/features/team-lifecycle-0*.feature` = 11 + 18 + 12 + 12 = 53 now, 0 at the end.
- `grep -c '^#\[ignore = "pending: team-lifecycle' crates/foundry-store/tests/team_{create,delete,rename,lifecycle_migrations}.rs crates/foundry-services/tests/team_lifecycle_use_cases.rs` = 5 + 10 + 6 + 6 + 7 = 34 now, 0 at the end.
- `grep -rn "SCAFFOLD: true — RED scaffold written by DISTILL (2026-10-11)" crates/` lists the three team-lifecycle scaffolds (core `team_name.rs`, the store `teams.rs` section, the services `teams.rs` section), 0 at the end. Other features' older `SCAFFOLD: true` markers (board-lane-overflow-menu, notifications) are not this feature's.
