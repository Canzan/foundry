//! external-integration-plugins — DELIVER review round 2 (the pre-release
//! safety set the user chose on 2026-10-25; feature-delta DISTILL DIST-33).
//! Crate-level tests, no Gherkin. Each RED case is ignored with
//! "RED until review fix R2-<x>" for the crafter to lift with the fix:
//!
//! `timeout 600 cargo test -p foundry-app --test eip_review_r2 -- --include-ignored`

use async_trait::async_trait;
use foundry_app::integrations::config::MIN_LEASE_SECONDS;
use foundry_app::{DeliveryError, Notification, NotificationProvider, Notifier, ProviderKind};
use foundry_integrations::OutboundEmail;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ===========================================================================
// R2-b: a project email's send never outlives the delivery's claim lease
// ===========================================================================

/// The send's share of the shortest lease an operator may configure
/// (`MIN_LEASE_SECONDS`: resolve 5 s + pre-send read 10 s + send 10 s, plus
/// 10 s spare). A send that runs longer can outlive the lease, and another
/// replica claims and sends the same email again.
const SEND_SHARE: Duration = Duration::from_secs(10);
/// What resolve, the pre-send read and the spare take of the lease.
const REST_OF_THE_LEASE: Duration = Duration::from_secs(25);
/// Scheduling slack on a loaded machine.
const SLACK: Duration = Duration::from_millis(750);

/// An SMTP relay that accepts the connection and never answers.
struct HungRelay;

#[async_trait]
impl NotificationProvider for HungRelay {
    async fn deliver(&self, _notification: &Notification) -> Result<(), DeliveryError> {
        std::future::pending().await
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Smtp
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        Ok(())
    }
}

fn project_email() -> OutboundEmail {
    OutboundEmail {
        recipient: "sam.okafor@canzan.dev".to_string(),
        subject: "[AUTH-7] Lena Park commented: Move Grafana sign-in to Keycloak".to_string(),
        text_body: "The new bind DN is live on staging.".to_string(),
        unsubscribe: Some((
            "https://foundry.canzan.dev/email/stop/one-click?t=x&sig=y".to_string(),
            "List-Unsubscribe=One-Click".to_string(),
        )),
    }
}

/// R2-b (review part 2, non-blocking: email lease vs
/// `NOTIFICATION_DELIVERY_TIMEOUT_MS`). An operator sets the notifier's
/// per-provider timeout to ten minutes, which account mail tolerates. A
/// project email through that notifier's transport to a relay that never
/// answers still gives up within the send's share of the shortest lease, so
/// the delivery is recorded (retried) under the claim that sent it.
/// RED today: the transport waits the whole notifier timeout.
///
/// The relation pinned: send bound + the rest of the claim-to-record path
/// stays within `MIN_LEASE_SECONDS`, for any notifier timeout.
///
/// CONTRACT_SHAPE: bounded-change
#[tokio::test]
async fn a_project_email_send_gives_up_within_the_leases_send_share() {
    assert_eq!(
        SEND_SHARE + REST_OF_THE_LEASE,
        Duration::from_secs(MIN_LEASE_SECONDS),
        "precondition: the lease budget the send share comes from"
    );
    let notifier =
        Notifier::new(vec![Arc::new(HungRelay)]).with_delivery_timeout(Duration::from_secs(600));
    let transport = notifier
        .email_transport()
        .expect("an smtp provider sends project email");

    let started = Instant::now();
    let sent = tokio::time::timeout(SEND_SHARE + SLACK, transport.send(&project_email())).await;
    let took = started.elapsed();

    let answer = sent.unwrap_or_else(|_| {
        panic!(
            "the project email's send was still waiting after {took:?}: a send may take at most \
             {SEND_SHARE:?} of the {MIN_LEASE_SECONDS} s lease"
        )
    });
    assert!(
        answer.is_err(),
        "a relay that never answers is not a delivered email"
    );
    assert!(
        took + REST_OF_THE_LEASE < Duration::from_secs(MIN_LEASE_SECONDS) + SLACK,
        "the send took {took:?}, leaving under {REST_OF_THE_LEASE:?} of the lease"
    );
}

// ===========================================================================
// R2-c: the Slack app's HTTP client is never a default client
// ===========================================================================

/// R2-c (review part 2, non-blocking: `SlackApp::new` builds its client with
/// `.build().unwrap_or_default()`). A client that cannot be built must stop
/// startup with an error, never fall back to `reqwest::Client::default()`,
/// which follows redirects and has no timeout: Slack calls would then follow
/// a redirect off slack.com and hang without bound.
///
/// Not behaviourally reachable: the builder's options are fixed and the TLS
/// backend (rustls with compiled-in roots) reads no setting, so no test can
/// make `build()` fail without a product seam. This pins the source instead:
/// the client build in `slack.rs` has no default fallback. RED today.
///
/// CONTRACT_SHAPE: pure-function
#[test]
fn the_slack_apps_client_never_falls_back_to_a_default_client() {
    let source = include_str!("../src/integrations/slack.rs");
    let builds: Vec<&str> = source
        .match_indices("reqwest::Client::builder()")
        .map(|(at, _)| {
            let rest = &source[at..];
            &rest[..rest.find(';').unwrap_or(rest.len())]
        })
        .collect();
    assert!(
        !builds.is_empty(),
        "precondition: slack.rs builds its own client"
    );
    for build in builds {
        let compact: String = build.split_whitespace().collect();
        assert!(
            !compact.contains(".build().unwrap_or_default()")
                && !compact.contains(".build().unwrap_or_else(")
                && !compact.contains(".build().unwrap_or("),
            "the Slack app's client falls back to a default client when it cannot be built:\n{build}"
        );
    }
}
