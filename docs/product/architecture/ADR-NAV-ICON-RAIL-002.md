# NAV\-ICON\-RAIL\-002 The head\-script gate answers every shape\, by what the asset names
Status: Superseded by NAV-ICON-RAIL-003

## Context

```text
The delivered pre-paint layer refuses defer, async and type=module on a head script whose asset reads stored state and stamps the document element. The whole-candidate review of nav-collapse-toggle found two holes in it, and the human confirmed on 2026-11-17 that both are fixed here. First, membership was decided by three narrow source substrings, so an asset reading through a bracket access or a local alias, or stamping through an element reference, escaped the set and was accepted with defer - the claim that a third pre-paint script is governed the moment the head references it held only for assets written in those idioms. Second, the oracle arm that observed the opposite direction had been weakened to at least one other head script defers, so a head in which six of seven neighbours silently lost their defer still passed, and no check in the repository observed that direction at all. Reading the repair literally - every head script that is not a pre-paint script defers - leaves eight of the twenty-four head-script shapes with no verdict, because the rule says nothing about async or type=module for a non-stamper.
```

## Decision

```text
The layer becomes total over the EXTERNAL head scripts of the document shell, with two arms of one rule. Membership is decided by what the comment-stripped asset source NAMES - localStorage or sessionStorage, and documentElement - in any idiom. A member must carry none of defer, async or type=module. Every other head script must carry the defer attribute: async and type=module are refused for it too and the contributor is told to add defer. Every script therefore gets exactly one verdict, accept or refuse, and every refusal names the script, what failed, why it matters and how to fix it. The fail-closed arm stays: a head that declares scripts and yields an empty pre-paint set is reported. The stale counts in the head comments, in the live refusal text and in the layer documentation are REMOVED rather than corrected to a new number, so no count can go stale again.
```

## Alternatives

```text
Keep the narrow substrings and state the idiom limit in the doc comment instead - rejected: the limit would be honest but the hole would stay open, and the review named a concrete escape. Read defers semantically, so type=module counts as deferred - total, and rejected because it admits a loading mode nothing in the shipped head uses and nothing observes; the defer-attribute reading changes no verdict for any script in the head today. Read defers as non-blocking, so async counts too - rejected: an async head script may run before the first paint, so it is not a safe neighbour. Pin the corrected head comments with a test - rejected: that is a wording pin on a prose target; de-numbering removes the measurable claim instead, and the texts are verified by review and by EXAMINE.
```

## Consequences

```text
A contributor who adds any head script now gets one verdict and a remedy. A contributor who deliberately wants an async or module head script changes this decision instead of meeting a silent gap. Two limits are stated rather than claimed away: the totality is over external head scripts, so an inline script or one added at runtime gets no verdict; and membership is over what the source NAMES, so an asset whose identifiers are built at runtime is treated as a non-stamper and asked for defer - a loud wrong verdict rather than a silent accept. The acceptance crate gets one owner for the served-head reading, called by both step modules; the gate keeps its own independent parse so it can still contradict the oracle.
```
