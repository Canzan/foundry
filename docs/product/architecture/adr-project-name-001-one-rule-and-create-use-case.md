# ADR-PROJECT-NAME-001: Both project-name doors parse through one `foundry-core` value object that shares the workspace rule's character predicate, and create becomes a `foundry-services` use-case

- Status: Accepted (2026-10-06)
- Feature: `project-name-rule` (D2-D6, D9-D14; DESIGN DDD-1 to DDD-8, DDD-12)
- Amends: ADR-WORKSPACE-NAME-001 ("Project and lane copy keep the convention"; the D4 predicate moves out of `workspace_name.rs`), ADR-PROJECT-RENAME-002 decision 1 (the handler no longer owns the project-name copy)

## Context

Two user doors write `projects.name`: the team-member create form
(`POST /team/{team_slug}/projects`, a handler calling `Store::insert_project` directly)
and the instance-admin rename (`foundry_services::projects::rename_project`). The doors
disagree. Rename trims, refuses empty, more than 256 scalars, and a duplicate (case-insensitive
name OR derived slug equal to a sibling's stored slug). Create trims, refuses empty, and
checks only the slug. Neither refuses control characters, so a NUL is a 500 on both. The
refusal copy is hand-kept in two adapter files. The feature requires one rule, byte-identical
copy, the workspace rule's exact control-character set (with no possibility of drift), and
one meaning of "unique within the team". The precedent (ADR-WORKSPACE-NAME-001) put the
workspace rule in a `foundry-core` value object whose error `Display` is the copy.

## Decision

1. **Shared predicate.** The refused-character predicate (Cc, U+202A-202E, U+2066-2069,
   U+2028, U+2029) moves from `workspace_name.rs` to one crate-private function in a neutral
   `foundry-core` module. `WorkspaceName` and `ProjectName` both call it. A core property test
   asserts the two types give the same control verdict for every scalar.
2. **Value object.** `foundry_core::ProjectName::try_new(raw)` trims, then refuses empty, then
   control, then more than `PROJECT_NAME_MAX_CHARS` (256) scalars. Private field, `as_str()`.
3. **One uniqueness definition.** A pure `ProjectName` method over the team's sibling
   `(name, slug)` pairs: case-insensitive name equality, or a **non-empty** derived slug equal
   to a sibling's stored slug. The derived slug has one statement (`ProjectName`'s own
   `slugify` call), shared with the slug mint (ADR-PROJECT-NAME-002).
4. **Copy.** `ProjectNameError { Empty, ControlCharacter, TooLong, NotUnique }` is a flat
   `Copy` enum whose `Display` carries the four strings. `try_new` never returns `NotUnique`;
   the sibling check returns only `NotUnique`. Doors render `to_string()`.
5. **Rename composes it.** `classify_rename`: trim, byte-equal no-op, `try_new`, sibling check.
   `RenameProjectError::{EmptyName, NameTooLong, DuplicateName}` become `InvalidName(ProjectNameError)`.
   The store reads stay before the classifier (an unknown id must answer 404 before any 422,
   and the no-op needs the current name).
6. **Create is a use-case.** `foundry_services::projects::create_project` (and
   `Services::create_project`) takes a handler-parsed `ProjectName`, the team and workspace ids
   the handler resolved behind its unchanged authz gate, and the raw key prefix. It reads the
   siblings (`list_team_sibling_projects`, excluding the fresh project id), runs the sibling
   check, parses the key, mints the slug, and inserts. The handler keeps D10's gates
   byte-identical and owns only HTTP shapes. No in-seam membership re-check: create is not a
   cross-tenant write, which was ADR-PROJECT-RENAME-002's reason for its re-check.
7. **Enforcement.** A `cargo xtask check-arch` rule, `project-name-one-source`: (a) the literal
   `"Project name must` appears in no `.rs` under
   `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src`; (b) `insert_project(`
   has no call site under `crates/{foundry-app,foundry-api}/src`. Each clause has an
   injected-violation gold test. The compile-time layer is the `ProjectName` parameter on
   `create_project`.
8. **Store unchanged.** Signatures stay `&str`; no DB CHECK; no migration.

## Alternatives considered

- **Thin create handler calling the core rule and a core mint, no service.** Viable, and it
  adds no type. Rejected: the create flow is now read, check, parse, mint, insert-with-retry.
  The retry loop is logic, and in a handler it is testable only through HTTP; the mint point
  would also sit in an adapter, where nothing stops a second door from inserting with its own
  slug. ADR-PROJECT-RENAME-002 named `foundry_services::projects` the seam for future project
  mutations.
- **A `classify_create` free function in `foundry-services` returning `String`.** Rejected for
  the same reason as ADR-WORKSPACE-NAME-001's equivalent: an unchecked `String` is
  indistinguishable from a checked one.
- **Two error types (three pure arms, plus a separate "taken" type).** More precise for
  `try_new`, but splits the copy over two `Display` impls and forces every handler to keep two
  arms. Rejected: D2 defines uniqueness as part of the rule, so one refusal enum matches the
  domain.
- **Duplicate the predicate in `project_name.rs`, guarded by a test.** Rejected: one definition
  is cheaper than a test that detects a second.
- **Re-check team membership inside the use-case.** One more query per create for a risk the
  handler's byte-pinned gate already closes; the provisioning and rename re-checks exist for
  cross-tenant writes.

## Consequences

- Positive: one rule, one predicate, one uniqueness definition, one copy source; KPI-2 parity
  holds by construction. NUL becomes a 422 on both doors.
- Positive: the pure parts sit in an I/O-free crate and are mutation-tested at exact boundaries.
- Negative: `foundry-core` holds a second set of user-facing strings. Accepted, as for workspaces.
- Negative: create's code path moves crates (handler shrinks, service grows) across slices 02-04.
- Neutral: the guarantee is "every production door", not "every row" (no DB CHECK, D12).

## Amendment (2026-10-09, name-db-checks)

Item 8 and the neutral consequence above are superseded for the pure arms (ADR-NAME-DB-001).
Migration 0020 installs `projects_name_rule_on_insert` and `projects_name_rule_on_rename`
(`UPDATE OF name`, only when the name changes) over 0019's shared functions at cap 256. A
write that breaks a pure arm is refused with SQLSTATE 23514 and the arm as the constraint
(`projects_name_<arm>`). The guarantee becomes "every door and every new name write". Still
not in the database: uniqueness within the team and the slug, which stay with the app.
Store signatures stay `&str`, and rows stored before 0020 keep their names and keep taking
issues, because the trigger never fires on a write that leaves the name unchanged.
