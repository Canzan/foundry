//! instance-integration-setup step definitions
//! (`tests/features/instance-integration-setup.feature`, DISTILL 2026-11-06,
//! Deliverable A, scaffolded RED with every scenario `@pending`).
//!
//! Every step body is one call into `support::instance_integration_setup` (or
//! the eip driver it builds on); the words are parsed by the driver's types
//! (`KeySetting`, `Restart`, `InstanceDoor`, `Button`, `Offer`,
//! `WorkspaceOffer`, `Signer`, `BadStart`, `Refusal`). Reused unchanged from the
//! eip step file: the Background's people, projects and time, "<who> reads …",
//! "<who> gets the same not-found page …" (Lena, Sam, Marco, a signed-out
//! visitor), "the workspace integrations page reads …", "the next issue filed
//! in … becomes …", "Lena has routed …", "Sam files …", "#auth-dev shows within
//! 10 seconds …", Slack's events-address answers, and the operator's health
//! signals. No step holds logic of its own (Mandate 12, criterion 3).
//!
//! Phrases are global across the suite, so every regex is anchored and names
//! something only this feature says ("instance integrations page", "this
//! Foundry starts with", "has since restarted", "a … request", "the second
//! server", "Foundry is started with").

use crate::support::external_integration_plugins as eip;
use crate::support::instance_integration_setup::{
    self as iis, BadStart, Button, InstanceDoor, KeySetting, Offer, Refusal, Restart, Signer,
    SlackControl, WorkspaceOffer,
};
use crate::support::integrations_wayfinding::{self as iw, Offered};
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};

/// The sentences a one-column table lists, in the order it lists them.
fn sentences(step: &Step) -> Vec<String> {
    step.table
        .as_ref()
        .expect("this step carries a table of sentences")
        .rows
        .iter()
        .skip(1)
        .map(|row| row[0].trim().to_string())
        .collect()
}

// =========================================================================== instance

#[given(regex = r"^(Priya Raman|Tomás Rivera|Lena Park) is this Foundry's instance admin$")]
async fn instance_admin(world: &mut FoundryWorld, person: String) {
    iis::given_instance_admin(world, &person).await;
}

#[given(
    regex = r"^this Foundry starts with (its instance key set by the operator|the legacy FOUNDRY_SECRETS_KEY setting|no instance key setting and a writable data directory|no instance key setting and a data directory it cannot write|no instance key setting and an instance key file it cannot read|FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY both set to empty values|FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to the same key)$"
)]
async fn starts_with(world: &mut FoundryWorld, setting: String) {
    iis::given_starts(world, KeySetting::parse(&setting)).await;
}

#[given(regex = r"^the operator has not given this Foundry its public address$")]
async fn public_url_unset(world: &mut FoundryWorld) {
    iis::given_public_url_unset(world).await;
}

#[given(regex = r"^Foundry's environment sets the Slack app 6123456789012\.7234567890123$")]
async fn environment_app(world: &mut FoundryWorld) {
    iis::given_environment_app(world).await;
}

#[given(
    regex = r"^this Foundry runs as two replicas with (its instance key set by the operator|the legacy FOUNDRY_SECRETS_KEY setting)$"
)]
async fn replicas(world: &mut FoundryWorld, setting: String) {
    iis::given_replicas(world, KeySetting::parse(&setting)).await;
}

#[given(regex = r"^Priya has generated the secrets key$")]
async fn generated(world: &mut FoundryWorld) {
    iis::given_generated(world).await;
}

#[given(regex = r"^Priya has saved the Slack app 6123456789012\.7234567890123$")]
async fn saved_app(world: &mut FoundryWorld) {
    iis::given_saved_app(world).await;
}

#[given(regex = r"^Priya has rotated the Slack app's signing secret$")]
async fn rotated_signing_secret(world: &mut FoundryWorld) {
    iis::given_rotated_signing_secret(world).await;
}

#[given(regex = r"^Priya has connected Canzan Labs to Slack with this Foundry's app$")]
async fn connected(world: &mut FoundryWorld) {
    iis::given_connected(world).await;
}

#[given(
    regex = r"^this Foundry has since restarted (with the same settings|with a different instance key set by the operator|with the same key under FOUNDRY_INSTANCE_KEY|without the Slack app in its environment|with a different instance key file)$"
)]
async fn has_restarted(world: &mut FoundryWorld, how: String) {
    iis::restart(world, Restart::parse(&how)).await;
}

#[when(
    regex = r"^this Foundry restarts (with the same settings|with a different instance key set by the operator)$"
)]
async fn restarts(world: &mut FoundryWorld, how: String) {
    iis::restart(world, Restart::parse(&how)).await;
}

// =========================================================================== operator processes

#[given(
    regex = r"^Foundry runs as the operator's standalone process with (its instance key set by the operator|no instance key setting and a writable data directory)$"
)]
async fn standalone(world: &mut FoundryWorld, setting: String) {
    iis::given_standalone(world, KeySetting::parse(&setting)).await;
}

#[when(regex = r"^the operator's standalone process restarts$")]
async fn process_restarts(world: &mut FoundryWorld) {
    iis::when_process_restarts(world).await;
}

#[given(
    regex = r"^two Foundry servers share one data directory and neither has an instance key setting$"
)]
async fn two_sharing(world: &mut FoundryWorld) {
    iis::given_two_servers_sharing(world).await;
}

#[when(regex = r"^both servers start at the same moment$")]
async fn both_start(world: &mut FoundryWorld) {
    iis::when_both_start(world).await;
}

#[given(
    regex = r"^Foundry runs as two servers on one database, the second with a different instance key set by the operator$"
)]
async fn two_keys(world: &mut FoundryWorld) {
    iis::given_two_servers_two_keys(world).await;
}

#[when(regex = r"^Priya generates the secrets key on the first server$")]
async fn generate_on_first(world: &mut FoundryWorld) {
    iis::when_generate_on_first(world).await;
}

#[when(
    regex = r"^Foundry is started with (an instance key of 20 characters|FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to different keys)$"
)]
async fn started_with(world: &mut FoundryWorld, setting: String) {
    iis::when_started_with(world, BadStart::parse(&setting)).await;
}

// =========================================================================== the instance page

#[when(
    regex = r"^(Priya|Tomás|Lena|Sam|Marco|a signed-out visitor) opens the instance integrations page$"
)]
async fn open_page(world: &mut FoundryWorld, who: String) {
    iis::when_open(world, &who).await;
}

#[when(regex = r#"^(Priya) follows "Integrations" on the instance admin dashboard$"#)]
async fn follow_dashboard(world: &mut FoundryWorld, who: String) {
    iis::when_follow_dashboard(world, &who).await;
}

#[when(
    regex = r#"^(Priya) chooses "(Generate secrets key|Move to a secrets key)" on the instance integrations page$"#
)]
async fn choose(world: &mut FoundryWorld, who: String, button: String) {
    iis::when_choose(world, &who, Button::parse(&button)).await;
}

#[when(
    regex = r#"^(Priya) saves the Slack app with client ID "([^"]*)", client secret "([^"]*)" and signing secret "([^"]*)"$"#
)]
async fn save_app(
    world: &mut FoundryWorld,
    who: String,
    client_id: String,
    client_secret: String,
    signing_secret: String,
) {
    iis::when_save_slack_app(world, &who, &client_id, &client_secret, &signing_secret).await;
}

#[when(
    regex = r"^(Priya|Tomás|Lena|a signed-out visitor) posts a (generate|move|Slack app save|Slack confirm|Slack remove) request(?: anyway| again)?$"
)]
async fn post_door(world: &mut FoundryWorld, who: String, door: String) {
    iis::when_post_door(world, &who, InstanceDoor::parse(&door)).await;
}

#[given(
    regex = r"^Priya has submitted the Slack app 6123456789012\.8888888888888 with both of its secrets$"
)]
async fn submitted_other_app(world: &mut FoundryWorld) {
    iis::given_submitted_other_app(world).await;
}

#[given(regex = r"^Priya has opened the confirm page twice without choosing anything$")]
async fn prefetched_confirm(world: &mut FoundryWorld) {
    iis::given_prefetched_confirm(world).await;
}

#[given(regex = r"^(Tomás) has posted a Slack confirm request$")]
async fn posted_confirm(world: &mut FoundryWorld, who: String) {
    iis::given_posted_confirm(world, &who).await;
}

#[given(regex = r#"^Priya has saved the new app from the confirm page$"#)]
async fn chose_save_new_app(world: &mut FoundryWorld) {
    iis::given_chose_save_new_app(world).await;
}

#[when(regex = r#"^(Priya) chooses "(Save the new app)" on the confirm page$"#)]
async fn choose_on_confirm(world: &mut FoundryWorld, who: String, button: String) {
    iis::when_choose_on_confirm(world, &who, Button::parse(&button)).await;
}

#[when(
    regex = r#"^(Priya) chooses "(Replace|Remove)" for Slack on the instance integrations page$"#
)]
async fn choose_slack_control(world: &mut FoundryWorld, who: String, control: String) {
    iis::when_choose_slack_control(world, &who, SlackControl::parse(&control)).await;
}

#[given(regex = r"^a browser has already opened the Slack remove page once$")]
async fn prefetched_remove(world: &mut FoundryWorld) {
    iis::given_prefetched_remove(world).await;
}

#[when(regex = r#"^(Priya) chooses "(Remove)" on the Slack remove page$"#)]
async fn choose_on_remove(world: &mut FoundryWorld, who: String, button: String) {
    iis::when_choose_on_remove_page(world, &who, Button::parse(&button)).await;
}

#[given(regex = r"^every replica has the Slack app 6123456789012\.7234567890123 in effect$")]
async fn replicas_have_saved_app(world: &mut FoundryWorld) {
    iis::given_replicas_have_saved_app(world).await;
}

#[given(regex = r"^Priya has removed the Slack app$")]
async fn removed_app(world: &mut FoundryWorld) {
    iis::given_removed_app(world).await;
}

#[then(regex = r"^(Priya) is on the Slack remove page$")]
async fn on_remove_page(world: &mut FoundryWorld, who: String) {
    iis::then_on_remove_page(world, &who);
}

#[when(regex = r"^(Priya) opens the Slack remove page$")]
async fn open_remove_page(world: &mut FoundryWorld, who: String) {
    iis::when_open_remove(world, &who).await;
}

#[then(regex = r"^(Priya) is offered both Replace and Remove for Slack$")]
async fn slack_controls(world: &mut FoundryWorld, who: String) {
    iis::then_slack_controls(world, &who).await;
}

#[then(regex = r"^(Priya) is offered neither Replace nor Remove for Slack$")]
async fn no_slack_controls(world: &mut FoundryWorld, who: String) {
    iis::then_no_slack_controls(world, &who).await;
}

#[then(
    regex = r"^the Slack app Foundry's environment sets is still in effect, with its activity unchanged$"
)]
async fn environment_app_unchanged(world: &mut FoundryWorld) {
    iis::then_environment_app_unchanged(world).await;
}

#[then(regex = r"^(Priya) is on the Slack confirm page$")]
async fn on_confirm_page(world: &mut FoundryWorld, who: String) {
    iis::then_on_confirm_page(world, &who);
}

#[then(regex = r"^no Slack app is waiting for Priya's confirmation$")]
async fn nothing_waiting(world: &mut FoundryWorld) {
    iis::then_nothing_waiting(world).await;
}

#[then(regex = r"^(Priya|Tomás)'s answer is this Foundry's integrations page$")]
async fn answers_integrations_page(world: &mut FoundryWorld, who: String) {
    iis::then_answers_integrations_page(world, &who);
}

#[then(
    regex = r#"^(Priya) is (offered|not offered) "(Generate secrets key|Move to a secrets key|Replace secrets key|Start over|Save the new app|Remove)"$"#
)]
async fn button_offered(world: &mut FoundryWorld, who: String, offered: String, button: String) {
    iis::then_button(world, &who, Button::parse(&button), Offer::parse(&offered));
}

#[then(regex = r"^(Priya) is offered no way to enter the Slack app$")]
async fn no_slack_form(world: &mut FoundryWorld, who: String) {
    iis::then_no_slack_form(world, &who);
}

#[then(regex = r#"^Priya sees the redirect URL "([^"]+)" and the events URL "([^"]+)"$"#)]
async fn addresses(world: &mut FoundryWorld, redirect: String, events: String) {
    iis::then_addresses(world, &redirect, &events);
}

#[then(regex = r"^the manifest shown equals the one on the Slack setup page$")]
async fn manifest_matches(world: &mut FoundryWorld) {
    iis::then_manifest_matches(world).await;
}

#[then(regex = r"^the warning about the unreachable address comes before the Slack addresses$")]
async fn warning_first(world: &mut FoundryWorld) {
    iis::then_warning_first(world);
}

#[then(regex = r"^(Priya) is on the instance integrations page$")]
async fn on_instance_page(world: &mut FoundryWorld, who: String) {
    iis::then_on_instance_page(world, &who);
}

#[then(regex = r"^(Tomás) gets the same not-found page any unknown address gets$")]
async fn tomas_not_found(world: &mut FoundryWorld, who: String) {
    eip::then_not_found(world, &who).await;
}

#[then(regex = r#"^the instance integrations page reads "(.+)"$"#)]
async fn page_reads(world: &mut FoundryWorld, words: String) {
    iis::then_page_reads(world, &words).await;
}

#[then(regex = r"^this Foundry still has no secrets key$")]
async fn still_no_key(world: &mut FoundryWorld) {
    iis::then_still_no_secrets_key(world).await;
}

#[then(regex = r#"^the instance integrations activity reads "(.+?)"( exactly once)?$"#)]
async fn activity(world: &mut FoundryWorld, line: String, once: String) {
    iis::then_activity(world, &line, !once.is_empty()).await;
}

#[then(regex = r#"^the form keeps the client ID "([^"]*)" and leaves both secrets empty$"#)]
async fn form_kept(world: &mut FoundryWorld, client_id: String) {
    iis::then_form_kept(world, &client_id);
}

#[then(
    regex = r#"^the Slack app form offers the client ID "([^"]*)" with both secret fields empty$"#
)]
async fn form_offers(world: &mut FoundryWorld, client_id: String) {
    iis::then_form_kept(world, &client_id);
}

#[then(regex = r"^the instance integrations activity reads these lines in this order$")]
async fn activity_lines(world: &mut FoundryWorld, step: &Step) {
    iis::then_activity_lines(world, &sentences(step)).await;
}

#[then(
    regex = r"^the Slack app (\d+\.\d+) is still in effect, with its secrets and its activity unchanged$"
)]
async fn saved_app_unchanged(world: &mut FoundryWorld, client_id: String) {
    iis::then_saved_app_unchanged(world, &client_id).await;
}

#[then(regex = r"^Slack is (?:still not|no longer) set up on this Foundry$")]
async fn slack_not_set_up(world: &mut FoundryWorld) {
    iis::then_slack_not_set_up(world).await;
}

#[then(regex = r"^the page shows no key and no Slack secret$")]
async fn page_shows_nothing_secret(world: &mut FoundryWorld) {
    iis::then_page_shows_nothing_secret(world).await;
}

// =========================================================================== workspaces, Slack, the board

#[then(
    regex = r"^Canzan Labs' workspace integrations page offers (to connect Matrix|Add to Slack)$"
)]
async fn workspace_offers(world: &mut FoundryWorld, offer: String) {
    iis::then_workspace_offers(world, WorkspaceOffer::parse(&offer)).await;
}

#[then(
    regex = r"^within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers (to connect Matrix|Add to Slack) on every replica$"
)]
async fn every_replica_offers(world: &mut FoundryWorld, offer: String) {
    iis::then_every_replica_offers(world, WorkspaceOffer::parse(&offer)).await;
}

#[then(
    regex = r"^within 60 seconds, without a restart, every replica verifies Slack's events signed with (the rotated signing secret|the signing secret now in effect)$"
)]
async fn every_replica_verifies(world: &mut FoundryWorld, signer: String) {
    iis::then_every_replica_verifies(world, Signer::parse(&signer)).await;
}

#[then(
    regex = r"^within 60 seconds, without a restart, every replica refuses as unauthorised Slack's events signed with (the signing secret the removed app was saved with)$"
)]
async fn no_replica_verifies(world: &mut FoundryWorld, signer: String) {
    iis::then_no_replica_verifies(world, Signer::parse(&signer)).await;
}

#[then(
    regex = r"^Canzan Labs' workspace integrations page offers neither Add to Slack nor to connect Matrix$"
)]
async fn workspace_offers_nothing(world: &mut FoundryWorld) {
    iis::then_workspace_offers_nothing(world).await;
}

#[when(regex = r"^a generate request reaches each replica at the same moment$")]
async fn race(world: &mut FoundryWorld) {
    iis::when_race(world).await;
}

#[then(regex = r"^exactly one secrets key is stored, and both requests were answered with it set$")]
async fn one_key(world: &mut FoundryWorld) {
    iis::then_one_key(world).await;
}

#[when(regex = r"^Priya connects Canzan Labs to Slack with this Foundry's app$")]
async fn connect(world: &mut FoundryWorld) {
    iis::when_connect(world).await;
}

#[then(regex = r"^within 60 seconds Canzan Labs' Slack connection opens on every replica$")]
async fn opens_on_every_replica(world: &mut FoundryWorld) {
    iis::then_connection_opens_on_every_replica(world).await;
}

#[then(
    regex = r"^within 60 seconds every replica's history of the route to (#[\w-]+) lists (AUTH-\d+) as delivered$"
)]
async fn every_replica_history(world: &mut FoundryWorld, destination: String, key: String) {
    iis::then_every_replica_history(world, &destination, &key).await;
}

#[then(regex = r#"^a new Matrix route for Identity Platform no longer reads "(.+)"$"#)]
async fn matrix_form_not(world: &mut FoundryWorld, words: String) {
    iis::then_matrix_form_not(world, &words).await;
}

#[then(regex = r"^Canzan Labs' saved Slack connection still opens$")]
async fn connection_opens(world: &mut FoundryWorld) {
    iis::then_connection_opens(world).await;
}

#[then(
    regex = r"^Canzan Labs' Slack connection is not marked lost and no workspace admin is emailed$"
)]
async fn not_lost(world: &mut FoundryWorld) {
    eip::then_not_lost(world).await;
}

#[then(
    regex = r"^Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID (\d+\.\d+)$"
)]
async fn approval_for(world: &mut FoundryWorld, client_id: String) {
    iis::then_approval_for(world, &client_id).await;
}

#[when(
    regex = r"^Slack sends its URL verification challenge signed with (the saved signing secret|the rotated signing secret|the signing secret it replaced|another signing secret)$"
)]
async fn challenge(world: &mut FoundryWorld, signer: String) {
    iis::when_challenge(world, Signer::parse(&signer)).await;
}

#[then(regex = r"^(Priya) still opens Identity Platform's board$")]
async fn board_opens(world: &mut FoundryWorld, who: String) {
    eip::then_board_opens(world, &who, "Identity Platform").await;
}

#[then(regex = r"^this Foundry still answers that it is ready$")]
async fn ready(world: &mut FoundryWorld) {
    iis::then_ready(world).await;
}

// =========================================================================== the key file, refusals, alerts

#[then(regex = r"^the instance key file is unchanged$")]
async fn file_unchanged(world: &mut FoundryWorld) {
    iis::then_file_unchanged(world);
}

#[then(regex = r"^the instance key file is readable only by Foundry's user$")]
async fn file_private(world: &mut FoundryWorld) {
    iis::then_file_private(world);
}

#[then(regex = r"^exactly one instance key file exists in the shared data directory$")]
async fn one_key_file(world: &mut FoundryWorld) {
    iis::then_one_key_file(world);
}

#[then(
    regex = r#"^within 60 seconds the second server's instance integrations page reads "(.+)"$"#
)]
async fn second_reads(world: &mut FoundryWorld, words: String) {
    iis::then_second_reads(world, &words).await;
}

#[then(regex = r"^the second server's health signals report the secrets key as degraded$")]
async fn second_degraded(world: &mut FoundryWorld) {
    iis::then_second_degraded(world).await;
}

#[then(
    regex = r#"^Foundry refuses to start with a "health\.startup\.refused" line naming (FOUNDRY_INSTANCE_KEY|FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY), never the value$"#
)]
async fn refused(world: &mut FoundryWorld, names: String) {
    iis::then_refused(world, Refusal::parse(&names));
}

// =========================================================================== the links a page offers (US-IIS-04)

// Tomás reads the workspace integrations page and a project's route form as a
// workspace admin who is not an instance admin. The eip and wayfinding drivers
// own the doors and the page reader; their step files name only Priya, Lena,
// Sam and Marco, so these phrases add his name and nothing else.
#[when(regex = r"^(Tomás) opens a new (Slack|Matrix) route for (Identity Platform)$")]
async fn tomas_opens_new_route(
    world: &mut FoundryWorld,
    who: String,
    kind: String,
    project: String,
) {
    iw::when_open_new_route(world, &who, eip::Kind::parse(&kind), &project).await;
}

#[when(
    regex = r"^(Tomás) (previews|saves|sends a test from) a new (Slack|Matrix) route for (Identity Platform)$"
)]
async fn tomas_posts_new_route(
    world: &mut FoundryWorld,
    who: String,
    door: String,
    kind: String,
    project: String,
) {
    iw::when_post_new_route(
        world,
        &who,
        iw::RouteDoor::parse(&door),
        eip::Kind::parse(&kind),
        &project,
    )
    .await;
}

#[when(regex = r"^(Tomás) opens the workspace integrations page$")]
async fn tomas_opens_workspace(world: &mut FoundryWorld, who: String) {
    eip::when_open_door(
        world,
        &who,
        eip::WorkspaceDoor::parse("the workspace integrations page"),
    )
    .await;
}

#[then(regex = r#"^(Tomás) reads "(.+)"$"#)]
async fn tomas_reads(world: &mut FoundryWorld, who: String, words: String) {
    eip::then_reads(world, &who, &words);
}

// Tomás also reads the Slack setup guide (I-20): he is a workspace admin and
// not this Foundry's instance admin, which is the reader the workspace page
// sends there.
#[when(regex = r"^(Tomás) opens the Slack setup page$")]
async fn tomas_opens_slack_setup(world: &mut FoundryWorld, who: String) {
    eip::when_open_door(
        world,
        &who,
        eip::WorkspaceDoor::parse("the Slack setup page"),
    )
    .await;
}

#[then(regex = r"^(Tomás) reads these sentences in this order$")]
async fn reads_in_order(world: &mut FoundryWorld, who: String, step: &Step) {
    iis::then_reads_in_order(world, &who, &sentences(step));
}

#[then(regex = r#"^nothing (Priya|Tomás) reads there says "([^"]+)"$"#)]
async fn reads_nothing_saying(world: &mut FoundryWorld, who: String, phrase: String) {
    iis::then_reads_nothing_saying(world, &who, &phrase);
}

#[then(regex = r"^the links (Priya|Tomás|Lena) is offered read (.+)$")]
async fn page_links(world: &mut FoundryWorld, who: String, offered: String) {
    iw::then_page_links(world, &who, Offered::parse(&offered));
}

#[then(regex = r"^every link (Priya|Tomás|Lena) is offered answers with its page$")]
async fn every_link_opens(world: &mut FoundryWorld, who: String) {
    iw::then_each_link_opens(world, &who).await;
}

#[then(regex = r#"^following "([^"]+)" lands (Priya|Lena) on the instance integrations page$"#)]
async fn lands_on_instance_page(world: &mut FoundryWorld, link: String, who: String) {
    iw::then_following_lands(world, &who, &link, "the instance integrations page").await;
}

// =========================================================================== KPI-IIS-G1

#[when(regex = r"^the database's contents are read without the instance key$")]
async fn dump(world: &mut FoundryWorld) {
    iis::when_dump(world).await;
}

#[then(
    regex = r"^neither the secrets key, the instance key, the Slack app's secrets nor Canzan Labs' Slack token can be recovered from them$"
)]
async fn dump_opens_nothing(world: &mut FoundryWorld) {
    iis::then_dump_opens_nothing(world).await;
}

#[when(
    regex = r"^everything Foundry shows, logs and stores is searched for the keys and the Slack secrets$"
)]
async fn sweep(world: &mut FoundryWorld) {
    iis::when_sweep(world).await;
}

#[then(
    regex = r"^none of them appears in any page, page source, log line or activity line, and none is stored in plaintext$"
)]
async fn sweep_clean(world: &mut FoundryWorld) {
    iis::then_sweep_clean(world).await;
}

// =========================================================================== replacing the secrets key (US-IIS-07)

#[given(
    regex = r"^this Foundry holds (nothing saved yet|the Slack app Priya entered|one workspace connection|one workspace connection and the Slack app Priya entered)$"
)]
async fn holds_saved(world: &mut FoundryWorld, saved: String) {
    iis::given_saved(world, iis::SavedSetup::parse(&saved)).await;
}

#[given(regex = r"^a browser has already opened the replace secrets key page once$")]
async fn prefetched_replace(world: &mut FoundryWorld) {
    iis::given_prefetched_replace(world).await;
}

#[when(regex = r"^(Priya) opens the replace secrets key page$")]
async fn open_replace_page(world: &mut FoundryWorld, who: String) {
    iis::when_open_replace(world, &who).await;
}

#[when(regex = r#"^(Priya) chooses "(Replace secrets key)" on the instance integrations page$"#)]
async fn choose_key_control(world: &mut FoundryWorld, who: String, button: String) {
    iis::when_choose_key_control(world, &who, Button::parse(&button)).await;
}

#[when(regex = r#"^(Priya) chooses "(Replace secrets key)" on the replace secrets key page$"#)]
async fn choose_on_replace_page(world: &mut FoundryWorld, who: String, button: String) {
    iis::when_choose_on_replace_page(world, &who, Button::parse(&button)).await;
}

#[then(regex = r"^(Priya) is on the replace secrets key page$")]
async fn on_replace_page(world: &mut FoundryWorld, who: String) {
    iis::then_on_replace_page(world, &who);
}

#[then(regex = r"^the stored secrets key and the Activity are as they were$")]
async fn nothing_replaced(world: &mut FoundryWorld) {
    iis::then_nothing_replaced(world).await;
}

#[then(regex = r"^the stored secrets key is a new one$")]
async fn key_replaced(world: &mut FoundryWorld) {
    iis::then_key_replaced(world).await;
}

#[when(regex = r"^a replace request reaches each replica at the same moment$")]
async fn replace_race(world: &mut FoundryWorld) {
    iis::when_replace_race(world).await;
}

#[then(
    regex = r"^the secrets key was replaced once, and both requests were answered with it in effect$"
)]
async fn replaced_once(world: &mut FoundryWorld) {
    iis::then_replaced_once(world).await;
}

#[then(regex = r"^the new secrets key appears nowhere Priya reads$")]
async fn new_key_never_shown(world: &mut FoundryWorld) {
    iis::then_new_key_never_shown(world).await;
}

#[given(regex = r"^Priya has replaced the secrets key$")]
async fn replaced_key(world: &mut FoundryWorld) {
    iis::given_replaced_key(world).await;
}

#[given(regex = r"^every replica has this Foundry's secrets key in effect$")]
async fn replicas_have_the_key(world: &mut FoundryWorld) {
    iis::given_replicas_have_the_key(world).await;
}
