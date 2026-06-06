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
    /// The project key prefix shown in the header (e.g. `AUTH`).
    pub key_prefix: String,
    pub columns: Vec<BoardColumn>,
    /// Hidden keyboard-navigation carrier issue keys, **sorted ASCENDING by
    /// issue number** (US-12 / NFR-WEBB-A11Y-01). Data ordering lives in the
    /// view-model; the template only renders the `<li>` list.
    pub kb_items: Vec<String>,
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
    /// `POST …/issues/{n}/attachments` — the upload form action.
    pub upload_url: String,
    pub attachments: Vec<AttachmentItem>,
    pub comments: Vec<CommentCard>,
}

/// The signed-in dashboard landing page served at `GET /` (US-R04). Extends
/// `base.html`, which links the vendored content-hashed `/static` stylesheet
/// (ADR-B03) — replacing the previous bare-`<head>` `format!` markup
/// (`signin.rs::dashboard_root`). The render contract is selector-and-
/// substring-identical to the prior page: the `<h1>Foundry</h1>` heading and the
/// "You are signed in. Welcome back." copy are preserved byte-identically. The
/// signed-out `/` branch keeps its `303 SEE_OTHER → /sign-in` control flow
/// (UNCHANGED, DB7) — only the signed-in body is templated.
#[derive(Debug, Clone, Template)]
#[template(path = "dashboard_root.html")]
pub struct DashboardRoot;

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
