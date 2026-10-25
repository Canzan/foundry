# Test-only TLS material — never trust it outside the acceptance harness

These files exist only so the `@needs-matrix` acceptance lane can reach the
pinned Synapse homeserver as `https://matrix.canzan.dev` through the in-process
fault proxy (`crates/foundry-acceptance/src/support/matrix_homeserver.rs`,
DEVOPS U-D1, C-7). Their private keys are public by being committed here.

| File | What it is |
|---|---|
| `matrix-test-ca.crt` | A self-signed test CA ("Foundry acceptance test CA (never trust)"). Its private key was discarded after signing the leaf below, so it can sign nothing else. Foundry trusts it only through `outbound_seam::admitting(.., root_pem)`, which exists only in a `test-support` build. |
| `matrix-canzan-dev.crt`, `matrix-canzan-dev.private-key` | The leaf for `DNS:matrix.canzan.dev` (ECDSA P-256, serverAuth), signed by the test CA. The fault proxy terminates TLS with it. |
| `mailpit-test.crt`, `mailpit-test.private-key` | The `@needs-smtp` relay's self-signed certificate (Foundry connects with `SMTP_TLS_VERIFY=false`). |

Never install the CA in a system or browser trust store, never mount these
files into a production image, never point a real host at the leaf.

Generated once on 2026-10-25 (DELIVER 04-02) inside `alpine:3.24` with its
`openssl` package (OpenSSL 3.5.9), not on the host: the CA and leaf are valid
for about a hundred years (until 2125), so the lane does not expire.

The private keys are named `.private-key`, not `.key` or `.pem`, so a
developer's global ignore rules for key files (common in `~/.gitignore`) do not
silently drop them from a commit (`mailpit-test.key` was renamed for this).
