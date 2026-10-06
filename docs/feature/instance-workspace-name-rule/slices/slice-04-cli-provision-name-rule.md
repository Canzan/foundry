# Slice 04: The operator CLI refuses an unfit name before touching the database

Story: US-WNR-04 | Estimate: 0.5 day | job_id: `job-instance-workspace-naming`

## Goal

`foundry doctor provision-workspace` trims `--name` and checks it with the
shared rule before any DB connection. A name that fails gets exit 2, the copy
on stderr after the command prefix, and an empty stdout. Success prints the
trimmed name.

## IN

- A pre-DB rule check in `run_provision_workspace`, beside the existing argument checks (exit 2, D8). The service-side check from slice 03 stays as the shared backstop.
- Trim before the check, the store write and the `workspace-name:` output (D10).
- Examples: 32 characters, blank, an embedded newline (forged line), padded name, and a 32-character name with `DATABASE_URL` unset.

## OUT

- New exit codes. Changes to the `export-workspace` output that prints legacy names.

## Learning Hypothesis

- **Disproves if it fails**: that exit 2 plus the shared copy on stderr is enough for a provisioning script to branch on. If a script cannot tell a name refusal from a missing flag (both exit 2) and needs to, a distinct exit code is required. Check this against the operator's real provisioning script or shell history.
- **Confirms if it succeeds**: that the forged-output-line hazard is closed at the only scripted door, and that trimming changes no name the operator actually provisioned. Check with the slice-01 dogfood query: no stored name has leading or trailing spaces, or the ones that do are listed.

## Acceptance Criteria

- [ ] `--name "Canzan Labs Platform Engineering"` exits 2 with stderr ending in "Workspace name must be at most 24 characters", an empty stdout, and zero rows.
- [ ] `--name "   "` exits 2 with the empty copy.
- [ ] A newline name exits 2 with the control copy and prints nothing on stdout.
- [ ] `--name "  Globex  "` exits 0, prints `workspace-name: Globex` and stores "Globex".
- [ ] With `DATABASE_URL` unset, a 32-character name exits 2, not 3.
- [ ] The mwt-slice-06 lane stays green. Rebuild `foundry` before the CLI lane.

## Dependencies

Slices 01 and 03. Reference class: mwt slice 06 (CLI subprocess scenarios).
