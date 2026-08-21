# Slice 03 — SSO works on the real cluster

## Goal
`https://foundry.<domain>/sign-in` offers Keycloak sign-in and it works in production.

## Learning hypothesis
**Disproves if it fails**: that foundry's OIDC client can be expressed entirely in
the Keycloak realm import — the homelab reproducibility rule (every change must
survive `make destroy && make up` with no admin-UI clicks). If the client needs a
hand-created step, that is a documented one-time bootstrap in the module README, and
the "rebuild from master" guarantee has a named hole in it.
**Confirms if it succeeds**: SSO survives a cluster rebuild, so adopting it does not
add a manual recovery step to the runbook.

## IN scope (repo: `jeffbailey/homelab`)
- A `foundry` OIDC client in the Keycloak realm import under
  `infrastructure/modules/keycloak/`, with `defaultClientScopes = ["roles",
  "web-origins", "acr", "basic", "profile", "email", "groups"]` as that module's
  AGENTS.md requires, and redirect URI `https://foundry.<domain>/auth/oidc/callback`.
- New variables on `infrastructure/modules/foundry/` for issuer URL, client id and
  client secret, threaded into the Deployment env; secret sourced the way the module's
  existing `session_secret` is, never hardcoded and never a literal domain (tfvar only).
- An e2e test beside `tests/e2e/test_portainer_keycloak_sso.py`.
- A Forgejo Actions build of the foundry image and a `foundry_image_tag` bump in
  `services-state.<domain>.tfvars` — the pin is per-cluster, so shipping is a build
  plus a pin, not just `make apply`.

## OUT of scope
- Enabling SSO on any other cluster (the pin is per-cluster; promote separately).
- Mapping a realm role onto `instance_admins` (out of feature).

## Acceptance criteria
AC-1.5 re-asserted against the real cluster (browser round trip to a signed-in
dashboard), AC-4.3 (bootstrap and invite-accept still reachable), plus the homelab
standing gates: `make plan` zero-diff after `make apply`, `make test-smoke` exit 0,
and the Keycloak client present in the realm import rather than the admin UI.

## Dependencies
Slices 01 and 02. A foundry image built and present in that cluster's zot.

## Effort
~0.5 day. Reference class: the shipped Keycloak SSO wiring for portainer, grafana
and argocd in the same repo, each with an e2e test under `tests/e2e/`.

## Taste-test note
Cross-repo and infrastructure-heavy, but NOT an `@infrastructure`-only slice: its
user-visible outcome is the operator signing into production foundry with Keycloak,
which is the whole feature's point and cannot be demonstrated by slices 01–02 (those
run against a test issuer). It ships the last mile, so it carries real value.

## Dogfood moment
Same day: sign into production foundry with Keycloak from a browser, then close the
feature's own issue from that session.
