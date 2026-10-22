//! external-integration-plugins — the pure core's property and table tests
//! (DESIGN [REF] Driven Ports and Contract Shapes; brief "Properties for layer
//! 1-2"; DEVOPS `eip-core-unit`). Pending stubs written by DISTILL 2026-10-21
//! (ADR-025 scaffolded RED): each is `#[ignore]`d with its DELIVER step id and
//! panics with the RED marker until DELIVER replaces its body with a proptest
//! (`proptest = "1"`, already locked) or a table test.

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
    dispatch_plan_never_matches_another_project,
    "01-04",
    "proptest: rows of another project never become deliveries"
);
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
// route status (DDR-19).
pending!(
    route_status_derive_precedence,
    "01-06",
    "table: every precedence pair of DDR-19's eight rules"
);
// plan functions (claim and sweeps).
pending!(
    claimable_head_takes_the_head_only_without_a_live_lease,
    "01-04",
    "proptest: only the route's oldest non-terminal delivery, only without a live lease"
);
pending!(
    expiry_set_is_exactly_past_give_up_and_unleased,
    "01-04",
    "proptest: exactly the deliveries past give_up_at and not leased"
);
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
