# Acceptance Criteria (Given/When/Then) — issue-change-history

Persist-contract + API + report ACs are acceptance-testable at the HTTP/store boundary (cucumber-rs, real
Postgres). The human timeline's *rendering* is HTTP-assertable (the server-rendered fragment); any live-refresh
polish is dogfood.

## US-01 — Status history timeline

### AC-01.1 / AC-01.3 — a status change is recorded and persists
```
Given issue GEN-1 is in Backlog
When Mei changes its status to Todo
Then a change event is stored: actor=Mei, field=status, old=backlog, new=todo, with a timestamp
And re-reading the issue (and any other viewer, on reload) shows that event on the timeline
```

### AC-01.2 — the timeline renders in plain language, newest first
```
Given GEN-1 has status events backlog→todo then todo→in_progress
When Mei opens the issue
Then the timeline shows the in_progress change above the todo change
And each entry names the actor, the transition (e.g. "Todo → In Progress"), and a relative time
```

### AC-01.4 — append-only
```
Given GEN-1 already has a recorded status change
When its status changes again
Then a NEW event is appended
And the earlier event is unchanged (never edited or deleted)
```

### AC-01.5 — tenancy / non-enumerable
```
Given an issue outside my acting workspace
When I request its timeline
Then it is refused uniformly (non-enumerable 404-class), never a 500
```

### AC-01.6 — no event for a no-op
```
Given GEN-1 is in Todo
When a save submits the SAME status (Todo)
Then no status change event is recorded
```

## US-02 — All editable fields

### AC-02.1 / AC-02.2 — title/description/rank recorded
```
Given GEN-1 with title "Login bug"
When Mei changes the title to "Login 500 on submit"
Then a change event is stored: field=title, old="Login bug", new="Login 500 on submit"
And it appears on the timeline (the update-details path now records, where it emitted nothing before)
```

### AC-02.3 — one event per changed field
```
Given GEN-1
When a single save changes BOTH the title and the description
Then two change events are stored (one field=title, one field=description)
And a field that did not change records no event
```

## US-03 — Program JSON feed

### AC-03.1 / AC-03.4 — JSON history over /api/v1, same events
```
Given GEN-1 has recorded status + title changes
When a program GETs /api/v1/.../issues/1/history with valid auth
Then it receives a JSON array of {actor, field, old, new, at} ordered oldest→newest
And those are the SAME stored events the human timeline renders
```

### AC-03.2 — API auth + non-enumerable
```
Given a request for the history of an issue outside the caller's workspace
When the API handles it
Then it returns the API's uniform non-enumerable refusal, never a 500
```

## US-04 — Project change report

### AC-04.1 / AC-04.2 — aggregated report
```
Given a project whose issues have accumulated change events
When a lead opens the project change report
Then it lists change events across the project (issue key, actor, field, old→new, when), most recent first
And it summarizes status-flow transition counts and per-actor change counts
```

### AC-04.3 — CSV export
```
Given the project change report
When the lead exports it
Then a CSV with a stable column contract is produced from the same stored events
```

### AC-04.4 — tenancy
```
Given two workspaces with issues
When a lead in workspace A opens the report
Then only workspace A's change events appear (no cross-tenant leakage)
```
