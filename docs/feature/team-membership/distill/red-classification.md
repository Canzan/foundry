# RED classification — team-membership

Gate output of the pre-DELIVER *fail-for-the-right-reason* check (ADR-025
scaffolded RED, rigor profile `adr-025-scaffolded-red`). DELIVER reads this file
at the start of each step to confirm the RED it un-pends is genuine.

## Acceptance (`@team-membership`, six feature files)

**Run** (2026-10-09): `@pending` stripped from the six
`team-membership-0*.feature` files only (a `sed` over copies kept in the
scratchpad), `foundry` rebuilt and warmed, then
`FOUNDRY_ACCEPTANCE_TAGS=team-membership cargo test -p foundry-acceptance --test acceptance`.
The files were then restored byte for byte from the copies (57 of 57 `@pending`
tags back: `grep -cE '^\s*@.*@pending'` per file = 17, 11, 10, 9, 7, 3).

**Result**: 6 features, **87 scenarios** (57 scenario declarations, outline rows
expanded), 491 steps; **85 RED (MISSING_FUNCTIONALITY), 2 GREEN_ALREADY guards,
0 BROKEN**; 0 parsing errors, 0 hook errors. 406 steps passed: every Background,
every sign-in (password and Keycloak), every precondition seed, every universe
capture and every byte-identical refusal comparison ran.

**Verdict: PASS.** Zero BROKEN, zero false-GREEN.

### Why no refusal passes for the wrong reason

None of the six routes exists yet, so today every one of them already answers the
uniform 404 that the authorization scenarios assert. A refusal scenario that only
compared bytes would be green now and prove nothing. Each refusal scenario
therefore also asserts that the team it hid exists ("Priya still finds Backend's
members page", "Hugo still finds Payments's members page", "Priya's teams page
still lists Backend", "the same add with the page's token puts Tomás Rivera on
Backend", "the teams page shows General as the default team"). Those controls are
what is RED today, and they turn green only when the route serves the authorized
caller, at which point the byte-identical refusal becomes meaningful. This
replaces the board-lane-reorder idiom of mounting 501 stubs at DISTILL, which the
DISTILL brief ruled out ("no route wiring that changes current behaviour").

### By failure point

| Where it stops | Rows | Classification |
|---|---|---|
| The dashboard has no "Teams" quick action (`When … follows Teams from her dashboard`) | 2 (walking skeleton; admin finds a team) | MISSING_FUNCTIONALITY |
| `/team/{slug}/members` answers 404, so the page offers no Add / role / Remove form (`When`/`Given … adds / makes / removes / opened … earlier`) | 20 | MISSING_FUNCTIONALITY |
| `/teams` answers 404 (`Then the teams page lists …`, `he sees "You're not on any team yet …"`, `the teams page has no default team setting`) | 3 | MISSING_FUNCTIONALITY |
| `/teams` offers no default-team form (`Given/When … sets … as the default team`) | 12 | MISSING_FUNCTIONALITY |
| A refusal's control: the hidden team's members page (or teams page, or the add with a token) answers 404 | 38 | MISSING_FUNCTIONALITY |
| A members page read answers 404 (`Then …'s members page lists …`, `Add a member … offers`, `the page offers …`) | 5 | MISSING_FUNCTIONALITY |
| The role route answers 404 instead of the stale re-render (DDD-9) | 1 | MISSING_FUNCTIONALITY |
| The HTTP race: run 1, both POSTs answer 404 instead of one 200 and one 409 | 2 | MISSING_FUNCTIONALITY |
| `POST /workspace/members/remove` answers 200 "member removed" for a last lead instead of the 409 (OQ-9) | 1 | MISSING_FUNCTIONALITY |
| State delta: Sam's team row survives his removal from the workspace (OQ-9) | 1 | MISSING_FUNCTIONALITY |
| Passes | 2 | GREEN_ALREADY (guards, below) |

### Per scenario

| # | File | Scenario | Rows | RED |
|---|---|---|---|---|
| 1 | 01 | **WS** Jeff puts his own Keycloak account on General, and it opens the Sandbox board | 1 | MF: no "Teams" quick action (the Keycloak first sign-in and the "on no team, Sandbox 404" precondition passed) |
| 2 | 01 | A workspace admin finds a team and sees who is on it, leads first | 1 | MF: no "Teams" quick action |
| 3 | 01 | A workspace admin sees the members of a team she is not on, but still cannot open its board | 1 | MF: `/team/ops/members` 404 |
| 4 | 01 | A team member sees their own team, without any way to change it | 1 | MF: `/teams` 404 |
| 5 | 01 | Someone on no team is told so on the teams page | 1 | MF: `/teams` 404 |
| 6 | 01 | Someone who may not see a team's members gets the page that never existed | 4 | MF: control (refusal already byte-identical) |
| 7 | 01 | Another workspace's team and a team that does not exist look the same from Canzan Labs | 1 | MF: control |
| 8 | 01 | A signed-out visitor learns nothing from the teams page | 1 | MF: control |
| 9 | 01 | A workspace admin puts a member on a team, and the member can open the team's board | 1 | MF: no Add form |
| 10 | 01 | Only workspace members who are not on the team are offered, each by name and address | 1 | MF: no Add form |
| 11 | 01 | When everyone is already on the team, there is nobody to add | 1 | MF: members page 404 |
| 12 | 01 | Adding someone already on the team, from an older page, changes nothing and says so | 1 | MF: members page 404 |
| 13 | 01 | Someone who is not a Canzan Labs member cannot be added | 3 | MF: control |
| 14 | 01 | Someone without the right cannot add anyone to a team | 5 | MF: control |
| 15 | 01 | An add aimed at another workspace's team or at a team that does not exist is refused the same way | 1 | MF: control |
| 16 | 01 | Someone who left the workspace cannot be added from a page opened before they left | 1 | MF: members page 404 |
| 17 | 01 | An add sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |
| 18 | 02 | A workspace admin makes a member a lead | 1 | MF: no role form |
| 19 | 02 | A workspace admin hands leadership over by making the previous lead a member | 1 | MF: no role form |
| 20 | 02 | The last lead cannot be made a member | 1 | MF: no role form |
| 21 | 02 | Making a lead a lead again changes nothing and says so | 1 | MF: no role form |
| 22 | 02 | A role change for a workspace member who is not on the team changes nothing | 1 | MF: role route 404 |
| 23 | 02 | Two leads made members at the same moment leave the team with exactly one lead | 1 | MF: race run 1, both 404 |
| 24 | 02 | A team that has no lead can be given one | 1 | MF: no role form |
| 25 | 02 | Someone without the right cannot change a role | 4 | MF: control |
| 26 | 02 | A role change naming someone outside the workspace, or no one, is refused | 3 | MF: control |
| 27 | 02 | A role that does not exist is refused, and nothing changes | 1 | MF: control |
| 28 | 02 | A role change aimed at another workspace's team is refused the same way | 1 | MF: control |
| 29 | 03 | A workspace admin takes a member off a team, and the member loses its board at once | 1 | MF: no Add form (Given) |
| 30 | 03 | Taking one of two leads off a team leaves the other in charge | 1 | MF: no role form (Given) |
| 31 | 03 | The last lead cannot be taken off the team | 1 | MF: no Remove form |
| 32 | 03 | Removing someone who has already left the team, from an older page, changes nothing | 1 | MF: members page 404 |
| 33 | 03 | A removal and a demotion at the same moment leave the team with exactly one lead | 1 | MF: race run 1, both 404 |
| 34 | 03 | Someone without the right cannot take anyone off a team | 4 | MF: control |
| 35 | 03 | A removal naming someone outside the workspace, or no one, is refused | 3 | MF: control |
| 36 | 03 | Removing someone from the workspace also takes them off its teams | 1 | MF: state delta, Sam's Backend row survives |
| 37 | 03 | A lead who shares the lead with someone else can be removed from the workspace | 1 | MF: no role form (Given) |
| 38 | 03 | The last lead of a team cannot be removed from the workspace | 1 | MF: 200 "member removed" instead of 409 |
| 39 | 04 | A team lead puts a workspace member on her own team | 1 | MF: members page 404 |
| 40 | 04 | A team lead hands leadership over and steps down | 1 | MF: no role form (Given) |
| 41 | 04 | A team lead takes a teammate off her team | 1 | MF: no Remove form |
| 42 | 04 | A team's only lead cannot step down or leave it | 2 | MF: no role / Remove form |
| 43 | 04 | A team lead can take a workspace admin off her team | 1 | MF: no Remove form |
| 44 | 04 | A team lead can make a workspace admin on her team a member again | 1 | MF: no role form (Given) |
| 45 | 04 | A team lead cannot change a team she does not lead | 6 | MF: control |
| 46 | 04 | A team lead sees a team she is only a member of without any way to change it | 1 | MF: members page 404 |
| 47 | 04 | A lead who has since been made a member cannot act from a page she opened as a lead | 1 | MF: members page 404 |
| 48 | 05 | A workspace admin picks the default team for new members, and no one already here is moved | 1 | MF: no default-team form |
| 49 | 05 | A workspace admin clears the default team | 1 | MF: no default-team form (Given) |
| 50 | 05 | An invited newcomer lands on the default team and opens its board | 1 | MF: no default-team form (Given) |
| 51 | 05 | Without a default team, an invited newcomer lands as today | 1 | **GA** guard (D17's floor, OQ-4) |
| 52 | 05 | A default team that no longer exists never stops a newcomer joining | 1 | MF: no default-team form (Given) |
| 53 | 05 | Only a workspace admin can set the default, and only to one of Canzan Labs's teams | 5 | MF: no default-team form (Given) |
| 54 | 05 | Only a workspace admin is offered the default team setting | 1 | MF: `/teams` 404 |
| 55 | 06 | A newcomer's first Keycloak sign-in lands them on the default team | 1 | MF: no default-team form (Given) |
| 56 | 06 | Signing in with Keycloak to an existing account adds no team | 1 | MF: no default-team form (Given) |
| 57 | 06 | An unset or missing default never stops a first Keycloak sign-in | 2 | row "no default team": **GA** guard (D17, keycloak-sso floor); row "deleted": MF (Given) |

### The two GREEN_ALREADY guards

Both pin D17's floor, "at worst the newcomer lands on no team, which is today's
behaviour". They pass now because no placement exists, and they must stay green
when DELIVER 05-01 and 06-01 add placement: a placement that ran with no default
set, or that touched a second team, would turn them red (the state delta allows
exactly one new account and one workspace row, nothing else). DELIVER un-pends
them with the slice that could break them (05-01, 06-01).

### What this run cannot prove (DELIVER's first check per step)

Most scenarios stop at the first missing page, so the steps after it have run only
by construction, not against a server. The driver's oracles that did run in full
are: the universe capture and the state delta (scenario 36 failed inside it with a
precise diff), the byte-identical 404 (every refusal row passed it), the race
harness (both runs fired two concurrent POSTs and read the lead count), the
Keycloak first sign-in and the invite issue-and-accept round trip (scenario 51
passed end to end). When DELIVER un-pends a scenario and it fails *after* the
point recorded above for a reason that is not the feature (a selector, a parsing
assumption about the page), that is a test defect to fix in the driver
(`support/team_membership.rs`), and DESIGN's pinned DOM (DDD-17) is the reference:
rows "{name} · {email} · {Lead|Member}", candidates "{name} — {email}" in
`select[name=user_id]` inside the form posting to `/team/{slug}/members`, role
forms posting `role` to `/team/{slug}/members/{user_id}/role`, Remove forms
posting to `/team/{slug}/members/{user_id}/remove`, the default form posting
`team` (a slug, or empty for "No team") to `/teams/default`, and `/teams` rows
"{team} · {n} member(s)" linking to the members page.

## Store (`crates/foundry-store/tests/`)

**Run**: `cargo test -p foundry-store --test team_lead_guard -- --include-ignored`
and `--test workspace_default_team -- --include-ignored`.

| File | Tests | RED (MF) | GREEN now (not ignored) | BROKEN |
|---|---|---|---|---|
| `team_lead_guard.rs` | 19 | 17: 11 stop on a scaffold panic (`Not yet implemented -- RED scaffold (team-membership 0N-01: …)`), 4 on the 0021 RED gate (`migration 0021 has not installed the lead guard yet`), 2 on an assertion against today's `remove_workspace_member` (`left: Removed, right: LastLead("backend")`; Sam's Backend row survives) | 2 substrate facts (ADR-001 Earned Trust 3 and 4: an add is not blocked by the team lock; a read after the lock wait sees the holder's commit) | 0 |
| `workspace_default_team.rs` | 12 | 10: all on the 0022 RED gate (`migration 0022 has not added workspaces.default_team_id yet`) | 2 guards (no default: an invite accept and a first provisioning place no one) | 0 |

Without `--include-ignored`: 2 passed, 17 ignored; 2 passed, 10 ignored.
