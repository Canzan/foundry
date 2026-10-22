//! external-integration-plugins — the pure core's property and table tests
//! (DESIGN [REF] Driven Ports and Contract Shapes; brief "Properties for layer
//! 1-2"; DEVOPS `eip-core-unit`). Pending stubs written by DISTILL 2026-10-21
//! (ADR-025 scaffolded RED): each is `#[ignore]`d with its DELIVER step id and
//! panics with the RED marker until DELIVER replaces its body with a proptest
//! (`proptest = "1"`, already locked) or a table test.

use foundry_integrations::{
    claimable_head, expiry_set, Attention, ChannelKind, ConnectionState, ContentLevel,
    DispatchPlan, EventKind, FinishedDelivery, OutboxRow, QueuedDelivery, ReasonCode, RouteFacts,
    RouteSpec, RouteStatus,
};
use proptest::prelude::*;
use std::collections::BTreeSet;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

macro_rules! pending {
    ($name:ident, $step:literal, $what:literal) => {
        #[test]
        #[ignore = concat!("pending: external-integration-plugins ", $step, " ", $what)]
        fn $name() {
            panic!(concat!(
                "Not yet implemented -- RED scaffold (external-integration-plugins ",
                $step,
                ": ",
                $what,
                ")"
            ));
        }
    };
}

// compose (D-3), slices 01-02.
pending!(compose_never_sends_description_attachment_email_or_internal_id, "02-02", "proptest: no description, attachment name, email address or id other than issue key and event id, at any level");
pending!(compose_cuts_comment_text_at_300_characters_with_ellipsis, "02-02", "proptest: the excerpt is at most 300 characters plus an ellipsis, plain text, only at Summary with comment text");
// encoders (never pings, DDR-12, DDR-13).
pending!(encode_slack_never_leaves_a_live_mention, "02-02", "proptest over arbitrary text: no <!…>, <@…>, <#…> live in mrkdwn; user text in plain_text, parse none, link_names false");
pending!(
    encode_matrix_never_mentions_the_room_or_a_user,
    "04-02",
    "proptest over arbitrary text: m.mentions empty, no literal @room, no pill, no unescaped HTML"
);
pending!(
    encode_email_carries_user_text_as_plain_text,
    "05-01",
    "proptest over arbitrary text: no markup or header injection"
);
// dispatch plan (DDR-7, DDR-8).
pending!(
    dispatch_plan_has_no_reorder_event,
    "02-01",
    "proptest: an in-lane reposition never becomes an event"
);
pending!(
    dispatch_plan_bursts_more_than_five_per_source_transaction,
    "02-01",
    "proptest: more than 5 same-kind rows of one source transaction make one burst entry per route"
);
// retry policy (DDR-5).
pending!(
    retry_policy_never_schedules_past_give_up,
    "03-01",
    "proptest: next attempt is never after give_up_at"
);
pending!(
    retry_policy_honours_a_larger_retry_after,
    "03-01",
    "proptest: a larger retry_after wins over the backoff"
);
pending!(
    retry_policy_is_monotone_in_attempts,
    "03-01",
    "proptest: the delay never shrinks as attempts grow (jitter bounded)"
);
// classifiers (closed lists, unknown -> transient).
pending!(
    classify_slack_recorded_answers,
    "03-01",
    "table over recorded Slack answers, including 200 with ok:false; unknown codes are transient"
);
pending!(
    classify_matrix_recorded_answers,
    "04-02",
    "table over recorded Matrix answers (401, 403, 429 retry_after_ms, 5xx, unknown errcode)"
);
// plan functions (claim and sweeps).
pending!(
    cancel_set_is_exactly_the_targets_unleased_non_terminal_rows,
    "03-03",
    "proptest: exactly the target's unleased non-terminal deliveries"
);
pending!(
    prune_set_keeps_the_newest_100_and_referenced_outbox_rows,
    "03-01",
    "proptest: past 30 days, beyond the newest 100 per route, outbox rows unreferenced"
);
// address judge (DDR-14).
pending!(judge_refuses_every_reserved_range_and_mapped_form, "04-01", "proptest: loopback, link-local, metadata, 0/8, 240/4, documentation and benchmark ranges, IPv4-mapped, NAT64, 6to4, decimal literals; private only via the allow-list; a port-less address defaults to 443 before judging. Carries the 04-matrix rows DISTILL moved out of acceptance (link-local 169.254.169.254, private 10.0.0.5, port-less https://10.0.0.5): nothing listens there, so only the judge can pin them");

// ---------------------------------------------------------------------------
// 01-04: the dispatch plan and the claim and expiry planners.
// ---------------------------------------------------------------------------

/// The fixed instant every generated time is an offset from; offsets of a few
/// seconds either side make `==` boundaries frequent.
fn epoch() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_790_000_000).expect("valid timestamp")
}

fn at(offset_seconds: i64) -> OffsetDateTime {
    epoch() + Duration::seconds(offset_seconds)
}

fn maybe_at() -> impl Strategy<Value = Option<OffsetDateTime>> {
    proptest::option::of((-5i64..=5).prop_map(at))
}

/// The six shipped outbox types plus one the catalogue does not know.
const OUTBOX_TYPES: [&str; 7] = [
    "IssueCreated",
    "IssueUpdated",
    "IssueDeleted",
    "CommentAdded",
    "CommentEdited",
    "CommentDeleted",
    "IssueReordered",
];

const WIRE_NAMES: [&str; 6] = [
    "issue.created",
    "issue.moved",
    "issue.deleted",
    "comment.added",
    "comment.edited",
    "comment.deleted",
];

const CONNECTION_STATES: [Option<ConnectionState>; 4] = [
    None,
    Some(ConnectionState::Connected),
    Some(ConnectionState::Lost),
    Some(ConnectionState::Disconnected),
];

fn project(index: usize) -> Uuid {
    Uuid::from_u128(0x1000 + index as u128)
}

fn workspace_of(project_index: usize) -> Uuid {
    Uuid::from_u128(0x2000 + project_index as u128)
}

fn outbox_rows() -> impl Strategy<Value = Vec<OutboxRow>> {
    proptest::collection::vec((0usize..3, 0usize..OUTBOX_TYPES.len(), -5i64..=5), 0..8).prop_map(
        |rows| {
            rows.into_iter()
                .enumerate()
                .map(|(index, (project_index, type_index, created))| {
                    let issue_id = Uuid::from_u128(0x3000 + index as u128);
                    OutboxRow {
                        id: 100 + index as i64,
                        event_id: Uuid::from_u128(0x4000 + index as u128),
                        source_txid: 7,
                        event_type: OUTBOX_TYPES[type_index].to_string(),
                        payload: serde_json::json!({
                            "issue_id": issue_id,
                            "project_id": project(project_index),
                            "workspace_id": workspace_of(project_index),
                            "key": format!("P{project_index}-{index}"),
                        }),
                        created_at: at(created),
                    }
                })
                .collect()
        },
    )
}

/// Generated routes, each with whether its stored wire names choose a New
/// issue (read from the wire text, not through the catalogue under test).
fn route_specs() -> impl Strategy<Value = Vec<(RouteSpec, bool)>> {
    let route = (
        0usize..3,
        proptest::sample::subsequence(WIRE_NAMES.to_vec(), 0..=WIRE_NAMES.len()),
        any::<bool>(),
        0usize..CONNECTION_STATES.len(),
        -5i64..=5,
    );
    proptest::collection::vec(route, 0..6).prop_map(|routes| {
        routes
            .into_iter()
            .enumerate()
            .map(|(index, (project_index, wires, paused, state, from))| {
                let spec = RouteSpec {
                    id: Uuid::from_u128(0x5000 + index as u128),
                    workspace_id: workspace_of(project_index),
                    project_id: project(project_index),
                    kind: ChannelKind::Slack,
                    connection_state: CONNECTION_STATES[state],
                    events: wires
                        .iter()
                        .map(|wire| EventKind::from_wire_name(wire).expect("a catalogue wire name"))
                        .collect(),
                    content_level: ContentLevel::Summary,
                    paused,
                    receives_from: at(from),
                    lane_ids: Vec::new(),
                };
                (spec, wires.contains(&"issue.created"))
            })
            .collect()
    })
}

/// The pairs a route of slice 01 receives: a New issue of its own project
/// that it chose, written at or after its `receives_from` (DDR-27), while it
/// is not paused and its connection is not disconnected.
fn expected_pairs(rows: &[OutboxRow], routes: &[(RouteSpec, bool)]) -> BTreeSet<(i64, Uuid)> {
    let mut pairs = BTreeSet::new();
    for row in rows {
        for (route, chooses_new_issue) in routes {
            let project_matches = row.payload["project_id"] == serde_json::json!(route.project_id);
            let chosen = row.event_type == "IssueCreated" && *chooses_new_issue;
            let open = !route.paused
                && route.connection_state != Some(ConnectionState::Disconnected)
                && row.created_at >= route.receives_from;
            if project_matches && chosen && open {
                pairs.insert((row.id, route.id));
            }
        }
    }
    pairs
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// DDR-8: a row of another project never becomes a route's delivery; the
    /// plan is exactly the eligible (row, route) pairs, each delivery carrying
    /// its own row's facts and its route's project.
    #[test]
    fn dispatch_plan_never_matches_another_project(
        rows in outbox_rows(),
        generated in route_specs(),
    ) {
        let routes: Vec<RouteSpec> = generated.iter().map(|(route, _)| route.clone()).collect();
        let plan = DispatchPlan::build(&rows, &routes);

        let planned: BTreeSet<(i64, Uuid)> = plan
            .deliveries()
            .iter()
            .map(|delivery| (delivery.outbox_ids[0], delivery.route_id))
            .collect();
        prop_assert_eq!(plan.deliveries().len(), planned.len(), "one delivery per pair");
        prop_assert_eq!(&planned, &expected_pairs(&rows, &generated));

        for delivery in plan.deliveries() {
            let route = routes.iter().find(|r| r.id == delivery.route_id).expect("a route");
            let row = rows.iter().find(|r| r.id == delivery.outbox_ids[0]).expect("a row");
            prop_assert_eq!(delivery.project_id, route.project_id);
            prop_assert_eq!(delivery.workspace_id, route.workspace_id);
            prop_assert_eq!(&delivery.outbox_ids, &vec![row.id]);
            prop_assert_eq!(delivery.event_id, row.event_id);
            prop_assert_eq!(delivery.event_kind.wire_name(), "issue.created");
            prop_assert_eq!(
                delivery.issue_id,
                Some(serde_json::from_value::<Uuid>(row.payload["issue_id"].clone()).unwrap())
            );
            prop_assert_eq!(delivery.issue_key.as_deref(), row.payload["key"].as_str());
            prop_assert_eq!(delivery.occurred_at, row.created_at);
        }
    }

    /// ADR-EIP-002 decision 4: the claim choice is the route's head (its
    /// lowest id) or nothing — the head only while it has no live lease, is due
    /// and its destination's pacing allows a send now.
    #[test]
    fn claimable_head_takes_the_head_only_without_a_live_lease(
        mut deliveries in proptest::collection::vec(
            (maybe_at(), maybe_at(), -5i64..=5), 0..6
        ).prop_map(|rows| rows.into_iter().enumerate().map(|(index, (next, lease, give_up))| {
            QueuedDelivery {
                id: 10 + 3 * index as i64,
                next_attempt_at: next,
                lease_until: lease,
                give_up_at: at(give_up),
            }
        }).collect::<Vec<_>>()),
        not_before in maybe_at(),
        now in (-3i64..=3).prop_map(at),
        rotate in 0usize..6,
    ) {
        let expected = deliveries.first().copied().filter(|head| {
            head.lease_until.is_none_or(|until| until <= now)
                && head.next_attempt_at.is_none_or(|next| next <= now)
                && not_before.is_none_or(|paced| paced <= now)
        }).map(|head| head.id);
        if !deliveries.is_empty() {
            let by = rotate % deliveries.len();
            deliveries.rotate_left(by);
        }

        prop_assert_eq!(claimable_head(&deliveries, not_before, now), expected);
    }

    /// ADR-EIP-002 decision 9: the expiry sweep's set is exactly the
    /// deliveries at or past their give-up time that hold no live lease.
    #[test]
    fn expiry_set_is_exactly_past_give_up_and_unleased(
        deliveries in proptest::collection::vec(
            (maybe_at(), maybe_at(), -5i64..=5), 0..8
        ).prop_map(|rows| rows.into_iter().enumerate().map(|(index, (next, lease, give_up))| {
            QueuedDelivery {
                id: 1 + index as i64,
                next_attempt_at: next,
                lease_until: lease,
                give_up_at: at(give_up),
            }
        }).collect::<Vec<_>>()),
        now in (-3i64..=3).prop_map(at),
    ) {
        let expected: BTreeSet<i64> = deliveries
            .iter()
            .filter(|d| d.give_up_at <= now && d.lease_until.is_none_or(|until| until <= now))
            .map(|d| d.id)
            .collect();

        let chosen = expiry_set(now, &deliveries);
        let as_set: BTreeSet<i64> = chosen.iter().copied().collect();

        prop_assert_eq!(chosen.len(), as_set.len(), "no id twice");
        prop_assert_eq!(as_set, expected);
    }
}

// ---------------------------------------------------------------------------
// 01-06: route status (DDR-19), every precedence pair of the eight rules.
// ---------------------------------------------------------------------------

/// A route that matches no rule but the last: connected, never paused, nothing
/// wrong, last delivered at 14:02.
fn delivering() -> RouteFacts {
    RouteFacts {
        kind: ChannelKind::Slack,
        paused: false,
        connection_state: Some(ConnectionState::Connected),
        problem: None,
        missing_lane: None,
        newest_finished: Some(FinishedDelivery::Other),
        head_failed: None,
        last_delivered_at: Some(time::macros::datetime!(2026-10-21 14:02 UTC)),
    }
}

/// Rule `n` (1-7) made true on `facts`.
fn with_rule(mut facts: RouteFacts, n: usize) -> RouteFacts {
    match n {
        1 => facts.paused = true,
        2 => facts.connection_state = Some(ConnectionState::Disconnected),
        3 => facts.connection_state = Some(ConnectionState::Lost),
        4 => facts.problem = Some(ReasonCode::NotInChannel),
        5 => facts.missing_lane = Some("QA".to_string()),
        6 => {
            facts.newest_finished = Some(FinishedDelivery::GaveUp {
                issue_key: "AUTH-12".to_string(),
            })
        }
        7 => facts.head_failed = Some(Some(ReasonCode::SlackNoAnswer)),
        _ => {}
    }
    facts
}

/// What rule `n` gives, in K-9's words for #auth-dev.
fn expected(n: usize) -> (RouteStatus, &'static str) {
    match n {
        1 => (RouteStatus::Paused, "Paused"),
        2 => (
            RouteStatus::Disconnected(ChannelKind::Slack),
            "Paused: Slack was disconnected",
        ),
        3 => (RouteStatus::ConnectionLost, "Paused: connection lost"),
        4 => (
            RouteStatus::NeedsAttention(Attention::Problem(ReasonCode::NotInChannel)),
            "Needs attention: Foundry is no longer in #auth-dev. Invite @Foundry back, then Resume.",
        ),
        5 => (
            RouteStatus::NeedsAttention(Attention::LaneMissing {
                label: "QA".to_string(),
            }),
            "Needs attention: the lane QA no longer exists. Choose lanes again.",
        ),
        6 => (
            RouteStatus::NeedsAttention(Attention::GaveUp {
                issue_key: "AUTH-12".to_string(),
            }),
            "Needs attention: AUTH-12's message could not be delivered within 24 hours",
        ),
        7 => (
            RouteStatus::Retrying(Some(ReasonCode::SlackNoAnswer)),
            "Retrying: Slack did not answer",
        ),
        _ => (
            RouteStatus::Delivering {
                last_delivery: Some(time::macros::datetime!(2026-10-21 14:02 UTC)),
            },
            "Delivering · last delivery 14:02",
        ),
    }
}

/// bypass: a precedence table is a finite, exhaustive table, not a property
/// over generated inputs: every pair of rules (and each rule alone) is listed.
#[test]
fn route_status_derive_precedence() {
    for higher in 1..=8 {
        for lower in higher..=8 {
            // Rules 2 and 3 are two states of one connection: never both.
            if (higher, lower) == (2, 3) {
                continue;
            }
            let facts = with_rule(with_rule(delivering(), lower), higher);
            let (status, words) = expected(higher);
            let derived = RouteStatus::derive(&facts);
            assert_eq!(
                derived, status,
                "rule {higher} over rule {lower}: {facts:?}"
            );
            assert_eq!(
                derived.words("#auth-dev"),
                words,
                "rule {higher} over rule {lower}"
            );
        }
    }
    // A webhook route has no connection: rules 2 and 3 never apply to it.
    let webhook = RouteFacts {
        kind: ChannelKind::Webhook,
        connection_state: None,
        ..delivering()
    };
    assert_eq!(RouteStatus::derive(&webhook), expected(8).0);
    // Never delivered, nothing wrong: "Delivering" without a time; a head that
    // failed without a recorded reason: "Retrying".
    let fresh = RouteFacts {
        last_delivered_at: None,
        newest_finished: None,
        ..delivering()
    };
    assert_eq!(RouteStatus::derive(&fresh).words("#auth-dev"), "Delivering");
    let unexplained = RouteFacts {
        head_failed: Some(None),
        ..delivering()
    };
    assert_eq!(
        RouteStatus::derive(&unexplained).words("#auth-dev"),
        "Retrying"
    );
    // A finished delivery is a give-up only when it failed for gave_up_24h.
    for (status, reason, key, gave_up) in [
        ("failed", Some("gave_up_24h"), Some("AUTH-12"), true),
        ("failed", Some("not_in_channel"), Some("AUTH-12"), false),
        ("skipped", Some("gave_up_24h"), Some("AUTH-12"), false),
        ("delivered", None, Some("AUTH-12"), false),
        ("failed", Some("gave_up_24h"), None, false),
    ] {
        let read = FinishedDelivery::read(status, reason, key);
        assert_eq!(
            read,
            if gave_up {
                FinishedDelivery::GaveUp {
                    issue_key: "AUTH-12".to_string(),
                }
            } else {
                FinishedDelivery::Other
            },
            "{status} {reason:?} {key:?}"
        );
    }
}
