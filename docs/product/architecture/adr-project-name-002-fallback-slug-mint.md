# ADR-PROJECT-NAME-002: One project-slug mint with a key-prefix fallback, and a bounded retry when a concurrent create takes the fallback slug

- Status: Accepted (2026-10-06)
- Feature: `project-name-rule` (D6, D7, D15-D17, OQ-3; DESIGN DDD-9 to DDD-11)
- Amends: ADR-PROJECT-RENAME-001 ("minted exactly once at creation by `foundry_core::slugify`"). Slugs are still minted once and never re-derived; the mint is now a function that wraps `slugify`.

## Context

`foundry_core::slugify` keeps only ASCII letters and digits, so a name such as "日本語ボード"
or "🚀" mints the empty slug. `projects.slug` accepts `""`, but no route matches an empty path
segment, so the project is created and unreachable; a second such name collides on `""` and is
refused as a duplicate name. The user chose (OQ-1, OQ-3) to mint the lower-cased key prefix
instead, with the lowest free `-N` suffix from 2 on collision, and never to refuse because of a
fallback. The key prefix is validated (`^[A-Z]{2,6}$`) before the mint. The `UNIQUE (team_id, slug)`
index backstops the check-then-insert. Today its violation maps to
`ProjectInsertError::DuplicateName`, which the handler renders as "Project name must be unique
within the team" - wrong copy for a slug the user did not choose.

## Decision

1. **One pure mint in core.** `foundry_core::mint_project_slug(name: &ProjectName, key: &ProjectKey, team_slugs)`
   returns a minted slug tagged with its origin: `Derived(slugify(name))` when that is non-empty,
   else `KeyFallback(candidate)`, where the candidate is the first of `key.lower()`,
   `key.lower()-2`, `key.lower()-3`, … that is not among the team's stored slugs. It is total,
   never refuses and never returns `""`. `Store::seed_initial_workspace`'s constant `"sandbox"`
   is pinned by a test to equal the mint of ("Sandbox", "GEN").
2. **Called once, by `create_project`, after the name rule, the sibling check and the key parse**
   (D5), from the same sibling read the check used. A refused create mints nothing.
3. **Race on a fallback slug: retry, bounded.** If the INSERT hits the slug unique index and the
   slug is `KeyFallback`, the use-case re-reads the siblings, re-runs the sibling check (a real
   concurrent same-name create now gets the true uniqueness refusal), re-mints and re-inserts.
   At most 3 attempts in total; exhaustion is a logged 500 with no name in the log. A `Derived`
   slug's violation keeps today's meaning: `NotUnique`. Each attempt is its own transaction, so a
   failed one leaves no row (D9).
4. **Store error renamed for honesty.** `ProjectInsertError::DuplicateName` becomes `DuplicateSlug`
   (the index it reports is the slug's); the use-case decides what it means.
5. **No migration, no data rewrite.** Legacy `slug = ''` rows stay. The sibling check skips an
   empty derived slug, and the mint compares against stored slugs, so a legacy `""` neither
   blocks nor collides.

## Alternatives considered

- **Accept the race (the ADR-PROJECT-RENAME-002 posture).** Rejected: the losing request would
  show "Project name must be unique within the team" for a name that is unique, the one outcome
  OQ-D4 forbids, and the user cannot act on it.
- **Serialize per team** (`pg_advisory_xact_lock(team)` or `SELECT … FOR UPDATE` around read, mint
  and insert). Closes the race for both arms, but moves the mint's read into a store transaction
  (the store would call domain code or take a closure) for a window that needs two members of one
  team creating fallback-slug projects in the same milliseconds. Remains the designated fix if a
  future wave closes the name-arm race too.
- **Mint in SQL** (`INSERT … ON CONFLICT DO NOTHING` in a loop, or a suffix query). Puts identity
  rules in SQL beside the Rust `slugify`, giving two statements of the slug alphabet.
- **`project-N`, a UUID fragment, transliteration, Unicode slugs.** Rejected by the user at OQ-3.
- **Suffix on any collision, including name-derived slugs.** Rewrites the user-locked meaning of
  uniqueness (D6, D17).

## Consequences

- Positive: every new project has a non-empty, team-unique, reachable slug; slugs stay identity.
- Positive: the mint is pure and exhaustively example-tested (suffix from 2, lowest free, 6-letter key).
- Negative: a fallback slug can later refuse a name that slugifies to it (D17, accepted).
- Negative: a 500 is possible after three lost races. Not reachable at single-operator scale.
- Neutral: an old replica during a rolling deploy can still mint `""`; such rows are inert (point 5).
