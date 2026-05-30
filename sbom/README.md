# SBOM

## `crates.cdx.json`

A [CycloneDX](https://cyclonedx.org) 1.6 Software Bill of Materials for
Foundry's **Cargo dependency graph** — every crate in `Cargo.lock` as a
`pkg:cargo/<name>@<version>` component, with `dependsOn` edges describing the
graph (not just a flat list).

This is the source-level / crate supply-chain SBOM. It is distinct from the
**runtime image** SBOM (distroless OS packages), which is published only as a
signed attestation on the container image.

### Regenerate

Regenerate whenever `Cargo.lock` changes:

```bash
./sbom/generate.sh        # requires syft + jq
```

The script strips the two non-deterministic syft fields (`metadata.timestamp`
and a random `serialNumber`); everything else is byte-stable for a given
`Cargo.lock`. So the checked-in file changes **only when the dependency set
changes**, keeping diffs meaningful. Don't run bare
`syft scan … -o cyclonedx-json=sbom/crates.cdx.json` — it reintroduces the
volatile fields and the file will churn on every run.

### The signed attestation

Each release also attaches this same CycloneDX SBOM to the container image as a
keyless cosign attestation (see `.github/workflows/release.yml`). Verify it:

```bash
cosign verify-attestation ghcr.io/canzan/foundry:<tag> --type cyclonedx \
  --certificate-identity-regexp 'https://github.com/Canzan/foundry/\.github/workflows/release\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
```
