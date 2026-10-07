# Slice 02: The create form applies the same name rule as rename, and creates nothing on refusal

Story: US-PNR-02 | Estimate: 1 day | job_id: `job-project-naming`

## Goal

`POST /team/{team_slug}/projects` parses the name through the same rule as
rename: empty, then control, then at most 256 scalars. Each refusal is a 422 with
the shared copy, in the shipped form shape (plain POST re-render, or the htmx
fragment). A refusal writes no project and no lanes.

## IN

- Create calls the shared rule after the membership gate and before the slug mint and uniqueness (D5, D7, D10). DESIGN decides whether a services seam is introduced (OQ-D2).
- Create's empty-name copy comes from the one source (D4).
- 422 with name and key prefix retained (plain POST), or the bare fragment (htmx) (D11).
- The bootstrap "Sandbox" constant is pinned by a test to pass the rule (D13).
- One `@needs-browser` example on the real form.

## OUT

- The uniqueness change (slice 03). Key-prefix rules. The fallback slug mint (slice 04). `maxlength` (D12).

## Learning Hypothesis

- **Disproves if it fails**: that a member door can take the admin door's rule without refusing names members actually use. If the dogfood query finds existing names over 256 or containing a D3 character that were created on purpose, the 256 cap or the set is wrong for the member door.
- **Confirms if it succeeds**: that the create handler's shipped 422 shapes carry the new arms with no template change beyond copy, and that name-before-key precedence survives.

## Acceptance Criteria

- [ ] A 256-scalar name (ASCII and "日" ×256) creates the project. 257 gets 422 with "Project name must be at most 256 characters", name and key retained, and no `projects` or `lanes` row.
- [ ] "Homelab\tOps" and "Reading\u{2028}List" get the control copy. "Homelab\u{0}Ops" gets the control copy, never a 500.
- [ ] After a refusal with key "OPS", "Homelab Ops" with "OPS" lands on `/team/backend/project/homelab-ops`.
- [ ] Marco (not on Backend) posting a 300-character name gets the byte-identical uniform 404.
- [ ] A 300-character name with key "ops" gets the length copy, not a key copy.
- [ ] us-07 and us-r01 stay green.
- [ ] Dogfood: Priya creates a real project on her instance through the form the same day.

## Dependencies

Slice 01 (the shared rule and the copy source). Reference class: precedent slice 03 (dashboard provisioning with a 422 re-render, about 1 day).
