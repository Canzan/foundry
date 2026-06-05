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
