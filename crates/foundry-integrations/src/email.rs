//! Project emails (US-EIP-10, ADR-EIP-007): which members a board event
//! emails, whether a queued email may still go, and what it says.
//!
//! - [`plan_emails`]: one email per (event, member with a matching choice in
//!   the event's project), never to the member who acted (OD-8, OD-11);
//! - [`email_gate`]: the send-time checks, in ADR-EIP-007 decision 3's
//!   order: the choice still exists, the member can still open the project,
//!   the workspace is not muted for them, the scope still matches;
//! - [`compose_email`] decides an email's subject and lines, with the K-12
//!   footer naming the choice that caused it; [`encode_email`] places them as
//!   plain text, its header fields on one line each.

use crate::dispatch::{payload_uuid, RoutableEvent};
use crate::message::actor;
use crate::{
    compose, ChannelKind, ContentLevel, EventKind, EventView, OutboundEmail, OutboxRow, ReasonCode,
    OPEN_IN_FOUNDRY,
};
use time::OffsetDateTime;
use uuid::Uuid;

/// Which issues of a project an email choice covers (US-EIP-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmailScope {
    All,
    /// "Issues I created or commented on": the member filed the issue or has
    /// a comment on it that is not deleted.
    Mine,
}

impl EmailScope {
    pub const ALL: [EmailScope; 2] = [EmailScope::All, EmailScope::Mine];

    pub fn from_wire_name(wire: &str) -> Option<EmailScope> {
        match wire {
            "all" => Some(EmailScope::All),
            "mine" => Some(EmailScope::Mine),
            _ => None,
        }
    }

    /// The stored name: `all` or `mine`.
    pub fn wire_name(self) -> &'static str {
        match self {
            EmailScope::All => "all",
            EmailScope::Mine => "mine",
        }
    }

    /// The scope as the member reads it (K-12).
    pub fn words(self) -> &'static str {
        match self {
            EmailScope::All => "all issues",
            EmailScope::Mine => "issues you created or commented on",
        }
    }
}

/// The events a member can choose to be emailed about (D-2).
pub fn email_events() -> impl Iterator<Item = EventKind> {
    EventKind::ALL
        .into_iter()
        .filter(|kind| kind.announced_to(ChannelKind::Email))
}

/// One member's choice of one event in one live project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmailChoice {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub event: EventKind,
    pub scope: EmailScope,
}

/// One email the plan asks the store to queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedEmail {
    pub recipient_user_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub event_kind: EventKind,
    pub event_id: Uuid,
    pub outbox_id: i64,
    pub issue_id: Option<Uuid>,
    pub issue_key: Option<String>,
    pub occurred_at: OffsetDateTime,
}

/// One email per (row, choice) the choice matches: an event of the choice's
/// project and kind, by someone other than the member (OD-11). The scope is
/// judged at send time ([`email_gate`]), when the member's involvement is
/// read as it then is.
pub fn plan_emails(rows: &[OutboxRow], choices: &[EmailChoice]) -> Vec<PlannedEmail> {
    rows.iter()
        .filter_map(RoutableEvent::read)
        .filter(|event| event.kind.announced_to(ChannelKind::Email))
        .flat_map(|event| {
            let actor = payload_uuid(&event.row.payload, "author_id");
            choices
                .iter()
                .filter(move |c| c.project_id == event.project_id && c.event == event.kind)
                .filter(move |c| actor != Some(c.user_id))
                .map(move |c| PlannedEmail {
                    recipient_user_id: c.user_id,
                    workspace_id: c.workspace_id,
                    project_id: c.project_id,
                    event_kind: event.kind,
                    event_id: event.row.event_id,
                    outbox_id: event.row.id,
                    issue_id: event.issue_id(),
                    issue_key: event.issue_key(),
                    occurred_at: event.occurred_at,
                })
        })
        .collect()
}

/// What the send-time checks read about one queued email, now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmailFacts {
    /// The member's choice for the email's project and event; `None` once
    /// it is gone.
    pub scope: Option<EmailScope>,
    /// The member can open the project (a member of its team).
    pub can_open: bool,
    /// The member muted the project's workspace (the master switch, D-9).
    pub muted: bool,
    /// The member filed the issue or has a comment on it that is not deleted.
    pub involved: bool,
}

/// ADR-EIP-007 decision 3: the scope the email goes out under, or why it
/// does not go.
pub fn email_gate(facts: &EmailFacts) -> Result<EmailScope, ReasonCode> {
    let scope = facts.scope.ok_or(ReasonCode::ChoiceGone)?;
    if !facts.can_open {
        return Err(ReasonCode::NoAccess);
    }
    if facts.muted {
        return Err(ReasonCode::Muted);
    }
    if scope == EmailScope::Mine && !facts.involved {
        return Err(ReasonCode::NotInvolved);
    }
    Ok(scope)
}

/// An email's subject and its lines, before encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailLines {
    pub subject: String,
    pub body: Vec<String>,
}

/// K-12: why the member gets this email.
pub fn email_footer(event: EventKind, scope: EmailScope, project_name: &str) -> String {
    format!(
        "You get this because you chose {} for {} in {project_name}. Stop these emails · Change what you get",
        event.display_name(),
        scope.words()
    )
}

/// The email for `view` under the member's `scope` (D-3: Summary with comment
/// text), ending with the K-12 footer and where to change what they get.
pub fn compose_email(view: &EventView, scope: EmailScope, settings_url: &str) -> EmailLines {
    let lines = compose(view, ContentLevel::SummaryWithComment);
    let mut body = vec![lines.headline];
    body.extend(lines.detail);
    if let Some(excerpt) = lines.excerpt {
        body.push(String::new());
        body.push(excerpt);
    }
    if let Some(link) = lines.link {
        body.push(String::new());
        body.push(format!("{OPEN_IN_FOUNDRY}: {link}"));
    }
    body.push(String::new());
    body.push(email_footer(view.kind, scope, &view.project_name));
    body.push(format!("Change what you get: {settings_url}"));
    EmailLines {
        subject: subject(view),
        body,
    }
}

/// The subject: the comment's is US-EIP-10's; the others are DESIGN UI-4's
/// proposal.
fn subject(view: &EventView) -> String {
    let (key, title) = (&view.issue_key, &view.title);
    match view.kind {
        EventKind::CommentAdded => {
            format!("[{key}] {} commented: {title}", actor(view))
        }
        EventKind::IssueCreated => format!("[{key}] New issue: {title}"),
        EventKind::IssueMoved => format!("[{key}] Moved to {}: {title}", view.lane_label),
        EventKind::IssueDeleted => format!("[{key}] Deleted: {title}"),
        EventKind::CommentEdited | EventKind::CommentDeleted => format!("[{key}] {title}"),
    }
}

/// The email as a transport sends it: plain text, the body's lines as
/// written (no markup), and the recipient and subject each one line, so no
/// text a person wrote can start a header.
pub fn encode_email(recipient: &str, lines: &EmailLines) -> OutboundEmail {
    OutboundEmail {
        recipient: one_line(recipient),
        subject: one_line(&lines.subject),
        text_body: lines.body.join("\n"),
        unsubscribe: None,
    }
}

/// Every control character (a line break among them) read as a space.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
