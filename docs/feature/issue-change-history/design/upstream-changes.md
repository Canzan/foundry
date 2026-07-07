# Upstream Changes — issue-change-history (DESIGN → DISCUSS)

Two clarifications for DISTILL. No scope reduction; both were open ODDs in DISCUSS.

## UC-1 — Genesis resolved to "start empty" (no created event in v1)

**Original (DISCUSS `requirements.md`, ODD-5)**: "issues created before this feature have no prior history; the
timeline starts from 'created' (backfill a single genesis entry, or start empty)."

**Change**: user chose **start empty** (2026-07-07). v1 records field *changes* only — no backfill and **no
'created' event even for new issues**. An issue with no field changes shows an **empty timeline**.

**Impact on acceptance (for DISTILL)**: write US-01 scenarios so an unchanged issue's timeline (and its
`/history` JSON) is **empty**, and the first recorded entry is the first field *change* (not a creation). Do not
assert any genesis/created entry. `old_value` remains nullable in the schema for a future creation-event kind,
but nothing writes it in v1.

## UC-2 — Timeline home is a new issue-detail page (ODD-3)

**Original (DISCUSS `requirements.md`, ODD-3)**: timeline home was open — "the edit dialog vs a new issue-detail
view."

**Change**: resolved to a **new issue-detail page** `/team/{t}/project/{p}/issues/{n}`; the board card gains a
link to it and the existing click→quick-edit modal is preserved.

**Impact on acceptance (for DISTILL)**: US-01's human-surface scenarios assert the **detail page** renders the
timeline (a server-rendered GET), and that the board still opens the quick-edit modal (regression). No new DISCUSS
story is added — this is the chosen realization of US-01's "issue view."

**Story/slice scope**: unchanged (US-01…US-04, slices 01–04). **DISCOVER impact**: none.
