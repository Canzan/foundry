# Product brief: nav\-collapse\-icon\-rail

## Request

```text
Follow-up to nav-collapse-toggle (EXAMINE accepted at 60a4e36). The user decided: (1) Sign out stays in the collapsed strip; (2) fix all five non-blocking findings of the nav-collapse-toggle whole-candidate review; (3) the collapsed left nav shows icons instead of hiding its links. The five findings: [oracle] feature_canzan_theme.rs neighbour arm weakened to deferred_neighbours > 0 — assert every non-pre-paint head script defers; [oracle] its panic text says theme.js is the ONE pre-paint script and its doc says five neighbouring tags (there are two pre-paint and seven deferred); [oracle] fn head_scripts duplicated in feature_nav_collapse.rs, feature_canzan_theme.rs and xtask — one shared owner under crates/foundry-acceptance/src/support/; [implementation] xtask check_arch pre-paint set decided by narrow substrings — widen to names localStorage/sessionStorage AND names documentElement; [implementation] base.html comments say the deferred five, the head declares seven. Second-tab sync stays out of scope.
```

## Work

M

```text
With the left nav collapsed, the operator still reaches every nav destination and Sign out from a narrow icon rail, and the pre-paint and theme guards keep their full strength.
```

### Rework

None.

## Promises

### P1

Outcome

```text
With the left nav collapsed, the operator reaches every destination the expanded nav offers her - Home, Board, Settings, keyboard shortcuts, and the workspace or instance admin entries when she has them - by activating an icon in the narrow rail.
```

### P2

Outcome

```text
With the left nav collapsed, the operator signs out from the rail with one press, as she does when the nav is expanded.
```

### P3

Outcome

```text
Each icon in the collapsed rail carries the name of the destination it opens, so a screen-reader operator hears the destination and a pointer operator sees it.
```

### P4

Outcome

```text
The collapsed rail marks the page the operator is on, as the expanded nav does.
```

### P5

Scenario

```text
Collapsed nav still goes everywhere
```

```text
the operator collapsed the left nav and is on the home page
```

```text
she activates the Board icon in the narrow rail
```

```text
the board opens, the left nav is still collapsed, and the Board icon is marked as the page she is on
```

### P6

Scenario

```text
Nothing becomes unreachable when the rail narrows
```

```text
the operator can reach Home, Board, Settings, keyboard shortcuts, her admin destinations and Sign out from the expanded left nav
```

```text
she collapses the left nav
```

```text
every one of those destinations is still reachable from the rail as an icon, Sign out included, and no link is left focusable or announced while it is invisible
```

### P7

Outcome

```text
At phone width (480 px and below) the page has no horizontal scroll with the icons shown in the collapsed bar.
```

### P8

Outcome

```text
A contributor who adds or changes a head script that stamps the document from stored state is refused by the project gate when that script defers, whichever stored-state spelling it uses, so no new pre-paint script slips past the gate and brings the flash back.
```

### P9

Scenario

```text
A silently deferred neighbour fails the suite
```

```text
the document head declares the scripts that must run before paint and the neighbours that defer
```

```text
any head script that is not one of the pre-paint scripts loses its defer
```

```text
the acceptance suite fails and names that script, instead of passing because another neighbour still defers
```

### P10

Outcome

```text
A contributor reading the head comments, the gate's refusal text and its documentation is told correctly which head scripts must run before paint and which must defer, with no claim that contradicts the head the suite asserts.
```

### P11

Scenario

```text
The collapsed choice survives signing out
```

```text
the operator collapsed the left nav and signed out from the rail in the same browser
```

```text
she signs in again
```

```text
the left nav is collapsed, as she left it, and its icons are the destinations her current rights give her
```

### P12

Scenario

```text
Every head script shape gets one verdict
```

```text
a head script that does not stamp stored state on the document
```

```text
it is loaded async or type=module and the gate checks the head
```

```text
the gate refuses it and tells the contributor to add defer, instead of answering no verdict
```

## Scope

```text
The collapsed left nav shows an icon for every destination the expanded nav shows the operator, and an icon for Sign out.
```

```text
The monogram, the collapse control and the theme control stay in the collapsed rail as they are today.
```

```text
The project gate recognises a head script that stamps the document from stored state by what it does - it names localStorage or sessionStorage and it names documentElement - instead of by a narrow spelling.
```

```text
The acceptance oracle asserts that every head script that is not a pre-paint script defers, and one shared owner reads the head for all of its callers instead of three copies.
```

```text
The head comments, the gate's refusal text and its documentation are corrected to agree with the head the suite asserts.
```

```text
At phone width (480 px and below) the collapsed bar shows the same icons and lets them wrap inside the bar.
```

```text
The gate answers every head-script shape with exactly one verdict: no shape is left with no verdict.
```

```text
Carrying the collapsed-or-expanded choice to a second tab, another browser, another device or another account.
```

```text
The workspace name and the operator's display name stay out of the collapsed rail: it carries destinations and controls only.
```

```text
The expanded left nav keeps the look and the links it has today; no icon is added beside its text labels.
```

```text
A confirmation step for Sign out: in the rail it stays the one press it is when the nav is expanded.
```

```text
Refreshing the nav's rights without a page load: the nav keeps showing the rights of the last load, in both states, as it does today.
```

## Decisions

```text
Sign out stays reachable in the collapsed strip. The user decided this in chat on 2026-11-17.
```

#### Status

Confirmed


```text
The collapsed left nav shows its links as icons instead of hiding them. The user decided this in chat on 2026-11-17.
```

#### Status

Confirmed


```text
All five non-blocking findings of the nav-collapse-toggle whole-candidate review are fixed in this feature. The user decided this in chat on 2026-11-17.
```

#### Status

Confirmed


```text
Carrying the collapse choice to a second tab stays out of scope. The user decided this in chat on 2026-11-17.
```

#### Status

Confirmed


```text
Every destination the expanded nav shows the operator becomes an icon, not only Home and Board. Reason: the supplied value says every nav destination and Sign out, and the user-section links are nav destinations.
```

#### Status

Proposed


```text
The icons are inline SVG vendored from Lucide (ISC), recorded in the vendored-asset provenance file, and they inherit the theme colour. Reason: this was stated to the user as a host default and not objected to; silence is not agreement, so it stays a proposal.
```

#### Status

Proposed


```text
Each icon link keeps the accessible name its text link has today, so the collapsed rail loses the word on screen and never the name. Reason: the rail is the operator's only route while collapsed, and an unnamed icon is not a destination she can choose.
```

#### Status

Proposed


```text
Sign out in the rail stays one press, with no new confirmation step. Reason: the request asks for Sign out to stay reachable, not for its behaviour to change; adding a confirmation would change the expanded nav too.
```

#### Status

Proposed


```text
The collapsed rail marks the page the operator is on the same way the expanded nav marks it. Reason: the shell already renders the current-page marking, and an icon strip with no mark leaves her guessing where she is.
```

#### Status

Proposed


```text
This feature extends the walking skeleton the delivered nav-collapse-toggle feature established - the collapse control, the stored choice and the pre-paint stamp, end to end - and adds no second skeleton of its own.
```

#### Status

Proposed


```text
Scenario exploration, guards value: relevant. Reason: its sad path is the gate's refusal, and the behaviour turns on execution order around the first paint and on which head scripts count as pre-paint - the boundary the review finding says let cases through silently.
```

#### Status

Proposed


```text
Scenario exploration, icon-rail value: relevant. Reason: its sad path is a trapping state - a collapsed rail from which a destination the operator has, or Sign out, cannot be reached - over the collapsed-or-expanded, scripting-on-or-off and permission combinations. Second-tab sync is excluded, so no race belongs in the exploration.
```

#### Status

Proposed


```text
At phone width (480 px and below) the collapsed bar shows the same icons and lets them wrap inside the bar; no destination is dropped below the breakpoint. The human chose this in chat on 2026-11-17, over a sideways-scrolled row and over keeping today's behaviour.
```

#### Status

Confirmed


```text
The expanded left nav stays text only; no icon is added beside its labels. The human chose this in chat on 2026-11-17.
```

#### Status

Confirmed


```text
Signing out does not clear the stored collapsed choice: the operator who collapsed the rail finds it collapsed at her next sign-in in the same browser. Reason: this follows from the delivered nav-collapse-toggle product - the choice is kept per browser the way the theme choice is kept, and it stays until the operator expands it with the toggle, which Sign out is not. The exploration shows both variants: keeping it starts the next sign-in collapsed (witness a_icons_keep_w_q3), clearing it starts it expanded (a_icons_clear_q3). The per-browser consequence is the accepted one: another person on the same browser inherits the choice, exactly as she inherits the theme.
```

#### Status

Proposed


```text
For a head script that does not stamp stored state, defers means it carries the defer attribute: async and type=module in the head are refused for it too, and the contributor is told to add defer. Reason: the exploration shows the rule as written leaves eight shapes with no verdict at all (a_written reading: 6 accept, 10 refuse, 8 none - the contributor reads 'no verdict'), which a gate must never answer; this reading is the literal one of the confirmed finding 'assert every non-pre-paint head script defers', it is total (6 accept, 18 refuse), and it changes no verdict for any script in the shipped head, where every neighbour already carries defer. A contributor who later wants an async or module head script changes this decision deliberately instead of slipping past a silent gap.
```

#### Status

Proposed


```text
The nav shows the rights of the last page load, in both the expanded and the collapsed state, and activating a destination whose right was revoked is refused. The exploration found this (a_icons_keep_rights); it is pre-existing, it is not caused by collapsing, and this feature neither repairs nor worsens it.
```

#### Status

Proposed


```text
Exploration evidence, icon rail: model iconRail.qnt, sha256 0a61ac1540f4f0fdf453c76aea6e322f0b35e6906ab1b571bca8a63e942f81e3, Quint 0.32.0 in the local-dev VM, typecheck exit 0. Command form: quint run iconRail.qnt <flags> --seed=7 --max-steps=8 --max-samples=20000 --out-itf=a_<name>.itf.json. With the icon rail, noTrap, matrixNoTrap, noInvisibleFocusable, focusNeverInvisible, matrixNoInvisibleFocusable and nextSignInStartsCollapsed hold together (icons_keep_all, exit 0): over all twelve state combinations the activatable set is the operator's destinations plus Sign out, and the invisible-focusable set is empty. The old design fails both: hiding the links leaves 'activatable links: {}' and 'Sign out can be activated: false' after collapse (hide_none_q1, exit 1), and hiding them visually leaves {Board, Home, Keyboard shortcuts, Settings, Sign out} able to take the focus while invisible (hide_clip_q2, exit 1).
```

#### Status

Proposed


```text
Exploration evidence, guards: model headGuard.qnt, sha256 099b5168dfba5919b8ba253ff0466816b1c4b2dca3c932bac56c8c378faac0f3, Quint 0.32.0 in the local-dev VM, typecheck exit 0. Command form: quint run headGuard.qnt <flags> --seed=11 --max-steps=12 --max-samples=5000 --out-itf=b_<name>.itf.json. In every reading a stamper is accepted plain and refused with defer, async or type=module, and a script that names documentElement but no stored state must defer. The chosen defer-attribute reading gives every one of the twenty-four shapes exactly one verdict and keeps gateMatchesPaint and stampsBeforePaint_acceptedHead (defer_attr_all, exit 0). Its paint-order trace runs the nav-state stamp and the theme stamp, then first paint with both stamps landed, then the app bundle and the stored-state reader (defer_attr_w_accepted); with the theme stamp loaded defer or async only the nav-state stamp has landed at first paint.
```

#### Status

Proposed


```text
Evidence limits of the exploration: quint run with a seed samples, it does not prove; quint verify did not run because Apalache is not cached in the VM. The static tables (matrix for the rail, verdicts for the gate) are exhaustive over their combinations. Not modelled, so not evidenced here: pixel layout, including the 480 px bar and the icon wrapping the human chose; screen-reader output beyond what is visible and focusable; a second tab; how the gate finds the names in script text, including aliases and strings built at runtime; inline scripts and scripts added at runtime. The icon wrapping and the accessible names need the real browser observation, not this model.
```

#### Status

Proposed


## Journey

```text
The operator collapses the left nav to give the board the width.
```

```text
Not supplied.
```

```text
The rail narrows to an icon strip; she still sees an icon for every destination she had, and one for Sign out.
```

```text
Not supplied.
```

```text
She focuses or hovers an icon and reads the destination's name before she presses it.
```

```text
Not supplied.
```

```text
She activates the Board icon and lands on the board with the rail still narrow and the Board icon marked as her page.
```

```text
Not supplied.
```

```text
When she is done she signs out from the narrow rail, in one press.
```

```text
Not supplied.
```

```text
A contributor later adds a head script that stamps the page from stored state; the gate refuses the deferred shape and the head's own text tells her which scripts must run before paint.
```

```text
Not supplied.
```

## Slices

### S1

```text
A contributor runs the project gate and the acceptance suite: a head script that stamps the document from stored state and defers is refused whichever stored-state spelling it uses, a non-stamping head script loaded plain, async or type=module is refused and told to add defer, any other head script that loses its defer makes the suite fail and name it, and the head comments, the refusal text and the documentation agree with the head the suite asserts. The operator sees no change in the app.
```

#### Promises

P8

P9

P10

P12

#### Depends on

None.

#### Slice work

S

```text
The guards that hold the pre-paint promise, and the texts a contributor reads before she touches the head, stop letting a flash-reintroducing change through in silence, and leave no head-script shape without a verdict.
```

##### Rework

None.

### S2

```text
With the left nav collapsed in the running app shell, the operator reaches Home, Board, Settings, keyboard shortcuts and her admin destinations by activating their icons in the narrow rail, signs out from it in one press, finds it still collapsed at her next sign-in, hears or sees each icon's destination name, sees the page she is on marked, and at 480 px and below the icons wrap inside the bar with no horizontal scroll.
```

#### Promises

P1

P2

P3

P4

P5

P6

P7

P11

#### Depends on

S1

#### Slice work

M

```text
The collapsed nav stops being a dead end: every destination the operator had, and Sign out, stay one press away in the narrow rail, at every width.
```

##### Rework

None.
