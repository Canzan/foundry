# Waivers: iis-secrets-key-replace

## W1: The Matrix room leg of E9 and E24

- **Date:** 2026-11-15
- **Decided by:** the human maintainer
- **Scope:** only the part of E9 (P4) and E24 (P11) where the next new issue after a replacement reaches its Matrix room (`#auth:canzan.dev` for Sam in E24). The rest of E9 and E24 is still in scope. That rest is the Slack delivery of the next new issue after a replacement (`#auth-dev` in E24), no connection shown as lost, no email to a workspace admin, and, in E24, the notice "Replaced. 3 saved credentials are sealed under the new key."
- **Why:** a release build of Foundry refuses a Matrix homeserver on loopback, and it trusts only public webpki roots. Only the `test-support` outbound seam gets past that check, and a release build does not contain it. So `scripts/examine-foundry` cannot put a Matrix homeserver in front of the release binary, and no public probe can observe this part.
- **Where it is covered:** the acceptance suite delivers Matrix messages to its Matrix homeserver double (`crates/foundry-acceptance/src/support/matrix_homeserver.rs`).
- **Blocking:** no.
- **Why this file:** nwdes cannot record this waiver itself. A published expectation row cannot change: `nwdes expectations close --revise` refused a lacuna on E9 and E24 with `RowRewritten`. A close diagnostic is not stored on disk. This file is the durable record.
