# Slice 03: The instance operator restores a deleted project exactly

Stories: US-PL-04 | Estimate: 1 day | job_id: `job-project-lifecycle`

## Goal

The instance operator lists deleted projects and restores one, and every
address, issue id, comment and attachment comes back as it was. While deleted,
the project's name, slug and key stay taken, so a restore never collides.

## IN

- `foundry doctor list-deleted-projects`: one `key: value` block per deleted project (id, workspace, team, name, key, issues, deleted-at, deleted-by), oldest first; `status: none` when there are none (D10, D14).
- `foundry doctor restore-project <uuid>`: `status: restored`, exit 0; exit 4 not restorable (with the reason), 2 invalid uuid, 3 database (the `restore-comment` discipline).
- The create and rename doors keep a deleted project's name, derived slug and key taken, with the shipped copy (OQ-4).
- (OQ-3) One restore record.

## OUT

- A web restore (OQ-6). Restoring a project whose team is gone (slice 04, follow-up). Purge.

## Learning Hypothesis

- **Disproves if it fails**: that clearing one marker is a complete restore. If any reader cached or derived something at delete time (a counter, a lane, an outbox position), INV-PL-4 needs more than the marker.
- **Confirms if it succeeds**: that holding names while deleted (OQ-4) makes restore collision-free with no schema change.

## Acceptance Criteria

- [ ] Priya lists Auth v2 Spike (SPIKE, 3 issues, deleted-by lena@canzan.dev), restores it → `status: restored`; SPIKE-2 opens at its old address with its comments and attachment; the next issue is SPIKE-4.
- [ ] No deleted project → `status: none`, exit 0.
- [ ] A live project's id or an unknown id → exit 4 with the reason; "not-a-uuid" → exit 2; nothing changes.
- [ ] While SPIKE is deleted, creating "auth v2 spike" in Backend or any project keyed SPIKE in Canzan Labs is refused with the shipped copy.
- [ ] Dogfood on dev: restore the slice-02 scratch project; its board and an old issue link open again; read-only counts equal the pre-delete counts.

## Dependencies

- Slice 02. OQ-4, OQ-6.
- Reference class: `foundry doctor restore-comment` (slice 7 of the MVP), about 0.5 day, plus the list command.
