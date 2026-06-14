# ADR-002 — Non-enumerable uniform refusal (no existence oracle)

## Status
Proposed (DESIGN wave). Resolves the non-enumerable-refusal open decision. OD-3 (status code) open.

## Context
NFR-3 / FR-4 / BR-4: every invalid-link reason — expired, already-used, invalid/tampered signature,
unknown id — must produce a **byte-identical** user-visible refusal (body AND status). Responses may
differ only in internal `tracing`. The accept page is a PUBLIC (signed-out) route, so both the GET
(form-vs-refusal) and the POST refusal arms must collapse to one page.

The codebase has TWO relevant precedents, pulling opposite ways:
- **GOOD** — the instance-admin / tenancy surface returns a uniform `resource_not_found_page()`
  (`bootstrap.rs:340`) byte-identical for signed-out, non-admin, and never-existed paths (no 403-vs-404
  oracle). This is the posture to copy.
- **BAD** — the **bootstrap claim flow** (`bootstrap.rs:124-139`) returns DISTINCT messages: "Link
  already used" / "Link expired" / "Link not found", all at 410 Gone. That is an **enumeration oracle**
  — a prober learns whether an id existed and its state. The invite-accept flow must NOT replicate it
  (recorded as a security follow-up in `upstream-changes.md`; bootstrap is NOT modified here).

## Options considered
- **(a) ONE `invite_refusal_page()` — single fixed body + single fixed status, for ALL four reasons,
  on BOTH GET and POST (RECOMMENDED).** Reasons differ only in `tracing` keyed on `invite_id`. A
  revert-reds-it litmus binds the byte-identity (collapsing any two arms re-REDs it).
- **(b) Reuse `resource_not_found_page()` (uniform 404) verbatim.** Non-enumerable and consistent with
  the tenancy posture — but a 404 on a real, reachable public path is slightly dishonest UX for a
  legitimate recipient (Priya), and the journey's refusal copy ("ask your instance administrator to
  re-issue") is specific to invites, not a generic not-found. Kept as the OD-3 alternative.
- **(c) Distinct messages per reason (the bootstrap pattern).** REJECTED — it is the exact enumeration
  oracle NFR-3 forbids.

## Decision
**(a)** — a single `invite_refusal_page()` rendering the journey's uniform copy ("This invite is no
longer valid… It may have expired, already been used, or been mistyped. Ask your instance administrator
to re-provision your workspace or re-issue the invitation."), at **one fixed status** (proposed **200
OK** — OD-3) with **one fixed body**. It leaks NONE of: `workspace_name`, account existence, invite
state. Every refusal arm — GET (bad sig / unknown / used / expired) and POST (re-verify fail / consume
0-rows) — returns this exact response. Internal `tracing::info!(invite_id = %id, reason = …)` records
the reason for operators; the reason NEVER reaches the body or status (NFR-3, NFR-5).

**OD-3 (status code)**: 200 OK avoids even a status-code oracle and is the most honest "this page
exists, the link is dead" UX. Alternative: a uniform 404 (option b). Either is non-enumerable; flagged
for confirmation. The byte-identity requirement holds regardless of which is chosen.

## Consequences
- **Positive**: closes the existence oracle; one page to maintain; the revert-reds-it litmus makes
  divergence a hard CI failure (NFR-3 @property, AC-02.1/02.2).
- **Negative**: a legitimate recipient gets no specific reason (by design — the journey accepts this:
  "ask your admin to re-issue" is the universal next action). Operators rely on `tracing` for diagnosis.
- **Security**: defeats account/workspace/invite enumeration on the public surface; aligns the new flow
  with the GOOD tenancy precedent and deliberately AGAINST the bootstrap leak.

## Relationship
Copies the `resource_not_found_page` uniform-refusal shape; diverges from `bootstrap.rs:124-139`
(recorded in `upstream-changes.md`). The litmus mirrors the shipped instance-admin 404 byte-identity test.
