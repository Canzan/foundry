# Load Balancer and TLS Termination

## Audience

Operators standing up the multi-replica Variant 2 deployment (US-02). Companion `solution-architect`'s `auth.md` owns *what* a session cookie is and how it survives a switch between replicas; this document owns *the LB choice and config* that routes requests to those replicas.

## Recommendation — Caddy as default

The MVP recommends Caddy 2.x as the default LB and TLS terminator. Rationale captured in ADR-103. Summary: Caddy's headline feature is automatic Let's Encrypt provisioning with a 5-line config, which matches the "under an hour" install promise (US-01) better than any alternative.

### Why Caddy over nginx and Traefik

| Dimension | Caddy (recommended) | nginx | Traefik |
|-----------|---------------------|-------|---------|
| Auto-TLS via Let's Encrypt | Built-in, zero config | Manual (certbot or similar) | Built-in |
| Config syntax | 5-line Caddyfile for our case | ~30 lines | Label/file annotation |
| Memory footprint | ~20 MB | ~10 MB | ~30 MB |
| Hot reload on config change | Yes (API or signal) | Yes (signal) | Yes (file watch) |
| Operator familiarity (2026) | High and growing | Highest | High for K8s-aware operators |
| K8s translation | Caddy Ingress controller exists, but most K8s shops use nginx-ingress or Traefik ingress | nginx-ingress is K8s default | Traefik ingress is K8s default |
| Round-robin LB with no sticky | Default | Default with `upstream` block | Default |
| HTTP/2 + HTTP/3 | Built-in | Yes (HTTP/3 newer) | Built-in |
| Health-check probing | Built-in `lb_policy` + `health_uri` | Requires `nginx-plus` for active checks, otherwise passive only | Built-in |

Caddy wins on "5-line config for the happy path + auto-TLS." nginx and Traefik are documented as drop-in alternates because some operators (particularly K8s shops) will prefer their existing tool. The Foundry binary does NOT depend on which LB is in front of it — every LB choice produces an identical contract.

## Reference Caddyfile (multi-replica)

```caddyfile
foundry.acme.com {
    # Auto-provisions Let's Encrypt cert on first request
    reverse_proxy foundry-app-1:3000 foundry-app-2:3000 foundry-app-3:3000 {
        lb_policy round_robin
        lb_try_duration 5s
        health_uri /readyz
        health_interval 10s
        health_timeout 2s
    }

    # Trust proxy headers for client IP (used in logs and rate-limit later)
    header_up X-Real-IP {http.request.remote.host}
    header_up X-Forwarded-Proto https
}
```

Five operational lines + auto-TLS. Notes:

- `lb_policy round_robin` is the default; spelled explicitly so it's documented.
- `lb_try_duration 5s` means Caddy will retry against the next replica if the chosen one returns 5xx within 5 s. This is the safety net that makes single-replica failure invisible.
- `health_uri /readyz` is the NFR-OBS-02 readiness endpoint. Replicas that flip to 503 (e.g., during graceful shutdown per NFR-AVAIL-02 or during DB outage) are removed from rotation within 10 s.
- `health_timeout 2s` means a replica that takes >2 s to respond to `/readyz` is also treated as unhealthy. This catches the "process alive but stuck" case.
- No `sticky` directive. Round-robin sends each request anywhere. Sessions in Postgres make this safe.

### nginx alternate

```nginx
upstream foundry_backend {
    server foundry-app-1:3000 max_fails=3 fail_timeout=10s;
    server foundry-app-2:3000 max_fails=3 fail_timeout=10s;
    server foundry-app-3:3000 max_fails=3 fail_timeout=10s;
}

server {
    listen 443 ssl http2;
    server_name foundry.acme.com;

    ssl_certificate     /etc/letsencrypt/live/foundry.acme.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/foundry.acme.com/privkey.pem;

    location / {
        proxy_pass http://foundry_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;

        # SSE needs these
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 1h;
    }
}
```

nginx requires the operator to run certbot separately for TLS — adds 5-10 minutes to the install runbook.

### Traefik alternate

Labels on the Foundry service in docker-compose:

```yaml
services:
  foundry-app:
    image: foundry/foundry:latest
    deploy:
      replicas: 3
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.foundry.rule=Host(`foundry.acme.com`)"
      - "traefik.http.routers.foundry.entrypoints=websecure"
      - "traefik.http.routers.foundry.tls.certresolver=letsencrypt"
      - "traefik.http.services.foundry.loadbalancer.server.port=3000"
      - "traefik.http.services.foundry.loadbalancer.healthcheck.path=/readyz"
      - "traefik.http.services.foundry.loadbalancer.healthcheck.interval=10s"
```

Traefik auto-detects replicas via Docker labels — most ergonomic for compose-only shops that don't want a separate `caddy` service.

## SSE-specific LB considerations

Server-Sent Events run over standard HTTP/1.1 long-lived connections. Two LB knobs matter:

1. **Disable response buffering**: if the LB buffers the SSE stream until it sees a complete response, events are delayed. Caddy disables buffering for `text/event-stream` automatically. nginx requires `proxy_buffering off` (shown above). Traefik does the right thing by default.

2. **Long idle-timeout**: SSE connections may sit idle for minutes between events. The LB's idle-connection timeout must be longer than the longest expected gap.
   - Caddy default: no timeout (good).
   - nginx default: 60 s — too short. Set `proxy_read_timeout 1h` (shown above).
   - Traefik default: 60 s for `serversTransport` — set `forwardingTimeouts.responseHeaderTimeout` to 1h.

The Foundry SSE handler also writes a `:keepalive\n\n` comment every 25 seconds (SSE protocol convention) to keep the connection from being killed by intermediaries (proxies, NAT timeouts). This is owned by solution-architect's SSE handler and documented here so operators understand why their LB sees periodic small writes.

## Health-check endpoint semantics (NFR-OBS-02)

The contract between the app and the LB:

| Endpoint | Returns 200 when | Returns 503 when | LB action on 503 |
|----------|------------------|------------------|------------------|
| `GET /healthz` (liveness) | Process can accept TCP connections | Process is hung or unresponsive | Container/Pod restart (Docker `restart: unless-stopped` or K8s `livenessProbe`) |
| `GET /readyz` (readiness) | Process is alive AND Postgres reachable AND migrations applied AND not in graceful-shutdown drain | Postgres unreachable OR migrations in progress OR SIGTERM received | Remove replica from LB rotation (NFR-AVAIL-01, NFR-AVAIL-02) |

**Why two endpoints, not one**: a single "is everything fine?" endpoint conflates "restart me" with "stop sending me traffic." A replica with a broken DB connection is alive (don't restart it; restart won't fix the DB) but not ready (stop sending traffic). Conflating these causes restart loops when the DB blips.

## TLS posture for the MVP

The MVP recommends auto-TLS via Let's Encrypt (Caddy default). Operators behind corporate firewalls or in private networks have three escape hatches:

1. **Custom cert files**: Caddy's `tls cert.pem key.pem` directive bypasses auto-TLS.
2. **TLS termination upstream**: operator runs their own LB (cloud LB, K8s ingress); Foundry's docker-compose ships a "no-TLS" variant that runs Caddy in HTTP-only mode on port 80, and the upstream LB handles TLS.
3. **No TLS** (development/localhost only): set `SESSION_COOKIE_SECURE=false` (NFR-SEC-03) to allow cookies over plain HTTP.

The default is TLS-on. The "no TLS" path requires an explicit env flag flip — fail-safe.

## Probe contract (Principle 9)

The LB layer itself doesn't have a startup probe in the same sense as the app (it's a separate process), but the *interaction* between LB and app has substrate properties we verify:

1. **`probe.lb.healthcheck_path_reachable`** (app side): on startup, the app logs the bound HTTP port and prints a one-line reminder of which health-check paths the LB should poll. This isn't a probe in the strict sense — it's a documentation aid — but it catches the "operator forgot to set `health_uri`" misconfiguration during the first deploy.

2. **`probe.lb.sse_buffering_off`** (manual operator check, post-deploy): the operator runbook includes a `curl -N https://foundry.acme.com/projects/<id>/events` test that exercises SSE through the LB. If the LB is buffering, the user sees no events until the stream closes — easy to spot. This isn't automated in the MVP but is documented.

The asymmetry (app probes itself; LB tested manually) is intentional. The app owns its own behavior; the LB is operator-supplied infrastructure and the contract is documented, not enforced.

## Cross-references

- Deploy variants where this LB sits: `topology.md`.
- Health-check endpoints: NFR-OBS-02 and `failure-modes.md`.
- SSE-specific behavior: `realtime-infrastructure.md`.
- ADR-103 (Caddy as default).
