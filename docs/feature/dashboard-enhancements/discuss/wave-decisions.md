# DISCUSS Decisions — dashboard-enhancements

## Key Decisions

- **[D1] Greeting fallback on load failure**: if the identity/greeting query errors, render a neutral
  greeting ("Welcome back.") with status 200 rather than 500. Rationale: a landing page must not hard-fail
  on a transient read; the projects list already uses this degrade-not-500 posture (`51ba981`).
  (see: `user-stories.md` US-01 AC-01.4, `acceptance-criteria.md`)
- **[D2] Sign-out CSRF ripples the response type**: adding the sign-out form requires a valid double-submit
  token, so `dashboard_root` must mint a CSRF cookie and return `(SET_COOKIE header, Html)` — mirroring
  `admin_tokens::show_index`. Sequenced last among the additive slices (slice 03) to isolate that change.
  (see: `story-map.md`, seam `signin.rs:305 ensure_csrf_cookie`)
- **[D3] Style hash-bump is manual (no hashing pipeline)**: the repo has no CSS-hashing build step; the
  `foundry.870985fc.css` hash is a hand-set string. Slice 04 renames the file to a new deterministic hash
  and updates `base.html` in the same commit. Verify: old filename 404s, new one 200s.
  (see: `user-stories.md` US-04)
- **[D4] Copy contract**: keep `<h1>Foundry</h1>`. US-R04's welcome sentence is intentionally REPLACED by
  the personalized greeting (no test asserts the sentence; the `<h1>` is the durable anchor).
  (see: `requirements.md` § Constraints)
- **[D5] Tenancy by session only**: US-01 and US-03 derive identity from `SessionUser {user_id,
  workspace_id}` — never a path/query id — so no new `check_arch` LAYER-1e allow-list line is introduced
  (consistent with the shipped dashboard). (see: `requirements.md` § Constraints)
- **[D6] Retroactive coverage is a first-class slice**: the base dashboard shipped untested (`51ba981`);
  slice 04 backfills the store-query + acceptance coverage rather than leaving it as latent debt.
- **[D7] SSOT model NOT adopted**: per user direction (2026-07-04), this feature follows the repo's
  established multi-file DISCUSS convention (matching all 27 prior features), NOT the newer
  `docs/product/` + `feature-delta.md` model the updated nWave 3.21 skill expects. No repo migration.

## Requirements Summary

- **Primary need**: a signed-in user orients (identity + workspace), navigates to role-appropriate tools,
  and can sign out — from a dashboard that is styled canonically and protected by tests.
- **Walking skeleton**: already shipped (`51ba981`); this feature is 4 thin slices on top.
- **Feature type**: user-facing (UI), brownfield.

## Constraints Established

- No migration, no new crate; all slices reuse shipped seams (see `requirements.md` seam table).
- ADR-002 tenancy: session-scoped identity only.
- US-R07: full-page HTML stays in templates extending `base.html`.

## Scope Assessment: PASS

Right-sized: 5 stories → 4 slices, 1 bounded context (the web adapter + store), no new integration points,
< 1 day each. No split needed.

## Upstream Changes

- None. No DISCOVER/DIVERGE artifacts exist for this feature (brownfield increment); requirements are
  grounded in the shipped code and the user's explicit slice list.
