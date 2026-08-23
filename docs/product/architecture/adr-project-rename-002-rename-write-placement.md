# ADR-PROJECT-RENAME-002 — The rename write is a foundry-services use-case; uniqueness is checked in the application, with no new migration

- Status: Accepted (2026-08-22)
- Feature: `instance-admin-project-rename` (D4, D5, D7; slices 02–03)

## Context

The instance-admin surface has two shipped write idioms: `submit_provision`
drives `Services::provision_workspace` — a `foundry-services` use-case that
**re-checks `is_instance_admin` inside** (defence-in-depth) — while
`submit_grant` calls the store directly, leaning on the session gate alone plus
an idempotent `INSERT … ON CONFLICT DO NOTHING`. The new rename is a
cross-tenant mutation (an instance admin editing any workspace's project) with
non-trivial validation: trimmed non-empty, ≤256 chars, and team-scoped
uniqueness defined as *case-insensitive name match OR slugify(new name) equals a
sibling's stored slug* (D4). `slugify` is domain code
(`foundry_core::slugify`, ADR-PROJECT-RENAME-001), so the uniqueness rule cannot
be evaluated in SQL. `projects.name` is `TEXT NOT NULL` with no CHECK and no
name-uniqueness constraint; the create path enforces uniqueness via slug
collision only (accepted residual D7). Migrations stand at 0014.

## Decision

1. **Placement**: a new `foundry_services::projects::rename_project` use-case
   owns authz re-check (`is_instance_admin`, fail-closed — the
   `provision_workspace` idiom, chosen because rename shares its risk class:
   a cross-tenant state change), trim/no-op detection, all D4 validation, and
   the write. It returns typed outcomes (`Renamed`/`NoOp`) and typed errors
   (`Forbidden`/`NotFound`/`EmptyName`/`NameTooLong`/`DuplicateName`); the
   handler owns the mapping to user-facing copy (422 fragments) and to the
   uniform non-enumerable 404.
2. **Uniqueness**: check-then-write in the application — fetch the team's
   sibling `(name, slug)` pairs (self excluded), compare app-side, then
   `UPDATE projects SET name = $2 WHERE id = $1`. The TOCTOU window is accepted
   and bounded: the worst outcome is a duplicate *display label*, a state D7
   already tolerates from the create path; identity (`slug`, `key_prefix`,
   URLs, issue keys) cannot be corrupted by the race.
3. **No migration**: no new CHECK, no functional unique index, no schema change
   of any kind. Migrations remain at 0014.

## Alternatives

**Handler-level validation with direct store calls (the `submit_grant` /
create-path idiom).** Fewer moving parts and honestly viable at this scale.
Rejected because (a) rename is a cross-tenant write, where the repo's precedent
(`provision_workspace`) deliberately pays for a second, in-seam authz check; and
(b) the validation is a pure function of `(new_name, current, siblings)` that a
service seam makes unit-testable without HTTP, sessions, or askama — directly
serving the feature's top quality attributes (correctness/testability) and the
≥80% mutation gate.

**Encode uniqueness/length in the database** — `UNIQUE (team_id, lower(name))`
plus `CHECK (length(name) BETWEEN 1 AND 256)`. Rejected: the index covers only
half of D4 (the slug-collision half needs `slugify`, which is Rust, not SQL);
both constraints can fail to apply against pre-existing rows the create path
legally produced (D7); and a constraint violation surfaces as a raw DB error
needing a second error-mapping path beside the 422 copy the UX contract pins.
Slice-03 explicitly scopes DB CHECKs out.

**Serialize the check and write** (`SELECT … FOR UPDATE` in one transaction, or
an advisory lock per team). Correctly closes the race; rejected *now* because
the race's worst case is an already-accepted cosmetic state on a
single-operator instance — the machinery is all cost. Recorded as the designated
fix if a future wave closes D7 properly; it is store-internal and would move no
port signature.

## Consequences

- Positive: validation precedence (empty → length → duplicate) and the no-op
  rule are pinned in one unit-testable function; the handler shrinks to
  parse → gate → delegate → render; defence-in-depth matches the surface's
  strongest shipped precedent.
- Positive: DELIVER runs no migration; rollback is a plain code revert.
- Negative: `foundry-services` grows a `projects` module for one use-case —
  accepted as the designated seam for any future project mutations (archive,
  transfer) rather than a one-off.
- Negative: duplicate display names remain reachable under concurrency (and via
  the untouched create path, D7). Deliberately accepted and documented in
  `data-models.md` §4 rather than half-fixed.
