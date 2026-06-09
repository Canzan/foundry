-- crates/foundry-store/migrations/0009_multi_workspace.sql
-- multi-workspace-tenancy — Slice 1: drop the single-workspace guard.
--
-- 0001_init.sql:15 created `uniq_one_workspace ON workspaces ((true))` to enforce
-- I-W1 ("at most one workspace per instance"). With the per-table `workspace_id`
-- scoping shipped, a Foundry instance can safely host multiple tenants, so this
-- guard is removed. Dropping the index is FORWARD-ONLY (ADR-006) and rewrites NO
-- existing row — the lone `((true))` entry simply ceases to be enforced, letting a
-- second `workspaces` row be inserted. Idempotent via `IF EXISTS` so re-runs and
-- fresh schemas without the index both succeed.

DROP INDEX IF EXISTS uniq_one_workspace;
