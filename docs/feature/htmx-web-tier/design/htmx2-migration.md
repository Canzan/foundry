# htmx Web Tier (Feature B) — htmx 1→2 Migration (Slice 4 / US-B05)

Owner: solution-architect (Morgan). Covers Open Decision #3 (htmx 2 approach + version pin).
Interaction mode: **Propose**. Companion: `architecture.md`, `assets.md`, `render-contract.md`,
`wave-decisions.md` (ADR-B04). DISCUSS authority: DB4 (htmx 1→2 is a DEDICATED final slice, NOT
per-surface) and DB5 (the 2.x pin is DESIGN). Slices 1-3 move `hx-*` directives into templates
**as-is, no version bump**; this slice is the ONLY one that changes directive convention or the htmx
version.

## The actual migration surface (grounded in code, small)

The DISCUSS "mixed prefixes" framing is refined by reading the handlers. The COMPLETE set of ACTIVE
htmx directives Foundry uses today:

| Directive | Where | Purpose |
|---|---|---|
| `hx-swap-oob="beforeend:[data-column='backlog']"` | `issues.rs:285` (create-card OOB) | append new issue card to Backlog |
| `hx-swap-oob="beforeend:[data-comment-list]"` | `comments.rs:857` (comment OOB) | append new comment card |
| `hx-get` + `hx-target="#comment-{id}"` + `hx-swap="outerHTML"` | `comments.rs:796` (Edit button), `:265` (Cancel) | load edit-form / cancel back to card |
| `hx-patch` + `hx-target` + `hx-swap="outerHTML"` | `comments.rs:262` (edit-form submit) | submit edit, swap card |
| `hx-delete` + `hx-target` + `hx-swap="outerHTML"` | `comments.rs:805` (Delete button) | soft-delete, swap card out |
| `HX-Request` header read | `issues.rs:261`, `comments.rs:658`, `projects.rs:111` | branch full-page vs fragment |
| `hx-csrf` / `HX-CSRF` header | `csrf.rs:29` (middleware reads it) | CSRF on htmx mutating calls |

That is the entire surface: `hx-swap-oob`, `hx-get`, `hx-post`(none — forms post natively),
`hx-patch`, `hx-delete`, `hx-target`, `hx-swap`, the `HX-Request` request header, and the `hx-csrf`
request header. **All bare `hx-*`** — there is no `data-hx-*` *directive* in use. The
`data-hx-fragment`/`data-column`/`data-comment-list`/`data-issue-key` attributes are **passive
scraper markers, NOT htmx directives** (DB4, verified) — they are render-contract and MUST NOT be
touched by normalization (`render-contract.md` §"data-* markers").

## htmx 2 breaking changes relevant to Foundry's usage

Consulting the htmx 2 migration guidance conceptually (htmx.org migration notes), the breaking
changes that *could* touch Foundry — and the verdict for each:

| htmx 2 change | Affects Foundry? | Action |
|---|---|---|
| `hx-swap-oob` core semantics retained; the OOB attribute and `beforeend:<selector>` modifier still supported | **YES — this is Foundry's main directive** | Verify the two OOB swaps (create-card → `[data-column='backlog']`, comment → `[data-comment-list]`) still fire under htmx 2; regression scenario each. Expected: unchanged. |
| Default `hx-swap` style / `swap` timing & settle defaults tweaked | Foundry pins `hx-swap="outerHTML"` explicitly everywhere | Low risk — explicit `outerHTML` is unchanged in htmx 2. Verify the edit/delete/cancel `outerHTML` swaps. |
| `hx-on` attribute syntax change (htmx 1 `hx-on` → htmx 2 `hx-on:event`) | **NO** — Foundry uses no `hx-on` | None. |
| Removal of the legacy `htmx.config` / deprecated extensions; `path-deps`, `class-tools` etc. now separate extension files | **NO** — Foundry uses no htmx extensions | None — confirm the vendored blob is core htmx 2 with no extension dependency. |
| `HX-*` response/request header names (e.g. `HX-Request`, `HX-Trigger`) | `HX-Request` read unchanged in htmx 2 | Verify `HX-Request: true` branching still works (it does — header name unchanged). |
| IE / legacy XHR drops, `withCredentials` defaults | Foundry is modern-browser, same-origin | None. |
| `hx-csrf` is **not a native htmx attribute** — Foundry's CSRF is a custom header set by an Alpine/htmx hook reading the non-HttpOnly cookie | The hook lives in the vendored JS / Alpine, not htmx core | Re-verify the hook fires under htmx 2 + Alpine; the `hx-csrf`/`x-csrf-token` *header* contract (`csrf.rs`) is server-side and UNCHANGED (NFR-WEBB-COMPAT-03). |

**Assessment: the migration surface is small and low-risk.** Foundry uses core htmx directives only
(`hx-get/patch/delete/target/swap/swap-oob`), no `hx-on`, no extensions — exactly the directives htmx
2 preserves. The real work is *verification*, not rewriting.

## Decision #3 — direct normalize-and-bump vs a staged compat path

**Option 3a — direct normalize-and-bump (one atomic slice). RECOMMENDED.**
In Slice 4, after all surfaces are templated: (1) normalize the active directives to one consistent
convention across the partials (they are already all bare `hx-*` — "normalization" here is mostly
*confirming* consistency and centralizing any incidental variation, e.g. consistent
`hx-swap="outerHTML"` ordering, into the partials); (2) replace the vendored htmx blob with one
pinned htmx 2.x file; (3) run the regression scenario set — one green scenario per hx-driven
interaction (create-card OOB, comment edit, delete, cancel, SSE fragment) — proving non-regression;
(4) assert every `data-*` marker is byte-stable.

- Pro: matches DB4 exactly (dedicated slice, not per-surface); **one atomic change with one
  regression pass**, no mixed-version window. The surface is small enough (above) that a single bump
  is genuinely low-risk. The version is pinned once, after the directives are already consistent and
  centralized in partials. Cleanest.
- Con: the bump and the normalization land together — but since the directives are already
  consistent and the htmx-2 deltas don't touch them, this is not the risk it would be in a sprawling
  codebase.

**Option 3b — staged compat path (run htmx 1 and 2 behind a flag / dual-vendor, migrate directives
incrementally, then drop htmx 1).**
- Pro: would let directives migrate one at a time with htmx 1 as a fallback.
- Con: **over-engineered for this surface.** It creates exactly the mixed-version window DB4 chose to
  avoid, doubles the vendored blob, and adds a flag with no payoff given the directive set is a
  handful of core attributes htmx 2 already supports. Rejected (Principle 8).

**Recommendation: 3a** — direct normalize-and-bump as one atomic, fully regression-gated slice. The
small, core-only directive surface makes a staged path pure overhead.

## Version pin (Open sub-decision, recommendation)

**Recommend pinning the latest stable htmx 2.x at implementation time** (e.g. `htmx 2.0.4` —
the crafter pins the then-current 2.0.x patch release and records it). Rationale:
- The 2.0.x line is the stable, widely-deployed htmx 2 series; a `.x` patch carries fixes without API
  change. Pinning the latest 2.0.x at build time maximizes fixes while staying on the stable major.
- Vendored as a single pinned `.min.js` blob in `static/vendor/` with provenance + sha256 in
  `VENDOR.md` (`assets.md`). Exactly one htmx file (US-B05 AC: "exactly one htmx file and its version
  is recorded").
- **Alpine** is pinned the same way (latest stable 3.14.x line); Alpine is not changing major, so
  this is a straightforward vendor+pin, not a migration.

The exact patch version is a pin-at-implementation detail (recorded in `VENDOR.md` + this doc by the
crafter), not an architecture decision — flagged for the user only as "DESIGN recommends latest
stable htmx 2.0.x; ratify the series."

## Regression gating (US-B05 ACs)

- A green scenario per hx-driven interaction AFTER the bump: create-card OOB swap into Backlog;
  comment post-append OOB; comment edit (`hx-patch` → card swap); comment delete (`hx-delete` → card
  removal); cancel (`hx-get` → card restore); the SSE fragment path. The suite *exercises* htmx 2
  against the real swaps (Earned Trust — `architecture.md` §htmx-2 behavioral probe).
- A render-contract test asserts every `data-*` marker (`data-hx-fragment`, `data-column`,
  `data-comment-list`, `data-issue-key`) is byte-unchanged across the bump (US-B05 scenario 4).
- The FULL acceptance suite stays green after the bump (NFR-WEBB-COMPAT-01).

## Sequencing recap (DB4)

Slices 1-3 move directives into templates AS-IS (htmx 1 behavior, no version change). Slice 4 is the
ONLY slice that changes directive convention or htmx version, keeping the bump atomic and
regression-tested — and it depends on US-B01/B03/B04 so every surface is templated and the directive
set is centralized in a few partials before the bump.
