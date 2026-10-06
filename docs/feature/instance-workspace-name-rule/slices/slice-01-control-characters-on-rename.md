# Slice 01: Control characters are refused at rename, by the shared rule

Story: US-WNR-01 | Estimate: 0.5-1 day | job_id: `job-instance-workspace-naming`

## Goal

The shipped rename door refuses a trimmed name that contains a D4 control
character. The refusal uses the new copy and lands in the row's error slot.
The rule gains its control-character arm in the one shared place that slices
02-04 will call.

## IN

- The control-character arm in the shared rule (D4 set). Order: empty, then control, then length (D5). The no-op stays first for rename (D6).
- A new 422 arm in `submit_workspace_rename` with the copy "Workspace name must not contain control characters" (OQ-2 may change the wording).
- A single source for the three refusal strings, so later doors do not copy them (D2/D3; DESIGN picks the shape).
- Examples: tab, U+202E, NUL (HTTP), U+2028, ZWJ emoji accepted, edge whitespace trimmed, legacy tab name as a no-op.

## OUT

- Other doors (slices 02-04). Project names. DB CHECK. Rewriting legacy names.

## Learning Hypothesis

- **Disproves if it fails**: that the D4 set refuses real paste accidents without refusing any name the operator really uses. The dogfood query on her instance must show 0 existing names refused for reasons other than a genuine stray character. Any legitimate name refused (for example one using U+200C or an emoji sequence) means D4 is wrong and OQ-1 reopens.
- **Confirms if it succeeds**: that the precedent's pure classifier takes a new arm without a change of shape, so slices 02-04 only add call sites.

## Acceptance Criteria

- [ ] "House\tHold", "Ops\u{202E}gnikcatS" and "Bailey\u{0}Family" each get 422 with the control-character copy in the row slot, with no write and no audit row. NUL is never a 500.
- [ ] "👨‍👩‍👧 Bailey" and "Ångström Øresund Société" are accepted and audited.
- [ ] "\tKitchen\n" is stored as "Kitchen".
- [ ] A legacy "Canzan\tLabs" resubmitted byte-equal gets a quiet 200 with no write.
- [ ] A 30-character name containing a tab gets the control-character copy.
- [ ] The iawr lane stays at 27/27.
- [ ] Dogfood: a read-only query on the operator's instance counts existing names that fail length or D4, and the count is recorded as the KPI-1 baseline.

## Dependencies

OQ-1 and OQ-2 (user) should be answered before DISTILL fixes the example
table. Reference class: precedent slice 01 step 02-02 (validation edges, about
half a day).
