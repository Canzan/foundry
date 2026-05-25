# RED classification — slice 5 (comment-edit-delete)

Per nw-distill § "Pre-DELIVER fail-for-the-right-reason gate" (Rust
adaptation). After scaffolds + .feature landed, the slice-5 scenarios
were executed against the RED scaffold step bodies; classification
below.

Command used:
```bash
cargo check -p foundry-acceptance --tests          # gate 1: compile
/.../target/debug/deps/acceptance-* -t "@slice5"   # gate 2: run only @slice5 scenarios
```

Gate 1 result: `Finished dev profile [unoptimized + debuginfo] target(s) in 10.24s` — compile passes.
Gate 2 result: `1 feature, 10 scenarios (10 failed), 70 steps (60 passed, 10 failed)`.

Each failure is the canonical RED scaffold panic emitted from the
slice-5 step bodies in `crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`.
Captured output verbatim: `Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this`.

## Per-scenario classification

All 10 scenarios fail at their first slice-5-specific Given (the
"Mei has previously posted a comment on …" step), because that's the
shortest path from the inherited slice-2 Background to the new slice-5
behaviour. Each entry below records: scenario title → classification
(category) → step that fired the panic.

1. `Comment author edits their own comment and the updated text replaces the original in the thread` → **RED (MISSING_FUNCTIONALITY)** → `Given Mei has previously posted a comment on "AUTH-3" with body "Looked into this — …"` (step body panics with scaffold message; no production handler / store method exists yet)
2. `A non-author cannot edit someone else's comment` → **RED (MISSING_FUNCTIONALITY)** → same Given step
3. `Workspace admin deletes any comment and remaining viewers see it disappear from the thread` → **RED (MISSING_FUNCTIONALITY)** → same
4. `Comment author deletes their own comment` → **RED (MISSING_FUNCTIONALITY)** → same
5. `An open subscriber receives a CommentEdited event when another viewer edits an existing comment` → **RED (MISSING_FUNCTIONALITY)** → same
6. `An open subscriber receives a CommentDeleted event when another viewer deletes a comment` → **RED (MISSING_FUNCTIONALITY)** → same
7. `PATCH on a comment that has already been soft-deleted returns 410 Gone with an htmx fragment` → **RED (MISSING_FUNCTIONALITY)** → same
8. `DELETE on a comment that has already been soft-deleted returns 410 Gone` → **RED (MISSING_FUNCTIONALITY)** → same
9. `The issue page lists only non-deleted comments` → **RED (MISSING_FUNCTIONALITY)** → same
10. `Author cancels the edit and the original card is returned by the server` → **RED (MISSING_FUNCTIONALITY)** → same

## Failure-mode categories

- **MISSING_FUNCTIONALITY** (correct RED): 10 of 10 — slice-5 production
  handlers / store methods are not yet implemented; the step body
  panics with the scaffold marker. DELIVER's responsibility.
- **IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE** (wrong RED): 0 of 10.
- **WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT** (wrong shape): 0 of 10.

All 60 background steps pass green (slice-2 inherited workspace +
team-member + project + issue + sign-in seed). No infrastructure or
fixture failure was observed; the only failures are the deliberate
scaffold panics. Pre-DELIVER gate: **PASSED** — proceed to DELIVER
under ADR-025 D2 (DELIVER RED phase = unskip these scaffolds, write
PBT unit tests, then implement).

## DELIVER read-back instructions

When DELIVER picks up:

1. The 10 slice-5 scenarios are all live (no `@skip` / `@ignore` tag).
   Each panics on its first slice-5-specific Given — that's the
   correct entry point for the GREEN phase.
2. Cucumber-rs treats `panic!` from a step body as a step failure with
   the panic message as the captured output (verified above). DELIVER
   does NOT need to change the step bodies' panic-to-implementation
   pattern — replace the body verbatim with the real implementation.
3. The step phrases (regex strings) registered in
   `crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`
   ARE the contract between DISTILL and DELIVER. They MUST NOT change
   during GREEN. If a phrase reads awkwardly during implementation,
   surface it as a DELIVER → DISTILL retro item, not a unilateral
   rename.
4. The scenario count is 10 (above the 7-9 prompt cap). Per
   `proposals.md` § "Scope confirmation", scenarios 7+8 (PATCH-on-
   tombstone + DELETE-on-tombstone) could merge into a Scenario
   Outline if the user prefers the smaller surface; flagged for user
   pick.
