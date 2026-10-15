# CONTEXT

## Current Task

**project-lifecycle** (rename, soft-delete, operator restore, detaching team delete) finished DELIVER on `main` (`ea90532`; pushed through `4cc3000`, later commits local). Not tagged. v0.14.1 is on dev and prod; **v0.14.2** (ammonia/rustls advisories) is ready on branch `fix-dependency-advisories` (`bd70538`, worktree `../foundry-v0142`) but not pushed or tagged. Evolution doc: `docs/evolution/2026-10-15-project-lifecycle.md`.

## Key Decisions

- The whole feature is ONE release (migrations 0025/0026/0027 together); 0025 and 0026 warn once on upgrade; pre-0026 dumps restore only into a fresh database.
- Machine-token API routes are exempt from project scoping (U1); review findings F-01/F-08 kept as designed, F-14 is a follow-up (upstream-issues UI-3).
- Watch disk: `target/` grew to 334 GB and ENOSPC wedged Docker Desktop; clear `target/debug` when idle.

## Next Steps

- Release on the user's go: R4 read-only OQ-D4 project counts on dev/prod + the CHANGELOG OQ-D4/Rollback lines; R5 `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`; then tag. Separately push/tag v0.14.2 and cherry-pick nothing more (main already has its lockfile fix).
- Follow-ups in the evolution doc: F-14 post-write 404, `list_project_lanes` not on `live_projects`, API 403-vs-404, SSE slug echo, token-route scoping.
