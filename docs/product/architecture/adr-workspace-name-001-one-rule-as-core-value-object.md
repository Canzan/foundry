# ADR-WORKSPACE-NAME-001: Every workspace-name door parses through one `foundry-core` value object, which also carries the refusal copy

- Status: Accepted (2026-10-05)
- Feature: `instance-workspace-name-rule` (D1-D5, D11; DESIGN DDD-1 to DDD-7, DDD-12)
- Amends: ADR-WORKSPACE-RENAME-001 decision 1 ("the handler owns the copy"), for workspace names only

## Context

Four production doors set `workspaces.name`: dashboard provisioning (P1), `foundry doctor
provision-workspace` (P2), the bootstrap claim (P3), and the instance-admin rename (P4).
Only P4 had a rule (`classify_workspace_rename` in `foundry-services`: trim, then no-op,
then empty, then at most 24 scalars), and only the rename handler held its copy. The
feature requires one rule and byte-identical copy on all four doors, plus a new arm that
refuses control characters (Cc, the bidi embedding, override and isolate controls, and
U+2028/2029). The doors span three driving adapters in `foundry-app` (instance-admin
HTTP, bootstrap HTTP, CLI) and two media (HTML and stderr). P3 calls the store directly,
without a service. `foundry-core` already holds the house's pure naming rules
(`ProjectKey::try_new` with a flat error enum, `slugify`, `lane_slug`). Every crate on a
write path depends on it. There is no DB CHECK on `workspaces.name` (follow-up D), and
fixtures must still be able to seed legacy over-length and tab-containing names.

## Decision

1. **The rule is a value object.** `foundry_core::WorkspaceName::try_new(raw)` trims, then
   refuses in order: empty, then any control character from the locked set, then more than
   `WORKSPACE_NAME_MAX_CHARS` (24) Unicode scalars. It returns the trimmed name in a type
   with a private field. `WorkspaceNameError { Empty, ControlCharacter, TooLong }` is a flat
   `Copy` enum. The cap constant moves to core. `foundry_services::workspaces` re-exports it.
2. **The copy is the error's `Display`.** The three user-facing strings live once, on
   `WorkspaceNameError`. Every door renders `err.to_string()` and never matches a variant
   to choose copy.
3. **The rename composes the rule.** `classify_workspace_rename` keeps its byte-equal
   no-op first, so a legacy name can be left untouched, then calls `try_new`.
   `RenameWorkspaceError::InvalidName(WorkspaceNameError)` replaces `EmptyName` and
   `NameTooLong`.
4. **Defence in depth by type.** `provisioning::ProvisionRequest.workspace_name` is a
   `WorkspaceName`. The use-case cannot receive an unchecked name, so it neither
   re-validates nor gains a `ServiceError` arm.
5. **The store does not validate.** Store signatures stay `&str`. `Store::create_initial_workspace`
   has no production caller. It stays as the fixture seeder and is documented as such.
6. **Enforcement.** A `cargo xtask check-arch` rule, `workspace-name-one-source`, fails the
   build if the copy literal `"Workspace name must` appears under
   `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src`. It also fails if
   `create_initial_workspace(` is called under `crates/{foundry-app,foundry-services,foundry-api}/src`.
   Each clause carries an injected-violation gold test. The typed provisioning port is the
   compile-time layer.

## Alternatives considered

- **A free function in `foundry-services::workspaces`, returning `String`.** Rejected. The
  returned `String` is indistinguishable from an unchecked one, so any later caller can skip
  the check without the compiler noticing. It also leaves a domain invariant in the
  use-case seam, where nothing below that seam can be typed by it.
- **A helper module in `foundry-app`.** All four doors live there, so this would work
  today. Rejected because it puts the rule in an adapter crate, and adapter-held rules
  are the drift this feature removes. A future `/api/v1` door would have to reach into
  `foundry-app`.
- **Per-handler copy (the precedent's convention).** Rejected for workspace names. With
  three adapter modules and two media, per-handler copy means three hand-kept copies, and
  KPI-2 measures their byte identity. Project and lane copy keep the convention.
- **Store methods take `&WorkspaceName`.** Rejected. It ripples through about 30 test call
  sites, and it blocks the legacy-name fixtures that DoD 8 needs. Persistence-level
  enforcement is follow-up D's job (a `CHECK … NOT VALID`).
- **Delete, or feature-gate, `create_initial_workspace`.** Deleting it means churn across 15
  fixture call sites for no user value. A `test-support` gate is unreliable under cargo
  feature unification in `--workspace` builds. The check-arch clause gives the same
  guarantee for production code.

## Consequences

- Positive: one rule and one copy. Adding an arm later reaches all four doors with no
  adapter edit. An unchecked name cannot reach the provisioning use-case. Copy drift
  and a production bypass through the fixture seeder both fail the build.
- Positive: the rule is pure and sits in an I/O-free crate, so it can be proptested and
  mutation-tested at exact boundaries without HTTP or a database.
- Negative: user-facing copy now lives in `foundry-core`, which until now held only
  developer-facing error strings. This is accepted for this one rule.
- Negative: `ProvisionRequest` construction changes at every call site, both production
  callers and tests, in the slice that lands it.
- Neutral: the store remains able to write any name. Until follow-up D, the guarantee is
  "every production door", not "every row".

## Amendment (2026-10-09, name-db-checks)

Follow-up D shipped as triggers, not a `CHECK … NOT VALID` (ADR-NAME-DB-001). Migration 0019
installs `workspaces_name_rule_on_insert` and `workspaces_name_rule_on_rename` (`UPDATE OF
name`, only when the name changes) over the shared verdict function
`foundry_name_rule_violation`. A write that breaks a pure arm is refused with SQLSTATE 23514
and the arm as the constraint (`workspaces_name_<arm>`). The guarantee becomes "every door and
every new name write". Rows stored before 0019 are not scanned and keep their names. Decisions
4 and 5 stand: the value object is still the user-facing rule and the store still takes `&str`;
fixtures that need a legacy name use the test-support seam `seed_row_predating_name_rule`.
