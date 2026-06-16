# ADR-004: verify-export design (completeness + isolation from the path alone)

## Status
Accepted

## Context
`verify-export <path>` must confirm an archive is (a) complete (all 10 tenant tables present) and
(b) isolation-clean (every row resolves to the declared workspace, zero siblings), using ONLY the
path — the declared workspace is read from the archive header, not passed as an argument
(NFR-PWB-INT-01). A planted sibling row MUST red the check (AC-02.4 falsifiability), and a truncated
archive MUST exit 4 (AC-03.5). verify reads from the FILE, not the live DB — it is the
proof-on-the-artifact, runnable on a different host than the export.

## Decision
verify-export pipeline (architecture.md Section 4):
1. Open the tar at `<path>`. Missing/unreadable/not-a-tar -> exit 4 (truncated/incomplete).
2. Read `manifest.json` -> `declared_workspace_id`, `tenant_tables[]`, `row_counts{}`.
3. **Completeness**: assert all 10 tables (the shared `TENANT_TABLES` constant, re-checked against
   the manifest's list) have a `tables/<t>.jsonl` entry, AND each file's line count == the manifest
   `row_counts` value. Mismatch or missing table -> exit 4.
4. **Isolation**: for each archived row, apply the SAME scope predicate (Section 5) against the
   archived rows themselves (verify resolves `team_memberships.team_id`, `comments.issue_id`, etc.
   against the archived `teams`/`issues` to confirm they point to the declared W). Any row whose
   resolved workspace != declared W (tables 1-9), or any `users` row that is not a member of W
   (table 10), -> non-zero exit, message NAMES the offending row + the foreign workspace.
5. `status: OK` + exit 0 on a clean archive.

verify operates on the archive contents (offline), not by re-querying the live DB — so it can be run
by the archive's recipient on another instance.

## Alternatives Considered
1. **Pass the declared workspace as a CLI argument to verify.** Rejected: defeats NFR-PWB-INT-01
   (the proof must be self-contained); an operator could verify against the wrong workspace.
2. **verify re-queries the live DB to resolve each row's workspace.** Rejected: the archive must be
   verifiable offline / on another host (handing it to a departing customer). The transitive
   resolution is done WITHIN the archive (archived `issues`/`teams` are the resolution source).
3. **Trust the manifest `row_counts` for isolation (count-only verify).** Rejected: a leaked sibling
   row would still pass a pure count check. Isolation MUST read every row and resolve it — the count
   is only the cheap truncation tripwire.

## Consequences
- Positive: path-only, offline-verifiable proof; the recipient can independently confirm isolation.
- Positive: falsifiability by construction — a planted sibling row reds step 4 (AC-02.4).
- Positive: truncation caught cheaply by the count tripwire before the full isolation pass.
- Negative: verify re-implements the predicate over archived rows (not live SQL); the shared
  definition (Section 5) must be expressible against in-memory rows too. The gold test guards that
  the two expressions agree (selection == isolation).
