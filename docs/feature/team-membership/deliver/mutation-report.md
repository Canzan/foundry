# Mutation report: team-membership

Strategy: per-feature (CLAUDE.md), gate >= 80% kill rate on every Rust
production file the feature modified (steps 01-01..06-01, `git diff
11785f0~1..42096e9`). cargo-mutants cannot mutate SQL, so migrations 0021 and
0022 and the SQL strings inside store methods are covered by hand-mutation
tables. Timeouts count as caught; unviable mutants (do not compile) are excluded
from the denominator.

Status at step 06-02: the per-step results below, plus one feature-diff pass
(`cargo mutants -j 1 --in-diff <feature diff>`) over the five modified files no
step had measured.

## Per-file verdict

| File | Viable | Caught | Kill rate | Where measured |
|---|---|---|---|---|
| `crates/foundry-core/src/team_membership.rs` | 11 (+11 re-run) | 11 (+11) | 100% | 01-01 (11/11, 2 unviable); 04-01 re-run after the lead arm flip, 11/11 |
| `crates/foundry-core/src/lib.rs` | 0 | 0 | n/a | 06-02 pass: the change is a module declaration, no mutant generated |
| `crates/foundry-store/src/teams.rs` | 14 + 6 + 7 | 27 | 100% | 02-01 (14/14, 1 unviable); 03-01 `remove_team_member` 6/11, raised to 6/6 viable (1 unviable) by eda1755; 05-01 7/7 |
| `crates/foundry-store/src/lib.rs` | 3 | 3 | 100% | 05-01 3/3 (2 unviable); 06-01 1 mutant, unviable; plus hand tables below (03-01 M1-M6, 05-01 H5/H6, 06-01) |
| `crates/foundry-services/src/teams.rs` | 8 + 5 + 1 | 14 | 100% | 02-02 8/8 (3 unviable); 03-02 5/5 (2 unviable); 05-02 1/1 (2 unviable), plus 05-02 hand mutations |
| `crates/foundry-services/src/lib.rs` | 0 | 0 | n/a | 06-02 pass: 6 mutants, all unviable (the `Services` delegators return types with no `Default`); every delegator is exercised by the services and acceptance tests through 02-02..05-02 |
| `crates/foundry-app/src/teams.rs` | 1 + 4 + 2 + 5 | 12 | 100% | 01-02 1/1; 02-02 4/4; 03-02 2/2; 05-02 5/5 (1 unviable) |
| `crates/foundry-app/src/views.rs` | 3 | 3 | 100% | 06-02 pass: 3 `TeamIndexRow::label` mutants missed by the crate's own tests, then killed by hand against the prebuilt binary in the tm-01 lane (see below) |
| `crates/foundry-app/src/lib.rs` | 1 | 1 | 100% | 06-02 pass: `build_router -> Default::default()` caught |
| `crates/foundry-app/src/member_invites.rs` | 1 (+2 hand) | 1 (+2) | 100% | 03-01: 2 hand mutations killed; 06-02 pass: `submit_remove_member -> Default::default()` missed by the crate's own tests, killed by hand in the tm-03 lane (see below) |
| `xtask/src/check_arch.rs` (`team-membership-one-seam`) | 26 | 26 | 100% | 01-03 `--in-diff`: 24 caught + 2 timeouts (counted caught); later steps add seams with an injected-violation gold test each |

Every Rust production file the feature modified is at 100% of viable mutants
(store `teams.rs`/0022 placement hand table 89%, one survivor, H8, below). The
06-02 pass (`mut-0602`, 11 mutants in 9m 26s) reported 1 caught, 4 missed, 6
unviable: the 4 misses sit in handlers and view models that only the
acceptance lanes reach, because cargo-mutants runs each crate's own tests and
the acceptance suite drives the prebuilt `foundry` binary. They were therefore
re-run by hand (below), and all 4 were killed.

### 06-02 acceptance kills of the cargo-mutants misses

Method: the mutation applied to the working tree, `cargo build -p foundry-app
--bin foundry` and a warm-up, the named lane run, then `git checkout` of the
file and a rebuild of the clean binary. Each failure was read to confirm it is
the mutated behaviour, not a flake.

| # | Mutation | Lane | Result |
|---|---|---|---|
| V1 | `TeamIndexRow::label` -> `String::new()` | tm-01 | killed, 4 of 26 scenarios fail ("he opens General from the teams page") |
| V2 | `TeamIndexRow::label` -> `"xyzzy".into()` | tm-01 | killed, 4 of 26 fail (same step) |
| V3 | `member_count == 1` -> `!= 1` | tm-01 | killed, 2 of 26 fail ("the teams page lists exactly: General · 1 member, Backend · 3 members, ...") |
| MI1 | `submit_remove_member` -> `Default::default()` | tm-03 | killed, 3 of 15 fail ("Sam Okafor is no longer in Canzan Labs or on any of its teams") |

## SQL and hand mutations

Method: each mutation was a temporary edit to the file, the named tests were
run, and the file was restored to HEAD afterwards.

### 0021 `team_lead_guard` (step 02-01): 11/11 killed

Killed by the `crates/foundry-store/tests/team_lead_guard.rs` tests (nine
02-01 store tests, four added by the acceptance designer, and the 100-run
demote/demote race).

| # | Mutation | Result |
|---|---|---|
| M1 | Trigger's team lock (`FOR NO KEY UPDATE`) removed | killed, by the hand-typed race (two sessions demoting the last two leads; without the lock both commit) |
| M2 | Cascade exemption dropped (always count) | killed (team/workspace delete refused) |
| M3 | Threshold `other_leads = 0` -> `<= 1` | killed |
| M4 | Threshold `other_leads = 0` -> `< 0` (never refuse) | killed |
| M5 | Count includes the row itself (`user_id <> OLD.user_id` dropped) | killed |
| M6 | UPDATE trigger returns `OLD` (demotion silently a no-op) | killed |
| M7 | Orphan cleanup `DELETE` dropped | killed |
| M8 | Orphan cleanup over-broad (deletes lead rows too) | killed |
| M9 | UPDATE trigger `WHEN` ignores role (only team moves) | killed |
| M10 | DELETE trigger dropped (`WHEN false`) | killed |
| M11 | Refusal constraint name changed | killed |

### `remove_workspace_member`, store `lib.rs` (step 03-01): 6/6 killed

M1 workspace-membership lock dropped, M2 team lock dropped, M3 workspace filter
on the team-row delete dropped, M4 last-lead check bypassed, M5 team-row delete
disabled, M6 workspace-membership delete disabled. All killed by the
`team_lead_guard.rs` removal tests (races at 100 runs) and tm-03.

`member_invites.rs` (03-01): 2 hand mutations on the workspace-removal
handler's last-lead path, both killed (per the 03-01 step record; tm-03
scenarios 36-38 pin the 409 `text/plain` OQ-9 copy and the absent
`member_removed` notification).

### 0022 `workspace_default_team` and placement (step 05-01): 8/9 killed

| # | Mutation | Result |
|---|---|---|
| H1 | Composite FK -> plain FK on `teams(id)` (self-check stripped) | killed |
| H1b | Composite FK -> plain FK, self-check kept | killed |
| H2 | `SET NULL (default_team_id)` -> `SET NULL` all columns (self-check stripped) | killed |
| H3 | `ON DELETE` clause dropped (NO ACTION) (self-check stripped) | killed |
| H4 | Savepoint dropped: placement on the outer transaction, error swallowed | killed |
| H5 | Probe line for `default_team_id` removed | killed by `the_probe_refuses_a_schema_without_the_default_team_column` |
| H6 | Placement call removed from `create_member_and_consume` | killed |
| H7 | Placed role `'member'` -> `'lead'` | killed |
| H8 | `AND w.default_team_id IS NOT NULL` dropped | **survived** (see verdict) |

**H8 verdict: not equivalent; needs a log-capturing test.** With the predicate
gone, a workspace with no default makes the placement `INSERT` a NULL
`team_id`, which fails NOT NULL inside the savepoint; the savepoint rolls back
and the newcomer joins on no team, which is the correct user-visible outcome,
so every behavioural test passes. But the helper then logs "placing a newcomer
on the default team failed" on every invite accept and every first Keycloak
sign-in in a workspace with no default, which after the upgrade is every
workspace. D17/DDD-20 reserve that warning for a real placement failure; H8
turns it into noise on the dominant path and hides real failures in it. The
outcome is the same for users and different for operators, so H8 is a
test-strength gap, not an equivalent mutant. It is not on a lock-order,
savepoint or check-arch clause boundary (the savepoint behaves correctly under
H8), so it does not block the gate: store `teams.rs`'s hand table stands at
8/9 (89%) and its cargo-mutants figure at 100%. Routed to
`nw-acceptance-designer` as a finalize follow-up: a store test that captures
`tracing` output (no capture harness exists in `crates/foundry-store/tests`
yet) and asserts that an accept in a workspace with no default logs no
warning.

### Step 05-02: 2/2 killed

`set_default_team`'s empty-slug branch forced to `false` and to `true`
(`let chosen = if raw_team_slug.is_empty()`): both killed by the tm-05 lane
(clearing the default, and choosing a team).

### Step 06-01: 2/2 killed

Two hand mutations on `provision_federated_member`'s placement (store
`lib.rs`), both killed (per the 06-01 step record) by the three 06-01
`workspace_default_team.rs` tests and tm-06.

## How the SQL is pinned

- 0021: the `team_lead_guard.rs` store tests (refusal SQLSTATE 23514 and
  constraint name, cascade exemption, orphan cleanup, typed `LastLead` without
  a write) and the 100-run races from 02-01 and 03-01, each leaving exactly
  one lead every run.
- 0022: the `workspace_default_team.rs` store tests (the composite FK refuses
  another workspace's team with 23503, deleting the default team clears only
  the default, placement in a savepoint never fails the accept), the migration's
  own apply-time self-check of the key columns and the column-list `SET NULL`,
  and the probe test.

No surviving Rust mutant is waived. The single SQL survivor, H8, is recorded
above with its verdict.
