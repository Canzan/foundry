# board-lane-overflow-menu — evolution archive

**Shipped**: 2026-09-03 (DISCUSS → DESIGN → DISTILL → DELIVER, one session)
**Job**: `job-board-lane-shaping` (extended, not duplicated)
**Predecessor**: `board-lane-management`, whose **D9** pre-registered this feature
**Commit status**: NOT committed — the change sits in the working tree

## What shipped

The board's per-column armed `×` is gone. Each column header now carries one
unobtrusive `⋯` menu with four items: **Edit list · Insert list before · Insert
list after · Delete list**. Delete reaches the shipped dialog unchanged; Edit
renames a lane's label; Insert places a new working lane exactly where asked,
making lane deletion no longer one-way.

Archive is deliberately not a duplicate of `docs/feature/board-lane-overflow-menu/feature-delta.md`
(995 lines, all four waves). This records what a future reader needs.

## The four things worth remembering

### 1. The user's original ask was for "Archive list". It was declined, on evidence.

The reference screenshot showed Trello's Archive-list. Reading the code first
showed foundry has **no archive concept at all** — every `archiv*` hit under
`crates/` is backup-tarball code. Adding one would have meant a new column, an
un-archive surface, and a second way for a card to become invisible: the exact
failure `board-lane-management`'s D1(b) had just removed. The user was offered
the choice with that context and chose Delete.

### 2. `DEFERRABLE INITIALLY IMMEDIATE` means *checked per statement*, not per row.

DISCUSS ranked the position shuffle as the feature's only real uncertainty and
assumed a `SET CONSTRAINTS ... DEFERRED` window would be needed, with migration
0016 as the fallback. DESIGN **ran it** against a disposable `postgres:16-alpine`
(the tag production pins) and found the naive bulk `UPDATE` already commits.

Three consequences, all still binding:

- **No migration. The counter stayed at 0015.**
- **The `DEFERRABLE` keyword on `0015:22` is load-bearing.** The identical
  statement against a non-deferrable constraint fails with `duplicate key`. A
  future tidy-up migration dropping it breaks Insert **while every existing test
  stays green** — nothing else in the codebase shifts positions.
- **Concurrency needed a lock nobody had specified.** Unguarded concurrent
  inserts hand the loser a raw Postgres error. `FOR UPDATE` plus an
  identity-resolved anchor fixes it; both commit.

A `check-arch` rule pinning that keyword was recommended and **is not yet
implemented**. Until it is, `adr-board-lane-003` is the only guard.

### 3. Making lane slugs mintable silently broke every write path to a new lane.

`normalize_state` folded incoming states against a **closed five-slug set**,
correctly, for as long as lanes could not be created — its own doc said
"unmintable", citing D9. Insert falsified that premise. Until it was fixed, an
inserted lane **rendered a column that dnd, the edit dialog and `/api/v1` all
refused**.

Caught by an acceptance scenario that inserts a lane and then puts a card in it.
Not by review. The lesson is the one `board-lane-management` D1 already taught:
an inherited assumption is only as good as the premise it was written under, and
the premise is not always in the same file.

Mutation testing then showed three of the five alias arms had become dead code
under the fix. They were removed, leaving only the two aliases whose spelling can
never be a legal lane slug: `in-progress` (hyphens are forbidden by the CHECK)
and `canceled`.

### 4. The menu is an *arm*, not a component.

`keyboard.js` holds exactly one document `keydown` and one document `click`
listener, and `Escape` has exactly one owner (BR-4). The menu adds neither: it is
the third arm of `closeTopLayer()`, and its open state is **derived from the
DOM** rather than stored — because `#board-columns` is replaced wholesale by the
out-of-band refresh, and a stored handle would be left detached, turning `Escape`
into a silent no-op with a menu on screen. A browser scenario pins exactly that.

If a future reviewer counts more than one of either listener, `adr-board-lane-005`
has been violated.

## Numbers

| | |
|---|---|
| Scenarios (this feature) | 23/23 |
| Full acceptance suite | 68 features, 590 scenarios, 4141 steps — all green |
| Workspace unit/integration | 252 passing |
| Mutation, per-feature gate ≥80% | **100%** (`foundry-core` 11/11, `foundry-services` 12/12) |
| Migrations added | **0** (stays at 0015) |
| Diff | 20 files modified, 8 added; ~1,490 insertions |

## Artifacts

- `docs/feature/board-lane-overflow-menu/feature-delta.md` — all four waves
- `docs/feature/board-lane-overflow-menu/design/` — C4, component boundaries, data models, stack
- `docs/feature/board-lane-overflow-menu/slices/` — three slice briefs
- `docs/product/architecture/adr-board-lane-003-deferrable-position-shuffle.md`
- `docs/product/architecture/adr-board-lane-004-lane-slug-mint.md`
- `docs/product/architecture/adr-board-lane-005-overflow-menu-as-layer-arm.md`
- SSOT updated: `jobs.yaml` (job widened + `scope_history`), `architecture/brief.md`
  (two sections extended), `outcomes/registry.yaml` (OUT-6/7/8),
  `architecture/atdd-infrastructure-policy.md` (one driven-internal row)

## Successors

1. **Reorder lanes** — the menu is the natural home; 03-02's shuffle machinery is what it needs.
2. **The `check-arch` `DEFERRABLE` rule** — carried, unimplemented.
3. **Undo a lane delete** — Insert makes recovery manual; real undo is still absent.
4. **Sort by** — the screenshot's last item; needs a card-ordering concept the board does not have.
