# RED classification — integrations-wayfinding

Run: 2026-11-03 (after the final review gate), Lima VM `foundry-test`,
`FOUNDRY_ACCEPTANCE_TAGS=integrations-wayfinding`, with `@pending` stripped from
the VM's copy of the feature only. Working tree at `e4f7e3f` plus the DISTILL
changes.

Result: 54 examples, 15 passed, 39 failed; 0 BROKEN. No Given failed. Where
each failure sits:
- 36 in an outcome assertion (`reads …`, `does not read …`, `is offered …`,
  the sidebar list).
- 3 in the When "Priya follows Integrations in the sidebar": the step looks
  for the entry W-1 adds and panics when there is none. That is the missing
  behaviour, not setup.
- A refused save answers 422 today, and the reads oracle reads that page's
  text, so those rows fail on the words, not on the status.

| Scenario (examples) | Result | Class | Observed |
|---|---|---|---|
| Sidebar follow (3) | RED | MISSING_FUNCTIONALITY | no Integrations in the sidebar (fails in the When) |
| Sidebar offered: Priya board, dashboard (2) | RED | MISSING_FUNCTIONALITY | sidebar lists Home, Board, Settings, Keyboard shortcuts |
| Sidebar not offered: Lena ×2, Sam ×2, Marco (5) | GREEN, vacuous | — | no one has the entry yet; discriminating once the Priya rows pass in the same outline |
| Sidebar order and current page (1) | RED | MISSING_FUNCTIONALITY | user section lacks Integrations |
| Form outline (8) | RED | MISSING_FUNCTIONALITY | each row reads K-4 instead of W-2, W-3 or W-4 |
| Admin never told to ask an admin (5) | RED | MISSING_FUNCTIONALITY | Priya reads K-4 ("Ask a workspace admin…", "Slack or Matrix") |
| K-4 for a lead (2) | GREEN | @control | unchanged |
| K-19 named first: replaced key, no key (2) | GREEN | @control | the form already answers K-19 |
| No Slack app over an unreadable connection (1) | RED | MISSING_FUNCTIONALITY | reads K-19 instead of W-2 |
| Preview, save, Send test (14) | 13 RED, 1 GREEN | MISSING_FUNCTIONALITY | Slack save answers 422 "Foundry cannot post to that channel"; Matrix preview and test render "New Slack route" (DESIGN D2). The green row is Lena saving with the Slack app set up: today's 422 page already contains K-4, which is also the right answer for her |
| Project page (6) | RED | MISSING_FUNCTIONALITY | no connection line (W-6 with "Canzan Personal", W-7, lost words, K-19, K-4) |
| No access, not-found (5) | GREEN | @control | uniform not-found for Sam and Marco |

DELIVER: un-pend story by story. Keep the vacuous sidebar rows in the outline
with the Priya rows so they become discriminating together.
