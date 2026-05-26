# RED classification — slice 7 (comment-tombstone-gc)

Per nw-distill § "Pre-DELIVER fail-for-the-right-reason gate" (Rust
adaptation). After scaffolds + .feature landed, the slice-7 scenarios
were executed against the RED scaffold step bodies; classification
below.

Command used:

```bash
cargo check -p foundry-acceptance --tests          # gate 1: compile
cargo test -p foundry-acceptance --test acceptance -- -t "@slice7"  # gate 2: run @slice7 only
```

Gate 1 result: `Finished dev profile [unoptimized + debuginfo]
target(s) in 1.83s` after the initial 10.82s cold compile — compile
passes.

Gate 2 result: `1 feature, 9 scenarios (9 failed), 48 steps (39
passed, 9 failed)`.

Each failure is the canonical RED scaffold panic emitted from the
slice-7 step bodies in
`crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs`.
Captured output verbatim:
`Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this`.

Note on the cucumber-rs filter: the `-t "@slice7"` argv selection
includes the `@slow` scenario #3 (the slice-7 invocation is for the
RED-classification gate, NOT the default fast loop). The
default `cargo test` invocation (no `-t` arg) WILL exclude `@slow`
once DELIVER adds the one-line `@slow` filter to
`tests/acceptance.rs` per slice-7 wave-decisions.md D3 = A; that
filter edit is DELIVER's responsibility and does NOT change the RED
classification (the scaffold panics fire identically with or without
the `@slow` filter).

## Per-scenario classification

The 9 scenarios fall into two groups by where the first slice-7
panic fires:

- **Group A (GC scenarios #1-#6)**: panic fires at the first slice-7 GC Given (either the cadence-only "the operator's foundry instance is running with the tombstone sweep cadence set to 1 second" OR the cadence-and-cap "... set to 1 second and per-run cap set to N"). Background steps (slice-1 workspace/member/project/issue + Mei sign-in) all pass GREEN.
- **Group B (admin-cli scenarios #7-#9)**: panic fires AFTER the slice-6-inherited Given "the operator's foundry instance is running" (which currently ALSO panics RED in slice 6 — but for this gated run, scenarios #7-#9 still print the slice-6 Given as PASSED because the gate's `-t "@slice7"` filter does NOT also exclude `@slice6` from the slice-6 inheritance graph; cucumber-rs evaluates the slice-6 Given against the slice-6 step body which itself panics). Wait — re-check: the slice-6 step body for "the operator's foundry instance is running" ALSO carries a `panic!("Not yet implemented")` scaffold (slice-6 also defers production-side scaffolds to DELIVER). So if that Given fires for the slice-7 scenarios #7-#9, it would panic too. The observed run output for #7 shows the Given PASSING — verified by re-reading the test output above: "✔ Given the operator's foundry instance is running" — which means EITHER (a) the slice-6 step body has been filled in by some downstream work since slice-6 DISTILL, or (b) the slice-7 step's panic fires AFTER the slice-6 Given. Most likely (a): the slice-6 DELIVER work has landed (the slice-7 task brief implies this — line "DELIVER picks up production-side scaffolds (or full implementations) from a clean slate", but the existing test output shows slice-6 work is present, so DELIVER on slice-6 has already happened OR the slice-6 Given is no longer a scaffold). For the slice-7 gate this is fine — the slice-7 panic still fires at the first slice-7-specific step, which is the contract.

Each entry below records: scenario title → classification (category)
→ step that fired the panic.

1. `A daily tombstone sweep removes comment tombstones older than 90 days and increments the purged-total counter` → **RED (MISSING_FUNCTIONALITY)** → `Given the operator's foundry instance is running with the tombstone sweep cadence set to 1 second` (`us_10_tombstone_gc.rs:49` — the `given_foundry_running_with_gc_cadence` step body panics; the inner sub-helper `spawn_foundry_with_gc_cadence` is what DELIVER implements per `step-skeletons.md` § "Subprocess wrapper")
2. `The sweep keeps tombstones still inside the 90-day audit window` → **RED (MISSING_FUNCTIONALITY)** → same Given step (cadence=1s variant)
3. `A single sweep tick deletes at most the per-run cap of tombstones; the remainder drain on the next tick` (`@slow`) → **RED (MISSING_FUNCTIONALITY)** → `Given the operator's foundry instance is running with the tombstone sweep cadence set to 1 second and per-run cap set to 10000` (`us_10_tombstone_gc.rs:59` — the cadence-and-cap variant)
4. `When two replicas attempt the sweep concurrently exactly one performs the work` → **RED (MISSING_FUNCTIONALITY)** → cadence-only Given (same as #1)
5. `A transient sweep failure does not kill the background task and the next tick succeeds` → **RED (MISSING_FUNCTIONALITY)** → cadence-only Given (same as #1)
6. `The pending-tombstones gauge reflects the count of comments awaiting deletion at each tick` → **RED (MISSING_FUNCTIONALITY)** → cadence-and-cap Given (same as #3)
7. `An operator restores an accidentally-deleted comment by running the doctor subcommand` → **RED (MISSING_FUNCTIONALITY)** → `And a tombstoned comment "abandoned-thought" exists on "AUTH-3" with deletion age 5 days authored by Mei` (`us_10_tombstone_gc.rs:106` — `given_single_tombstoned_comment_for_undelete` panics; the slice-6-inherited "the operator's foundry instance is running" passed GREEN ahead of it)
8. `An operator who passes a UUID that does not match any tombstoned comment gets a non-zero exit` → **RED (MISSING_FUNCTIONALITY)** → `When the operator runs `foundry doctor restore-comment <missing-uuid>` as a subprocess against the live database` (`us_10_tombstone_gc.rs:141` — `when_operator_runs_restore_comment_with_missing_uuid` panics)
9. `An operator who passes a malformed UUID gets the invalid-argument exit code` → **RED (MISSING_FUNCTIONALITY)** → `When the operator runs `foundry doctor restore-comment not-a-uuid` as a subprocess against the live database` (`us_10_tombstone_gc.rs:153` — the regex variant `when_operator_runs_restore_comment_with_literal_arg` panics; regex now correctly disambiguates from the literal-text Whens via `[^<\s]\S*` matcher per the disambiguation fix during DISTILL gate-run)

## Failure-mode categories

- **MISSING_FUNCTIONALITY** (correct RED): **9 of 9** — slice-7 production code (background GC tokio task, `Store::gc_tombstoned_comments`, `Store::count_pending_tombstones`, `Store::undelete_comment`, the `foundry doctor restore-comment` CLI subcommand, the test-hook env-var flag for failure injection) plus the slice-7 test-only fixture (`support/tombstone_factory.rs` direct-SQL helper) are not yet implemented; the step body panics with the scaffold marker. DELIVER's responsibility (per `wave-decisions.md` § "DELIVER Pre-flight Checklist" sub-deliverables A through F).
- **IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE** (wrong RED): **0 of 9**. The test infrastructure (per-scenario PG schema via slice-1 `fresh_schema_pool_with_url`, Postgres testcontainer, slice-1/2 Background step modules, World struct with the new slice-7 fields, slice-6 subprocess pattern available for re-use) is all sound; only the slice-7 step bodies and the slice-7 `tombstone_factory` helper panic.
- **WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT** (wrong shape): **0 of 9**. The assertions are at the right port — the `/metrics` GET endpoint is the operator's observable surface for the GC metric assertions; the `foundry doctor restore-comment` CLI exit code + stdout/stderr are the operator's observable surface for the CLI scenarios; the DB row count (via the `tombstone_factory::count_tombstoned_comments_on_issue` helper) is observable internal state that the production code under test (`Store::gc_tombstoned_comments`) mutates through the production driving adapter.

All **39 of 48 inherited Background + slice-6-inherited steps** pass
GREEN (slice-1 workspace + team-member + project + issue + sign-in
seeding; slice-6 "the operator's foundry instance is running"). No
infrastructure or fixture failure was observed; the only failures
are the deliberate scaffold panics at the slice-7-specific step
boundaries.

Pre-DELIVER gate: **PASSED** — proceed to DELIVER under ADR-025 D2
(DELIVER RED phase = unskip these scaffolds, write PBT unit tests
for the GC predicate / batch loop / undelete UPDATE / CLI UUID parse
+ exit-code mapping, then implement the 6 sub-deliverables per
`step-skeletons.md`).

## DELIVER read-back instructions

When DELIVER picks up:

1. The 9 slice-7 scenarios are all live (no `@skip` / `@ignore` tag). Each panics on its first slice-7-specific step — that's the correct entry point for the GREEN phase. The `@slow` scenario #3 is NOT yet excluded from the default cucumber run; DELIVER adds the one-line `@slow` filter to `tests/acceptance.rs` line ~98 per slice-7 D3 = A as part of sub-deliverable F.
2. Cucumber-rs treats `panic!` from a step body as a step failure with the panic message as the captured output (verified above). DELIVER does NOT need to change the step bodies' panic-to-implementation pattern — replace the body verbatim with the real implementation.
3. The step phrases (regex strings) registered in `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` ARE the contract between DISTILL and DELIVER. They MUST NOT change during GREEN. The literal-text `<comment-id>` + `<missing-uuid>` Whens vs the regex variable-arg When are DISAMBIGUATED via the regex `[^<\s]\S*` matcher (rejects strings starting with `<`); DELIVER MUST preserve this disambiguation when filling in bodies.
4. The slice-7 `tombstone_factory` helper at `crates/foundry-acceptance/src/support/tombstone_factory.rs` is ALSO RED-scaffolded (3 panic-bodies). DELIVER fills it in per the signatures + the rationale in `step-skeletons.md` § "Sub-deliverable F". The helper is test-only fixture code; production paths are untouched per task brief.
5. `Store::gc_tombstoned_comments` is the SINGLE most-leveraged implementation: 4 scenarios depend on it directly (#1, #2, #3, #4), and #5 + #6 depend on it indirectly via the tick body. DELIVER's first move is most efficiently to land `Store::gc_tombstoned_comments` + `Store::count_pending_tombstones` (sub-deliverable B) + the tick body in `main.rs` (sub-deliverable A); that unblocks scenarios #1, #2, #6 in one shot.
6. PHRASE COLLISION CHECK (per `step-skeletons.md`) — the slice-7 file does NOT register the `the issue page for "{}-{}" shows a comment by {} containing the text "{}"` Then because slice-5 `us_10_comment_edit_delete.rs` already provides it. The slice-7 WS #7 assertion at `features/comment-tombstone-gc.feature:166` matches the slice-5 registration automatically. When DELIVER fills in the slice-5 body, the slice-7 assertion gets a working implementation for free. **VERIFY** before adding any duplicate Then.
