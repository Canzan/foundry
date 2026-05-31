# Programmatic Foundry (Feature A) — Upstream Changes / Challenged DISCUSS Assumptions

Owner: solution-architect (Morgan). This file records where DESIGN, grounded in the actual code,
refines or challenges a DISCUSS assumption. It is informational for the upstream (DISCUSS) artifacts
and the user; it does NOT modify those artifacts.

## CHG-1 — "Core might be secretly HTML-shaped" is refined: the store layer is ALREADY neutral

- **DISCUSS framing** (`wave-decisions.md` risk register; `slices/slice-01-json-read.md` learning
  hypothesis; NFR-WEB-BND-05): the headline risk is that `foundry-core` is presentation-coupled, so
  the JSON API might not be able to be a true peer. Slice 1 is framed as the test of "can core feed
  a non-HTML JSON consumer through the same call path the UI uses?"
- **What the code shows** (file:line evidence):
  - `foundry-store` is already presentation-neutral: `Store::list_issues_by_project(project_id) ->
    Vec<IssueRow>` (`crates/foundry-store/src/lib.rs:702`); `insert_issue_with_outbox`,
    `update_issue_state_with_outbox`, `insert_comment_with_outbox`, etc. all return data/units, with
    **zero** `format!`-HTML and **zero** serde-JSON on these paths.
  - `foundry-core` holds only value objects (`ProjectKey`, `IssueKey`) + markdown sanitization
    (`crates/foundry-core/src/markdown.rs`); it contains **no** use-case orchestration to be
    HTML-shaped.
  - The actual presentation coupling is the **inlined orchestration** in `foundry-app` handlers —
    e.g. `issues::submit_create` (`crates/foundry-app/src/issues.rs:46`) interleaves session/authz/
    store/`format!`-HTML in one function; `projects::show_board` (`:208`) calls
    `store.list_issues_by_project` then `render_board(...)`.
- **Refinement (not contradiction)**: the risk is real but its locus moves. Core is neutral; the
  thing that must change so two adapters share rules is the **orchestration**, which DESIGN lifts
  into a shared service seam (ADR-W04). The slice-1 work changes from "prove core can feed JSON" to
  "extract the shared service seam and prove both the HTML board and the new JSON board endpoint call
  the very same function." *(Post-ratification 2026-05-31: that seam now lives in a new
  `foundry-services` crate, not a `foundry-app::services` module — see CHG-3 + ADR-W07. The
  substance of CHG-1 is unchanged.)*
- **Impact on scope/stories**: **none.** US-W05a's elevator pitch, ACs, and the walking-skeleton
  value (a real JSON read served from the same core path, zero HTML, suite green) are unchanged. The
  learning hypothesis is still validated by the same demo; only the internal explanation of *why it
  works* is sharper. No NFR changes. No re-slicing.
- **Action for upstream**: optional — DISCUSS may annotate the slice-1 hypothesis to note that the
  neutrality already exists at the store layer and the proof is the shared-service extraction. Not
  required for DESIGN to proceed.

## CHG-2 — NFR-WEB-INFRA-01's "no new dependency" clause is broken by the ratified JWT choice (2026-05-31)

- **DISCUSS framing** (`nfrs.md` NFR-WEB-INFRA-01): "The extraction adds NO new runtime service (no
  Redis, no Node server, no separate api process), **NO new container, and NO CDN dependency.** Still
  one binary, one Postgres, `docker compose up`." The DESIGN read this (and the surrounding
  one-binary/no-bloat ethos) as an implied **"zero new runtime dependencies"** invariant, and the
  prior recommendation (opaque token + SHA-256) honored it literally (zero new crates).
- **What the ratified decision changes**: on **2026-05-31 the user ratified the machine-token
  mechanism as a JWT signed with Ed25519** (`wave-decisions.md` ADR-W02), OVERRIDING the
  zero-new-deps recommendation. This introduces **ONE new compile-time dependency, `jsonwebtoken =
  "9"` (MIT)**. So the "no new dependency" reading of NFR-WEB-INFRA-01 no longer holds.
- **Reconciliation against the literal NFR text**:
  - NFR-WEB-INFRA-01's **explicitly enumerated** prohibitions — no new runtime *service*, no Redis,
    no Node server, no separate api process, **no new container, no CDN** — are **ALL still
    honored**. `foundry-api` is a new *crate* compiled INTO the one binary, not a process; `docker
    compose up` topology is unchanged (one foundry container + Postgres).
  - The clause that is broken is the **implied "no new dependency"** reading — narrowed to "one new
    *library crate* (`jsonwebtoken`), compiled in, MIT-licensed, on the already-vendored `ring`
    crypto backend." No new *service*, *container*, *CDN*, or *process* — only one new linked crate.
  - Crypto-surface honesty: `ring` (the backend) is **already in `Cargo.lock`** via rustls, so the
    net-new *crypto* code surface is minimal; the genuinely new crates are `jsonwebtoken` + its JWT
    helpers (`simple_asn1`, `pem`). `cargo deny check` must be re-run; no license-set change is
    anticipated (MIT is allowed).
- **Impact on scope/stories**: **none.** No story, AC, or other NFR changes. The trade-off is a
  deliberate, ratified one (standards-based asymmetric credential) with the cost stated plainly
  (one new dep + alg-confusion footgun closed by pinning `alg=EdDSA` + new key-management surface).
- **Action for upstream**: DISCUSS may wish to **tighten the wording of NFR-WEB-INFRA-01** to
  distinguish "no new runtime *service*/container/CDN/process" (a hard invariant, still met) from
  "minimize new *library* dependencies" (a preference, now consciously spent once on `jsonwebtoken`
  per user ratification). DESIGN does not edit the discuss artifact; this note records the
  reconciliation. Also note the two new env vars (`MACHINE_TOKEN_PUBLIC_KEYS`,
  `MACHINE_TOKEN_SIGNING_KEY`) as a new secret-delivery surface parallel to `SESSION_SECRET`
  (NFR-WEB-INFRA-02 "no build-time secrets" is unaffected — these are runtime config, like the
  existing `SESSION_SECRET`/`DATABASE_URL`).

## CHG-3 — topology ratified to a crate split now (2026-05-31; informational)

The user ratified a **new `foundry-api` crate** (plus a forced `foundry-services` crate) for Feature
A, OVERRIDING the proposed `api`-module-inside-`foundry-app` recommendation (ADR-W01/W07). This does
not contradict any DISCUSS assumption — DISCUSS sketched the crate split as the end-state and only
*sequenced* it loosely; the user simply chose to pay part of that cost now for the stronger
crate-graph-enforced boundary. No story/NFR change; the only consequence is a larger Feature-A blast
radius (one new `AppState` field across ~5 construction sites + the service-extraction move), honestly
recorded in `architecture.md` §Composition & harness blast radius.

## No other DISCUSS assumptions challenged

All other DISCUSS decisions (D5 additive auth, D9 scope = Feature A only, one-binary/no-Redis/
no-new-service constraints, render-contract preservation, boundary-guard requirement) are honored
exactly as written. CHG-1 (core neutrality locus), CHG-2 (the JWT new-dep reconciliation), and CHG-3
(topology informational) are the only items requiring upstream awareness; none changes scope or
stories.
