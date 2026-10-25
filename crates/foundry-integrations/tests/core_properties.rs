//! external-integration-plugins — the pure core's property and table tests
//! (DESIGN [REF] Driven Ports and Contract Shapes; brief "Properties for layer
//! 1-2"; DEVOPS `eip-core-unit`). Pending stubs written by DISTILL 2026-10-21
//! (ADR-025 scaffolded RED): each is `#[ignore]`d with its DELIVER step id and
//! panics with the RED marker until DELIVER replaces its body with a proptest
//! (`proptest = "1"`, already locked) or a table test.

use foundry_integrations::{
    cancel_set, claimable_head, classify_slack, compose, encode_slack, expiry_set, issue_url,
    prune_set, Attempt, Attention, ChannelKind, ChannelOutcome, ConnectionState, ContentLevel,
    DispatchPlan, EventKind, EventView, FilterLane, FinishedDelivery, Jitter, OutboxRow,
    QueuedDelivery, ReasonCode, RetainedDelivery, RetainedOutboxRow, RetryPolicy, RouteFacts,
    RouteSpec, RouteStatus, Settlement, SlackAnswer, TargetedDelivery, RETAINED_PER_ROUTE,
};
use foundry_integrations::{classify_matrix, encode_matrix, MatrixAnswer, MessageLines};
use foundry_integrations::{
    judge, judge_host, OutboundAllow, OutboundTarget, Refusal, TargetHost, Verdict,
};
use proptest::prelude::*;
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

// encoders (never pings, DDR-12, DDR-13).
// (04-02, below: encode_matrix_never_mentions_the_room_or_a_user.)
proptest! {
    /// 05-01: whatever people wrote (a title, a name, a comment, line breaks,
    /// markup, header-shaped text) reaches the body exactly as written, with
    /// no markup added, and never starts a header: the recipient and subject
    /// are one line each, every control character read as a space.
    #[test]
    fn encode_email_carries_user_text_as_plain_text(
        recipient in ".{0,40}",
        subject in ".{0,80}|[\\r\\n\\t]{0,3}Bcc: x@y.z[\\r\\n]{0,2}.{0,20}",
        body in proptest::collection::vec("(.|\\n|\\r){0,120}", 0..6),
    ) {
        let lines = foundry_integrations::EmailLines { subject: subject.clone(), body: body.clone() };
        let email = foundry_integrations::encode_email(&recipient, &lines);
        prop_assert_eq!(&email.text_body, &body.join("\n"));
        for (field, original) in [(&email.recipient, &recipient), (&email.subject, &subject)] {
            prop_assert!(!field.chars().any(char::is_control), "{:?}", field);
            let spaced: String = original
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            prop_assert_eq!(field, &spaced);
        }
        prop_assert_eq!(email.unsubscribe, None);
    }
}
// dispatch plan (DDR-7, DDR-8).
// classifiers (closed lists, unknown -> transient).
// (04-02, below: classify_matrix_recorded_answers.)
// plan functions (claim and sweeps).
// address judge (DDR-14).
// (04-01, below the plan functions: judge_refuses_every_reserved_range_and_mapped_form.)

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
                        // One transaction per row: bursts are their own property.
                        source_txid: index as u64,
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

/// Generated routes, each with its stored wire names (read as text, not
/// through the catalogue under test).
fn route_specs() -> impl Strategy<Value = Vec<(RouteSpec, Vec<&'static str>)>> {
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
                (spec, wires)
            })
            .collect()
    })
}

/// The wire name a chat route knows an outbox type by: the four chat events
/// of D-2 (ADR-EIP-003); comment edits and deletes, and unknown types, none.
fn chat_wire(event_type: &str) -> Option<&'static str> {
    match event_type {
        "IssueCreated" => Some("issue.created"),
        "IssueUpdated" => Some("issue.moved"),
        "IssueDeleted" => Some("issue.deleted"),
        "CommentAdded" => Some("comment.added"),
        _ => None,
    }
}

/// The pairs a chat route receives: a chat event of its own project that it
/// chose, written at or after its `receives_from` (DDR-27), while it is not
/// paused and its connection is not disconnected.
fn expected_pairs(
    rows: &[OutboxRow],
    routes: &[(RouteSpec, Vec<&'static str>)],
) -> BTreeSet<(i64, Uuid)> {
    let mut pairs = BTreeSet::new();
    for row in rows {
        for (route, wires) in routes {
            let project_matches = row.payload["project_id"] == serde_json::json!(route.project_id);
            let chosen = chat_wire(&row.event_type).is_some_and(|wire| wires.contains(&wire));
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
            prop_assert_eq!(Some(delivery.event_kind.wire_name()), chat_wire(&row.event_type));
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
// 03-03: the cancel set of a pause (DD-8).
// ---------------------------------------------------------------------------

/// Two targets, so another target's rows are always in play.
fn target(index: u8) -> Uuid {
    Uuid::from_u128(0x0303_0000 + u128::from(index))
}

proptest! {
    #[test]
    fn cancel_set_is_exactly_the_targets_unleased_non_terminal_rows(
        deliveries in proptest::collection::vec(
            (0u8..2, any::<bool>(), maybe_at()), 0..10
        ).prop_map(|rows| rows.into_iter().enumerate().map(|(index, (whose, finished, lease))| {
            TargetedDelivery {
                id: 1 + index as i64,
                target: target(whose),
                finished,
                lease_until: lease,
            }
        }).collect::<Vec<_>>()),
        whose in 0u8..2,
        now in (-3i64..=3).prop_map(at),
    ) {
        let chosen_target = target(whose);
        let expected: BTreeSet<i64> = deliveries
            .iter()
            .filter(|d| d.target == chosen_target && !d.finished)
            .filter(|d| d.lease_until.is_none_or(|until| until <= now))
            .map(|d| d.id)
            .collect();

        let chosen = cancel_set(chosen_target, now, &deliveries);
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

// ---------------------------------------------------------------------------
// 02-01: no reorder event, and bursts (DDR-7, DDR-8, D-11, ADR-EIP-003).
// ---------------------------------------------------------------------------

/// One open Slack route per index on project 0, choosing every chat event.
fn chat_routes(count: usize) -> Vec<RouteSpec> {
    (0..count)
        .map(|index| RouteSpec {
            id: Uuid::from_u128(0x5000 + index as u128),
            workspace_id: workspace_of(0),
            project_id: project(0),
            kind: ChannelKind::Slack,
            connection_state: Some(ConnectionState::Connected),
            events: [
                "issue.created",
                "issue.moved",
                "issue.deleted",
                "comment.added",
            ]
            .iter()
            .map(|wire| EventKind::from_wire_name(wire).expect("a chat event"))
            .collect(),
            content_level: ContentLevel::Summary,
            paused: false,
            receives_from: at(-5),
            lane_ids: Vec::new(),
        })
        .collect()
}

/// A row of project 0 in transaction `source_txid`, `from` and `to` its lanes.
fn board_row(index: usize, source_txid: u64, event_type: &str, from: &str, to: &str) -> OutboxRow {
    OutboxRow {
        id: 100 + index as i64,
        event_id: Uuid::from_u128(0x4000 + index as u128),
        source_txid,
        event_type: event_type.to_string(),
        payload: serde_json::json!({
            "issue_id": Uuid::from_u128(0x3000 + index as u128),
            "project_id": project(0),
            "workspace_id": workspace_of(0),
            "key": format!("AUTH-{index}"),
            "from_state": from,
            "state": to,
        }),
        created_at: at(0),
    }
}

const LANES: [&str; 3] = ["todo", "in-progress", "done"];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// ADR-EIP-003 decision 1: there is no reorder event. A reposition inside
    /// one lane (an `IssueUpdated` whose lane did not change) and a type the
    /// catalogue does not know never reach a delivery, while every real move
    /// still reaches each route.
    #[test]
    fn dispatch_plan_has_no_reorder_event(
        picks in proptest::collection::vec(
            (0usize..3, 0usize..LANES.len(), 0usize..LANES.len()), 0..12
        ),
        route_count in 1usize..3,
    ) {
        let rows: Vec<OutboxRow> = picks
            .iter()
            .enumerate()
            .map(|(index, (shape, from, to))| {
                let event_type = if *shape == 2 { "IssueReordered" } else { "IssueUpdated" };
                let to = if *shape == 1 { from } else { to };
                board_row(index, index as u64, event_type, LANES[*from], LANES[*to])
            })
            .collect();
        let routes = chat_routes(route_count);

        let plan = DispatchPlan::build(&rows, &routes);

        let moves: BTreeSet<i64> = rows
            .iter()
            .filter(|row| {
                row.event_type == "IssueUpdated" && row.payload["from_state"] != row.payload["state"]
            })
            .map(|row| row.id)
            .collect();
        let delivered: Vec<(i64, Uuid)> = plan
            .deliveries()
            .iter()
            .flat_map(|d| d.outbox_ids.iter().map(move |id| (*id, d.route_id)))
            .collect();
        let expected: BTreeSet<(i64, Uuid)> = moves
            .iter()
            .flat_map(|id| routes.iter().map(move |route| (*id, route.id)))
            .collect();
        prop_assert_eq!(delivered.len(), expected.len(), "no row twice to one route");
        prop_assert_eq!(delivered.into_iter().collect::<BTreeSet<_>>(), expected);
        for delivery in plan.deliveries() {
            prop_assert_eq!(delivery.event_kind, EventKind::IssueMoved);
        }
    }

    /// D-11, DDR-8: more than five rows of one kind from one source
    /// transaction make one delivery per route listing every row (its event
    /// id the first row's, no single issue); five or fewer stay one delivery
    /// per row. Each route's deliveries keep the order the rows happened in.
    #[test]
    fn dispatch_plan_bursts_more_than_five_per_source_transaction(
        picks in proptest::collection::vec((1u64..4, 0usize..3), 0..40),
        route_count in 1usize..3,
    ) {
        const TYPES: [&str; 3] = ["IssueDeleted", "IssueUpdated", "CommentAdded"];
        let rows: Vec<OutboxRow> = picks
            .iter()
            .enumerate()
            .map(|(index, (txid, kind))| board_row(index, *txid, TYPES[*kind], "spikes", "todo"))
            .collect();
        let routes = chat_routes(route_count);

        let plan = DispatchPlan::build(&rows, &routes);

        let mut expected: BTreeSet<(Uuid, Vec<i64>)> = BTreeSet::new();
        for route in &routes {
            for txid in 1u64..4 {
                for kind in TYPES {
                    let group: Vec<i64> = rows
                        .iter()
                        .filter(|row| row.source_txid == txid && row.event_type == kind)
                        .map(|row| row.id)
                        .collect();
                    if group.len() > 5 {
                        expected.insert((route.id, group));
                    } else {
                        expected.extend(group.into_iter().map(|id| (route.id, vec![id])));
                    }
                }
            }
        }
        let planned: Vec<(Uuid, Vec<i64>)> = plan
            .deliveries()
            .iter()
            .map(|d| (d.route_id, d.outbox_ids.clone()))
            .collect();
        prop_assert_eq!(planned.len(), expected.len(), "one delivery per group or row");
        prop_assert_eq!(planned.iter().cloned().collect::<BTreeSet<_>>(), expected);

        for delivery in plan.deliveries() {
            let first = rows.iter().find(|r| r.id == delivery.outbox_ids[0]).expect("a row");
            prop_assert_eq!(delivery.event_id, first.event_id);
            prop_assert_eq!(delivery.occurred_at, first.created_at);
            let burst = delivery.outbox_ids.len() > 1;
            prop_assert_eq!(delivery.issue_id.is_none(), burst, "a burst names no one issue");
            prop_assert_eq!(delivery.issue_key.is_none(), burst);
        }
        for route in &routes {
            let firsts: Vec<i64> = plan
                .deliveries()
                .iter()
                .filter(|d| d.route_id == route.id)
                .map(|d| d.outbox_ids[0])
                .collect();
            prop_assert!(firsts.windows(2).all(|w| w[0] < w[1]), "in the order it happened");
        }
    }
}

// ---------------------------------------------------------------------------
// 02-02: lane-filtered routes (DDR-19 rule 5, US-EIP-03 examples 4-5).
// ---------------------------------------------------------------------------

/// One open chat route on project 0 limited to `filter`: each chosen lane by
/// its id, with the slug the project has for it now, or `None` once gone.
fn lane_filtered_route(filter: &[Option<usize>]) -> RouteSpec {
    let lane_ids = filter
        .iter()
        .enumerate()
        .map(|(index, lane)| FilterLane {
            id: Uuid::from_u128(0x6000 + index as u128),
            slug: lane.map(|lane| LANES[lane].to_string()),
        })
        .collect();
    RouteSpec {
        lane_ids,
        ..chat_routes(1).remove(0)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A lane filter limits Issue moved and nothing else: every new issue,
    /// deletion and comment still reaches a lane-filtered route, whatever lane
    /// it is in; a move reaches it only when it lands in one of the chosen
    /// lanes and every chosen lane still exists; an empty filter takes every
    /// move.
    #[test]
    fn a_lane_filter_limits_only_moves_and_only_into_chosen_live_lanes(
        picks in proptest::collection::vec(
            (0usize..4, 0usize..LANES.len(), 0usize..LANES.len()), 1..12
        ),
        filter in proptest::collection::vec(
            proptest::option::weighted(0.8, 0usize..LANES.len()), 0..=LANES.len()
        ),
    ) {
        const TYPES: [&str; 4] = ["IssueCreated", "IssueUpdated", "IssueDeleted", "CommentAdded"];
        let rows: Vec<OutboxRow> = picks
            .iter()
            .enumerate()
            .map(|(index, (kind, from, to))| {
                // A move always changes lane: a reposition is its own property.
                let from = if TYPES[*kind] == "IssueUpdated" && from == to {
                    (to + 1) % LANES.len()
                } else {
                    *from
                };
                board_row(index, index as u64, TYPES[*kind], LANES[from], LANES[*to])
            })
            .collect();
        let route = lane_filtered_route(&filter);

        let plan = DispatchPlan::build(&rows, std::slice::from_ref(&route));

        let every_lane_lives = filter.iter().all(Option::is_some);
        let expected: BTreeSet<i64> = rows
            .iter()
            .zip(&picks)
            .filter(|(_, (kind, _, to))| {
                TYPES[*kind] != "IssueUpdated"
                    || filter.is_empty()
                    || (every_lane_lives && filter.contains(&Some(*to)))
            })
            .map(|(row, _)| row.id)
            .collect();
        let delivered: Vec<i64> = plan.deliveries().iter().map(|d| d.outbox_ids[0]).collect();
        prop_assert_eq!(delivered.len(), expected.len(), "no row twice");
        prop_assert_eq!(delivered.into_iter().collect::<BTreeSet<_>>(), expected);
    }
}

/// US-EIP-03 example 4: a renamed lane keeps its id, so the route follows it
/// by the slug the project has for it now; a move into another lane does not
/// reach the route.
#[test]
fn a_lane_filter_follows_its_lane_by_the_slug_it_has_now() {
    let done = LANES.iter().position(|lane| *lane == "done").expect("done");
    let route = lane_filtered_route(&[Some(done)]);
    let rows = [
        board_row(0, 1, "IssueUpdated", "in-progress", "done"),
        board_row(1, 2, "IssueUpdated", "done", "todo"),
        board_row(2, 3, "IssueCreated", "todo", "todo"),
    ];

    let plan = DispatchPlan::build(&rows, &[route]);

    let delivered: Vec<(i64, EventKind)> = plan
        .deliveries()
        .iter()
        .map(|d| (d.outbox_ids[0], d.event_kind))
        .collect();
    assert_eq!(
        delivered,
        vec![(100, EventKind::IssueMoved), (102, EventKind::IssueCreated)]
    );
}

/// US-EIP-03 example 5: a route with a gone lane posts no move, even into a
/// lane it still names, until it is edited; its other events still arrive.
#[test]
fn a_lane_filter_with_a_gone_lane_posts_no_move() {
    let done = LANES.iter().position(|lane| *lane == "done").expect("done");
    let route = lane_filtered_route(&[None, Some(done)]);
    let rows = [
        board_row(0, 1, "IssueUpdated", "in-progress", "done"),
        board_row(1, 2, "CommentAdded", "done", "done"),
    ];

    let plan = DispatchPlan::build(&rows, &[route]);

    let delivered: Vec<(i64, EventKind)> = plan
        .deliveries()
        .iter()
        .map(|d| (d.outbox_ids[0], d.event_kind))
        .collect();
    assert_eq!(delivered, vec![(101, EventKind::CommentAdded)]);
}

// ---------------------------------------------------------------------------
// 02-03: compose (D-3) and the Slack encoder (DDR-12).
// ---------------------------------------------------------------------------

const EVENT_KINDS: [EventKind; 6] = [
    EventKind::IssueCreated,
    EventKind::IssueMoved,
    EventKind::IssueDeleted,
    EventKind::CommentAdded,
    EventKind::CommentEdited,
    EventKind::CommentDeleted,
];

const LEVELS: [ContentLevel; 3] = [
    ContentLevel::LinkOnly,
    ContentLevel::Summary,
    ContentLevel::SummaryWithComment,
];

/// Text a person may type: letters, digits, spaces, punctuation, Slack's
/// control characters and their mention markers, and a few non-ASCII ones.
fn people_text(max: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::char::range('a', 'z'),
            1 => proptest::char::range('A', 'Z'),
            1 => proptest::sample::select(vec![
                ' ', '<', '>', '&', '!', '@', '#', '^', '|', '*', '_', '~', '`', ':', '.', ',',
                'é', '…', '✓', '\n',
            ]),
            1 => proptest::sample::select(vec!['U', 'C', 'S', 'W']),
        ],
        0..=max,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// One issue as Foundry holds it, with everything a message must never carry
/// beside what it may: the never-sent parts are drawn from an alphabet
/// (digits and `§`) people's text above never uses, so finding one in a
/// message can only mean it leaked.
#[derive(Debug, Clone)]
struct HeldIssue {
    view: EventView,
    description: String,
    attachment: String,
    author_email: String,
    ids: Vec<Uuid>,
}

fn held_issue() -> impl Strategy<Value = HeldIssue> {
    (
        0usize..EVENT_KINDS.len(),
        people_text(40),
        people_text(400),
        people_text(20),
        people_text(12),
        1i32..10_000,
        1usize..9,
        any::<u128>(),
        "§[0-9]{6,12}",
        "§[0-9]{4,8}\\.env",
    )
        .prop_map(
            |(kind, title, comment, actor, lane, number, count, seed, description, attachment)| {
                let ids: Vec<Uuid> = (0..3)
                    .map(|n| Uuid::from_u128(seed.wrapping_add(n)))
                    .collect();
                HeldIssue {
                    view: EventView {
                        kind: EVENT_KINDS[kind],
                        issue_key: format!("AUTH-{number}"),
                        title,
                        project_name: "Identity Platform".into(),
                        actor_name: actor,
                        lane_label: lane.clone(),
                        from_lane_label: lane,
                        issue_url: issue_url(
                            "https://foundry.canzan.dev",
                            "backend",
                            "identity-platform",
                            number,
                        ),
                        // A burst, a fifth of the time (D-11).
                        issue_count: if count > 7 { count } else { 1 },
                        comment_text: comment,
                    },
                    description,
                    attachment,
                    author_email: format!("§{number}@canzan.dev"),
                    ids,
                }
            },
        )
}

/// Every string value an encoded message carries, wherever it sits.
fn strings_in(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|v| strings_in(v, out)),
        serde_json::Value::Object(map) => map.values().for_each(|v| strings_in(v, out)),
        _ => {}
    }
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// D-3: at every level, no description, attachment name, email address
    /// or internal id reaches a message; the issue key does (unless a burst
    /// names no one issue).
    #[test]
    fn compose_never_sends_description_attachment_email_or_internal_id(issue in held_issue()) {
        for level in LEVELS {
            let sent = encode_slack(&compose(&issue.view, level)).0.to_string();
            prop_assert!(!sent.contains('§'), "a never-sent part leaked at {level:?}: {sent}");
            prop_assert!(!sent.contains(&issue.description));
            prop_assert!(!sent.contains(&issue.attachment));
            prop_assert!(!sent.contains(&issue.author_email));
            for id in &issue.ids {
                prop_assert!(!sent.contains(&id.to_string()), "an id leaked: {sent}");
                prop_assert!(!sent.contains(&id.simple().to_string()), "an id leaked: {sent}");
            }
            if issue.view.issue_count == 1 {
                prop_assert!(sent.contains(&issue.view.issue_key), "the key is sent: {sent}");
            }
        }
    }

    /// D-3: a comment's text is sent only at Summary with comment text, for a
    /// single new comment, whole up to 300 characters and otherwise its first
    /// 300 followed by "…", as plain text.
    #[test]
    fn compose_cuts_comment_text_at_300_characters_with_ellipsis(
        issue in held_issue(),
        long in people_text(700),
    ) {
        let mut view = issue.view;
        view.comment_text = long;
        for level in LEVELS {
            let lines = compose(&view, level);
            let chosen = level == ContentLevel::SummaryWithComment
                && view.kind == EventKind::CommentAdded
                && view.issue_count == 1
                && !view.comment_text.is_empty();
            if !chosen {
                prop_assert_eq!(&lines.excerpt, &None, "{:?} {:?}", level, view.kind);
                continue;
            }
            let excerpt = lines.excerpt.clone().expect("an excerpt");
            let length = view.comment_text.chars().count();
            if length <= 300 {
                prop_assert_eq!(&excerpt, &view.comment_text);
            } else {
                let first: String = view.comment_text.chars().take(300).collect();
                prop_assert_eq!(excerpt.clone(), format!("{first}…"));
            }
            let message = encode_slack(&lines).0;
            let placed = message["blocks"]
                .as_array()
                .expect("blocks")
                .iter()
                .find(|block| block["text"]["text"].as_str().map(unescape) == Some(excerpt.clone()))
                .cloned();
            prop_assert!(placed.is_some(), "the excerpt is a block: {message}");
            prop_assert_eq!(&placed.unwrap()["text"]["type"], "plain_text");
        }
    }

    /// DDR-12: whatever people type, Slack is given no live `<!…>`, `<@…>`
    /// or `<#…>`: every text is plain_text (never mrkdwn) with `<`, `>` and
    /// `&` escaped, `parse: none` and `link_names: false`; and the text still
    /// reads as typed once Slack shows the escapes.
    #[test]
    fn encode_slack_never_leaves_a_live_mention(issue in held_issue()) {
        for level in LEVELS {
            let lines = compose(&issue.view, level);
            let message = encode_slack(&lines).0;
            prop_assert_eq!(&message["parse"], "none");
            prop_assert_eq!(&message["link_names"], false);
            prop_assert_eq!(&message["mrkdwn"], false);
            let mut strings = Vec::new();
            strings_in(&message, &mut strings);
            prop_assert!(!strings.iter().any(|s| s == "mrkdwn"), "no mrkdwn object: {message}");
            for text in strings.iter().filter(|s| !s.starts_with("https://")) {
                prop_assert!(!text.contains('<') && !text.contains('>'), "a live control sequence: {text:?}");
            }
            for typed in [Some(&lines.headline), lines.detail.as_ref(), lines.excerpt.as_ref()].into_iter().flatten() {
                prop_assert!(
                    strings.iter().any(|s| unescape(s) == *typed),
                    "{typed:?} reads as typed: {message}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 03-01: the retry policy, Slack's answers and the retention plan.
// ---------------------------------------------------------------------------

fn attempt() -> impl Strategy<Value = Attempt> {
    (1u32..=40, -100i64..=0, -400i16..=400).prop_map(|(number, started, jitter)| Attempt {
        number,
        started_at: at(started),
        jitter: Jitter::thousandths(jitter),
    })
}

fn transient(retry_after: Option<u64>) -> ChannelOutcome {
    ChannelOutcome::Transient {
        reason: ReasonCode::SlackNoAnswer,
        retry_after: retry_after.map(std::time::Duration::from_secs),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// DDR-5: whatever the attempt, its jitter and any "retry after", a retry
    /// is scheduled strictly before `give_up_at`, never before the answer, or
    /// the delivery gives up.
    #[test]
    fn retry_policy_never_schedules_past_give_up(
        attempt in attempt(),
        retry_after in proptest::option::of(0u64..=4000),
        give_up in -50i64..=3600,
    ) {
        let now = at(0);
        let give_up_at = at(give_up);
        match RetryPolicy::BACKOFF.settle(transient(retry_after), attempt, now, give_up_at) {
            Settlement::RetryAt { at: next, reason, .. } => {
                prop_assert!(next < give_up_at, "{next} is before {give_up_at}");
                prop_assert!(next >= attempt.started_at);
                prop_assert_eq!(reason, ReasonCode::SlackNoAnswer);
            }
            Settlement::GaveUp { vendor_reason } => {
                prop_assert_eq!(vendor_reason, ReasonCode::SlackNoAnswer);
                let next = RetryPolicy::BACKOFF.next(
                    attempt,
                    retry_after.map(std::time::Duration::from_secs),
                    now,
                    give_up_at,
                );
                prop_assert_eq!(next, None);
            }
            other => prop_assert!(false, "a transient answer is retried or given up: {other:?}"),
        }
    }

    /// A vendor's "retry after" longer than the backoff wins: the retry is
    /// that long after the answer; a shorter one leaves the backoff.
    #[test]
    fn retry_policy_honours_a_larger_retry_after(
        attempt in attempt(),
        retry_after in 0u64..=3600,
    ) {
        let now = at(0);
        let far = now + Duration::days(1);
        let wait = std::time::Duration::from_secs(retry_after);
        let backoff = attempt.started_at + RetryPolicy::BACKOFF.delay(attempt.number, attempt.jitter);
        let honoured = RetryPolicy::BACKOFF.next(attempt, Some(wait), now, far);
        prop_assert_eq!(honoured, Some(backoff.max(now + wait)));
        prop_assert!(honoured >= Some(now + wait));
        let slowed = ChannelOutcome::Transient {
            reason: ReasonCode::SlackSlowDown,
            retry_after: Some(wait),
        };
        prop_assert_eq!(
            RetryPolicy::BACKOFF.settle(slowed, attempt, now, far),
            Settlement::RetryAt {
                at: backoff.max(now + wait),
                reason: ReasonCode::SlackSlowDown,
                slowed_down: true,
                not_before: Some(now + wait),
            }
        );
    }

    /// One delivery's wait never shrinks as its attempts grow; it starts at
    /// 10 s, never passes 15 min, and its jitter stays within ±20%.
    #[test]
    fn retry_policy_is_monotone_in_attempts(
        attempt_number in 1u32..=60,
        jitter in -1000i16..=1000,
        delivery_id in any::<i64>(),
    ) {
        let policy = RetryPolicy::BACKOFF;
        for jitter in [Jitter::thousandths(jitter), Jitter::of(delivery_id)] {
            let this = policy.delay(attempt_number, jitter);
            let next = policy.delay(attempt_number + 1, jitter);
            prop_assert!(next >= this, "{next:?} after {this:?}");
            let bare = policy.delay(attempt_number, Jitter::NONE);
            prop_assert!(this.as_millis() * 10 >= bare.as_millis() * 8);
            prop_assert!(this.as_millis() * 10 <= bare.as_millis() * 12);
            prop_assert!(bare >= std::time::Duration::from_secs(10));
            prop_assert!(bare <= std::time::Duration::from_secs(15 * 60));
        }
        prop_assert_eq!(policy.delay(1, Jitter::NONE), std::time::Duration::from_secs(10));
        prop_assert_eq!(policy.delay(2, Jitter::NONE), std::time::Duration::from_secs(20));
        prop_assert_eq!(Jitter::of(delivery_id), Jitter::of(delivery_id));
    }
}

/// `(name, HTTP status, Retry-After seconds, body, outcome)`.
type AnswerRow = (
    &'static str,
    u16,
    Option<u64>,
    Option<serde_json::Value>,
    ChannelOutcome,
);

/// Slack's answers, one row per classifier class (ADR-EIP-005 decision 5).
/// Hand-written shapes (FOUNDRY_EIP_HANDWRITTEN_SLACK=1, the user's decision;
/// recorded fixtures, DEVOPS H-3, replace them before release).
#[test]
fn classify_slack_recorded_answers() {
    use serde_json::json;
    let slow = |s: Option<u64>| ChannelOutcome::Transient {
        reason: ReasonCode::SlackSlowDown,
        retry_after: s.map(std::time::Duration::from_secs),
    };
    let no_answer = transient(None);
    let route = |reason| ChannelOutcome::RouteRefused { reason };
    let connection = |reason| ChannelOutcome::ConnectionRefused { reason };
    let refused = |code: &str| json!({ "ok": false, "error": code });
    let delivered = json!({ "ok": true, "channel": "C0AUTHDEV", "ts": "1761055320.000100" });
    let rows: Vec<AnswerRow> = vec![
        (
            "posted",
            200,
            None,
            Some(delivered),
            ChannelOutcome::Delivered,
        ),
        (
            "429 with Retry-After",
            429,
            Some(30),
            Some(refused("ratelimited")),
            slow(Some(30)),
        ),
        ("429 without Retry-After", 429, None, None, slow(None)),
        (
            "200 ok:false ratelimited",
            200,
            Some(5),
            Some(refused("ratelimited")),
            slow(Some(5)),
        ),
        (
            "archived",
            200,
            None,
            Some(refused("is_archived")),
            route(ReasonCode::ChannelArchived),
        ),
        (
            "channel archived",
            200,
            None,
            Some(refused("channel_is_archived")),
            route(ReasonCode::ChannelArchived),
        ),
        (
            "not in channel",
            200,
            None,
            Some(refused("not_in_channel")),
            route(ReasonCode::NotInChannel),
        ),
        (
            "channel not found",
            200,
            None,
            Some(refused("channel_not_found")),
            route(ReasonCode::ChannelNotFound),
        ),
        (
            "token revoked",
            200,
            None,
            Some(refused("token_revoked")),
            connection(ReasonCode::TokenRevoked),
        ),
        (
            "invalid auth",
            200,
            None,
            Some(refused("invalid_auth")),
            connection(ReasonCode::TokenRevoked),
        ),
        (
            "not authed",
            200,
            None,
            Some(refused("not_authed")),
            connection(ReasonCode::TokenRevoked),
        ),
        (
            "workspace inactive",
            200,
            None,
            Some(refused("account_inactive")),
            connection(ReasonCode::AppRemoved),
        ),
        (
            "team access not granted",
            200,
            None,
            Some(refused("team_access_not_granted")),
            connection(ReasonCode::AppRemoved),
        ),
        (
            "unknown code",
            200,
            None,
            Some(refused("eip_code_nobody_documented")),
            no_answer,
        ),
        (
            "ok:false without a code",
            200,
            None,
            Some(json!({ "ok": false })),
            no_answer,
        ),
        (
            "no ok field",
            200,
            None,
            Some(json!({ "channel": "C0AUTHDEV" })),
            no_answer,
        ),
        (
            "ok is not a boolean",
            200,
            None,
            Some(json!({ "ok": "true" })),
            no_answer,
        ),
        ("HTML 503 from a proxy", 503, None, None, no_answer),
        (
            "500 with ok:false",
            500,
            None,
            Some(refused("fatal_error")),
            no_answer,
        ),
        (
            "unknown code with raw text",
            200,
            None,
            Some(
                json!({ "ok": false, "error": "eip_code_nobody_documented", "warning": "Raw vendor text" }),
            ),
            no_answer,
        ),
    ];
    for (name, status, retry_after, body, want) in rows {
        let answer = SlackAnswer {
            status,
            retry_after: retry_after.map(std::time::Duration::from_secs),
            body: body.as_ref(),
        };
        assert_eq!(classify_slack(Some(answer)), want, "{name}");
    }
    assert_eq!(classify_slack(None), no_answer, "no answer at all");
}

fn retained_deliveries() -> impl Strategy<Value = Vec<RetainedDelivery>> {
    // Up to three routes; each delivery finished (an offset in days from now)
    // or pending; ids ascend with insertion, as BIGSERIAL does.
    proptest::collection::vec((0usize..3, proptest::option::of(-40i64..=0)), 0..260).prop_map(
        |rows| {
            let mut newer = [0u32; 3];
            let mut out: Vec<RetainedDelivery> = rows
                .iter()
                .enumerate()
                .rev()
                .map(|(index, (route, finished))| {
                    let d = RetainedDelivery {
                        id: 1 + index as i64,
                        finished_at: finished.map(|days| at(0) + Duration::days(days)),
                        newer_on_target: newer[*route],
                    };
                    newer[*route] += 1;
                    d
                })
                .collect();
            out.reverse();
            out
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// DD-10: exactly the finished deliveries at least 30 days old beyond
    /// their route's newest 100, and exactly the outbox rows dispatched at
    /// least 30 days ago that no pending delivery reads.
    #[test]
    fn prune_set_keeps_the_newest_100_and_referenced_outbox_rows(
        deliveries in retained_deliveries(),
        outbox in proptest::collection::vec(
            (proptest::option::of(-40i64..=0), any::<bool>()), 0..20
        ).prop_map(|rows| rows.into_iter().enumerate().map(|(index, (dispatched, referenced))| {
            RetainedOutboxRow {
                id: 1 + index as i64,
                dispatched_at: dispatched.map(|days| at(0) + Duration::days(days)),
                referenced_by_pending: referenced,
            }
        }).collect::<Vec<_>>()),
    ) {
        let now = at(0);
        let cutoff = now - Duration::days(30);
        let plan = prune_set(now, &deliveries, &outbox);

        let expected: BTreeSet<i64> = deliveries
            .iter()
            .filter(|d| d.finished_at.is_some_and(|f| f <= cutoff) && d.newer_on_target >= RETAINED_PER_ROUTE)
            .map(|d| d.id)
            .collect();
        let chosen: BTreeSet<i64> = plan.deliveries.iter().copied().collect();
        prop_assert_eq!(chosen.len(), plan.deliveries.len(), "no id twice");
        prop_assert_eq!(&chosen, &expected);
        for d in &deliveries {
            if d.finished_at.is_none() || d.newer_on_target < RETAINED_PER_ROUTE {
                prop_assert!(!chosen.contains(&d.id), "pending or newest-100 {} kept", d.id);
            }
        }

        let expected_outbox: BTreeSet<i64> = outbox
            .iter()
            .filter(|o| o.dispatched_at.is_some_and(|d| d <= cutoff) && !o.referenced_by_pending)
            .map(|o| o.id)
            .collect();
        let chosen_outbox: BTreeSet<i64> = plan.outbox.iter().copied().collect();
        prop_assert_eq!(chosen_outbox, expected_outbox);
        prop_assert_eq!(plan.heartbeats_before, now - Duration::days(7));
    }
}

/// The 30-day edge: a delivery finished exactly 30 days ago beyond the newest
/// 100 goes; one a second younger, or the 100th newest, stays.
#[test]
fn prune_set_cuts_at_exactly_30_days_and_the_100th_newest() {
    let now = at(0);
    let thirty = now - Duration::days(30);
    let d = |id, finished_at, newer_on_target| RetainedDelivery {
        id,
        finished_at: Some(finished_at),
        newer_on_target,
    };
    let plan = prune_set(
        now,
        &[
            d(1, thirty, 100),
            d(2, thirty + Duration::seconds(1), 100),
            d(3, thirty, 99),
        ],
        &[],
    );
    assert_eq!(plan.deliveries, vec![1]);
}

/// DDR-5's numbers exactly: 10 s doubling to a 15-minute cap, a jitter of
/// thousandths that scales the wait up or down by exactly that share, the
/// give-up time itself never scheduled, and per-delivery jitter that reaches
/// both ends of ±20%.
#[test]
fn retry_policy_waits_are_ddr5s_exactly() {
    let secs = std::time::Duration::from_secs;
    let millis = std::time::Duration::from_millis;
    let policy = RetryPolicy::BACKOFF;
    let waits: Vec<_> = (1..=9).map(|n| policy.delay(n, Jitter::NONE)).collect();
    assert_eq!(
        waits,
        [10, 20, 40, 80, 160, 320, 640, 900, 900].map(secs).to_vec()
    );
    assert_eq!(policy.delay(1_000, Jitter::NONE), secs(900));
    assert_eq!(policy.delay(1, Jitter::thousandths(150)), millis(11_500));
    assert_eq!(policy.delay(1, Jitter::thousandths(-150)), millis(8_500));
    assert_eq!(policy.delay(1, Jitter::thousandths(-900)), secs(8));
    assert_eq!(policy.delay(1, Jitter::thousandths(900)), secs(12));

    let started = at(0);
    let attempt = Attempt {
        number: 1,
        started_at: started,
        jitter: Jitter::NONE,
    };
    let due = started + Duration::seconds(10);
    assert_eq!(
        policy.next(attempt, None, started, due),
        None,
        "never at give-up"
    );
    assert_eq!(
        policy.next(attempt, None, started, due + Duration::seconds(1)),
        Some(due)
    );

    let jitters: BTreeSet<i16> = (0..20_000i64)
        .map(|id| match Jitter::of(id) {
            j if j == Jitter::thousandths(-200) => -200,
            j if j == Jitter::thousandths(200) => 200,
            j if j == Jitter::NONE => 0,
            _ => 1,
        })
        .collect();
    assert_eq!(
        jitters,
        BTreeSet::from([-200, 0, 1, 200]),
        "both ends, none and the values between"
    );
    let shares: BTreeSet<std::time::Duration> = (0..2_000i64)
        .map(|id| policy.delay(1, Jitter::of(id)))
        .collect();
    assert!(
        shares.len() > 300,
        "jitter spreads deliveries: {}",
        shares.len()
    );
    assert!(shares.iter().any(|d| *d < secs(10)) && shares.iter().any(|d| *d > secs(10)));
}

// ---------------------------------------------------------------------------
// 04-01: the outbound address judge (DDR-14, ADR-EIP-006 decision 6). The
// ranges below are the ADR's own list, written out here as the oracle.
// ---------------------------------------------------------------------------

/// Always refused, whatever the allow-list says: (network, prefix length).
const ALWAYS_REFUSED: [([u8; 4], u8); 9] = [
    ([0, 0, 0, 0], 8),
    ([127, 0, 0, 0], 8),
    ([169, 254, 0, 0], 16),
    ([192, 0, 2, 0], 24),
    ([198, 51, 100, 0], 24),
    ([203, 0, 113, 0], 24),
    ([198, 18, 0, 0], 15),
    ([224, 0, 0, 0], 4),
    ([240, 0, 0, 0], 4),
];
/// Refused unless allow-listed: RFC 1918 and CGNAT.
const PRIVATE: [([u8; 4], u8); 4] = [
    ([10, 0, 0, 0], 8),
    ([172, 16, 0, 0], 12),
    ([192, 168, 0, 0], 16),
    ([100, 64, 0, 0], 10),
];

fn within(range: ([u8; 4], u8), host_bits: u32) -> Ipv4Addr {
    let (net, len) = range;
    let mask = if len == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(len))
    };
    Ipv4Addr::from((u32::from(Ipv4Addr::from(net)) & mask) | (host_bits & !mask))
}

fn in_any(ranges: &[([u8; 4], u8)], ip: Ipv4Addr) -> bool {
    ranges
        .iter()
        .any(|&range| within(range, u32::from(ip)) == ip)
}

fn v4_in(ranges: &'static [([u8; 4], u8)]) -> impl Strategy<Value = Ipv4Addr> {
    (0..ranges.len(), any::<u32>()).prop_map(move |(i, bits)| within(ranges[i], bits))
}

fn public_v4() -> impl Strategy<Value = Ipv4Addr> {
    any::<u32>()
        .prop_map(Ipv4Addr::from)
        .prop_filter("public", |ip| {
            !in_any(&ALWAYS_REFUSED, *ip) && !in_any(&PRIVATE, *ip)
        })
}

/// Every IPv6 form that embeds `v4`, by name.
fn embeddings(v4: Ipv4Addr) -> Vec<(&'static str, Ipv6Addr)> {
    let [a, b, c, d] = v4.octets();
    let hi = u16::from_be_bytes([a, b]);
    let lo = u16::from_be_bytes([c, d]);
    vec![
        ("IPv4-mapped", v4.to_ipv6_mapped()),
        ("IPv4-compatible", Ipv6Addr::new(0, 0, 0, 0, 0, 0, hi, lo)),
        ("NAT64", Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, hi, lo)),
        (
            "NAT64 local",
            Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, hi, lo),
        ),
        ("6to4", Ipv6Addr::new(0x2002, hi, lo, 0, 0, 0, 0, 1)),
    ]
}

/// The spellings of `v4` a browser reads as that address.
fn literals(v4: Ipv4Addr) -> Vec<String> {
    let [a, b, c, d] = v4.octets();
    let whole = u32::from(v4);
    vec![
        v4.to_string(),
        whole.to_string(),
        format!("0x{whole:x}"),
        format!("0{a:o}.0{b:o}.0{c:o}.0{d:o}"),
        format!("0x{a:x}.{b}.0x{c:X}.{d}"),
        format!("{a}.{}", whole & 0x00ff_ffff),
        format!("{a}.{b}.{}", whole & 0xffff),
        format!("{v4}."),
    ]
}

fn everything() -> OutboundAllow {
    OutboundAllow::parse("0.0.0.0/0, ::/0, matrix.canzan.dev").expect("allow-list")
}

/// Carries the rows DISTILL moved out of acceptance (B2: link-local
/// 169.254.169.254, private 10.0.0.5, port-less https://10.0.0.5): nothing
/// listens there, so only the judge can pin them.
#[test]
fn judge_refuses_every_reserved_range_and_mapped_form() {
    rows_moved_out_of_acceptance();
    proptest!(ProptestConfig::with_cases(512), |(
        reserved in v4_in(&ALWAYS_REFUSED),
        private in v4_in(&PRIVATE),
        public in public_v4(),
        ula_bits in any::<u128>(),
        global in (any::<[u16; 8]>(), any::<bool>()),
    )| {
        let none = OutboundAllow::default();
        let refused = Verdict::Refuse(Refusal::Reserved);
        let needs_listing = Verdict::Refuse(Refusal::Private);
        // Always refused, even when every range and the host are listed.
        for allow in [&none, &everything()] {
            prop_assert_eq!(judge(reserved.into(), allow), refused, "{}", reserved);
            prop_assert_eq!(judge_host("matrix.canzan.dev", reserved.into(), allow), refused);
            for (form, v6) in embeddings(reserved) {
                prop_assert_eq!(judge(v6.into(), allow), refused, "{} {}", form, v6);
            }
        }
        // Private: refused unless a listed range holds it or the listed host
        // resolved to it; the embedded forms are judged the same way.
        let own = OutboundAllow::parse(&format!("{private}/32")).expect("own range");
        let by_name = OutboundAllow::parse("Matrix.Canzan.Dev.").expect("host");
        prop_assert_eq!(judge(private.into(), &none), needs_listing, "{}", private);
        prop_assert_eq!(judge(private.into(), &by_name), needs_listing);
        prop_assert_eq!(judge_host("elsewhere.canzan.dev", private.into(), &by_name), needs_listing);
        prop_assert_eq!(judge_host("matrix.canzan.dev", private.into(), &by_name), Verdict::Allow);
        prop_assert_eq!(judge(private.into(), &own), Verdict::Allow);
        prop_assert_eq!(judge(private.into(), &everything()), Verdict::Allow);
        for (form, v6) in embeddings(private) {
            prop_assert_eq!(judge(v6.into(), &none), needs_listing, "{} {}", form, v6);
            prop_assert_eq!(judge(v6.into(), &own), Verdict::Allow, "{} {}", form, v6);
        }
        // Public addresses, and their embedded forms, need no listing.
        prop_assert_eq!(judge(public.into(), &none), Verdict::Allow, "{}", public);
        for (form, v6) in embeddings(public) {
            prop_assert_eq!(judge(v6.into(), &none), Verdict::Allow, "{} {}", form, v6);
        }
        // A global IPv6 address embeds nothing, even with ffff or zeros where
        // a mapped address has them.
        let (mut segments, ffff) = global;
        segments[0] = 0x2400 | (segments[0] & 0x0fff);
        if ffff {
            segments[5] = 0xffff;
            segments[..5].iter_mut().skip(1).for_each(|s| *s = 0);
        }
        let global = Ipv6Addr::from(segments);
        prop_assert_eq!(judge(global.into(), &none), Verdict::Allow, "{}", global);
        // IPv6's own ranges: link-local, multicast, documentation always;
        // unique-local only by listing; the metadata address never.
        let tail = |prefix: u128, len: u32| {
            Ipv6Addr::from((prefix & !(u128::MAX >> len)) | (ula_bits & (u128::MAX >> len)))
        };
        for (net, len) in [(0xfe80u128 << 112, 10), (0xff00 << 112, 8), (0x2001_0db8 << 96, 32)] {
            let v6 = tail(net, len);
            prop_assert_eq!(judge(v6.into(), &everything()), refused, "{}", v6);
        }
        let ula = tail(0xfc00 << 112, 7);
        prop_assume!(ula != "fd00:ec2::254".parse::<Ipv6Addr>().expect("v6"));
        prop_assert_eq!(judge(ula.into(), &none), needs_listing, "{}", ula);
        prop_assert_eq!(judge(ula.into(), &everything()), Verdict::Allow, "{}", ula);
        let metadata: IpAddr = "fd00:ec2::254".parse().expect("v6");
        prop_assert_eq!(judge(metadata, &everything()), refused);
        prop_assert_eq!(judge(Ipv6Addr::LOCALHOST.into(), &everything()), refused);
        prop_assert_eq!(judge(Ipv6Addr::UNSPECIFIED.into(), &everything()), refused);
        // Every literal spelling reads as the address it names, so the judge
        // sees it; a port-less address is its scheme's port.
        for v4 in [reserved, private, public] {
            for literal in literals(v4) {
                let target = OutboundTarget::parse(&format!("https://{literal}/x?y#z"))
                    .expect("a literal target");
                prop_assert_eq!(&target.host, &TargetHost::Ip(v4.into()), "{}", literal);
                prop_assert_eq!(target.port, 443);
                prop_assert_eq!(target.verdict(&none), Some(judge(v4.into(), &none)));
            }
        }
    });
}

/// The moved rows, and the URL and allow-list reading the judge relies on.
fn rows_moved_out_of_acceptance() {
    let none = OutboundAllow::default();
    let ten = OutboundAllow::parse("10.0.0.0/8").expect("range");
    let target = |url: &str| OutboundTarget::parse(url).expect(url);
    assert_eq!(
        target("https://169.254.169.254/latest/meta-data/").verdict(&ten),
        Some(Verdict::Refuse(Refusal::Reserved))
    );
    assert_eq!(
        target("https://10.0.0.5:8448").verdict(&none),
        Some(Verdict::Refuse(Refusal::Private))
    );
    let port_less = target("https://10.0.0.5");
    assert_eq!(port_less.port, 443);
    assert_eq!(
        port_less.verdict(&none),
        Some(Verdict::Refuse(Refusal::Private))
    );
    assert_eq!(port_less.verdict(&ten), Some(Verdict::Allow));
    assert_eq!(target("HTTP://10.0.0.5").port, 80);
    assert_eq!(target("https://user:pw@10.0.0.5:8448/").port, 8448);
    assert_eq!(
        target("https://[::ffff:127.0.0.1]:8448/").host,
        TargetHost::Ip("::ffff:127.0.0.1".parse().expect("v6"))
    );
    assert_eq!(target("https://[::1]").port, 443);
    assert_eq!(
        target("https://Matrix.Canzan.Dev/_matrix"),
        OutboundTarget {
            host: TargetHost::Name("matrix.canzan.dev".into()),
            port: 443
        }
    );
    assert_eq!(target("https://matrix.canzan.dev").verdict(&none), None);
    for refused in [
        "ftp://10.0.0.5",
        "10.0.0.5",
        "https://",
        "https://:443",
        "https://10.0.0.5:http",
        "https://10.0.0.5:70000",
        "https://[::1",
        "https://[::1]x",
        "https://256.0.0.1",
        "https://1.2.3.4.5",
        "https://127.0.0.256.",
        "https://1.16777216",
        "https://exa_mple.com",
    ] {
        assert!(OutboundTarget::parse(refused).is_err(), "{refused}");
    }
    for malformed in [
        "10.0.0.0/33",
        "999.1.1.1",
        "not a host",
        "-bad.example",
        "10.0.0.0/8,,999.1.1.1/8",
    ] {
        assert!(OutboundAllow::parse(malformed).is_err(), "{malformed}");
    }
    let error = OutboundAllow::parse("10.0.0.0/8, x/y").expect_err("malformed");
    assert_eq!(error.position, 2);
    assert_eq!(
        error.to_string(),
        "FOUNDRY_OUTBOUND_ALLOW entry 2 is neither a CIDR range nor a host name"
    );
    assert_eq!(
        OutboundAllow::parse(" , ").expect("blank"),
        OutboundAllow::default()
    );
    let bare = OutboundAllow::parse("192.168.1.9").expect("bare address");
    assert_eq!(
        judge("192.168.1.9".parse().expect("v4"), &bare),
        Verdict::Allow
    );
    assert_eq!(
        judge("192.168.1.10".parse().expect("v4"), &bare),
        Verdict::Refuse(Refusal::Private)
    );
}

// ---------------------------------------------------------------------------
// 04-02: the Matrix encoder and classifier (DDR-13, ADR-EIP-006 decisions 4-5).
// ---------------------------------------------------------------------------

/// Text people type, with the ways to ping a Matrix room mixed in.
fn hostile_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            3 => people_text(12),
            1 => proptest::sample::select(vec![
                "@room".to_string(),
                "@lena:canzan.dev".to_string(),
                "<a href=\"https://matrix.to/#/@lena:canzan.dev\">Lena</a>".to_string(),
                "https://matrix.to/#/@lena:canzan.dev".to_string(),
                "<b>now</b>".to_string(),
                "\"quoted\" & 'single'".to_string(),
            ]),
        ],
        0..6,
    )
    .prop_map(|parts| parts.concat())
}

/// The HTML Foundry itself places: the bold headline, the line breaks and the
/// Open in Foundry link.
fn without_foundry_markup(formatted: &str, link: Option<&str>) -> String {
    let mut rest = formatted
        .replace("<strong>", "")
        .replace("</strong>", "")
        .replace("<br>", "");
    if let Some(url) = link {
        rest = rest.replace(&format!("<a href=\"{url}\">Open in Foundry</a>"), "");
    }
    rest
}

fn unescape_html(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// DDR-13: whatever people type, the notice mentions no one (`m.mentions`
    /// empty), carries no literal `@room` and no user pill, and its HTML has
    /// no markup but Foundry's own; and the text still reads as typed once
    /// the word joiners and escapes are taken out.
    #[test]
    fn encode_matrix_never_mentions_the_room_or_a_user(
        issue in held_issue(),
        title in hostile_text(),
        actor in hostile_text(),
        comment in hostile_text(),
    ) {
        let mut view = issue.view;
        view.title = title;
        view.actor_name = actor;
        view.comment_text = comment;
        for level in LEVELS {
            let lines: MessageLines = compose(&view, level);
            let message = encode_matrix(&lines).0;
            prop_assert_eq!(&message["msgtype"], "m.notice");
            prop_assert_eq!(&message["format"], "org.matrix.custom.html");
            prop_assert_eq!(&message["m.mentions"], &serde_json::json!({}));
            let body = message["body"].as_str().expect("a body");
            let formatted = message["formatted_body"].as_str().expect("a formatted body");
            for text in [body, formatted] {
                prop_assert!(!text.contains("@room"), "a literal @room: {text:?}");
                prop_assert!(!text.contains("matrix.to/#/@"), "a user pill: {text:?}");
                prop_assert!(!text.contains("@lena"), "a user id: {text:?}");
            }
            let rest = without_foundry_markup(formatted, lines.link.as_deref());
            prop_assert!(
                !rest.contains('<') && !rest.contains('>') && !rest.contains('"'),
                "unescaped HTML: {rest:?}"
            );
            prop_assert!(formatted.starts_with("<strong>"), "the headline is bold: {formatted}");
            let readable = body.replace('\u{2060}', "");
            let shown = unescape_html(&formatted.replace('\u{2060}', ""));
            for typed in [Some(&lines.headline), lines.detail.as_ref(), lines.excerpt.as_ref()].into_iter().flatten() {
                prop_assert!(readable.contains(typed.as_str()), "{typed:?} reads as typed in {readable:?}");
                prop_assert!(shown.contains(typed.as_str()), "{typed:?} shows as typed in {shown:?}");
            }
            if let Some(url) = &lines.link {
                prop_assert!(body.contains(url.as_str()) && formatted.contains(url.as_str()));
            }
        }
    }
}

/// ADR-EIP-006 decision 5, over recorded homeserver answers: success is
/// delivered; a slow-down waits its `retry_after_ms`; a logged-out token
/// loses the connection; a forbidden send stops the route; every other answer
/// is the homeserver not answering, retried.
#[test]
fn classify_matrix_recorded_answers() {
    use serde_json::json;
    use std::time::Duration as Wait;
    let retry = |after: Option<u64>| ChannelOutcome::Transient {
        reason: ReasonCode::HomeserverNoAnswer,
        retry_after: after.map(Wait::from_millis),
    };
    let table: Vec<(u16, Option<serde_json::Value>, ChannelOutcome)> = vec![
        (
            200,
            Some(json!({ "event_id": "$e" })),
            ChannelOutcome::Delivered,
        ),
        (200, None, ChannelOutcome::Delivered),
        (
            429,
            Some(json!({ "errcode": "M_LIMIT_EXCEEDED", "retry_after_ms": 30000 })),
            retry(Some(30_000)),
        ),
        (429, None, retry(None)),
        (
            400,
            Some(json!({ "errcode": "M_LIMIT_EXCEEDED", "retry_after_ms": 1500 })),
            retry(Some(1_500)),
        ),
        (
            401,
            Some(json!({ "errcode": "M_UNKNOWN_TOKEN" })),
            ChannelOutcome::ConnectionRefused {
                reason: ReasonCode::TokenRevoked,
            },
        ),
        (
            401,
            Some(json!({ "errcode": "M_MISSING_TOKEN" })),
            retry(None),
        ),
        (
            403,
            Some(json!({ "errcode": "M_FORBIDDEN" })),
            ChannelOutcome::RouteRefused {
                reason: ReasonCode::BotNotInRoom,
            },
        ),
        (
            403,
            Some(json!({ "errcode": "M_UNKNOWN_TOKEN" })),
            retry(None),
        ),
        (401, None, retry(None)),
        (403, None, retry(None)),
        (500, Some(json!({ "errcode": "M_UNKNOWN" })), retry(None)),
        (502, None, retry(None)),
        (404, Some(json!({ "errcode": "M_NOT_FOUND" })), retry(None)),
        (
            400,
            Some(json!({ "errcode": "M_SOMETHING_NEW" })),
            retry(None),
        ),
        (302, None, retry(None)),
    ];
    for (status, body, expected) in &table {
        let answer = MatrixAnswer {
            status: *status,
            body: body.as_ref(),
        };
        assert_eq!(
            classify_matrix(Some(answer)),
            *expected,
            "{status} {body:?}"
        );
    }
    assert_eq!(classify_matrix(None), retry(None), "no answer at all");
}
