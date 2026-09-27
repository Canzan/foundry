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
