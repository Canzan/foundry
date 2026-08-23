# Data Models — instance-admin-project-rename

## 1. `projects` table as-is (`crates/foundry-store/migrations/0001_init.sql:51-62`)

```sql
CREATE TABLE projects (
    id                 UUID PRIMARY KEY,
    team_id            UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    workspace_id       UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,                       -- no CHECK, no UNIQUE
    slug               TEXT NOT NULL,
    key_prefix         TEXT NOT NULL CHECK (key_prefix ~ '^[A-Z]{2,6}$'),
    next_issue_number  INTEGER NOT NULL DEFAULT 1 CHECK (next_issue_number >= 1),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (team_id, slug),
    UNIQUE (workspace_id, key_prefix)
);
```

Rename touches exactly one column: `name`. `slug`, `key_prefix`,
`next_issue_number`, both UNIQUE constraints, and every issue row are untouched
(D1).

## 2. Migration verdict: **NO new migration** (migrations stay at 0014)

Decided, with justification (the expected outcome held under code reading):

1. `projects.name` already exists as `TEXT NOT NULL` with no constraint we must
   add — the write is a plain `UPDATE`.
2. D4's uniqueness rule is **not expressible as one DB constraint**: it is
   "case-insensitive name match against sibling *names* OR slugify(new name)
   collision against sibling *slugs*", and `slugify` is domain code
   (`foundry_core::slugify`), not SQL. A partial encoding (e.g.
   `UNIQUE (team_id, lower(name))`) was considered and rejected: it covers only
   half the rule, could fail to apply against pre-existing rows that already
   violate it (creates only ever checked slug collision — D7), and would turn a
   422-with-copy into a raw constraint error needing a second mapping path.
3. A `CHECK (length(name) BETWEEN 1 AND 256)` mirroring `issues.title` was
   considered and rejected for the same pre-existing-rows risk; the application
   enforces it at the only mutation points (create already rejects empty;
   rename now enforces both bounds). Slice-03 explicitly scopes DB CHECKs out.

Length semantics: Postgres `length(text)` counts characters; the Rust check is
`trimmed.chars().count() <= 256` — Unicode scalars, matching the
`issues.title` precedent.

## 3. Query set (all new, `foundry-store`; full signatures in component-boundaries.md)

| Query | SQL | Scope note |
|---|---|---|
| Listing | `SELECT p.workspace_id, p.id, p.name, p.key_prefix, t.name FROM projects p JOIN teams t ON p.team_id = t.id ORDER BY p.name` | Deliberately instance-wide (no WHERE): consumed only by the LAYER-1e allow-listed instance-admin surface; the `_for_instance` name makes the scope explicit |
| Rename context | `SELECT team_id, name, slug FROM projects WHERE id = $1` | By id — non-enumerable 404 on `None` |
| Siblings | `SELECT name, slug FROM projects WHERE team_id = $1 AND id <> $2` | Self excluded in SQL so app-side comparison needs no special-casing |
| Write | `UPDATE projects SET name = $2 WHERE id = $1` | `rows_affected == 0` ⇒ NotFound (project deleted mid-flight) |

## 4. Uniqueness check-then-write: TOCTOU consideration

The duplicate check (siblings read + app-side compare) and the `UPDATE` are two
statements with no lock between them. The races and their worst cases:

| Race | Outcome | Assessment |
|---|---|---|
| Two concurrent renames in one team to the same name | Two projects share a display name | Cosmetic only: identity is `id`/`slug`/`key_prefix`, all untouched. This end state is **already reachable and accepted** via D7 (the create path checks slug collision only), so the race adds no state the system does not already tolerate. Next rename of either project surfaces the 422. |
| Rename concurrent with create of a colliding name | Same as above | Same assessment; additionally `UNIQUE (team_id, slug)` still hard-blocks any *slug* collision on the create side |
| Rename concurrent with project delete | `rows_affected == 0` | Mapped to the uniform 404 — no partial state |

Accepted at homelab scale (single-digit operators, slice-03 OUT item), and —
per the Earned Trust posture — accepted *by name*, with the blast radius bounded
above, not by silence. If a future wave wants the race closed, the recorded
option is `SELECT … FOR UPDATE` on the team's project rows inside one
transaction (a store-internal change; no port signature would move). No
serializable-isolation or advisory-lock machinery is warranted now.

## 5. Read-surface propagation (feature-delta Shared Artifacts)

`projects.name` consumers all read the column per-render — no cache, no
denormalized copy exists anywhere (verified: board and report both fetch via
`find_project_by_slug`; the dashboard row via the new listing read). A committed
rename is therefore visible on the next render of every surface with no
invalidation step. The one *derived* consumer — `build_board_page`'s render-time
`slugify(name)` — is removed by the D2 fix rather than propagated to.
