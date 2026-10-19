# Slice 02: The description covers every operation and every answer

Stories: US-AOD-03, US-AOD-04 | Estimate: 2-2.5 days | job_id: `job-api-machine-readable-contract`

## Goal

The document describes all of `/api/v1`: the 5 issue and comment operations, the
2 token operations, the list from slice 01, the uniform 404 catch-all and its own
route, each with every status it can answer and the body of each, as the code
answers today (D6, C1-C11). D13's "not yet documented" list is emptied and
removed, so any undocumented operation fails CI from now on.

## IN

- Create issue, change state, history, create comment, edit comment: `201` with `Location`, `200`, `403`, `410`, the 422 codes (`title_required`, `description_too_long`, `invalid_state`, `comment_empty`, `comment_too_long`), `401`, the uniform `404`, `500`.
- `state` as a lane slug with the D14 copy (C3); `description` optional (C8); `old` nullable in history.
- The text/plain 400, 415 and 422 extractor refusals and the empty 405, documented as they are (C4, C6; OQ-9).
- List tokens and revoke token: `200`, `204`, `403`, `404`, `429` `rate_limited`, `400` for a malformed `{jti}`, `500`; `TokenJson`'s exact 8 keys; no create-token operation; `POST …/tokens` documented as 405; the scheme says tokens are minted at `/admin/tokens` (D9).
- An acceptance check that triggers each deterministic documented answer for real and validates it against the document.
- An acceptance check that `/admin/tokens` answers a signed-in workspace admin.

## OUT

- Enveloping the extractor refusals (OQ-9 follow-up, unless the user folds it in: then +1 day and one story here).
- The UI page (slice 03).

## Learning Hypothesis

- **Disproves if it fails**: that the code's answers can be listed completely from code reading plus `status_for`. If the real-request check finds an answer nobody listed, the code-reading inventory in US-AOD-03's Technical Notes was wrong, and DESIGN's status-reachability rule must change.
- **Confirms if it succeeds**: that C1-C11 are the full list of prose-versus-code gaps for the existing operations.

## Acceptance Criteria

- [ ] All 9 operations and the catch-all are in the document; D13's list no longer exists.
- [ ] Each deterministic documented answer of each operation is produced by a real request and matches the document.
- [ ] No 409 is documented anywhere (no `/api/v1` use-case raises Conflict).
- [ ] The token schema has no value-bearing key, and there is no create-token operation.
- [ ] A new undocumented `/api/v1` operation fails CI.
- [ ] Dogfood: Sven-style, write one call (create an issue on a dev scratch project) from the document alone.

## Dependencies

- Slice 01 (the mechanism and the drift check).
- OQ-9 and OQ-11 answered.
- Reference class: issue-change-history's API story (one route, its shapes and refusals, about 1 day), times two.
