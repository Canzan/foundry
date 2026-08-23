# Slice 02 — Rename a project's display name (happy path, URLs intact)

Story: US-IAPR-02 | Estimate: 1 day | job_id: `job-instance-project-rename`

## Goal

Each project row on the instance dashboard carries an htmx rename form; a
super-admin submits a new display name and the row swaps in place — while the
project's slug, board/report URLs, key prefix, and issue keys stay
byte-identical (D1).

## IN

- Rename form per `data-project-row`: text input pre-filled with the current
  name, hidden `_csrf` field (MANDATORY — htmx mutating trigger, D5), htmx POST
  to an instance-admin rename route keyed by project id.
- Rename write port: set `projects.name` only; `slug`/`key_prefix` untouched.
- Success: bare htmx fragment re-rendering the row with the new name (no
  `base.html` extension — double-wrap hazard).
- No-op rename (unchanged name) succeeds quietly (D4).
- Authz: `require_instance_admin` on the POST; failure = uniform 404 (D5).
- **D2 prerequisite:** correct `build_board_page`'s render-time
  `slugify(project.name)` / `slugify(team_name)` derivation so board issue-card
  edit/state URLs use the stored/request slugs — otherwise a rename breaks
  every card action on the board.

## OUT

- Validation beyond trim/no-op (empty, over-long, duplicate → slice 03).
- Slug regeneration, redirects, key_prefix changes, deletion (feature OUT).

## Learning Hypothesis

Display-name-only rename is genuinely non-destructive: the D2 correction is the
only place name→URL coupling hides. Slice 02's board-survival scenario proves
it or flushes out another derivation site.

## Acceptance Criteria

- [ ] Submitting "Identity Platform" on the "Auth v2" row swaps the row in
      place (no reload); a dashboard reload still shows the new name.
- [ ] `/team/backend/project/auth-v2` and its `/report` still serve, titled
      "Identity Platform"; AUTH-7 keeps its key; issue edit + state actions
      still work (D2 verified).
- [ ] Unchanged-name submit is a quiet success.
- [ ] Non-admin/signed-out rename POST → uniform 404, name untouched; POST
      without a valid `_csrf` pair is refused by the middleware.

## Dependencies

Slice 01 (row markup + project id in the listing read).
