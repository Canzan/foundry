# Slice 04: A project named without Latin letters or digits gets a board you can open

Story: US-PNR-04 | Estimate: 0.5-1 day | job_id: `job-project-naming`

## Goal

When `slugify(name)` is empty (for example "日本語ボード" or "🚀"), create mints
the lower-cased key prefix as the slug, with the lowest free `-2`, `-3`… suffix
if the team already holds it. The redirect then lands on a board that opens,
and a second such project is not refused as a duplicate. Existing slugs are
never changed.

## IN

- One create-time slug mint (D7, D15; DESIGN places it, OQ-D4): use `slugify(name)` if it is non-empty, otherwise the key prefix lower-cased, plus a suffix chosen against the team's stored slugs.
- The mint runs after the name rule, the uniqueness check and the key checks (D5), so it always has a valid key.
- A refused create mints nothing (D9).
- Handling of a race on the fallback slug: never show the "must be unique" copy for a fallback slug (OQ-D4).

## OUT

- Changing `slugify`. Transliteration or Unicode slugs. Repairing existing `slug = ''` rows, and any migration (D16). A slug shape CHECK. Changing any slug on rename.

## Learning Hypothesis

- **Disproves if it fails**: that the key prefix is a good-enough URL for non-Latin names. If the dogfood create (a real board named in Japanese or with an emoji) leaves the operator wanting a different address, or if the fallback collides often in practice (more than an occasional `-2`), the D15 form is wrong and OQ-3 reopens before release.
- **Confirms if it succeeds**: that the slug-is-identity invariant holds with a second mint source. Every route (board, report, lanes, issues, `/api/v1`) works off the stored slug with no other change, so ADR-PROJECT-RENAME-001's request-path discipline already covers the fallback.

## Scenarios (from US-PNR-04)

1. "日本語ボード" with key "JP" lands on `/team/backend/project/jp`. The board and `/report` open.
2. "🚀" with key "RKT", next to it, lands on `/team/backend/project/rkt` and is not refused.
3. With "Ops" holding `ops`, "🛠" with key "OPS" lands on `/team/backend/project/ops-2`.
4. "Ωmega 2" with key "OMG" lands on `/team/backend/project/mega-2` (unchanged behaviour).
5. Renaming "日本語ボード" to "Japanese Board" keeps `/team/backend/project/jp`.

## Acceptance Criteria

- [ ] Scenarios 1-5 pass on the HTTP lane, following each redirect to a 200 board. One `@needs-browser` example covers scenario 1.
- [ ] Mint example pairs: non-empty slug used verbatim; empty gives the key; taken gives `-2`; `-2` taken too gives `-3`; the suffix starts at 2.
- [ ] No existing `projects.slug` value changes (universe check over all rows before and after).
- [ ] KPI-5: 0 rows with `slug = ''` created after the release.
- [ ] Dogfood: the count of legacy `slug = ''` projects on the operator's instance is recorded. If it is above zero, it goes to the user (D16). Priya creates one real non-Latin-named board, and it opens.

## Dependencies

Slices 02 (key validated before the mint) and 03 (the shared sibling read, and the empty-slug skip in the uniqueness check). **OQ-3** (the user confirms the key-prefix URL form) must be answered before DISTILL fixes the examples. Reference class: ADR-PROJECT-RENAME-001's slug work (`slugify` moved to core, about half a day).
