# ADR-BOARD-LANE-004: Lane slugs are minted by a lane-specific function, not `slugify`

## Status

Accepted (board-lane-overflow-menu DESIGN wave, 2026-09-02)

## Context

Inserting a lane is the first path in this codebase that mints a **lane** slug at
runtime. Until now every lane slug came from the 0015 grandfather seed or the
three-lane creation seed — all literals.

`lanes.slug` is constrained `CHECK (slug ~ '^[a-z][a-z0-9_]*$')`: lowercase,
underscore-or-alnum, **letter-anchored**. The shipped lane is `in_progress`.

`foundry_core::slugify` exists and is the house slug minter for projects — but it
emits **hyphens** (`slugify("Auth v2") == "auth-v2"`, `foundry-core/src/lib.rs:300`).
Verified against the real schema:

```
ERROR:  new row for relation "lanes" violates check constraint "lanes_slug_check"
DETAIL:  Failing row contains (…, in-progress, Hyphen, 99).
```

So reusing `slugify` is not a stylistic preference — it produces values the
schema rejects.

## Decision

**Add `foundry_core::lane_slug`, a sibling of `slugify`, as the sole minter of
lane slugs.**

- lowercase; runs of non-alphanumerics collapse to a single `_`; leading and trailing `_` trimmed
- if the result is **digit-leading**, prefix `lane_` (the `^[a-z]` anchor)
- if the result is **empty**, return empty → the caller refuses inline (D7)
- minted **once**, at insert; never re-derived (names-are-labels, extended to lanes by `brief.md` §lanes)

| Label | Slug |
|---|---|
| `Staging` | `staging` |
| `In Progress` | `in_progress` |
| `Code Review!!` | `code_review` |
| `2024 Review` | `lane_2024_review` |
| `...` / `   ` | *(empty → refused)* |

It lives in `foundry-core`, not `foundry-app`: slug minting is a pure domain
function, and `fn slugify(` under `crates/foundry-app/src` is a `check-arch`
build failure by standing rule.

### The digit-leading prefix is a disclosed deviation from D7

D7 says collisions are refused inline, never auto-suffixed — because a
`done_2` slug drifts from its label permanently. The `lane_` prefix is a
different kind of act: it is *normalisation to satisfy a CHECK*, the label is
preserved verbatim, and the slug is never surfaced to the operator (it appears
only in `data-column`, `issues.state` and API payloads). Refusing a legitimate
name like "2024 Review" would be user-hostile; relaxing the CHECK would cost a
migration to save something nobody sees.

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| Reuse `foundry_core::slugify` | Emits hyphens; the schema rejects the output. Measured, not assumed. |
| Teach `slugify` a separator parameter | Would change a function 48 features depend on, to serve one caller, and puts project-slug behaviour one argument away from lane-slug behaviour. |
| Migration 0016 relaxing the CHECK to `^[a-z0-9][a-z0-9_]*$` | Removes the need for the prefix, but spends a forward-only migration on live data to avoid a prefix the operator never sees. |
| Opaque slugs (`lane_a1b2c3`) | Always valid and collision-free, but `data-column="lane_a1b2c3"` is hostile to every future debugging session, and `issues.state` becomes unreadable in the DB. |
| Let the operator type the slug | Exposes identity to someone who has no reason to care about it, and creates a second thing to validate and refuse. |
| Refuse digit-leading names outright | "2024 Review" and "1:1s" are legitimate lane names. |

## Consequences

- `lane_slug` is property-testable beside the existing `slugify` properties: a
  fixed point, and output that always either satisfies `^[a-z][a-z0-9_]*$` or is empty.
- Two slug minters now exist. The naming (`slugify` for URL identity, `lane_slug`
  for lane identity) must stay obvious, or a future contributor will reach for
  the wrong one — and the CHECK will catch them at runtime, not compile time.
- Slug collisions remain a **refusal**, pre-checked inside the insert's lock so
  the operator never sees the raw `duplicate key value violates unique constraint
  "lanes_project_id_slug_key"` that surfaces otherwise.
