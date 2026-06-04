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
