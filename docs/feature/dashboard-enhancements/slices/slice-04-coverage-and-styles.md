# Slice 04 — Coverage + style promotion

**Goal**: backfill test coverage for the (previously untested) base dashboard, and promote the inline
styles into the vendored stylesheet.

**Stories**: US-05 (retroactive base coverage) + US-04 (styles promote, `@refactor`).

**IN scope**
- Store test: `list_projects_for_workspace` scoping (isolation), ordering, empty case (AC-05.1/.2).
- Acceptance scenario: signed-in dashboard lists project(s) + links to board (AC-05.3).
- Move `dashboard_root.html` inline `<style>` → `static/css/foundry.<newhash>.css`; bump `base.html` hash;
  delete old file (AC-04.1–.4).

**OUT of scope**
- A design-token system / restyling other pages. New dashboard behaviour.

**Learning hypothesis**: disproves "the base dashboard is faithfully coverable and styles promote with no
visual drift" if the acceptance scenario surfaces a latent bug or the hash bump breaks asset caching.

**Acceptance**: `acceptance-criteria.md` US-04 + US-05.

**Seams**: acceptance harness (`crates/foundry-acceptance/`); store test harness; `static/css/`,
`base.html:5`, ServeDir (`lib.rs:265`).

**Slice composition**: pairs `@refactor` + test-debt with a user-observable acceptance scenario (dashboard
end-to-end) — not infrastructure-only ✓.

**Dependencies**: last — the acceptance scenario exercises the fully-assembled dashboard (slices 01–03).
**Effort**: ~1 day.
