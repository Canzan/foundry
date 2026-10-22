//! external-integration-plugins — foundry-app gold tests below the acceptance
//! lane (DESIGN [REF] Driven Ports and Contract Shapes, check-arch rule 5,
//! Earned Trust; DEVOPS C-7, C-13). Pending stubs written by DISTILL 2026-10-21
//! (ADR-025 scaffolded RED): each is `#[ignore]`d with its DELIVER step id and
//! panics with the RED marker until DELIVER replaces its body (through
//! `foundry_app::build_router`, like `api_docs_page.rs`).

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

// Handlers (slices 01-03).
pending!(
    preview_calls_no_port_and_writes_nothing,
    "02-02",
    "DDR-25: routes/preview re-renders compose output; no vendor call, no integration_* row"
);
pending!(
    send_test_posts_once_and_writes_no_row,
    "01-06",
    "DDR-25: routes/test sends exactly one K-7 message and changes no integration_* row"
);
pending!(
    seventh_send_test_in_a_minute_is_refused_without_a_vendor_call,
    "01-06",
    "6 per minute per user (the RevokeRateLimiter shape)"
);
pending!(
    save_never_sends,
    "01-06",
    "routes POST writes the route and its audit row and calls no DeliveryChannel"
);
pending!(
    refused_destination_check_writes_nothing,
    "01-06",
    "route create: a refused catalogue check or enrolment writes no route, lane or audit row"
);
pending!(
    reconnect_to_another_slack_team_changes_nothing,
    "03-02",
    "connection upsert: a different team id on the return leaves the row and audit untouched"
);
pending!(
    audit_rows_carry_no_secret,
    "01-05",
    "D-16, SC-3: no integration_audit_events column holds a token, code or sealed value"
);
pending!(csrf_exempt_set_is_exactly_three_paths, "03-02", "check-arch rule 5: is_exempt_path is /bootstrap, /integrations/slack/events, /email/stop/one-click and nothing else");
pending!(connection_lost_event_is_registered_at_zero, "03-02", "DDR-23: NotificationEvent::IntegrationConnectionLost is suppressible and its series register at 0");

// Outbound address policy (slice 04).
pending!(
    outbound_refuses_a_name_that_rebinds_to_loopback,
    "04-01",
    "DNS rebinding: the judging resolver refuses at connect time"
);
pending!(
    outbound_ignores_an_https_proxy_pointed_at_metadata,
    "04-01",
    "no_proxy(): HTTPS_PROXY set to a forwarder for 169.254.169.254 is never used"
);
pending!(
    outbound_follows_no_redirect_to_metadata,
    "04-01",
    "redirect::Policy::none(): a 302 to 169.254.169.254 is an error, not a request"
);
pending!(
    outbound_without_the_test_seam_refuses_loopback,
    "04-01",
    "C-7: with the test-support seam off, a 127.0.0.1 homeserver is refused"
);
