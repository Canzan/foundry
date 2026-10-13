# RCA: fix-issue-attachment-scoping

Date: 2026-10-12. Method: Toyota 5 Whys, more than one cause per defect. Read-only: no code changed, no lanes run.
Every finding below comes from source, git history or schema. Items that still need checking are marked **Hypothesis**.

## 0. Problem statement and scope

| | Defect 1: cross-workspace issue lookup by key prefix | Defect 2: attachment download ignores the resolved issue |
|---|---|---|
| Surface | Board move (web dnd, web dialog, `PATCH /api/v1/.../issues/{n}`), issue edit submit, issue edit pre-fill | `GET /team/{t}/project/{p}/issues/{n}/attachments/{id}` |
| Defect | The issue row is chosen by `(key_prefix, number)` across **all** workspaces | The attachment is served by `(id, workspace)`. The team, project and issue in the URL are not used to pick it |
| Precondition | Two workspaces each have a project with the same `key_prefix` | The caller is on at least one team in the workspace and knows an attachment id from another team or issue |
| Impact | Reads (title and description) and writes (state, rank, title, description, change history, outbox event) on another tenant's issue | Disclosure of any attachment in the workspace across team boundaries |
| Severity | High: cross-tenant read and write. Latent today | High: cross-team read. Live today wherever a workspace has more than one team |

Out of scope: the uniform-404 refusal-shape work (MWT ADR-003). It is mentioned only where it bears on these defects.

---

## 1. Defect 1: confirmed

### Evidence that the defect is real

- `crates/foundry-store/src/lib.rs:1959` `reposition_issue_with_outbox`: the query at `:1970-1974` runs `SELECT i.id, i.project_id, i.workspace_id, i.state, i.position FROM issues i JOIN projects p ON p.id = i.project_id WHERE p.key_prefix = $1 AND i.number = $2` and then calls `fetch_optional`.
- `:2302` `update_issue_details`: the same predicate at `:2316`. It then UPDATEs the row and writes `issue_change_events` using the **found** row's `workspace_id`/`project_id`.
- `:2373` `issue_edit_view`: the same predicate at `:2382`. It returns `title, description_md, state`.
- Schema: `0001_init.sql:61` has `UNIQUE (workspace_id, key_prefix)`. No later migration adds a global unique on `key_prefix`; a grep of `migrations/*.sql` finds only `0001:57,61`. No trigger touches `projects.key_prefix`: the only triggers on `projects` are `0020` name-rule triggers, which check the name only.
- `0009_multi_workspace.sql:12` (`223c148`, 2026-06-09) runs `DROP INDEX IF EXISTS uniq_one_workspace`. Until then `0001:15` limited the instance to **one** workspace, which made `key_prefix` globally unique by accident.
- Project creation takes `key_prefix` from user input (`foundry-services/src/projects.rs:252`). Two workspaces choosing `OPS` is ordinary use, not an edge case.
- Nothing else stops it:
  - `fetch_optional` in sqlx returns the first row and does **not** fail on more than one row.
  - The query has no `ORDER BY`, so which of two matching rows comes back depends on the query plan.
  - `UNIQUE (project_id, number)` (`0001`, issues) does not limit `(key_prefix, number)` across projects.
- Callers in `crates/foundry-services/src/issues.rs` resolve the caller's project correctly but pass on only the prefix:
  - `change_issue_state` (`:215`) gets `project` from `resolve_member_project` (`:225`), checks the lane against **A's** `project.id` (`:231`), then calls `reposition_issue_with_outbox(key_prefix.as_str(), …)` (`:248`).
  - `edit_issue_details` (`:285`) ignores the project with `_project` (`:294`) and calls `update_issue_details(key_prefix.as_str(), …)` (`:313`).
  - `edit_issue_form` (`:348`) calls `issue_edit_view(key_prefix.as_str(), number)` (`:359`).
- Reachable from:
  - web `foundry-app/src/issues.rs:207,256,509,526,564`
  - API `foundry-api/src/lib.rs:437`, which is `PATCH /api/v1/teams/{t}/projects/{p}/issues/{number}` with a machine token

### What an attacker can do (forward trace)

Workspace A has project `OPS` (team X). Workspace B has project `OPS`. A member of X in A sends a request for `OPS-n`:

- **Deterministic when A has no `OPS-n` but B does.** Example: B's issues go higher than A's. Exactly one row matches, B's, so:
  - the edit pre-fill **returns B's title and description**
  - edit submit **overwrites B's issue**
  - a move **changes B's state and rank**
  - In every case the actor recorded in B's `issue_change_events` is A's user, which leaks A's user identity into B's history.
  - The move's outbox `IssueUpdated` carries B's `workspace_id`, so B's SSE viewers receive it.
  - The attacker can target any number above A's highest by guessing numbers. Issue numbers are dense and sequential.
- **Nondeterministic when both have `OPS-n`.** An honest edit in A may silently land on B's issue. This also breaks data integrity for users who mean no harm.
- A side effect of the move path: `validate_project_lane` checks A's lane set, but the write lands in B. If B lacks that lane, the `fk_issues_lane` FK fails and the user gets a 500. If B has the lane, the write commits.

### Sweep: every store query that filters projects or issues by `key_prefix` or slug

| # | Site | Predicate | Scoped? | Caller guard | Verdict |
|---|---|---|---|---|---|
| 1 | `lib.rs:1974` `reposition_issue_with_outbox` | `p.key_prefix=$1 AND i.number=$2` | **No** | `resolve_member_project` resolves A's project, then drops its id | **Vulnerable** |
| 2 | `lib.rs:2316` `update_issue_details` | same | **No** | same | **Vulnerable** |
| 3 | `lib.rs:2382` `issue_edit_view` | same | **No** | same | **Vulnerable** (read) |
| 4 | `lib.rs:2439` `find_issue_by_team_project_number` | `p.team_id=$1 AND p.slug=$2 AND i.number=$3` | team_id (team comes from a workspace-scoped lookup) | yes | Safe |
| 5 | `lib.rs:1718` `find_project_by_slug` | `team_id=$1 AND slug=$2` | team_id | yes | Safe |
| 6 | `lib.rs:1583` `find_team_by_slug` | `workspace_id=$1 AND slug=$2` | workspace | yes | Safe |
| 7 | `teams.rs:202` team access facts | `t.workspace_id=$1 AND t.slug=$2` | workspace | yes | Safe |
| 8 | `lanes.rs:195,231,555,615,637,719,741` | `project_id=$1 AND slug=$2` | project_id | yes | Safe |
| 9 | `teams.rs:589` `TEAM_PROJECTS_SQL` | `team_id=$1` | team | yes | Safe |
| 10 | `lib.rs:2248` `list_project_changes` | `e.project_id=$1` (key_prefix is only selected) | project | yes | Safe |
| 11 | `lib.rs:2775,2836` comment edit/delete sub-selects | `comments.id=$1 AND workspace_id=$2`; key_prefix comes from the comment's own issue | workspace (see §2 sibling) | author/admin | Safe for Defect 1 |
| 12 | `lanes.rs:302,361`, `issue_delete.rs:83` | `projects WHERE id=$1` | id | yes | Safe |

No production SQL exists outside `foundry-store`: grep finds `sqlx::query` only in `foundry-app/src/admin_cli.rs`, which has no key_prefix or slug filter, and in acceptance steps. **Rows 1-3 are the complete set for Defect 1.**

### History: why the scoping was missed

- `33e5f6f` (2026-05-24, initial commit): `update_issue_state_with_outbox` introduced `WHERE p.key_prefix = $1 AND i.number = $2` (old `lib.rs:661`). At that point `uniq_one_workspace` meant one workspace per instance, so the query was correct.
- `223c148` (2026-06-09): MWT dropped `uniq_one_workspace`. The key-prefix lookup was not changed.
- MWT ADR-002 *Context* (`docs/feature/multi-workspace-tenancy/design/adr-002-isolation-enforcement-seam.md`) says: "The shipped code already scopes every tenant query by `workspace_id`". That claim was never checked by an inventory, and it was false for this query.
- `9e5484f` (2026-07-05, issue-edit-dialog): **after** MWT, two new copies of the predicate were added, in `update_issue_details` and `issue_edit_view`. The pattern was copied from the pre-MWT state-change query.
- `b8e3ae6` (2026-07-06, card-ranking): `update_issue_state_with_outbox` became `reposition_issue_with_outbox` and kept the predicate. `1e4740a` (2026-07-07) reworked the history.
- `crates/foundry-store/tests/update_issue_details.rs:164-226` is named `..._is_tenant_isolated` and says it tests isolation, but it seeds the two workspaces with **different** prefixes (`GEN` vs `FGN`). Distinct prefixes make the isolation automatic, so the test cannot fail for this defect.

### 5 Whys: Defect 1

| Level | Branch 1A: the query | Branch 1B: the guards |
|---|---|---|
| WHY 1 (symptom) | Moving, editing or pre-filling `OPS-n` can resolve B's issue. [`lib.rs:1974/2316/2382`; `fetch_optional` takes the first row] | No test, review or guard flagged it. [test `GEN`/`FGN`; check-arch passes] |
| WHY 2 (context) | The store looks up by `(key_prefix, number)`, which is not a unique key once there are many workspaces. [`0001:61` is per-workspace unique only] | The isolation tests always use distinct prefixes. [`update_issue_details.rs:171,181`; mwt-slice-04 `ACME`/`GLOBEX`, feature lines 99-100] |
| WHY 3 (system) | The services resolve `project` but pass only `key_prefix` downstream. The identifier that was already resolved is thrown away. [`issues.rs:225→248`, `:295→313`, `:356→359`] | check-arch LAYER-1e only checks where the workspace argument of `*_in_workspace(` calls in `foundry-app` comes from. A query with **no** scope argument is invisible to it. [`xtask/src/check_arch.rs:391-449`] |
| WHY 4 (design) | The query was written when there was one workspace per instance, so `key_prefix` was globally unique by accident. [`33e5f6f`; `0001:15`] | ADR-002 assumed "already scopes every tenant query" and chose a guard on argument origin over a store inventory or a scoped-store wrapper. [ADR-002 Context and Decision; option (b) rejected] |
| WHY 5 (root cause) | **RC-1: removing the one-workspace invariant (`0009`) was not followed by a review of the queries that silently relied on it.** The pattern was then copied into new code (`9e5484f`). | **RC-2: tenant isolation is checked against the wrong property.** Guards check *where the workspace id comes from*, not *whether every lookup is keyed by a resolved owner id*. The fixtures make the dangerous case (identical natural keys across tenants) impossible to hit. Mutation testing cannot catch a missing SQL predicate inside a string literal; `probe_schema_scoping.rs:1-17` documents the same blind spot. |

Backwards check: if RC-1 holds and the queries are unscoped, and RC-2 holds and nothing tests a prefix collision, then the defect ships and stays undetected. Both match what we observe.

---

## 2. Defect 2: confirmed

### Evidence

- In `crates/foundry-app/src/attachments.rs:232-238`:

  ```rust
  if let Err(err) = state.store
      .find_issue_by_team_project_number(team.id, &project_slug, issue_number).await
  { return internal_error(...); }
  ```

  `Ok(None)` and `Ok(Some(issue))` are both thrown away.
- `:240-252` then calls `find_attachment_in_workspace(attachment_id, user.workspace_id)`. The store query (`foundry-store/src/attachments.rs:110-122`) is `WHERE id = $1 AND workspace_id = $2`.
- Combined effect: any member of **any** team X in workspace W gets `200` plus the bytes for any attachment id in W, using `/team/X/project/<anything>/issues/<anything>/attachments/{id}`. The project slug and issue number do not even have to exist.
- History:
  - The "Defensive lookup" comment, which says "both layers must agree", has been unchanged since `b2475f6` (2026-05-25, Slice 3).
  - `02cc6fb` (2026-06-10, MWT 04-02) changed this handler's refusal shape. It did not notice that the issue lookup result is discarded, because the change was about cross-workspace behaviour and workspace scoping did hold.
- How guessable the ids are:
  - `attachments.rs:157` uses `uuid::Uuid::now_v7()` (uuid 1.23.1).
  - v7 is a 48-bit millisecond timestamp plus about 74 bits filled from random data (uuid ≥1.9 may use part of it as a randomly seeded counter).
  - Ids cannot be brute-forced. The real exposure is **leaked or kept ids**:
    - the link is in Y's issue-page HTML (`comments.rs:751`)
    - links get pasted into chat or email
    - a user removed from team Y but still on team X keeps every URL they saw
  - The id prefix also reveals the upload time.

### The same pattern elsewhere

| Site | Pattern | Exposure |
|---|---|---|
| `attachments.rs:64` `submit_upload` | `Ok(None)` → `resource_not_found_page()` (`:121`); inserts on `issue.issue_id` | **Safe** |
| `comments.rs:58` `show_issue` → `list_attachments_for_issue(issue.issue_id)` (`:107`) | resolved issue | **Safe** |
| `comments.rs:502` `show_single_comment` (GET `/…/comments/{id}`) | team membership on the path team, then `find_comment_by_id(workspace, id)` (store `lib.rs:2594`: `c.id=$1 AND c.workspace_id=$2`). Path project and issue are ignored; there is no author check | **Same defect class**: any team member can read any comment body and author email in the workspace by comment id (`now_v7`, `services/comments.rs:71`). Also returns `team_not_found_page(&team_slug)`, which echoes the slug (non-uniform) |
| `comments.rs:264` `show_edit_form`, `:343` edit, `:418` delete; `services/comments.rs:103` `edit_comment` (`_project_slug`, `_issue_number` ignored, `:107-108`); API `PATCH …/comments/{comment_id}` | lookup by `(workspace, comment_id)`; path is ignored (`let _ = (project_slug, issue_number)`) | Low: author-only (edit) and author-or-admin (delete) gates stop privilege gain. A former member of team Y can still edit or delete their own Y comments through a team-X URL |
| `foundry-api` | no attachment routes (`lib.rs:312-341`) | n/a |

### Why the tests missed it

- `us-11-attachments.feature:82` ("outside the team cannot download"): the step `perform_download` (`us_11_attachments.rs:857-882`) always builds the URL from the **owning** team `"backend"`. Rita fails the membership check, so the test never tries her **own** team's URL with a foreign id.
- `us-mwt-slice-04-non-enumerability.feature:193` only covers cross-**workspace** access, which workspace scoping blocks correctly.
- No unit or app test in `foundry-app/tests` covers attachments; the folder holds only 3 files, none about attachments.
- check-arch LAYER-1e (`check_arch.rs:396`) treats `find_*_in_workspace(id, user.workspace_id)` as the canonical **safe** idiom, so it approves this call.

### 5 Whys: Defect 2

| Level | Branch 2A: handler | Branch 2B: model and guards |
|---|---|---|
| WHY 1 | A team-X member downloads a team-Y attachment. [`attachments.rs:232-252`] | Tests, review and check-arch all passed. [`us_11_attachments.rs:858`; `check_arch.rs:396`] |
| WHY 2 | The issue lookup result is discarded, and the attachment is chosen by `(id, workspace)`. [`:232-238`, store `attachments.rs:117-120`] | The negative test goes through the owning team's URL, so the membership check, not the issue check, refuses it. [feature `:82-88`] |
| WHY 3 | The store has no "attachment of issue" lookup. The only lookup is by workspace (`find_attachment_in_workspace`), whose doc comment says "Authorisation is the handler's job". [store `attachments.rs:9-13`] | Workspace is treated as the authorisation boundary for resources, but within a workspace authorisation is per team. [MWT ADR-002/003 focus; `show_single_comment` repeats the pattern] |
| WHY 4 | Slice 3 (`b2475f6`) was single-workspace and effectively single-team. "Workspace-scoped" felt like full scoping, and the lookup was labelled "defensive" rather than authoritative. | MWT 04-02 (`02cc6fb`) hardened this handler only against the cross-*workspace* threat. Cross-*team* within a workspace was not in its threat model. |
| WHY 5 | **RC-3: child resources (attachments, comments) are looked up by their own id plus tenant, not through their parent chain (team → project → issue → child).** So the authorised path and the served resource are never joined. | **RC-2 (shared): isolation guards and fixtures cover one boundary, workspace, and miss the narrower one, team.** The guard's "safe idiom" itself encodes the weaker scope. |

Backwards check: RC-3 explains both `download_attachment` and `show_single_comment`. RC-2 explains why neither was caught. Nothing contradicts either chain.

---

## 3. Shared cause and contributing factors

**Shared root cause (RC-2):** authorisation scoping has no single rule that is enforced. The rule should be that every lookup of an issue-level resource is keyed by the **resolved** id of its authorised parent. Today scoping is enforced partly (workspace-argument origin, in `foundry-app` only), assumed elsewhere (the ADR-002 Context claim), and tested with fixtures whose natural keys never collide.

Contributing factors:

1. **CF-1:** The resolver returns `(team, project, key_prefix)` and callers re-identify the issue from a natural key instead of the resolved id. The underscore-prefixed `_project` at `issues.rs:294` shows the id was available and deliberately not used.
2. **CF-2:** sqlx `fetch_optional` takes the first of several rows silently. A `fetch_one`, or a unique-assumption assertion, would have turned the ambiguity into an error.
3. **CF-3:** Mutation testing, the project's ≥80% gate, cannot mutate SQL string literals, so a missing predicate survives. Precedent: `probe_schema_scoping.rs`.
4. **CF-4:** Copy-paste spread. `9e5484f` copied the pre-MWT predicate 4 weeks after MWT shipped.
5. **CF-5:** Fixture design. Every multi-workspace fixture uses distinct prefixes (`GEN`/`FGN`, `ACME`/`GLOBEX`), and the attachment negative test uses the owning team's URL.
6. **CF-6:** Doc drift. mwt-slice-04 (feature lines 40, 188) names a `find_attachment_for_requester` idiom that does not exist, and `attachments.rs:221-225` still says "keeps its shipped 403 shape" above a 404 return. Reviewers reading the comments would believe the path is tighter than it is.

---

## 4. Proposed fix

### Defect 1: key the three store functions by the resolved `project_id` (project-lifecycle DDD-5)

`crates/foundry-store/src/lib.rs`:

```rust
// reposition_issue_with_outbox(:1959)
pub async fn reposition_issue_with_outbox(&self, project_id: uuid::Uuid, project_key_prefix: &str,
    issue_number: i32, new_state: &str, after_number: Option<i32>, actor_id: uuid::Uuid) -> …
//   SELECT i.id, i.project_id, i.workspace_id, i.state, i.position
//     FROM issues i WHERE i.project_id = $1 AND i.number = $2
// (project_key_prefix kept only for the outbox payload `key`; or SELECT p.key_prefix
//  via JOIN projects p ON p.id = i.project_id and drop the parameter.)

// update_issue_details(:2302)
pub async fn update_issue_details(&self, project_id: uuid::Uuid, issue_number: i32, …)
//   … FROM issues i WHERE i.project_id = $1 AND i.number = $2

// issue_edit_view(:2373)
pub async fn issue_edit_view(&self, project_id: uuid::Uuid, issue_number: i32)
//   SELECT i.title, i.description_md, i.state FROM issues i
//    WHERE i.project_id = $1 AND i.number = $2
```

`(project_id, number)` is `UNIQUE` (`0001` issues), so each lookup now returns at most one row, by construction. When project-lifecycle step 02 lands, these joins switch to `live_projects` (DDD-5 table `:925`). Coordinate so only one change rewrites these lines.

`crates/foundry-services/src/issues.rs`:
- `change_issue_state` `:248`: pass `project.id`.
- `edit_issue_details` `:294`: rename `_project` to `project` and pass `project.id` at `:313`.
- `edit_issue_form` `:359`: pass `project.id`.

Defence in depth, optional: add `-- one row by UNIQUE(project_id, number)` comments. Consider `fetch_optional` → a helper that errors on more than one row, for natural-key lookups that remain.

### Defect 2: honour `Ok(None)` and look the attachment up by (attachment id, resolved issue id) (DDD-32)

`crates/foundry-app/src/attachments.rs` `:228-252`:

```rust
let issue = match state.store
    .find_issue_by_team_project_number(team.id, &project_slug, issue_number).await
{
    Ok(Some(i)) => i,
    Ok(None) => return resource_not_found_page(),
    Err(err) => return internal_error("find_issue_by_team_project_number", err),
};
match state.store.find_attachment_for_issue(attachment_id, issue.issue_id).await {
    Ok(Some(row)) => attachment_download_response(&row),
    Ok(None) => resource_not_found_page(),
    Err(err) => internal_error("find_attachment_for_issue", err),
}
```

`crates/foundry-store/src/attachments.rs`:
- Add `find_attachment_for_issue(attachment_id, issue_id)` with `WHERE id = $1 AND issue_id = $2`.
- Optionally keep `AND workspace_id = $3`, bound from `issue.workspace_id`, for defence in depth.
- Remove `find_attachment_in_workspace` once nothing calls it. Its only remaining reference is the check-arch planted-violation gold string at `foundry-acceptance/src/steps/feature_a_programmatic.rs:1626`, which must be moved to another `*_in_workspace` name.
- Fix the stale comments at `:221-225` and `:228-231`.

### Sibling found by the sweep (recommended in the same fix)

`crates/foundry-app/src/comments.rs:502` `show_single_comment`:
- Resolve the issue with `find_issue_by_team_project_number`, and return `resource_not_found_page()` on `Ok(None)`.
- Key the comment by `(comment_id, issue.issue_id)`, using a new `find_comment_for_issue`, or check `comment.issue_id == issue.issue_id` if `CommentLookupRow` carries `issue_id`.
- Replace `team_not_found_page(&team_slug)` with `resource_not_found_page()`.

Apply the same issue-binding to `show_edit_form`, `submit_edit_comment`/`services::comments::edit_comment`, `submit_delete_comment` and the API comment PATCH. These are lower severity but the same class.

### Prevention (systemic, maps to RC-1/RC-2/RC-3)

| Root cause | Action | Type |
|---|---|---|
| RC-1 | When a schema invariant is relaxed (a dropped UNIQUE or index), require a grep inventory of queries that relied on it. Add it to the DESIGN checklist and record it in the feature-delta, as project-lifecycle's Reader Inventory does | Permanent |
| RC-2 | Add a check-arch rule: in `foundry-store`, a `WHERE` on `key_prefix` or `slug` must also include `workspace_id`, `team_id` or `project_id`. Plant a violation as a gold test | Permanent |
| RC-2 | Make **colliding natural keys** the default in two-tenant fixtures: same `key_prefix` and same team and project slugs in both workspaces, and cross-team negative tests that use the attacker's **own** URL | Permanent |
| RC-3 | Store convention: child resources (attachments, comments) are fetched only by `(child id, parent id)`. Document it in `foundry-store/src/attachments.rs` and the comments module header | Permanent |
| CF-3 | Use deterministic SQL-predicate regression tests, like the ones in §5, wherever a predicate lives in a string literal | Permanent |

---

## 5. Files affected

- `crates/foundry-store/src/lib.rs` (`:1959`, `:2302`, `:2373`; optional `find_comment_for_issue` near `:2580`)
- `crates/foundry-store/src/attachments.rs` (`:110` replaced or added)
- `crates/foundry-services/src/issues.rs` (`:248`, `:294`, `:313`, `:359`)
- `crates/foundry-services/src/comments.rs` (`:103-140`, sibling)
- `crates/foundry-app/src/attachments.rs` (`:228-252`)
- `crates/foundry-app/src/comments.rs` (`:264`, `:343`, `:418`, `:502`, sibling)
- Existing tests that call the changed signatures:
  - `crates/foundry-store/tests/reposition_issue.rs:185,219`
  - `crates/foundry-store/tests/update_issue_details.rs:200,247`
  - `crates/foundry-services/tests/board_use_case.rs:141`
- `crates/foundry-acceptance/src/steps/feature_a_programmatic.rs:1626` (gold string, if `find_attachment_in_workspace` is removed)
- `xtask/src/check_arch.rs` (prevention rule)

## 6. Regression tests (each must fail on current code)

| # | Crate / file | Setup | Asserts | Why it fails today |
|---|---|---|---|---|
| T1 | `foundry-services/tests/write_use_cases.rs` (new test) | Workspaces A and B, each with a project prefix `OPS`. A has `OPS-1`; B has `OPS-1..OPS-3`. Member of A's team | `edit_issue_form(A team, A project, 3)` → `Err(NotFound)` | Returns B's `OPS-3` title. Deterministic: one matching row |
| T2 | same | same | `edit_issue_details(A…, 3, "pwn", …)` → `Err(NotFound)`; B's `OPS-3` title unchanged; no `issue_change_events` row on B | Overwrites B and records A's actor in B's history |
| T3 | same | same, B's `OPS-3` in `backlog` | `change_issue_state(A…, 3, "todo", None)` → `Err(NotFound)`; B's state still `backlog`; no outbox row | Moves B's issue and emits B's `IssueUpdated` |
| T4 | `foundry-store/tests/update_issue_details.rs` + `reposition_issue.rs` (new tests) | Two workspaces, both prefix `GEN`, both `GEN-1` | Calling with A's `project_id` changes only A's row; B is byte-identical | Fails to compile against the current signature (RED). After the fix it pins the predicate |
| T5 | `foundry-app/tests/attachment_scoping.rs` (new; `build_router` + `oneshot` + testcontainers, like `password_less_account_doors.rs`) | One workspace; teams X and Y; Rita on X only; Y's project has issue 1 with an attachment `id` | `GET /team/x/project/<x-proj>/issues/1/attachments/{id}` as Rita → 404, body equals `resource_not_found_page` | Returns 200 with the bytes |
| T6 | same | same | `GET /team/x/project/no-such/issues/999/attachments/{id}` → 404 | Returns 200: `Ok(None)` is ignored |
| T7 | same | Y member; Y issues 1 and 2; attachment on issue 1 | `GET /team/y/project/<y-proj>/issues/2/attachments/{id}` → 404 | Returns 200: issue not bound |
| T8 | same (positive control) | Y member | the correct URL → 200, byte-identical | Passes today; guards against over-tightening |
| T9 | same, or `comments` test | Rita on X; comment `c` on Y's issue | `GET /team/x/project/<x>/issues/1/comments/{c}` → 404 | Returns 200 with Y's comment body and author email |
| T10 | `foundry-acceptance` `us-11-attachments.feature` (new scenario, `@nfr-sec-06`) | "Rita attempts to download screenshot.png from AUTH-1 through her own team Partners' address" | refused as if it never existed | Step builds the URL with `partners`; today it gets 200 |
| T11 | `foundry-acceptance` `us-mwt-slice-04` (new scenario or Background variant) | Globex project prefix `ACME`, colliding | Marco editing or moving ACME-n through Acme's own URL never touches Globex | Today it touches Globex when Globex has n and Acme does not |

## 7. Risk assessment and exposure

**Fix risk: low.**

- Defect 1:
  - The change is a signature change with 3 production call sites, all of which already hold `project.id`, plus 5 test call sites.
  - Single-workspace behaviour is the same: `(project_id, number)` picks the same row that `(key_prefix, number)` picks when there is one workspace.
  - Merge risk: project-lifecycle step 02 rewrites the same three queries (to `live_projects`). Land one first or fold this into step 02.
- Defect 2:
  - It only tightens access. Every legitimate link is rendered from the issue's own team, project and issue path (`comments.rs:751`), and issues never move between projects.
  - Slugs are immutable on rename (projects D1, teams D1), so old links still resolve.
  - After project-lifecycle soft-delete, links to deleted projects will 404, which is intended (INV-PL-2).
- Comment sibling: same reasoning. Watch the htmx cancel flow (`show_single_comment`), which always uses the issue's own path.

**Current exposure:**

- **Defect 1: latent on dev and prod.** Each has exactly one workspace (per caller), so `key_prefix` is effectively unique and the query resolves correctly. It becomes exploitable the moment a second workspace is provisioned, which is supported since `0009` and the provisioning feature, and a project there picks a prefix already in use. Prefixes are user-chosen 2-6 uppercase letters, so collisions are likely (`OPS`, `WEB`, `API`). The machine-token API PATCH makes it scriptable. **Fix before provisioning a second workspace on prod.**
- **Defect 2: live** on any workspace that has two or more teams with different membership. **Hypothesis, needs verification:** whether dev or prod currently has more than one team, and whether any attachments exist. I could not query the deployed databases read-only from here. Exploiting it needs a known attachment id; ids are not guessable (UUIDv7), so the realistic actors are former members of the owning team and anyone a link was forwarded to. Access logs could show past exploitation: look for 200s on `/attachments/{id}` where the path's team or issue differs from the attachment's `issue_id`.
- **Comment sibling (`show_single_comment`): live**, same conditions as Defect 2.

## Notes

- Peer review (troubleshooter-reviewer) was not run in this pass.
- Every line number refers to the working tree at `3b8f577` (2026-10-12).
