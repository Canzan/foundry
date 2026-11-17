# NAV\-ICON\-RAIL\-003 A names test over\-approximates\, and its refusal says so
Status: Accepted

## Context

```text
ADR-NAV-ICON-RAIL-002 made the head-script gate total and moved membership from three narrow idiom substrings to what the comment-stripped asset source NAMES. The design review found that only ONE direction of that change had a failure state. A names test errs in both. The false negative was recorded: an asset whose identifiers are built at runtime names nothing the gate can see, so it is treated as a non-stamper. The FALSE POSITIVE was not, and it is one grep from the shipped tree: board-dnd.js, form-errors.js and keyboard.js already name documentElement, so one added storage name classes a settled-DOM neighbour as a pre-paint script. ADR-NAV-ICON-RAIL-002 also said the asset reads stored state and stamps the document element, which a names test cannot establish, and claimed that the gate keeps its own independent parse so it can still contradict the oracle without anything observing that the two agree today.
```

## Decision

```text
The membership test stays a NAMES test and its over-approximation is made explicit rather than tolerated. (1) The refusal says the gate decided BY NAME - the asset NAMES stored state and the document element - never that it was observed to read and stamp. (2) A refused settled-DOM asset is given its two legitimate resolutions: split the stored-state read into its own deferred asset so it no longer names both, or record a deliberate exception as a design decision with its reason. The refusal warns against the third answer, dropping defer, because the gate ACCEPTS the asset once it is plain and a non-pre-paint script would then sit on the render-blocking path with the gate green. (3) The soundness claim is stated over the TRUE pre-paint stampers: no real stamp is missed, which is the one direction the flash promise needs, and the other direction is loud rather than silent. (4) The deliberately separate gate reader and the shared acceptance reader are PAIRED by a contract test on the shipped head, each asserted against the same independently written table of nine src and deferring-attribute pairs, so their agreement is observed instead of asserted; the drift it cannot catch - one that changes both readers identically - is stated.
```

## Alternatives

```text
Decide membership by what the asset DOES rather than by what it names - rejected: no source test can decide when an asset touches what it names, and a runtime test would have to run the asset before the paint it is meant to protect. Add a gate arm refusing a plain head script that looks settled-DOM - rejected for the same reason, and it would refuse the two legitimate pre-paint scripts. Narrow membership again to keep the false positive out - rejected: that reopens the false negative the review found, and missing a real stamp brings the flash back, which is worse than refusing a neighbour. Give the gate and the oracle one shared reader so no pairing is needed - rejected for a technical reason: xtask depends on anyhow, sha2, serde_json and tempfile only, so it would then build the whole scraper, cucumber, fantoccini and testcontainers stack for a gate that must run on a host with no Docker, and it would depend on the crate it has to be able to contradict.
```

## Consequences

```text
A contributor refused by name is told what the gate could and could not decide, and is steered away from the one answer that silently reintroduces the hazard. One hazard is carried rather than closed: a settled-DOM asset made plain is accepted and nothing observes it, which the deliberately refutable settledOnlyNeverRenderBlocking exhibits as a trace and which lives on in the refusal wording and in a recorded risk with the architect as owner. The head gains a standing review obligation: a new PLAIN head script needs a recorded reason. The pairing adds one assertion to each side of a seam that already existed and removes none.
```
