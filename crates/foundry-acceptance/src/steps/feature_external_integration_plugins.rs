//! external-integration-plugins step definitions, slices 01-05
//! (`tests/features/external-integration-plugins-0{1..5}-*.feature`, DISTILL
//! 2026-10-21/22, every scenario `@pending`, ADR-025 scaffolded RED).
//!
//! Every step body is one call into `support::external_integration_plugins`
//! (the driver: this feature's vocabulary, doors, outside-world doubles,
//! universe and state-delta oracle), which builds on
//! `support::team_membership`. Reused unchanged from the team-membership step
//! file: "the answer is turned away as a forged form". No step holds business
//! logic, control flow or SQL of its own (Mandate 12, criterion 3): a step
//! parses its words into the driver's types and calls one driver function.
//!
//! Driving ports (DESIGN [REF] Driving Ports, deliverables A and B): the
//! workspace integrations page and its Slack doors, the public Slack events
//! address, the project's integrations pages and route doors, the account
//! settings' Project emails, the stop link and its one-click, `foundry doctor
//! rotate-secrets` and `restore-project` through the real binary, and the
//! shipped board doors as event sources. Layer 3 (real HTTP, real Postgres,
//! real binary, Slack as an HTTP double, Matrix as a pinned container):
//! example-only, sad paths enumerated (Mandates 9 and 11).
//!
//! Phrases are global across the suite (cucumber-rs has one registry), so every
//! regex is anchored and names something only this feature says (a channel
//! `#…`, Slack, Matrix, a route, an `AUTH-`/`OPS-` key, Project emails).

use crate::support::external_integration_plugins::{
    self as eip, BoardAction, EventChoice, HeldReturn, Kind, RouteAttempt, SlackAnswer,
    WorkspaceDoor,
};
use crate::support::slack_double::Mood;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use std::time::Duration;

fn list(words: &str) -> Vec<String> {
    words
        .replace(" and ", ", ")
        .split(", ")
        .map(|s| s.trim().to_string())
        .collect()
}

// =========================================================================== Background

#[given(regex = r"^the Canzan Labs workspace with these people$")]
async fn workspace_people(world: &mut FoundryWorld, step: &Step) {
    eip::given_workspace_people(world, step).await;
}

#[given(
    regex = r"^([A-Z][\w ]+) \(([A-Z]+)\) belongs to (\w+) and ([A-Z][\w ]+) \(([A-Z]+)\) belongs to (\w+)$"
)]
async fn projects(
    world: &mut FoundryWorld,
    a: String,
    ak: String,
    at: String,
    b: String,
    bk: String,
    bt: String,
) {
    eip::given_projects(world, &[(a, ak, at), (b, bk, bt)]).await;
}

#[given(regex = r"^Identity Platform has the lanes (.+)$")]
async fn lanes(world: &mut FoundryWorld, words: String) {
    eip::given_lanes(world, "Identity Platform", &words).await;
}

#[given(regex = r"^Identity Platform holds these issues$")]
async fn issues(world: &mut FoundryWorld, step: &Step) {
    eip::given_issues(world, "Identity Platform", step).await;
}

#[given(regex = r"^the next issue filed in (Identity Platform|Homelab Ops) becomes ([A-Z]+-\d+)$")]
async fn next_issue(world: &mut FoundryWorld, project: String, key: String) {
    eip::given_next_issue(world, &project, &key).await;
}

#[given(regex = r"^it is (\d\d:\d\d) on 21 Oct 2026$")]
async fn time_is(world: &mut FoundryWorld, hhmm: String) {
    eip::given_time(world, &hhmm);
}

#[given(
    regex = r"^the instance has (the Foundry Slack app set up|no Foundry Slack app set up|the Foundry Slack app set up but no secrets key)$"
)]
async fn instance_has(world: &mut FoundryWorld, setup: String) {
    eip::given_instance(world, &setup).await;
}

#[given(regex = r"^the instance (can|cannot) send email$")]
async fn instance_email(world: &mut FoundryWorld, can: String) {
    eip::given_email_setup(world, can == "can").await;
}

#[given(regex = r"^the instance sends email through its SMTP relay$")]
async fn smtp_relay(world: &mut FoundryWorld) {
    eip::given_smtp_relay(world).await;
}

// =========================================================================== Slack connection

#[given(regex = r"^Priya has connected Canzan Labs to Slack$")]
async fn slack_connected(world: &mut FoundryWorld) {
    eip::given_slack_connected(world).await;
}

#[given(regex = r"^Canzan Labs has no (?:Slack|Slack or Matrix) connection$")]
async fn no_connection(world: &mut FoundryWorld) {
    eip::given_no_connection(world).await;
}

#[when(
    regex = r#"^Priya (?:chooses Add to Slack|reconnects Slack) and approves Foundry for the Slack workspace "([^"]+)"$"#
)]
async fn approve(world: &mut FoundryWorld, team: String) {
    eip::when_connect_slack(world, "Priya", &team).await;
}

#[when(regex = r"^Priya chooses Add to Slack and cancels on Slack's approval screen$")]
async fn cancel(world: &mut FoundryWorld) {
    eip::when_cancel_slack(world, "Priya").await;
}

#[given(regex = r"^Priya holds a Slack return address with (.+)$")]
async fn held_return(world: &mut FoundryWorld, state: String) {
    eip::given_held_return(world, HeldReturn::parse(&state)).await;
}

#[when(regex = r"^Priya opens that Slack return address$")]
async fn open_held(world: &mut FoundryWorld) {
    eip::when_open_held_return(world).await;
}

#[then(regex = r"^Slack was never asked to exchange that return's code$")]
async fn never_exchanged(world: &mut FoundryWorld) {
    eip::then_code_never_exchanged(world);
}

#[when(
    regex = r"^(Lena|Marco|Priya|a signed-out visitor) opens (the workspace integrations page|the Slack setup page|Add to Slack|the Slack return address)$"
)]
async fn open_door(world: &mut FoundryWorld, who: String, door: String) {
    eip::when_open_door(world, &who, WorkspaceDoor::parse(&door)).await;
}

#[when(regex = r"^(Priya|Lena) opens (Identity Platform's integrations page)$")]
async fn open_page(world: &mut FoundryWorld, who: String, page: String) {
    eip::when_open(world, &who, &eip::page_path(world, &page)).await;
}

#[then(regex = r#"^Priya is back on the workspace integrations page and reads "([^"]+)"$"#)]
async fn back_on_page(world: &mut FoundryWorld, wanted: String) {
    eip::then_back_on_integrations(world, "Priya", &wanted).await;
}

#[then(regex = r#"^the workspace's integration activity reads "([^"]+)"$"#)]
async fn activity_reads(world: &mut FoundryWorld, line: String) {
    eip::then_activity_reads(world, &line).await;
}

#[then(regex = r#"^the workspace's integration activity gains the line "([^"]+)"$"#)]
async fn activity_gains(world: &mut FoundryWorld, line: String) {
    eip::then_activity_gains_line(world, &line).await;
}

#[then(regex = r"^the workspace's integration activity gains no line$")]
async fn activity_none(world: &mut FoundryWorld) {
    eip::then_activity_gains(world, 0, &[]).await;
}

#[then(
    regex = r"^nothing changed for Canzan Labs except its Slack connection and one activity line$"
)]
async fn only_connection(world: &mut FoundryWorld) {
    eip::then_only_connection_changed(world).await;
}

#[then(regex = r"^nothing changed for Canzan Labs$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    eip::then_nothing_changed(world).await;
}

#[then(regex = r#"^(Priya|Lena|Sam) reads "(.+)"$"#)]
async fn reads(world: &mut FoundryWorld, who: String, wanted: String) {
    eip::then_reads(world, &who, &wanted);
}

#[then(regex = r"^Priya is offered no Add to Slack$")]
async fn no_add(world: &mut FoundryWorld) {
    eip::then_add_to_slack(world, false).await;
}

#[then(regex = r"^Priya is offered Add to Slack on the workspace integrations page$")]
async fn add_offered(world: &mut FoundryWorld) {
    eip::then_add_to_slack(world, true).await;
}

#[then(
    regex = r"^(Lena|Marco|Sam|a signed-out visitor) gets the same not-found page any unknown address gets$"
)]
async fn not_found(world: &mut FoundryWorld, who: String) {
    eip::then_not_found(world, &who).await;
}

#[then(regex = r"^the visitor is sent to sign in, as from any page that needs a session$")]
async fn sent_to_sign_in(world: &mut FoundryWorld) {
    eip::then_sent_to_sign_in(world).await;
}

#[then(regex = r"^Priya still finds the workspace integrations page$")]
async fn priya_finds(world: &mut FoundryWorld) {
    eip::then_priya_still_finds(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;
}

#[then(
    regex = r"^the Slack app manifest names this instance's Slack return address and Slack events address$"
)]
async fn manifest(world: &mut FoundryWorld) {
    eip::then_manifest(world).await;
}

#[then(regex = r"^it asks Slack only for the bot permissions slices 01 to 03 need$")]
async fn scopes(world: &mut FoundryWorld) {
    eip::then_manifest_scopes(world, eip::BOT_SCOPES);
}

#[then(regex = r"^no Slack credential appears (?:on it|anywhere in what Priya or Lena receives)$")]
async fn no_credential(world: &mut FoundryWorld) {
    eip::then_no_credential(world);
}

// =========================================================================== Routes

#[given(
    regex = r"^(Lena|Priya) has routed (every chat event|New issue|Issue moved|New comment|Issue deleted)(?: into ([A-Z][\w ]*?))? on (Identity Platform|Homelab Ops) to (#[\w:.-]+)(?: with (Link only|Summary with comment text|Summary))?$"
)]
async fn routed(
    world: &mut FoundryWorld,
    who: String,
    events: String,
    lanes: String,
    project: String,
    dest: String,
    level: String,
) {
    eip::given_routed(
        world,
        &who,
        eip::route_from(&events, &lanes, &project, &dest, &level),
    )
    .await;
}

#[given(
    regex = r"^(Lena|Priya) is creating a (Slack|Matrix) route from (Identity Platform|Homelab Ops) to (#[\w:.-]+)(?: with (Link only|Summary with comment text|Summary|no event chosen))?$"
)]
async fn creating(
    world: &mut FoundryWorld,
    who: String,
    kind: String,
    project: String,
    dest: String,
    with: String,
) {
    eip::given_creating(
        world,
        &who,
        eip::draft_route(&project, Kind::parse(&kind), &dest, &with),
    )
    .await;
}

#[given(regex = r"^Lena (has ticked|left blank) the shared-channel confirmation$")]
async fn shared_tick(world: &mut FoundryWorld, ticked: String) {
    eip::draft_mut(world).shared_confirmed = ticked == "has ticked";
}

#[when(regex = r"^(Lena|Priya) saves the route$")]
async fn save_route(world: &mut FoundryWorld, who: String) {
    eip::when_save_route(world, &who).await;
}

#[when(regex = r"^Lena chooses Send test$")]
async fn send_test(world: &mut FoundryWorld) {
    eip::when_send_test(world, "Lena").await;
}

#[given(regex = r"^Lena has sent (\d+) test messages in the last minute$")]
async fn tests_sent(world: &mut FoundryWorld, n: usize) {
    eip::given_tests_sent(world, "Lena", n).await;
}

#[when(regex = r"^Lena previews the route$")]
async fn preview(world: &mut FoundryWorld) {
    eip::when_preview(world, "Lena").await;
}

#[when(
    regex = r"^Lena chooses (#[\w-]+) as the destination of a new Slack route on (Identity Platform)$"
)]
async fn choose_destination(world: &mut FoundryWorld, dest: String, project: String) {
    eip::when_choose_destination(world, "Lena", &dest, &project).await;
}

#[when(
    regex = r"^Lena chooses (#[\w-]+:[\w.]+) as the destination of a new Matrix route on (Identity Platform)$"
)]
async fn choose_room(world: &mut FoundryWorld, room: String, project: String) {
    eip::when_choose_room(world, "Lena", &room, &project).await;
}

#[when(regex = r"^Lena chooses New route on (Identity Platform)'s integrations page$")]
async fn new_route(world: &mut FoundryWorld, project: String) {
    eip::when_choose_new_route(world, "Lena", &project).await;
}

#[when(
    regex = r"^(Sam|Marco|Lena|a signed-out visitor) (opens the integrations page|opens a new Slack route|submits a new route to #[\w-]+) on (Identity Platform|Homelab Ops)$"
)]
async fn route_attempt(world: &mut FoundryWorld, who: String, attempt: String, project: String) {
    eip::when_route_attempt(world, &who, RouteAttempt::parse(&attempt), &project).await;
}

#[then(regex = r"^(Identity Platform|Homelab Ops) still has no route$")]
async fn no_route(world: &mut FoundryWorld, project: String) {
    eip::then_no_route(world, &project).await;
}

#[then(regex = r"^(Lena|Priya) still finds (Identity Platform|Homelab Ops)'s integrations page$")]
async fn owner_finds(world: &mut FoundryWorld, who: String, project: String) {
    eip::then_priya_still_finds(world, &who, &eip::integrations_path(world, &project)).await;
}

#[then(
    regex = r#"^(Identity Platform|Homelab Ops)'s integrations page lists the route to (#[\w-]+) reading "([^"]+)"$"#
)]
async fn lists_route(world: &mut FoundryWorld, project: String, dest: String, status: String) {
    eip::then_lists_route(world, &project, &dest, true, Some(&status)).await;
}

#[then(
    regex = r"^Identity Platform's integrations page (still lists|no longer lists) the route to (#[\w:.-]+)$"
)]
async fn still_lists(world: &mut FoundryWorld, how: String, dest: String) {
    eip::then_lists_route(
        world,
        "Identity Platform",
        &dest,
        how == "still lists",
        None,
    )
    .await;
}

#[then(regex = r"^Identity Platform (has|still has no) a route to (#[\w-]+)$")]
async fn has_route(world: &mut FoundryWorld, has: String, dest: String) {
    eip::then_lists_route(world, "Identity Platform", &dest, has == "has", None).await;
}

#[then(
    regex = r#"^(?:Lena|Priya)'s route to (#[\w:.-]+) reads "([^"]+)"( with the time of that delivery)?$"#
)]
async fn route_reads(world: &mut FoundryWorld, dest: String, status: String, timed: String) {
    eip::then_route_reads(world, &dest, &status, !timed.is_empty()).await;
}

#[then(regex = r#"^Lena's route to (#[\w:.-]+) reads a status starting "([^"]+)"$"#)]
async fn route_reads_prefix(world: &mut FoundryWorld, dest: String, prefix: String) {
    eip::then_route_reads_prefix(world, &dest, &prefix).await;
}

#[given(regex = r#"^Lena's route to (#[\w:.-]+) (?:needs attention|reads "([^"]+)")$"#)]
async fn given_route_reads(world: &mut FoundryWorld, dest: String, status: String) {
    eip::given_route_reads(world, &dest, &status).await;
}

#[then(regex = r#"^the routes to (.+) each read "([^"]+)"$"#)]
async fn routes_each(world: &mut FoundryWorld, dests: String, status: String) {
    eip::then_routes_each_read(world, &list(&dests), &status).await;
}

#[then(regex = r#"^Canzan Labs' Slack stays connected and every route reads "Delivering"$"#)]
async fn stays_connected(world: &mut FoundryWorld) {
    eip::then_connected_and_delivering(world).await;
}

#[then(regex = r"^the channel picker offers (#[\w-]+, #[\w-]+ and #[\w-]+)$")]
async fn picker_offers(world: &mut FoundryWorld, channels: String) {
    eip::then_picker(
        world,
        &list(&channels)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        true,
    );
}

#[then(regex = r"^the channel picker (offers|does not offer) (#[\w-]+)$")]
async fn picker_one(world: &mut FoundryWorld, offers: String, channel: String) {
    eip::then_picker(world, &[channel.as_str()], offers == "offers");
}

#[given(regex = r"^Foundry (is not in|has been invited to) the private channel (#[\w-]+)$")]
async fn private_membership(world: &mut FoundryWorld, membership: String, channel: String) {
    eip::slack_membership(world, &channel, membership == "has been invited to");
}

#[then(
    regex = r"^the route form offers the events New issue, Issue moved, New comment and Issue deleted, and no other$"
)]
async fn offers_events(world: &mut FoundryWorld) {
    eip::then_form_offers_events(world);
}

#[then(regex = r"^Issue moved offers the lanes (.+) to limit it to$")]
async fn offers_lanes(world: &mut FoundryWorld, lanes: String) {
    eip::then_form_offers_lanes(world, &lanes);
}

#[then(
    regex = r"^the route form offers Link only, Summary and Summary with comment text, with Summary chosen$"
)]
async fn offers_levels(world: &mut FoundryWorld) {
    eip::then_levels(world);
}

#[then(
    regex = r#"^the preview reads "([^"]+)" with Open in Foundry, and no title, actor or lane$"#
)]
async fn preview_link_only(world: &mut FoundryWorld, line: String) {
    eip::then_preview(
        world,
        &[&line],
        true,
        &["Rotate LLDAP", "Sam Okafor", "Backlog"],
    );
}

#[then(regex = r#"^the preview reads "([^"]+)" / "([^"]+)" with Open in Foundry$"#)]
async fn preview_summary(world: &mut FoundryWorld, a: String, b: String) {
    eip::then_preview(world, &[&a, &b], true, &[]);
}

#[then(regex = r"^Slack was asked nothing by the preview$")]
async fn preview_nothing(world: &mut FoundryWorld) {
    eip::then_preview_asked_nothing(world);
}

#[when(regex = r"^(Lena|Sam|Marco) (pauses|resumes) the route to (#[\w:.-]+)$")]
async fn pause_resume(world: &mut FoundryWorld, who: String, action: String, dest: String) {
    eip::when_route_action(world, &who, eip::verb(&action), &dest).await;
}

#[given(regex = r"^Lena has (paused|resumed) the route to (#[\w:.-]+)$")]
async fn has_paused(world: &mut FoundryWorld, action: String, dest: String) {
    eip::given_route_action(world, "Lena", eip::verb(&action), &dest).await;
}

#[when(regex = r"^(Lena|Sam) chooses Delete on the route to (#[\w:.-]+)$")]
async fn delete_dialog(world: &mut FoundryWorld, who: String, dest: String) {
    eip::when_delete_dialog(world, &who, &dest).await;
}

#[given(regex = r"^Lena has chosen Delete on the route to (#[\w:.-]+)$")]
async fn has_chosen_delete(world: &mut FoundryWorld, dest: String) {
    eip::when_delete_dialog(world, "Lena", &dest).await;
}

#[when(regex = r"^Lena confirms the route's delete$")]
async fn confirm_delete(world: &mut FoundryWorld) {
    eip::when_confirm_delete(world, "Lena").await;
}

#[when(regex = r"^Lena confirms a delete of the route to (#[\w-]+) from outside the page$")]
async fn confirm_delete_outside(world: &mut FoundryWorld, dest: String) {
    eip::when_route_action_without_token(world, "Lena", "delete", &dest).await;
}

#[when(regex = r"^Lena pauses the route to (#[\w-]+) from outside the page$")]
async fn pause_outside(world: &mut FoundryWorld, dest: String) {
    eip::when_route_action_without_token(world, "Lena", "pause", &dest).await;
}

#[given(regex = r"^Lena has added (New comment) to the route to (#[\w:.-]+)$")]
async fn added_event(world: &mut FoundryWorld, event: String, dest: String) {
    eip::given_edit_add_event(world, "Lena", EventChoice::parse(&event), &dest).await;
}

#[when(regex = r"^Lena adds (Issue deleted) to the route to (#[\w-]+)$")]
async fn adds_event(world: &mut FoundryWorld, event: String, dest: String) {
    eip::when_edit_add_event(world, "Lena", EventChoice::parse(&event), &dest).await;
}

#[when(regex = r"^Sam opens the edit form of the route to (#[\w-]+)$")]
async fn edit_form(world: &mut FoundryWorld, dest: String) {
    eip::when_edit_open(world, "Sam", &dest).await;
}

#[when(
    regex = r"^(Lena|Sam|Marco|a signed-out visitor) opens the history of the route to (#[\w:.-]+)$"
)]
async fn open_history(world: &mut FoundryWorld, who: String, dest: String) {
    eip::when_open_history(world, &who, &dest).await;
}

#[then(regex = r"^Lena still finds the history of the route to (#[\w-]+)$")]
async fn still_history(world: &mut FoundryWorld, dest: String) {
    eip::then_history_still_opens(world, &dest).await;
}

#[then(regex = r#"^the route's history shows "([^"]+)"$"#)]
async fn sees_history(world: &mut FoundryWorld, line: String) {
    eip::then_history_line(world, &line);
}

#[then(regex = r#"^the history of the route to (#[\w:.-]+) reads "([^"]+)"$"#)]
async fn history_reads(world: &mut FoundryWorld, dest: String, line: String) {
    eip::then_history_reads(world, &dest, &line).await;
}

#[then(
    regex = r"^the history of the route to (#[\w:.-]+) (still lists|no longer lists) (AUTH-\d+)$"
)]
async fn history_lists(world: &mut FoundryWorld, dest: String, how: String, key: String) {
    eip::then_history_lists(world, &dest, &key, how == "still lists").await;
}

#[then(regex = r"^the history of the route to (#[\w:.-]+) lists (AUTH-\d+) exactly once$")]
async fn history_once(world: &mut FoundryWorld, dest: String, key: String) {
    eip::then_history_lists_once(world, &dest, &key).await;
}

#[then(regex = r"^the history shows none of the raw Slack text and no Slack credential$")]
async fn history_clean(world: &mut FoundryWorld) {
    eip::then_history_clean(world);
}

// =========================================================================== Disconnect, events, broken connections

#[when(regex = r"^(Priya|Lena) chooses Disconnect Slack$")]
async fn disconnect_dialog(world: &mut FoundryWorld, who: String) {
    eip::when_disconnect_dialog(world, &who).await;
}

#[given(regex = r"^Priya has chosen Disconnect Slack$")]
async fn has_chosen_disconnect(world: &mut FoundryWorld) {
    eip::when_disconnect_dialog(world, "Priya").await;
}

#[when(regex = r"^Priya (?:confirms the disconnect|disconnects Slack)$")]
async fn confirm_disconnect(world: &mut FoundryWorld) {
    eip::when_disconnect(world, "Priya", true).await;
}

#[when(regex = r"^(Lena|Priya) confirms a disconnect of Slack$")]
async fn confirm_disconnect_as(world: &mut FoundryWorld, who: String) {
    eip::when_disconnect(world, &who, true).await;
}

#[when(regex = r"^(Lena|Priya) confirms a disconnect of Slack from outside the page$")]
async fn confirm_disconnect_outside(world: &mut FoundryWorld, who: String) {
    eip::when_disconnect(world, &who, false).await;
}

#[given(regex = r"^Priya has disconnected Slack$")]
async fn has_disconnected(world: &mut FoundryWorld) {
    eip::given_disconnected(world).await;
}

#[then(regex = r"^Slack was asked once to remove Foundry's access to Canzan Labs$")]
async fn uninstalled_once(world: &mut FoundryWorld) {
    eip::then_uninstalls(world, 1);
}

#[then(regex = r"^Slack was asked to remove no message from (#[\w-]+)$")]
async fn no_message_removed(world: &mut FoundryWorld, channel: String) {
    eip::then_no_removal(world, &channel);
}

#[given(
    regex = r"^the Foundry app has been removed from Canzan Labs' Slack without Foundry being told$"
)]
async fn app_removed(world: &mut FoundryWorld) {
    eip::given_app_removed(world);
}

#[when(regex = r"^Slack tells Foundry the Foundry app was uninstalled from Canzan Labs$")]
async fn uninstall_event(world: &mut FoundryWorld) {
    eip::when_slack_uninstall_event(world).await;
}

#[given(regex = r"^Slack has told Foundry the Foundry app was uninstalled from Canzan Labs$")]
async fn has_uninstall_event(world: &mut FoundryWorld) {
    eip::slack_uninstall_event(world).await;
}

#[when(regex = r"^(?:someone|Slack) posts (.+) to Foundry's Slack events address$")]
async fn post_event(world: &mut FoundryWorld, message: String) {
    eip::when_post_event(world, &message).await;
}

#[then(regex = r"^Foundry (refuses as unauthorised|acknowledges without acting on) the message$")]
async fn event_answer(world: &mut FoundryWorld, how: String) {
    eip::then_event_answer(world, &how);
}

#[when(regex = r"^Slack verifies Foundry's Slack events address$")]
async fn verifies(world: &mut FoundryWorld) {
    eip::when_url_verification(world).await;
}

#[then(regex = r"^Slack's confirmation of Foundry's events address succeeds$")]
async fn challenge(world: &mut FoundryWorld) {
    eip::then_challenge_answered(world);
}

#[then(
    regex = r#"^the workspace integrations page (?:reads|tells Priya) "([^"]+)"(?: and "([^"]+)")?$"#
)]
async fn workspace_page_reads(world: &mut FoundryWorld, a: String, b: String) {
    eip::then_page_reads(world, "Priya", eip::WORKSPACE_INTEGRATIONS, &[&a, &b]).await;
}

#[given(regex = r"^(#[\w-]+) has been archived in Slack$")]
async fn archived(world: &mut FoundryWorld, channel: String) {
    eip::slack_next_answer(world, &channel, SlackAnswer::Archived);
}

#[given(
    regex = r"^Foundry (has been removed from|has been invited back to|has still not been invited to) (#[\w-]+) in Slack$"
)]
async fn foundry_in_channel(world: &mut FoundryWorld, how: String, channel: String) {
    eip::slack_membership(world, &channel, how == "has been invited back to");
}

// =========================================================================== Slack moods and time

#[given(regex = r"^Slack does not answer$")]
async fn slack_silent(world: &mut FoundryWorld) {
    eip::slack_mood(world, Mood::Silent);
}

#[given(regex = r"^Slack holds Foundry's post without answering$")]
async fn slack_holds(world: &mut FoundryWorld) {
    eip::slack_mood(world, Mood::Held);
}

#[given(regex = r"^Slack answers again$")]
async fn slack_heals(world: &mut FoundryWorld) {
    eip::slack_heals(world);
}

#[when(regex = r"^Slack answers again$")]
async fn when_slack_heals(world: &mut FoundryWorld) {
    eip::when_slack_heals(world).await;
}

#[when(regex = r"^Slack answers again and a minute passes$")]
async fn when_slack_heals_and_a_minute_passes(world: &mut FoundryWorld) {
    eip::when_slack_heals_and_a_minute_passes(world).await;
}

#[given(regex = r"^Slack will answer Foundry's next post to (#[\w-]+) with (.+)$")]
async fn next_answer(world: &mut FoundryWorld, channel: String, answer: String) {
    eip::slack_next_answer(world, &channel, SlackAnswer::parse(&answer));
}

#[given(regex = r"^Slack asked Foundry to retry Sam's comment on (AUTH-\d+) after (\d+) seconds$")]
async fn rate_limited(world: &mut FoundryWorld, key: String, secs: u64) {
    eip::given_rate_limited_comment(world, &key, secs).await;
}

#[given(regex = r"^(\d+) (hours|minutes?|seconds|days) have passed(?: without Slack answering)?$")]
async fn time_passed(world: &mut FoundryWorld, n: u64, unit: String) {
    eip::advance(world, eip::duration(n, &unit)).await;
}

#[when(regex = r"^(\d+) (hours|minutes?|seconds) pass(?:es)?(?: without Slack answering)?$")]
async fn time_passes(world: &mut FoundryWorld, n: u64, unit: String) {
    eip::when_advance(world, eip::duration(n, &unit)).await;
}

#[then(regex = r"^Slack was asked to post (AUTH-\d+)'s message (once|twice|more than once)$")]
async fn attempts(world: &mut FoundryWorld, key: String, how: String) {
    eip::then_attempts(world, "#auth-dev", &key, &how).await;
}

// =========================================================================== Board actions

#[when(regex = r#"^(Sam|Lena) files "([^"]+)" in (Identity Platform|Homelab Ops)$"#)]
async fn files(world: &mut FoundryWorld, who: String, title: String, project: String) {
    eip::when_board(world, &who, BoardAction::File { project, title }).await;
}

#[given(
    regex = r#"^Sam has filed "([^"]+)" in (Identity Platform|Homelab Ops)( and #auth-dev has shown it)?$"#
)]
async fn has_filed(world: &mut FoundryWorld, title: String, project: String, shown: String) {
    eip::given_filed(world, "Sam", &project, &title, !shown.is_empty()).await;
}

#[given(
    regex = r#"^Sam has filed "([^"]+)" in Identity Platform with the description "([^"]+)" and the attachment "([^"]+)"$"#
)]
async fn filed_secrets(
    world: &mut FoundryWorld,
    title: String,
    description: String,
    attachment: String,
) {
    eip::file_with_secrets(world, "Sam", &title, &description, &attachment).await;
}

#[given(
    regex = r"^Sam has filed (\d+) issues in (Identity Platform) and #auth-dev has shown them$"
)]
async fn filed_many_shown(world: &mut FoundryWorld, n: usize, project: String) {
    eip::given_filed_many_shown(world, "Sam", n, &project).await;
}

#[when(regex = r"^(Sam|Lena) moves ((?:AUTH|OPS)-\d+) from [\w ]+ to ([A-Z][\w ]*)$")]
async fn moves(world: &mut FoundryWorld, who: String, key: String, lane: String) {
    eip::when_board(world, &who, BoardAction::Move { key, to_lane: lane }).await;
}

#[given(regex = r"^Sam has moved ((?:AUTH|OPS)-\d+) from [\w ]+ to ([A-Z][\w ]*)$")]
async fn has_moved(world: &mut FoundryWorld, key: String, lane: String) {
    eip::move_issue(world, "Sam", &key, &lane).await;
}

#[when(regex = r"^Sam moves (AUTH-\d+) above (AUTH-\d+) inside (\w+)$")]
async fn reorders(world: &mut FoundryWorld, key: String, above: String, lane: String) {
    eip::when_board(world, "Sam", BoardAction::Reorder { key, above, lane }).await;
}

#[when(regex = r#"^(Sam|Lena) comments "(.*)" on (AUTH-\d+)$"#)]
async fn comments(world: &mut FoundryWorld, who: String, body: String, key: String) {
    eip::when_board(world, &who, BoardAction::Comment { key, body }).await;
}

#[given(regex = r#"^Lena has commented "([^"]+)" on (AUTH-\d+) and #auth-dev has shown it$"#)]
async fn has_commented(world: &mut FoundryWorld, body: String, key: String) {
    eip::given_commented_and_shown(world, "Lena", &key, &body).await;
}

#[when(regex = r"^Lena (edits|deletes) that comment$")]
async fn change_comment(world: &mut FoundryWorld, how: String) {
    eip::when_change_comment(world, "Lena", how == "deletes").await;
}

#[when(regex = r"^Lena posts a (\d+)-character comment on (AUTH-\d+)$")]
async fn long_comment(world: &mut FoundryWorld, n: usize, key: String) {
    eip::when_board(
        world,
        "Lena",
        BoardAction::Comment {
            key,
            body: eip::comment_of_length(n),
        },
    )
    .await;
}

#[when(regex = r"^(Lena) deletes (AUTH-\d+)$")]
async fn deletes_issue(world: &mut FoundryWorld, who: String, key: String) {
    eip::when_board(world, &who, BoardAction::DeleteIssue { key }).await;
}

#[given(regex = r"^the lane (\w+) holds (\d+) cards$")]
async fn lane_cards(world: &mut FoundryWorld, lane: String, n: usize) {
    eip::given_lane_cards(world, &lane, n).await;
}

#[when(regex = r"^Lena deletes the lane (\w+)(?: and (its cards|moves its cards to (\w+)))?$")]
async fn deletes_lane(world: &mut FoundryWorld, lane: String, _fate: String, to: String) {
    eip::when_delete_lane(
        world,
        "Lena",
        &lane,
        Some(to.as_str()).filter(|t| !t.is_empty()),
    )
    .await;
}

#[given(regex = r"^Lena has deleted the lane (\w+)$")]
async fn has_deleted_lane(world: &mut FoundryWorld, lane: String) {
    eip::delete_lane(world, "Lena", &lane, None).await;
}

#[given(regex = r"^Lena has renamed the lane (\w+) to (\w+)$")]
async fn renamed_lane(world: &mut FoundryWorld, lane: String, to: String) {
    eip::rename_lane(world, "Lena", &lane, &to).await;
}

#[given(regex = r"^(Identity Platform) has been deleted by Lena$")]
async fn deleted_project(world: &mut FoundryWorld, project: String) {
    eip::delete_project(world, "Lena", &project).await;
}

#[given(regex = r"^the instance operator has restored (Identity Platform)$")]
async fn restored(world: &mut FoundryWorld, project: String) {
    eip::restore_project(world, &project).await;
}

#[when(regex = r"^the instance operator restores (Identity Platform)$")]
async fn restores(world: &mut FoundryWorld, project: String) {
    eip::when_restore(world, &project).await;
}

#[given(regex = r"^(Tomás) has been removed from (Backend) by Lena$")]
async fn removed(world: &mut FoundryWorld, who: String, team: String) {
    eip::remove_from_team(world, "Lena", &who, &team).await;
}

#[when(regex = r"^Sam files (\d+) issues in (Identity Platform) at once$")]
async fn file_many(world: &mut FoundryWorld, n: usize, project: String) {
    eip::file_many(world, "Sam", n, &project).await;
}

// =========================================================================== What channels show

#[then(
    regex = r#"^(#[\w-]+) shows within 10 seconds a Foundry message "([^"]+)" / "([^"]+)" (with Open in Foundry|without a link)$"#
)]
async fn shows_message(
    world: &mut FoundryWorld,
    channel: String,
    a: String,
    b: String,
    link: String,
) {
    eip::then_message(
        world,
        &channel,
        &a,
        &b,
        Some(link == "with Open in Foundry"),
    )
    .await;
}

#[then(regex = r#"^(#[\w-]+) shows exactly one message, "([^"]+)"(?: / "([^"]+)")?$"#)]
async fn exactly_one(world: &mut FoundryWorld, channel: String, a: String, b: String) {
    eip::then_exactly_one(
        world,
        &channel,
        &a,
        Some(b.as_str()).filter(|b| !b.is_empty()),
    )
    .await;
}

#[then(regex = r#"^(#[\w-]+) shows (\d+) messages, each "([^"]+)"$"#)]
async fn n_messages(world: &mut FoundryWorld, channel: String, n: usize, each: String) {
    eip::then_message_count(world, &channel, n, Some(&each)).await;
}

#[then(regex = r"^(#[\w-]+) shows ((?:AUTH|OPS)-\d+)'s message( exactly once)?$")]
async fn shows_key(world: &mut FoundryWorld, channel: String, key: String, once: String) {
    eip::then_shows_key_times(world, &channel, &key, !once.is_empty()).await;
}

#[then(regex = r"^#auth-dev shows each of (.+) exactly once$")]
async fn each_once(world: &mut FoundryWorld, keys: String) {
    eip::then_each_once(world, "#auth-dev", &list(&keys)).await;
}

#[then(regex = r"^#auth-dev shows (AUTH-\d+)'s message before (AUTH-\d+)'s message$")]
async fn key_order(world: &mut FoundryWorld, a: String, b: String) {
    eip::then_order(world, "#auth-dev", (&a, &a), (&b, &b), false).await;
}

#[then(
    regex = r"^#auth-dev shows (AUTH-\d+)'s New issue message before its Moved message(, each exactly once)?$"
)]
async fn created_before_moved(world: &mut FoundryWorld, key: String, once: String) {
    eip::then_order(
        world,
        "#auth-dev",
        (&key, "New issue"),
        (&key, "Moved"),
        !once.is_empty(),
    )
    .await;
}

#[then(regex = r#"^(#[\w-]+) shows (AUTH-\d+)'s comment message(?: (with|without) "([^"]+)")?$"#)]
async fn comment_message(
    world: &mut FoundryWorld,
    channel: String,
    key: String,
    with: String,
    text: String,
) {
    eip::then_comment_message_words(world, &channel, &key, &with, &text).await;
}

#[then(
    regex = r"^#auth-dev shows (the whole comment|the comment's first 300 characters followed by .….)$"
)]
async fn excerpt(world: &mut FoundryWorld, which: String) {
    eip::then_excerpt_of_when(world, "#auth-dev", which == "the whole comment").await;
}

#[then(regex = r"^#auth-dev shows that text without notifying anyone in the channel$")]
async fn no_ping(world: &mut FoundryWorld) {
    eip::then_no_ping(world, "#auth-dev").await;
}

#[then(regex = r#"^(#[\w-]+) shows "([^"]+)"$"#)]
async fn shows_text(world: &mut FoundryWorld, channel: String, text: String) {
    eip::then_shows_text(world, &channel, &text).await;
}

#[then(regex = r"^nothing is posted to (#[\w-]+) within 10 seconds$")]
async fn nothing_posted(world: &mut FoundryWorld, channel: String) {
    eip::then_nothing_posted(world, &channel).await;
}

#[then(regex = r"^(#[\w-]+) shows no further message within 10 seconds$")]
async fn no_further(world: &mut FoundryWorld, channel: String) {
    eip::then_nothing_posted(world, &channel).await;
}

#[then(regex = r"^Slack has received no message for (#[\w-]+)$")]
async fn no_message_for(world: &mut FoundryWorld, channel: String) {
    eip::then_no_message_for(world, &channel);
}

#[then(regex = r"^(#[\w-]+) holds exactly (\d+) test messages$")]
async fn test_count(world: &mut FoundryWorld, channel: String, n: usize) {
    eip::then_test_count(world, &channel, n);
}

#[then(regex = r"^(#[\w-]+) receives (\d+) messages, no two of them less than one second apart$")]
async fn paced(world: &mut FoundryWorld, channel: String, n: usize) {
    eip::then_paced(world, &channel, n).await;
}

#[then(
    regex = r#"^no message in any channel contains any part of (AUTH-\d+)'s description, the name "([^"]+)", any email address, or any identifier other than (?:AUTH-\d+)$"#
)]
async fn never_sent(world: &mut FoundryWorld, key: String, attachment: String) {
    eip::then_never_sent(world, &key, &attachment).await;
}

#[then(regex = r"^no message Slack received carries any Slack credential or the secrets key$")]
async fn posts_clean(world: &mut FoundryWorld) {
    eip::then_posts_carry_no_credential(world);
}

#[then(regex = r"^both of Slack's posts for (AUTH-\d+) carried the same message$")]
async fn posts_identical(world: &mut FoundryWorld, key: String) {
    eip::then_posts_identical(world, &key);
}

#[then(
    regex = r"^Open in Foundry opens (AUTH-\d+) for Sam and gives Marco the same not-found page any unknown address gets$"
)]
async fn link_opens(world: &mut FoundryWorld, key: String) {
    eip::then_link_opens(world, "#auth-dev", &key).await;
}

#[then(regex = r"^(AUTH-\d+) is created and shown on the board within 2 seconds$")]
async fn board_quick(world: &mut FoundryWorld, key: String) {
    eip::then_board_quick(world, &key, Duration::from_secs(2)).await;
}

// =========================================================================== Emails about connections

#[then(regex = r#"^Priya receives one email "([^"]+)" within 5 minutes$"#)]
async fn one_email(world: &mut FoundryWorld, subject: String) {
    eip::then_email_count(world, "Priya", &subject, 1, Duration::from_secs(300)).await;
}

#[then(regex = r#"^Priya has received exactly one email "([^"]+)"$"#)]
async fn exactly_one_email(world: &mut FoundryWorld, subject: String) {
    eip::then_email_count(world, "Priya", &subject, 1, Duration::from_secs(10)).await;
}

#[then(regex = r"^(Priya|Lena) receives no email about (?:it|the Slack connection)$")]
async fn no_connection_email(world: &mut FoundryWorld, who: String) {
    eip::then_no_connection_email(world, &who).await;
}

#[given(regex = r"^(Priya|Sam) has muted Canzan Labs$")]
async fn muted(world: &mut FoundryWorld, who: String) {
    eip::mute(world, &who).await;
}

// =========================================================================== Project emails

#[given(
    regex = r"^(Sam|Tomás) has chosen (.+?) for (all issues|issues he created or commented on) in (Identity Platform)$"
)]
async fn chosen(
    world: &mut FoundryWorld,
    who: String,
    events: String,
    scope: String,
    project: String,
) {
    eip::given_chosen(world, &who, &events, &scope, &project).await;
}

#[given(
    regex = r"^Sam has no project email choices and has followed Settings in the sidebar of the (Identity Platform) board$"
)]
async fn followed_settings(world: &mut FoundryWorld, project: String) {
    eip::given_followed_settings(world, "Sam", &project).await;
}

#[when(
    regex = r"^Sam ticks (New comment) for (issues he created or commented on) in (Identity Platform) and saves$"
)]
async fn tick_and_save(world: &mut FoundryWorld, event: String, scope: String, project: String) {
    eip::when_tick_and_save(world, "Sam", &event, &scope, &project).await;
}

#[when(regex = r"^Sam opens Project emails in his settings$")]
async fn open_settings(world: &mut FoundryWorld) {
    eip::when_open(world, "Sam", eip::ACCOUNT_SETTINGS).await;
}

#[then(regex = r"^Sam is offered (Identity Platform) and not (Homelab Ops)$")]
async fn offered_projects(world: &mut FoundryWorld, offered: String, hidden: String) {
    eip::then_settings_offer(world, "Sam", &offered, &hidden).await;
}

#[then(regex = r"^no event is ticked for Identity Platform$")]
async fn nothing_ticked(world: &mut FoundryWorld) {
    eip::then_nothing_ticked(world);
}

#[then(
    regex = r"^Identity Platform offers New issue, Issue moved, New comment and Issue deleted, for all issues or only issues he created or commented on$"
)]
async fn every_choice(world: &mut FoundryWorld) {
    eip::then_offers_every_choice(world);
}

#[then(regex = r"^Sam is offered no way to save project email choices$")]
async fn no_save(world: &mut FoundryWorld) {
    eip::then_no_save(world);
}

#[when(
    regex = r"^Sam submits a project email choice of (New issue) for all issues in (Homelab Ops)$"
)]
async fn foreign_choice(world: &mut FoundryWorld, event: String, project: String) {
    eip::when_choose_foreign(world, "Sam", &event, &project).await;
}

#[then(regex = r"^Sam has no project email choices$")]
async fn no_choices(world: &mut FoundryWorld) {
    eip::then_no_choices(world, "Sam").await;
}

#[then(regex = r"^Sam's own choice for (Identity Platform) is still accepted$")]
async fn own_choice(world: &mut FoundryWorld, project: String) {
    eip::then_own_choice_accepted(world, "Sam", &project).await;
}

#[then(regex = r#"^Sam receives within 2 minutes an email "([^"]+)"$"#)]
async fn email_subject(world: &mut FoundryWorld, subject: String) {
    eip::then_email_subject(world, "Sam", &subject, eip::EMAIL_BUDGET).await;
}

#[then(regex = r#"^its footer reads "([^"]+)"$"#)]
async fn footer(world: &mut FoundryWorld, wanted: String) {
    eip::then_received_footer(world, &wanted);
}

#[then(regex = r#"^it carries the comment "([^"]+)" and Open in Foundry$"#)]
async fn carries(world: &mut FoundryWorld, comment: String) {
    eip::then_received_carries(world, &comment);
}

#[then(
    regex = r#"^Sam receives within 2 minutes an email about (AUTH-\d+) whose footer reads "([^"]+)"$"#
)]
async fn email_footer(world: &mut FoundryWorld, key: String, wanted: String) {
    eip::then_email_about(world, "Sam", &key, Some(&wanted)).await;
}

#[then(regex = r"^Sam receives within 2 minutes an email about (AUTH-\d+)$")]
async fn email_about(world: &mut FoundryWorld, key: String) {
    eip::then_email_about(world, "Sam", &key, None).await;
}

#[then(regex = r"^(Sam|Tomás) receives no email about (AUTH-\d+)$")]
async fn no_email(world: &mut FoundryWorld, who: String, key: String) {
    eip::then_no_email_about(world, &who, &key).await;
}

#[given(regex = r#"^Sam has commented "([^"]+)" on (AUTH-\d+)( and deleted it)?$"#)]
async fn sam_commented(world: &mut FoundryWorld, body: String, key: String, deleted: String) {
    eip::given_sam_commented(world, &key, &body, !deleted.is_empty()).await;
}

#[given(regex = r"^Sam has never touched (AUTH-\d+)$")]
async fn never_touched(world: &mut FoundryWorld, _key: String) {
    eip::given_nothing(world);
}

#[then(regex = r#"^Sam's Project emails read "([^"]+)"$"#)]
async fn settings_read(world: &mut FoundryWorld, wanted: String) {
    eip::then_page_reads(world, "Sam", eip::ACCOUNT_SETTINGS, &[&wanted]).await;
}

#[then(
    regex = r"^Sam's Project emails show (New issue|New comment) chosen(?: and (New comment) not chosen)? for Identity Platform$"
)]
async fn settings_show(world: &mut FoundryWorld, chosen: String, not_chosen: String) {
    eip::then_settings_show(
        world,
        "Sam",
        &[EventChoice::parse(&chosen)],
        &EventChoice::parse_list_opt(&not_chosen),
        "Identity Platform",
    )
    .await;
}

#[given(regex = r"^Sam has received a New comment email from Identity Platform$")]
async fn received_comment_email(world: &mut FoundryWorld) {
    eip::given_received_comment_email(world, "Sam").await;
}

#[given(regex = r"^Sam has followed Stop these emails in it while signed out$")]
async fn followed_stop(world: &mut FoundryWorld) {
    eip::given_followed_stop(world, "Sam").await;
}

#[then(regex = r#"^the page Sam confirmed on asked "([^"]+)"$"#)]
async fn confirm_page_asked(world: &mut FoundryWorld, wanted: String) {
    eip::then_in_front_reads(world, &wanted);
}

#[when(regex = r"^Sam confirms the stop$")]
async fn confirm_stop(world: &mut FoundryWorld) {
    eip::when_confirm_stop(world).await;
}

#[given(regex = r"^Sam has stopped New comment emails from Identity Platform from that email$")]
async fn stopped(world: &mut FoundryWorld) {
    eip::given_stopped(world).await;
}

#[when(regex = r"^a link scanner opens its Stop these emails link without confirming$")]
async fn scanner(world: &mut FoundryWorld) {
    eip::when_scanner_opens(world).await;
}

#[when(regex = r"^Sam's mail client sends the one-click unsubscribe of that email$")]
async fn one_click(world: &mut FoundryWorld) {
    eip::when_one_click(world).await;
}

#[when(regex = r"^someone (opens|confirms|sends as a one-click to) a copy of its stop link (.+)$")]
async fn tampered(world: &mut FoundryWorld, uses: String, how: String) {
    eip::when_tampered(world, &uses, &how).await;
}

#[then(regex = r"^they get the same refusal page any invalid stop link gets$")]
async fn invalid_link(world: &mut FoundryWorld) {
    eip::then_invalid_link_refusal(world).await;
}

#[when(regex = r"^Sam asks for a password reset$")]
async fn password_reset(world: &mut FoundryWorld) {
    eip::when_password_reset(world, "Sam").await;
}

#[then(regex = r"^Sam receives the password-reset email$")]
async fn reset_email(world: &mut FoundryWorld) {
    eip::then_reset_email(world, "Sam").await;
}

#[then(regex = r"^the relay holds one email to (\S+) carrying Foundry's one-click stop link$")]
async fn relay(world: &mut FoundryWorld, to: String) {
    eip::then_relay_one_click(world, &to).await;
}

// =========================================================================== Operator

#[given(regex = r"^Foundry runs as the operator's process with the Foundry Slack app set up$")]
async fn process(world: &mut FoundryWorld) {
    eip::given_process(world).await;
}

#[given(regex = r"^Foundry has been killed and started again while Slack was still down$")]
async fn killed(world: &mut FoundryWorld) {
    eip::restart_process(world).await;
}

#[given(regex = r"^Foundry has been started again without its secrets key$")]
async fn without_key(world: &mut FoundryWorld) {
    eip::restart_with_keys(world, None, None).await;
}

#[given(
    regex = r"^Foundry has been started again with a new secrets key and the old one as the previous key$"
)]
async fn new_key(world: &mut FoundryWorld) {
    eip::restart_with_keys(world, Some(eip::NEW_SECRETS_KEY), Some(eip::SECRETS_KEY)).await;
}

#[then(regex = r"^Foundry's output never contains any Slack credential or the secrets key$")]
async fn output_clean(world: &mut FoundryWorld) {
    eip::then_output_clean(world);
}

#[then(regex = r"^the board of (Identity Platform) still opens for Sam$")]
async fn board_opens(world: &mut FoundryWorld, project: String) {
    eip::then_board_opens(world, "Sam", &project).await;
}

#[then(regex = r"^the operator's health signals report the secrets key as degraded$")]
async fn degraded(world: &mut FoundryWorld) {
    eip::then_degraded(world).await;
}

#[then(regex = r"^the operator's health signals report the secrets key as healthy$")]
async fn secrets_healthy(world: &mut FoundryWorld) {
    eip::then_secrets_healthy(world).await;
}

#[then(regex = r"^Canzan Labs' Slack is not marked lost and Priya receives no email about it$")]
async fn not_lost(world: &mut FoundryWorld) {
    eip::then_not_lost(world).await;
}

#[when(regex = r"^the operator starts Foundry with (.+)$")]
async fn start_with(world: &mut FoundryWorld, setting: String) {
    eip::when_start_with(world, &setting).await;
}

#[then(regex = r"^Foundry refuses to start, naming (.+)$")]
async fn refused(world: &mut FoundryWorld, name: String) {
    eip::then_refused_start(world, &name);
}

#[when(regex = r"^the operator runs foundry doctor rotate-secrets$")]
async fn rotate(world: &mut FoundryWorld) {
    eip::when_rotate(world).await;
}

#[when(
    regex = r"^the operator finishes the key rotation: runs foundry doctor rotate-secrets, then starts Foundry with only the new secrets key$"
)]
async fn finish_rotation(world: &mut FoundryWorld) {
    eip::when_finish_rotation(world).await;
}

#[then(regex = r"^the command reports (\d+) credential sealed again under the new key$")]
async fn rotated(world: &mut FoundryWorld, n: usize) {
    eip::then_rotated(world, n);
}

#[given(regex = r"^the operator's delivery counters all read zero$")]
async fn counters_zero(world: &mut FoundryWorld) {
    eip::then_counters_zero(world).await;
}

#[then(regex = r"^the operator's delivery counters show one Slack New issue delivered$")]
async fn counter_delivered(world: &mut FoundryWorld) {
    eip::then_counter_delivered(world).await;
}

#[then(
    regex = r"^the delivery counters name no channel, room, project, workspace, address or person$"
)]
async fn counter_labels(world: &mut FoundryWorld) {
    eip::then_counter_labels_closed(world).await;
}

#[given(
    regex = r"^Foundry runs as two replicas on one database with the Foundry Slack app set up$"
)]
async fn replicas(world: &mut FoundryWorld) {
    eip::given_replicas(world).await;
}

#[given(
    regex = r"^every replica stopped in the middle of sending (AUTH-\d+)'s message and a fresh replica started$"
)]
async fn stop_replica(world: &mut FoundryWorld, _key: String) {
    eip::stop_sending_replica(world).await;
}

#[given(
    regex = r"^the replica sending (AUTH-\d+)'s message stopped before hearing back, and the other replica kept running$"
)]
async fn stop_claimant_only(world: &mut FoundryWorld, key: String) {
    eip::stop_claiming_replica_only(world, &key).await;
}

// =========================================================================== Matrix

#[given(
    regex = r"^the homeserver matrix\.canzan\.dev has the bot account (@[\w.:-]+) and Lena's account (@[\w.:-]+)$"
)]
async fn homeserver(world: &mut FoundryWorld, bot: String, lena: String) {
    eip::given_homeserver_accounts(world, &bot, &lena).await;
}

#[given(regex = r#"^Lena has the unencrypted room (#[\w.:-]+) named "([^"]+)" and (\d+) members$"#)]
async fn room(world: &mut FoundryWorld, alias: String, name: String, members: usize) {
    eip::given_room(world, &alias, Some(&name), members, false).await;
}

#[given(regex = r"^Lena has the end-to-end encrypted room (#[\w.:-]+)$")]
async fn encrypted_room(world: &mut FoundryWorld, alias: String) {
    eip::given_room(world, &alias, None, 1, true).await;
}

#[given(regex = r"^Lena has invited @foundry:canzan\.dev to (#[\w.:-]+)$")]
async fn invited(world: &mut FoundryWorld, room: String) {
    eip::invite_bot(world, &room).await;
}

#[given(regex = r"^@foundry:canzan\.dev has not been invited to (#[\w.:-]+)$")]
async fn not_invited(world: &mut FoundryWorld, room: String) {
    eip::then_bot_member(world, &room, false).await;
}

#[given(regex = r"^a room moderator has removed @foundry:canzan\.dev from (#[\w.:-]+)$")]
async fn kicked(world: &mut FoundryWorld, room: String) {
    eip::kick_bot(world, &room).await;
}

#[given(regex = r"^Lena has turned on end-to-end encryption in (#[\w.:-]+)$")]
async fn encrypt(world: &mut FoundryWorld, room: String) {
    eip::encrypt_room(world, &room).await;
}

#[given(regex = r"^the operator has logged the bot out of matrix\.canzan\.dev$")]
async fn logout(world: &mut FoundryWorld) {
    eip::log_bot_out(world).await;
}

#[given(regex = r"^Priya has connected Canzan Labs to Matrix with the bot's access token$")]
async fn matrix_connected(world: &mut FoundryWorld) {
    eip::given_matrix_connected(world).await;
}

#[given(regex = r"^the homeserver (does not answer|answers again|answers)$")]
async fn homeserver_mood(world: &mut FoundryWorld, mood: String) {
    eip::homeserver_answers(world, mood != "does not answer");
}

#[when(regex = r"^the homeserver answers again$")]
async fn homeserver_heals(world: &mut FoundryWorld) {
    eip::when_homeserver_answers(world).await;
}

#[given(regex = r"^the homeserver will answer Foundry's next send with (.+)$")]
async fn homeserver_fault(world: &mut FoundryWorld, answer: String) {
    eip::homeserver_next_answer(world, &answer);
}

#[then(regex = r"^the homeserver gave the answer it was set to give$")]
async fn fault_given(world: &mut FoundryWorld) {
    eip::then_fault_given(world);
}

#[then(regex = r"^the homeserver was asked to send (AUTH-\d+)'s notice only once$")]
async fn sends_once(world: &mut FoundryWorld, _key: String) {
    eip::then_sends_counted(world, 1);
}

#[when(regex = r"^(Priya|Lena|Marco) connects Matrix with (https://\S+) and (.+)$")]
async fn connect_matrix(world: &mut FoundryWorld, who: String, address: String, token: String) {
    eip::when_connect_matrix(world, &who, &address, &token).await;
}

#[when(
    regex = r"^Priya connects Matrix with a homeserver listening on this machine, written (as 127\.0\.0\.1|as an IPv4-mapped IPv6 address)$"
)]
async fn connect_local(world: &mut FoundryWorld, form: String) {
    eip::when_connect_matrix_local(world, "Priya", &form).await;
}

#[then(regex = r#"^the access token field reads "([^"]+)"$"#)]
async fn token_field(world: &mut FoundryWorld, wanted: String) {
    eip::then_page_reads(world, "Priya", eip::WORKSPACE_INTEGRATIONS, &[&wanted]).await;
}

#[then(regex = r"^the bot's access token appears nowhere on the workspace integrations page$")]
async fn token_hidden(world: &mut FoundryWorld) {
    eip::then_matrix_token_hidden(world).await;
}

#[then(regex = r"^Canzan Labs has no Matrix connection$")]
async fn no_matrix(world: &mut FoundryWorld) {
    eip::then_no_matrix_connection(world).await;
}

#[then(regex = r"^nothing reached the listener on this machine or the homeserver$")]
async fn untouched(world: &mut FoundryWorld) {
    eip::then_homeserver_untouched(world);
}

#[then(regex = r"^@foundry:canzan\.dev (is|is not) a member of (#[\w.:-]+)$")]
async fn bot_member(world: &mut FoundryWorld, is: String, room: String) {
    eip::then_bot_member(world, &room, is == "is").await;
}

#[then(
    regex = r#"^(#[\w-]+:[\w.]+) shows within 10 seconds a notice from @foundry:canzan\.dev with "([^"]+)" in bold, "([^"]+)" and an Open in Foundry link$"#
)]
async fn notice(world: &mut FoundryWorld, room: String, bold: String, line: String) {
    eip::then_notice(world, &room, &bold, &line).await;
}

#[then(regex = r"^the notice's plain text carries the same two lines$")]
async fn plain_text(world: &mut FoundryWorld) {
    eip::then_notice_plain_text(world).await;
}

#[then(regex = r#"^(#[\w-]+:[\w.]+) shows a notice "([^"]+)"$"#)]
async fn notice_text(world: &mut FoundryWorld, room: String, text: String) {
    eip::then_notice_text(world, &room, &text).await;
}

#[then(
    regex = r"^(#[\w-]+:[\w.]+) shows (a|no) notice from @foundry:canzan\.dev about (AUTH-\d+)$"
)]
async fn notice_about(world: &mut FoundryWorld, room: String, some: String, key: String) {
    eip::then_notice_about(world, &room, &key, some == "a").await;
}

#[then(
    regex = r#"^(#[\w-]+:[\w.]+) shows (AUTH-\d+)'s comment notice (with|without) "([^"]+)" in both its formatted and plain text$"#
)]
async fn comment_notice(
    world: &mut FoundryWorld,
    room: String,
    key: String,
    with: String,
    text: String,
) {
    eip::then_comment_notice(world, &room, &key, with == "with", &text).await;
}

#[then(regex = r"^(#[\w-]+:[\w.]+) shows that text without notifying the room or anyone in it$")]
async fn matrix_no_ping(world: &mut FoundryWorld, room: String) {
    eip::then_matrix_no_ping(world, &room).await;
}

#[then(
    regex = r"^no notice in (#[\w-]+:[\w.]+) carries the bot's access token or the secrets key$"
)]
async fn notices_clean(world: &mut FoundryWorld, room: String) {
    eip::then_notices_carry_no_credential(world, &room).await;
}

#[when(regex = r"^Lena chooses Send test on the route to (#[\w-]+:[\w.]+)$")]
async fn matrix_test(world: &mut FoundryWorld, room: String) {
    eip::when_matrix_test(world, "Lena", &room).await;
}
