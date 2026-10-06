# Slice 03: Dashboard provisioning refuses an unfit name and creates nothing

Story: US-WNR-03 | Estimate: 0.5-1 day | job_id: `job-instance-workspace-naming`

## Goal

`POST /admin/instance/workspaces` refuses a name that fails the shared rule,
with status 422 and the copy. It creates no workspace, user, membership or
invite, and keeps the first-admin email for the retry. The use-case
`provisioning::provision_workspace` gains typed name refusals that slice 04
reuses.

## IN

- The shared rule applied in the provisioning use-case, after its `is_instance_admin` re-check. The handler keeps the uniform 404 before the rule (D8).
- The 422 response shape (OQ-D2: re-render the dashboard with the error at the form, or a fragment). Either way the copy is visible on the real page and the email is retained.
- Examples: 24 accepted, 32 refused, blank, U+202E, the corrected retry with the same email, and Marco with a 40-character name.

## OUT

- Converting the form to htmx and redesigning the success page. The CLI door (slice 04).

## Learning Hypothesis

- **Disproves if it fails**: that the plain-POST Provision form can explain a refusal legibly without new interaction machinery. If the operator loses her typed email or has to navigate back, the form needs the htmx/error-slot idiom, and that would grow this slice.
- **Confirms if it succeeds**: that the dashboard no longer contradicts itself, because the rename row and the Provision form refuse the same names in the same words.

## Acceptance Criteria

- [ ] "Canzan Labs Platform Ops" (24) is provisioned. "Canzan Labs Platform Engineering" (32) gets 422 with the length copy and zero new rows.
- [ ] "   " and "Globex\u{202E}" get the matching copy.
- [ ] The retry with "Canzan Platform Eng" and "dana@canzan.net" succeeds.
- [ ] Marco's 40-character provision gets a byte-identical uniform 404.
- [ ] One `@needs-browser` example shows the copy on the real dashboard page.
- [ ] Dogfood: Priya provisions a real workspace on her instance after a deliberate over-long first attempt.

## Dependencies

Slice 01. Reference class: web-provisioning-flow 01-02 (dashboard form plus
handler, about 1 day).
