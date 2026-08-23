# ADR-PROJECT-RENAME-001 — Board slugs come from the request path, never re-derived from names

- Status: Accepted (2026-08-22)
- Feature: `instance-admin-project-rename` (D2 prerequisite of US-IAPR-02)

## Context

`projects.slug` is the URL identity of a project: boards and reports resolve via
`find_project_by_slug(team_id, slug)`, and `UNIQUE (team_id, slug)` pins it. Yet
`crates/foundry-app/src/projects.rs::build_board_page` (lines 861–862) computes
`team_slug = slugify(team_name)` and `project_slug = slugify(&project.name)` **at
render time** and bakes them into every issue card's edit/state URL. That was
invisible while `name` and `slug` could never diverge — no rename path existed.
This feature creates one (display-name-only rename, D1), so a naive rename would
break every card action on the board while the board URL itself still resolved.

Traced inventory of every production `slugify(` call site: the two lines above
(the defect); `projects.rs:170` (create path — mints the slug **once**, the
intended derivation point); `admin_tokens.rs:284` (normalizes user-typed team
input into a lookup key — input normalization, not stored-name re-derivation).
The report/CSV path already builds `board_url`, `csv_url`, and the CSV filename
from the `Path`-provided slugs and is correct. The ~20 test-local copies in
`foundry-acceptance` derive URLs from names those tests seeded (slug ==
slugify(name) by construction) and are unaffected.

## Decision

1. `build_board_page` (and its caller `render_board`) take `team_slug` and
   `project_slug` as parameters; `show_board` passes the request-path values.
   The two derivation lines are deleted. This is provably equivalent to reading
   the stored columns: the handler found the project **by** that slug
   (`WHERE slug = $2`), so request slug and stored slug are byte-equal.
2. `slugify` moves to `foundry-core` as the single production definition
   (`pub fn slugify`); `projects.rs` (create path) and `admin_tokens.rs` call
   it instead of keeping private copies.
3. `cargo xtask check-arch` gains a rule: any `fn slugify(` **definition** under
   `crates/foundry-app/src` fails the build. Using `foundry_core::slugify` is
   fine; growing a new private derivation — the regression class behind this
   defect — is not. This mirrors the repo's existing posture that invariants
   live in build-time scanners, not conventions (cf. `check_jwt_alg_pin`).

## Alternatives

**Add `slug` to `foundry_store::ProjectRow` and read the stored column in
`build_board_page`.** Functionally identical output. Rejected because it widens
a store struct and its query for data the handler already holds validated in
scope — a larger diff for zero additional correctness. It also leaves the
`team_slug` half unsolved (there is no `TeamRow` in that path), forcing a second
plumbing change anyway.

**Regenerate the slug on rename so name and derived slug stay convergent.**
Rejected outright: it converts a display-label edit into a URL migration —
breaking bookmarks, requiring redirects, and contradicting locked decision D1
("slug, URLs, key_prefix, issue keys immutable"). Recorded as a possible future
"change project URL" feature, which would be a different job.

## Consequences

- Positive: renames become genuinely non-destructive; the invariant "slugs are
  minted once at creation and never derived again" is machine-enforced; two
  duplicated `slugify` copies collapse into one domain function with its unit
  tests moved to `foundry-core`.
- Positive: `build_board_page` stays a pure, unit-testable view-model builder;
  its tests gain the divergence case (name "Identity Platform", slug "auth-v2"
  ⇒ card URLs contain "auth-v2").
- Negative: two render-path signatures widen by two parameters, and existing
  `build_board_page` unit tests must pass slugs explicitly (mechanical update).
- Negative: the new check-arch rule adds one more thing `cargo xtask ci`
  scans — accepted, that is the point.
