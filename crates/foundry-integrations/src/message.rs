//! What a message says (D-3, K-7, K-8) and how Slack is given it (DDR-12).
//!
//! [`compose`] decides the lines from an event and a content level; the
//! encoders only place them. Nothing here reads a description, an attachment
//! or an email address: [`EventView`] has no field for them.

use crate::{ContentLevel, EncodedMessage, EventKind};
use serde_json::json;

/// The facts of one event a message may carry, read at send time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventView {
    pub kind: EventKind,
    pub issue_key: String,
    pub title: String,
    pub project_name: String,
    pub actor_name: String,
    pub lane_label: String,
    /// The issue's address in Foundry (Open in Foundry).
    pub issue_url: String,
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

/// The lines for `view` at `level` (K-8). Link only carries the key and the
/// button; Summary adds the title and the event line.
pub fn compose(view: &EventView, level: ContentLevel) -> MessageLines {
    let link = Some(view.issue_url.clone());
    if level == ContentLevel::LinkOnly {
        return MessageLines {
            headline: view.issue_key.clone(),
            detail: None,
            link,
        };
    }
    MessageLines {
        headline: format!("{} {}", view.issue_key, view.title),
        detail: Some(event_line(view)),
        link,
    }
}

fn event_line(view: &EventView) -> String {
    match view.kind {
        EventKind::IssueCreated => format!(
            "New issue in {} by {} · {}",
            view.project_name, view.actor_name, view.lane_label
        ),
        EventKind::IssueDeleted => {
            format!("Deleted by {} · {}", view.actor_name, view.project_name)
        }
        EventKind::IssueMoved
        | EventKind::CommentAdded
        | EventKind::CommentEdited
        | EventKind::CommentDeleted => {
            format!("{} · {}", view.actor_name, view.project_name)
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
