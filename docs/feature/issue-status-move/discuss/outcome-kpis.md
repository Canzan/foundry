# Outcome KPIs — issue-status-move

| KPI | Target | Measurement |
|-----|--------|-------------|
| Dialog status works | pick a status → save → card moves columns, persists | dogfood + acceptance (S slice-1) |
| Drag-and-drop works | drag a card to a column → lands there, persists | dogfood (gesture) + persist-contract acceptance |
| One persist path | 0 new state-write paths; both mechanisms hit change_issue_state | code review at finalize |
| Reverts on failure | a rejected/failed move returns the card to its origin | acceptance (AC-02.3) + dogfood |
| Progressive enhancement | no-JS board unchanged; dialog is the no-JS status path | acceptance (AC-01.6/AC-02.5) |
| No regressions | @all lane + xtask ci green | full CI |

**North-star**: a member keeps the board's columns accurate by dragging cards or picking a status in the
dialog — with one shipped persist path behind both.
