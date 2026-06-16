# DISTILL Walking Skeleton: per-workspace-backup

> The single `@walking_skeleton @wiring_e2e` scenario, its strategy declaration, and the
> demo-to-stakeholder proof. Source: `crates/foundry-acceptance/tests/features/us-per-workspace-backup.feature`
> scenario 1.

## The walking skeleton scenario

```gherkin
@walking_skeleton @wiring_e2e @us-pwb01
Scenario: An operator exports one workspace to a verifiable archive reporting all ten tables
  When Devansh exports "globex" to a backup path
  Then an archive file exists at that path
  And the output reports a row count for all 10 tenant tables
  And the output ends with "status: OK"
  And the command exits with code 0
```

Background (shared Given setup, two real coexisting workspaces seeded in testcontainers PG16):

```gherkin
Background:
  Given an instance with workspaces "Acme Corp" and "Globex LLC"
  And "Globex LLC" has its own members, teams, projects, issues, and comments
  And "Acme Corp" has its own members, teams, projects, issues, and comments
```

## Litmus test (Mandate 5 / critique Dim 5)

1. **Title describes a user goal?** YES — "An operator exports one workspace to a verifiable archive
   reporting all ten tables" is Devansh's job-to-be-done, not "export pipeline touches all layers".
2. **Given/When describe user actions/context?** YES — "exports 'globex' to a backup path" is the
   operator's actual command; the Background is the instance state he sees, not internal DB setup framing.
3. **Then describe user observations?** YES — an archive file at the path, a per-table count report, the
   terminating `status: OK` line, exit 0. These are exactly what the operator reads on stdout + sees on
   disk — NOT internal side effects (no "row inserted", no private field).
4. **Non-technical stakeholder confirms "yes, that is what users need"?** YES — "the operator can lift one
   tenant's data into a single archive and the tool tells him it captured all 10 kinds of data and
   succeeded." This is the headline value of the whole feature.

## Strategy declaration (Architecture of Reference + Project Infrastructure Policy)

Per the retired A/B/C/D model → structural decision via port class:

| Port | Class | Treatment | Mechanism (inherited from policy) |
|------|-------|-----------|-----------------------------------|
| `foundry doctor` CLI | Driving | Real adapter | `assert_cmd::Command::cargo_bin("foundry")` subprocess |
| `Store::export_workspace` / Postgres | Driven internal (shared state) | Real adapter | testcontainers PG16, per-scenario schema |
| tar archive on filesystem | Driven internal | Real adapter | real filesystem via `tempfile::TempDir` |

No driven external / non-deterministic ports (no clock/email/LLM/paid API). Nothing is faked. This is the
equivalent of the legacy **Strategy C (real local resources)** — every adapter the WS touches is real I/O.
Tag accordingly: `@real-io`. Consistent with the shipped slice-05 / slice-06 doctor-CLI scenarios.

## Why this is the thinnest end-to-end cut

The WS proves the riskiest wiring once: the CLI subprocess parses `export-workspace <selector> <path>`,
resolves the selector to a workspace id, opens a REPEATABLE READ snapshot on the real DB, runs the 10
scoped SELECTs, writes a tar (manifest + 10 JSONL files) atomically to a real path, and prints structured
stdout ending `status: OK` with exit 0. If those wires connect, every focused scenario (isolation,
falsifiability, failure paths) builds on the same established path. It deliberately does NOT yet assert
isolation (scenario 5), falsifiability (scenario 9), or any failure path — those are the `@pending`
focused scenarios unskipped one at a time in DELIVER.

## RED → GREEN lifecycle

- **DISTILL (now)**: scenario authored, `@walking_skeleton @wiring_e2e` (NOT `@pending`). Crate compiles
  (Gherkin only; no `.rs` changes; `acceptance.rs` untouched).
- **DELIVER RED**: the step glue is authored; the WS scenario runs and fails as MISSING_FUNCTIONALITY — the
  `export-workspace` subcommand is unknown / `Store::export_workspace` is absent. Genuine RED.
- **DELIVER GREEN**: implement the thinnest slice — selector resolution, the scoped reader, the tar writer,
  the structured stdout — until the WS goes green. No isolation/failure handling beyond what the WS asserts.
- **DELIVER COMMIT**: commit with `Step-Id:` trailer; then unskip scenario 2, repeat.

## Demo-ability

After GREEN, the WS is demo-able to Devansh in one session: run `foundry doctor export-workspace globex
/backups/globex.dump`, watch the per-table counts print and `status: OK`, then `ls /backups/globex.dump`.
That is the "from impossible-without-manual-surgery to a one-command export" outcome KPI (US-PWB-01) made
observable.
