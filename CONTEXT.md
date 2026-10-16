# CONTEXT

## Current Task

**project-lifecycle** (rename, soft-delete, operator restore, detaching team delete) finished DELIVER plus all follow-ups on `main` (through `7286bb4`; pushed through `4cc3000`, 13 later commits local). Not tagged. v0.14.1 is on dev and prod; **v0.14.2** (ammonia/rustls advisories) is ready on branch `fix-dependency-advisories` (`bd70538`, worktree `../foundry-v0142`) but not pushed or tagged. Evolution doc: `docs/evolution/2026-10-15-project-lifecycle.md`.

## Key Decisions

- The whole feature is ONE release (migrations 0025/0026/0027 together); 0025 and 0026 warn once on upgrade; pre-0026 dumps restore only into a fresh database.
- 2026-10-15: API, live-updates and token-route refusals answer the uniform 404 (non-admin token gate and non-author comment edit stay 403); a lead who removes themselves lands on their teams list.
- Watch disk: `target/` grew to 334 GB and ENOSPC wedged Docker Desktop; clear `target/debug` when idle.

## Next Steps

- Push the local commits to `main` only on the user's "push"; push/tag v0.14.2 from `../foundry-v0142` on the user's go.
- Release on the user's go: R4 read-only OQ-D4 project counts on dev/prod + the CHANGELOG OQ-D4/Rollback lines; R5 `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`; then tag.
- Optional cleanup: the scoped-token check is copied in 5 service modules; move it onto `Principal`.
