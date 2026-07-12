//! Askama view-models for the htmx web tier (Feature B / ADR-B01).
//!
//! These typed `#[derive(Template)]` structs are the rendering seam: handlers
//! materialize a fully-resolved view-model from the `foundry-services` data
//! (NEVER the DB directly — NFR-WEBB-BND-01) and hand it to Askama, which
//! compiles `templates/*.html` into the binary at build time. The render
//! contract is **selector-and-substring-identical** to the previous `format!`
//! markup (design/render-contract.md): same elements, CSS classes/ids,
//! `data-column` / `data-issue-key` / `#kb-items` markers, and literal copy;
//! incidental whitespace is free because the acceptance suite parses the DOM
//! via `scraper`.
//!
//! Askama auto-escapes `{{ … }}` for `.html` templates (matching the previous
//! `html_escape` calls), so view-model fields carry raw, un-escaped values.

use askama::Template;

/// The project-create full page (US-R01). Extends `base.html`, which links the
/// vendored `/static` stylesheet + htmx/Alpine scripts (US-B01/B02) — replacing
/// the previous bare-`<head>` `format!` markup (`projects.rs::render_create_form`).
/// The render contract is selector-and-substring-identical to the prior form:
/// `method="post"`, `action="/team/{slug}/projects"`, the hidden `_csrf` field,
/// the `name` + `key_prefix` required text inputs with their repopulated values,
/// and the optional `.error` paragraph. CSRF/sessions are csrf.rs invariants —
/// the template emits ONLY the hidden field (auto-escaped, matching the previous
/// `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "project_create.html")]
pub struct ProjectCreatePage {
    pub team_name: String,
    /// `/team/{slug}/projects` — the form POST action.
    pub action: String,
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
    /// Conflict/validation copy shown in the `.error` slot; `None` on initial GET.
    pub error: Option<String>,
    /// Repopulated project-name input value (empty on initial GET).
    pub raw_name: String,
    /// Repopulated key-prefix input value (empty on initial GET).
    pub raw_key: String,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// A SHARED bare error fragment (US-R01 / US-R03 / US-R05). Emits
/// `<div class="error" data-hx-fragment="{marker}">{message}</div>` — a BARE
/// fragment that MUST NOT extend `base.html` (extending it double-wraps the
/// htmx swap, NFR-WEBB-COMPAT-02 fragment-vs-full-page rule). The
/// `fragment_marker` is the byte-stable `data-hx-fragment` scraper marker
/// (`project-create-error`, `issue-create-error`, `attachment-upload-error`);
/// `message` is auto-escaped (matching the previous `html_escape`). Parameterized
/// so later steps reuse this ONE template instead of three per-surface copies.
#[derive(Debug, Clone, Template)]
#[template(path = "error_fragment.html")]
pub struct ErrorFragment {
    /// The byte-stable `data-hx-fragment` marker for this surface.
    pub fragment_marker: String,
    /// The user-facing error copy (auto-escaped).
    pub message: String,
}

/// The new-issue modal FRAGMENT (US-R02 / US-12). A BARE htmx fragment — it
/// MUST NOT extend `base.html` (htmx swaps it into the live page DOM; extending
/// base double-wraps the swap, NFR-WEBB-COMPAT-02). Renders the ONE shared
/// `partials/new_issue_modal.html` partial directly (the one-partial rule,
/// NFR-WEBB-MAINT-02) — the SAME partial the full-page fallback includes. The
/// render contract is selector-and-substring-identical to the previous
/// `keyboard.rs::render_modal_fragment` `format!`: `data-modal="new-issue"`,
/// `role="dialog"`, `aria-modal`, `method="post"` with the identical `action`,
/// the hidden `_csrf` field, and `input[name=title][autofocus]`. Fields are
/// auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(
    source = r#"{% include "partials/new_issue_modal.html" %}"#,
    ext = "html"
)]
pub struct NewIssueModal {
    /// Project name shown in the modal header (auto-escaped).
    pub project_name: String,
    /// `/team/{slug}/project/{slug}/issues` — the form POST action.
    pub action: String,
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
}

/// The no-JS new-issue FULL-PAGE fallback (US-R02). Extends `base.html`, which
/// links the vendored content-hashed `/static` stylesheet (ADR-B03) + the
/// htmx/Alpine scripts (US-B01/B02) — replacing the previous bare-`<head>`
/// `format!` markup (`keyboard.rs::render_modal_full_page`). It `{% include %}`s
/// the SAME `partials/new_issue_modal.html` partial the htmx fragment renders
/// (the one-partial rule, NFR-WEBB-MAINT-02), so a no-script submit posts to the
/// identical `action`, carries the identical `data-modal`/`role=dialog`/
/// `aria-modal`/`_csrf`/`input[name=title][autofocus]`. The included partial
/// resolves its `project_name`/`action`/`csrf` from these same-named fields.
#[derive(Debug, Clone, Template)]
#[template(path = "new_issue_modal_page.html")]
pub struct NewIssueModalPage {
    /// Project name shown in the modal header + page title (auto-escaped).
    pub project_name: String,
    /// `/team/{slug}/project/{slug}/issues` — the form POST action.
    pub action: String,
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
    /// Team slug shown in the full-page `<h1>` header (auto-escaped).
    pub team_slug: String,
}

/// A single keyboard search result row. The `key` is the canonical
/// `{PREFIX}-{N}` issue key (e.g. `AUTH-4`) — both the `data-issue-key`
/// attribute and the visible `.key` span; `title` is the issue title shown in
/// the `.title` span. Both auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone)]
pub struct SearchResultRow {
    pub key: String,
    pub title: String,
}

/// The keyboard `/`-search results FRAGMENT (US-K01 / US-12). A BARE htmx
/// fragment — it MUST NOT extend `base.html` (the alpine.js handler swaps it
/// into the live page DOM; extending base double-wraps the swap,
/// NFR-WEBB-COMPAT-02). Renders `partials/search_results.html`. The render
/// contract is selector-and-substring-identical to the previous
/// `keyboard.rs::render_search_fragment` `format!`: the `ul.search-results`
/// wrapper with one `li.search-result[data-issue-key="{PREFIX}-{N}"]` per match
/// (each with its `.key` + `.title` spans), AND the empty
/// `ul.search-results[data-empty="true"]` no-match state. Fields are
/// auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/search_results.html")]
pub struct SearchResults {
    pub items: Vec<SearchResultRow>,
}

/// A single keyboard-help shortcut entry. `key` is the shortcut key (the
/// `dt[data-shortcut]` attribute + the visible `<dt>` text); `label` is the
/// human description in the `<dd>`. Both auto-escaped.
#[derive(Debug, Clone)]
pub struct ShortcutEntry {
    pub key: String,
    pub label: String,
}

/// The keyboard-help overlay FRAGMENT (US-K02 / US-12). A BARE htmx fragment —
/// it MUST NOT extend `base.html` (the alpine.js bootstrap GETs it once and
/// caches it into the live DOM; extending base double-wraps the swap,
/// NFR-WEBB-COMPAT-02). Renders `partials/keyboard_help.html`. The render
/// contract is selector-and-substring-identical to the previous
/// `keyboard.rs::show_keyboard_help` `format!`:
/// `section.keyboard-help[role="dialog"][aria-label="Keyboard shortcuts"]` with
/// a `header>h2` "Keyboard shortcuts" heading and one
/// `dt[data-shortcut="{key}"]` then `dd` pair per shortcut. Fields are
/// auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/keyboard_help.html")]
pub struct KeyboardHelp {
    pub entries: Vec<ShortcutEntry>,
}

/// The state-change chip FRAGMENT (US-R03). A BARE htmx fragment — it MUST NOT
/// extend `base.html` (htmx swaps it into the live board DOM; extending base
/// double-wraps the swap, NFR-WEBB-COMPAT-02). Renders the `partials/state_chip.html`
/// partial directly. The render contract is selector-and-substring-identical to
/// the previous `issues.rs::submit_state_change` `format!`:
/// `<span class="state" data-state="{normalized}">{normalized}</span>`, where
/// `normalized` is the underscore-normalized state value (e.g. `in_progress`).
/// The field is auto-escaped (matching the previous output — the normalized
/// value is byte-stable and carries no markup-significant characters).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/state_chip.html")]
pub struct StateChip {
    /// The underscore-normalized state value (`in_progress`, `done`, …) — the
    /// `data-state` marker AND the visible chip text (auto-escaped).
    pub normalized: String,
}

/// A single issue card. One definition, included by every board column that
/// has cards (the one-partial rule, NFR-WEBB-MAINT-02).
#[derive(Debug, Clone)]
pub struct IssueCard {
    /// Canonical `{PREFIX}-{N}` key (e.g. `AUTH-2`).
    pub key: String,
    pub title: String,
    /// `GET …/issues/{n}/edit` — the card's `hx-get` to open the edit dialog
    /// (issue-edit-dialog, R1). The card targets `#modal-root` so a click
    /// swaps the pre-filled dialog in.
    pub edit_url: String,
    /// `POST …/issues/{n}/state` — the endpoint the DnD drop handler
    /// (`board-dnd.js`, issue-status-move slice 02) posts the target column's
    /// slug to. Rendered as `data-state-url` so the client script can read it
    /// off the dragged card without a server round-trip.
    pub state_url: String,
}

/// The issue-edit dialog FRAGMENT (issue-edit-dialog). A BARE htmx fragment — it
/// MUST NOT extend `base.html` (htmx swaps it into `#modal-root`; extending base
/// double-wraps the swap, NFR-WEBB-COMPAT-02). Mirrors `NewIssueModal` (reuses
/// the board-new-issue `.modal`/`.modal-dialog` styling) plus a pre-filled
/// description `<textarea>`. Carries `method="post"`/`action` for the no-JS
/// fallback AND `hx-post` to the save endpoint targeting `#modal-root`. Fields
/// are auto-escaped (safe pre-fill of user-entered title/description).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/issue_edit_modal.html")]
pub struct IssueEditModal {
    /// `/team/{slug}/project/{slug}/issues/{n}/edit` — the save POST action.
    pub action: String,
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
    /// The issue key shown in the dialog header (e.g. `GEN-1`).
    pub key: String,
    /// The current title, pre-filled into the title input (auto-escaped).
    pub title: String,
    /// The current markdown description, pre-filled into the textarea
    /// (auto-escaped).
    pub description: String,
    /// The issue's current state slug (`backlog`, `todo`, `in_progress`, `done`)
    /// — the status `<select>` pre-selects the matching option
    /// (issue-status-move slice 01).
    pub selected_state: String,
    /// `GET …/issues/{n}` — the full issue-detail page. Rendered as an explicit
    /// "open full page" link in the dialog (issue-change-history ADR-002 §1): the
    /// board card itself no longer navigates, so this is the recipient's route
    /// from the quick-edit dialog to the full view.
    pub detail_url: String,
}

/// A board column with its (already state-filtered) cards in display order.
#[derive(Debug, Clone)]
pub struct BoardColumn {
    /// Scraper marker slug: lowercased label with `-` → `_` (`in-progress`
    /// → `in_progress`). Also the `hx-swap-oob` target for the backlog column.
    pub slug: String,
    /// Visible heading text: `Backlog`, `Todo`, `In-Progress`, `Done`.
    pub label: String,
    pub cards: Vec<IssueCard>,
}

/// The full board page. Extends `base.html`, which links the vendored
/// `/static` stylesheet + htmx/Alpine scripts (US-B01/B02).
#[derive(Debug, Clone, Template)]
#[template(path = "board.html")]
pub struct BoardPage {
    pub team_name: String,
    pub project_name: String,
    /// Team slug for board-scoped links (e.g. `hx-get` to the new-issue modal).
    pub team_slug: String,
    /// Project slug for board-scoped links (e.g. `hx-get` to the new-issue modal).
    pub project_slug: String,
    /// The project key prefix shown in the header (e.g. `AUTH`).
    pub key_prefix: String,
    pub columns: Vec<BoardColumn>,
    /// Hidden keyboard-navigation carrier issue keys, **sorted ASCENDING by
    /// issue number** (US-12 / NFR-WEBB-A11Y-01). Data ordering lives in the
    /// view-model; the template only renders the `<li>` list.
    pub kb_items: Vec<String>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// The sign-in page. Extends `base.html`, which links the vendored `/static`
/// stylesheet + htmx/Alpine scripts (US-B01/B02) — replacing the previous bare
/// `<head>` `format!` markup (US-B04). The render contract is
/// selector-and-substring-identical to the prior form: same `method="post"`
/// `action="/sign-in"`, the hidden `_csrf` field (DD12 — the template emits ONLY
/// this; the cookie/header/middleware are csrf.rs invariants), and the
/// non-enumerable `GENERIC_SIGNIN_ERROR` copy in the `.error` slot. The auth
/// logic in `signin.rs` is UNCHANGED (markup only, DB7).
#[derive(Debug, Clone, Template)]
#[template(path = "signin.html")]
pub struct SigninPage {
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field
    /// (auto-escaped — matches the previous `html_escape`).
    pub csrf_token: String,
    /// The non-enumerable error copy (`GENERIC_SIGNIN_ERROR`), shown in the
    /// `.error` slot on a failed sign-in; `None` on the initial GET.
    pub error: Option<String>,
}

/// The forgot-password page. Extends `base.html` (US-B04) — same vendored
/// `/static` stylesheet link as the board, replacing the prior bare `<head>`.
/// Render contract is selector-and-substring-identical to the prior form:
/// `method="post"` `action="/forgot-password"` with the hidden `_csrf` field.
#[derive(Debug, Clone, Template)]
#[template(path = "forgot.html")]
pub struct ForgotPage {
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf_token: String,
}

/// The invite-accept set-password page (invite-accept-flow, ADR-001/003/004).
/// Extends `base.html`. Renders for a LIVE invite only: names the workspace,
/// carries the hidden `_csrf` (double-submit) + `id` + `sig` so the POST
/// re-verifies the same token, and the password + confirm inputs. The optional
/// `.error` slot shows the inline recovery message (weak / mismatch) on re-render.
#[derive(Debug, Clone, Template)]
#[template(path = "invite_accept.html")]
pub struct InviteAcceptPage {
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf_token: String,
    /// The invite row id, echoed into the hidden `id` field for the POST.
    pub invite_id: String,
    /// The HMAC signature, echoed into the hidden `sig` field for the POST.
    pub sig: String,
    /// The workspace name the form is setting a password for.
    pub workspace_name: String,
    /// Inline recovery copy (weak / mismatch); `None` on the initial GET render.
    pub error: Option<String>,
}

/// The member-invite ISSUANCE form (workspace-member-invites US-01). Extends
/// `base.html`. Rendered for a signed-in WORKSPACE ADMIN: names the workspace,
/// carries the hidden double-submit `_csrf`, and a single email field posting to
/// `/workspace/invites`. The optional `.error` slot shows the inline blank-email
/// recovery message on re-render.
#[derive(Debug, Clone, Template)]
#[template(path = "member_invite_form.html")]
pub struct MemberInviteForm {
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf_token: String,
    /// The workspace name the admin is inviting a member to.
    pub workspace_name: String,
    /// Inline recovery copy (blank email); `None` on the initial GET render.
    pub error: Option<String>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// The "invite sent" FRAGMENT returned by `POST /workspace/invites`
/// (workspace-member-invites US-01). Reports the invitee email + the shareable
/// signed `/invites/accept` link (valid 7 days). The signed `invite_url` is already
/// URL-encoded and embedded verbatim via `|safe` so the signature stays byte-stable;
/// the invitee email is auto-escaped.
#[derive(Debug, Clone, Template)]
#[template(path = "member_invite_sent.html")]
pub struct MemberInviteSent {
    /// The email the invite was issued to (auto-escaped).
    pub invitee_email: String,
    /// The signed, already-URL-encoded accept link; embedded verbatim via `|safe`.
    pub invite_url: String,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// The POST /forgot-password success page. Extends `base.html` (Phase-4 FIX 3)
/// so the confirmation links the same vendored `/static` stylesheet as every
/// other surface, replacing the prior inline `format!` bare-`<head>` HTML.
/// The confirmation copy ("If that email is on file, a reset link has been
/// sent.") is preserved byte-identically (NFR-WEBB-COMPAT-02) — it is a static
/// constant in the template, so there is no field. The response stays uniform
/// regardless of whether the email was on file (no enumeration leak).
#[derive(Debug, Clone, Template)]
#[template(path = "forgot_sent.html")]
pub struct ForgotSentPage;

/// A single comment card. One definition, included by every render path
/// (full issue page, single-comment re-render, edit-cancel) — the
/// one-partial rule (NFR-WEBB-MAINT-02 / DD10). The OOB live-append wrapper
/// (step 02-02) will include this SAME partial.
///
/// **Sanitization stays in `foundry_core`**: `body_html` is the ALREADY-
/// sanitized HTML from `foundry_core::render_comment_markdown`; the template
/// embeds it through Askama's `|safe` (no-escape) filter so the rendered
/// tags survive, while every OTHER field (`author`, urls) stays
/// auto-escaped. Affordance flags (`can_edit`/`can_delete`) are computed in
/// the handler (NFR-WEBB-BND-03); the template only renders them.
#[derive(Debug, Clone)]
pub struct CommentCard {
    /// Comment uuid as a string — the `id="comment-{id}"` anchor + the
    /// `data-comment-id` marker + the `#comment-{id}` hx-target.
    pub id: String,
    /// Author's `email_display`, the `data-author` scraper marker + the
    /// visible author in `.comment-author` (auto-escaped — it is user input).
    pub author: String,
    /// Pre-sanitized comment HTML from core; embedded verbatim via `|safe`.
    pub body_html: String,
    /// Whether the `(edited)` marker shows (Q4 = A; surfaces on any edit).
    pub edited: bool,
    /// Author-only edit affordance (ADR-006), computed in the handler.
    pub can_edit: bool,
    /// Author-or-admin delete affordance (ADR-007), computed in the handler.
    pub can_delete: bool,
    /// `GET …/comments/{id}/edit` — the Edit button `hx-get` target.
    pub edit_url: String,
    /// `DELETE …/comments/{id}` — the Delete button `hx-delete` target.
    pub delete_url: String,
}

/// Standalone single-card render (PATCH edit re-render, GET cancel /
/// single-comment) — reuses the SAME `comment_card.html` partial as the
/// issue-page loop (the one-partial rule). The inline source binds the
/// wrapped card to `card` so the included partial resolves identically to
/// the `{% for card in comments %}` path in `issue.html`.
#[derive(Debug, Clone, Template)]
#[template(source = r#"{% include "partials/comment_card.html" %}"#, ext = "html")]
pub struct CommentCardFragment {
    pub card: CommentCard,
}

/// htmx OOB live-append wrapper (step 02-02 / DD10). Wraps the SAME
/// `comment_card.html` partial in the `hx-swap-oob="beforeend:[data-comment-list]"`
/// envelope the POST-comment handler returns. Because it `{% include %}`s the
/// shared partial — instead of re-emitting a divergent, affordance-less card —
/// the live-appended card is SELECTOR-IDENTICAL to the same card after a full
/// page reload, including the Edit/Delete affordances (the omission at
/// comments.rs:841 is gone). The `card` field binds identically to the
/// issue-page loop and the single-card fragment paths.
#[derive(Debug, Clone, Template)]
#[template(path = "partials/oob/comment_card_oob.html")]
pub struct CommentCardOob {
    pub card: CommentCard,
}

/// The comment-create error fragment (400) — literal copy preserved
/// byte-identically (`data-hx-fragment="comment-create-error"`).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/errors/issue_400.html")]
pub struct CommentCreateError {
    /// Validation message — auto-escaped (matches the previous `html_escape`).
    pub message: String,
}

/// The comment-edit-form fragment, rendered into `#comment-{id}` when the
/// author clicks Edit (ADR-009 — CSRF rides in the body on PATCH).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/comment_edit_form.html")]
pub struct CommentEditForm {
    pub id: String,
    /// `PATCH …/comments/{id}` — the form `hx-patch` target.
    pub patch_url: String,
    /// `GET …/comments/{id}` — the Cancel button `hx-get` (single-card re-render).
    pub cancel_url: String,
    /// The current raw markdown, pre-filled into the `<textarea>` (auto-escaped).
    pub body_markdown: String,
}

/// A single attachment row on the issue page.
#[derive(Debug, Clone)]
pub struct AttachmentItem {
    pub filename: String,
    pub href: String,
    /// Human-readable size string (already humanized in the handler).
    pub size: String,
}

/// htmx OOB live-append wrapper for a newly-uploaded attachment (US-R05 / US-11).
/// A BARE htmx fragment — it MUST NOT extend `base.html` (htmx swaps it into the
/// live issue-page DOM; extending base double-wraps the swap, NFR-WEBB-COMPAT-02).
/// Renders the ONE `partials/attachment_row.html` partial wrapped in the
/// `hx-swap-oob="beforeend:[data-attachment-list]"` envelope the POST-upload
/// handler returns (the one-partial rule, NFR-WEBB-MAINT-02 — mirrors Feature B's
/// `CommentCardOob`). The render contract is selector-and-substring-identical to
/// the previous `attachments.rs::render_attachment_row_oob` `format!`:
/// `hx-swap-oob="beforeend:[data-attachment-list]"`, `<li class="attachment"
/// data-filename="…">`, the `.filename` + `.size` spans. Both fields are
/// auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/oob/attachment_row_oob.html")]
pub struct AttachmentRow {
    /// The uploaded filename — the `data-filename` marker + visible `.filename`.
    pub filename: String,
    /// The humanized size string (e.g. `9 MB`), shown in the `.size` span.
    pub size_label: String,
}

/// The over-limit (413) too-large FULL PAGE (US-R05). Extends `base.html`, which
/// links the vendored content-hashed `/static` stylesheet (ADR-B03) — replacing
/// the previous bare-`<head>` `format!` markup (`attachments.rs::payload_too_large`).
/// The render contract is selector-and-substring-identical to the prior page: the
/// literal "Upload too large" `<h1>` copy and the "exceeds the configured limit of
/// {limit_mb} megabytes. Reduce the file size and try again." `<p>` are preserved
/// byte-identically. The handler keeps its `413 PAYLOAD_TOO_LARGE` status
/// (UNCHANGED, DB7) — only the rendered body is templated.
#[derive(Debug, Clone, Template)]
#[template(path = "payload_too_large.html")]
pub struct PayloadTooLarge {
    /// The configured megabyte cap, rendered into the limit copy.
    pub limit_mb: u64,
}

/// The issue-detail page. Extends `base.html`; shows the attachments
/// section, the comment thread (one `comment_card.html` per comment), and
/// the add-comment + upload forms. Render contract is selector-and-
/// substring-identical to the previous `render_issue_page` `format!`.
#[derive(Debug, Clone, Template)]
#[template(path = "issue.html")]
pub struct IssuePage {
    /// Canonical issue key (e.g. `AUTH-3`) — page title + `<h1>`.
    pub issue_key: String,
    pub project_slug: String,
    /// `POST …/issues/{n}/comments` — the add-comment form action.
    pub post_url: String,
    /// CSRF double-submit token minted by `show_issue` — rendered into the
    /// add-comment form's hidden `_csrf` field so the urlencoded POST clears
    /// `csrf_middleware` (comment-add-csrf 01-01).
    pub csrf: String,
    /// `POST …/issues/{n}/attachments` — the upload form action.
    pub upload_url: String,
    pub attachments: Vec<AttachmentItem>,
    pub comments: Vec<CommentCard>,
    /// The change timeline (issue-change-history ADR-002 §1), NEWEST-first.
    /// Empty for an unchanged issue (genesis = start empty, UC-1).
    pub timeline: Vec<TimelineEntry>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// One rendered change-timeline entry (issue-change-history ADR-002 §1). Carries
/// the raw `field` + `new_value` slug as scraper-stable `data-` markers plus the
/// plain-language `summary` a reader sees (e.g. `Mei moved status Todo → In
/// Progress`). Built in the handler from `IssueChangeRow`; the template only
/// loops.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    /// The changed field slug (`status`, …) — the `data-change-field` marker.
    pub field: String,
    /// The raw new-value slug (`in_progress`, `todo`, …) — the `data-new-value`
    /// marker used for order + presence assertions.
    pub new_value: String,
    /// The attributed, plain-language sentence (auto-escaped by Askama).
    pub summary: String,
}

/// The project change-report page (issue-change-history ADR-002 §3, US-04).
/// Extends `base.html`. Renders a table of change events across the project's
/// issues (newest-first) plus two summaries — status-flow transition counts and
/// per-actor change counts — all from the SAME `list_project_changes` events the
/// CSV export serializes (one source of truth). Workspace-scoped by
/// `project_id`, so a foreign issue never appears. The handler builds every
/// view-model (grouping + ordering); the template only loops.
#[derive(Debug, Clone, Template)]
#[template(path = "report.html")]
pub struct ReportPage {
    pub team_name: String,
    pub project_name: String,
    pub key_prefix: String,
    /// `GET …/project/{p}` — back-link to the board.
    pub board_url: String,
    /// `GET …/report?format=csv` — the Export → CSV action.
    pub csv_url: String,
    /// The change events across the project, NEWEST-first.
    pub events: Vec<ReportEvent>,
    /// Status-flow transition counts (`old → new` for `field=status`), ordered.
    pub transitions: Vec<TransitionCount>,
    /// Per-actor change counts, ordered.
    pub actor_counts: Vec<ActorCount>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// One row of the project change-report table. `issue_key` rides through as the
/// scraper-stable `data-issue-key` marker; `old_display`/`new_display` are the
/// human-facing values (auto-escaped).
#[derive(Debug, Clone)]
pub struct ReportEvent {
    pub issue_key: String,
    pub field: String,
    pub old_display: String,
    pub new_display: String,
    pub actor: String,
    pub when: String,
}

/// One status-flow transition tally (`old → new` for `field=status`).
#[derive(Debug, Clone)]
pub struct TransitionCount {
    /// Human label, e.g. `Todo → In Progress` (auto-escaped).
    pub label: String,
    pub count: u32,
}

/// One per-actor change tally.
#[derive(Debug, Clone)]
pub struct ActorCount {
    pub actor: String,
    pub count: u32,
}

/// The signed-in dashboard landing page served at `GET /` (US-R04 / US-01). Extends
/// `base.html`, which links the vendored content-hashed `/static` stylesheet
/// (ADR-B03). The `<h1>Foundry</h1>` heading is preserved (US-R04); the prior
/// "You are signed in. Welcome back." copy is REPLACED by the personalized
/// greeting (US-01 / D1): "Welcome back, {display_name}" + "Workspace:
/// {workspace_name}". Both fields are auto-escaped by Askama (a `display_name`
/// carrying `<`/`&` renders inert, AC-01.3). The signed-out `/` branch keeps its
/// `303 SEE_OTHER → /sign-in` control flow (UNCHANGED).
#[derive(Debug, Clone, Template)]
#[template(path = "dashboard_root.html")]
pub struct DashboardRoot {
    /// The signed-in user's display name (auto-escaped) — the greeting subject.
    /// A neutral fallback when the identity lookup yields nothing (AC-01.4 / D1).
    pub display_name: String,
    /// The acting workspace's name (auto-escaped) — resolved from the SESSION
    /// `workspace_id`. A neutral fallback on lookup failure (AC-01.4 / D1).
    pub workspace_name: String,
    /// Projects in the acting workspace, rendered as board links.
    pub projects: Vec<ProjectLink>,
    /// US-03: whether the SESSION user is an instance super-admin. Gates the
    /// "Instance admin" quick-action link to `/admin/instance/workspaces`. Resolved
    /// from `Store::is_instance_admin(session user_id)`; fail-closed to `false` on
    /// lookup error so the link is ABSENT (not merely CSS-hidden) unless proven.
    pub is_instance_admin: bool,
    /// US-02 (D2): the double-submit CSRF token, rendered into the sign-out
    /// form's hidden `_csrf` field. Minted via `ensure_csrf_cookie` in the
    /// handler, which also emits the matching `foundry_csrf` Set-Cookie — so the
    /// `/` response is `(SET_COOKIE, Html)`, mirroring `admin_tokens::show_index`.
    pub csrf: String,
    /// navigation-bar-linear-ui (US-01): the shared sidebar carrier, assembled
    /// once per page. The dashboard is the `Home` section; `base.html` →
    /// `app_shell.html` injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// One project row on the dashboard project index.
#[derive(Debug, Clone)]
pub struct ProjectLink {
    pub team_slug: String,
    pub project_slug: String,
    pub name: String,
    pub key_prefix: String,
}

/// The events sign-in-required page served when an unauthenticated request hits
/// the SSE stream (US-R04). Extends `base.html` so it links the vendored
/// `/static` stylesheet, replacing the previous bare-`<head>` `format!` markup
/// (`events.rs::unauthorized_response`). The render contract is selector-and-
/// substring-identical to the prior page: the "Sign-in required to subscribe to
/// events." copy and the `<a href="/sign-in">Sign in</a>` link are preserved.
/// The handler keeps its `401 UNAUTHORIZED` status (UNCHANGED, DB7) — only the
/// rendered body is templated.
#[derive(Debug, Clone, Template)]
#[template(path = "events_signin_required.html")]
pub struct EventsSigninRequired;

/// The bootstrap workspace dashboard served at `GET /dashboard` (US-R06). Extends
/// `base.html`, which links the vendored `/static` stylesheet — replacing the prior
/// bare-`<head>` `format!` markup (`bootstrap.rs::dashboard`). The render contract is
/// selector-and-substring-identical to the prior page: the `<h1>Workspace dashboard</h1>`
/// heading, the `Signed in: {signed_in}` line, and the "Invite teammates from the
/// invite-teammates panel." copy are preserved byte-identically.
#[derive(Debug, Clone, Template)]
#[template(path = "bootstrap_dashboard.html")]
pub struct BootstrapDashboard {
    /// Whether a session is present — rendered into the `Signed in: {…}` line.
    pub signed_in: bool,
}

/// The bootstrap claim form served by `GET /bootstrap?token=…` (US-R06). Extends
/// `base.html`, which links the vendored `/static` stylesheet — replacing the prior
/// bare-`<head>` `format!` markup (`bootstrap.rs::render_claim_form`). The render
/// contract is selector-and-substring-identical to the prior form: the
/// `method="post"` `action="/bootstrap?token={token}"`, the email/password/
/// display_name/workspace_name required inputs, and the Claim button. The
/// `/bootstrap` POST is **CSRF-EXEMPT** — the form carries NO `_csrf` field; this is
/// preserved exactly (do NOT add CSRF). The `token` is auto-escaped (matching the
/// previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "bootstrap_claim.html")]
pub struct BootstrapClaim {
    /// The bootstrap token, rendered into the form `action` (auto-escaped).
    pub token: String,
}

/// The invite-link page returned by `POST /invites` (US-R06). Extends `base.html`,
/// which links the vendored `/static` stylesheet — replacing the prior bare-`<head>`
/// `format!` markup (`bootstrap.rs::create_invite`). The render contract is
/// selector-and-substring-identical to the prior page: the "Invite link" `<h1>`, the
/// "Share this URL…" copy, and the `<a href="{invite_url}">{invite_url}</a>`. The
/// signed `invite_url` is already URL-encoded and was embedded RAW (unescaped) by the
/// prior `format!`; it is rendered through `|safe` to keep the signed URL
/// byte-stable.
#[derive(Debug, Clone, Template)]
#[template(path = "bootstrap_invite.html")]
pub struct BootstrapInvite {
    /// The signed, already-URL-encoded invite URL; embedded verbatim via `|safe`.
    pub invite_url: String,
}

/// A SHARED full-page invalid/error page (US-R05 not-found + US-R06 ~17 call
/// sites). Extends `base.html` so it links the vendored `/static` stylesheet,
/// replacing the prior bare-`<head>` `format!` markup (`bootstrap.rs::invalid_page`).
/// The render contract is the byte-stable `<h1>{heading}</h1><p>{message}</p>`
/// shape; both fields are auto-escaped (matching the previous `html_escape`).
/// Created here so US-R05 (05-01) + US-R06 (06-01) reuse this ONE template
/// instead of per-surface copies; the ~17 callers are rewired in 06-01.
#[derive(Debug, Clone, Template)]
#[template(path = "invalid_page.html")]
pub struct InvalidPage {
    /// The `<h1>` heading text (auto-escaped).
    pub heading: String,
    /// The `<p>` body copy (auto-escaped).
    pub message: String,
}

/// A single workspace row on the SIGNED-IN notification status page
/// (recipient-notification-preferences US-05). `muted` selects the row's
/// `data-status="muted"`/`"subscribed"` scraper marker (the prose-immune oracle
/// the acceptance suite asserts against) and the visible `— Muted`/`— Subscribed`
/// suffix; `name` is auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone)]
pub struct NotificationRow {
    /// The workspace name (auto-escaped) — visible in the `<li>`.
    pub name: String,
    /// Whether the caller has muted this workspace's suppressible invitations.
    pub muted: bool,
}

/// The SIGNED-IN per-workspace notification status page (US-05, ADR-006). Extends
/// `base.html`, which links the vendored content-hashed `/static` stylesheet —
/// replacing the prior bare-`<head>` `format!` markup
/// (`unsubscribe.rs::render_notifications_page`). The render contract is preserved
/// byte-stably: one `<li data-status="muted|subscribed">{name} — {Muted|Subscribed}</li>`
/// per membership plus the descriptive lead copy.
#[derive(Debug, Clone, Template)]
#[template(path = "notifications.html")]
pub struct NotificationsPage {
    /// One row per workspace the caller belongs to.
    pub rows: Vec<NotificationRow>,
}

/// The STATE-AWARE unsubscribe confirm page (US-01/US-06, ADR-006). Extends
/// `base.html` — replacing the prior bare-`<head>` `format!` markup
/// (`unsubscribe.rs::render_confirm_page`). `already_unsubscribed` flips the page
/// between the Unsubscribe offer (subscribed) and the Resubscribe offer (muted),
/// keeping the hidden `_csrf`/`t`/`sig`/`action` fields the CSRF POST re-verifies.
/// All fields are auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "unsubscribe_confirm.html")]
pub struct UnsubscribeConfirmPage {
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
    /// The opaque `t` param, echoed into the hidden `t` field for the POST.
    pub t: String,
    /// The HMAC signature, echoed into the hidden `sig` field for the POST.
    pub sig: String,
    /// The workspace name the token authorizes (auto-escaped).
    pub workspace_name: String,
    /// True when the pair is already muted — offers Resubscribe instead of Unsubscribe.
    pub already_unsubscribed: bool,
}

/// The post-action unsubscribe RESULT page (US-01/US-06, ADR-006). Extends
/// `base.html` — replacing the prior bare-`<head>` `format!` markup
/// (`unsubscribe.rs::render_result_page`). `resubscribed` selects the "subscribed
/// again" vs "invitations are stopped" confirmation; `workspace_name` is
/// auto-escaped (matching the previous `html_escape`).
#[derive(Debug, Clone, Template)]
#[template(path = "unsubscribe_result.html")]
pub struct UnsubscribeResultPage {
    /// The workspace name the action applied to (auto-escaped).
    pub workspace_name: String,
    /// True after a resubscribe (clear), false after an unsubscribe (opt-out).
    pub resubscribed: bool,
}

/// A single issued-token row on the `/admin/tokens` list (US-MT01/US-MT06).
/// There is deliberately NO value field (NFR-MT-SEC-02) — no surface ever
/// re-displays a token value. The `data-token-*` markers are the scraper
/// contract the acceptance suite asserts against (`feature_machine_token_admin`).
#[derive(Debug, Clone)]
pub struct TokenRow {
    /// The credential id — `data-token-jti` marker + the visible id.
    pub jti: String,
    /// Human label the admin gave the token (auto-escaped) — `data-token-label`.
    pub label: String,
    /// Scope label: "Whole workspace" or a team name — `data-token-scope`.
    pub scope_label: String,
    /// Human expiry timestamp — `data-token-expiry`.
    pub expires_at: String,
    /// `active` or `revoked` — the lowercase `data-token-status` marker.
    pub status: String,
    /// The minting admin's email (US-MT06); "—" when unattributed.
    pub minted_by: String,
    /// Human last-used timestamp, or "never" — `data-token-last-used`.
    pub last_used: String,
}

/// The `/admin/tokens` index (US-MT01 mint form + US-MT02/US-MT06 list).
/// Extends `base.html` (vendored `/static`). On an issuer-configured server
/// `mint_enabled` is true and the mint form (`form[data-mint-form]`) renders;
/// on a verifier-only server it is false and an "issuing not enabled" notice
/// renders instead (OD1/DD2 graceful degradation, signer.md). The token list
/// shows METADATA ONLY — there is no value field anywhere (NFR-MT-SEC-02), so
/// a minted token's value is never re-retrievable (us-mt-display-once).
#[derive(Debug, Clone, Template)]
#[template(path = "token_mint_form.html")]
pub struct TokenListPage {
    /// True on an issuer binary (signer present) — gates the mint form.
    pub mint_enabled: bool,
    /// The double-submit CSRF token, rendered into the hidden `_csrf` field.
    pub csrf: String,
    /// Validation copy shown in the `.error` slot; `None` on a clean GET.
    pub error: Option<String>,
    /// The workspace's issued tokens, newest first (metadata only).
    pub tokens: Vec<TokenRow>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// The one-time token-display page (US-MT01, DD7 / NFR-MT-SEC-01). This is the
/// ONLY view-model that ever carries a token value: `value_once` is exposed
/// EXACTLY ONCE here (via `expose_secret()` in the handler) and dropped with
/// the response — never stored, never logged, never re-displayed. Renders the
/// `[data-token-value]` + `[data-copy-token]` markers and the unmistakable
/// "only time you'll see this" warning + revoke-and-reissue guidance.
#[derive(Debug, Clone, Template)]
#[template(path = "token_minted.html")]
pub struct TokenMintedPage {
    /// The one-time token value, exposed once into `[data-token-value]`.
    pub value_once: String,
    /// The credential id — `data-token-jti` marker.
    pub jti: String,
    /// The label the admin gave — `data-token-label`.
    pub label: String,
    /// Scope label ("Whole workspace" / team name) — `data-token-scope`.
    pub scope_label: String,
    /// Human expiry timestamp — `data-token-expiry`.
    pub expires_at: String,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// A single existing-workspace row on the instance dashboard
/// (`GET /admin/instance/workspaces`, web-provisioning-flow 01-02). Carries the
/// workspace id + name the thin `list_workspaces` read returned; both rendered
/// into the `data-workspace-row` list (the `data-workspace-id` marker + the
/// visible name). The name is auto-escaped (it is operator-entered).
#[derive(Debug, Clone)]
pub struct InstanceWorkspaceRow {
    /// The workspace id — `data-workspace-id` marker + visible id.
    pub workspace_id: String,
    /// The workspace name (auto-escaped) — visible in the row.
    pub name: String,
}

/// The instance super-admin DASHBOARD full page served by
/// `GET /admin/instance/workspaces` (web-provisioning-flow 01-02, ADR-001 / D1).
/// Extends `base.html` (the no-JS entry point of the surface). Renders the
/// existing-workspace list (the thin `list_workspaces` read, D4) plus BOTH
/// state-changing forms — a provision-workspace form (`POST
/// /admin/instance/workspaces`) and a grant-super-admin form (`POST
/// /admin/instance/super-admins`) — each carrying the hidden double-submit
/// `_csrf` field the shipped `csrf_middleware` enforces on the POST. The grant
/// POST route lands in a later step (01-03/04); this GET only RENDERS its form
/// action. The `data-*` markers are the scraper contract the acceptance suite
/// asserts against (`feature_web_provisioning_flow`).
#[derive(Debug, Clone, Template)]
#[template(path = "instance_dashboard.html")]
pub struct InstanceDashboardPage {
    /// The double-submit CSRF token, rendered into BOTH forms' hidden `_csrf`.
    pub csrf: String,
    /// Every existing workspace (newest first) — listed for the super-admin.
    pub workspaces: Vec<InstanceWorkspaceRow>,
    /// navigation-bar-linear-ui (step 02-01): the shared sidebar carrier,
    /// assembled once per page via `NavContext::home_for`. `app_shell.html`
    /// injects `partials/sidebar.html`, which reads `nav.*`.
    pub nav: crate::nav::NavContext,
}

/// The htmx success FRAGMENT returned by `POST /admin/instance/workspaces`
/// (web-provisioning-flow, US-MWT07 web leg). A BARE htmx fragment — it MUST NOT
/// extend `base.html` (htmx swaps it into the live instance dashboard; extending
/// base double-wraps the swap). Reports the newly-provisioned workspace's id +
/// name + first-admin email and the (informational, D5) first-admin invite link.
/// The `data-*` markers are the scraper contract the acceptance suite asserts
/// against (`feature_web_provisioning_flow`). Per D5 the invite link is rendered
/// for the operator to relay; there is NO accept route in v1 (the URL is a dead
/// link today). The signed `invite_link` is already URL-encoded and embedded
/// verbatim via `|safe` so the signature stays byte-stable; every other field is
/// auto-escaped.
#[derive(Debug, Clone, Template)]
#[template(path = "instance_provisioned.html")]
pub struct InstanceProvisionedFragment {
    /// The new workspace's id — `data-provisioned-workspace-id` marker + visible.
    pub workspace_id: String,
    /// The new workspace's name (auto-escaped) — `data-provisioned-workspace-name`.
    pub workspace_name: String,
    /// The first admin's email (auto-escaped) — `data-first-admin-email`.
    pub first_admin_email: String,
    /// The signed, already-URL-encoded first-admin invite link; embedded verbatim
    /// via `|safe` — `data-first-admin-invite-link` marker.
    pub invite_link: String,
}

/// The htmx confirmation FRAGMENT returned by `POST /admin/instance/super-admins`
/// (web-provisioning-flow 01-03, ADR-001 / D1). A BARE htmx fragment (it MUST NOT
/// extend `base.html` — htmx swaps it into the live dashboard). Confirms the grant
/// NON-COMMITTALLY: the same confirmation is rendered whether or not the email
/// belonged to a real user (D2 (g) — the grant form is not a user-enumeration
/// oracle). The `data-grant-confirmation` marker is the scraper contract the
/// acceptance suite asserts against; the operator-entered email is auto-escaped.
#[derive(Debug, Clone, Template)]
#[template(path = "instance_grant_confirmed.html")]
pub struct InstanceGrantConfirmedFragment {
    /// The operator email the grant was submitted for (auto-escaped) —
    /// `data-granted-email` marker + visible in the confirmation copy.
    pub email: String,
}
