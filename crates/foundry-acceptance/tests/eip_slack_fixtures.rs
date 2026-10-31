//! external-integration-plugins — the recorded Slack answers (DEVOPS H-3) held
//! against the code that reads them and the double that stands in for Slack.
//! Refresh the fixtures with `scripts/slack-record.sh`.
//!
//! Not recorded, because a real workspace will not produce them on demand:
//! `ratelimited`, `not_in_channel`, and an archived channel's answers where the
//! workspace has none. Those stay hand-written in the double.

use foundry_acceptance::support::slack_double::{self as sd, SlackDouble, SlackTeam};
use foundry_app::integrations::config::SlackSettings;
use foundry_app::integrations::slack::{SlackApp, SlackEvent};
use foundry_integrations::{classify_slack, ChannelOutcome, ReasonCode, SlackAnswer};
use secrecy::SecretString;
use serde_json::{json, Value};
use std::path::PathBuf;
use time::OffsetDateTime;

const REDIRECT_URI: &str = "http://127.0.0.1:9/return";

fn fixture(name: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slack");
    let text = std::fs::read_to_string(root.join(format!("{name}.json")))
        .unwrap_or_else(|err| panic!("{name}: {err}"));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn classified(name: &str) -> ChannelOutcome {
    let body = fixture(name);
    classify_slack(Some(SlackAnswer {
        status: 200,
        retry_after: None,
        body: Some(&body),
    }))
}

#[test]
fn every_recorded_refusal_means_what_the_classifier_says() {
    let token_gone = ChannelOutcome::ConnectionRefused {
        reason: ReasonCode::TokenRevoked,
    };
    let app_gone = ChannelOutcome::ConnectionRefused {
        reason: ReasonCode::AppRemoved,
    };
    let no_channel = ChannelOutcome::RouteRefused {
        reason: ReasonCode::ChannelNotFound,
    };
    for (name, expected) in [
        ("chat.postMessage/ok", ChannelOutcome::Delivered),
        ("auth.test/ok", ChannelOutcome::Delivered),
        ("chat.postMessage/not_authed", token_gone),
        ("chat.postMessage/invalid_auth", token_gone),
        ("auth.test/not_authed", token_gone),
        ("auth.test/invalid_auth", token_gone),
        ("chat.postMessage/after_uninstall", app_gone),
        ("auth.test/after_uninstall", app_gone),
        ("chat.postMessage/channel_not_found", no_channel),
        ("conversations.info/channel_not_found", no_channel),
    ] {
        assert_eq!(classified(name), expected, "{name}");
    }
}

fn app() -> SlackApp {
    SlackApp::build(&SlackSettings {
        client_id: sd::CLIENT_ID.to_string(),
        client_secret: SecretString::from(sd::CLIENT_SECRET),
        signing_secret: SecretString::from(sd::SIGNING_SECRET),
        api_base_url: "http://127.0.0.1:9".to_string(),
        authorize_url: "http://127.0.0.1:9".to_string(),
    })
    .expect("the Slack app's client builds")
}

/// Foundry's events endpoint reading a recorded envelope, signed the way Slack
/// signs, since sanitising a fixture changes the bytes Slack signed.
fn read(name: &str) -> SlackEvent {
    let body = fixture(name).to_string();
    let now = OffsetDateTime::now_utc();
    let timestamp = now.unix_timestamp().to_string();
    let signature = sd::sign_v0(sd::SIGNING_SECRET, &timestamp, &body);
    app().read_event(Some(&timestamp), Some(&signature), body.as_bytes(), now)
}

#[test]
fn foundry_reads_the_events_slack_really_sent() {
    for name in ["events/app_uninstalled", "events/tokens_revoked"] {
        assert!(
            matches!(read(name), SlackEvent::AppLost { .. }),
            "{name} loses the connection"
        );
    }
    assert!(matches!(
        read("events/url_verification"),
        SlackEvent::UrlVerification { .. }
    ));
}

/// Keys the double answers with and Slack did not: `at` names the place.
fn invented_keys(double: &Value, recorded: &Value, at: &str) -> Vec<String> {
    let (Some(ours), Some(slacks)) = (double.as_object(), recorded.as_object()) else {
        return Vec::new();
    };
    ours.iter()
        .flat_map(|(key, value)| match slacks.get(key) {
            Some(theirs) => invented_keys(value, theirs, &format!("{at}.{key}")),
            None => vec![format!("{at}.{key}")],
        })
        .collect()
}

fn assert_same_shape(double: &Value, recorded: &Value, at: &str) {
    assert!(
        double.is_object() && recorded.is_object(),
        "{at}: both sides are answers, not a missing field"
    );
    assert_eq!(invented_keys(double, recorded, at), Vec::<String>::new());
}

#[test]
fn a_key_slack_never_sent_is_named_with_where_it_was() {
    let double = json!({ "ok": true, "team": { "id": "T", "slug": "s" }, "extra": 1 });
    let recorded = json!({ "ok": true, "team": { "id": "T" } });
    assert_eq!(
        invented_keys(&double, &recorded, "m"),
        vec!["m.extra".to_string(), "m.team.slug".to_string()]
    );
}

#[test]
#[should_panic(expected = "not a missing field")]
fn a_missing_field_is_not_a_matching_shape() {
    assert_same_shape(&Value::Null, &json!({}), "m");
}

#[tokio::test]
async fn the_doubles_answers_are_shaped_like_slacks() {
    let double = SlackDouble::spawn().await;
    let api = double.api_base_url();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("a client");
    let approved = http
        .get(double.authorize_url())
        .query(&[("redirect_uri", REDIRECT_URI), ("state", "s")])
        .send()
        .await
        .expect("the double's approval screen");
    let location = approved.headers()["location"].to_str().expect("a location");
    let code = location
        .split("code=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .expect("an approval carries a code");

    let answer = |request: reqwest::RequestBuilder| async move {
        let response = request.send().await.expect("the double answers");
        response.json::<Value>().await.expect("the double's JSON")
    };
    let installed = answer(
        http.post(format!("{api}/oauth.v2.access"))
            .form(&[("client_secret", sd::CLIENT_SECRET), ("code", code)]),
    )
    .await;
    assert_same_shape(
        &installed,
        &fixture("oauth.v2.access/ok"),
        "oauth.v2.access",
    );
    let token = installed["access_token"].as_str().expect("a token");

    let auth = answer(http.get(format!("{api}/auth.test")).bearer_auth(token)).await;
    assert_same_shape(&auth, &fixture("auth.test/ok"), "auth.test");

    let listed = answer(
        http.get(format!("{api}/conversations.list"))
            .bearer_auth(token),
    )
    .await;
    let recorded_list = fixture("conversations.list/ok");
    assert_same_shape(
        &listed["channels"][0],
        &recorded_list["channels"][0],
        "conversations.list.channels",
    );

    let channel = double.channel_id("auth-dev");
    let info = answer(
        http.get(format!("{api}/conversations.info"))
            .bearer_auth(token)
            .query(&[("channel", channel.as_str())]),
    )
    .await;
    assert_same_shape(
        &info,
        &fixture("conversations.info/ok"),
        "conversations.info",
    );

    let posted = answer(
        http.post(format!("{api}/chat.postMessage"))
            .bearer_auth(token)
            .json(&json!({ "channel": channel, "text": "hello" })),
    )
    .await;
    assert_same_shape(&posted, &fixture("chat.postMessage/ok"), "chat.postMessage");

    let uninstalled = answer(
        http.post(format!("{api}/apps.uninstall"))
            .bearer_auth(token),
    )
    .await;
    assert_same_shape(
        &uninstalled,
        &fixture("apps.uninstall/ok"),
        "apps.uninstall",
    );

    let team = SlackTeam::named("Canzan Labs");
    assert_same_shape(
        &sd::uninstall_envelope(&team, sd::APP_ID),
        &fixture("events/app_uninstalled"),
        "events/app_uninstalled",
    );
    assert_same_shape(
        &sd::url_verification_envelope("c"),
        &fixture("events/url_verification"),
        "events/url_verification",
    );
}
