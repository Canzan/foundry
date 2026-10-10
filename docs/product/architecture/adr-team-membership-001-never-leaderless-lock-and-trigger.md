# ADR-TEAM-MEMBERSHIP-001: A team keeps its last lead through a per-team row lock taken by the store, with a database trigger taking the same lock as the backstop for every other caller

- Status: Accepted (2026-10-09)
- Feature: `team-membership` (DISCUSS D6, OQ-2, OQ-9, INV-1 to INV-4, INV-8; DESIGN DDD-6, DDD-7, DDD-8, DDD-11)

## Context

`team_memberships(team_id, user_id, role IN ('lead','member'))` has had no writer that removes
a lead, so "a team has a lead" was never threatened. This feature adds three verbs that can
remove one: demote (`role` lead → member), remove from a team, and (OQ-9) removal from the
workspace, which now deletes the person's team rows. D6 requires that no write leave a team that
has a lead with none, and that the refusal be a stated reason, not a 404. INV-2 and INV-3 require
this under concurrency: two leads demoted at once, or a remove and a demote, must not both
succeed when they are the last two. INV-8 requires it for any future caller, not only these
handlers.

The store runs Postgres 16 at READ COMMITTED. A read-check-then-write is a write-skew: two
transactions each see the other lead still in place, and both commit. No CHECK or UNIQUE
constraint can state "at least one row with role lead per team", because the rule spans rows.

Precedents: ADR-BOARD-LANE-002 and -003 serialise per-project lane writes with `FOR UPDATE` on
the rows being decided on, and reject SERIALIZABLE and advisory locks. ADR-NAME-DB-001 put a rule
"for every caller" into BEFORE triggers that raise SQLSTATE 23514 with a named arm. The 0015 lane
constraint is `DEFERRABLE` because a statement-level check was needed there.

## Decision

1. **The lock.** Every write that can remove a lead takes `SELECT … FROM teams WHERE id = ANY($ids)
   ORDER BY id FOR NO KEY UPDATE` on the affected team rows, as its first statement after any
   workspace-membership lock (decision 5). The lock is held until commit. `FOR NO KEY UPDATE`
   conflicts with itself, so lead-removing writes on one team are totally ordered. It does not
   conflict with the `FOR KEY SHARE` that foreign-key checks take, so adding a member or creating a
   project on the team is never blocked by it. Locking in team-id order means a multi-team write
   (workspace removal) and a single-team write cannot form a lock cycle.
2. **The decision, in the store, under the lock.** At READ COMMITTED each statement takes a fresh
   snapshot. Once the lock is granted, the next statement sees every lead removal committed before
   it, and no other lead removal on that team can commit until this transaction ends. The store
   then reads "is there another lead on this team" (exact, because serialised) and either writes
   or returns a typed `LastLead` outcome without writing. Writes that add leads (promote) take no
   lock. They can only raise the count, which makes a concurrent refusal conservative, never wrong.
3. **The backstop, in the database (INV-8).** Migration `0021_team_lead_guard.sql` adds one
   plpgsql function, `foundry_team_keeps_a_lead()` (`SET search_path FROM CURRENT`, as 0019), and
   two triggers on `team_memberships`:
   - `team_memberships_lead_guard_on_delete`: `BEFORE DELETE … FOR EACH ROW WHEN (OLD.role = 'lead')`.
   - `team_memberships_lead_guard_on_update`: `BEFORE UPDATE OF role, team_id … FOR EACH ROW WHEN
     (OLD.role = 'lead' AND (NEW.role <> 'lead' OR NEW.team_id <> OLD.team_id))`.

   The function takes the **same** `FOR NO KEY UPDATE` lock on `teams` row `OLD.team_id` (a no-op
   when the store already holds it). If that row no longer exists, the team is being deleted (a
   cascade from `teams` or `workspaces`), and the function returns `OLD`, because a team that is
   gone needs no lead. Otherwise it counts the other lead rows of the team. Row-level BEFORE
   triggers see the effects of rows processed earlier in the same outer command, so a single
   statement demoting both leads is refused on its second row. With no other lead, it raises
   `ERRCODE 'check_violation'` (23514), `CONSTRAINT = 'team_memberships_team_keeps_a_lead'`,
   `TABLE = 'team_memberships'`, the native CHECK wording, and a one-line HINT. There is no DETAIL
   row dump. A psql statement, a script, or a future Rust caller that skips the store's lock is
   still serialised by the trigger's own lock and refused by its own count.
4. **Mapping.** The store maps that SQLSTATE and constraint name to the same typed `LastLead`
   outcome, so even a path that skipped the pre-read gets the right answer rather than a 500. On
   the store's own paths the trigger never fires, because the pre-read under the lock already
   decided. The race tests assert zero 500s.
5. **Workspace removal (OQ-9) shares the lock and the trigger.** `Store::remove_workspace_member`
   becomes one transaction:
   1. Lock the workspace-membership row (`FOR UPDATE`; see DDD-10 for why first).
   2. Lock the person's teams in that workspace, in id order (decision 1).
   3. Read the teams where the person is the only lead, ordered by team name. If there are any,
      roll back and return `LastLead { team }` for the first.
   4. Delete the person's team rows in that workspace (the trigger checks each one).
   5. Delete the workspace-membership row.
6. **"Lead" means a lead row.** Before it creates the triggers, 0021 deletes the team rows of
   users who are not members of the team's workspace (orphans left by the pre-OQ-9 removal). That
   is OQ-9's rule applied to legacy rows. Such rows confer nothing (a session cannot act in a
   workspace its user has left), and the count is expected to be 0 on dev and prod. The migration
   reports the count with `RAISE NOTICE`. After it, "lead" has one definition everywhere: a
   `team_memberships` row with `role = 'lead'`.
7. **Re-runnable body**, as 0019: `CREATE OR REPLACE FUNCTION`, then `DROP TRIGGER IF EXISTS`
   followed by `CREATE TRIGGER`, each with a `COMMENT`. The undo is documented SQL that drops the
   two triggers and the function.
8. **`Store::probe` is unchanged.** The app's correctness does not depend on the trigger, which is
   a backstop. A probe that refused readiness without it would turn the documented undo into an
   outage (ADR-NAME-DB-001 decision 5).

## Alternatives considered

- **Store lock and pre-check only, no database guard.** This is correct for today's callers.
  Rejected for INV-8: it holds only while every future writer remembers the lock. That is the
  convention-spanning-all-adapters argument ADR-BOARD-LANE-002 used to reject advisory locks.
- **Trigger only, with no store lock or pre-check.** Viable, and it states the rule once. Rejected:
  a multi-team workspace removal would take team locks in heap order inside the trigger and could
  deadlock against another removal. The OQ-9 copy also needs the team's name, which would mean
  parsing it out of an error. The store's ordered lock and exact pre-read solve both. The trigger
  keeps the guarantee.
- **SERIALIZABLE with retry.** Rejected, as in ADR-BOARD-LANE-002. It protects only transactions
  that themselves run SERIALIZABLE, so a READ COMMITTED psql session or a future caller bypasses
  it. It also adds a retry loop and turns unrelated membership writes into potential aborts.
- **A `pg_advisory_xact_lock(team)` per team.** Rejected: correct only if every writer takes it,
  the same convention problem. The row lock is taken by the trigger itself, so nobody can forget it.
- **A `DEFERRABLE INITIALLY DEFERRED` constraint trigger, checked at commit.** It is equally
  correct if it takes the same lock, and it would allow a lead swap inside one transaction.
  Rejected: no product flow swaps leads in one transaction (hand-over is promote, then demote), the
  refusal would surface at `COMMIT` rather than at the statement, and the name-rule precedent
  refuses at the statement.
- **Counting only leads who are still workspace members (keeping orphan rows).** Rejected: the
  trigger would then depend on `workspace_memberships` as well, and deleting a membership row would
  silently de-lead a team with no trigger firing. Cleaning the orphans once (decision 6) keeps one
  simple definition.

## Consequences

- Positive: INV-1 to INV-4 hold for these handlers by serialisation plus an exact read, and INV-8
  holds for every caller by the trigger, including psql. The workspace-removal refusal reuses the
  same lock and trigger, so slice 03's learning hypothesis ("the seam generalises to a second
  verb") is answered by construction.
- Positive: no new isolation level, no advisory locks, no retry loop.
- Negative: a non-store caller that updates a lead row while a store transaction holds that
  team's lock can deadlock with it (the row lock is taken first by the outer statement, then the
  team lock by the trigger). Postgres aborts one transaction with 40P01. No invariant breaks, and
  the store path, if it is the victim, answers 500. This is accepted at homelab scale.
- Negative: a future user hard-delete (`users … ON DELETE CASCADE`) would be refused for a sole
  lead. No user delete exists. That feature must decide, as ADR-WORKSPACE-RENAME-001 noted for its
  actor FK.
- Negative: 0021 deletes rows (orphans, expected 0). It is forward-only, like every migration.
  The CHANGELOG states it.
- Neutral: the table owner can still `DROP` or `DISABLE` the triggers. The guard protects against
  slips, not against a hostile DBA.
- Earned Trust: store-level gold tests on the pinned `postgres:16` image prove the five substrate
  facts this relies on:
  1. A BEFORE ROW trigger sees rows processed earlier in the same command.
  2. A cascade-deleted team row is invisible to the trigger.
  3. `FOR NO KEY UPDATE` does not block an FK `KEY SHARE`.
  4. A statement after a lock wait sees the predecessor's commit.
  5. Two concurrent demotes, and a remove racing a demote, of the last two leads end with exactly
     one lead over at least 100 runs.

  The shipped "workspace is deleted" acceptance scenario (recipient-notification-preferences)
  stays green as the cascade regression guard.
