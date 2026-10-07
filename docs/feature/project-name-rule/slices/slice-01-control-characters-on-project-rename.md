# Slice 01: Control characters are refused at project rename, by the one shared rule

Story: US-PNR-01 | Estimate: 0.5-1 day | job_id: `job-project-naming`

## Goal

The shipped instance-admin project rename refuses a trimmed name that contains a
D3 control character. The refusal uses the new copy and lands in the row's error
slot. The project-name rule and its copy get their one source, and the D3
character set is shared with `WorkspaceName`, so slices 02 and 03 only add call
sites.

## IN

- The control arm in the shared project-name rule. Order: no-op (rename only), then empty, then control, then length (256), then uniqueness (D5).
- One definition of the D3 set, used by both `WorkspaceName` and the project rule (D3; DESIGN picks the shape).
- One source for the four project-name strings, so the rename handler stops holding its own copy (D4).
- Examples: tab, U+202E, NUL (HTTP lane), U+2028, ZWJ emoji accepted, edge whitespace trimmed, a legacy tab name as a no-op, and control before length.

## OUT

- The create door (slices 02-03). DB CHECK. Rewriting legacy names. Slug changes.

## Learning Hypothesis

- **Disproves if it fails**: that the workspace D4 set suits project names too. The dogfood query must show 0 existing project names that a real person typed on purpose being refused for a D3 character. Any legitimate name refused (for example one using ZWNJ or an emoji sequence) means the shared set is wrong for projects. The "exactly the workspace set" decision then reopens.
- **Confirms if it succeeds**: that `classify_rename` takes the shared rule without changing its shape, and that `WorkspaceName` and the project rule can share one predicate with no behaviour change to the workspace lanes (iwnr stays 61/61).

## Acceptance Criteria

- [ ] "Sand\tbox", "Ops\u{202E}spoH" and "Identity\u{0}Platform" each get 422 with "Project name must not contain control characters" in the row slot. Nothing is written. NUL is never a 500.
- [ ] "Café Roadmap 👨‍👩‍👧" is accepted, and `/team/backend/project/sandbox` still serves the board.
- [ ] "\tSandbox Experiments\n" is stored as "Sandbox Experiments".
- [ ] A legacy "Homelab\tOps" resubmitted byte-equal gets a quiet 200 with no write.
- [ ] A 300-character name with a tab gets the control copy.
- [ ] The iapr lane stays at 21/21, and iwnr stays at 61/61.
- [ ] Dogfood: a read-only query on the operator's instance counts project names over 256 or containing a D3 character, and the count is recorded as the KPI-1 baseline.

## Dependencies

None outstanding. Reference class: precedent `instance-workspace-name-rule` steps 01-01 and 01-02 (value object plus rename composition, about 1 day together).
