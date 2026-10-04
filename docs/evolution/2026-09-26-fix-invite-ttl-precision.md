# Evolution: fix-invite-ttl-precision (the invite TTL assertion compared µs with ns)

**Finalized:** 2026-09-26.
**Commit:** `2274835` (fix, Step-ID 01-01), docs in `e91f24f`. One DES-monitored step. First
released in v0.6.0. Test-only, so it has no CHANGELOG entry.
**Origin:** GitHub CI run 36276820914 on `0cd73c3`. Run through `/nw-bugfix`; the user approved
option A on 2026-09-26. RCA: `docs/feature/fix-invite-ttl-precision/rca.md`.

## Defect: test-only, not user-facing

`feature_member_invites.rs` `sees_confirmation_link_7_days` failed on Linux CI with `ttl_days = 6`
(`expires_at = …29.817272`, `issued_at = …29.817272728`). It never failed on macOS, so it had no
local repro. CI has been red since June (1 green of the last 200 runs, per the RCA), so this
failure was never triaged on its own.

**Production is not affected:** the invite HMAC signs whole seconds (`foundry-auth/src/lib.rs`),
and the `expires_at > now` check is off by under 1µs at most.

## Root cause

`expires_at` was read back from Postgres `timestamptz` (microseconds; sqlx truncates), `issued_at`
was the in-memory `MockClock` value (nanoseconds). The Background seeded the clock from
`OffsetDateTime::now_utc()`: nanoseconds on Linux, whole microseconds on macOS. The difference was
7 days minus the sub-µs nanos, and `whole_days()` floored it to 6. The µs round-trip rule had been
fixed once for a store test (`6b6bc0c`) but never generalised to the acceptance steps.

## Fix (acceptance crate only)

- New `crates/foundry-acceptance/src/support/pg_time.rs`: `to_pg_micros` truncates an
  `OffsetDateTime` to microseconds (the `6b6bc0c` rule).
- The step asserts `expires_at - to_pg_micros(issued_at) == Duration::days(7)` exactly, instead
  of a floored `whole_days() == 7`.
- No production code and no Gherkin changed.

## Regression

The member-invites Background step (`dana_signed_in_admin`) seeds the clock with deliberate
sub-microsecond nanos (`microsecond * 1000 + 789`), reproducing the Linux clock on every OS. The
execution log records RED_ACCEPTANCE as PASS (the unmodified assertion failed locally), then GREEN.
RED_UNIT was skipped as NOT_APPLICABLE: `to_pg_micros` is a one-line truncation exercised by the
789ns seed.

## Gates

- Execution log: PREPARE, RED_ACCEPTANCE, GREEN and COMMIT executed; RED_UNIT skipped as above.
- The roadmap's criteria required member-invites and invite-accept to stay green and fmt, clippy
  `-D warnings` and check-arch to pass. **No results for those runs are recorded** in the RCA,
  the execution log or the commit.
- No mutation run or full `cargo xtask ci` run is recorded for this fix.
- Whether GitHub CI went green after `2274835` is not recorded here.

## Follow-ups

- **User-approved, separate bugfix:** investigate CI health, including the 45-minute timeout
  cancellations and `us-r05-attachments.feature:55`. No commit for it is found in `git log`.
