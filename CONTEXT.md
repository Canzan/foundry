# CONTEXT

## Current Task

**Feature A "Programmatic Foundry" shipped to trunk** (`main` at `ba791ee`). Full nWave pipeline (DISCUSS→DESIGN→DISTILL→DELIVER): JSON `/api/v1` read+write API + JWT/Ed25519 machine-token auth, in new `foundry-api` + `foundry-services` crates over a presentation-neutral core, with a `cargo xtask check-arch` boundary guard. One binary, one new dep (`jsonwebtoken`). 135/135 default-lane scenarios green; foundry-auth mutation 81.8%.

## Key Decisions

- **Ratified split**: shipped Feature A (the JSON API); **Feature B** (htmx web-tier templating + htmx 2) deferred to its own `/nw:new`, reusing the `foundry-services` seam.
- **Auth**: JWT/Ed25519 (user override of opaque-token), `jti` denylist revocation, env keys, `alg=[EdDSA]` pinned, `iss/aud/exp/nbf` validated. Boundary guard enforces `foundry-api ⊀ foundry-store`.
- **Trunk-based workflow** recorded in `AGENTS.md` + memory: commit to `main`, no PRs, CI is not a commit gate, validate with `cargo xtask ci`.

## Next Steps

- **Open**: `cargo xtask ci` is green on every stage EXCEPT US-03 `@needs-pgclient` backup. `brew link --force libpq` (pg_dump→18.4) fixed the *capture* version-mismatch, but the `@all` re-run then HUNG on the *restore* round-trip (~26h, killed; 10 leaked containers cleaned). Investigate: install a matching **postgresql@16** client (not 18) and/or check Docker resource exhaustion before re-running `@all`. Feature A itself is green (default lane 135/135).
- Optionally delete the `feature/web-tier-extraction` branch (work is on trunk).
- Start Feature B via `/nw:new` when ready.
