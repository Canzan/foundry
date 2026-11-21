# NAV\-POLISH\-005 Unobservable rail\-identity failure is closed by model and unit test
Status: Accepted

## Context

```text
The design declares that a render whose rail identity read failed marks Home on /workspace/invites and /admin/tokens, where the rail will not offer the page own member. Nothing public produces that render: resolve_admin at crates/foundry-app/src/admin_tokens.rs:293 admits only a workspace admin, and the nav_identity read carries no public fault injection and no declared seam. An acceptance row that named the planned totality unit test would name a craft target, which an acceptance oracle may not bind, so the two rows stood as obligations no part could close.
```

## Decision

```text
The two failed-read rows leave the acceptance obligations and become one law whose treatment is NoTest. Its DESIGN claim is closed by the Quint invariant markNeverEmpty, whose resolve=none mutant reproduces the zero-mark defect with a trace, and its craft evidence is the crafter totality unit test over every section-by-rights pair of NavSection::offered_mark. Its public reachability is recorded BLOCKED with an indeterminate runtime, never PASS, because no public port can observe it. The reconciliation itself stays inside resolved(), so the branch is unreachable from outside, not unreasoned.
```

## Alternatives

```text
Extend the T1 mark-carrier architecture check to read the fallback in nav.rs (rejected: it would decide on the SHAPE of the source text rather than on the served rail, and a source pattern cannot tell a correct fallback from a call that never runs). Let an acceptance row name the crafter totality unit test (rejected: the acceptance oracle may not bind a craft target). Add a public fault-injection seam to the nav identity read (rejected: it widens the production surface for one unobservable branch, and the model already holds the counterexample). Claim the branch PASS on the strength of the model alone (rejected: a model closes its own claim, never the runtime).
```

## Consequences

```text
The failed-read render stays unobserved at runtime until something public can produce it, and the closure rows say so rather than implying a lane covers it. Any later public seam or fault injection on the nav identity read owes this law an acceptance row. The crafter owes the totality unit test over every section-by-rights pair, and a change to NavSection::offered_mark without it leaves the branch with model evidence alone.
```
