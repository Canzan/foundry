# Acceptance Criteria: per-workspace-backup

> Testable Given-When-Then derived from the UAT scenarios in `user-stories.md`. All criteria are
> observable at the `foundry doctor` CLI surface (stdout lines + exit codes). Acceptance-designer
> (DISTILL) formalizes these against the shipped harness; the `.feature` file is the source Gherkin.

## US-PWB-01 — Export one workspace's data

### AC-01.1 — list-workspaces shows identity

```gherkin
Given an instance with workspaces "Acme Corp" (slug "acme") and "Globex LLC" (slug "globex")
When Devansh runs "foundry doctor list-workspaces"
Then stdout lists each workspace's id, slug, and name
And stdout ends with "status: OK"
And the command exits with code 0
```

### AC-01.2 — export by slug writes a per-table report

```gherkin
Given "Globex LLC" has 7 members, 3 teams, 8 projects, 412 issues, and 1893 comments
When Devansh runs "foundry doctor export-workspace globex /backups/globex-2026-06-16.dump"
Then a file exists at "/backups/globex-2026-06-16.dump"
And stdout reports a row count for all 10 tenant tables
And stdout ends with "status: OK"
And the command exits with code 0
```

### AC-01.3 — export by id resolves the same workspace

```gherkin
Given Acme Corp's workspace id is known
When Devansh runs "foundry doctor export-workspace <acme-id> /backups/acme.dump"
Then the id resolves to Acme Corp
And an archive of Acme Corp is written to "/backups/acme.dump"
```

### AC-01.4 — DB unreachable is a clean failure

```gherkin
Given DATABASE_URL points at an unreachable database
When Devansh runs "foundry doctor export-workspace globex /backups/globex.dump"
Then the command exits with code 3
And stderr says it could not connect to the database
```

## US-PWB-02 — Isolation + verification (the crux)

### AC-02.1 — export contains only the target's rows

```gherkin
Given Acme and Globex each have their own teams, projects, issues, and comments
When Devansh exports "globex"
Then every row in the archive belongs to the Globex workspace
And no row in the archive belongs to the Acme workspace
And the archive's member set is exactly Globex's members
```

### AC-02.2 — verify-export confirms completeness + isolation

```gherkin
Given a freshly exported Globex archive at "/backups/globex-2026-06-16.dump"
When Devansh runs "foundry doctor verify-export /backups/globex-2026-06-16.dump"
Then stdout confirms all 10 tenant tables are present
And stdout confirms every row belongs to the declared Globex workspace
And stdout confirms no row references a sibling workspace
And the command exits with code 0
```

### AC-02.3 — transitive scope is isolation-checked

```gherkin
Given Globex comments reach their workspace only via issue and project foreign keys
When Devansh verifies a Globex export
Then each comment is resolved to its owning workspace through the foreign-key chain
And every comment is confirmed to belong to Globex
```

### AC-02.4 — the isolation check bites (falsifiability)

```gherkin
Given an archive that wrongly contains one row belonging to Acme
When Devansh runs "foundry doctor verify-export" on that archive
Then the isolation check fails
And the command exits with a non-zero code
And the message identifies a row resolving to a workspace other than the declared one
```

### AC-02.5 — property: any single-workspace export is sibling-free

```gherkin
@property
Given an instance with two or more workspaces holding tenant data
When any one workspace is exported and then verified
Then verification confirms zero rows resolve to any other workspace
```

## US-PWB-03 — Failure paths & safety

### AC-03.1 — unknown workspace refused with guidance

```gherkin
Given no workspace has the id or slug "nope"
When Devansh runs "foundry doctor export-workspace nope /backups/x.dump"
Then the command exits with code 2
And the message tells Devansh to run "foundry doctor list-workspaces"
And no archive file is created
```

### AC-03.2 — output-path error fails before any DB read

```gherkin
Given the output path "/nope/x.dump" parent directory does not exist
When Devansh runs "foundry doctor export-workspace globex /nope/x.dump"
Then the command exits with code 5
And no file exists at "/nope/x.dump"
And no tenant data was read
```

### AC-03.3 — atomic write: no complete-looking partial

```gherkin
Given the backup disk fills while exporting "globex"
When the export fails mid-write
Then no file exists at the final output path
And only a discardable partial file may remain
And a later verify-export on the final path finds no archive to accept
```

### AC-03.4 — sole workspace export is valid and read-only

```gherkin
Given a single-tenant instance whose only workspace is "Acme Corp"
When Devansh runs "foundry doctor export-workspace acme /backups/acme.dump"
Then an archive is written
And stdout notes this is the only workspace on the instance
And Acme Corp and all its data still exist on the instance unchanged
```

### AC-03.5 — truncated archive detected

```gherkin
Given an archive truncated when the disk filled mid-export
When Devansh runs "foundry doctor verify-export" on it
Then the command exits with code 4
And the message says the archive is truncated or incomplete and to re-run the export
```

### AC-03.6 — at-rest sensitivity disclosed

```gherkin
Given Devansh successfully exports "globex"
When the export completes
Then stdout prints a note that the archive contains password hashes and machine-token rows
And the note advises treating the archive as sensitive at rest
```

## Exit-code contract (cross-cutting, mirrors admin_cli.rs)

| Code | Meaning |
|------|---------|
| 0 | success (`status: OK` / `status: <verb>` printed) |
| 2 | invalid argument (unknown workspace, missing args) |
| 3 | DB / infra failure (DATABASE_URL unreachable, mid-export DB error) |
| 4 | archive truncated / incomplete (verify-export) |
| 5 | output-path error (parent missing / unwritable; fails before DB read) |
