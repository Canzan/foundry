# Data Models — board-lane-overflow-menu

## 1. Schema delta: NONE

**No migration. The counter stays at 0015.** This was the feature's one open
question (DISCUSS D8) and it was settled by running the operations against a
`postgres:16-alpine` container carrying a faithful 0015 reproduction — see
`architecture-design.md` §1 for the eight tests and their results.

The `lanes` table as shipped is sufficient:

```sql
CREATE TABLE lanes (
    id            UUID PRIMARY KEY,
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slug          TEXT NOT NULL CHECK (slug ~ '^[a-z][a-z0-9_]*$'),
    label         TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 64),
    position      INTEGER NOT NULL CHECK (position >= 0),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, slug),
    UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE   -- load-bearing
);
```

## 2. Which columns each operation touches

| Operation | `slug` | `label` | `position` | `issues.state` | `issue_change_events` |
|---|---|---|---|---|---|
| **Rename** (US-BLO-02) | — | **write** | — | — | — |
| **Insert** (US-BLO-03) | write (new row) | write (new row) | **write** (new row + shift) | — | — |
| Delete (shipped) | — | — | — | write (move fate) | write (move fate) |

Rename and insert write **zero** issue rows and **zero** change events. That is
AC-2.2 and AC-3.3, and it was verified in the spike, not assumed: after the
concurrent-insert test all 8 seeded issues were still in their original states.

## 3. The `DEFERRABLE` keyword is a load-bearing schema fact

`DEFERRABLE INITIALLY IMMEDIATE` does **not** mean "checked immediately, per
row". It means the constraint is checked **after each statement** rather than
after each row. That single distinction is what makes a mid-board insert possible
with no migration:

```sql
-- Against the shipped DEFERRABLE constraint: COMMITS.
UPDATE lanes SET position = position + 1 WHERE project_id = $1 AND position >= 1;

-- The identical statement against a non-deferrable UNIQUE(project_id, position):
-- ERROR: duplicate key value violates unique constraint
```

Both were run. This is why the keyword must never be dropped by a later
"cleanup" migration, and why `architecture-design.md` §6 recommends a
`check-arch` rule pinning it. See `adr-board-lane-003`.

## 4. Position invariants, before and after

Positions are **contiguous from 0** and **unique per project**. The shipped
migration seeds them contiguously; delete-with-fate preserves contiguity; insert
must too.

| Invariant | Enforced by |
|---|---|
| Unique per project | `UNIQUE (project_id, position)` — DB |
| Non-negative | `CHECK (position >= 0)` — DB |
| Contiguous from 0 | **Convention, not the DB.** Preserved by the shift-then-insert shape; asserted by acceptance oracles |
| Board order = `ORDER BY position` | `list_project_lanes` |

Contiguity has no DB constraint and never has; a gap would be invisible to
Postgres and merely cosmetic to the board (`ORDER BY position` still renders
correctly). The acceptance suite is the real guard, which is why AC-3.2 asserts
it explicitly rather than trusting the schema.

## 5. Lane slug minting

`lanes.slug` must satisfy `^[a-z][a-z0-9_]*$`. `foundry_core::slugify` emits
hyphens and therefore **cannot** mint lane slugs — an insert of `in-progress` is
rejected by `lanes_slug_check` (verified). New pure function
`foundry_core::lane_slug`:

| Label | Slug | Rule |
|---|---|---|
| `Staging` | `staging` | lowercase |
| `In Progress` | `in_progress` | non-alnum runs → one `_` (matches the shipped seed) |
| `Code Review!!` | `code_review` | trim trailing `_` |
| `2024 Review` | `lane_2024_review` | `^[a-z]` anchor → `lane_` prefix |
| `...` / `   ` | *(empty)* | → refuse inline (D7) |

Slug is minted **once**, at insert, and never re-derived — the names-are-labels
invariant, which `brief.md` §lanes already extends to lanes. A rename never
touches it.

## 6. Uniqueness: labels vs slugs

| | Unique? | Enforced |
|---|---|---|
| `label` within a project | **No** — duplicates allowed | Two lanes may both read "Doing" (verified) |
| `slug` within a project | **Yes** | `UNIQUE (project_id, slug)` |

So renaming a lane to an existing lane's label succeeds (AC-2.6), while
inserting a lane whose *minted slug* collides is refused (AC-3.6). The
collision is pre-checked **inside the insert's lock** so the operator sees the
D7 refusal copy rather than a raw `duplicate key value violates unique
constraint "lanes_project_id_slug_key"` — which is what surfaces otherwise.
