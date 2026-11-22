# PAGE\-FRAME\-003 A quiet control keeps its boundary
Status: Accepted

## Context

```text
The issue dialog Delete resolved no rule, so it inherited the global button --cz-black fill and read as the dialog main action beside Save. The obvious cure, a borderless text control, collides with the shipped control-boundary law of ADR-NAV-POLISH-007 and ADR-BARE-PAGES-002, which holds every control to an identifiable boundary at 3 to 1.
```

## Decision

```text
The quiet tier IS the shipped secondary control: the --cz-surface fill, the --cz-muted border and the --cz-text label. Delete takes it, and so do the two board header actions, Change report and Integrations. The primary --cz-black fill stays reserved for the one main action of a surface, which in the edit dialog is Save. No third, border-less tier is introduced.
```

## Alternatives

```text
A borderless text or ghost Delete: refused, because a control with no fill and no border has no boundary colour to measure, so the 3 to 1 obligation cannot be met. A red destructive Delete: refused, it makes the control louder, not quieter, and would add a colour outside the token set.
```

## Consequences

```text
Two tiers only, measurable in both palettes by the shipped contrast reading. A destructive action is distinguished by its PLACE and its confirm step, not by a colour of its own. Change report and Integrations stop being underlined links and become drawn controls, so the board header reads as a row of controls.
```
