# DELIVER — integrations-wayfinding

Delivered 2026-11-03 through four DES-monitored steps (roadmap.json), crafter
`nw-software-crafter`, rigor `[RED, GREEN, COMMIT]`. COMMIT was logged as an
approved skip on every step: the user's rule is one commit after `cargo xtask
smoke` passes, made by the orchestrator at wave end.

## Implementation summary

A workspace admin now has Integrations in the sidebar on every app page
(`Store::nav_identity` carries the admin fact in the identity query the nav
already made). Every refusal on the route form names its real cause per kind
and role, from one pure module (`integrations/no_connection.rs`), on every door
that renders the form: open, preview, save, Send test and edit. The project
integrations page says which workspace connection its routes use (W-6), or that
none is connected (W-7 for an admin, K-4 for a lead), or that it is lost or
unreadable. The connection model is unchanged.

## Files modified

- Production (foundry-app): `integrations/no_connection.rs` (new: cause, words,
  fix link, project-page lines), `integrations/mod.rs` (offer table test),
  `handlers/route_form.rs` (door holds the cause, renders by its own kind),
  `handlers/routes.rs` (preview, save, Send test, edit apply the kind; project
  page lines), `handlers/matrix_setup.rs` (Matrix test refuses with the cause),
  `handlers/workspace.rs` and `workspace.html` (shared wording, one source),
  `handlers/mod.rs`, `nav.rs`, `signin.rs`, `projects.rs`, templates
  `route_form.html`, `project.html`, `partials/sidebar.html`.
- Production (foundry-services): `integrations/mod.rs` (`managed_project`
  returns access facts; `RouteTarget` and `ProjectIntegrations` carry the
  admin fact; `project_view` returns connections).
- Production (foundry-store): `lib.rs` (`NavIdentity`, `nav_identity`).
- Tests: the DISTILL feature and driver; unit tables in `no_connection.rs`,
  `integrations/mod.rs`, `route_form.rs`; store test in
  `create_member_and_consume_store.rs`.
- Removed (unreachable after the save guard): the no-bot `cannot_post` arm,
  K-4 at `matrix_setup.rs:162`, `RouteDoor::unavailable`, `RouteDoor::unreadable`.

## Scenarios green

`integrations-wayfinding`: 54 of 54 examples (400 steps), Lima VM, 2026-11-03.
Regression: external-integration-plugins 181/181 (1942 steps), needs-matrix
38/38 (536 steps), navigation-bar 33/33, foundry-app 130 and foundry-services 41
unit tests.

## Quality gates

| Gate | Outcome |
|---|---|
| Roadmap review | needs_revision (eip lanes, ordering, query ceilings, formatter home) → fixed, approved |
| Per-step TDD | 4/4 steps RED → GREEN, logged; RED observed for the stated reason each time |
| Refactor L1-L6 | applied (names, doc placement, ordering); door merge declined as not clearer |
| Adversarial review | needs_revision: 1 high (Matrix edit form posted kind=slack), 3 medium (Matrix Send test 404 after disconnect, K-19 duplicated on the workspace page, offer precedence untested), 3 low → all fixed in one pass |
| Mutation (per feature) | `no_connection.rs`: 9/9 viable caught. services+store diff: 2/3 by their own crates' tests; the survivor (`project_view` filter `!=`→`==`) applied by hand fails 4 project-page acceptance scenarios, so 11/11 effective, 100%. 11 unviable (types without `Default`) |
| Integrity | `des-verify-integrity`: all 4 steps have complete DES traces |

Mutation notes: the VM's `/tmp` is a 12 GB tmpfs; cargo-mutants' scratch build
does not fit, so run it with `TMPDIR` on disk. cargo-mutants was installed in
the `foundry-test` VM (`cargo install cargo-mutants --locked`, v27.1.0).

## Demo evidence

A web feature: the Elevator Pitches are demonstrated by the acceptance outlines
above, which open the pages as Priya and Lena and follow every offered link
(KPI-IW-G1). The live demo is KPI-IW-M, a walk on dev after v0.18.2 deploys
(Default workspace, project ABC, no Slack app): the new Slack route must read
"Slack is not set up on this Foundry yet…" with "How to set up Slack".

## DoD

- [x] US-IW-01 AC 1-4 (sidebar outline, order, current page, follow from 3 pages)
- [x] US-IW-02 AC 1-7 (form outline, never-ask, precedence controls, every door)
- [x] US-IW-03 AC 1-4 (Slack via acceptance; Matrix line by unit table)
- [ ] KPI-IW-M dogfood on dev — after release

## Follow-ups (not in scope)

- On the project page, a lost or unreadable connection offers an admin no link
  to the workspace page (copy W-6 places the link on the connected line only).
- Send test with no usable connection now answers 422 (was 200), matching save.
