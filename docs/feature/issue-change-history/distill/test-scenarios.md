# DISTILL Test Scenarios — issue-change-history

> SSOT: `crates/foundry-acceptance/tests/features/issue-change-history.feature`. All `@pending`; DELIVER un-@pends
> per slice. One model (`issue_change_events`, ADR-001) captured in-tx; three read surfaces (ADR-002). Genesis =
> start empty (UC-1). NET-NEW coverage = the record contract, the detail-page timeline, the `/api/v1` history
> JSON, and the project report + CSV.

## Config
- framework: cucumber-rs; glue in DELIVER at `steps/feature_issue_change_history.rs`. Reuse the shared Background
  (workspace/member/team seed, `a project "…" with issues:` data-table Given from card-ranking, `(\w+) is signed
  in`, `Mei fetches the "…" board`, `Mei saves the edit dialog for "…" with status "…"` from issue-status-move,
  `a foreign issue "…" exists in another workspace` from card-ranking, `Mei drops "…" after "…" in "…"` from
  card-ranking). Real Postgres (testcontainers) + reqwest + scraper. `@real-io`.
- HARNESS BOUNDARY: HTTP + store level. Automated: the in-tx record contract (`issue_change_events` reads), the
  detail-page timeline render (scrape), the `/api/v1/.../history` JSON, the report + CSV, tenancy/non-enumerability.
  Plain-language phrasing polish + any live refresh = dogfood (converge on reload; no live push, UC-1 lineage).
- NEW glue: `record` store reads (`… has N change events`, `field/old/new/actor` assertions, `no change event`);
  `Mei opens the detail page for "…"` (GET `/team/…/issues/{n}`); timeline scrape (`shows a "status" change to
  "…"`, `lists … above …`, `timeline is empty`, `card links to its detail page`, `card still carries its
  edit-dialog control`); `Mei edits "…" title to "…"[ and description to "…"]`; `a program requests the change
  history of "…"` (GET `/api/v1/…/history`) + JSON assertions; `Mei opens/exports the change report for project
  "…"` + CSV assertions.

## Catalog
| # | Scenario | Slice/AC | Drives |
|---|----------|----------|--------|
| S1 | Status change records one event in-tx | 01 / AC-01.1 | change status; `issue_change_events` has (status, backlog→todo, Mei) |
| S2 | Detail page renders timeline newest-first | 01 / AC-01.2 | two status changes; GET detail page; in_progress above todo, attributed |
| S3 | Unchanged issue → empty timeline (no created) | 01 / AC-01.3, UC-1 | GET detail page for a never-changed issue; timeline empty |
| S4 | Append-only | 01 / AC-01.4 | two changes → 2 events; earliest unchanged |
| S5 | No-op → no event | 01 / AC-01.6 | same-status save → no event |
| S6 | Foreign-issue timeline refused | 01 / AC-01.5 | detail page for a foreign issue → non-enumerable refusal |
| S7 | Board still opens modal + card links to detail (UC-2) | 01 | GET board; card has edit control + a detail-page link |
| S8 | Title edit records field=title (silent path now records) | 02 / AC-02.1/.2 | edit title; event (title, old→new) |
| S9 | Multi-field save = one event per changed field | 02 / AC-02.3 | title+desc save → 2 events; status none |
| S10 | Reorder records field=rank | 02 / AC-02.1 | drop reorder → event field=rank |
| S11 | History JSON, oldest-first, same events | 03 / AC-03.1/.4 | GET `/api/v1/…/history` → `[{actor,field,old,new,at}]` = stored events |
| S12 | History API refuses foreign issue | 03 / AC-03.2 | GET history for a foreign issue → API non-enumerable 404 |
| S13 | Project report lists + summarizes | 04 / AC-04.1/.2 | GET report; cross-issue list + status-flow + per-actor counts |
| S14 | Report CSV export | 04 / AC-04.3 | export → `text/csv` attachment, columns `issue,actor,field,old,new,at` |
| S15 | Report is workspace-scoped | 04 / AC-04.4 | report contains no foreign-workspace events |

## Walking-skeleton first RED
**S1** — a status change records one event to `issue_change_events` (the model + in-tx capture). Then S2 (detail
page timeline). Everything else reuses that model.

## Browser-dogfood checklist (not automated)
1. Change an issue's status → open its detail page → the timeline shows the attributed change.
2. Edit the title → the timeline gains a title change; reorder → a rank change.
3. Open the project report → the change table + summaries; Export → a CSV downloads.

## Reconciliation (DESIGN ADR-001/002, ODD-1..6; upstream UC-1/UC-2)
- ADR-001 dedicated table + in-tx capture → S1/S4/S5/S8/S9/S10 assert via `issue_change_events` reads; append-only
  (S4); no-op no event (S5); one row per changed field (S9).
- ODD-5 / UC-1 start-empty → S3 asserts an unchanged issue's timeline is EMPTY (no created entry).
- ADR-002 issue-detail page (UC-2) → S2/S7 assert the detail page renders the timeline AND the board still opens
  the quick-edit modal + the card links to the detail page.
- ADR-002 program feed → S11 (oldest-first, same events) + S12 (non-enumerable); report → S13/S14/S15.
- Non-enumerability (ADR-003 lineage) → S6 (web) + S12 (API) + S15 (report tenancy).
