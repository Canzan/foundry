# Architecture brief: ui\-bare\-pages

## Paradigm

procedural

## Shared records

### Decisions

None.

### Constraints

#### K1

```text
User decision 2026-11-20: the UI clean-up covers groups A to D, delivered as three Requests; this one is group A plus D3.
```

#### K2

```text
User decision 2026-11-20: A1 style the shortcuts page, A2 centred card for signed-out pages, A3 the issue full page shows title, description and status, D3 autocomplete on the six password fields.
```

#### K3

```text
User decision 2026-11-21: the first-admin claim password field is a new password (autocomplete new-password), not a current one.
```

#### K4

```text
User decision 2026-11-21: the issue full page shows the shipped status chip with the stored status value; human status labels are a later change.
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

crates\/foundry\-app\/src\/keyboard\.rs

Extend

```text
The total variant decision, the one row builder, the page view-model construction, and the Vary: Sec-Fetch-Dest response header set once beside that decision and carried by BOTH arms.
```

crates\/foundry\-app\/src\/views\.rs

Extend

```text
The page view-model beside the overlay one, and the four new issue-page fields.
```

crates\/foundry\-app\/src\/comments\.rs

Extend

```text
The issue page view-model gains title, description, status slug and the rendered chip from the existing projection read.
```

crates\/foundry\-app\/src\/issues\.rs

Extend

```text
The chip renderer becomes crate-visible; its bytes are unchanged.
```

crates\/foundry\-app\/templates\/partials\/keyboard\_shortcut\_rows\.html

Create

```text
The single owner of the shortcut rows and the selection instruction.
```

crates\/foundry\-app\/templates\/partials\/keyboard\_help\.html

Extend

```text
Keeps the dialog container and includes the rows partial.
```

crates\/foundry\-app\/templates\/keyboard\_help\_page\.html

Create

```text
The framed page arm of the shortcut list, on the card frame.
```

crates\/foundry\-app\/templates\/card\_page\.html

Create

```text
The chrome-less centred frame with the product name, extended by the signed-out family and the shortcut page.
```

crates\/foundry\-app\/templates\/signin\.html

Extend

```text
Card frame, and the two autocomplete tokens.
```

crates\/foundry\-app\/templates\/forgot\.html

Extend

```text
Card frame.
```

crates\/foundry\-app\/templates\/forgot\_sent\.html

Extend

```text
Card frame.
```

crates\/foundry\-app\/templates\/bootstrap\_claim\.html

Extend

```text
Card frame, and the username and created-password tokens.
```

crates\/foundry\-app\/templates\/invite\_accept\.html

Extend

```text
Card frame, and the created-password token on both fields.
```

crates\/foundry\-app\/templates\/reset\_password\.html

Extend

```text
Card frame, and the created-password token on both fields.
```

crates\/foundry\-app\/templates\/issue\.html

Extend

```text
Title, description region and status beside the unchanged key heading; presentation classes on the Delete anchor and the upload control.
```

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css

Extend

```text
The card block, the shortcut rows unscoped from the overlay host, the issue title, description and status rules and the file-input rules. The file carries the sha256 prefix of its own bytes, so this change renames it again and the old name goes.
```

crates\/foundry\-app\/templates\/base\.html

Extend

```text
The stylesheet link takes the new hash.
```

crates\/foundry\-app\/static\/VENDOR\.md

Extend

```text
The stylesheet row name and its recomputed sha256.
```

crates\/foundry\-app\/src\/lib\.rs

Extend

```text
The three cache-policy assertions that name the stylesheet path literally.
```

crates\/foundry\-acceptance\/tests\/features\/ui\-bare\-pages\.feature

#### Verification

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo test -p foundry-app"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=ui-bare-pages cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=keyboard cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=us-b04 cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=canzan-theme-system cargo test -p foundry-acceptance"
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance"
```

#### Supports

crates\/foundry\-acceptance\/src\/steps\/feature\_ui\_bare\_pages\.rs

crates\/foundry\-acceptance\/src\/steps\/mod\.rs

crates\/foundry\-acceptance\/tests\/features\/us\-k01\-keyboard\-templating\.feature

crates\/foundry\-acceptance\/src\/steps\/keyboard\_fragments\_templating\.rs

crates\/foundry\-acceptance\/tests\/features\/us\-12\-keyboard\-nav\.feature

crates\/foundry\-acceptance\/src\/steps\/us\_12\_keyboard\_nav\.rs

crates\/foundry\-acceptance\/tests\/features\/us\-b04\-signin\-layout\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_b\_web\_tier\.rs

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature

crates\/foundry\-acceptance\/src\/steps\/feature\_canzan\_theme\.rs

#### Public oracle

```text
Through the real public driving port. Signed out, with no session cookie, a visitor opens /sign-in, /forgot-password, the first-admin claim at /bootstrap with a live token, /invites/accept with a live id and signature, and /reset-password with a live token; each page is read as served markup and rendered in the browser lane at the harness desk viewport and at the emulated 390 by 844 phone device, in both palettes. A visitor NAVIGATES the browser to /keyboard-help from the rail link; separately the same URL is read by a plain HTTP client that declares no destination, by a client that declares the document destination, and by a script fetch from a loaded page that declares the empty destination, and the response HEADERS are read on each of those reads. A signed-in team member opens /team/{team}/project/{project}/issues/{n} for an issue with a title, a description and a known state, and again for an issue whose description is empty, and presses the shortcut key that opens the overlay on a board page.
```

```text
Each signed-out page is one full document that links the stylesheet from the application's own static path and draws exactly one centred card: the product name above the page's own single heading, the form inside the card, the card inside the viewport with visible margin at desk width and full width inside the page padding on the phone, and no navigation rail and no theme-named control anywhere on it. The sign-in email carries the username token and its password the current-password token; the claim email carries username and its password the created-password token; both invite-accept fields and both password-reset fields carry the created-password token; no other input gained one. The browser NAVIGATION to /keyboard-help returns a full styled document whose rows pair each shipped shortcut key with its description on one line, with the selection instruction below them, and the plain HTTP read and the script fetch of the same URL still return the bare fragment with no html wrapper, the dialog container with its role and label, and the identical rows in the identical order; EVERY one of those responses, page arm and fragment arm alike, carries Vary: Sec-Fetch-Dest; the overlay on the board still draws its keycaps and muted descriptions over the scrim. The issue page shows the key as its heading text, the stored title, the stored description with its line breaks preserved and nothing of it drawn as markup, the empty copy instead when nothing is stored, and the shipped status chip whose data-state and text are the stored status value; its Delete control still follows its href to the confirm page and its upload control is drawn with the stylesheet's own control type.
```

```text
The shipped build fails every one of these today: the shortcut navigation answers an unstyled fragment in browser-default serif with each key on its own line and no Vary header at all; the four signed-out forms run edge to edge at desk width with no card, no margin and no product name; the issue page shows the key with a lone Delete link, a raw file control, and no title, description or status; and all eight inputs carry no autocomplete token. Each law also fails on its own plausible wrong implementation: serving the framed page to every client is caught by the script fetch and the plain HTTP read, which must still get no html wrapper; setting Vary on the page arm alone is caught by the header read on the fragment arm, which is the body a cache is most likely to hold; letting the page build its own row list is caught by the row-identity read between the two arms; keeping the shortcut rules scoped to the overlay host is caught by the navigated page, whose keys would still sit on their own line; putting the title in the heading is caught by the heading-equals-key read; giving the claim form the current-password token is caught by reading the served token itself; rendering the stored description as markup is caught by the generated property with a script tag in the text; and editing the stylesheet without renaming it is caught by the shipped asset-hash layer.
```

##### Promises

P1

P2

P3

P4

#### Prefactoring

```text
L1 to L2, in crates/foundry-app/templates/partials/keyboard_help.html and crates/foundry-app/src/keyboard.rs: extract the shortcut rows and the selection instruction into partials/keyboard_shortcut_rows.html, included by the unchanged dialog container, and extract the table-to-view-model mapping inside show_keyboard_help into one private shortcut_entries(). No behaviour change: the route still answers the same bare fragment. The follow-up change then adds the page arm by including the SAME rows partial and calling the SAME builder, so a page advertising a different list is not something a crafter has to remember not to write.
```

```text
GET /keyboard-help answers the same bare fragment: the dialog container with its role and its label, the heading, one key-and-description pair per shipped row in order, the selection-instruction paragraph, and no html wrapper.
```

crates\/foundry\-acceptance\/tests\/features\/us\-k01\-keyboard\-templating\.feature

##### Verification

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 FOUNDRY_ACCEPTANCE_TAGS=keyboard cargo test -p foundry-acceptance"
```

#### Records

##### Decisions

```text
Shortcut page and overlay from ONE route. GET /keyboard-help keeps one URL and one row source. The arm is decided from the destination the CLIENT declares in its Sec-Fetch-Dest request header: the document destination, which only a top-level navigation carries, answers a framed page; every other declared destination and an absent header answer the bare fragment the route serves today, which is what keyboard.js assigns into the overlay host and what the shipped HTTP-lane read asserts. Page script cannot set that header, so the arm follows a property of the request rather than a name a caller picks. Carrier: a two-variant private enum HelpVariant in crates/foundry-app/src/keyboard.rs, decided once by a total private fn help_variant(headers) that answers Fragment for every input it does not recognise; the per-request state is that one value; no effects and no new error. Rows: one private fn shortcut_entries() over the SHORTCUTS table feeds both arms, so the two arms cannot advertise different lists. Layers. Domain: none, this is presentation. Application: none, the route stays public and stateless, so no session read and no rail. Adapter: partials/keyboard_shortcut_rows.html owns the shortcut rows and the selection instruction; partials/keyboard_help.html keeps the dialog container and includes them; keyboard_help_page.html extends the card page and includes the same rows under its own h1. Observation: the response body shape and the rows it lists.
```

```text
ONE chrome-less page frame for the signed-out family and the shortcut page. New templates/card_page.html extends base.html and renders one main.page-card holding a product-name line and then its own content block. signin.html, forgot.html, forgot_sent.html, bootstrap_claim.html, invite_accept.html, reset_password.html and the new keyboard_help_page.html extend it instead of base.html; each keeps its own h1, its own copy and its own form action, method, hidden fields and inputs unchanged. The product name is a brand LINE, not a heading, so every page keeps exactly one h1 and the shipped heading reads do not move. No sidebar partial and no theme control is included, because the shipped theme read asserts the sign-in screen has neither. Stylesheet: one .page-card block, centred with auto inline margins and a max inline size, on the raised surface tier the dashboard cards already use, and full width inside the page padding below the shipped phone breakpoint. Layers: Adapter only; no Rust carrier and no view-model field changes.
```

```text
The issue page reads as the issue. views::IssuePage gains title, description, status_slug and status_chip; comments.rs::render_issue_page fills them from ONE existing read, store::issue_edit_view over the project and number the handler already resolved, so there is no new SQL, no new store function and no new authorization decision. issue.html keeps its h1 holding the issue key alone, because a shipped browser read asserts that heading text EQUALS the key; the title takes its own h2 and the status sits beside the key. The description region is ALWAYS present: the stored text, escaped by the template engine and wrapped with preserved line breaks, or the empty copy, matching the shipped attachment and comment empty states. The comment markdown sanitizer in foundry-core is deliberately NOT consumed here: rendering issue markdown would add a product promise and a second consumer of that sanitizer, which a visual pass does not hold. The status element is the shipped chip: the handler renders it through issues.rs::render_state_chip and the template embeds the string with the no-escape filter, exactly as base.html embeds the site footer, so the chip markup keeps one owner and its data-state is the stored slug. The Delete anchor keeps its href mechanism and its data-action marker and takes the shipped secondary-button presentation; the upload form label and its file input take the stylesheet's own control tokens. Layers. Domain: none. Application: show_issue reads one more projection of the issue it already resolved, and a gap between the two reads refuses rather than renders a blank issue. Adapter: issue.html plus the title, description, status-chip and file-input rules. Observation: the page text and the chip's data-state.
```

```text
Autocomplete by flow on the eight named inputs: on signin.html the email takes username and the password takes current-password; on bootstrap_claim.html the email takes username and the password takes new-password, because that form CREATES the first admin's password and current-password there makes a password manager offer a stored secret instead of generating and saving one; on invite_accept.html and reset_password.html the password and its confirmation both take new-password. No other input in the repository gains a token in this Request. Every input keeps its type, name, required flag and order, so each shipped form POST, each hidden token and each server-side validation is untouched. Layers: Adapter only; no carrier and no state.
```

```text
ONE content-hash migration. The served stylesheet is the only file whose NAME changes: its bytes change, so the shipped asset-hash rule requires the name to carry the sha256 prefix of the new bytes, and the rename travels in the same commit to its three readers, the link in templates/base.html, the row and sha256 in static/VENDOR.md, and the three cache-policy assertions in crates/foundry-app/src/lib.rs. The shipped check-arch refuses a /static reference that does not resolve and a content-hashed name that is not its own hash, so a half-done rename is caught by the gate rather than by review.
```

```text
The shortcut rows are styled ONCE for both arms. The row rules move off the overlay-host prefix onto the rows' own class with the SAME declarations, so the overlay keeps the computed keycap background, keycap border and muted description colour the shipped theme read measures, and the navigated page gets the same rows. The overlay-only chrome stays scoped to the host: the fixed scrim on the non-empty host and the raised card on the dialog container, because on the page that chrome is the card page's own. This is why the navigated page draws browser-default serif with each key on its own line today: every shipped shortcut rule is host-scoped.
```

```text
Claim-to-checker. The one claim with a state-and-arm algebra is the help-variant decision and the row identity between the arms, a closed one-step function over the four destinations a client can declare, so Quint carries it at docs/feature/ui-bare-pages/design/keyboard_help_variant.qnt, verified exhaustively with the TLA-backed backend; TLA+ is not needed because there is no temporal or fairness claim, and Agda is not needed because no proof over an open domain is claimed. No model for the card geometry, the autocomplete tokens or the issue page content: each names no shared state, no multi-step protocol, no concurrency and no crash window, and each domain is the closed set of named pages or named inputs the finite laws enumerate. The one broad domain is the stored description text, carried by a generated property over the page render rather than by a model.
```

```text
Cache contract of the two-arm route. One URL answers two bodies by a REQUEST header, so any cache between the browser and the application would otherwise serve one arm's body for the other arm's request: a framed document handed to the overlay fetch, or a bare fragment painted as a whole page. Both arms therefore declare the same contract in their response: Vary: Sec-Fetch-Dest, which names the one request header the arm is decided from, so a shared or private cache keys its entry on that header and never crosses the arms. The header is set in one place, beside the arm decision in crates/foundry-app/src/keyboard.rs, and on BOTH arms, including the fragment arm that is unchanged in body; a Vary on one arm alone leaves the other arm's cached body free to answer the wrong request. Cache-Control: no-store on both arms was the alternative and is rejected: it forbids the browser's own back-forward reuse of the help page for no gain, while Vary keeps the body cacheable and correct. No other route changes, because no other route in the application decides a body from a request header the client declares.
```

##### Constraints

```text
Scope out stays out: app_shell.html, partials/sidebar.html, the board header, the issue dialog's Delete control and the general form, label and section-heading sizing belong to the next Request, ui-page-frame. The card's own max inline size is the only width rule this Request adds, and no rule changes who may open a page or what a form does.
```

```text
Shipped promises this Request must not break: GET /keyboard-help answers a body with no html wrapper to every request that does not declare the document destination; the overlay keeps its dialog container with its role and label and its key-and-description pairs; the sign-in screen carries no rail and no theme-named control and renders in both palettes; the sign-in and forgot pages stay full documents linking the stylesheet from the application's own static path; the issue page heading text equals the issue key; the board status chip keeps the slug as its data-state AND its text; no colour literal lives outside the three stylesheet token regions and the three regions declare the identical token set; every static reference resolves and every content-hashed name is its own sha256 prefix; the user menu still links to /keyboard-help; the pre-paint head scripts keep no defer, async or module attribute and every other head script keeps defer.
```

```text
No authority bounds the stored issue title or description length, and the store holds both as unbounded text, so the page shows whatever is stored, wrapped, with no truncation and no ellipsis. That is wider than any bound because no bound exists; a truncating page would be a product decision this Request does not hold. The shortcut list is the closed seven-row shipped table.
```

```text
P4 names eight inputs: the email and the password on sign-in and on first-admin claim, and the password and its confirmation on invite accept and password reset. The Request's six fields is the count of PASSWORD inputs in the screenshot review, at signin.html:9, bootstrap_claim.html:8, invite_accept.html:11-12 and reset_password.html:9-10; the two email inputs come from P4's own wording. The email input of forgot.html gets no token, because no promise names it.
```

```text
The first-admin claim password carries the created-password token, decided by the user on 2026-11-21 and now the wording of P4 itself: that form CREATES the password, and the current-password token would make a password manager offer a stored secret instead of generating and saving one. No question is open here, and the eight-row autocomplete law is unchanged.
```

```text
The shortcut page carries no rail. GET /keyboard-help is public and session-free by its own shipped rationale, the sign-in page loads the keyboard layer too, and the app shell needs a resolved nav identity. Adding a session read would change who the route serves, which scope-out forbids. Its same look as the rest of Foundry is the card frame, the stylesheet and the typography, not the rail.
```

```text
The issue page shows the stored status value in the shipped chip, decided by the user on 2026-11-21: the chip is one element whose text the board swap asserts equals the stored slug, so the page embeds that same renderer and shows that same value. Human status labels are a later change and no question is open here.
```

```text
The shortcut route's two arms are distinguished by a request header, so every response from GET /keyboard-help, page arm and fragment arm alike, carries Vary: Sec-Fetch-Dest. A response from either arm without it is a defect even when its body is right, because the defect appears only at a cache and only for the OTHER arm's reader.
```

##### Reuse

show\_keyboard\_help

crates\/foundry\-app\/src\/keyboard\.rs\:317

Extend

```text
The one owner of the route. It gains the total variant decision and reads the request headers; its public, session-free contract and its 200 HTML response are unchanged, so no caller and no link moves.
```

SHORTCUTS

crates\/foundry\-app\/src\/keyboard\.rs\:68

Reuse

```text
The single source of the advertised rows. Both arms derive from it through one new private builder, so an added row reaches the page and the overlay together; a page with its own hard-coded list is the drift this file's own comment warns about.
```

KeyboardHelp

crates\/foundry\-app\/src\/views\.rs\:233

Extend

```text
The overlay view-model keeps its selector-identical contract and now includes the extracted rows partial; a sibling page view-model carries the same two fields, so neither arm owns a second copy of the row shape.
```

keyboard\-help

crates\/foundry\-app\/templates\/partials\/keyboard\_help\.html\:1

Extend

```text
Keeps the dialog container with its role and label that the shipped templating read asserts, and includes the rows partial instead of declaring the rows itself.
```

keyboard\_shortcut\_rows\.html

crates\/foundry\-app\/templates\/partials\/keyboard\_shortcut\_rows\.html\:1

CreateNew

```text
No template today owns the shortcut rows apart from the dialog container that wraps them, and the page arm needs the rows WITHOUT that container: a page is not a dialog. Extracting them gives both arms one row owner; including the whole dialog on the page would put a dialog role on a document.
```

keyboard\_help\_page\.html

crates\/foundry\-app\/templates\/keyboard\_help\_page\.html\:1

CreateNew

```text
The page arm needs a full document with its own h1; the overlay fragment cannot be that document, and no shipped template renders the shortcut list as a page.
```

card\_page\.html

crates\/foundry\-app\/templates\/card\_page\.html\:1

CreateNew

```text
No template owns a chrome-less centred frame: the signed-out templates each place their content directly in base.html's content block, and app_shell.html is the rail frame a signed-out page and the public shortcut page may not render. One new frame is what keeps the card, its product name and its width in a single place instead of seven.
```

base\.html

crates\/foundry\-app\/templates\/base\.html\:6

Extend

```text
The one owner of the head, the stylesheet link, the pre-paint scripts and the footer. The card page extends it rather than declaring a second head; the only edit here is the stylesheet link taking the new content hash.
```

Sign in

crates\/foundry\-app\/templates\/signin\.html\:4

Extend

```text
Keeps its h1, its error paragraph, its OIDC link, its forgot link and its form exactly as shipped; it changes which frame it extends and its two inputs gain their autocomplete tokens.
```

Forgot password

crates\/foundry\-app\/templates\/forgot\.html\:4

Extend

```text
A signed-out page of the same flow, paired with sign-in by the shipped layout expectation, so it takes the same card frame. Its email input gains no token, because no promise names it.
```

Check your email

crates\/foundry\-app\/templates\/forgot\_sent\.html\:4

Extend

```text
The confirmation the forgot form lands on; leaving it bare would show the operator one styled page and then an unstyled one inside a single flow.
```

Claim your workspace

crates\/foundry\-app\/templates\/bootstrap\_claim\.html\:4

Extend

```text
The first-admin claim page named by P2 and P4: it takes the card frame, and its email and password inputs take their tokens; its four inputs, its token query and its submit are unchanged.
```

Set your password

crates\/foundry\-app\/templates\/invite\_accept\.html\:4

Extend

```text
The invite-accept page named by P2 and P4: card frame, and both password inputs take the created-password token; its three hidden fields are untouched.
```

Choose a new password

crates\/foundry\-app\/templates\/reset\_password\.html\:4

Extend

```text
The password-reset page named by P2 and P4: card frame, and both password inputs take the created-password token; its hidden token field is untouched.
```

issue\_key

crates\/foundry\-app\/templates\/issue\.html\:4

Extend

```text
The one owner of the issue full page. Its h1, its Delete anchor mechanism, its attachment list, its upload form, its timeline and its comment loop stay; the title, the description region and the status join the header, and the Delete anchor and the file input take shipped presentation classes.
```

IssuePage

crates\/foundry\-app\/src\/views\.rs\:1077

Extend

```text
The page's one view-model. Four fields join it and the handler keeps building every value, so the template still only renders.
```

render\_issue\_page

crates\/foundry\-app\/src\/comments\.rs\:692

Extend

```text
The single place the issue page view-model is materialized; it fills the four new fields from the existing read, so no handler grows a second render path.
```

issue\_edit\_view

crates\/foundry\-store\/src\/lib\.rs\:2558

Reuse

```text
The shipped projection that already answers title, description and state for the edit dialog, over the project id and number the page handler holds. A new query or a widened lookup row would make two owners of the same projection.
```

render\_state\_chip

crates\/foundry\-app\/src\/issues\.rs\:699

Extend

```text
The one owner of the status chip markup, today serving the board's drag-and-drop swap. It becomes crate-visible so the issue page embeds the SAME chip; a second chip shape on the page would let the two drift.
```

kb\-overlay\-root

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1411

Extend

```text
The one served stylesheet and this feature's main carrier: the card block, the shortcut row rules unscoped from the overlay host, the issue title, description and status rules, and the file-input rules. Every value names an existing token, and the file is renamed to the sha256 prefix of its new bytes.
```

VENDOR\.md

crates\/foundry\-app\/static\/VENDOR\.md\:33

Extend

```text
The asset register that records the stylesheet name and its sha256; it is a reader of the content-hashed name, so the rename travels here in the same commit.
```

static\_cache\_control\_value

crates\/foundry\-app\/src\/lib\.rs\:344

Extend

```text
Three cache-policy assertions name the stylesheet path literally, so they are readers of the content-hashed name and move with the rename.
```

##### Impact

handler

crates\/foundry\-app\/src\/keyboard\.rs\:317

Changed

```text
Gains the total variant decision, the shared row builder and the page view-model; route, path, status and public access unchanged.
```

view\-model

crates\/foundry\-app\/src\/views\.rs\:233

Changed

```text
The overlay view-model keeps its fields; a page sibling joins it, and IssuePage gains four fields.
```

handler

crates\/foundry\-app\/src\/comments\.rs\:692

Changed

```text
Fills the four new issue-page fields from the existing projection read; the comment, attachment and timeline assembly is untouched.
```

handler

crates\/foundry\-app\/src\/issues\.rs\:699

Changed

```text
The chip renderer becomes crate-visible; its output bytes are unchanged, so the board swap is unaffected.
```

template

crates\/foundry\-app\/templates\/partials\/keyboard\_help\.html\:1

Changed

```text
Keeps its dialog container and includes the extracted rows partial; the served bytes stay selector-identical.
```

template

crates\/foundry\-app\/templates\/partials\/keyboard\_shortcut\_rows\.html\:1

Changed

```text
New: the single owner of the shortcut rows and the selection instruction, included by both arms.
```

template

crates\/foundry\-app\/templates\/keyboard\_help\_page\.html\:1

Changed

```text
New: the framed page arm of the shortcut list.
```

template

crates\/foundry\-app\/templates\/card\_page\.html\:1

Changed

```text
New: the chrome-less centred frame the signed-out family and the shortcut page extend.
```

template

crates\/foundry\-app\/templates\/signin\.html\:1

Changed

```text
Extends the card frame; its two inputs take their tokens; form and copy unchanged.
```

template

crates\/foundry\-app\/templates\/forgot\.html\:1

Changed

```text
Extends the card frame; no token added.
```

template

crates\/foundry\-app\/templates\/forgot\_sent\.html\:1

Changed

```text
Extends the card frame so the flow does not change look between form and confirmation.
```

template

crates\/foundry\-app\/templates\/bootstrap\_claim\.html\:1

Changed

```text
Extends the card frame; email and password take their tokens.
```

template

crates\/foundry\-app\/templates\/invite\_accept\.html\:1

Changed

```text
Extends the card frame; both password inputs take the created-password token.
```

template

crates\/foundry\-app\/templates\/reset\_password\.html\:1

Changed

```text
Extends the card frame; both password inputs take the created-password token.
```

template

crates\/foundry\-app\/templates\/issue\.html\:4

Changed

```text
Title, description region and status join the header; the heading, the Delete mechanism and every form are preserved.
```

stylesheet

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1411

Migrated

```text
New card, row, issue and file-input rules, the shortcut rows unscoped from the overlay host, and the file renamed to the sha256 prefix of its new bytes; browsers holding the old immutable URL are unaffected because the old name is gone and the new link ships with it.
```

template

crates\/foundry\-app\/templates\/base\.html\:6

Migrated

```text
The stylesheet link takes the new content hash; nothing else in the head moves.
```

asset\-register

crates\/foundry\-app\/static\/VENDOR\.md\:33

Migrated

```text
The stylesheet row name and its recomputed sha256.
```

test

crates\/foundry\-app\/src\/lib\.rs\:344

Migrated

```text
The three cache-policy assertions that name the stylesheet path literally.
```

client\-script

crates\/foundry\-app\/static\/js\/keyboard\.js\:323

UnchangedCompatible

```text
Its fetch declares the empty destination, so it keeps receiving the same bare fragment and keeps assigning it into the overlay host; no header is added to it.
```

template

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:78

UnchangedCompatible

```text
The rail link keeps its href, and a click on it is now the navigation that gets the framed page.
```

template

crates\/foundry\-app\/templates\/dashboard\_root\.html\:32

UnchangedCompatible

```text
The dashboard link keeps its href and gets the framed page for the same reason.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-12\-keyboard\-nav\.feature\:82

UnchangedCompatible

```text
Its HTTP read declares no destination, so it still receives the fragment with no html wrapper its step asserts.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-k01\-keyboard\-templating\.feature\:75

UnchangedCompatible

```text
The dialog container, its role, its label and its key-and-description pairs stay in the overlay arm, and the rows move to an included partial, not to inline Rust markup.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-b04\-signin\-layout\.feature\:44

UnchangedCompatible

```text
The card frame extends base.html, so sign-in and forgot stay full documents that link the stylesheet from the application's own static path.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-06\-signin\.feature\:1

UnchangedCompatible

```text
Credentials, errors, cookies and the anti-forgery contract are untouched; only the frame around the form changes.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/canzan\-theme\-system\.feature\:1

UnchangedCompatible

```text
Its sweep walks the board, the dashboard, an issue, the shortcut list and the sign-in screen. The overlay keeps its host-scoped selectors and its computed keycap and description colours, the sign-in screen grows no rail and no theme control, and every new value names an existing token.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/issue\-edit\-modal\-close\-icon\.feature\:1

UnchangedCompatible

```text
Its navigation oracle asserts the issue page heading text EQUALS the key, which is why the key keeps the h1 alone and the title takes an h2.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/issue\-status\-move\.feature\:1

UnchangedCompatible

```text
The chip renderer's output bytes do not change, so the swapped board chip is identical.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/issue\-card\-delete\.feature\:1

UnchangedCompatible

```text
The Delete anchor keeps its href and its data-action marker; it gains a presentation class only.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-10\-comments\.feature\:1

UnchangedCompatible

```text
The comment list, its markers and the add-comment form are untouched on the page.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/us\-11\-attachments\.feature\:1

UnchangedCompatible

```text
The attachment list, its markers and the upload form keep their shape; the label and the file input gain presentation only.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/navigation\-bar\-linear\-ui\.feature\:110

UnchangedCompatible

```text
The user menu still links to /keyboard-help, because the URL does not change.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/keycloak\-sso\.feature\:1

UnchangedCompatible

```text
The sign-in page keeps its OIDC link and its action; only its frame changes.
```

acceptance\-lane

crates\/foundry\-acceptance\/tests\/features\/external\-integration\-plugins\-01\-signed\-out\.feature\:1

UnchangedCompatible

```text
A signed-out read of the sign-in surface; the redirect and the copy are unchanged.
```

architectural\-check

xtask\/src\/check\_arch\.rs\:1698

UnchangedCompatible

```text
The asset-hash and asset-reference layers need no change: they are what refuses a half-done stylesheet rename.
```

##### Agreement

the GET \/keyboard\-help response body shape

crates\/foundry\-app\/src\/keyboard\.rs\:317

Producer

Migrated

```text
Serves a second arm, the framed page, to a declared document destination; the fragment arm is byte-compatible for every other request.
```

the GET \/keyboard\-help response body shape

crates\/foundry\-app\/static\/js\/keyboard\.js\:323

Consumer

UnchangedCompatible

```text
Its fetch declares the empty destination and keeps receiving the bare fragment it assigns into the overlay host.
```

the GET \/keyboard\-help response body shape

crates\/foundry\-app\/templates\/partials\/sidebar\.html\:78

Consumer

Migrated

```text
A click on the rail link is a navigation, so it now receives the framed page instead of the bare fragment; the href is unchanged.
```

the GET \/keyboard\-help response body shape

crates\/foundry\-app\/templates\/dashboard\_root\.html\:32

Consumer

Migrated

```text
The dashboard link receives the framed page for the same reason.
```

the GET \/keyboard\-help response body shape

crates\/foundry\-acceptance\/src\/steps\/us\_12\_keyboard\_nav\.rs\:370

Consumer

UnchangedCompatible

```text
Its HTTP read declares no destination, so the no-html-wrapper assertion still holds.
```

the content\-hashed stylesheet URL under \/static\/css

crates\/foundry\-app\/static\/css\/foundry\.2f9e0cb1\.css\:1

Producer

Migrated

```text
The bytes change, so the name takes the sha256 prefix of the new bytes and the old URL stops existing.
```

the content\-hashed stylesheet URL under \/static\/css

crates\/foundry\-app\/templates\/base\.html\:6

Consumer

Migrated

```text
The link takes the new name in the same commit.
```

the content\-hashed stylesheet URL under \/static\/css

crates\/foundry\-app\/static\/VENDOR\.md\:33

Consumer

Migrated

```text
The register row and its sha256 take the new name in the same commit.
```

the content\-hashed stylesheet URL under \/static\/css

crates\/foundry\-app\/src\/lib\.rs\:361

Consumer

Migrated

```text
The three cache-policy assertions name the path literally and take the new name in the same commit.
```

the status chip markup\, one span carrying the state slug as data\-state and as its text

crates\/foundry\-app\/src\/issues\.rs\:699

Producer

UnchangedCompatible

```text
The renderer's output bytes are unchanged; only its visibility widens.
```

the status chip markup\, one span carrying the state slug as data\-state and as its text

crates\/foundry\-app\/templates\/issue\.html\:4

Consumer

Migrated

```text
A new consumer: the page embeds the rendered chip string, so the page and the board swap can never draw two different chips.
```

the status chip markup\, one span carrying the state slug as data\-state and as its text

crates\/foundry\-app\/static\/js\/board\-dnd\.js\:1

Consumer

UnchangedCompatible

```text
The board swap keeps receiving the same bytes from the same renderer.
```

##### Not applicable

contract_tests

```text
Every producer and consumer named in the agreement analysis lives in the one deployable and ships in the same commit, so the primary oracle covers them. The one datum that outlives a version is the immutable stylesheet URL a browser caches, and the shipped asset-reference and asset-hash layers of check-arch already refuse a name that is not its own hash and a reference that does not resolve; a producer-to-consumer contract test would duplicate that gate.
```

##### Boundaries

```text
The shipped HTTP routes of foundry-app, driven as a real browser navigation and as a plain HTTP read: GET /keyboard-help, GET /sign-in, GET /forgot-password, GET /bootstrap with its token, GET /invites/accept with its id and signature, GET /reset-password with its token, and GET /team/{team}/project/{project}/issues/{n}; plus the same shortcut URL fetched as a script would fetch it.
```

###### Driven ports

```text
Postgres through foundry-store, one existing projection read for the issue page
```

```text
the served static stylesheet under /static/css, read by the browser from the link in the shared head
```

```text
Templates and the stylesheet (adapter) depend on the app handlers; the handlers depend on foundry-services and foundry-store; no template reads the store and nothing new points outward. The shortcut route keeps no dependency on the session layer.
```

###### Failures

####### B1

```text
The projection read in crates/foundry-app/src/comments.rs show_issue (the handler at comments.rs:58, which resolves the issue first and reads its title, description and state second) answers nothing for an issue the resolution just admitted, because the issue was deleted between the two reads.
```

Refusal

```text
The shipped uniform not-found page of crates/foundry-app/src/bootstrap.rs resource_not_found_page, never a page with an empty title or a blank status. UNOBSERVED by this Request, and deliberately so: the gap is a race between two reads of one handler, and no public stimulus can hold it open. The driving port offers no way to delete the issue between them, and the handler takes the concrete store over a live pool, so there is no seam a foundry-app test could use to answer the first read and refuse the second; a test would have to invent that seam, which is production design this Request does not hold. What IS observed is the refusal BODY: the same uniform not-found page the shipped lanes already read for an issue that is absent before the handler starts. The residual risk row for the two-read gap carries the owner.
```

####### B2

```text
That same projection read errors, for example on a lost database connection, after the resolution succeeded.
```

Refusal

```text
The shipped internal-error path of show_issue, through the handler's existing internal_error helper in crates/foundry-app/src/comments.rs, logged with its label and answering 500 with no partial page. UNOBSERVED by this Request, for the same reason as the read that answers nothing: a store error is not reachable from the public driving port, and the handler has no injection seam for one. The existing error helper is the one that already covers every other read in this handler, so this arm adds no new behaviour to observe; it names which arm the new read joins.
```

####### B3

```text
A client navigates to the shortcut URL without declaring a destination.
```

Indeterminate

```text
The bare fragment, today's unstyled body: the server cannot tell that navigation from a fetch, and it refuses nothing.
```

##### Observations

```text
The body shape GET /keyboard-help answers
```

framed document or bare section

```text
Two clients that declare the same destination: it cannot see whether a human or a script asked, only what the request declared.
```

```text
The shortcut rows a response lists, in order
```

ordered list of key and description pairs

```text
Which arm drew them, and whether a key is actually BOUND in the client.
```

```text
The rendered box of a signed-out form and of the shortcut page
```

one centred card with the product name\, read at the harness desk viewport and at the emulated phone device

```text
A pixel value at an untested width, and what the form does when it is submitted.
```

```text
The autocomplete token of a named input, as served
```

token string or absent

```text
Whether a particular password manager really fills or saves; only the declared token is observable.
```

```text
The text and the chip the issue page draws
```

key\, title\, description region and status slug

```text
Markdown source from its rendered form, because the region shows the stored text as written.
```

##### Equality

the shortcut rows a response lists

```text
Two responses are equal when they list the same rows in the same order. The dialog container, the page frame and the heading level around them are not part of this equality, which is what lets one row partial serve both arms.
```

the autocomplete token of a named input

```text
Two served forms are equal when every named input carries the same token. Attribute order, whitespace and the surrounding frame are not part of this equality.
```

##### Constructors

help\_variant

fn help\_variant\(headers\: and HeaderMap\) \-\> HelpVariant

shortcut\_entries

fn shortcut\_entries\(\) \-\> Vec\<views\:\:ShortcutEntry\>

KeyboardHelpPage

struct KeyboardHelpPage \{ entries\: Vec\<ShortcutEntry\>\, selection\_instruction\: String \}

IssuePage

struct IssuePage \{ issue\_key\, title\, description\, status\_slug\, status\_chip\, \.\.\. \}

##### Laws

###### L1

```text
A request to GET /keyboard-help that declares the document destination receives a framed page: a full document that links the stylesheet and carries the shortcut rows under its own heading. Every other declared destination, and a request that declares none, receives the bare fragment with no html wrapper.
```

Invariant

Adapter

crates\/foundry\-app\/src\/keyboard\.rs

###### Treatment

Finite

###### Cases

```text
declared destination document: framed page
```

```text
declared destination empty, which is what the overlay fetch sends: bare fragment
```

```text
declared destination iframe: bare fragment
```

```text
no destination header at all: bare fragment
```

```text
The destinations a client can declare on this route are a closed set and the decision is one step with no state, so these four rows exhaust the domain and a generator would redraw the same table.
```

###### Evidence

docs\/feature\/ui\-bare\-pages\/design\/keyboard\_help\_variant\.qnt

###### L2

```text
The page and the fragment advertise exactly the shipped shortcut rows, in the same order, with no row added, dropped or reordered, because both arms derive them from the one table through one builder and render the one rows partial.
```

Invariant

Adapter

crates\/foundry\-app\/src\/keyboard\.rs

###### Treatment

Finite

###### Cases

```text
the seven shipped rows in the fragment arm
```

```text
the same seven rows, same order, in the page arm
```

```text
a row added to the table appears in both arms
```

```text
The table is the closed seven-row shipped list, so the rows are enumerable and the law is a row-identity check between two renders.
```

###### Evidence

docs\/feature\/ui\-bare\-pages\/design\/keyboard\_help\_variant\.qnt

###### L3

```text
Each signed-out page of the auth family and the shortcut page is one full document that links the stylesheet from the application's own static path and draws exactly one centred card carrying the product name and that page's single h1, with no navigation rail and no theme control, at the harness desk viewport and at the emulated phone device.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/card\_page\.html

###### Treatment

Finite

###### Cases

```text
sign-in
```

```text
forgot password
```

```text
forgot sent
```

```text
first-admin claim
```

```text
invite accept
```

```text
password reset
```

```text
the shortcut page
```

```text
The pages are the closed set this Request names, and each is read once per viewport, so the seven rows exhaust the domain.
```

###### Evidence

None.

###### L4

```text
Each of the eight named inputs carries exactly the autocomplete token its flow needs, as served: username and current-password on sign-in, username and new-password on first-admin claim, new-password on both invite-accept fields and on both password-reset fields; and no other input in the repository carries a token this Request added.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/signin\.html

###### Treatment

Finite

###### Cases

```text
sign-in email: username
```

```text
sign-in password: current-password
```

```text
claim email: username
```

```text
claim password: new-password
```

```text
invite accept password and confirm: new-password
```

```text
reset password and confirm: new-password
```

```text
forgot email: no token
```

```text
The inputs are the closed set P4 names, so the rows exhaust the domain.
```

###### Evidence

None.

###### L5

```text
The issue full page draws the issue key as its heading text, the stored title, exactly one description region, and the status chip whose data-state is the stored state slug; its Delete control keeps the href that is its whole mechanism.
```

Invariant

Adapter

crates\/foundry\-app\/templates\/issue\.html

###### Treatment

Finite

###### Cases

```text
an issue with a title and a description: title shown, description region holds the stored text
```

```text
an issue with an empty description: the description region holds the empty copy, and there is still exactly one region
```

```text
an issue in each shipped state: the chip data-state equals the stored slug
```

```text
the heading text equals the key, never the key and the title together
```

```text
The states are the project's closed lane set and the description is either present or empty, so the rows exhaust the structural domain; the text domain itself is carried by the generated property.
```

###### Evidence

None.

###### L6

```text
For any stored description text, the issue page shows that text with its line breaks preserved, escaped so no stored character is drawn as markup, and never truncated.
```

Invariant

Adapter

crates\/foundry\-app\/src\/comments\.rs

###### Treatment

Broad

###### Generator

```text
arbitrary unicode text for the description, including the empty string, HTML and markdown markup such as a script tag, an image tag with an event attribute, angle brackets, ampersands, quotes and backticks, CRLF and LF runs, long unbroken words, and right-to-left text
```

4000 characters\, 24 draws per run

No generated\-test library\: the acceptance crate carries none\, so the oracle draws from a standard\-library\-style StdRng seeded with the fixed value 20261121\, which makes every run reproduce the same 24 cases from the seed printed in the failure message\. There is no persisted failure file\; a failure is replayed from that seed and from the offending text the message carries\.

###### Must include

```text
the empty string
```

```text
a script tag
```

```text
a CRLF run
```

```text
a 4000-character unbroken word
```

flat text\, no nesting

```text
No library shrinking. The oracle reports the SHORTEST offending text it drew, so the failure message names the smallest counterexample in the run rather than the first; a smaller one outside the drawn set is not searched for, and that limit is stated with the result.
```

```text
the rendered page contains the stored text with each markup character replaced by its entity, computed by an escape reference in the test rather than by the render under test, and the page holds exactly one description region
```

```text
a render that lets a script tag through unescaped, that drops text past a length, or that collapses the stored line breaks into one line
```

###### Evidence

None.

###### L7

```text
The served stylesheet's name carries the sha256 prefix of its own bytes, and every static reference in the application resolves on disk.
```

Invariant

Infrastructure

crates\/foundry\-app\/static\/css

###### Treatment

NoTest

```text
The shipped asset-reference and asset-hash layers of cargo xtask check-arch are this law's executable owner, recorded as an architectural test; a second oracle would duplicate the gate.
```

###### Evidence

None.

###### L8

```text
Every response of GET /keyboard-help, from both arms, carries Vary: Sec-Fetch-Dest, so no cache can answer one arm's request with the other arm's body.
```

Invariant

Adapter

crates\/foundry\-app\/src\/keyboard\.rs

###### Treatment

Finite

###### Cases

```text
a request declaring the document destination: the framed page AND the Vary header
```

```text
a request declaring the empty destination, as the overlay fetch does: the bare fragment AND the same Vary header
```

```text
a request declaring no destination: the bare fragment AND the same Vary header
```

```text
The arms are the closed pair the variant law enumerates, and the header is one named value on each, so three rows exhaust the domain; a generated property would redraw the same table.
```

###### Evidence

None.

##### Refuted

```text
Serving the framed page to every client of GET /keyboard-help is adequate, because the overlay would still show the list.
```

```text
The overlay assigns the response into its host with an inner-HTML write, so a framed document would nest a second head and body inside the live page, and the shipped HTTP read asserts the response carries no html wrapper. The carrier must therefore keep two arms, not one widened body.
```

```text
keyboard.js fetches the URL at line 323 and assigns the markup at line 331; us_12_keyboard_nav.rs line 370 asserts the body contains neither a doctype nor an html tag.
```

```text
One stylesheet rule set scoped to the overlay host can style both the overlay and the page.
```

```text
The page renders the rows outside that host, so a host-scoped selector cannot match them. The rows need their own class and the host keeps only the overlay chrome.
```

```text
Every shipped shortcut rule from line 1399 to line 1449 of the stylesheet is prefixed with the overlay host, which is exactly why the navigated page draws browser-default serif with each key on its own line.
```

```text
The issue page can carry the key and the title together in its heading.
```

```text
A shipped browser read asserts the heading text EQUALS the key, so a combined heading is refused by the existing oracle; the title takes its own second-level heading.
```

```text
feature_issue_edit_modal_close.rs lines 837 to 845 assert that the heading text equals the issue key.
```

```text
A marker header added by the client script can mark the fragment arm, leaving the page as the default.
```

```text
The default arm would then be the page, and every shipped client that sends no marker, including the HTTP read and any other fetch, would receive a framed document. The safe default has to be the fragment, so the POSITIVE marker must be on the page arm, which only the browser's own declared destination supplies.
```

```text
us_12_keyboard_nav.rs line 370 reads the route with a plain HTTP client that sends no such header and asserts a bare body.
```

##### Claims

###### C1

```text
The arm GET /keyboard-help serves is one of exactly two variants, decided in one total function from the request headers.
```

```text
A framed document assigned into the overlay host, or a navigation that lands on an unstyled fragment: the two defects this Request exists to prevent.
```

crates\/foundry\-app\/src\/keyboard\.rs

```text
any header map, including an unknown destination value, an empty value and no header at all
```

help\_variant

show\_keyboard\_help

```text
the response body shape of GET /keyboard-help
```

```text
A client that navigates without declaring a destination receives the fragment, which is the safe arm and today's behaviour. Rust's checker makes neither arm unrepresentable for a future caller that renders the page view-model directly, so the finite acceptance read holds that line.
```

###### C2

```text
Both arms draw their rows from the one shipped table through one builder and one partial, so the advertised list cannot differ between them.
```

```text
A page that advertises a key the overlay does not, which is the drift the keyboard layer's own comment warns about.
```

crates\/foundry\-app\/src\/keyboard\.rs

```text
the shipped shortcut table
```

shortcut\_entries

the overlay view\-model and the page view\-model

```text
the rows each arm lists, in order
```

```text
A template could hard-code a row; no type refuses it. The single rows partial is what makes such a row visible in review, and the row-identity law is what fails on it.
```

###### C3

```text
The issue page's status marker carries the stored state slug, and its text is that same slug, because the page embeds the one shipped chip renderer.
```

```text
A page showing a status the store does not hold, or two chip shapes drifting apart between the page and the board.
```

crates\/foundry\-app\/src\/issues\.rs

```text
the state column of the resolved issue row
```

render\_state\_chip

issue\.html through the no\-escape filter

```text
the status chip and its data-state on the issue page
```

```text
The no-escape embed trusts the renderer's own escaping, exactly as the shared head embeds the site footer; a caller passing unescaped text into that field would bypass it, which is why the field is filled at one site only.
```

##### States

Admitted

```text
A navigation to the shortcut page with no session at all
```

crates\/foundry\-app\/src\/keyboard\.rs

Admitted

```text
A script fetch of the shortcut URL from any page, signed in or signed out
```

crates\/foundry\-app\/static\/js\/keyboard\.js

Admitted

```text
An issue with an empty description, and an issue whose title or description is arbitrarily long
```

crates\/foundry\-app\/src\/comments\.rs

Admitted

```text
A signed-out page rendered at the emulated phone device and at the harness desk viewport
```

crates\/foundry\-app\/templates\/card\_page\.html

Excluded

```text
A framed document assigned into the overlay host
```

crates\/foundry\-app\/src\/keyboard\.rs

Excluded

```text
Two h1 elements on a card page, and a heading that holds the issue key and the title together
```

crates\/foundry\-app\/templates\/card\_page\.html

Excluded

```text
A navigation rail or a theme control on a signed-out page or on the public shortcut page
```

crates\/foundry\-app\/templates\/card\_page\.html

Unknown

```text
Which clients navigate without declaring a destination: they keep today's unstyled fragment, and no authority bounds that set
```

user

Unknown

```text
Whether a given password manager honours the standard tokens; only the served token is in this design's reach
```

user

##### Models

Quint

docs\/feature\/ui\-bare\-pages\/design\/keyboard\_help\_variant\.qnt

###### Properties

fragmentUnlessDocument

Invariant

```text
Every request that does not declare the document destination receives the bare fragment.
```

show\_keyboard\_help

crates\/foundry\-app\/src\/keyboard\.rs\:317

pageIsFramed

Invariant

```text
A page body is answered only to a declared document destination, and it is always framed.
```

help\_variant

crates\/foundry\-app\/src\/keyboard\.rs\:317

rowsNeverDrift

Invariant

```text
Neither arm drops, adds or reorders a shortcut row.
```

shortcut\_entries

crates\/foundry\-app\/src\/keyboard\.rs\:68

###### Atoms

State

dest

the Sec\-Fetch\-Dest value read from the request headers

crates\/foundry\-app\/src\/keyboard\.rs\:317

None.

State

served

the response body show\_keyboard\_help returns

crates\/foundry\-app\/src\/keyboard\.rs\:317

None.

Action

request

show\_keyboard\_help

crates\/foundry\-app\/src\/keyboard\.rs\:317

```text
one whole request and its response; no interleaving, because the handler reads no shared state
```

Constant

ROWS

SHORTCUTS

crates\/foundry\-app\/src\/keyboard\.rs\:68

None.

###### Assumptions

```text
One client request per step; the shortcut table is the closed seven-row shipped list; the destinations are the closed four-value set a client can declare.
```

```text
The model carries no browser layout, no stylesheet and no session: the card geometry, the palette readings and the autocomplete tokens are outside it.
```

###### Mutants

######## N1

```text
The overlay fetch also receives the page: the arm answers a page for the empty destination as well as for document.
```

###### Kills

fragmentUnlessDocument

######## N2

```text
The page arm is not framed: it answers a page body that links no stylesheet.
```

###### Kills

pageIsFramed

######## N3

```text
The page arm drops the last advertised row, so it lists six of the seven shortcuts.
```

###### Kills

rowsNeverDrift

##### Closure

```text
the shortcut page arm (P1)
```

Safety

Pass

DesignedNotBuilt

```text
The arms are a closed two-variant sum decided in one total function that answers the fragment for every input it does not recognise, so no third body and no silent page can be built. Quint checked fragmentUnlessDocument and pageIsFramed exhaustively over the four destinations a client can declare, and each failed on its own mutant: a page served to the overlay fetch, and a page body that links no stylesheet. The runtime still serves one unstyled fragment to a navigation.
```

###### Evidence

docs\/feature\/ui\-bare\-pages\/design\/keyboard\_help\_variant\.qnt

```text
the shortcut page arm (P1)
```

Inhabitation

Pass

DesignedNotBuilt

```text
A navigation declaring the document destination builds the page from the shipped shortcut table alone, through help_variant, shortcut_entries and the page view-model; no session, no store read and no request body is needed, and the model exhibits the page state.
```

###### Evidence

docs\/feature\/ui\-bare\-pages\/design\/keyboard\_help\_variant\.qnt

```text
the shortcut page arm (P1)
```

PublicReachability

Pass

DesignedNotBuilt

```text
Input: a browser navigation from the rail link at partials/sidebar.html:78 or the dashboard link at dashboard_root.html:32. Constructor: help_variant and the page view-model in keyboard.rs:317. Consumer: the HTML response of show_keyboard_help. Public port: GET /keyboard-help, which is the oracle stimulus.
```

###### Evidence

None.

```text
the shortcut page arm (P1)
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: one request, one response, no retry, no queue, no background work and no progress obligation anywhere in this Request.
```

###### Evidence

None.

```text
the signed-out card (P2)
```

Safety

Pass

DesignedNotBuilt

```text
The card frame is the only frame these templates extend, and it includes no sidebar partial and no theme control, so a rail or a theme control on a signed-out page cannot be rendered by them; the product name is a brand line, not a heading, so a second h1 cannot appear. No checker proves a template shape, so the finite page law and the shipped theme read are the checks, and both are designed and not yet built.
```

###### Evidence

None.

```text
the signed-out card (P2)
```

Inhabitation

Pass

DesignedNotBuilt

```text
Each of the seven pages is built by its own existing handler from the view-model it already fills; the frame adds no field, so every page that renders today renders inside the card.
```

###### Evidence

None.

```text
the signed-out card (P2)
```

PublicReachability

Pass

DesignedNotBuilt

```text
Input: a signed-out GET of /sign-in, /forgot-password, /bootstrap with its token, /invites/accept with its id and signature, or /reset-password with its token. Constructor: the shipped handler and its view-model. Consumer: the card frame template. Public port: those same GETs, read at the desk viewport and at the emulated phone device.
```

###### Evidence

None.

```text
the signed-out card (P2)
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: a page render has no progress obligation, and no form behaviour changes.
```

###### Evidence

None.

```text
the issue page content (P3)
```

Safety

Pass

DesignedNotBuilt

```text
The description is escaped by the template engine and the status is the one shipped chip renderer's own output, so neither stored text nor a second chip shape can reach the page as markup; the heading keeps the key alone, which the shipped browser read already enforces. The generated description property and the finite page law are the checks, both designed and not yet built.
```

###### Evidence

None.

```text
the issue page content (P3)
```

Inhabitation

Pass

DesignedNotBuilt

```text
The title, description and state come from the shipped projection read the edit dialog already uses, over the project and number the page handler has resolved; an empty description still builds the region through its empty copy.
```

###### Evidence

None.

```text
the issue page content (P3)
```

PublicReachability

Pass

DesignedNotBuilt

```text
Input: a signed-in member's GET of /team/{team}/project/{project}/issues/{n}. Constructor: render_issue_page in comments.rs:692 filling IssuePage from issue_edit_view. Consumer: issue.html. Public port: that same GET, which is the oracle stimulus.
```

###### Evidence

None.

```text
the issue page content (P3)
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: one request, one page; the second read either answers or refuses, with no waiting and no retry.
```

###### Evidence

None.

```text
the autocomplete tokens (P4)
```

Safety

Pass

DesignedNotBuilt

```text
A token is a served attribute on a named input; the wrong-state risk is a wrong or missing token, which the finite eight-row law reads on the served markup. No type can exclude it, and no model applies: there is no state, no protocol and no crash window.
```

###### Evidence

None.

```text
the autocomplete tokens (P4)
```

Inhabitation

Pass

DesignedNotBuilt

```text
Each token is a literal attribute in the template that already renders the input, so every intended value is built by the existing render.
```

###### Evidence

None.

```text
the autocomplete tokens (P4)
```

PublicReachability

Pass

DesignedNotBuilt

```text
Input: a signed-out GET of the four pages. Constructor: the four templates. Consumer: the browser and its password manager. Public port: the served HTML of those GETs, read as served markup.
```

###### Evidence

None.

```text
the autocomplete tokens (P4)
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: a served attribute has no progress obligation. Whether a particular manager acts on it is outside this design's reach and is recorded as an unknown state.
```

###### Evidence

None.

##### Risks

###### Y1

Stressor

```text
Negating the assumption that every navigating client declares its destination: an old browser, a text browser, a command-line client or a proxy that strips fetch metadata navigates to the shortcut URL.
```

```text
The shortcut list is readable: the rows arrive as the bare fragment, in order, with the selection instruction.
```

```text
No frame and no stylesheet, so browser-default type, which is exactly the shipped state today.
```

```text
A 404, an empty body, or a framed document handed to the overlay host.
```

```text
The unstyled navigation survives for metadata-less clients, and no authority bounds which clients those are.
```

user

###### Y2

Stressor

```text
Negating the assumption that the overlay keeps declaring a non-document destination: a future client fetches the help through a mechanism that declares document, or a reader opens the rail link in a new tab expecting the overlay.
```

```text
Both arms list identical rows, so the operator still reads the whole list whichever body arrives.
```

```text
The overlay host receiving a framed document and drawing wrongly.
```

```text
A different row set between the two arms.
```

```text
The host assignment in the client script is not type-checked; the shipped templating read and the fragment case of the arm law hold that line.
```

crates\/foundry\-app\/static\/js\/keyboard\.js

###### Y3

Stressor

```text
Negating the assumption that the stylesheet rename reaches all three readers in one commit: the bytes change and one reader keeps the old name.
```

```text
Nothing ships: the shipped asset-reference and asset-hash layers refuse the commit.
```

```text
A red local gate.
```

```text
A deployed page whose stylesheet URL answers 404 while browsers hold the old immutable URL.
```

```text
Only readers inside the scanned application tree are seen by that gate; a reader outside it would be invisible.
```

xtask\/src\/check\_arch\.rs

###### Y4

Stressor

```text
Negating the assumption that a stored description is short and ordinary: an operator stores a hundred-kilobyte description, a single unbroken four-thousand-character word, or text that is markdown and HTML at once.
```

```text
The key, the title and the status stay readable, and the stored text is shown in full, escaped, with its line breaks.
```

```text
A long page and a wrapped word.
```

```text
Horizontal overflow that hides the status or the controls, silent truncation, and any stored character drawn as markup.
```

```text
No authority bounds the stored length, so the page grows with the data; the generated property reads the escaping and the wrapping, not a pixel bound.
```

user

###### Y5

Stressor

```text
Negating the assumption that the signed-out pages are opened by a human: a mail client or a link-preview bot fetches the invite-accept or password-reset URL.
```

```text
The page renders and no single-use token is consumed, because the card frame adds no form, no link and no script and the GET keeps its shipped behaviour.
```

```text
A preview that shows the card and the product name.
```

```text
The frame introducing any effect on a GET.
```

```text
None new from this Request; the token lifecycle is unchanged.
```

crates\/foundry\-app\/templates\/card\_page\.html

###### Y6

Stressor

```text
Negating the assumption that the issue page's second read always answers: the issue is deleted between the resolution and the projection read.
```

```text
The handler refuses with its shipped uniform not-found page, or its shipped internal-error path, never a half-filled page.
```

```text
A not-found page for an issue that existed a moment earlier.
```

```text
A page rendered with an empty title and a blank status as if that were the issue.
```

```text
The handler now holds two reads of the same issue, so the gap exists; it resolves to a refusal, which the failure rows record.
```

crates\/foundry\-app\/src\/comments\.rs

###### Y7

SinglePointOfFailure

```text
One served stylesheet carries every surface, so a syntax error in the new card block degrades every page at once, and the file has no build step and no linter.
```

```text
Every page's markup, every form and every link keep working: a stylesheet cannot break a POST.
```

```text
An unstyled or partly styled page.
```

```text
A page that cannot be used without its stylesheet.
```

```text
The shipped theme and contrast reads plus the token-seam layers are the only checks on the file; there is no CSS parser in the gate.
```

crates\/foundry\-app\/static\/css

###### Y8

Bypass

```text
The no-escape embed of the status chip string in issue.html, the same mechanism the shared head uses for the site footer.
```

```text
Only the chip renderer's own escaped output is passed, from one call site.
```

```text
Nothing: no other value enters that field.
```

```text
Any caller passing unescaped user text through the chip field.
```

```text
Held by the single call site and by review, not by a type; the template engine cannot check a pre-rendered string.
```

crates\/foundry\-app\/src\/comments\.rs

##### Architectural tests

###### T1

```text
Every static reference in the application resolves on disk and every content-hashed filename is the sha256 prefix of its own bytes, so the stylesheet rename cannot land half-done.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

###### T2

```text
No colour literal lives outside the three stylesheet token regions, and the three regions declare the identical colour-token set, so the new card, row, issue and file-input rules name tokens only.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

###### T3

```text
The pre-paint head scripts carry no defer, async or module attribute and every other head script carries defer, which the stylesheet-link edit in the shared head must not disturb.
```

xtask\/src\/check\_arch\.rs

```text
scripts/test-in-lima.sh bash -c "CARGO_BUILD_JOBS=4 cargo xtask check-arch"
```

##### Contract tests

None.

## ADRs

- 002 One shortcut route\, two arms by declared destination — Superseded by BARE-PAGES-001
- 003 One card frame owns every chrome\-less page — Accepted
- BARE\-PAGES\-001 Both shortcut arms vary on the declared destination — Accepted
