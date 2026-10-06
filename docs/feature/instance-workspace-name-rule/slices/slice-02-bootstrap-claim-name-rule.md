# Slice 02: The first-run claim refuses an unfit name without burning the link

Story: US-WNR-02 | Estimate: 0.5-1 day | job_id: `job-instance-workspace-naming`

## Goal

`POST /bootstrap?token=…` applies the shared rule to the workspace name for a
live link, before the claim transaction. On refusal, the claim page comes back
with status 422, the copy, and the entered fields (not the password). The link
stays claimable.

## IN

- The order is: liveness (non-consuming, as `GET` already does), then the name rule, then the existing atomic claim. A dead link answers byte-identically to today, whatever the name (D8).
- The claim page re-rendered with an error line, email, display name and workspace name retained, and the password empty.
- Examples: 33 characters, blank, newline, trimmed, a dead link with a bad name, and the corrected retry on the same link.

## OUT

- Display-name validation. Any change to the dead-link refusal page. Moving bootstrap behind a service, unless DESIGN chooses to.

## Learning Hypothesis

- **Disproves if it fails**: that a name rule can sit in front of the one-time claim without creating a new token-state oracle. If the dead-link answer must differ by name, or the liveness pre-check races the claim in a way a scenario can observe, the "check before claim" design is wrong and the rule has to move inside the transaction.
- **Confirms if it succeeds**: that a refused claim leaves zero rows and a live link, so the first operator is never stranded by a typo.

## Acceptance Criteria

- [ ] "Raman Household Operations Center" gets 422 with the length copy, the fields retained, no password echoed, zero rows created, and the link still live.
- [ ] The same link with "Raman Household" signs in at `/dashboard`, and the sidebar reads "Raman Household".
- [ ] "   " and "Raman\nHousehold" get the matching copy.
- [ ] "  Raman Household  " is stored trimmed.
- [ ] A used link with a 33-character name gets a byte-identical refusal page.
- [ ] Dogfood: on a throwaway instance from the operator's compose stack, mint a link, claim with a 33-character name, then claim with the same link and a valid name.

## Dependencies

Slice 01 (the shared rule and copy source). Reference class: the
bootstrap-claim-enumeration-oracle feature (byte-identical refusal discipline).
