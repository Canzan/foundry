# Outcome KPIs — issue-change-history

| KPI | Target | Measurement |
|-----|--------|-------------|
| Every change recorded | 100% of tracked-field mutations (status/title/desc/rank) write exactly one in-tx event | acceptance (AC-01.1, AC-02.1/.3) + store review |
| No phantom / no drop | a rolled-back mutation records NO event; a committed one ALWAYS records | acceptance (in-tx) + a rollback test |
| Human timeline works | open an issue → attributed, plain-language, newest-first change timeline that persists | dogfood + acceptance (AC-01.2/.3) |
| Program feed works | `GET …/issues/{n}/history` → JSON `{actor,field,old,new,at}`, same events as the timeline | acceptance (AC-03.1/.4) |
| Report + export work | project report lists + summarizes changes; CSV export with a stable contract | acceptance (AC-04.1/.2/.3) |
| One model, three surfaces | 0 second sources of truth — human/API/report all read the same stored events | code review at finalize (AC-03.4) |
| Append-only integrity | 0 code paths edit or delete a history entry | code review + acceptance (AC-01.4) |
| Tenancy preserved | foreign/absent issue → uniform non-enumerable refusal on every surface, never a 500 | acceptance (AC-01.5, AC-03.2, AC-04.4) |
| No realtime regression | issue-status-move + card-ranking + `@all` lane stay green | full CI |

**North-star**: every change to an issue becomes a durable, attributable record that a member can read, a program
can consume, and a lead can report — all from one model, with nothing silently lost.
