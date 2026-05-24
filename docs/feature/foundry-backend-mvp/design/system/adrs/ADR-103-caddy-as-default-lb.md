# ADR-103: Caddy as default load balancer + TLS terminator

## Status

Accepted (2026-05-23).

## Context

The Variant 2 multi-replica deploy (US-02) needs a load balancer in front of N foundry replicas. The LB also needs to terminate TLS for any production deploy reachable from the public internet. The MVP wants to recommend one option as the documented default while leaving the door open for operators with existing LB preferences.

Three credible candidates in 2026 for the self-host audience: Caddy 2.x, nginx (with certbot), Traefik 3.x.

## Decision

**Caddy 2.x is the documented default.** nginx and Traefik are documented as drop-in alternates in `loadbalancer-and-tls.md` with working configs for each.

The choice is driven by the under-an-hour install promise (US-01): Caddy's auto-Let's-Encrypt with a 5-line Caddyfile minimizes the operator step count more than any alternative.

## Alternatives considered

### A — nginx as default

- **Pros**: highest operator familiarity (2026); smallest memory footprint (~10 MB); battle-tested for SSE workloads with the right `proxy_buffering off` config.
- **Cons** (decisive for default):
  - TLS provisioning is a separate process (certbot or similar). Adds 5-10 minutes to the install runbook and a separate cron job for renewal.
  - Config is significantly more verbose (~30 lines vs. ~5 for Caddy) for the same outcome.
  - Auto-discovery of Foundry replicas requires either a separate template-renderer (jinja, gomplate) or the operator hand-listing each replica.

nginx remains a fully-supported alternate. Operators with existing nginx infrastructure or with policy requirements (e.g., FIPS-validated TLS via nginx's OpenSSL build) are not penalized.

### B — Traefik 3.x as default

- **Pros**: native Docker label discovery — replicas are auto-detected; auto-TLS built in; first-class for K8s.
- **Cons**:
  - Operationally more complex than Caddy for a single-host compose deploy (more concepts: entrypoints, routers, services, middlewares).
  - Label-based config is great in compose, awkward elsewhere (file-based config in K8s, third-party tooling for bare-metal).
  - Default config feels K8s-centric in a way that distracts from the MVP's compose-first stance.

Traefik is a strong alternate for operators on Docker Swarm or who'll soon move to K8s.

### C — Operator-supplied LB (no default)

- **Pros**: maximum flexibility; matches the "bring your own infra" ethos.
- **Cons**: violates the under-an-hour promise. A new operator with no LB experience now has to choose one — adds 30 minutes of research before the first replica is reachable from the internet.

### D — Embed an LB inside the Foundry binary (e.g., axum-as-its-own-LB)

- Rejected: the LB must be able to drain a Foundry replica that's shutting down; an embedded LB couldn't survive its own host's termination.

## Consequences

### Positive

- 5-line Caddyfile for the happy path; auto-TLS for any public deploy.
- SSE works out of the box (Caddy doesn't buffer `text/event-stream` and has no default short-idle timeout that would kill long-lived SSE connections).
- Health-check polling against `/readyz` is built in (NFR-OBS-02).
- Caddy's small footprint (~20 MB RSS) keeps the multi-replica stack inside a $10/month VPS.

### Negative (explicit trade-offs)

- Caddy is less ubiquitous in enterprise SOC reviews than nginx; some procurement processes will mandate nginx. We mitigate by offering an equivalent nginx config, equally well-documented.
- Caddy's storage backend for ACME certs is local-disk by default — operators with multiple Caddy instances need to point all of them at shared storage or use Caddy's clustering (post-MVP concern). Variant 2 in the MVP runs a single Caddy in front of multiple Foundry replicas; this is acceptable.
- Caddy's automatic HTTPS may surprise operators expecting plain HTTP; the docs prominently flag the `auto_https off` directive for development.
- **TLS renewal health is not in Foundry's hot loop.** Foundry's `/readyz` deliberately does not gate on Caddy's certificate freshness — that would couple application liveness to LB-state and break the "every replica can serve any request" invariant. As an explicit consequence, a Caddy renewal failure can degrade silently for up to 30 days before the cert expires. We accept this and mitigate by (a) scraping Caddy's `caddy_certificates_renewals_failed_total` and `caddy_certificate_expiry_seconds` metrics with the alert thresholds in `failure-modes.md` (Failure 13), and (b) shipping a `foundry doctor tls-expiry` CLI check for operators who skip the observability overlay. Operators who replace Caddy with nginx/Traefik inherit that LB's renewal-health story; the runbook in `loadbalancer-and-tls.md` documents the equivalent metric for each.

## Review trigger

Revisit if:

1. Survey or issue-tracker signal indicates >25% of operators replace the default Caddy with nginx — suggests the default isn't matching the audience.
2. Caddy's HTTP/3 or quic stack starts causing issues in the field (a relatively new code path).
3. A required compliance regime (FIPS, FedRAMP) forces nginx + a FIPS-validated OpenSSL.

In all three cases, the alternate is one of the already-documented configs — no architectural change.
