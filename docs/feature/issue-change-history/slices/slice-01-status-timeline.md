# Slice 01 — Status-change timeline (walking skeleton)

**Goal**: every status change writes a durable, attributed change event, and the issue shows a plain-language,
newest-first timeline of them.
**Story**: US-01.

**IN scope**
- The durable change-event **model** — `actor · field · old → new · timestamp`, append-only, immutable
  (DESIGN ODD-1: enrich the outbox vs a dedicated `issue_events` table; likely a new table + a migration).
- **In-tx capture at the status write paths** — `update_issue_state_with_outbox` + `reposition_issue_with_outbox`
  record `field=status, old, new` in the SAME transaction as the state write (no phantom, no drop); a same-value
  save records nothing.
- The **human timeline** render — an attributed, plain-language, newest-first list on the issue (ODD-3: edit
  dialog vs a new issue-detail page).
- Genesis story for pre-existing issues (ODD-5).
- Acceptance: the record contract (store) + the rendered timeline (HTTP fragment) + tenancy non-enumerability; the
  live-refresh polish is dogfood.

**OUT of scope**: title/description/rank capture (slice 02); the program JSON feed (03); the report (04); adding
priority/assignee editing; live push to open viewers; retention.

**Learning hypothesis**: disproves "a durable per-field change-event model, captured in-tx at the state write
path, rendered as a plain-language timeline, is a clean increment" if the model, the in-tx capture, the genesis
story, or the timeline's home needs machinery we lack.

**Seams**: `update_issue_state_with_outbox` (`lib.rs:1287`) + `reposition_issue_with_outbox` (`lib.rs:1364`); the
outbox precedent (`0003`); the issue-edit-dialog render / a new issue view; tenancy `resolve_member_project`.
**Dependencies**: DESIGN ODD-1/2/3/5. **Effort**: ~1–1.5 days (the model + migration + capture + render carry the
feature's uncertainty).
