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
