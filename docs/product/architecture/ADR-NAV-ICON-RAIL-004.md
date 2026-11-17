# NAV\-ICON\-RAIL\-004 One refusal text for the shape a names test cannot split
Status: Accepted

## Context

```text
ADR-NAV-ICON-RAIL-003 made the names test over-approximation explicit and gave the refusal its two cases. The second design review found that it stated them as two REMEDIES without one text: Decision (1) tells the contributor to remove the attribute, Decision (2) tells her never to drop it. The gate decides by names alone, so an aliased true stamper and a settled-DOM neighbour that merely names storage and documentElement are the SAME input class and receive the SAME entry. No single text could satisfy both halves as written, so an implementation could pass one oracle case only by failing the other or by emitting self-contradictory advice. The defect was in the prose, not in the model: head_guard.qnt already holds one verdict record, and therefore one message, per shape.
```

## Decision

```text
The shape NAMES-BOTH-AND-CARRIES-A-DEFERRING-ATTRIBUTE gets ONE refusal text with five required clauses, and the contributor - not the gate - picks the case. C1 names the src and the attributes the tag carries. C2 says the gate decided BY NAME, that the asset names localStorage or sessionStorage and names documentElement, and that source cannot show WHEN it touches them. C3 gives case one, the asset really stamps from stored state before paint: remove the attributes, a plain external script in the head being render-blocking by specification. C4 gives case two, the asset only touches those names on the settled DOM: split the stored-state read into its own deferred asset so this one no longer names both, or record a deliberate exception as a design decision with its reason. C5 is the warning that belongs to case two: do not drop the attributes to satisfy this refusal, because a plain tag puts a non-pre-paint script on the render-blocking path and this gate would then accept it. The S1 oracle asserts those five clauses, and asserts that the entries for the aliased stamper and for the settled-DOM neighbour are the SAME text. The settled-DOM asset made plain is asserted as ACCEPTED, because the gate cannot refuse it without claiming a judgement about intent its source cannot support.
```

## Alternatives

```text
Keep two texts and choose between them by a heuristic about intent - rejected: the gate would be guessing at a fact its input does not contain, which is the class of defect this feature exists to remove. Refuse the plain settled-DOM asset as well, so case two never arises - rejected: it would also refuse the two legitimate pre-paint scripts, since from source they are the same shape. Drop clause C5 and let the remedy be remove-the-attribute alone - rejected: that is the wording that puts a non-pre-paint script on the render-blocking path, and it is the exact harm the review named. Drop C3 and make splitting the read the only remedy - rejected symmetrically: it would send a real stamper away from the one fix that keeps the pre-paint promise.
```

## Consequences

```text
One input class, one entry, one text, five clauses the oracle can assert. The contributor reads what the gate could and could not decide and makes the one judgement only she can make. The wrong remedy stays accepted-but-unobserved: clause C5 and the standing review obligation that a new PLAIN head script needs a recorded reason are its only guards, and the deliberately refutable settledOnlyNeverRenderBlocking exhibits it as a trace. The models and their replayed receipts are unchanged by this ADR: the correction was to the specified text, not to a checked property.
```
