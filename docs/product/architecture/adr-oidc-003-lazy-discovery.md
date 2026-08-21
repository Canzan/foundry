# ADR-OIDC-003 — Validate config shape at boot; fetch discovery and JWKS lazily

- Status: Accepted (2026-08-21)
- Feature: `keycloak-sso` (DDD-9, DDD-10)

## Context

foundry keeps local password sign-in permanently (DISCUSS D2) for one reason: Keycloak,
LLDAP and foundry run on the same cluster, so the tracker must open when that cluster
is broken — it holds the issue describing how to fix it.

An OIDC implementation that resolves `/.well-known/openid-configuration` during startup
would quietly undo that: foundry's readiness would depend on the identity provider it
exists to outlive, and a Keycloak outage would take the tracker down with it. The
deploy order makes it worse — the same `make apply` that creates foundry's Keycloak
client also deploys foundry.

## Decision

Boot validates SHAPE only: the issuer URL parses, client id and secret are non-empty,
the redirect URL is absolute. All present and well-formed → `Some(OidcProvider)`. All
absent → `None`, the feature is off and the sign-in control is not rendered. Partial or
malformed → `health.startup.refused` with a named reason and a metrics counter,
following `MACHINE_TOKEN_SIGNING_KEY` at `main.rs:204`.

Discovery and JWKS are fetched on first use and cached; JWKS refreshes on an unknown
`kid`, rate-limited. Any network failure refuses that sign-in attempt with the generic
error, never the boot, and never a 500.

## Alternatives

**Fetch discovery at boot and refuse to start on failure.** Rejected: makes foundry
unstartable while Keycloak is down, which is the exact scenario D2 exists for, and
creates a first-boot ordering dependency on a cluster that has not converged.

**Fetch at boot but tolerate failure.** Rejected as the worst of both: it pays the
startup latency and still needs the lazy path as a fallback, so it is the lazy design
plus a boot-time network call.

**Pin the JWKS in configuration.** Rejected: Keycloak rotates signing keys, and a
pinned key turns routine rotation into an outage requiring a redeploy.

## Consequences

An unconfigured foundry is a normal foundry, so CI and a contributor's `run.sh` need no
identity provider. A misconfigured foundry fails loudly at boot rather than at the
callback, where it would read as a broken deploy. A correctly configured foundry with
an unreachable Keycloak still serves, still accepts local passwords, and refuses SSO
attempts with the same message everything else refuses with.
