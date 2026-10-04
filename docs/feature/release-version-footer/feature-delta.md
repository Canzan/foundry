# Feature Delta — release-version-footer

Show the running release version at the bottom of every page, so an operator
can confirm a deploy landed without shelling into the cluster. Shipped together
with the v0.5.0 release, whose tag is also the end-to-end test of the homelab
delivery path (Forgejo mirror → zot → Argo CD Image Updater; homelab
`modules/foundry`, ADR-059).

Process: **lean by user decision (2026-09-26)** — one story, one acceptance
scenario, one TDD step plus the release chore. No DESIGN wave, no ADR.

## Wave: DISCUSS

### [REF] Persona / JTBD

`persona-instance-operator` (Priya). Job: *when I push a release tag, I want to
see which release the instance is actually serving, so I know the rollout
finished without checking Argo CD or the pod image.*

### [REF] Locked Decisions

| ID | Decision |
|---|---|
| D1 | A site-wide footer in `base.html`, on every page including sign-in (visible without an account). |
| D2 | The text is `Foundry v<version>`, where `<version>` is the `foundry-app` crate version compiled into the binary (`env!("CARGO_PKG_VERSION")`). The crates are bumped together at release, so it equals the git tag without the build pipeline passing anything in. |
| D3 | Muted, small, non-interactive; uses existing `--cz-*` tokens only; does not change any page's layout or scroll (the board's full-height layout included). A CSS change carries the D18 stylesheet re-hash. |
| D4 | Release is **v0.5.0** (minor: adds the footer and slice 01 of `card-pointer-drag`, mouse drag on Pointer Events). |

### [REF] User Story US-RVF-01

As Priya, I want the version the server is running shown at the bottom of every
page, so I can tell a release has deployed.

#### Elevator Pitch
Before: after tagging a release she cannot tell from the app whether the new image is serving.
After: open `https://foundry.<domain>/sign-in` → sees `Foundry v0.5.0` at the bottom of the page.
Decision enabled: the rollout is done (or still pending) — no need to inspect Argo CD.

#### Acceptance Criteria
- AC-1: every server-rendered full page (signed-out sign-in page and a signed-in board page at least) contains the footer text `Foundry v` followed by exactly the binary's crate version.
- AC-2: the footer appears once per page, below the main content, and is not rendered inside htmx fragment/OOB responses.
- AC-3: no existing acceptance scenario or layout check regresses.

### [REF] Out of Scope

Git SHA or build date in the footer; an API endpoint for the version; changing the release pipeline.

## Wave: DELIVER

> Finalized 2026-10-04 from `deliver/roadmap.json`, `deliver/execution-log.json` (2 steps),
> commits `0cd73c3`, `8f06528` and `00dc174`, and the CHANGELOG v0.5.0 entry. The work was delivered
> 2026-09-26 and tagged **v0.5.0**. Evolution record:
> `docs/evolution/2026-09-26-release-version-footer.md`.

### [REF] Implementation summary

`base.html` renders `<footer class="site-footer">Foundry v{{ crate::views::RELEASE_VERSION }}</footer>`
as the immediate next sibling of the content block. That puts it on every full page, signed in or out
(D1). `views::RELEASE_VERSION` is `env!("CARGO_PKG_VERSION")` of `foundry-app` (D2), and no page
struct changed. htmx fragments never extend `base.html`, so they carry no footer.

The styling is `.site-footer`: 11px, `--cz-muted`, right-aligned, and existing tokens only (D3).
`.app-shell + .site-footer` pulls the footer up 16px into the shell's bottom padding, with
`pointer-events: none`, so no authed page gains a scrollbar. The stylesheet was re-hashed `438142d2` →
`f9143163` (D18).

Step 01-02 (`8f06528`) bumped every crate 0.4.0 → 0.5.0 and closed the CHANGELOG (D4).

### [REF] Files modified

- **Production (`0cd73c3`):**
  - `crates/foundry-app/templates/base.html`: footer and stylesheet href.
  - `crates/foundry-app/src/views.rs`: `RELEASE_VERSION`.
  - `crates/foundry-app/static/css/foundry.438142d2.css` → `foundry.f9143163.css`: +22 lines.
  - `crates/foundry-app/static/VENDOR.md`: hash row and notes.
  - `crates/foundry-app/src/lib.rs`: only the three cache-policy test literals.
- **Tests (`0cd73c3`):**
  - NEW `crates/foundry-acceptance/tests/features/release-version-footer.feature`.
  - NEW `crates/foundry-acceptance/src/steps/feature_release_version_footer.rs`.
  - Registration in `src/lib.rs` and `tests/acceptance.rs`.
- **Release (`8f06528`):** every `crates/*/Cargo.toml`, `Cargo.lock` and `CHANGELOG.md`.
- **Docs (`00dc174`):** `RELEASING.md` (crate bump convention and "Verifying a deployment"), plus
  this file, `roadmap.json`, `execution-log.json` and `.develop-progress.json`.

### [REF] Scenarios green count

There are 3 scenarios in `release-version-footer.feature` (`@rvf`, HTTP lane): sign-in page, board
page, and new-issue modal fragment. The `0cd73c3` message records the `rvf` lane green. The scenario
and step counts were not recorded. The preamble above says "one acceptance scenario"; three shipped.
0 new unit tests (RED_UNIT SKIPPED, NOT_APPLICABLE).

### [REF] DoD check

DISCUSS defines no separate DoD, so this checks the acceptance criteria of US-RVF-01:

1. **AC-1, the footer reads exactly the crate version on sign-in and board: PASS.** Scenarios 1 and 2
   assert this, with the oracle reading `foundry-app/Cargo.toml`.
2. **AC-2, the footer appears once, below the content, and not in fragments: PASS for the asserted
   cases.** The oracle checks that the footer occurs once, as the last content child of `<body>`, and
   that the new-issue modal fragment has none. OOB responses (named in the roadmap 01-01 criterion) are
   not separately asserted.
3. **AC-3, no regression: PASS as recorded.** The rvf, canzan-theme-system, pwa-mobile-rendering, cdf,
   cpd, kb and blr lanes are green. No full-suite `cargo xtask ci` run is recorded.

### [REF] Demo evidence

On 2026-10-04 the footer read `Foundry v0.7.0` on https://foundry.unintelligent-design.us/sign-in
(dev) and https://foundry.jeffbailey.us/sign-in (prod). It was used to verify the v0.6.0 through
v0.7.0 rollouts, which is the Elevator Pitch's "After" in use.

### [REF] Quality gates

| Gate | Outcome |
|---|---|
| Roadmap review | approved by the orchestrator (lean path by user decision, 2026-09-26) |
| DES phases | 01-01: PREPARE, RED_ACCEPTANCE, GREEN, COMMIT PASS; RED_UNIT SKIPPED. 01-02: PREPARE, GREEN, COMMIT PASS; RED_ACCEPTANCE, RED_UNIT SKIPPED |
| Lanes (01-01) | rvf, canzan-theme-system, pwa-mobile-rendering, cdf, cpd, kb, blr green |
| Static | check-arch, fmt, clippy `-D warnings` pass; xtask smoke pass (01-01) |
| 01-02 smoke | roadmap criterion; the execution log records GREEN PASS, and no explicit smoke result is recorded |
| Peer review | not recorded |
| Mutation (per-feature) | not recorded. The Rust production delta is one `env!` constant plus test literals |
| DES integrity check | not recorded |

### [REF] Pre-requisites / carried notes

- **Gap: no DESIGN or DISTILL section in this file.** The waves were skipped on the lean path, and
  the user chose on 2026-10-04 not to back-fill them. The `.feature` file was authored inside step
  01-01, as its header records.
- `00dc174` wrote the verify path as `/signin`. `c2980f1` (2026-09-27) corrected RELEASING.md to
  `/sign-in`.
- No follow-ups are recorded. The out-of-scope items (git SHA or build date, a version endpoint,
  pipeline changes) remain out of scope.
