# CONTEXT

## Current Task

**v0.16.0** shipped 2026-10-17/18 (`doctor set-role`, last-admin removal guard, ammonia 4.1.8 for RUSTSEC-2026-0373). Tag on `c2999cd`; dev and prod both show v0.16.0 · c2999cd. `main` is clean and matches origin.

## Key Decisions

- The workspace row `FOR NO KEY UPDATE` lock serializes role changes and member removals, so two admins can't each demote or remove the other.
- Removing a workspace's only admin is refused (409, "Make someone else an admin first"), checked before the last-lead logic.
- Tags are annotated, not signed (no GPG key); the local gate needs `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false`.

## Next Steps

- Still open from `finalize-owed.md`: rerun `cargo mutants`.
- Optional: move the scoped-token check (copied in 5 service modules) onto `Principal`.
