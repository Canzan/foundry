# Outcome KPIs — card-ranking-within-status

| KPI | Target | Measurement |
|-----|--------|-------------|
| Within-status reorder works | drag a card to an exact slot in its column → order persists across reload/viewers | dogfood (gesture) + persist-contract acceptance (AC-01.1/.2) |
| Cross-status positional drop works | drop a card into another column at an exact slot → state AND rank persist in one gesture | dogfood + acceptance (AC-02.1/.2/.3) |
| Deterministic ordered read | every render of a column produces the identical rank order (no flicker/ambiguity) | acceptance (AC-01.3) |
| Reverts on failure | a rejected/failed reorder or positional drop returns the card to its origin slot (and state) | acceptance (AC-01.4 / AC-02.4) + dogfood |
| Zero-shuffle migration | existing boards render in their prior order immediately after `0012` | acceptance (AC-01.6) + dogfood on the sandbox board |
| Tenancy/CSRF preserved | foreign-issue reorder → non-enumerable refusal, never a 500 | acceptance (AC-01.5 / AC-02.5) |
| Progressive enhancement | no-JS renders the ranked order; reorder is a JS-only enhancement | acceptance (AC-01.7 / AC-02.6) |
| No regressions | `@all` acceptance lane + `cargo xtask ci` green (incl. issue-status-move 6/6) | full CI |

**North-star**: a member arranges each status column top-to-bottom by what to do next — within a status or by
dropping into another — and that order sticks for the whole team, behind one persisted rank model.
