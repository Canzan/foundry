# Slice 04 — Upstream Issues (real oracles surfaced by the adversarial matrix)

Per the slice-04 DISTILL RED-state contract: a matrix cell that reds for a REAL
oracle (a 403, a body echo, or a body-shape diff that confirms a foreign resource
exists) is flagged here, then closed test-first to the uniform non-enumerable 404
(ADR-003 / NFR-MWT-SEC-02).

## ISSUE-04-01 — web attachment-download leaked an existence oracle (RESOLVED in step 04-02)

**Surface:** `GET /team/{t}/project/{p}/issues/{n}/attachments/{id}`
(`crates/foundry-app/src/attachments.rs::download_attachment`).

**Scenario:** "A foreign web attachment-download and a never-existed download are
indistinguishable" (slice-04 feature scenario #7).

**Oracle (right-reason RED, observed against real testcontainers PG16):**
The two cross-tenant refusal bodies were NOT byte-identical — a body-shape oracle
plus an id echo, both 404 (no 403 was involved):

- Foreign reach (Globex team slug `platform`, scoped to Acme) died at the team
  layer and rendered the `team_not_found_page` — `<title>Team not found</title>`
  with body `No team with slug "platform" exists in this workspace.` — **echoing
  the foreign team slug** `platform`.
- Never-existed reach reached the attachment layer and rendered
  `not_found_page` — `<title>Not found</title>` with body
  `Attachment <uuid> not found in this workspace` — **echoing the attachment
  UUID**.

Either body lets an attacker distinguish "exists but elsewhere" from "never
existed" (and the team-slug echo is itself a foreign identifier). This was the
ONE web read surface not migrated to the canonical `resource_not_found_page`
idiom that `show_board` / `submit_create` / the comment + project read paths
already use — even though `bootstrap::resource_not_found_page`'s own doc comment
claims to "generalise the shipped `find_attachment_in_workspace → None → 404`
idiom".

**Fix (test-first, step 04-02):** in `download_attachment`, collapse the two
CROSS-tenant refusal paths to the single uniform `resource_not_found_page()`:
- foreign/missing team → `resource_not_found_page()` (was `team_not_found_page`)
- foreign/missing attachment → `resource_not_found_page()` (was `not_found_page`)

The intra-workspace membership branch keeps its shipped **403** `non_member_page`
(ADR-003 boundary clause — a member reaching their OWN workspace's team is not a
cross-tenant concern; a cross-tenant reach 404s at the team layer above and never
reaches it). The now-dead `not_found_page` helper + its `InvalidPage` import were
removed.

**Result:** foreign attachment download is now byte-identical to a never-existed
download (uniform 404, no slug/UUID echo). Scenario #7 GREEN; revert-reds-it
confirmed (reverting `attachments.rs` re-reds the scenario on the body-equality
assertion).

## ISSUE-04-02 — three web WRITE surfaces leaked the SAME existence oracle (RESOLVED in step 04-03)

**Surfaces (all three shared one root cause):**
- `POST /team/{t}/project/{p}/issues/{n}/comments` (`comments.rs::submit_comment`
  → `resolve_comment_not_found_page`)
- `POST /team/{t}/project/{p}/issues/{n}/state` (`issues.rs::submit_state_change`
  → `resolve_not_found_page`)
- `POST /team/{t}/project/{p}/issues/{n}/attachments` (`attachments.rs::submit_upload`)

**Scenarios:** slice-04 feature scenarios #4 (web comment), #5 (web state-change),
#6 (web attachment-upload). Scenario #3 (web file-issue, `submit_create`) was
ALREADY clean — it routes `ServiceError::NotFound → resource_not_found_page()` —
and stayed GREEN by inheritance, which localised the defect to the other three.

**Oracle (right-reason RED, observed against real testcontainers PG16):**
Same family as ISSUE-04-01 — a cross-tenant write died at the team layer and
rendered the slug-ECHOING `team_not_found_page` (`<title>Team not found</title>`,
body `No team with slug "platform" exists in this workspace.`), whereas the
never-existed comparator (the actor's OWN workspace, missing issue) rendered a
DIFFERENT page — `issue_not_found_page` (comment/upload) or `project_not_found_page`
(state-change), each echoing the actor's slugs. Two oracles in one: (a) a
body-SHAPE difference (Team-not-found vs Issue/Project-not-found) distinguishing
"exists elsewhere" from "never existed", and (b) the foreign team slug `platform`
echoed verbatim. All 404 (no 403 on the comment/state paths).

These three were the web WRITE surfaces never migrated to the canonical
`resource_not_found_page()` idiom that `submit_create` / `show_board` / the read
paths (and, post-04-02, `download_attachment`) already use.

**Masking step-def bug (fixed):** the slice-04 `web_upload` helper sent the CSRF
token in the urlencoded `_csrf` FORM field, but `csrf::csrf_middleware` requires
multipart uploads to carry it in the `x-csrf-token` HEADER (the form field is not
parsed for multipart bodies). The foreign upload therefore 403'd at the CSRF layer
BEFORE reaching `submit_upload`, masking the real refusal surface. Fixed the helper
to set the `x-csrf-token` header (matching the canonical US-11 upload client),
which exposed the genuine slug-echo oracle above.

**Fix (test-first, step 04-03):** collapse each handler's CROSS-tenant /
missing-resource refusal branches to the single uniform `resource_not_found_page()`:
- `resolve_comment_not_found_page` → returns `resource_not_found_page()` (was
  `team_not_found_page` / `issue_not_found_page`).
- `resolve_not_found_page` (state-change) → returns `resource_not_found_page()`
  (was `team_not_found_page` / `project_not_found_page`).
- `submit_upload` → team-None, issue-None, and `IssueNotFound` race fallback all
  return `resource_not_found_page()` (were the slug-echoing pages).

The intra-workspace membership failures keep their shipped **403** `non_member_page`
(ADR-003 boundary clause — a cross-tenant reach 404s at the team layer above and
never reaches the 403 branch). The now-dead `team_not_found_page` /
`project_not_found_page` (issues.rs), `issue_not_found_page` (comments.rs +
attachments.rs), and `team_not_found_page` (attachments.rs) helpers were removed.
The comment edit/delete paths' intra-workspace `team_not_found_page` (comments.rs)
is unaffected and retained.

**Result:** the three foreign web writes are now byte-identical to their
never-existed comparators (uniform 404, no slug echo, no shape diff), and no row
is created / no state mutated in Globex. Scenarios #3–#6 GREEN; revert-reds-it
confirmed (pre-fix, #4/#5/#6 red on body-shape inequality and the upload on the
masked-then-real refusal; #3 stayed green, isolating the defect). Full
default-lane suite green (255 scenarios / 2120 steps).
