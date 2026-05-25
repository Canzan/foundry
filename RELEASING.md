# Releasing Foundry

Foundry follows [Semantic Versioning](https://semver.org). Pre-1.0
releases (the current state of the project) permit minor-version
breaking changes; we still flag them in `CHANGELOG.md` with the
`BREAKING:` prefix.

## Cadence

- **Patch (`v0.x.y` -> `v0.x.{y+1}`)**: bug fixes only. Cut whenever a
  user-facing bug ships.
- **Minor (`v0.{x}.y` -> `v0.{x+1}.0`)**: new slice / feature ships.
  Roughly every 2-4 weeks during active MVP development.
- **Major (`v1.0.0` and beyond)**: API/UI stabilization milestone;
  triggers the SemVer "no breaking changes within a major" contract.

## Cutting a release

```sh
# 0. (First release only — and any time the GitHub org changes.)
#    Confirm the release workflow will push to the intended GHCR path.
#    Default is `ghcr.io/${{ github.repository_owner }}/foundry`; this
#    resolves to whatever org owns the repo. If you want a different
#    image namespace, override `IMAGE_NAME:` at the top of
#    `.github/workflows/release.yml` BEFORE the first tag push — once
#    consumers pull from one path, moving it is disruptive.
#
#    Quick check:
#      gh repo view --json owner,name --jq '"ghcr.io/" + .owner.login + "/" + .name'

# 1. Make sure main is green.
git checkout main && git pull --ff-only
cargo xtask ci    # full local pipeline; same gates as remote CI

# 2. Update CHANGELOG.md — move everything under "## Unreleased" into
#    a new "## [vX.Y.Z] - YYYY-MM-DD" heading. See `keep-a-changelog`
#    format.
$EDITOR CHANGELOG.md

# 3. Bump the workspace version (workspace.package version once we
#    move it to workspace.package; today every crate is 0.1.0 and
#    bumps individually).
$EDITOR crates/*/Cargo.toml

# 4. Commit, tag, push.
git add CHANGELOG.md crates/*/Cargo.toml
git commit -m "release: vX.Y.Z"
git tag -s vX.Y.Z -m "vX.Y.Z"   # signed tags strongly recommended
git push origin main
git push origin vX.Y.Z

# 5. .github/workflows/release.yml fires automatically on the tag
#    push. It builds linux/amd64 + linux/arm64, signs with cosign
#    keyless, generates an SPDX SBOM, and publishes to:
#
#      ghcr.io/<owner>/foundry:vX.Y.Z
#      ghcr.io/<owner>/foundry:vX.Y
#      ghcr.io/<owner>/foundry:latest
#
#    Track progress in the Actions tab.
```

## Verifying a published image

```sh
# Verify cosign signature (keyless / sigstore):
cosign verify ghcr.io/<owner>/foundry:vX.Y.Z \
  --certificate-identity-regexp "https://github.com/<owner>/foundry" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com

# Pull and inspect the SBOM:
cosign download attestation \
  --predicate-type https://spdx.dev/Document \
  ghcr.io/<owner>/foundry:vX.Y.Z | jq -r .payload | base64 -d | jq .
```

## CHANGELOG.md format

We follow [keep-a-changelog](https://keepachangelog.com/en/1.1.0/).

```markdown
# Changelog

## [Unreleased]

### Added
- ...

### Changed
- ...

### Fixed
- ...

## [v0.3.0] - 2026-06-10

### Added
- Slice 3 (issue assignment, US-13/14/15). See migration notes below.

### BREAKING
- `FOUNDRY_PORT` env var renamed from `PORT`. Compose users update
  their `.env`; K8s users update `foundry-configmap.yaml`.

## [v0.2.0] - 2026-05-23

Initial public release. Slices 1 + 2.
```

## Operator CLI: `foundry doctor backup-verify`

The release binary ships a `doctor` subcommand for running operational
checks against a Foundry deployment. The first available is
`backup-verify`, which validates a `pg_dump -Fc` custom-format backup
file and reports per-table row counts.

### Why you can't run it directly inside the production container

The runtime image (`gcr.io/distroless/cc-debian12`) is intentionally
minimal — no shell, no `pg_restore`, no `psql`. Running
`foundry doctor backup-verify` inside the container will exit with a
clear error pointing at the missing tooling.

Operators have three supported patterns.

### Pattern 1 — From the host (or any machine with the Postgres client tools)

Easiest if Foundry isn't yet K8s-resident. Install the Postgres client
tools (matching the major version Foundry runs on — currently 16):

```sh
# macOS
brew install postgresql@16

# Debian/Ubuntu
sudo apt-get install postgresql-client-16
```

Capture a backup from the running stack and validate:

```sh
docker compose exec -T postgres pg_dump -U foundry -Fc -d foundry > foundry.dump
foundry doctor backup-verify foundry.dump
```

Exit code 0 + `status: OK` line on stdout = the backup is sound and
the row counts are what you expect. Pipe `stdout` through
`grep -q 'status: OK'` from cron to fail loudly on corruption.

### Pattern 2 — Via a transient container that bundles the client tools

For environments where installing client tools on the host isn't
desirable, run an ephemeral container that has both the foundry
binary AND `pg_restore`:

```sh
# Mount your backup file read-only into a postgres:16-alpine container
# (which ships pg_restore matching the production major version),
# then exec the foundry binary against it.

docker run --rm \
  -v $PWD/foundry.dump:/backup/foundry.dump:ro \
  -v /usr/local/bin/foundry:/usr/local/bin/foundry:ro \
  postgres:16-alpine \
  /usr/local/bin/foundry doctor backup-verify /backup/foundry.dump
```

(Replace `/usr/local/bin/foundry` with the path to a copy of the
binary on your host — `docker cp foundry-foundry-1:/app/foundry .`
extracts it from the deployed container.)

### Pattern 3 — As a Kubernetes Job alongside the StatefulSet

For K8s deploys, ship a small Job manifest that pairs the foundry
image with `postgres:16-alpine` in a single Pod and exec'es the CLI:

```yaml
# Not bundled in deploy/k8s/ — operator-specific recipe.
apiVersion: batch/v1
kind: Job
metadata:
  name: foundry-backup-verify
spec:
  template:
    spec:
      restartPolicy: Never
      initContainers:
        - name: dump
          image: postgres:16-alpine
          command: ["sh", "-c", "pg_dump -Fc -h $PG_HOST -U $PG_USER -d $PG_DB > /shared/foundry.dump"]
          envFrom: [{ secretRef: { name: foundry-secret } }]
          volumeMounts: [{ name: shared, mountPath: /shared }]
      containers:
        - name: verify
          image: ghcr.io/foundry-project/foundry:vX.Y.Z
          command: ["/app/foundry", "doctor", "backup-verify", "/shared/foundry.dump"]
          volumeMounts: [{ name: shared, mountPath: /shared }]
      volumes:
        - name: shared
          emptyDir: {}
```

A future release may ship a separate `foundry-doctor:vX.Y.Z` image
variant with the client tools baked in. Until then, one of the three
patterns above is the supported path.

## Backing out a release

For pre-1.0, the simplest correct path:

```sh
# 1. Delete the bad tag (locally and on origin).
git tag -d vX.Y.Z
git push --delete origin vX.Y.Z

# 2. Delete the bad image tag from GHCR (use the web UI or the
#    `gh api` route). Cosign signatures bound to the digest become
#    orphaned but harmless.

# 3. Fix the bug, cut vX.Y.{Z+1} or vX.{Y+1}.0 as appropriate.
```

Once Foundry hits v1.0, retracting a published version is no longer
acceptable; cut a forward fix instead and document the prior version
as "do not use" in the CHANGELOG.

## Migration notes per release (NFR-MIG-03)

Every release that ships a non-trivially-safe schema migration MUST
call it out in the CHANGELOG under a `### Migration notes` heading:

- expected runtime on a 100k-issue / 10k-user database;
- whether the migration is forward-compatible with the previous
  release (safe for rolling deploys) or requires a maintenance window;
- if non-transactional (`CREATE INDEX CONCURRENTLY` etc.), the
  recovery procedure if the migration fails partway.

See `docs/feature/foundry-backend-mvp/design/system/migrations.md`
for the expand-contract patterns.
