# Architecture brief: ui\-page\-frame

## Paradigm

procedural

## Shared records

### Decisions

None.

### Constraints

#### K1

```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group B plus D1 and D2.
```

#### K5

```text
User decision 2026-11-22: Home loses its centred 880 px column; its content starts at the common left edge and its cards span the frame width.
```

#### K6

```text
User decision 2026-11-22: the frame sets no maximum content width; every page shares the frame's content box, and each field is capped by its own size.
```

#### K7

```text
User decision 2026-11-22: the board header keeps the label 'Key prefix', with the key as a labelled value beside it.
```

### Reuse

None.

### Impact

None.

### Agreement

None.

### Not applicable

None.

### Boundaries

None.

### Observations

None.

### Equality

None.

### Constructors

None.

### Laws

None.

### Refuted

None.

### Claims

None.

### States

None.

### Models

None.

### Closure

None.

### Risks

None.

### Architectural tests

None.

### Contract tests

None.

## Slices

### S1

#### Targets

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

Extend

```text
The one owner of the page frame, the field sizes, the control family, the section rhythm, the quiet Delete and the board header row. The file carries the sha256 prefix of its own bytes, so this change renames it again and the old name goes.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
The stylesheet link takes the new hash. Nothing else in the head moves.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
The stylesheet row name and its recomputed sha256, and the prose naming at line 68.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The three cache-policy assertions that name the stylesheet path literally.
```

crates\/foundry\-app\/templates\/board\.html

Extend

```text
The header row, the labelled key in its own element with data-key-prefix, and the two action anchors as secondary controls. The New issue trigger keeps its parent, so the search panel insertion point is untouched.
```

crates\/foundry\-app\/templates\/partials\/issue\_edit\_modal\.html

Extend

```text
Delete gains the secondary class; its type, its position outside the form, its data-action and its hx-get are unchanged.
```

crates\/foundry\-app\/templates\/teams\_index\.html

Extend

```text
Two sections with their own headings and their own buttons; the team-name field and the default-team list sized.
```

crates\/foundry\-app\/templates\/team\_members\.html

Extend

```text
The member list joins the control family and the add form becomes a section.
```

crates\/foundry\-app\/templates\/token\_mint\_form\.html

Extend

```text
Section headings; the label field at the name size and the expiry number field at the short size, with its min and max unchanged.
```

crates\/foundry\-app\/templates\/member\_invite\_form\.html

Extend

```text
The email field at the long size; the action and the CSRF field unchanged.
```

crates\/foundry\-app\/templates\/settings\.html

Extend

```text
The workspace rows and the Project emails include become sections under their own headings.
```

crates\/foundry\-app\/templates\/integrations\/project\_emails\.html

Extend

```text
The fieldset and legend stay and take the styled grouping class; the Save sits inside the section.
```

crates\/foundry\-app\/templates\/integrations\/route\_form\.html

Extend

```text
The Matrix room field and the channel list sized, the three fieldsets styled; every name, value and hidden field unchanged.
```

crates\/foundry\-app\/templates\/instance\_dashboard\.html

Extend

```text
A section heading over the workspace group; the list loses its UA bullets and indent.
```

crates\/foundry\-app\/templates\/partials\/instance\_project\_row\.html

Extend

```text
A row class and a sized rename field. The li, its markers, the hx-target, the hx-swap and the error slot id are unchanged, because the rename handler swaps this element by outerHTML.
```

crates\/foundry\-app\/templates\/partials\/instance\_workspace\_head\.html

Extend

```text
A row class and a sized rename field; the div, its markers and its swap target are unchanged.
```

CHANGELOG\.md

Extend

```text
The release note for this visual pass, naming the new stylesheet hash.
```

crates\/foundry\-app\/templates\/report\.html

Extend

```text
The change report header takes the same labelled key idiom as the board header: the label Key prefix in its own element and the key in its own element carrying data-key-prefix.
```

crates\/foundry\-acceptance\/tests\/features\/ui\-page\-frame\.feature

#### Verification

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo test -p foundry-app"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=ui-page-frame cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=ui-nav-polish cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=ui-bare-pages cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=us-07 cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance"
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_ui\_page\_frame\.rs

crates\/foundry\-acceptance\/src\/lib\.rs

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs

crates\/foundry\-acceptance\/src\/support\/browser\_harness\.rs

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

#### Public oracle

```text
Through the real public driving port. A signed-in operator with a workspace-admin seat opens, in one browser session, every page of the frame set with a GET: Home at /, her board, the change report of that project, /teams, /workspace/invites, /admin/tokens, /account/settings and /admin/instance, plus a Matrix route form. Each page is read as served markup and in the browser lane, at the harness desk viewport and at the emulated 390 by 844 phone, with the device preference dark and again light. On each page the computed inline start and block start of the page first content box are resolved in the live browser, together with the frame own padding box, and the document scrollWidth is read against its clientWidth. She also opens /unsubscribe, which is OUTSIDE the frame, and its main page padding is read there. On each touched form the computed inline size of every text, email, number, list and file control is read, with a generated stored value at the bound of its own rule (a 64-character team name, a 24-character workspace name, a 256-character unbroken project name, a 254-character email). On her board she presses the slash key and reads the injected search panel box, waits for one live refresh after a change made elsewhere, follows Change report and Integrations, opens the edit dialog from a card to read the computed fill and border of Delete against those of Save, the computed box of the status list and the width of the title field against its description textarea, then presses Delete and reads the confirm fragment; she opens the new-issue dialog and the lane-delete dialog and reads the same field and list geometry there. Signed out, with no session cookie, the sign-in card page is opened and its password field width is read against the card content width. On the instance dashboard she renames a project and a workspace, once accepted and once refused with an over-long name.
```

```text
At each viewport, all eight framed pages report the SAME content inline start and the SAME content block start, equal to the frame own padding box (the rail plus 32px inline and 24px block at desk width, 16px on the phone), and no page start moves when the window widens, so Home no longer starts at 424px, Teams, tokens, settings and the instance dashboard no longer at 292px, and the board and invites keep 272px as the one edge. /unsubscribe, outside the frame, still reports its own 20px page padding. No page reports a scrollWidth over its clientWidth in any generated case. Inside the frame, every text field reports an inline size at or below its cap, the token expiry field the short cap, a name field the name cap, an email or Matrix room field the long cap, and none reports the frame width. In a DIALOG the title field reports the dialog content width and EQUALS its description textarea width, in the edit dialog and in the new-issue dialog, because the default cap is scoped to the frame. On the sign-in card page the password field still reports the card content width, unchanged from the shipped build. Every list, number and file control, in the frame and in both dialogs, reports the 1px --cz-muted border, the --cz-surface fill, the shared radius, the inherited face and a 32px minimum height, with its label at 4.5 to 1 or better and its boundary at 3 to 1 or better in BOTH palettes; the list still draws its own arrow. On Teams, the instance dashboard and account settings, every form sits in a section holding exactly one heading and its own submit control, the instance rename rows draw no UA bullet and no UA indent while keeping their markers, the settings groups draw as styled groups and not as raw bordered fieldsets, and no section adds inline padding to the page first content box. In the edit dialog, Save alone resolves the --cz-black fill and Delete resolves the --cz-surface fill with the --cz-muted border in both palettes, and pressing Delete still answers the confirm fragment. The board header AND the change report header each report the key in its own element whose data-key-prefix equals the stored key, under the label Key prefix, with Change report and Integrations on the board each resolving the secondary control, a non-zero gap between their boxes and no underline, the report anchor still carrying its report-link class and its /report href; the live board still refreshes after the change made elsewhere, and the slash key still injects the search panel at the New issue trigger next sibling with its own block box. Both renames still swap their own row and both refusals still land in their own error slot.
```

```text
The shipped build fails these: Home reports 424px and the main-wrapped pages 292px while the board reports 272px, so the all-pages equality fails and names each page; the Home start changes when the window widens, which the two window widths catch; a team name field reports about 1100px, which the cap reading catches; the default-team list, the expiry number field, the Matrix room field and the two dialog lists report the platform box, which the border, fill and face readings catch; the Teams Save sits in no section, the instance rename forms sit in indented bullet lists and the settings groups draw the raw UA fieldset border, which the section reading catches; Delete resolves the --cz-black fill, which the tier comparison against Save catches; and the two project headers show a text run with no element, while the two board links resolve an underline and no gap, which the attribute and gap readings catch. Each law also fails on its own plausible wrong implementation: deleting the main element rule instead of scoping it is caught by the /unsubscribe padding reading; padding the frame but leaving the centred Home column is caught by the window-width pair on Home alone; capping the frame width is caught by the board lane row, which must still use the window; applying the default cap globally instead of scoping it to the frame is caught twice, by the sign-in card field that would narrow and by the dialog title field that would stop short of its textarea; sizing a field with width instead of max-inline-size is caught by the 256-character unbroken value, which then pushes the document scrollWidth past its clientWidth; giving Delete a borderless quiet look is caught by the 3 to 1 boundary reading on it; moving the New issue trigger into the header row is caught by the search panel box after the slash key; renaming the report-link class or its /report href is caught by the live refresh, which then never arrives; leaving the change report header as the old text run is caught by its own data-key-prefix reading; changing the rename row outer element or its markers is caught by the two renames and their two refusals; naming a colour literal for the new surfaces is caught by check-arch S1; and editing the stylesheet without renaming it is caught by check-arch R2.
```

##### Promises

P1

P2

P3

P4

P5

#### Prefactoring

None.

#### Records

##### Decisions

```text
ONE BOX OWNS WHERE A PAGE STARTS. The app shell content box (.app-shell__content) is the single padding box between the window and an app page first content box: 24px block and 32px inline at desk width, 16px below the shipped 480px phone breakpoint. The shipped stylesheet has THREE writers of that one fact, which is the whole of B1: the frame box, the element rule main { padding: 20px } which adds a second box on every page that wraps its content in <main> (Teams, tokens, settings, instance admin, the integrations pages: 292px), and .dash which adds a centred max-width 880px column whose inline start is a function of the WINDOW (Home: 424px). The element rule is SCOPED, not deleted, to body > main, because four out-of-frame pages (email_stop, unsubscribe_confirm, unsubscribe_result, notifications) render <main> as a direct child of body and are out of scope; scoping states the real rule (a page with no frame pads itself, a framed page is padded by the frame) rather than defeating the old one with a padding: 0 override. .dash keeps its class and its colour and type rules, because feature_ui_nav_polish.rs:77 and feature_canzan_theme.rs:950 select on it, and loses its max-width, its auto margins and its padding. .page-card keeps its own padding (a class beats an element rule), so the card pages are unchanged. Carriers: the frame box in the stylesheet app-shell block; no new template, no new Rust type, no new state. Layers. Domain: none. Application: none. Adapter: app_shell.html unchanged, one scoped element rule, one .dash rule set reduced. Infrastructure: none. Observation: the computed inline start and block start of each page first content box, at both viewports.
```

```text
THE FRAME SETS NO MAXIMUM CONTENT WIDTH. Promise P1 asks for the same width on every page, and the frame content box gives exactly that: the window minus the rail minus its own inline padding, identical on every page. A max-inline-size on the frame was rejected twice over: centred it reintroduces a window-dependent left edge, which is the Home defect being removed, and left-aligned it still narrows the BOARD, whose lanes need the window. Readable line length is therefore a FIELD decision (P2) and a SECTION decision (P3), not a frame decision. Layers: Adapter only.
```

```text
A FIELD IS AS WIDE AS WHAT IT HOLDS, FROM A CLOSED SET OF THREE SIZES, AND THE DEFAULT CAP IS SCOPED TO THE FRAMED FORMS. The shipped rule input[type=text], input[type=email], input[type=password], textarea { width: 100% } is what stretches a team name to about 1100px. Its width becomes inline-size, and a max-inline-size is taken from three size classes: .field--short (about 8rem, a token expiry or a count), .field--name (about 24rem, a team, project or workspace name), .field--long (about 36rem, an email, a URL or a Matrix room id). THE THREE CLASSES WORK WHEREVER THEY ARE NAMED; THE UNCLASSED DEFAULT CAP APPLIES ONLY INSIDE .app-shell__content. That scope is deliberate, because the shipped rule reaches text, email and password inputs in 22 templates, and two families of them are not framed forms. FIRST, the signed-out card pages delivered by ui-bare-pages (signin.html:9, forgot, bootstrap_claim, invite_accept, reset_password) sit in a 28rem .page-card with 1.75rem padding, which leaves about 24.5rem of content, so a 24rem default cap would visibly narrow every one of them; they are out of scope and must not change, so outside the frame a field keeps inline-size: 100% and no cap. SECOND, the dialogs: .modal-dialog is width: min(30rem, 92vw) with 1.5rem padding (foundry.8a68a324.css:864), so a capped title field would stop short while its description textarea ran the full dialog width. A DIALOG FIELD THEREFORE KEEPS inline-size: 100% AND NO CAP, which is exactly what keeps the edit dialog title field level with its textarea and the new-issue dialog title field level with its own; both dialogs are reached by the oracle and by the shipped dialog lanes. textarea keeps the full-width box everywhere, because a description is a paragraph and not a field. The cap is PRESENTATION ONLY and never refuses: the stored bounds stay exactly where they are (team name 64 chars, project name 256, workspace name 24, the token TTL number input min 1 max 365), and a value longer than the cap scrolls inside its own field. Carriers: three size classes plus one frame-scoped default in the stylesheet forms block, named on the input in the template; no new Rust type. Layers: Adapter only. Observation: the computed inline size of each field, inside the frame, inside a dialog and on a card page, and the absence of page-wide horizontal overflow.
```

```text
THE LIST, NUMBER AND FILE CONTROLS JOIN THE FOUNDRY CONTROL FAMILY, GLOBALLY, AND THE LIST KEEPS ITS OWN ARROW. select and input[type=number] join the existing text-input rule group, so they take the same min-height 32px, the same 6px/10px padding, the same --cz-muted hairline, the same --cz-surface fill, the same radius and font: inherit. The file control already took that treatment in the previous Request through input[type=file]::file-selector-button, which is the precedent this follows. THE CONTROL FAMILY IS NOT FRAME-SCOPED, ON PURPOSE: a list is drawn by the platform wherever it sits, so every shipped list takes the Foundry box, and that reaches FIVE of them. Three are already named by the promise (the Teams default-team list, the team-member list and the Matrix channel list) and TWO more are in dialogs: the issue edit dialog status list (partials/issue_edit_modal.html:23) and the lane-delete destination list (partials/delete_lane_modal.html:20). Both are a deliberate improvement and neither changes a name, a value or an option order, so the shipped dialog lanes still resolve them. No signed-out card page carries a list, a number or a file input, so the control family leaves those pages untouched as well. appearance: none is DELIBERATELY NOT USED on select: removing the UA arrow obliges this stylesheet to draw a chevron, and a drawn chevron is either an asset or a colour literal below the token seam, which check-arch S1 refuses. So the list keeps the arrow the platform draws and takes the box Foundry draws. The number input keeps its spinner for the same reason. Layers: Adapter only. Observation: the computed border, fill, radius and font family of each control kind, in the frame and in a dialog, and its contrast in both palettes.
```

```text
A STACKED FORM BECOMES A SECTION WITH A HEADING AND ITS OWN ACTION. One .page-section block carries the rhythm the dashboard already uses (the retired .dash__section margin), a heading and its fields, and holds the submit button INSIDE the section it belongs to. Teams gains two sections, Create a team and Default team for new members, which is what stops the Save button reading as orphaned at the foot of the page. Settings wraps its rows and its Project emails include as sections; the fieldset and legend in integrations/project_emails.html and integrations/route_form.html STAY, because a checkbox or radio group is exactly what a fieldset is for, and they are styled instead: no UA border, the legend drawn as the section sub-heading, the choices in a spaced column. Instance admin keeps every htmx marker and every outer element kind of its rename rows (li[data-project-row], div[data-workspace-head]) because form-errors.js and the hx-swap outerHTML targets resolve them; the nested bullet lists lose their markers and their UA indent through list-style: none and padding-inline-start: 0, and the hierarchy is carried by the workspace heading plus a hairline on the nested row group. NO inline padding is added to a page first content box, or B1 would be undone one section at a time. Layers: Adapter only. Observation: the served section structure, one heading per section with its submit inside it, and the computed inline start of the first content box.
```

```text
D1: DELETE IS THE SECONDARY CONTROL, NOT A GHOST. .modal-delete resolves NO rule today, so it inherits the global button fill --cz-black and reads as the dialog main action beside Save. It takes the shipped button.secondary pairing exactly: --cz-surface fill, --cz-muted border, --cz-text label. A borderless text or ghost control was REJECTED: ADR-NAV-POLISH-007 and ADR-BARE-PAGES-002 hold every control to an identifiable boundary at 3:1, and a quiet control with no fill and no border has no boundary to measure. Save stays the only --cz-black fill in the dialog, so the primary tier is unambiguous. The control keeps its type=button, its position outside the form, its data-action, its hx-get to the confirm fragment and its 24px target, so she is still asked to confirm. Layers: Adapter only. Observation: the computed fill and border of Delete against those of Save, in both palettes, and the confirm fragment it still fetches.
```

```text
D2: THE BOARD HEADER IS ONE ROW, THE KEY BECOMES A PROPERTY RATHER THAN A SCRAPE, AND THE CHANGE REPORT HEADER TAKES THE SAME IDIOM. The header draws the project name and the key as a labelled value on one row: the label Key prefix, which the user chose on 2026-11-22, in its own muted element, and the key in its OWN element carrying data-key-prefix with the stored key, in the mono face the card key and the issue card key already use. That element is why the shipped reader must migrate: us_07_project_create.rs:821 finds the literal text Key prefix: and takes the characters up to the next <, so ANY element between the label and the value reads as an empty key. The reading moves to the data-key-prefix attribute, which is the property rather than a designation of it. report.html:4 RENDERS THE SAME p.key HEADER AND TAKES THE SAME IDIOM, with the same label and its own data-key-prefix: one key idiom for the two project headers, rather than a deliberate difference the operator would read as two things. The oracle reads the key on both pages, and the migrated reader resolves both. Change report and Integrations become secondary controls in the existing nav.board-actions with a gap, so they are two drawn controls and not two run-together underlined links. THEY KEEP THEIR SHIPPED CLASSES AND HREFS: the secondary classes are ADDED BESIDE report-link and integrations-link, and the report anchor keeps its /report href, because board-live.js:65 selects a.report-link[href] and derives the live board URL by stripping that /report suffix; a renamed class or href would silently turn the live board off. THE NEW ISSUE BUTTON DOES NOT MOVE INTO THAT ROW: keyboard.js inserts the search control and the search panel as trigger.parentNode.insertBefore(node, trigger.nextSibling), so the trigger parent must stay a plain block; a flex row would lay the injected search panel out as a row item. Layers: Adapter only. Observation: the data-key-prefix value on the board and on the change report, the computed presentation and the gap between the two control boxes, the live board still refreshing, and the search panel geometry after the slash key.
```

```text
ONE CONTENT-HASH MIGRATION. The served stylesheet is the only file whose NAME changes: its bytes change, so the shipped asset-hash rule requires the name to carry the sha256 prefix of the new bytes, and the rename travels in the same commit to its three readers, the link in templates/base.html:6, the row and the sha256 in static/VENDOR.md:33 and :68, and the three cache-policy assertions in crates/foundry-app/src/lib.rs:344, :361 and :380. check-arch R1 refuses a /static reference that does not resolve and R2 refuses a content-hashed name that is not its own hash, so a half-done rename is caught by the gate and not by review.
```

```text
CLAIM-TO-CHECKER, AND WHAT THE MODEL DOES NOT COVER. The one claim with an algebra is B1, because a page content inline start is a SUM over the boxes on the path from the shell to the page first content box, so the defect is invisible per page and only shows across pages and viewports. Quint carries it at docs/feature/ui-page-frame/design/page_frame.qnt. Tools and literal argv: quint 0.33.1 with Java openjdk 27.0.1.1 (Amazon Corretto 27.0.1.1.4), run as quint verify page_frame.qnt --invariant=uniformFrame, and the mutants as quint run page_frame.qnt --invariant=uniformFrameHere --max-samples=400 --max-steps=1 --seed=SEED. THE MODEL COVERS THE SUM ALGEBRA ONLY: it decides whether a declared chain of padding boxes yields one edge for every page at both viewports, and it refutes the three chains that do not. It does NOT read the stylesheet, resolve a selector, evaluate the cascade, or know whether the shipped file matches the chain it was given. THE ONLY CHECK OF THE REAL STYLESHEET IS THE BROWSER ALL-PAGES READING in the oracle: the computed inline start and block start of every framed page at both viewports, plus a reading of /unsubscribe, which is outside the frame and must keep its own page padding when the element rule is scoped. check-arch covers the asset and token-seam rules, never the geometry. TLA+ is not needed: there is no temporal, fairness or concurrency claim anywhere in this Request, every observation is of a settled document. Agda is not needed: no proof over an open domain is claimed. No model for the field sizes, the control family, the sections, the Delete tier or the board header: each names no shared state, no multi-step protocol and no crash window, and each domain is the closed set of named pages, named field kinds or named control kinds the finite laws enumerate. The one broad domain is the stored name or label a field must hold, carried by a generated property over the rendered page rather than by a model.
```

##### Constraints

```text
Stored input bounds the sized fields must hold without refusing: team name at most 64 characters (TEAM_NAME_MAX_CHARS, crates/foundry-core/src/team_name.rs:17), project name at most 256 (project_name.rs:20), workspace name at most 24 (workspace_name.rs:18), and the token expiry number input carries min 1 and max 365 days (templates/token_mint_form.html:11). The token label and the invite email carry NO stated length bound in the authority, and this design adds none: the three size classes are a presentation cap, so a value longer than its cap scrolls inside its own field and the server refusals stay exactly as shipped. Any narrower bound in a field would be a deviation and is not designed.
```

```text
Out of scope and unchanged: the left nav, the signed-out pages, the shortcuts page, the issue full page, the card-less error pages (spent invite, spent reset, bootstrap with no token), and who may open a page or what a form does. Every form keeps its method, action, field names, required flags, hidden CSRF token and server-side validation; no route, no authorization read and no cache header changes in this Request.
```

```text
Shipped laws this change must not break: the paint band of ADR-NAV-POLISH-004 (the rail owns its own stacking context; no new positioned or z-indexed box on the page content path), the control-boundary 3:1 and text 4.5:1 obligations of ADR-NAV-POLISH-007 and ADR-BARE-PAGES-002 in both palettes, the colour-token seam (check-arch S1 and S2: no colour literal outside the three token regions and identical token sets), and the asset rules (check-arch R1 and R2).
```

```text
Swap contracts that must survive: the instance-admin rename rows keep their outer element kind and their markers (li[data-project-row], div[data-workspace-head], div[data-error-slot] with its per-row id and the data-error-target child selector), because the rename handlers swap them by hx-swap outerHTML and form-errors.js routes a 422 at the slot current child. keyboard.js inserts the search control and the search panel as the New issue trigger next sibling inside the trigger own parent, so that parent must stay a plain block.
```

##### Reuse

app\-shell

crates\/foundry\-app\/templates\/app\_shell\.html\:3

Reuse

```text
The shipped frame already wraps every signed-in app page (44 reads answered by code-fact query.reads-of, textsearch provider, noisy confidence: 14 Rust view sites and 30 template extends). The frame is correct; only its content box gains the single-owner padding rule. No new frame and no template moved.
```

button\.secondary

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:318

Reuse

```text
The shipped quiet control tier, already measured in both palettes by the theme lane. It is what D1 Delete and the two D2 board links become, so no third control tier is invented.
```

file\-selector\-button

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:2134

Reuse

```text
The previous Request already drew the file control button with the secondary pairing. The select and number rules follow that precedent rather than inventing a second control presentation.
```

input

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:331

Extend

```text
The one owner of a field box, the rule group at this line. select and input[type=number] join its selector list, and its width: 100% becomes inline-size with a capped max-inline-size. Extending the owner keeps one description of a field rather than a parallel rule set.
```

main

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:284

Replace

```text
The second writer of the page geometry fact. Replaced by body > main, which states the real rule and leaves the four out-of-frame pages padded; a padding: 0 override inside the frame would be a guard over an unchanged representation.
```

dash

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:671

Extend

```text
The third writer of the page geometry fact. The class SURVIVES because two shipped lanes select on it (feature_ui_nav_polish.rs:77, feature_canzan_theme.rs:950); it keeps colour and type and loses max-width, auto margins and padding.
```

page\-section

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:697

CreateNew

```text
No shipped class owns a form section: .dash__section carries only a bottom margin on the dashboard and no heading or action rhythm, and a fieldset is a grouping element with its own semantics. The new block owns the section rhythm for every app page, and .dash__section folds into it.
```

board\_key

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs\:821

Extend

```text
The only shipped reader of the board key, confirmed by a repository-wide search for the literal Key prefix. It scrapes the text up to the next angle bracket, which the labelled value breaks, so it moves to the data-key-prefix attribute. Owned by the acceptance designer and listed as a support.
```

foundry\.8a68a324\.css

crates\/foundry\-app\/templates\/base\.html\:6

Extend

```text
The one stylesheet link of the document shell. It takes the new content hash and nothing else in the head moves.
```

css\/foundry\.8a68a324\.css

crates\/foundry\-app\/static\/VENDOR\.md\:33

Extend

```text
The asset inventory row for the stylesheet. Its name and its sha256 are recomputed in the same commit as the rename.
```

static\_cache\_control\_value

crates\/foundry\-app\/src\/lib\.rs\:344

Extend

```text
The three cache-policy assertions that name the stylesheet path literally, at lines 344, 361 and 380. They are extended with the new name; no new assertion and no new policy.
```

key\_prefix

crates\/foundry\-app\/templates\/board\.html\:4

Extend

```text
The shipped board header is the owner of the project name and the key. It gains a header row and a labelled key in its own element; the New issue trigger and its parent are left alone.
```

modal\-delete

crates\/foundry\-app\/templates\/partials\/issue\_edit\_modal\.html\:16

Extend

```text
The shipped Delete control already carries the class, the type, the position outside the form, the data-action and the hx-get. Only its presentation tier is added, so no control is created and none moves.
```

teams\_\_create

crates\/foundry\-app\/templates\/teams\_index\.html\:19

Extend

```text
The page already owns both forms; it gains the two section wrappers and their headings and the two field sizes. No form, action or field name is created.
```

user\_id

crates\/foundry\-app\/templates\/team\_members\.html\:70

Extend

```text
The member list and the add form are already here; the list joins the control family and the form becomes a section.
```

ttl\_days

crates\/foundry\-app\/templates\/token\_mint\_form\.html\:11

Extend

```text
The mint form already owns the label and the expiry fields with their min and max. They gain their size classes and the page gains its section headings.
```

data\-member\-invite\-form

crates\/foundry\-app\/templates\/member\_invite\_form\.html\:8

Extend

```text
The invite form already owns the email field; it gains the long size class. The action and the CSRF field are untouched.
```

settings\-rows

crates\/foundry\-app\/templates\/settings\.html\:8

Extend

```text
The page already owns the rows list and the Project emails include; both become sections under their own headings.
```

fieldset

crates\/foundry\-app\/templates\/integrations\/project\_emails\.html\:18

Extend

```text
The fieldset and legend are the right grouping element for a checkbox group and are KEPT; they gain the styled grouping class, so no markup is replaced.
```

route\-destination

crates\/foundry\-app\/templates\/integrations\/route\_form\.html\:33

Extend

```text
The route form already owns the Matrix room field, the channel list and the three fieldsets; they gain their size classes and the styled grouping. Every name, value and hidden field is unchanged.
```

data\-instance\-dashboard

crates\/foundry\-app\/templates\/instance\_dashboard\.html\:4

Extend

```text
The page already owns the workspace and project lists; it gains a section heading and the un-indented list presentation.
```

data\-project\-row

crates\/foundry\-app\/templates\/partials\/instance\_project\_row\.html\:10

Extend

```text
The row partial is the owner of the project rename form and is swapped by outerHTML, so its li, its markers, its hx-target, its hx-swap and its error slot id are preserved; it gains a row class and a sized field.
```

data\-workspace\-head

crates\/foundry\-app\/templates\/partials\/instance\_workspace\_head\.html\:6

Extend

```text
The head partial is the owner of the workspace rename form and is swapped by outerHTML, so its div, its markers and its swap target are preserved; it gains a row class and a sized field.
```

foundry\.8a68a324\.css

CHANGELOG\.md\:41

Extend

```text
The release note file already records each visual pass and the stylesheet hash it shipped; this change adds its own note and names the new hash.
```

key\_prefix

crates\/foundry\-app\/templates\/report\.html\:4

Extend

```text
The shipped change report header already owns the project name and the key in the same p.key shape as the board. It is extended with the one key idiom rather than left as a second idiom for the same datum.
```

##### Impact

stylesheet

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:1

Migrated

```text
Bytes change, so the content-hashed name changes with them and the old name goes.
```

template\-head

crates\/foundry\-app\/templates\/base\.html\:6

Changed

```text
The stylesheet link takes the new hash. Nothing else in the head moves.
```

asset\-inventory

crates\/foundry\-app\/static\/VENDOR\.md\:33

Changed

```text
The row name and its recomputed sha256, and the prose naming at line 68.
```

cache\-policy\-assertions

crates\/foundry\-app\/src\/lib\.rs\:344

Changed

```text
The three assertions name the stylesheet path literally, at lines 344, 361 and 380.
```

page\-template

crates\/foundry\-app\/templates\/board\.html\:4

Changed

```text
The header row, the labelled key with data-key-prefix, and the two anchors as secondary controls. The New issue trigger and its parent are untouched.
```

dialog\-partial

crates\/foundry\-app\/templates\/partials\/issue\_edit\_modal\.html\:16

Changed

```text
Delete gains the secondary class; its type, position, data-action and hx-get are unchanged.
```

page\-template

crates\/foundry\-app\/templates\/teams\_index\.html\:19

Changed

```text
Two sections with headings, the select and the text field sized, the Save button inside its own section.
```

page\-template

crates\/foundry\-app\/templates\/token\_mint\_form\.html\:8

Changed

```text
Section headings, the label field sized, the expiry number field short-sized; its min and max are unchanged.
```

page\-template

crates\/foundry\-app\/templates\/member\_invite\_form\.html\:7

Changed

```text
The email field sized long; the form keeps its action and its CSRF field.
```

page\-template

crates\/foundry\-app\/templates\/settings\.html\:4

Changed

```text
The rows and the Project emails include become sections.
```

form\-partial

crates\/foundry\-app\/templates\/integrations\/project\_emails\.html\:18

Changed

```text
The fieldset and legend stay and take the styled grouping class.
```

form\-partial

crates\/foundry\-app\/templates\/integrations\/route\_form\.html\:28

Changed

```text
The Matrix room field and the channel list sized, the three fieldsets styled; every name, value and hidden field unchanged.
```

page\-template

crates\/foundry\-app\/templates\/instance\_dashboard\.html\:5

Changed

```text
Section headings over the workspace list; the list loses its UA bullets and indent.
```

row\-partial

crates\/foundry\-app\/templates\/partials\/instance\_project\_row\.html\:10

Changed

```text
A row class and a sized rename field. The li, its markers, the hx-target, the hx-swap and the error slot id are unchanged, because the rename handler swaps this element by outerHTML.
```

row\-partial

crates\/foundry\-app\/templates\/partials\/instance\_workspace\_head\.html\:6

Changed

```text
A row class and a sized rename field; the div, its markers and its swap target are unchanged.
```

page\-template

crates\/foundry\-app\/templates\/team\_members\.html\:70

Changed

```text
The member select takes the control family and the add form becomes a section.
```

release\-note

CHANGELOG\.md\:1

Changed

```text
The release note for this visual pass, naming the new stylesheet hash.
```

acceptance\-support

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs\:821

Migrated

```text
The board key reading moves from a text scrape to the data-key-prefix attribute.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:950

UnchangedCompatible

```text
It selects .dash and reads its colour; the class and its colour rule survive, only its geometry goes.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_ui\_nav\_polish\.rs\:77

UnchangedCompatible

```text
Its dashboard witness is the .dash selector, which survives.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_pwa\_mobile\.rs\:433

UnchangedCompatible

```text
It reads box widths against the client width below the 480px breakpoint; the phone frame padding is unchanged at 16px and no new rule leaves the media bound.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_board\_lane\_overflow\_menu\.rs\:1

UnchangedCompatible

```text
The lane menu reads its own paint level; no new positioned, overflow or z-indexed box is added on the page content path.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_instance\_admin\_workspace\_rename\.rs\:1

UnchangedCompatible

```text
It resolves the rename row by its markers and its error slot, all of which are preserved.
```

acceptance\-lane

crates\/foundry\-acceptance\/src\/steps\/feature\_issue\_edit\_modal\_close\.rs\:1

UnchangedCompatible

```text
It resolves the close glyph and the Delete control by data-action, which is unchanged; only the Delete presentation changes.
```

dialog\-control

crates\/foundry\-app\/templates\/partials\/issue\_edit\_modal\.html\:23

Changed

```text
The status list gains the Foundry control box with the rest of the control family; its name, its options and their order are unchanged, and the title field beside it keeps inline-size 100% with no cap so it stays level with the description textarea.
```

dialog\-control

crates\/foundry\-app\/templates\/partials\/delete\_lane\_modal\.html\:20

UnchangedCompatible

```text
Its bytes do not change. The global control family gives its destination list the Foundry box, which is a deliberate improvement; the shipped lane-delete lane resolves it by name and option value, both unchanged.
```

dialog\-field

crates\/foundry\-app\/templates\/partials\/new\_issue\_modal\.html\:6

UnchangedCompatible

```text
Its bytes do not change. It sits in a dialog, so the frame default cap does not reach it and its title field keeps the dialog content width beside its description textarea.
```

card\-page\-field

crates\/foundry\-app\/templates\/signin\.html\:9

UnchangedCompatible

```text
Its bytes do not change, and the frame default cap is scoped to .app-shell__content, so this field and the other card-page fields of ui-bare-pages keep the card content width. The oracle reads one of them to prove the scope.
```

board\-script

crates\/foundry\-app\/static\/js\/board\-live\.js\:65

UnchangedCompatible

```text
It selects a.report-link[href] and strips the /report suffix to derive the live board URL. The class and the href stay on the new anchor, so the derivation is unchanged; only the anchor presentation changes.
```

page\-template

crates\/foundry\-app\/templates\/report\.html\:4

Changed

```text
The change report header takes the same labelled key idiom as the board header, with the label Key prefix and its own data-key-prefix.
```

##### Agreement

The board header project key reading

crates\/foundry\-app\/templates\/board\.html\:4

Producer

Migrated

```text
The key moves into its own element carrying data-key-prefix, so the text run the old reader scraped no longer exists.
```

The board header project key reading

crates\/foundry\-acceptance\/src\/steps\/us\_07\_project\_create\.rs\:821

Consumer

Migrated

```text
The only shipped consumer. It moves from the text scrape to the attribute, which is the property and not a designation of it.
```

The instance\-admin rename row swap shape

crates\/foundry\-app\/templates\/partials\/instance\_project\_row\.html\:10

Producer

UnchangedCompatible

```text
The li, its two markers, the per-row error slot id and the outer element kind are preserved; only a class and a field size are added.
```

The instance\-admin rename row swap shape

crates\/foundry\-app\/static\/js\/form\-errors\.js\:1

Consumer

UnchangedCompatible

```text
It routes a refusal at the slot current child through the unchanged data-error-target selector.
```

The board search panel insertion point

crates\/foundry\-app\/templates\/board\.html\:6

Producer

UnchangedCompatible

```text
The New issue trigger keeps its parent and its hx-get suffix, so the insertion point and the derived search URL are unchanged.
```

The board search panel insertion point

crates\/foundry\-app\/static\/js\/keyboard\.js\:629

Consumer

UnchangedCompatible

```text
It inserts the control and the panel as the trigger next sibling in the trigger own parent; that parent stays a plain block.
```

The dashboard witness selector \.dash

crates\/foundry\-app\/templates\/dashboard\_root\.html\:1

Producer

UnchangedCompatible

```text
The class stays on the same element; only its geometry declarations go.
```

The dashboard witness selector \.dash

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs\:950

Consumer

UnchangedCompatible

```text
It reads the colour of .dash, which is unchanged.
```

The served stylesheet name

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:1

Producer

Migrated

```text
The bytes change, so the content-hashed name changes.
```

The served stylesheet name

crates\/foundry\-app\/templates\/base\.html\:6

Consumer

Migrated

```text
The link takes the new name in the same commit.
```

The served stylesheet name

crates\/foundry\-app\/src\/lib\.rs\:344

Consumer

Migrated

```text
The three cache-policy assertions name the path literally.
```

The served stylesheet name

crates\/foundry\-app\/static\/VENDOR\.md\:33

Consumer

Migrated

```text
The inventory row name and its recomputed sha256.
```

The live board URL derived from the report anchor

crates\/foundry\-app\/templates\/board\.html\:5

Producer

UnchangedCompatible

```text
The anchor keeps its report-link class and its /report href; the secondary control classes are added beside them.
```

The live board URL derived from the report anchor

crates\/foundry\-app\/static\/js\/board\-live\.js\:65

Consumer

UnchangedCompatible

```text
It still finds a.report-link[href] and still strips the /report suffix, so the live board keeps refreshing.
```

The board header project key reading

crates\/foundry\-app\/templates\/report\.html\:4

Producer

Migrated

```text
The change report header carries the same idiom, so the key moves into its own data-key-prefix element there too and the one migrated reader resolves both pages.
```

##### Not applicable

contract_tests

```text
No party here can change apart from another. Every producer and consumer of the touched contracts is in this one deployable and changes in the same commit: the templates, the stylesheet, the two shipped scripts and the acceptance supports. Nothing is persisted or put on a wire by this Request, and no version of it outlives its producer, so the primary oracle covers the agreements instead.
```

prefactoring

```text
No behaviour-preserving move precedes this change. The one restructuring is the frame becoming the single padding owner, which IS the promised change and is observed by the new oracle, not by an existing GREEN one; an extracted but unapplied size class or section class would be dead CSS and would preserve nothing worth verifying.
```

##### Boundaries

```text
HTTP GET of the shipped signed-in app pages (/, the board, /teams, /workspace/invites, /admin/tokens, /account/settings, /admin/instance and the integrations pages) through the real server, read as served markup and rendered in the browser lane, with the computed style resolved in the live browser at the harness desk viewport and at the emulated 390 by 844 phone, in both palettes
```

###### Driven ports

```text
The served static stylesheet at its content-hashed /static path
```

```text
The shipped project, team, workspace, token and integration reads that already fill these pages
```

```text
Templates and the stylesheet depend on the token seam and on the shipped control classes; nothing new depends on them. No handler, store function, route or authorization read is added or reversed, so the dependency direction of the web tier is unchanged.
```

###### Failures

####### B1

```text
A submitted value breaks a stored bound (a team name over 64 characters, a workspace name over 24, a project name over 256, an expiry outside 1 to 365).
```

Refusal

```text
The shipped refusal copy in the existing error paragraph or error slot, unchanged by this Request; the field cap never refuses and no new message is added.
```

####### B2

```text
The stylesheet bytes change without the content-hashed name changing with them.
```

Refusal

```text
check-arch asset-hash refuses, naming the file, the claimed hash and the computed hash.
```

####### B3

```text
A new rule names a colour literal below the token seam, or the three token regions stop declaring the same token set.
```

Refusal

```text
check-arch token-seam S1 refuses with the file and line, or S2 refuses with the diverging token names.
```

##### Observations

```text
The computed inline start and block start of a page first content box, measured in the live browser against the frame own padding box, per viewport
```

pixels

```text
WHICH box on the path contributed an offset; two chains that sum to the same number are indistinguishable here, which is why the model carries the chain and the box count separately
```

```text
The computed inline size of a form field and the presence of page-wide horizontal overflow (documentElement scrollWidth against clientWidth)
```

pixels and a boolean

```text
Whether the cap or the content bound the width on a short value
```

```text
The computed fill, border colour, border width, radius and font family of a control, and its contrast ratio against its resolved ancestor background in both palettes
```

colour triples and ratios

```text
Perceived visual weight beyond the measured pair; it reads the tier a control resolves, not how loud it looks
```

```text
The served section structure of a page: the sections, one heading per section, and whether each submit control is inside the section holding its fields
```

a list of sections with their heading text and their action

```text
Reading order for assistive technology beyond DOM order
```

```text
The board header labelled key: the element carrying data-key-prefix and its text, and the box gap between the two action controls
```

a string and pixels

```text
Typography of the label
```

##### Equality

the frame observation at one viewport

```text
Two app pages are frame-equal when their first content box inline start and block start agree. Equality is NOT structural: a page may hold any sections, any list or any board, and two pages with different markup are equal here exactly when those two numbers agree.
```

the control observation in both palettes

```text
Two controls are of the same tier when their computed fill, border colour and border width agree in both palettes. Delete and Save are REQUIRED to be of different tiers; Delete, Change report and Integrations are required to be of the same tier as the shipped button.secondary.
```

##### Constructors

page\-frame

app\_shell\.html block app\_content \-\> the one padded content box\; a page supplies content and never its own padding box

field\-size

field kind \-\> one of short\, name\, long\; the default for an unclassed field is name

page\-section

heading plus fields plus its own action \-\> one section box with the shared rhythm

##### Laws

###### L1

```text
At each viewport, every signed-in app page first content box starts at the frame own inline start and block start, and no page start is a function of the window width. The chain from the shell to that box holds exactly one padding box. A page OUTSIDE the frame keeps its own page padding.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

###### Treatment

Finite

###### Cases

```text
Home at desk width and at 390 by 844
```

```text
The board at both viewports
```

```text
Teams at both viewports
```

```text
Workspace invites at both viewports
```

```text
Machine tokens at both viewports
```

```text
Account settings at both viewports
```

```text
The instance dashboard at both viewports
```

```text
The change report at both viewports
```

```text
/unsubscribe, which is outside the frame: its main keeps its own 20px page padding after the element rule is scoped to body > main
```

```text
The domain is the closed set of named app pages times two viewports, plus one out-of-frame page; the browser lane reads every member, so a property adds no reach. The Quint module covers the SUM ALGEBRA only and never reads the stylesheet, so this browser reading is the only check of the real file.
```

###### Evidence

docs\/feature\/ui\-page\-frame\/design\/page\_frame\.qnt

###### L2

```text
For any stored value within its own bound, a text field computed inline size stays at or below the cap that applies where it sits (its size class, the frame default, or no cap in a dialog or on a card page) and the page gains no horizontal overflow: documentElement scrollWidth equals clientWidth.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

###### Treatment

Broad

###### Generator

```text
Stored names and labels at their authority bounds: team names of 1 to 64 characters, project names of 1 to 256, workspace names of 1 to 24, emails up to 254, each as a mixture of spaced words and unbroken runs
```

256 characters\, the widest stored bound in the authority

A recorded seed per run\, printed on failure so the case replays

###### Must include

```text
A single unbroken 256-character run with no space
```

```text
A 24-character workspace name of wide glyphs
```

```text
A one-character name
```

```text
The same value in the issue edit dialog title field, whose width must equal its description textarea width inside the min(30rem, 92vw) dialog box
```

```text
The same value in a signed-out card page field, whose width must stay the card content width with no cap
```

1\: a field holds a flat string

```text
Shrink the value towards the shortest string that still overflows, and report it with its seed
```

```text
The browser own layout: the field getBoundingClientRect width against the cap resolved from the computed max-inline-size, the dialog title field width against its sibling textarea width, and the document scrollWidth against its clientWidth. None of those numbers is computed by the rule under test.
```

```text
A field whose cap is expressed as a width rather than a max-inline-size; a default cap applied outside the frame, which the card page and dialog readings catch by a field that is suddenly narrower than its box; or a row that lets a long unbroken value push the page wider, which the overflow reading catches and names with its field and value
```

###### Evidence

None.

###### L3

```text
Each enumerated control kind resolves the Foundry control box and no platform default: a 1px --cz-muted border, the --cz-surface fill, the shared radius, the inherited face and a 32px minimum height, with its label or glyphs reaching 4.5 to 1 and its boundary reaching 3 to 1 in both palettes.
```

Invariant

Adapter

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

###### Treatment

Finite

###### Cases

```text
select on Teams (default team), on team members (the member list) and on a Matrix route (the channel list), each inside the frame
```

```text
select in the issue edit dialog (the status list, partials/issue_edit_modal.html:23) and in the lane-delete dialog (the destination list, partials/delete_lane_modal.html:20), each inside the min(30rem, 92vw) dialog box and each uncapped
```

```text
input type=number on the token mint form (expiry days), at the short size
```

```text
input type=file on the issue page (the control and its file-selector button)
```

```text
input type=text and type=email on the touched framed forms, at their size class
```

```text
input type=text in the two dialogs (the edit and new-issue title fields) and type=password on a signed-out card page: the control family presentation applies, the frame default cap does not
```

```text
The domain is the closed set of control kinds and of the places a control sits: framed form, dialog, card page.
```

###### Evidence

None.

###### L4

```text
On Teams, the instance dashboard and account settings, every form sits in a section that holds exactly one heading and holds that form submit control inside the same section, and no section adds inline padding to the page first content box.
```

Invariant

Adapter

crates\/foundry\-app\/templates

###### Treatment

Finite

###### Cases

```text
Teams: the create-team section and the default-team section, each with its own heading and its own button
```

```text
The instance dashboard: a heading over the workspace group, with each rename form inside its own row and no UA bullet or indent
```

```text
Account settings: the workspace rows section and the Project emails section, the fieldset groups styled and the Save inside the Project emails section
```

```text
The domain is the three named pages the promise names.
```

###### Evidence

None.

###### L5

```text
In the issue edit dialog, Delete resolves the secondary tier and Save alone resolves the primary --cz-black fill, and Delete still fetches the confirm fragment it fetches today.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/partials\/issue\_edit\_modal\.html

###### Treatment

Finite

###### Cases

```text
The dialog open on a board card, device dark and device light: the two computed tiers differ and Delete is the one with no black fill
```

```text
Delete pressed: the confirm fragment replaces the modal root, as shipped
```

```text
One dialog, two controls, two palettes: a closed domain.
```

###### Evidence

None.

###### L6

```text
The board header and the change report header each expose the project key in its own element whose data-key-prefix equals the stored key, under the label Key prefix, and on the board Change report and Integrations each resolve the secondary tier with a non-zero gap between their boxes and no text-decoration underline, keeping their shipped classes and their /report href.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/board\.html

###### Treatment

Finite

###### Cases

```text
A board whose project carries a short key, read as served markup and in the browser
```

```text
The change report of the same project: the same label and its own data-key-prefix equal to the stored key
```

```text
A board where the operator may manage the project, so both controls are drawn
```

```text
A board where she may not, so only Change report is drawn and the row still reads as a row
```

```text
The live board after a change elsewhere: board-live.js still derives its URL from a.report-link[href] and the board still refreshes
```

```text
Two headers and a closed set of board states.
```

###### Evidence

None.

###### L7

```text
The served stylesheet name carries the sha256 prefix of its own bytes, every /static reference in the app resolves on disk, and the three readers of that name all name the same name.
```

Invariant

Infrastructure

xtask\/src\/check\_arch\.rs

###### Treatment

Finite

###### Cases

```text
The renamed stylesheet against its own bytes (check-arch R2)
```

```text
The link in base.html, the VENDOR.md row and the three lib.rs assertions (check-arch R1 and the recomputed VENDOR sha256)
```

```text
The gate already enumerates every asset and every reference.
```

###### Evidence

None.

##### Refuted

```text
One max-inline-size on the frame content box gives every app page the same width.
```

```text
Law scope, not carrier: the width equality holds over the frame content box, not over a capped reading column. A cap is a different promise and it binds the board.
```

```text
The board at 1920px with six lanes: a 76rem cap would make the lanes scroll inside the cap while the frame still has width to spare, and a centred cap would additionally restore a window-dependent left edge, which is the Home defect being removed.
```

```text
The element rule main { padding: 20px } can simply be deleted, because every main sits inside the frame.
```

```text
Boundary: four pages render main as a direct child of body, outside the app shell, and they are out of scope.
```

```text
email_stop, unsubscribe_confirm, unsubscribe_result and notifications: /unsubscribe would lose all page padding. The rule is scoped to body > main instead.
```

```text
A quiet Delete is a borderless text control.
```

```text
Boundary: the shipped control-boundary law of ADR-NAV-POLISH-007 and ADR-BARE-PAGES-002 requires an identifiable boundary at 3 to 1.
```

```text
A fill-less and border-less control has no boundary colour to measure at all, so the contrast reading cannot pass; the secondary tier is the quiet control that keeps the boundary.
```

##### Claims

###### C1

```text
A page cannot contribute a second padding box on the path from the app shell to its first content box.
```

```text
A page whose content starts at a different left edge than its neighbours, which is the defect the operator reported and which no test and no compiler sees today.
```

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

```text
Any app page template that extends app_shell.html
```

app\_shell\.html block app\_content\, the one padded content box

The browser layout of each served page

```text
The computed inline start and block start of the page first content box at both viewports
```

```text
CSS has no checker, so the exclusion is centralisation and not proof: any later rule, any inline style and any fragment swapped into the page can add a box; a page that does not extend app_shell.html is outside the frame entirely. The detectors are the all-pages browser reading and review, never a type.
```

###### C2

```text
A form field cannot be sized by a free value: its width comes from one of three named size classes, and an unclassed field takes the name size.
```

```text
A field stretched to the frame width, which is the 1100px field the operator reported.
```

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

```text
Any text, email, password, number or select control on an app form
```

The text\-input rule group with its three size classes

The browser layout of the form

```text
The computed inline size of the field and the absence of page-wide horizontal overflow
```

```text
A template may name no class, which is why the default is a size and not 100 percent; an inline style or a later rule can still override it. Rust and the template engine check neither, so the strength claimed is centralised construction, not a proved predicate.
```

##### States

Admitted

```text
Seven named app pages times two viewports times two palettes, each with the frame as the only padding box; fields at the three size caps; sections with one heading and their own action; Delete secondary; the board key as a labelled value.
```

crates\/foundry\-app\/templates

Excluded

```text
A page contributing a second padding box, a field at 100 percent of the frame, a control drawn in a platform default, a submit control outside the section holding its fields, and a Delete with the primary fill. Excluded by centralisation and by the oracle reading, not by any checker.
```

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css

Unknown

```text
A page added later that does not extend app_shell.html, a fragment swapped into a page that carries its own padding, and an operator font size or zoom far from the default: none is enumerated by the oracle, and the frame ownership over them rests on review.
```

solution architect

##### Models

Quint

docs\/feature\/ui\-page\-frame\/design\/page\_frame\.qnt

###### Properties

uniformFrame

Invariant

```text
Under the single-owner design, at both viewports, every page inline start agrees, every page block start agrees, no page start is window-dependent, and each page path holds exactly one padding box
```

\.app\-shell\_\_content

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:953

sameInlineStart

Invariant

```text
Every page inline start equals every other page inline start at the live viewport
```

\.app\-shell\_\_content padding\-inline

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:956

noWindowDependentStart

Invariant

```text
No page carries a centred column, so no page start is a function of the window width
```

\.dash

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:670

oneOwningPaddingBox

Invariant

```text
Each page path from the shell to its first content box holds exactly one padding box
```

body \> main

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:284

###### Atoms

Constant

rail

\.sidebar flex\-basis

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:1020

None.

Constant

pages

the templates extending app\_shell\.html

crates\/foundry\-app\/templates\/app\_shell\.html\:3

None.

State

choice

which writer owns the page padding

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:953

```text
Fixed once per run by init; one stylesheet cannot hold two owners at a time
```

State

viewport

the 480px phone media bound

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:1533

None.

Action

step

the operator resizing or changing device

crates\/foundry\-app\/static\/css\/foundry\.8a68a324\.css\:1727

None.

###### Assumptions

```text
The rail is 240px wide and the frame pads 24px block and 32px inline at desk width, 16px below the 480px phone breakpoint, as the shipped stylesheet declares
```

```text
A page content inline start is the rail plus the sum of the padding boxes on the path from the shell to the page first content box
```

```text
A centred column inline start is not a constant at all, so it is recorded as a value no uniform chain can produce
```

```text
The seven named pages carry the three box chains the screenshot review measured: framed, main-wrapped and centred
```

###### Mutants

######## N1

```text
mainPads: the shipped element rule main { padding: 20px } stays, so the main-wrapped pages keep a second box
```

###### Kills

sameInlineStart

oneOwningPaddingBox

######## N2

```text
homeCentres: the shipped centred .dash column stays, so the Home start is a function of the window
```

###### Kills

noWindowDependentStart

sameInlineStart

######## N3

```text
phonePerPage: the phone block pads one page family differently from the rest
```

###### Kills

sameInlineStart

##### Closure

```text
Every app page starts at the one frame edge (P1)
```

Safety

Pass

DesignedNotBuilt

```text
The Quint module verifies the invariant exhaustively over the four design variants and the two viewports, and the three mutants are each refuted with a counterexample naming the variant. The runtime is not built: no byte of the stylesheet or the templates is changed by this design.
```

###### Evidence

docs\/feature\/ui\-page\-frame\/design\/page\_frame\.qnt

```text
A page can be built that starts at the one frame edge
```

Inhabitation

Pass

DesignedNotBuilt

```text
app_shell.html already supplies the one padded content box and 30 templates already extend it, so a page needs only to add no box of its own. The model owner variant is inhabited at both viewports.
```

###### Evidence

docs\/feature\/ui\-page\-frame\/design\/page\_frame\.qnt

```text
The frame, the fields, the sections, the quiet Delete and the board header are reachable through the public port
```

PublicReachability

Pass

DesignedNotBuilt

```text
Every page in the chain is a shipped route a signed-in operator opens with a GET, and the issue dialog is a shipped htmx fragment she opens from a board card; the oracle reads each one as served markup and as computed style in the live browser. The presentation itself is designed and not built.
```

###### Evidence

None.

```text
Progress or eventual settling
```

Liveness

Pass

DesignedNotBuilt

```text
Not applicable, with a concrete reason: every observation in this Request is of a settled document. There is no queue, no retry, no background worker, no lock and no fairness assumption anywhere in it, and no state advances over time, so there is no progress obligation to check.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negates the assumption that the frame stays the only padding box: a page added later, or a section added to an existing page, pads its own first content box.
```

```text
The page still renders and every form still works.
```

```text
A wider gutter on that one page.
```

```text
A silently different left edge with no refusal anywhere, which is exactly the shipped defect returning.
```

```text
Only the all-pages browser reading detects it, so the oracle must enumerate the pages from the shipped frame users rather than from a hand-written list, and a page added later is not covered until it is enumerated.
```

acceptance designer

###### Y2

Stressor

```text
Negates the assumption that the search panel insertion point survives: the board header row becomes the New issue trigger parent.
```

```text
The slash key still opens the panel.
```

```text
Nothing.
```

```text
The injected search panel laid out as a flex row item beside the buttons.
```

```text
Keep the trigger parent a plain block; the oracle presses slash on the board and reads the panel box.
```

crafter

###### Y3

Stressor

```text
Negates the assumption that stored values are short: a 256-character project name with no space, in a sized field.
```

```text
The field stays at its cap and the value scrolls inside it.
```

```text
The value is clipped in view and reachable by caret.
```

```text
Page-wide horizontal overflow, which moves every other page element sideways.
```

```text
The broad property over generated values, with its seed and its shrink.
```

acceptance designer

###### Y4

Stressor

```text
Negates the assumption that the operator uses the device palette: she chooses light while the device is dark, or the reverse, on the new section headings, the quiet Delete and the two board controls.
```

```text
Every new surface resolves a token from both dark regions.
```

```text
Nothing.
```

```text
A new colour literal below the seam, or a pair that falls under 4.5 to 1 for text and 3 to 1 for a control boundary in either palette.
```

```text
check-arch S1 and S2 plus the contrast readings in both palettes; browser chrome still follows the device, which is the accepted limitation recorded in base.html.
```

crafter

###### Y5

Stressor

```text
Negates the assumption that scripting is on: the operator browses with no script, so the issue edit dialog does not exist at all.
```

```text
The board header controls are plain anchors and still follow their hrefs; the forms still POST.
```

```text
D1 is unreachable without scripting, as it is today.
```

```text
A board header control that only works with scripting.
```

```text
The two board controls stay anchors, never buttons; the no-script reading covers the header, not the dialog.
```

crafter

###### Y6

Stressor

```text
Negates the assumption that an un-indented list still reads as a hierarchy: an instance dashboard with a 24-character workspace name and several nested projects, with the UA bullets and indent removed.
```

```text
Every workspace and project row is readable and every rename form still swaps.
```

```text
A hairline or a background tier instead of an indent.
```

```text
Inline padding on the page first content box, which would undo B1 one section at a time, and a flat list in which a project cannot be told from a workspace.
```

```text
The section heading plus a hairline on the nested row group; the oracle reads the first content box inline start on this page as well.
```

crafter

###### Y7

SinglePointOfFailure

```text
One content-hashed stylesheet name with four readers (base.html, two VENDOR.md places, three lib.rs assertions).
```

```text
The page still serves.
```

```text
Nothing.
```

```text
A half-done rename: every browser holding the old bytes at that Cache-Control immutable URL is pinned stale, or the link resolves to no file and the app serves unstyled pages.
```

```text
check-arch R1 and R2 refuse both halves before the commit; the rename travels in one commit.
```

crafter

###### Y8

Bypass

```text
A page that does not extend app_shell.html, or a fragment swapped into a page by htmx that carries its own padding box: neither is reached by the frame rule.
```

```text
Nothing is claimed for them.
```

```text
They are outside the frame ownership.
```

```text
Claiming the frame law over a page the frame does not wrap.
```

```text
The law states the closed page set; a future page must extend app_shell.html or be designed in.
```

solution architect

###### Y9

Bypass

```text
appearance: none on a select, added later, removes the platform arrow.
```

```text
The box the stylesheet draws.
```

```text
Nothing.
```

```text
A list control with no affordance at all, or a drawn chevron as a colour literal below the token seam.
```

```text
The decision records why the arrow is kept; check-arch S1 refuses the literal.
```

crafter

##### Architectural tests

###### T1

```text
Every /static reference resolves on disk, every content-hashed filename is its own sha256 prefix, every VENDOR.md sha256 recomputes, no colour literal sits outside the three stylesheet token regions, and the three regions declare the identical colour-token set.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

###### T2

```text
The pre-paint head family stays render-blocking and every other head script keeps defer, so the frame change cannot silently move a script onto or off the render-blocking path.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

##### Contract tests

None.

## ADRs

- PAGE\-FRAME\-001 The frame owns page geometry — Accepted
- PAGE\-FRAME\-002 Three field sizes\, never full width — Superseded by PAGE-FRAME-004
- PAGE\-FRAME\-003 A quiet control keeps its boundary — Accepted
- PAGE\-FRAME\-004 Field cap belongs to the frame — Accepted
