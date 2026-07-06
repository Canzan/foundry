# Acceptance Criteria (Given/When/Then) — card-ranking-within-status

Persist-contract ACs are acceptance-testable at the endpoint/store; the drag *gesture* itself is browser-
dogfooded (per `issue-status-move`: synthetic CDP mouse events don't fire native HTML5 DnD, so the handler is
exercised via genuine `dragstart`→`drop` DragEvents — the same path a human drag takes).

## US-01 — Reorder within a status

### AC-01.1 / AC-01.2 — drop at a position, and it persists
```
Given a Todo column showing GEN-4 above GEN-2
When I drag GEN-2 and drop it above GEN-4
Then GEN-2 is ordered before GEN-4 in Todo
And reloading the board (and any other viewer's board) shows GEN-2 above GEN-4
```

### AC-01.3 — deterministic ordered read
```
Given issues in a status with persisted ranks
When the board renders that column
Then cards appear in ascending rank order with a deterministic tiebreak
And two renders of the same data produce the identical order
```

### AC-01.4 — revert on failure
```
Given I drag a card to a new slot within its column
When the position write is rejected or the network fails
Then the card returns to its pre-drag slot
And no reorder is shown as succeeded
```

### AC-01.5 — tenancy / CSRF
```
Given a position write for an issue outside my acting workspace
When the write is attempted
Then it is refused uniformly (non-enumerable 404-class refusal), never a 500
And the CSRF double-submit token is required exactly as for change_issue_state
```

### AC-01.6 — migration backfill + new-issue default
```
Given a board that existed before this feature
When the 0012 migration runs
Then each issue is assigned an initial rank per (project, state) matching the prior number DESC order
And the board's first render after migration shows the same order as before (no visible shuffle)
And a newly created issue appears at its defined default slot in its column
```

### AC-01.7 — progressive enhancement (read honors rank)
```
Given JavaScript is disabled
When I view the board
Then columns render in persisted rank order
And no drag/reorder gesture is available (the order is read-only without JS)
```

## US-02 — Cross-status positional drop

### AC-02.1 / AC-02.2 / AC-02.3 — the GEN-3 scenario, atomic state+rank
```
Given GEN-3 is in Backlog, and Todo shows GEN-4 above GEN-2
When I drag GEN-3 and drop it between GEN-4 and GEN-2 in Todo
Then GEN-3's state becomes todo
And GEN-3 is ranked between GEN-4 and GEN-2 in Todo
And both the state change and the rank are persisted by the one gesture
And a reload (and other viewers) show GEN-3 in Todo between GEN-4 and GEN-2
```

### AC-02.4 — atomic revert
```
Given I drop a card into a different column at a specific slot
When the write is rejected or the network fails
Then the card returns to its origin column AND its origin slot
And neither the state nor the rank is shown as changed
```

### AC-02.5 — reuse state persist + tenancy/CSRF
```
Given a cross-status positional drop
When it persists
Then the state part is written through the shipped change_issue_state path
And tenancy/CSRF are preserved; a foreign issue is refused non-enumerably (no 500)
```

### AC-02.6 — progressive enhancement unchanged
```
Given JavaScript is disabled
When I want to change an issue's status
Then the edit dialog remains the no-JS status path (from issue-status-move)
And the resulting ranked order renders for everyone
```
