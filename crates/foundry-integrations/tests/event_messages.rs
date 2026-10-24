//! external-integration-plugins slice 02 (US-EIP-03) — which events a chat
//! route is given, which survive their issue, and the message each one reads
//! as: `EventKind::{announced_to, outlives_its_issue,
//! sendable_without_its_issue}` and `compose`, pinned word for word.
//!
//! Lines marked RATIFIED are quoted from the feature delta: D-13 ("New issue
//! in Identity Platform by Sam Okafor · Backlog", "Moved In progress → Done
//! by Sam Okafor · Identity Platform", "Lena Park commented · Identity
//! Platform", "Deleted by Lena Park · Identity Platform", "6 issues deleted
//! from Identity Platform by Lena Park") and R-10 ("7 issues moved to Todo in
//! Identity Platform by Lena Park"). The unknown actor reads "A Foundry
//! member" (ADR-EIP-003 decision 4). Lines with no ratified string (a burst
//! of new issues or comments) are pinned as the core produces them today.
//!
//! Pure: no I/O, no clock.

use foundry_integrations::{
    compose, ChannelKind, ContentLevel, EventKind, EventView, MessageLines,
};

const URL: &str = "https://foundry.example/team/backend/project/identity-platform/issues/7";

const ALL_KINDS: [EventKind; 6] = [
    EventKind::IssueCreated,
    EventKind::IssueMoved,
    EventKind::IssueDeleted,
    EventKind::CommentAdded,
    EventKind::CommentEdited,
    EventKind::CommentDeleted,
];

/// One event on AUTH-7 in Identity Platform by Sam Okafor, moved from In
/// progress to Done, touching one issue.
fn view(kind: EventKind) -> EventView {
    EventView {
        kind,
        issue_key: "AUTH-7".into(),
        title: "Move Grafana sign-in to Keycloak".into(),
        project_name: "Identity Platform".into(),
        actor_name: "Sam Okafor".into(),
        lane_label: "Done".into(),
        from_lane_label: "In progress".into(),
        issue_url: URL.into(),
        issue_count: 1,
        comment_text: String::new(),
    }
}

fn by(mut v: EventView, actor: &str) -> EventView {
    v.actor_name = actor.into();
    v
}

fn burst(kind: EventKind, count: usize, lane: &str) -> EventView {
    EventView {
        issue_count: count,
        lane_label: lane.into(),
        ..by(view(kind), "Lena Park")
    }
}

fn summary(v: &EventView) -> MessageLines {
    compose(v, ContentLevel::Summary)
}

// ------------------------------------------------- which events chat is given

#[test]
fn chat_and_email_are_given_every_event_but_a_comment_edit_or_delete() {
    for channel in [ChannelKind::Slack, ChannelKind::Matrix, ChannelKind::Email] {
        for kind in ALL_KINDS {
            let expected = !matches!(kind, EventKind::CommentEdited | EventKind::CommentDeleted);
            assert_eq!(
                kind.announced_to(channel),
                expected,
                "{kind:?} announced to {channel:?}"
            );
        }
    }
}

#[test]
fn a_webhook_is_given_all_six_events() {
    for kind in ALL_KINDS {
        assert!(
            kind.announced_to(ChannelKind::Webhook),
            "{kind:?} to a webhook"
        );
    }
}

// ------------------------------------------ what can be told once the issue is gone

#[test]
fn only_a_delete_outlives_its_issue() {
    for kind in ALL_KINDS {
        assert_eq!(
            kind.outlives_its_issue(),
            kind == EventKind::IssueDeleted,
            "{kind:?}"
        );
    }
}

#[test]
fn a_single_delete_is_sendable_without_its_issue() {
    assert!(EventKind::IssueDeleted.sendable_without_its_issue(1));
}

#[test]
fn a_single_event_of_any_other_kind_is_not_sendable_without_its_issue() {
    for kind in ALL_KINDS
        .into_iter()
        .filter(|k| *k != EventKind::IssueDeleted)
    {
        assert!(
            !kind.sendable_without_its_issue(1),
            "{kind:?} for one issue"
        );
    }
}

#[test]
fn a_burst_of_any_kind_is_sendable_without_its_issue() {
    for kind in ALL_KINDS {
        for count in [2, 6, 7] {
            assert!(
                kind.sendable_without_its_issue(count),
                "{kind:?} standing for {count} issues"
            );
        }
    }
}

// ------------------------------------------------------------ one event's message

#[test]
fn a_new_issue_reads_the_lane_it_was_filed_in() {
    // RATIFIED (D-13).
    let v = EventView {
        issue_key: "AUTH-12".into(),
        title: "Rotate LLDAP bind password".into(),
        lane_label: "Backlog".into(),
        from_lane_label: String::new(),
        ..view(EventKind::IssueCreated)
    };
    assert_eq!(
        summary(&v),
        MessageLines {
            headline: "AUTH-12 Rotate LLDAP bind password".into(),
            detail: Some("New issue in Identity Platform by Sam Okafor · Backlog".into()),
            excerpt: None,
            link: Some(URL.into()),
        }
    );
}

#[test]
fn a_move_reads_both_lanes_and_links_to_the_issue() {
    // RATIFIED (D-13).
    assert_eq!(
        summary(&view(EventKind::IssueMoved)),
        MessageLines {
            headline: "AUTH-7 Move Grafana sign-in to Keycloak".into(),
            detail: Some("Moved In progress → Done by Sam Okafor · Identity Platform".into()),
            excerpt: None,
            link: Some(URL.into()),
        }
    );
}

#[test]
fn a_comment_reads_who_commented_and_links_to_the_issue() {
    // RATIFIED (D-13).
    assert_eq!(
        summary(&by(view(EventKind::CommentAdded), "Lena Park")),
        MessageLines {
            headline: "AUTH-7 Move Grafana sign-in to Keycloak".into(),
            detail: Some("Lena Park commented · Identity Platform".into()),
            excerpt: None,
            link: Some(URL.into()),
        }
    );
}

#[test]
fn a_delete_reads_who_deleted_it_and_has_no_link() {
    // RATIFIED (D-13): a deleted issue has nowhere to open.
    let v = EventView {
        issue_key: "AUTH-9".into(),
        title: "Retire the old bind account".into(),
        ..by(view(EventKind::IssueDeleted), "Lena Park")
    };
    assert_eq!(
        summary(&v),
        MessageLines {
            headline: "AUTH-9 Retire the old bind account".into(),
            detail: Some("Deleted by Lena Park · Identity Platform".into()),
            excerpt: None,
            link: None,
        }
    );
}

#[test]
fn a_delete_whose_title_was_never_recorded_reads_its_key_alone() {
    let v = EventView {
        issue_key: "AUTH-9".into(),
        title: String::new(),
        ..by(view(EventKind::IssueDeleted), "Lena Park")
    };
    assert_eq!(summary(&v).headline, "AUTH-9");
}

#[test]
fn at_link_only_a_delete_still_has_no_link_and_anything_else_does() {
    for kind in ALL_KINDS {
        let lines = compose(&view(kind), ContentLevel::LinkOnly);
        // D-3 (02-03): Link only names the key, the project and the event.
        let event = match kind {
            EventKind::CommentAdded => "Issue commented on",
            other => other.display_name(),
        };
        assert_eq!(
            lines.headline,
            format!("AUTH-7 · Identity Platform · {event}"),
            "{kind:?}"
        );
        assert_eq!(lines.detail, None, "{kind:?}");
        let expected = (kind != EventKind::IssueDeleted).then(|| URL.to_string());
        assert_eq!(lines.link, expected, "{kind:?}");
    }
}

#[test]
fn an_unknown_actor_reads_as_a_foundry_member() {
    let cases = [
        (
            EventKind::IssueCreated,
            "New issue in Identity Platform by A Foundry member · Done",
        ),
        (
            EventKind::IssueMoved,
            "Moved In progress → Done by A Foundry member · Identity Platform",
        ),
        (
            EventKind::CommentAdded,
            "A Foundry member commented · Identity Platform",
        ),
        (
            EventKind::IssueDeleted,
            "Deleted by A Foundry member · Identity Platform",
        ),
    ];
    for (kind, line) in cases {
        assert_eq!(
            summary(&by(view(kind), "")).detail.as_deref(),
            Some(line),
            "{kind:?}"
        );
    }
}

// ---------------------------------------------------------------- a burst (D-11)

#[test]
fn a_burst_of_deletes_is_one_line_with_no_link() {
    // RATIFIED (D-13 / D-11).
    for (count, line) in [
        (6, "6 issues deleted from Identity Platform by Lena Park"),
        (7, "7 issues deleted from Identity Platform by Lena Park"),
    ] {
        assert_eq!(
            summary(&burst(EventKind::IssueDeleted, count, "Spikes")),
            MessageLines {
                headline: line.into(),
                detail: None,
                excerpt: None,
                link: None,
            }
        );
    }
}

#[test]
fn a_burst_of_moves_names_the_lane_they_went_to() {
    // RATIFIED (R-10).
    assert_eq!(
        summary(&burst(EventKind::IssueMoved, 7, "Todo")),
        MessageLines {
            headline: "7 issues moved to Todo in Identity Platform by Lena Park".into(),
            detail: None,
            excerpt: None,
            link: None,
        }
    );
}

#[test]
fn a_burst_reads_the_same_at_every_content_level() {
    let v = burst(EventKind::IssueMoved, 7, "Todo");
    for level in [
        ContentLevel::LinkOnly,
        ContentLevel::Summary,
        ContentLevel::SummaryWithComment,
    ] {
        assert_eq!(compose(&v, level), summary(&v), "{level:?}");
    }
}

#[test]
fn a_burst_of_new_issues_or_comments_is_one_line() {
    // Not ratified: pinned as the core produces it today.
    assert_eq!(
        summary(&burst(EventKind::IssueCreated, 6, "Backlog")).headline,
        "6 new issues in Identity Platform by Lena Park"
    );
    for kind in [
        EventKind::CommentAdded,
        EventKind::CommentEdited,
        EventKind::CommentDeleted,
    ] {
        assert_eq!(
            summary(&burst(kind, 6, "Backlog")).headline,
            "6 comments in Identity Platform by Lena Park",
            "{kind:?}"
        );
    }
}

#[test]
fn a_burst_by_an_unknown_actor_reads_as_a_foundry_member() {
    assert_eq!(
        summary(&by(burst(EventKind::IssueDeleted, 6, "Spikes"), "")).headline,
        "6 issues deleted from Identity Platform by A Foundry member"
    );
}

// ------------------------------------------------------- Slack's control characters

#[test]
fn slack_reads_a_title_with_control_characters_as_written() {
    // A title is shown as typed: Slack's three control characters are escaped
    // so it can never become a mention or a link.
    assert_eq!(
        foundry_integrations::escape_slack("Fix <!channel> & <@U1> > 0"),
        "Fix &lt;!channel&gt; &amp; &lt;@U1&gt; &gt; 0"
    );
    assert_eq!(foundry_integrations::escape_slack("AUTH-7"), "AUTH-7");
}
