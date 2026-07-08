# ADR-004: CSS re-hash procedure under the hand-maintained content-hash constraint

## Status
Accepted

## Context
Styling is a single hand-written stylesheet served under a **content-hash-in-filename** convention
(ADR-B03): `static/css/foundry.<sha256-prefix>.css`, paired with `Cache-Control: immutable`. There is
**no build step** that regenerates the hash — the hash is committed in the filename, referenced from
`base.html`, and asserted in a test. Adding the sidebar (`.app-shell`, `.sidebar`, …) changes the
file's content, so under an `immutable` cache the URL **must** change or browsers pin the stale
stylesheet for a year (the exact bug `lib.rs:271-288` guards against). This feature must therefore
perform a correct manual re-hash.

**Grounded blast-radius audit (verified by reading the tree, correcting the task brief):**
- `static/css/foundry.4c43c2a8.css` — the file to edit + rename.
- `base.html:5` — `<link rel="stylesheet" href="/static/css/foundry.4c43c2a8.css">` — hardcoded.
- `crates/foundry-app/src/lib.rs:284` — `static_cache_control_value("/static/css/foundry.4c43c2a8.css")` — the **only** other hardcoded literal of the full hash.
- `crates/foundry-app/src/projects.rs:958` — **NOT** hardcoded: it asserts only the prefix
  `href="/static/css/foundry.` and suffix `.css">` (hash-agnostic). **No edit needed** — the brief's
  claim that this line pins the hash is incorrect.

## Decision
Treat the re-hash as a **first-class, explicit DELIVER task**, performed as an atomic step:

1. Append the sidebar rules to `foundry.4c43c2a8.css`.
2. Compute the new content hash (`sha256` of the edited file, take the committed prefix length).
3. `git mv` the file to `foundry.<newhash>.css` (old file removed — no orphan, matching the
   `board-new-issue` precedent that bumped `386eb83b` → `4c43c2a8`).
4. Update the two hardcoded literals: `base.html:5` and `lib.rs:284`.
5. Leave `projects.rs:958` untouched (verified hash-agnostic).
6. Run the suite; add/keep an **asset-resolution probe** — a `#[test]` (or the existing Feature-B
   `xtask check-assets`) that parses `base.html`'s `<link href>` and asserts the referenced
   `foundry.<hash>.css` exists on disk — so a forgotten rename reds CI (self-correcting guard,
   Earned-Trust Principle 12).

## Alternatives considered

1. **Version query string (`foundry.css?v=N`) instead of a renamed file.**
   *Rejected:* changing conventions mid-project; some proxies ignore query strings for caching, and
   ADR-B03 already weighed and rejected this (its Option 4b) precisely because it lacks the
   self-correcting "wrong URL → 404 → probe reds" safety of a renamed file.

2. **Introduce a build step that computes the hash automatically.**
   *Rejected:* violates the air-gap / no-toolchain posture (DB6, ADR-B03). The whole convention exists
   to avoid a build pipeline; a 4-line manual edit guarded by a CI probe is the accepted trade.

3. **Drop `immutable` caching so the filename can stay stable.**
   *Rejected:* regresses the cache NFR (ADR-B03 Option 4c) — every deploy would risk serving stale CSS
   until revalidation. The hashed-filename + immutable pairing is the point.

## Consequences
- **Positive:** correct cache invalidation with zero build step; only **two** literals change (fewer
  than the brief assumed — `projects.rs` is safe). The rename removes the old file, so no stale asset
  ships.
- **Positive:** the asset-resolution probe makes the one failure mode (forgetting the rename or a
  literal) a **red CI**, not a silent year-long stale-cache bug.
- **Negative:** the re-hash is a manual, order-sensitive step; it must be done as one commit
  (edit + rename + both literals together) or an intermediate state links a non-existent file. Called
  out explicitly so DISTILL/DELIVER sequences it as a single atomic task.
- **Constraint recorded:** the hash remains **hand-maintained** (ADR-B03). This feature does not
  introduce automation; it follows the established manual discipline.
</content>
