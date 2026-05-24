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
