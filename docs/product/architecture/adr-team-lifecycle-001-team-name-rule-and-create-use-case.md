# ADR-TEAM-LIFECYCLE-001: One team-name value object on both doors, and one create use-case that mints the slug and writes the team with its first lead

- Status: Accepted (2026-10-11)
- Feature: `team-lifecycle` (DISCUSS D3, D6, D7, D16, OQ-2, OQ-3, OQ-7, INV-TL-4, INV-TL-7; DESIGN DDD-2 to DDD-6, DDD-13)
- Follows: ADR-WORKSPACE-NAME-001, ADR-PROJECT-NAME-001, ADR-PROJECT-NAME-002, ADR-NAME-DB-001

## Context

The only production writer of `teams` is `seed_initial_workspace` (bootstrap's "General"). Every
other team was written in psql, with no name rule. The feature adds two user doors that set
`teams.name`: create (`POST /teams`, workspace admin) and rename (`POST /team/{slug}/rename`,
`TeamAccess` Manage). DISCUSS requires one rule on both (D6): trim, then refuse empty, then the
shared refused-character set, then more than 64 scalars (OQ-2); "unique within the workspace" means
a case-insensitive name match or a non-empty derived slug equal to a sibling's stored slug. The slug
is minted once and is never empty (D7, OQ-3: `team`, `team-2`, …). The creator is the team's first
lead, in the same commit as the team (D3, INV-TL-7). Concurrent same-name creates give exactly one
team (INV-TL-4). The database must refuse a bad name from any caller, on new writes only (OQ-7).

The house has solved each part once already: `WorkspaceName` and `ProjectName` (a `foundry-core`
value object whose error `Display` is the copy, sharing `name_chars::is_refused_name_char`),
`ProjectName::ensure_unique_among` (the uniqueness definition), `mint_project_slug` (lowest free
`-N` suffix from 2), `create_project`'s bounded retry, and 0019's `foundry_name_rule_violation` /
`foundry_enforce_name_rule()`.

## Decision

1. **`foundry_core::TeamName`** (new module `team_name.rs`, re-exported from `lib.rs`):
   `try_new(raw)` trims, then refuses `Empty`, `ControlCharacter` (through the shared
   `is_refused_name_char`, never a copy), `TooLong` (more than `TEAM_NAME_MAX_CHARS = 64` scalars).
   `TeamNameError { Empty, ControlCharacter, TooLong, NotUnique }` is a flat `Copy` enum whose
   `Display` is the D12 copy. `ensure_unique_among(siblings: &[(name, stored slug)])` is the
   project rule's definition. The comparison is extracted into one crate-private helper that
   `ProjectName` and `TeamName` both call, so the two definitions cannot drift. The whole object
   lands in slice 01. Slice 03 proves the remaining arms.
2. **One mint, `foundry_core::mint_team_slug(name, workspace_slugs)`.** It returns
   `slugify(name)` tagged as derived when that is non-empty. Otherwise it returns the first of
   `team`, `team-2`, `team-3`, … that is not among the workspace's stored team slugs, tagged as
   fallback. It is total and never returns `""`. The lowest-free-suffix search is the project
   mint's, extracted into one crate-private helper. Whether the tag reuses `MintedSlug` (with
   `KeyFallback` renamed to `Fallback`) or a sibling type is the crafter's choice.
3. **One create use-case, `foundry_services::teams::create_team(store, principal, raw_name)`.** It is
   the only place a team slug is minted (check-arch, DDD-19). Order:
   1. `is_workspace_admin` (else `NotFound`, the uniform 404);
   2. `TeamName::try_new` (else `InvalidName`, 422);
   3. per attempt: read the workspace's `(name, slug)` siblings, run `ensure_unique_among`, mint,
      then call the store.
4. **The store writes the team born led, in one transaction:
   `Store::create_team_with_lead(workspace_id, creator_id, name, slug)`.**
   1. `SELECT 1 FROM workspace_memberships WHERE (ws, creator) AND role = 'admin' FOR KEY SHARE`.
      No row returns `NotAdmin`, which the use-case maps to `NotFound`. This is the
      `add_team_member` idiom (team-membership DDD-10). A concurrent workspace removal, which locks
      that row `FOR UPDATE`, either waits for the create or makes it refuse. No team is ever led by
      someone outside its workspace.
   2. `INSERT INTO teams` with an app-minted UUID v7.
   3. `INSERT INTO team_memberships (team, creator, 'lead')`.
   4. Commit.

   A 23505 on `teams_workspace_id_slug_key` returns `DuplicateSlug`. Nothing else is written, so
   no reader sees a team without a lead (INV-TL-7). The 0021 guard fires only on DELETE and UPDATE,
   so it never meets the create.
5. **The race (INV-TL-4) follows ADR-PROJECT-NAME-002 decision 3.**
   - `DuplicateSlug` on a derived slug is `NotUnique`. The unique index makes concurrent same-name
     Latin creates give one team.
   - `DuplicateSlug` on a fallback slug is retried: re-read, re-check, re-mint, re-insert. There
     are at most 3 attempts in total, and each is its own transaction. A concurrent create of the
     same non-Latin name is caught by the re-check and gets `NotUnique`. Concurrent creates of
     different non-Latin names both succeed, with distinct `team-N` slugs.
   - Exhaustion is a logged 500 with no name in the log.
6. **Rename composes the same rule.** `classify_team_rename(raw, current, siblings_without_self)`
   runs: trim, then byte-equal no-op (D15), then `try_new`, then `ensure_unique_among`. The slug is
   never re-derived (D4). The write is in ADR-TEAM-LIFECYCLE-003.
7. **Database rule (OQ-7), migration `0024_team_name_rule.sql` (slice 03):**
   - `teams_name_rule_on_insert` (`BEFORE INSERT`) and `teams_name_rule_on_rename` (`BEFORE UPDATE
     OF name … WHEN (OLD.name IS DISTINCT FROM NEW.name)`), both `EXECUTE FUNCTION
     foundry_enforce_name_rule('64')`. 0019's verdict function is reused unchanged.
   - `CREATE OR REPLACE FUNCTION foundry_enforce_name_rule()` adds one `WHEN 'teams' THEN
     'TeamName'` arm to its HINT `CASE`. Otherwise the HINT would name `foundry_core::teams`. The
     body is otherwise byte-identical.
   - A `DO` self-check runs at cap 64: 64 and 65 characters in ASCII, U+00E9, U+65E5 and astral,
     plus padded and empty.
   - It is re-runnable (`DROP TRIGGER IF EXISTS`, then `CREATE`), with `COMMENT`s.
   - Legacy rows are never scanned. Refusals are 23514 with `teams_name_<arm>`.
8. **Enforcement.**
   - check-arch `team-name-one-source`: the literal `"Team name must` appears in no `.rs` under
     `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src`.
   - check-arch `team-lifecycle-one-seam` (see DDD-19): `INSERT INTO`, `UPDATE` and `DELETE FROM
     teams` appear only in `foundry-store` src. `create_team_with_lead(` is not called from
     `foundry-app` or `foundry-api`.
   - The compile-time layer is the `TeamName` parameter on the store-facing use-case path.

## Alternatives considered

- **Reuse `ProjectName` with a cap parameter.** Rejected: the copy differs ("Team name must…",
  "…within the workspace"), and a parameterised type makes the copy a runtime argument that a door
  can get wrong. Three small value objects over two shared helpers keep one predicate, one
  uniqueness definition and one copy source per rule.
- **Serialise creates per workspace** (lock the workspace row, then read, mint and insert in one
  transaction). This closes every name race, including the rare three-way fallback race and
  rename-versus-create. Rejected for the same reason as ADR-PROJECT-NAME-002: the store would have to
  run the domain rule inside its transaction (a closure, or domain code in the store), for windows
  that need two admins acting within milliseconds. It remains the designated fix.
- **A functional unique index `UNIQUE (workspace_id, lower(name))`.** Rejected: it covers half of D6
  (the slug arm needs `slugify`), it may fail to build against legacy psql duplicates, and its 23505
  would need a second mapping. The slug index already backstops the derived arm.
- **Refuse an empty slugify result in slice 01 and defer the fallback.** DISCUSS allowed it.
  Rejected: "must not be empty" is wrong copy for a name the user typed. Landing the mint in slice 01
  costs one pure function.
- **Seed a "General" team on provisioning (OQ-5).** Out of scope by the user's decision.

## Consequences

- Positive: three name rules share one predicate, one uniqueness definition, one DB verdict function
  and one trigger function. A fourth is mechanical (slice 03's hypothesis).
- Positive: a new team is usable at once. The creator can create a project on it, because project
  create is gated on being on the team.
- Negative: a fallback slug can later refuse a Latin name that slugifies to it ("Team" after
  "インフラ" took `team`). This is accepted, as for projects (DISCUSS slice 03's learning
  hypothesis watches for it).
- Negative: rename-versus-create, and a three-way fallback race, can leave two teams with
  case-insensitively equal names. This is cosmetic: slugs and URLs cannot collide (the unique index).
  It is accepted under INV-TL-5, as ADR-PROJECT-RENAME-002 accepted it.
- Negative: 0024 redefines a shared function. Its parity tests (`name_rule_in_database.rs`) must
  run over all three tables.
- Earned Trust (store gold tests on `postgres:16`):
  - Two concurrent creates of "Infra" give one team and one `NotUnique`. Two concurrent creates of
    "インフラ" give one team and one `NotUnique`. "インフラ" racing "運用" gives `team` and `team-2`.
    Each runs at least 100 times with zero 500s.
  - A create racing the creator's workspace removal never leaves a lead outside the workspace.
  - A U+0001, U+202E or 65-character `teams.name` written in psql gets 23514 with the named arm.
    A NUL never reaches the trigger: Postgres refuses U+0000 in `text` with 22021 (ADR-NAME-DB-001).
  - An untouched legacy name survives an UPDATE of another column.
