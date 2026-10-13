# Evolution: fix-issue-attachment-scoping (issues, attachments and comments are found only through the URL's own parent)

**Finalized**: 2026-10-13 · **Commits**: RCA + roadmap `06e7112`; steps `ff62b40` (01-01), `4701a69` (01-02), `5911837` (01-03), `7ba8b22` (01-04), `9626a6a` (01-05); refactor `4b957ed`; review fixes `88599ba`; mutation tests `5878aac`. DES integrity: all 5 steps complete. Mutation: check-arch rule 118/119 (99.2%, the one survivor is equivalent); app, services and store 22/22 viable (100%).

RCA: `docs/feature/fix-issue-attachment-scoping/rca.md`. Roadmap: `docs/feature/fix-issue-attachment-scoping/deliver/roadmap.json`.

## Defects

1. **Cross-workspace issue lookup by key prefix.**
   - **Where:** `Store::reposition_issue_with_outbox`, `update_issue_details` and `issue_edit_view` chose the issue with `WHERE p.key_prefix = $1 AND i.number = $2`. `key_prefix` is only `UNIQUE (workspace_id, key_prefix)`. Multi-workspace (`223c148`) dropped the one-workspace limit that had made it globally unique by accident.
   - **Effect:** a member of workspace A could move, edit or pre-fill workspace B's `OPS-n`, if B also used `OPS`. That is a cross-tenant read and write.
2. **Attachment download ignored the resolved issue.** `download_attachment` discarded `Ok(None)` from the issue lookup. It then served `find_attachment_in_workspace(id, workspace)`, so any team member in the workspace could download any attachment by id.
3. **Comments: the same class of defect, found during the RCA.**
   - **Where:** the single-comment view, the edit form, edit submit, delete and the API PATCH all used `find_comment_by_id(workspace, id)` and ignored the project and issue in the URL.
   - **Effect:** any team member could read any comment in the workspace. Edit and delete were still limited by the author-only and author-or-admin checks.

**Exposure at fix time:** none. Dev and prod each have one workspace, with 0 attachments and 0 comments.

## Fix

- **Issues:** the three issue queries are keyed by the resolved `project_id` (`WHERE i.project_id = $1 AND i.number = $2`). The board move reads the outbox `key` prefix from `projects`, joined in the same query, so the caller no longer supplies it.
- **Children by (child id, parent id):** `Store::find_attachment_for_issue(id, issue_id)` and `Store::find_comment_for_issue(id, issue_id)` replace the workspace-only lookups, which are removed. The comment UPDATE and soft-delete statements carry `AND issue_id = $n`. The convention (RC-3) is documented on the store's comment section header.
- **Uniform 404:** an unknown team, project or issue, a non-member, or a child of another issue all get the same byte-exact `resource_not_found_page()`, with no slug echo.
  - The edit form and the mutations answer a mismatched comment with the same fragment as a nonexistent comment id.
  - The keyboard search and the new-issue modal lost their slug-echoing pages as well (review M2).
- **Guard:** a new check-arch rule, `store-natural-key-scoped`. Any foundry-store SQL that filters on `key_prefix`, `slug` or `number` must also compare `workspace_id`, `team_id` or `project_id` with a bind parameter.
  - The scope must sit in the same OR-branch and the same parenthesis level as the key; a scope inside a subquery doesn't count.
  - It catches wrapped keys (`lower(slug)`, `slug::text`, `COLLATE`) and judges `format!`/`concat!` fragments per literal, failing closed.
  - Against the pre-fix tree it flags exactly the three Defect 1 queries.
- **Acceptance:**
  - us-11 has a scenario where Rita is refused a Backend attachment through her own Partners issue.
  - mwt-slice-04 has T11: Globex's project also uses prefix `ACME`. Marco's edit pre-fill, edit submit, web move and API move all get 404s byte-identical to the unknown-address responses, and Globex's issue is unchanged.
  - T11 was proven red against `06e7112`, as recorded in `9626a6a`.

Behaviour change: an edit or delete through an unknown team, project or issue now gets the uniform 404 page. Before, it got the "Comment not found" fragment (edit) or the slug-echoing team page (delete).

**Lanes on the final code:**
- **Acceptance:** default lane 1080/1080 at `88599ba`.
- **Crate tests:** xtask 126, foundry-app, foundry-services, foundry-api and foundry-store all green.
- **Static checks:** check-arch, fmt and clippy clean.
- **Smoke:** green at `5878aac`. The first run hit a connection error (SSLRequest 0x00) in an unrelated store test while the load average was about 30. That test passed alone, and the full rerun was green.

## Lessons

1. **A natural key is only as unique as its constraint.** `key_prefix` became tenant-local the day multi-workspace landed, but the queries kept treating it as global. Removing an instance-wide invariant needs a sweep of every query that relied on it. The new check-arch rule now does that sweep mechanically.
2. **Resolve, then use what you resolved.** Every defect had a caller that had already resolved the right parent (project or issue) and then dropped it, passing a weaker key downstream. Fetching a child by (child id, parent id) makes the URL's address binding.
3. **Hand-pins catch weak tests.** In 01-01, two regression tests stayed green under the reintroduced bug, because `fetch_optional` happened to pick the right one of two rows. A one-row-only fixture (GEN-2) made them sound.
4. **Mutation gates find missing positives.** The first run (68%) showed that no store test proved the new lookups return the row on the happy path, and no app test had a non-member reach a real team URL.

## Follow-ups

- **API 403 vs 404 (review M3):** on team-addressed routes, the JSON API answers 403 for an existing team the token's user isn't on and 404 for an unknown team, which reveals whether a team exists. The API's docs call this deliberate. Decide whether to make it uniform.
- **SSE refusal text (review L6):** `events.rs` answers 403 with a body that echoes the team slug. There is no existence oracle (unknown team, non-member and unknown project are identical), but it is not uniform with the 404 contract.
- **`undelete_comment` is id-only (review L5):** it is operator-CLI-only and documented as such. Scope it before exposing it to any tenant request.
- **Shared test harness:** five foundry-app test files each copy the testcontainers + `build_router` setup. A `tests/common/mod.rs` would remove the duplication.
- **project-lifecycle DELIVER step 02** points these same three issue queries at `live_projects`. They are now keyed by `project_id`; don't reintroduce `key_prefix`.
- **Release:** a v0.14.1 patch, only on the user's go. Two sign-in timing checks (us-06, keycloak-sso-provisioning) failed during 01-01 under a load average of about 100. They passed in every later full run.
