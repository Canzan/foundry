//! What a message says (D-3, K-7, K-8) and how Slack is given it (DDR-12).
//!
//! [`compose`] decides the lines from an event and a content level; the
//! encoders only place them. Nothing here reads a description, an attachment
//! or an email address: [`EventView`] has no field for them.

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
}

/// A message's lines: a headline, an optional detail line and the button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageLines {
    pub headline: String,
    pub detail: Option<String>,
    pub link: Option<String>,
}

/// The button's label (K-8).
pub const OPEN_IN_FOUNDRY: &str = "Open in Foundry";

/// Who an unknown actor reads as (ADR-EIP-003 decision 4).
const UNKNOWN_ACTOR: &str = "A Foundry member";

/// The lines for `view` at `level` (K-8). Link only carries the key and the
/// button; Summary adds the title and the event line. A burst is its one
/// summary line (D-11), and neither a burst nor a deleted issue has a link.
pub fn compose(view: &EventView, level: ContentLevel) -> MessageLines {
    if view.issue_count > 1 {
        return MessageLines {
            headline: burst_line(view),
            detail: None,
            link: None,
        };
    }
    let link = (!view.kind.outlives_its_issue()).then(|| view.issue_url.clone());
    if level == ContentLevel::LinkOnly {
        return MessageLines {
            headline: view.issue_key.clone(),
            detail: None,
            link,
        };
    }
    MessageLines {
        headline: [view.issue_key.as_str(), view.title.as_str()]
            .join(" ")
            .trim_end()
            .to_string(),
        detail: Some(event_line(view)),
        link,
    }
}

fn actor(view: &EventView) -> &str {
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
    if let Some(url) = &lines.link {
        blocks.push(json!({
            "type": "actions",
            "elements": [{ "type": "button", "text": plain(OPEN_IN_FOUNDRY), "url": url }]
        }));
    }
    let fallback = match &lines.detail {
        Some(detail) => format!("{}\n{}", lines.headline, detail),
        None => lines.headline.clone(),
    };
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
