//! What a message says (D-3, K-7, K-8) and how Slack (DDR-12) and Matrix
//! (DDR-13) are given it.
//!
//! [`compose`] decides the lines from an event and a content level; the
//! encoders only place them. Nothing here reads a description, an attachment
//! or an email address: [`EventView`] has no field for them. The route form's
//! preview and the worker both call [`compose`], so a preview is the message.

use crate::{ContentLevel, EncodedMessage, EventKind};
use serde_json::json;

/// The facts of one event a message may carry: what the event recorded as it
/// happened (its lanes, its actor, a deleted issue's title), and the names
/// read at send time (D-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventView {
    pub kind: EventKind,
    pub issue_key: String,
    /// Empty when a deleted issue's title was never recorded.
    pub title: String,
    pub project_name: String,
    /// Empty when the actor is unknown.
    pub actor_name: String,
    /// The lane the event put the issue in (filed in, moved to).
    pub lane_label: String,
    /// The lane a move took the issue from; empty for any other event.
    pub from_lane_label: String,
    /// The issue's address in Foundry (Open in Foundry).
    pub issue_url: String,
    /// How many issues one action touched: more than one is a burst (D-11).
    pub issue_count: usize,
    /// A new comment's text as its author wrote it; empty for any other event.
    pub comment_text: String,
}

/// A message's lines: a headline, an optional detail line, the comment's
/// excerpt (Summary with comment text only) and the button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageLines {
    pub headline: String,
    pub detail: Option<String>,
    pub excerpt: Option<String>,
    pub link: Option<String>,
}

impl MessageLines {
    /// The headline, the detail and the excerpt, in order: every line of
    /// text (the button's address aside).
    fn text(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.headline)
            .chain(&self.detail)
            .chain(&self.excerpt)
    }
}

/// The button's label (K-8).
pub const OPEN_IN_FOUNDRY: &str = "Open in Foundry";

/// The most characters of a comment a message carries (D-3).
pub const EXCERPT_CHARACTERS: usize = 300;

/// An issue's address in Foundry, by numbers and slugs only: never an id.
pub fn issue_url(public_url: &str, team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("{public_url}/team/{team_slug}/project/{project_slug}/issues/{number}")
}

/// Who an unknown actor reads as (ADR-EIP-003 decision 4).
const UNKNOWN_ACTOR: &str = "A Foundry member";

/// The lines for `view` at `level` (D-3, K-8). Link only carries the key,
/// the project and the event's D-2 name with the button; Summary adds the title and
/// the event line; Summary with comment text adds a new comment's first 300
/// characters. A burst is its one summary line (D-11), and neither a burst nor
/// a deleted issue has a link.
pub fn compose(view: &EventView, level: ContentLevel) -> MessageLines {
    if view.issue_count > 1 {
        return MessageLines {
            headline: burst_line(view),
            detail: None,
            excerpt: None,
            link: None,
        };
    }
    let link = (!view.kind.outlives_its_issue()).then(|| view.issue_url.clone());
    if level == ContentLevel::LinkOnly {
        return MessageLines {
            headline: format!(
                "{} · {} · {}",
                view.issue_key,
                view.project_name,
                view.kind.display_name()
            ),
            detail: None,
            excerpt: None,
            link,
        };
    }
    MessageLines {
        headline: [view.issue_key.as_str(), view.title.as_str()]
            .join(" ")
            .trim_end()
            .to_string(),
        detail: Some(event_line(view)),
        excerpt: excerpt(view, level),
        link,
    }
}

/// A new comment's text, cut at [`EXCERPT_CHARACTERS`] with "…", when the
/// route chose Summary with comment text.
fn excerpt(view: &EventView, level: ContentLevel) -> Option<String> {
    let chosen = level == ContentLevel::SummaryWithComment && view.kind == EventKind::CommentAdded;
    if !chosen || view.comment_text.is_empty() {
        return None;
    }
    let mut cut: String = view.comment_text.chars().take(EXCERPT_CHARACTERS).collect();
    if cut.len() < view.comment_text.len() {
        cut.push('…');
    }
    Some(cut)
}

/// Who acted, or how an unknown actor reads.
pub(crate) fn actor(view: &EventView) -> &str {
    match view.actor_name.as_str() {
        "" => UNKNOWN_ACTOR,
        name => name,
    }
}

fn event_line(view: &EventView) -> String {
    let (actor, project) = (actor(view), &view.project_name);
    match view.kind {
        EventKind::IssueCreated => {
            format!("New issue in {project} by {actor} · {}", view.lane_label)
        }
        EventKind::IssueMoved => format!(
            "Moved {} → {} by {actor} · {project}",
            view.from_lane_label, view.lane_label
        ),
        EventKind::IssueDeleted => format!("Deleted by {actor} · {project}"),
        EventKind::CommentAdded => format!("{actor} commented · {project}"),
        EventKind::CommentEdited | EventKind::CommentDeleted => format!("{actor} · {project}"),
    }
}

/// D-11's one line for a burst; R-10 for moves.
fn burst_line(view: &EventView) -> String {
    let (count, actor, project) = (view.issue_count, actor(view), &view.project_name);
    match view.kind {
        EventKind::IssueDeleted => format!("{count} issues deleted from {project} by {actor}"),
        EventKind::IssueMoved => format!(
            "{count} issues moved to {} in {project} by {actor}",
            view.lane_label
        ),
        EventKind::IssueCreated => format!("{count} new issues in {project} by {actor}"),
        EventKind::CommentAdded | EventKind::CommentEdited | EventKind::CommentDeleted => {
            format!("{count} comments in {project} by {actor}")
        }
    }
}

/// K-7: the one line Send test posts.
pub fn test_message(project_name: &str, key_prefix: &str) -> MessageLines {
    MessageLines {
        headline: format!(
            "Test message from Foundry for {project_name} ({key_prefix}). This route will post here."
        ),
        detail: None,
        excerpt: None,
        link: None,
    }
}

/// Slack's `chat.postMessage` body without the channel: every line a
/// `plain_text` object, escaped, with `parse: none` and `link_names: false`,
/// so no text can mention anyone; the button carries the issue's address.
pub fn encode_slack(lines: &MessageLines) -> EncodedMessage {
    let plain =
        |text: &str| json!({ "type": "plain_text", "text": escape_slack(text), "emoji": false });
    let mut blocks = vec![json!({ "type": "section", "text": plain(&lines.headline) })];
    if let Some(detail) = &lines.detail {
        blocks.push(json!({ "type": "context", "elements": [plain(detail)] }));
    }
    if let Some(excerpt) = &lines.excerpt {
        blocks.push(json!({ "type": "section", "text": plain(excerpt) }));
    }
    if let Some(url) = &lines.link {
        blocks.push(json!({
            "type": "actions",
            "elements": [{ "type": "button", "text": plain(OPEN_IN_FOUNDRY), "url": url }]
        }));
    }
    let fallback = lines
        .text()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    EncodedMessage(json!({
        "text": escape_slack(&fallback),
        "blocks": blocks,
        "parse": "none",
        "link_names": false,
        "mrkdwn": false,
        "unfurl_links": false,
        "unfurl_media": false
    }))
}

/// Slack's three control characters, as its formatting guide requires.
pub fn escape_slack(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// U+2060 WORD JOINER: set around every `@` of people's text, so no legacy
/// push rule can read `@room` or a user id in it (ADR-EIP-006 decision 4).
const WORD_JOINER: char = '\u{2060}';

/// A Matrix `m.notice`: the lines as plain `body` and as HTML
/// `formatted_body` (the headline bold, an Open in Foundry link), every line
/// HTML-escaped with its `@`s joined apart, and `m.mentions: {}`, so no text
/// can mention the room or anyone in it.
pub fn encode_matrix(lines: &MessageLines) -> EncodedMessage {
    let text: Vec<String> = lines.text().map(|line| defuse_mentions(line)).collect();
    let mut html: Vec<String> = text.iter().map(|line| escape_html(line)).collect();
    html[0] = format!("<strong>{}</strong>", html[0]);
    let mut body = text.join("\n");
    if let Some(url) = &lines.link {
        body.push_str(&format!("\n{OPEN_IN_FOUNDRY}: {url}"));
        html.push(format!(
            "<a href=\"{}\">{OPEN_IN_FOUNDRY}</a>",
            escape_html(url)
        ));
    }
    EncodedMessage(json!({
        "msgtype": "m.notice",
        "body": body,
        "format": "org.matrix.custom.html",
        "formatted_body": html.join("<br>"),
        "m.mentions": {}
    }))
}

fn defuse_mentions(text: &str) -> String {
    text.replace('@', &format!("{WORD_JOINER}@{WORD_JOINER}"))
}

/// HTML's five special characters.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
