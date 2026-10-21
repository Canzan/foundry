# Slice 08: Tools get scoped, revocable access through Foundry's own consent screen

Stories: US-EIP-14, US-EIP-15, US-EIP-16 | Estimate: 5.5 days | job_id: `job-grant-custom-integration-access` | Deliverable C (recommended as its own feature, OD-1)

## Goal

Priya registers "n8n homelab". Sven connects n8n through Foundry's consent page,
granting only "Read issues and comments" and "Create issues, move them and
comment" for Backend; n8n creates AUTH-31 as Sven and cannot reach Ops. Priya
sees the grant with its last use and revokes it; n8n is refused within 60 seconds,
and the record shows who granted and who revoked.

## IN

- App registry per workspace: name, exact redirects (https, or http on localhost), allowed scopes (OD-15), client id, secret once, rotate, delete with a counted confirm.
- Authorization code with PKCE, consent page (K-17), scopes in plain words, team selection, single-use short-lived codes, unknown redirects never redirected to.
- Effective access = grant ∩ registration ∩ the member's current access, per request; issuance outside `/api/v1` (OD-14); its own credential class and pin (SC-8).
- Grant lists (admin: all; member: own), revoke within 60 s, routes created under a grant pause with it, audit.

## OUT

- Foundry as an OAuth client of other services. OAuth sign-in to Foundry (Keycloak covers it). Dynamic client registration. Finer scopes.

## Learning Hypothesis

- **Disproves if it fails**: that three scopes (OD-15) are enough for the user's custom integrations. If n8n's first workflow needs a scope outside them, the scope catalogue is the next decision.
- **Confirms if it succeeds**: EIP-KPI-8 in full (an OAuth-connected n8n creates an issue and receives a webhook within 15 minutes of registering the app) and EIP-KPI-9 (revoke to refusal ≤ 60 s).

## Acceptance Criteria

- [ ] Register shows the client id and the secret once; an http redirect off localhost is refused; rotate ends the old secret at once; delete revokes every grant.
- [ ] Consent grants only allowed scopes for chosen teams; the app acts as the member and no further.
- [ ] An unknown redirect shows "This app's request is not valid." and redirects nowhere; cancel grants nothing.
- [ ] Removal from a team ends the app's access there at once.
- [ ] Revoke (admin or self) refuses the app within 60 s and pauses its routes; every grant and revoke is on record.

## Dependencies

- Slices 06, 07. A threat model for the authorization server (R7) before DESIGN of this slice.
- OD-1 (separate feature recommended), OD-2, OD-14, OD-15 answered.
