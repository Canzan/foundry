# Mutation Report — canzan-theme-system (DELIVER, step 05-03)

- **Tool**: cargo-mutants 25.3.1 | **Date**: 2026-08-30 | **Gate**: kill rate >= 80% of viable mutants
- **Scope**: the production Rust this feature added, selected with `--file xtask/src/check_arch.rs --in-diff <dd24fcb..HEAD diff of that file>` (per-feature strategy, project `CLAUDE.md`).
- **Verdict**: **PASS — 191/218 viable mutants killed (87.6%)**, 27 survivors, 0 flakes.

```
cargo mutants --file xtask/src/check_arch.rs --in-diff feature.diff \
  --jobs 3 --timeout 300 -- --package xtask
220 mutants tested in 11m 22s: 27 missed, 186 caught, 2 unviable, 5 timeouts
```

## Why this file is the whole mutatable surface

The feature touched six Rust files. Five are not mutation subjects and this is
stated rather than assumed:

| File | Δ | Why not mutated |
|---|---|---|
| `xtask/src/check_arch.rs` | +1152 | **The subject.** `check_static_asset_integrity` (R1/R2/R3) and `check_stylesheet_token_seam` (S1/S2) are new code with real branching. |
| `crates/foundry-app/src/lib.rs` | +6/-6 | Three string literals naming the hashed stylesheet, inside `#[cfg(test)]` assertions. No branching. |
| `crates/foundry-acceptance/src/steps/feature_canzan_theme.rs` | +3848 | Test code. |
| `crates/foundry-acceptance/src/support/browser_harness.rs` | +109 | Test harness. |
| `crates/foundry-acceptance/src/lib.rs`, `tests/acceptance.rs` | +3 | Module registration. |

So the feature is **NOT** recorded as unmutatable. It has a real mutatable
surface and that surface was mutated.

## Kill rate, both ways

| Basis | Killed | Viable | Rate |
|---|---|---|---|
| Timeouts counted as kills (repo convention, `board-lane-management` precedent) | 191 | 218 | **87.6%** |
| Timeouts excluded entirely | 186 | 218 | **85.3%** |

Both clear the gate, so the convention is not doing load-bearing work here. The
5 timeouts are all `+=` → `*=` / `-=` on a byte-scanner cursor
(`static_references:902`, `strip_css_block_comments:1374`, `colour_literals`
×3), each of which turns a bounded forward scan into a non-terminating loop that
the suite detects by hanging into the 300s deadline. Unviable = 2 (does not
compile).

## The 27 survivors, and what they have in common

Twenty-five of the twenty-seven are inside the four hand-written byte scanners
that S1 and R1/R2 are built on — `static_references`, `content_hash_segment`,
`css_regions`, `colour_literals`, `is_identifier_byte`, `line_starts`,
`is_sha256_hex`. They are off-by-one and boundary mutations
(`<` → `<=`, `+` → `-`, `&&` → `||`) on cursor arithmetic. They survive for one
structural reason: **the gold tests assert the VERDICT, not the offsets.** A
planted violation is still found when a cursor lands one byte early, and a clean
tree still passes when it lands one byte late, so the mutant changes an
intermediate the assertion never reads.

The two that are not scanner arithmetic:

| Mutant | Reading |
|---|---|
| `check_arch.rs:46` `run -> ExitCode` with `Default::default()` | `ExitCode::default()` **is** `SUCCESS`, so this mutant makes check-arch always pass. It survives because the gold tests call the rule functions directly against staged trees rather than driving `run()`. The real guard on `run()` is `cargo xtask ci` itself, which is not in the `-p xtask` test command this run used. Recorded, not papered over. |
| `is_sha256_hex -> true` (`:1039`) | R3 would accept a malformed hash string. The mutant survives because every gold-test fixture uses well-formed hashes; the rule still fails on a hash that is well-formed but WRONG, which is the case R3 exists for. |

**Not fixed here, deliberately.** Killing scanner-arithmetic mutants means
asserting on byte offsets, which couples the tests to the scan strategy the ADR
explicitly leaves as an implementation detail ("this is a scanner, not a CSS
parser"). The gate is cleared with 7.6 points of margin; spending the budget on
offset assertions would buy a worse test suite. The two non-arithmetic survivors
are recorded above so the next reader inherits the knowledge rather than the
surprise.
