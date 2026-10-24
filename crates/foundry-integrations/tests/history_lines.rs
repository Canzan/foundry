//! external-integration-plugins — a route's delivery history line in
//! Foundry's words (US-EIP-05, OD-7, UI-7): `DeliveryRecord::line` pinned
//! word for word, one test per outcome arm.
//!
//! Lines marked RATIFIED are quoted from the feature delta's DISCUSS copy
//! (US-EIP-05 "After" and its examples): "14:02 · New issue · AUTH-12 ·
//! Delivered", "… Delivered after 4 attempts (Slack asked Foundry to slow
//! down)", "… Retrying (Slack did not answer) · next attempt 16:15", "Failed:
//! gave up after 24 hours (Slack did not answer)". Every other line (Waiting,
//! Sending, Skipped, Failed with another reason) has no ratified string; it
//! is pinned as the core produces it today, with each reason's K-9 words.
//!
//! Pure: no I/O, no clock — `now` is passed in.

use foundry_integrations::{ChannelKind, DeliveryRecord};
use time::{Date, Duration, Month, OffsetDateTime};

const DEST: &str = "#auth-dev";

/// 21 Oct 2026 at `h:m` UTC.
fn at(h: u8, m: u8) -> OffsetDateTime {
    Date::from_calendar_date(2026, Month::October, 21)
        .expect("a date")
        .with_hms(h, m, 0)
        .expect("a time")
        .assume_utc()
}

/// A new-issue delivery for AUTH-12 on a Slack route, happened 14:02, never
/// tried, giving up 24 hours later.
fn record() -> DeliveryRecord<'static> {
    DeliveryRecord {
        channel: ChannelKind::Slack,
        occurred_at: at(14, 2),
        event_kind: "issue.created",
        issue_key: Some("AUTH-12"),
        status: "pending",
        attempts: 0,
        first_attempt_at: None,
        finished_at: None,
        give_up_at: at(14, 2) + Duration::hours(24),
        lease_until: None,
        next_attempt_at: None,
        last_vendor_reason: None,
        final_reason: None,
        slowed_down: false,
    }
}

fn line(r: &DeliveryRecord<'_>, now: OffsetDateTime) -> String {
    r.line(DEST, now)
}

// --------------------------------------------------------------- delivered

#[test]
fn delivered_at_the_first_attempt_reads_delivered() {
    // RATIFIED: "14:02 · New issue · AUTH-12 · Delivered".
    let r = DeliveryRecord {
        status: "delivered",
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 2)),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 5)),
        "14:02 · New issue · AUTH-12 · Delivered"
    );
}

#[test]
fn delivered_after_slack_asked_to_slow_down_names_the_attempts_and_why() {
    // RATIFIED: "15:40 · New comment · AUTH-7 · Delivered after 4 attempts
    // (Slack asked Foundry to slow down)". Slowing down outranks a recorded
    // reason.
    let r = DeliveryRecord {
        event_kind: "comment.added",
        issue_key: Some("AUTH-7"),
        status: "delivered",
        attempts: 4,
        first_attempt_at: Some(at(15, 30)),
        finished_at: Some(at(15, 40)),
        last_vendor_reason: Some("slack_no_answer"),
        slowed_down: true,
        ..record()
    };
    assert_eq!(
        line(&r, at(16, 0)),
        "15:40 · New comment · AUTH-7 · Delivered after 4 attempts (Slack asked Foundry to slow down)"
    );
}

#[test]
fn delivered_after_twice_with_a_recorded_reason_names_that_reason() {
    let r = DeliveryRecord {
        status: "delivered",
        attempts: 2,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 3)),
        last_vendor_reason: Some("channel_not_found"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 5)),
        "14:03 · New issue · AUTH-12 · Delivered after 2 attempts (#auth-dev was not found)"
    );
}

#[test]
fn delivered_after_unanswered_attempts_says_the_vendor_did_not_answer() {
    let slack = DeliveryRecord {
        status: "delivered",
        attempts: 3,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 4)),
        ..record()
    };
    assert_eq!(
        line(&slack, at(14, 5)),
        "14:04 · New issue · AUTH-12 · Delivered after 3 attempts (Slack did not answer)"
    );
    let matrix = DeliveryRecord {
        channel: ChannelKind::Matrix,
        ..slack.clone()
    };
    assert_eq!(
        matrix.line("#auth:canzan.dev", at(14, 5)),
        "14:04 · New issue · AUTH-12 · Delivered after 3 attempts (the homeserver did not answer)"
    );
    // A kind with no vendor to answer names no reason.
    let email = DeliveryRecord {
        channel: ChannelKind::Email,
        ..slack
    };
    assert_eq!(
        line(&email, at(14, 5)),
        "14:04 · New issue · AUTH-12 · Delivered after 3 attempts"
    );
}

// ----------------------------------------------------------- still pending

#[test]
fn never_tried_reads_waiting_at_the_time_it_happened() {
    assert_eq!(
        line(&record(), at(14, 3)),
        "14:02 · New issue · AUTH-12 · Waiting"
    );
}

#[test]
fn tried_with_no_answer_recorded_yet_reads_sending_at_its_first_attempt() {
    let r = DeliveryRecord {
        attempts: 1,
        first_attempt_at: Some(at(14, 3)),
        lease_until: Some(at(14, 4)),
        ..record()
    };
    assert_eq!(line(&r, at(14, 3)), "14:03 · New issue · AUTH-12 · Sending");
}

#[test]
fn retrying_after_slack_did_not_answer_names_the_next_attempt() {
    // RATIFIED: "16:10 · Issue moved · AUTH-8 · Retrying (Slack did not
    // answer) · next attempt 16:15".
    let r = DeliveryRecord {
        occurred_at: at(16, 9),
        event_kind: "issue.moved",
        issue_key: Some("AUTH-8"),
        attempts: 1,
        first_attempt_at: Some(at(16, 10)),
        give_up_at: at(16, 9) + Duration::hours(24),
        next_attempt_at: Some(at(16, 15)),
        last_vendor_reason: Some("slack_no_answer"),
        ..record()
    };
    assert_eq!(
        line(&r, at(16, 12)),
        "16:10 · Issue moved · AUTH-8 · Retrying (Slack did not answer) · next attempt 16:15"
    );
}

#[test]
fn retrying_with_no_next_attempt_set_ends_at_the_reason() {
    let r = DeliveryRecord {
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        last_vendor_reason: Some("slack_no_answer"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 3)),
        "14:02 · New issue · AUTH-12 · Retrying (Slack did not answer)"
    );
}

#[test]
fn retrying_after_a_slow_down_says_slack_asked_foundry_to_slow_down() {
    let r = DeliveryRecord {
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        next_attempt_at: Some(at(14, 3)),
        slowed_down: true,
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 2)),
        "14:02 · New issue · AUTH-12 · Retrying (Slack asked Foundry to slow down) · next attempt 14:03"
    );
}

#[test]
fn retrying_after_a_reason_foundry_does_not_know_reads_as_no_answer_never_the_stored_text() {
    // A stored reason that is not one of Foundry's own is never shown.
    let r = DeliveryRecord {
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        next_attempt_at: Some(at(14, 3)),
        last_vendor_reason: Some("xoxb-leaked raw-vendor-detail"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 2)),
        "14:02 · New issue · AUTH-12 · Retrying (Slack did not answer) · next attempt 14:03"
    );
}

#[test]
fn retrying_on_a_kind_with_no_vendor_and_no_known_reason_reads_retrying() {
    let r = DeliveryRecord {
        channel: ChannelKind::Email,
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        last_vendor_reason: Some("not-a-reason"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 2)),
        "14:02 · New issue · AUTH-12 · Retrying"
    );
}

// ------------------------------------------------------------------- ended

#[test]
fn skipped_names_why_in_foundrys_words() {
    let r = DeliveryRecord {
        status: "skipped",
        finished_at: Some(at(14, 4)),
        final_reason: Some("route_paused"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 5)),
        "14:04 · New issue · AUTH-12 · Skipped: the route was paused"
    );
}

#[test]
fn skipped_without_a_known_reason_reads_skipped() {
    let r = DeliveryRecord {
        status: "skipped",
        finished_at: Some(at(14, 4)),
        final_reason: Some("not-a-reason"),
        ..record()
    };
    assert_eq!(line(&r, at(14, 5)), "14:04 · New issue · AUTH-12 · Skipped");
}

#[test]
fn failed_for_another_reason_names_it_in_foundrys_words() {
    let r = DeliveryRecord {
        status: "failed",
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 3)),
        final_reason: Some("channel_archived"),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 5)),
        "14:03 · New issue · AUTH-12 · Failed: #auth-dev was archived"
    );
}

#[test]
fn failed_with_no_reason_reads_failed() {
    let r = DeliveryRecord {
        status: "failed",
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 3)),
        ..record()
    };
    assert_eq!(line(&r, at(14, 5)), "14:03 · New issue · AUTH-12 · Failed");
}

#[test]
fn stored_as_given_up_reads_gave_up_after_24_hours_with_why() {
    // RATIFIED: "Failed: gave up after 24 hours (Slack did not answer)".
    let r = DeliveryRecord {
        issue_key: Some("AUTH-14"),
        status: "failed",
        attempts: 5,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 2) + Duration::hours(24)),
        final_reason: Some("gave_up_24h"),
        ..record()
    };
    assert_eq!(
        line(&r, at(15, 0) + Duration::hours(24)),
        "14:02 · New issue · AUTH-14 · Failed: gave up after 24 hours (Slack did not answer)"
    );
}

#[test]
fn stored_as_given_up_after_a_slow_down_says_so() {
    let r = DeliveryRecord {
        status: "failed",
        attempts: 2,
        first_attempt_at: Some(at(14, 2)),
        finished_at: Some(at(14, 2) + Duration::hours(24)),
        final_reason: Some("gave_up_24h"),
        slowed_down: true,
        ..record()
    };
    assert_eq!(
        line(&r, at(15, 0) + Duration::hours(24)),
        "14:02 · New issue · AUTH-12 · Failed: gave up after 24 hours (Slack asked Foundry to slow down)"
    );
}

#[test]
fn stored_as_given_up_never_tried_names_no_reason() {
    let r = DeliveryRecord {
        status: "failed",
        finished_at: Some(at(14, 2) + Duration::hours(24)),
        final_reason: Some("gave_up_24h"),
        ..record()
    };
    assert_eq!(
        line(&r, at(15, 0) + Duration::hours(24)),
        "14:02 · New issue · AUTH-12 · Failed: gave up after 24 hours"
    );
}

#[test]
fn pending_at_its_give_up_time_with_no_live_lease_has_given_up() {
    let r = DeliveryRecord {
        attempts: 3,
        first_attempt_at: Some(at(14, 2)),
        next_attempt_at: Some(at(14, 32)),
        last_vendor_reason: Some("slack_no_answer"),
        ..record()
    };
    let give_up = r.give_up_at;
    assert_eq!(
        line(&r, give_up),
        "14:02 · New issue · AUTH-12 · Failed: gave up after 24 hours (Slack did not answer)"
    );
    // One second earlier it is still retrying.
    assert_eq!(
        line(&r, give_up - Duration::seconds(1)),
        "14:02 · New issue · AUTH-12 · Retrying (Slack did not answer) · next attempt 14:32"
    );
}

#[test]
fn pending_past_its_give_up_time_under_a_live_lease_is_still_being_sent() {
    let r = DeliveryRecord {
        attempts: 1,
        first_attempt_at: Some(at(14, 2)),
        lease_until: Some(at(14, 2) + Duration::hours(25)),
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 2) + Duration::hours(24)),
        "14:02 · New issue · AUTH-12 · Sending"
    );
}

// ------------------------------------------------------- the event and key

#[test]
fn every_event_reads_by_its_display_name() {
    for (wire, shown) in [
        ("issue.created", "New issue"),
        ("issue.moved", "Issue moved"),
        ("issue.deleted", "Issue deleted"),
        ("comment.added", "New comment"),
        ("comment.edited", "Comment edited"),
        ("comment.deleted", "Comment deleted"),
    ] {
        let r = DeliveryRecord {
            event_kind: wire,
            ..record()
        };
        assert_eq!(
            line(&r, at(14, 3)),
            format!("14:02 · {shown} · AUTH-12 · Waiting")
        );
    }
}

#[test]
fn a_delivery_without_an_issue_key_has_no_key_part() {
    let r = DeliveryRecord {
        issue_key: None,
        ..record()
    };
    assert_eq!(line(&r, at(14, 3)), "14:02 · New issue · Waiting");
}

#[test]
fn the_time_is_when_it_ended_else_first_tried_else_happened() {
    let ended = DeliveryRecord {
        status: "delivered",
        attempts: 1,
        first_attempt_at: Some(at(9, 5)),
        finished_at: Some(at(9, 7)),
        occurred_at: at(9, 1),
        give_up_at: at(9, 1) + Duration::hours(24),
        ..record()
    };
    assert!(line(&ended, at(10, 0)).starts_with("09:07 · "));
    let tried = DeliveryRecord {
        status: "pending",
        finished_at: None,
        ..ended.clone()
    };
    assert!(line(&tried, at(10, 0)).starts_with("09:05 · "));
    let happened = DeliveryRecord {
        first_attempt_at: None,
        attempts: 0,
        ..tried
    };
    assert!(line(&happened, at(10, 0)).starts_with("09:01 · "));
}

#[test]
fn an_event_outside_the_catalogue_reads_as_its_stored_name() {
    let r = DeliveryRecord {
        event_kind: "issue.archived",
        ..record()
    };
    assert_eq!(
        line(&r, at(14, 3)),
        "14:02 · issue.archived · AUTH-12 · Waiting"
    );
}
