//! external-integration-plugins — DELIVER Phase 4 review findings (2026-10-25,
//! feature-delta DISTILL DIST-32): crate-level tests that reach the product
//! through the same doors and doubles as the acceptance lane (the in-process
//! app over the shared Postgres, the Slack double, the pinned homeserver
//! behind its fault proxy), written without Gherkin: no scenario was added.
//!
//! Each case is RED today for the defect it names and is ignored with
//! "RED until review fix <ID>"; the crafter lifts the ignore with the fix.
//! Later rounds add their cases here too: R1/R2 (DIST-33) and the review fix
//! FU-11 (DIST-34, `fu11_*`: a connection this Foundry cannot read).
//!
//! Its own runner (`harness = false`, like `tests/acceptance.rs`): the shared
//! Postgres and homeserver containers live in process-wide slots that must be
//! removed while the tokio runtime is alive (support::harness explains why a
//! `Drop` cannot). Cases run one after another; libtest's flags are honoured
//! as far as they apply:
//!
//! `timeout 1800 cargo test -p foundry-acceptance --release --test eip_review_findings -- --ignored [filter]`
//!
//! `--ignored` runs only the ignored cases, `--include-ignored` every case,
//! `--list` names them, a bare word filters by substring (`--exact` for the
//! whole name).

use foundry_acceptance::support::external_integration_plugins as eip;
use foundry_acceptance::support::matrix_homeserver as mx;
use foundry_acceptance::support::slack_double::{Approval, Mood, SlackTeam};
use foundry_acceptance::support::team_membership as tm;
use foundry_acceptance::world::FoundryWorld;
use futures::future::BoxFuture;
use futures::FutureExt;
use reqwest::StatusCode;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

/// The room the Matrix cases route to.
const ROOM: &str = "#auth:canzan.dev";
/// The bot's removal, as the route reads it (US-EIP-09, scenario 136).
const BOT_REMOVED: &str = "Needs attention: @foundry:canzan.dev was removed from #auth:canzan.dev. Invite it back, then Resume.";
/// K-18 (DESIGN UI-1, DD-6; ratified 2026-10-22).
const K18: &str = "Matrix is not set up on this Foundry yet. An instance operator sets it up once.";
/// K-19 with {Kind} = Matrix (DESIGN UI-1, DD-2; ratified 2026-10-22).
const K19_MATRIX: &str = "Matrix is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key.";
/// R-12 (ratified 2026-10-25) around the archived-channel reason words.
const R12_ARCHIVED: &str = "The test message was not delivered: #auth-dev was archived.";
/// A case's bound: setup, the homeserver, and the status budget.
const CASE_BOUND: Duration = Duration::from_secs(300);

type Body = fn(&mut FoundryWorld) -> BoxFuture<'_, ()>;

struct Case {
    name: &'static str,
    ignore: Option<&'static str>,
    body: Body,
}

const CASES: &[Case] = &[
    Case {
        name: "a1_refused_matrix_route_resumes_once_the_bot_is_back_with_slack_connected",
        ignore: None,
        body: |w| a1_resume(w, true).boxed(),
    },
    Case {
        name: "a1_refused_matrix_route_resumes_once_the_bot_is_back_without_slack",
        ignore: None,
        body: |w| a1_resume(w, false).boxed(),
    },
    Case {
        name: "b1_a_save_the_lane_check_refuses_never_takes_the_bot_out_of_a_routed_room",
        ignore: None,
        body: |w| b1_lane_gone(w).boxed(),
    },
    Case {
        name: "b1_a_failed_room_read_never_takes_the_bot_out_of_a_routed_room",
        ignore: None,
        body: |w| b1_room_read_fails(w).boxed(),
    },
    Case {
        name: "b2_a_route_delete_never_leaves_the_bot_in_a_room_no_route_uses",
        ignore: None,
        body: |w| b2_delete_reads_fail(w).boxed(),
    },
    Case {
        name: "b3_send_test_never_posts_to_a_private_channel_foundry_is_not_in",
        ignore: None,
        body: |w| b3_send_test_refused(w, Unoffered::PrivateChannel).boxed(),
    },
    Case {
        name: "b3_send_test_never_posts_a_direct_message",
        ignore: None,
        body: |w| b3_send_test_refused(w, Unoffered::DirectMessage).boxed(),
    },
    Case {
        name: "b4i_a_failed_slack_send_test_names_the_channel",
        ignore: None,
        body: |w| b4_send_test_names_channel(w).boxed(),
    },
    Case {
        name: "b4ii_matrix_without_a_secrets_key_reads_k18",
        ignore: None,
        body: |w| b4_matrix_without_key(w).boxed(),
    },
    Case {
        name: "b4iii_a_matrix_connection_the_key_cannot_open_reads_k19",
        ignore: None,
        body: |w| b4_matrix_unreadable(w).boxed(),
    },
    Case {
        name: "r1_deleting_the_only_route_to_a_room_takes_the_bot_out_of_it",
        ignore: None,
        body: |w| r1_delete_leaves_the_room(w).boxed(),
    },
    Case {
        name: "r1_a_refused_save_to_a_room_no_route_uses_takes_the_bot_back_out",
        ignore: None,
        body: |w| r1_refused_save_leaves_the_room(w).boxed(),
    },
    Case {
        name: "r2a_a_project_email_without_its_stop_link_is_failed_not_sent",
        ignore: None,
        body: |w| r2a_no_stop_link(w).boxed(),
    },    Case {
        name: "fu11_a_slack_create_on_an_unreadable_connection_reads_k19_and_saves_nothing",
        ignore: None,
        body: |w| fu11_create(w, eip::Kind::Slack).boxed(),
    },
    Case {
        name: "fu11_a_slack_edit_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Slack, RouteWrite::Edit).boxed(),
    },
    Case {
        name: "fu11_a_slack_pause_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Slack, RouteWrite::Pause).boxed(),
    },
    Case {
        name: "fu11_a_slack_resume_of_a_paused_route_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Slack, RouteWrite::ResumePaused).boxed(),
    },
    Case {
        name: "fu11_a_slack_resume_of_a_refusal_stopped_route_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Slack, RouteWrite::ResumeStopped).boxed(),
    },
    Case {
        name: "fu11_a_slack_delete_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Slack, RouteWrite::Delete).boxed(),
    },
    Case {
        name: "fu11_a_matrix_create_on_an_unreadable_connection_reads_k19_and_saves_nothing",
        ignore: None,
        body: |w| fu11_create(w, eip::Kind::Matrix).boxed(),
    },
    Case {
        name: "fu11_a_matrix_edit_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Matrix, RouteWrite::Edit).boxed(),
    },
    Case {
        name: "fu11_a_matrix_pause_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Matrix, RouteWrite::Pause).boxed(),
    },
    Case {
        name: "fu11_a_matrix_resume_of_a_paused_route_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Matrix, RouteWrite::ResumePaused).boxed(),
    },
    Case {
        name: "fu11_a_matrix_resume_of_a_refusal_stopped_route_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Matrix, RouteWrite::ResumeStopped).boxed(),
    },
    Case {
        name: "fu11_a_matrix_delete_on_an_unreadable_connection_reads_k19_and_writes_nothing",
        ignore: None,
        body: |w| fu11_route_write(w, eip::Kind::Matrix, RouteWrite::Delete).boxed(),
    },
    Case {
        name: "fu11_b_slack_send_test_on_an_unreadable_connection_reads_k19_and_sends_nothing",
        ignore: None,
        body: |w| fu11_send_test(w, eip::Kind::Slack).boxed(),
    },
    Case {
        name: "fu11_b_matrix_send_test_on_an_unreadable_connection_reads_k19_and_sends_nothing",
        ignore: None,
        body: |w| fu11_send_test(w, eip::Kind::Matrix).boxed(),
    },
    Case {
        name: "fu11_c_slack_disconnect_of_an_unreadable_connection_warns_and_forgets_it",
        ignore: None,
        body: |w| fu11_disconnect(w, eip::Kind::Slack).boxed(),
    },
    Case {
        name: "fu11_c_matrix_disconnect_of_an_unreadable_connection_forgets_it",
        ignore: None,
        body: |w| fu11_disconnect(w, eip::Kind::Matrix).boxed(),
    },
    Case {
        name: "fu11_c_slack_disconnect_of_a_readable_connection_keeps_its_question",
        ignore: None,
        body: |w| fu11_readable_disconnect_question(w).boxed(),
    },
    Case {
        name: "fu11_c_slack_page_offers_disconnect_beside_k19",
        ignore: None,
        body: |w| fu11_repair_doors(w, eip::Kind::Slack).boxed(),
    },
    Case {
        name: "fu11_c_matrix_page_offers_disconnect_and_replace_token_beside_k19",
        ignore: None,
        body: |w| fu11_repair_doors(w, eip::Kind::Matrix).boxed(),
    },
    Case {
        name: "fu11_c_replacing_the_matrix_token_repairs_an_unreadable_connection",
        ignore: None,
        body: |w| fu11_replace_token(w).boxed(),
    },
    Case {
        name: "fu11_d_matrix_with_its_secrets_key_removed_reads_k19_not_k18",
        ignore: None,
        body: |w| fu11_key_removed(w).boxed(),
    },
    Case {
        name: "fu11_e_slack_unavailable_is_judged_per_connection",
        ignore: None,
        body: |w| fu11_per_connection(w, eip::Kind::Slack).boxed(),
    },
    Case {
        name: "fu11_e_matrix_unavailable_is_judged_per_connection",
        ignore: None,
        body: |w| fu11_per_connection(w, eip::Kind::Matrix).boxed(),
    },
];

// ===========================================================================
// Setup: the features' Background, without its tables
// ===========================================================================

fn row(person: &str, role: &str, teams: &str) -> Vec<String> {
    vec![person.into(), role.into(), teams.into()]
}

/// Canzan Labs with its people, Identity Platform (Backend) and Homelab Ops
/// (Ops), at 14:02 on 21 Oct 2026.
async fn workspace(world: &mut FoundryWorld) {
    eip::seed_workspace_people(
        world,
        &[
            row("Priya Raman", "admin", "Backend as lead, Ops as lead"),
            row("Lena Park", "member", "Backend as lead"),
            row("Sam Okafor", "member", "Backend as member"),
        ],
    )
    .await;
    eip::given_projects(
        world,
        &[
            ("Identity Platform".into(), "AUTH".into(), "Backend".into()),
            ("Homelab Ops".into(), "OPS".into(), "Ops".into()),
        ],
    )
    .await;
    eip::given_time(world, "14:02");
}

/// The 04 matrix feature's Background and its first Given: the homeserver,
/// Lena's room, Matrix connected and the bot invited.
async fn matrix_connected(world: &mut FoundryWorld) {
    eip::given_homeserver_accounts(world, "@foundry:canzan.dev", "@lena:canzan.dev").await;
    eip::given_room(world, ROOM, Some("Identity Platform"), 5, false).await;
    eip::given_matrix_connected(world).await;
    eip::invite_bot(world, ROOM).await;
}

async fn slack_connected(world: &mut FoundryWorld) {
    eip::given_instance(world, "the Foundry Slack app set up").await;
    eip::given_slack_connected(world).await;
}

fn new_issue_route(project: &str, destination: &str) -> eip::Route {
    eip::route_from("New issue", "", project, destination, "")
}

fn notice(answer: &tm::Answer) -> Option<String> {
    let doc = scraper::Html::parse_document(&answer.body);
    let selector = scraper::Selector::parse("[data-route-notice]").expect("selector");
    doc.select(&selector)
        .next()
        .map(|e| e.text().collect::<Vec<_>>().join(" ").trim().to_string())
}

fn section_text(answer: &tm::Answer, integration: &str) -> String {
    let doc = scraper::Html::parse_document(&answer.body);
    let selector =
        scraper::Selector::parse(&format!("section[data-integration=\"{integration}\"]"))
            .expect("selector");
    doc.select(&selector)
        .next()
        .map(|e| {
            e.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| {
            panic!(
                "the page has its {integration} section; {}",
                tm::describe(answer)
            )
        })
}

// ===========================================================================
// A1: a refused Matrix route can be resumed
// ===========================================================================

/// A1 (services `problem_cleared` re-checks every route through the Slack
/// connection; `DestinationCatalogs::catalogue` always builds a Slack bot).
/// A Matrix route stopped because the bot was removed from its room resumes
/// once the room reads the bot as a member that can post: Lena chooses
/// Resume and the route reads "Delivering". Both with the workspace's Slack
/// connected (today Slack is asked about a Matrix room) and without it
/// (today nothing is asked and the route stays stopped).
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09 (a team lead routes a
/// project to a Matrix room and the bot's problems are explained and fixable).
async fn a1_resume(world: &mut FoundryWorld, with_slack: bool) {
    workspace(world).await;
    if with_slack {
        slack_connected(world).await;
    }
    matrix_connected(world).await;
    eip::given_routed(world, "Lena", new_issue_route("Identity Platform", ROOM)).await;
    eip::kick_bot(world, ROOM).await;
    eip::given_filed(
        world,
        "Sam",
        "Identity Platform",
        "Rotate LLDAP bind password",
        false,
    )
    .await;
    eip::then_route_reads(world, ROOM, BOT_REMOVED, false).await;
    // Lena invites the bot back and it is in the room again.
    eip::invite_bot(world, ROOM).await;
    eip::bot_joins(world, ROOM).await;
    assert!(
        eip::bot_is_member(world, ROOM).await,
        "precondition: the room reads the bot as a member"
    );

    eip::when_route_action(world, "Lena", "resume", ROOM).await;

    eip::then_route_reads(world, ROOM, "Delivering", false).await;
}

// ===========================================================================
// B1: a failed Matrix save never takes the bot out of a room another route uses
// ===========================================================================

/// Homelab Ops already routes to the room (Priya, the Ops lead): the bot is
/// in it on that route's behalf.
async fn room_routed_by_homelab_ops(world: &mut FoundryWorld) {
    workspace(world).await;
    matrix_connected(world).await;
    eip::given_routed(world, "Priya", new_issue_route("Homelab Ops", ROOM)).await;
    assert!(
        eip::bot_is_member(world, ROOM).await,
        "precondition: the bot is in the room for Homelab Ops' route"
    );
}

/// After a failed save to the room: the bot never left it, and Homelab Ops'
/// route still reads as it did.
async fn bot_stayed(world: &mut FoundryWorld, leaves_before: usize) {
    let leaves = eip::fault_proxy(world).requests_to("/leave") - leaves_before;
    assert_eq!(
        leaves, 0,
        "Foundry asked the homeserver to take the bot out of {ROOM}, which Homelab Ops still routes to"
    );
    assert!(
        eip::bot_is_member(world, ROOM).await,
        "the bot is still in {ROOM} for Homelab Ops' route"
    );
}

/// B1 (handlers `create_matrix_route`: `bot.withdraw` on any `create_route`
/// error). Lena opens a route form from Identity Platform to the room with the
/// lane QA chosen; QA is deleted before she saves, so the save is refused.
/// The bot, which another project's route keeps in the room, stays there.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09.
async fn b1_lane_gone(world: &mut FoundryWorld) {
    room_routed_by_homelab_ops(world).await;
    eip::given_lanes(
        world,
        "Identity Platform",
        "Backlog, Todo, In progress, QA, Done",
    )
    .await;
    eip::given_creating(
        world,
        "Lena",
        eip::route_from("Issue moved", "QA", "Identity Platform", ROOM, ""),
    )
    .await;
    eip::delete_lane(world, "Lena", "QA", None).await;
    let leaves_before = eip::fault_proxy(world).requests_to("/leave");

    eip::when_save_route(world, "Lena").await;

    eip::then_no_route(world, "Identity Platform").await;
    bot_stayed(world, leaves_before).await;
}

/// B1 (handlers `enrol_in_room`: withdraw on any refusal but NotInvited,
/// a transient read failure included). Lena saves a route from Identity
/// Platform to the room; the homeserver fails the room's member read once
/// (502). The save is refused, and the bot stays in the room.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09.
async fn b1_room_read_fails(world: &mut FoundryWorld) {
    room_routed_by_homelab_ops(world).await;
    eip::given_creating(world, "Lena", new_issue_route("Identity Platform", ROOM)).await;
    let leaves_before = eip::fault_proxy(world).requests_to("/leave");
    eip::fault_proxy(world).arm("/joined_members", mx::Fault::ServerError);

    eip::when_save_route(world, "Lena").await;

    assert!(
        eip::fault_proxy(world).fault_given(),
        "precondition: the save read the room and the homeserver failed that read"
    );
    eip::then_no_route(world, "Identity Platform").await;
    bot_stayed(world, leaves_before).await;
}

// ===========================================================================
// B2: delete_route never deletes without leaving the room
// ===========================================================================

/// The store fault: the route's settings read fails (its lane-filter table
/// loses the column that read joins on); the delete itself does not read it.
/// Injected into this scenario's own schema from the test, not a product seam.
const BREAK_SETTINGS_READ: &str =
    "ALTER TABLE integration_route_lanes RENAME COLUMN lane_id TO lane_id_review_b2";
const MEND_SETTINGS_READ: &str =
    "ALTER TABLE integration_route_lanes RENAME COLUMN lane_id_review_b2 TO lane_id";

/// B2 (handlers `delete_route` falls back to `change_route(Delete)` when
/// `managed_route` errs). Identity Platform's route is the only one to the
/// room; Lena confirms its delete while the route's settings cannot be read.
/// Either the route is still there, or it is gone and the bot left the room:
/// never a deleted route with the bot left behind in a room no route uses.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09.
async fn b2_delete_reads_fail(world: &mut FoundryWorld) {
    workspace(world).await;
    matrix_connected(world).await;
    eip::given_routed(world, "Lena", new_issue_route("Identity Platform", ROOM)).await;
    let dialog = eip::delete_dialog(world, "Lena", ROOM).await;
    let confirm = tm::forms(&dialog.body)
        .into_iter()
        .find(|f| f.action.ends_with("/delete"))
        .unwrap_or_else(|| {
            panic!(
                "the delete dialog's confirm form; {}",
                tm::describe(&dialog)
            )
        });
    let pool = tm::pool(world);
    sqlx::query(BREAK_SETTINGS_READ)
        .execute(&pool)
        .await
        .expect("break the route settings read");

    let answer = eip::submit_form(world, "Lena", &confirm.action, &confirm.fields).await;

    sqlx::query(MEND_SETTINGS_READ)
        .execute(&pool)
        .await
        .expect("mend the route settings read");
    let listed = eip::route_status(world, "Identity Platform", ROOM)
        .await
        .is_some();
    let member = eip::bot_is_member(world, ROOM).await;
    assert!(
        listed || !member,
        "the route to {ROOM} was deleted and the bot was left in a room no route uses \
         (the delete answered {})",
        answer.status
    );
}

// ===========================================================================
// B3: Send test checks the destination the way save does
// ===========================================================================

#[derive(Clone, Copy)]
enum Unoffered {
    /// #backend-private: private, and Foundry is not in it.
    PrivateChannel,
    /// A member's id: chat.postMessage would open a direct message.
    DirectMessage,
}

const SAM_IN_SLACK: &str = "U0SAMOKAF";

/// B3 (handlers `send_test` posts K-7 to `form.destination` without the
/// `postable_destination` check save makes). Lena submits Send test for a
/// destination the channel picker does not offer. Slack is asked to post
/// nothing, and she gets the answer Save gives for the same destination.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-02 (a team lead tests a
/// route before saving it, and only to a channel Foundry may post to).
async fn b3_send_test_refused(world: &mut FoundryWorld, destination: Unoffered) {
    workspace(world).await;
    slack_connected(world).await;
    let slack = eip::slack_double(world);
    let reference = match destination {
        Unoffered::PrivateChannel => slack.channel_id("#backend-private"),
        Unoffered::DirectMessage => SAM_IN_SLACK.to_string(),
    };
    let integrations = eip::integrations_path(world, "Identity Platform");
    let save_fields = vec![
        ("kind".to_string(), "slack".to_string()),
        ("destination".to_string(), reference.clone()),
        ("events".to_string(), "issue.created".to_string()),
        ("content_level".to_string(), "summary".to_string()),
    ];
    let saved = eip::submit_form(
        world,
        "Lena",
        &format!("{integrations}/routes"),
        &save_fields,
    )
    .await;
    assert_ne!(
        saved.status,
        StatusCode::SEE_OTHER,
        "precondition: Save refuses this destination; {}",
        tm::describe(&saved)
    );

    let tested = eip::send_test_for_reference(world, "Lena", "Identity Platform", &reference).await;

    let asked: Vec<String> = slack
        .all_posts()
        .into_iter()
        .filter(|p| p.channel == reference)
        .map(|p| p.all_text())
        .collect();
    assert!(
        asked.is_empty(),
        "Slack was asked to post to {reference}, which the picker never offers: {asked:?}"
    );
    assert_eq!(
        (tested.status, notice(&tested)),
        (saved.status, notice(&saved)),
        "Send test answers as Save does for {reference}"
    );
}

// ===========================================================================
// B4: copy drift
// ===========================================================================

/// B4 (i) (handlers `send_test`: `test_notice(outcome, "the channel")`).
/// Slack refuses Send test's post because #auth-dev was archived meanwhile;
/// the notice names the channel as the picker showed it ("#auth-dev was
/// archived", the ratified reason words), never "the channel", inside the
/// wrapper the user ratified on 2026-10-25 (R-12): "The test message was not
/// delivered: {reason}."
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-02.
async fn b4_send_test_names_channel(world: &mut FoundryWorld) {
    workspace(world).await;
    slack_connected(world).await;
    let slack = eip::slack_double(world);
    slack.queue_post_mood(
        "#auth-dev",
        Mood::Refuses {
            code: "is_archived".into(),
            text: None,
        },
    );
    let reference = slack.channel_id("#auth-dev");

    let tested = eip::send_test_for_reference(world, "Lena", "Identity Platform", &reference).await;

    let said = notice(&tested).unwrap_or_default();
    assert!(
        said.contains(R12_ARCHIVED),
        "Send test's failure reads R-12 {R12_ARCHIVED:?}; it reads {said:?}"
    );
    assert!(
        said.contains("#auth-dev was archived"),
        "Send test's failure names #auth-dev; it reads {said:?}"
    );
    assert!(
        !said.contains("the channel"),
        "Send test's failure names the channel, not \"the channel\"; it reads {said:?}"
    );
}

/// B4 (ii) (workspace.html: the Matrix section's no-key line is not K-18).
/// With no secrets key, the workspace integrations page's Matrix section
/// reads K-18 exactly.
///
/// CONTRACT_SHAPE: pure-function
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-08 (a workspace admin
/// connects Foundry's Matrix bot, or learns why it cannot be connected yet).
async fn b4_matrix_without_key(world: &mut FoundryWorld) {
    workspace(world).await;
    eip::restart_in_process_with_key(world, None).await;

    eip::when_open(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;

    let page = world.eip.when_answer.clone().expect("the page");
    let matrix = section_text(&page, "matrix");
    assert!(
        matrix.contains(K18),
        "the Matrix section reads K-18 {K18:?}; it reads {matrix:?}"
    );
}

/// B4 (iii) (no K-19 for Matrix). Foundry restarts with a secrets key that
/// cannot open the saved Matrix connection: the Matrix section reads K-19
/// with {Kind} = Matrix exactly.
///
/// CONTRACT_SHAPE: pure-function
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-08.
async fn b4_matrix_unreadable(world: &mut FoundryWorld) {
    workspace(world).await;
    matrix_connected(world).await;
    eip::restart_in_process_with_key(world, Some(eip::NEW_SECRETS_KEY)).await;

    eip::when_open(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;

    let page = world.eip.when_answer.clone().expect("the page");
    let matrix = section_text(&page, "matrix");
    assert!(
        matrix.contains(K19_MATRIX),
        "the Matrix section reads K-19 {K19_MATRIX:?}; it reads {matrix:?}"
    );
}

// ===========================================================================
// R1 mutation gaps: the bot leaves a room no route uses (passing today)
// ===========================================================================

/// After a delete or a refused save: Foundry asked the homeserver to take
/// the bot out of the room, and the room no longer reads it as a member.
async fn bot_left(world: &mut FoundryWorld, leaves_before: usize) {
    let leaves = eip::fault_proxy(world).requests_to("/leave") - leaves_before;
    assert!(
        leaves >= 1,
        "Foundry never asked the homeserver to take the bot out of {ROOM}, which no route uses"
    );
    assert!(
        !eip::bot_is_member(world, ROOM).await,
        "the bot is still in {ROOM}, which no route uses"
    );
}

/// R1 mutation gap (handlers `delete_route` -> `Default::default()`; the
/// leave after a delete). Identity Platform's route is the only one to the
/// room; Lena confirms its delete. She is sent back to the project's page,
/// the route is gone, and the bot left the room.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09.
async fn r1_delete_leaves_the_room(world: &mut FoundryWorld) {
    workspace(world).await;
    matrix_connected(world).await;
    eip::given_routed(world, "Lena", new_issue_route("Identity Platform", ROOM)).await;
    assert!(
        eip::bot_is_member(world, ROOM).await,
        "precondition: the bot is in the room for the route"
    );
    let dialog = eip::delete_dialog(world, "Lena", ROOM).await;
    let confirm = tm::forms(&dialog.body)
        .into_iter()
        .find(|f| f.action.ends_with("/delete"))
        .unwrap_or_else(|| {
            panic!(
                "the delete dialog's confirm form; {}",
                tm::describe(&dialog)
            )
        });
    let leaves_before = eip::fault_proxy(world).requests_to("/leave");

    let answer = eip::submit_form(world, "Lena", &confirm.action, &confirm.fields).await;

    assert_eq!(
        answer.status,
        StatusCode::SEE_OTHER,
        "the delete sends Lena back to the project's page; {}",
        tm::describe(&answer)
    );
    assert!(
        eip::route_status(world, "Identity Platform", ROOM)
            .await
            .is_none(),
        "Identity Platform's route to {ROOM} is deleted"
    );
    bot_left(world, leaves_before).await;
}

/// R1 mutation gaps (handlers `withdraw_unless_routed` -> `()`, and
/// `enrol_in_room`'s `refusal != RoomRefusal::NotInvited`). The bot is
/// invited to an encrypted room no route uses; Lena saves a route to it. The
/// bot joins, finds the room encrypted, and the save is refused; the bot,
/// which nothing keeps there, leaves.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-09.
async fn r1_refused_save_leaves_the_room(world: &mut FoundryWorld) {
    workspace(world).await;
    eip::given_homeserver_accounts(world, "@foundry:canzan.dev", "@lena:canzan.dev").await;
    eip::given_room(world, ROOM, Some("Identity Platform"), 5, true).await;
    eip::given_matrix_connected(world).await;
    eip::invite_bot(world, ROOM).await;
    eip::given_creating(world, "Lena", new_issue_route("Identity Platform", ROOM)).await;
    let leaves_before = eip::fault_proxy(world).requests_to("/leave");

    eip::when_save_route(world, "Lena").await;

    eip::then_no_route(world, "Identity Platform").await;
    bot_left(world, leaves_before).await;
}

// ===========================================================================
// R2-a: a project email never goes out without its stop link
// ===========================================================================

/// The status of the latest project-email delivery to `email`, once it is
/// no longer pending (`None` if none settles within the email budget).
/// Crate-level oracle: project emails have no status page.
async fn settled_email_delivery(world: &FoundryWorld, email: &str) -> Option<String> {
    let pool = tm::pool(world);
    let start = std::time::Instant::now();
    while start.elapsed() < eip::EMAIL_BUDGET {
        let status: Option<String> = sqlx::query_scalar(
            "SELECT d.status FROM integration_deliveries d
               JOIN users u ON u.id = d.recipient_user_id
              WHERE u.email_lower = lower($1)
              ORDER BY d.id DESC LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&pool)
        .await
        .expect("read the email delivery");
        if let Some(status) = status.filter(|s| s != "pending") {
            return Some(status);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    None
}

/// R2-a (review part 2, non-blocking: "missing stop link sends the email
/// silently"; the user chose to fail it, 2026-10-25). Sam chose New issue for
/// all issues in Identity Platform. The delivery worker's project mail can
/// send but cannot build a stop link (it was never handed the secret stop
/// links are signed with). Lena files an issue: Sam is not emailed, and the
/// delivery is recorded as failed. A project email never goes out without
/// its "Stop these emails" footer and its List-Unsubscribe headers. RED
/// today: the email is sent without them and recorded as delivered.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-11 (a member stops one kind
/// of project email from the email itself).
async fn r2a_no_stop_link(world: &mut FoundryWorld) {
    workspace(world).await;
    eip::given_email_setup(world, true).await;
    eip::given_chosen(world, "Sam", "New issue", "all issues", "Identity Platform").await;
    eip::run_worker_without_stop_links(world);
    let sam = tm::resolve_person(world, "Sam").email;

    eip::file(
        world,
        "Lena",
        "Identity Platform",
        "Rotate the bind password",
    )
    .await;

    let settled = settled_email_delivery(world, &sam).await;
    let received: Vec<(String, bool)> = eip::received_emails(world, &sam)
        .into_iter()
        .map(|e| (e.subject, e.body.contains("Stop these emails")))
        .collect();
    assert!(
        received.is_empty(),
        "Sam was emailed although no stop link could be built ((subject, carries a stop \
         link) per email): {received:?}"
    );
    assert_eq!(
        settled.as_deref(),
        Some("failed"),
        "Sam's New issue email is recorded as failed (it reads {settled:?})"
    );
}

// ===========================================================================
// FU-11: a connection this Foundry cannot read refuses route writes (DD-2,
// amended 2026-10-27): K-19 for every write; Disconnect and Replace token
// stay open as repairs; the unavailable state is judged per connection
// ===========================================================================

/// The channel the Slack cases route to.
const CHANNEL: &str = "#auth-dev";
/// K-19 with {Kind} = Slack (DESIGN UI-1, DD-2; ratified 2026-10-22, R-2).
const K19_SLACK: &str = "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key.";
/// R-13 (ratified by the user 2026-10-27): the Slack disconnect question of
/// a connection this Foundry cannot read, for Canzan Labs. Foundry cannot
/// open the bot token, so it cannot ask Slack to withdraw its access
/// (`apps.uninstall`); the admin removes the app in Slack. It replaces the
/// question's last sentence ([`ACCESS_REMOVED_CANZAN_LABS`]) there, and only
/// there.
const R13_CANZAN_LABS: &str =
    "Foundry cannot remove its access to Canzan Labs' Slack: remove the Foundry app in Slack by hand.";
/// The last sentence of US-EIP-07's Slack disconnect question, for Canzan
/// Labs: true only when Foundry can ask Slack to withdraw its access.
const ACCESS_REMOVED_CANZAN_LABS: &str = "Foundry's access to Canzan Labs' Slack is removed.";
/// The second workspace of the per-connection cases (`tm::FOREIGN`).
const GLOBEX_PROJECT: &str = "Globex Platform";
/// Globex's Matrix rooms (the per-connection Matrix case).
const GLOBEX_ROOM: &str = "#globex:canzan.dev";
const GLOBEX_OPS_ROOM: &str = "#globex-ops:canzan.dev";

/// A route write the FU-11 cases try on a connection this Foundry cannot read.
#[derive(Clone, Copy, Debug)]
enum RouteWrite {
    Edit,
    Pause,
    /// Resume of a route Lena paused.
    ResumePaused,
    /// Resume of a route a refusal stopped (Slack: the channel was
    /// archived; Matrix: the bot was removed from the room).
    ResumeStopped,
    Delete,
}

fn k19(kind: eip::Kind) -> &'static str {
    match kind {
        eip::Kind::Slack => K19_SLACK,
        eip::Kind::Matrix => K19_MATRIX,
    }
}

fn destination(kind: eip::Kind) -> &'static str {
    match kind {
        eip::Kind::Slack => CHANNEL,
        eip::Kind::Matrix => ROOM,
    }
}

/// Canzan Labs' `kind` connection, made by Priya through the product.
async fn connected(world: &mut FoundryWorld, kind: eip::Kind) {
    match kind {
        eip::Kind::Slack => slack_connected(world).await,
        eip::Kind::Matrix => matrix_connected(world).await,
    }
}

/// Canzan Labs with its `kind` connection, and Lena's New issue route from
/// Identity Platform to it (#auth-dev, or the Matrix room).
async fn routed(world: &mut FoundryWorld, kind: eip::Kind) {
    workspace(world).await;
    connected(world, kind).await;
    eip::given_routed(
        world,
        "Lena",
        new_issue_route("Identity Platform", destination(kind)),
    )
    .await;
}

/// The route stopped by a refusal: #auth-dev was archived, or the bot was
/// removed from the room, and Sam's issue met the refusal.
async fn stopped_by_refusal(world: &mut FoundryWorld, kind: eip::Kind) {
    match kind {
        eip::Kind::Slack => eip::slack_next_answer(world, CHANNEL, eip::SlackAnswer::Archived),
        eip::Kind::Matrix => eip::kick_bot(world, ROOM).await,
    }
    eip::given_filed(
        world,
        "Sam",
        "Identity Platform",
        "Rotate LLDAP bind password",
        false,
    )
    .await;
    eip::then_route_reads_prefix(world, destination(kind), "Needs attention").await;
}

/// Foundry restarts with a secrets key that cannot open the saved
/// connections (they were sealed under the key it was started with).
async fn unreadable(world: &mut FoundryWorld) {
    eip::restart_in_process_with_key(world, Some(eip::NEW_SECRETS_KEY)).await;
}

/// Sam files an issue and its delivery waits on the unreadable connection:
/// the route has something queued for a write to keep or drop.
async fn delivery_waiting(world: &mut FoundryWorld) {
    eip::file(
        world,
        "Sam",
        "Identity Platform",
        "Rotate the bind password",
    )
    .await;
    let pool = tm::pool(world);
    let workspace = tm::workspace_id(world, tm::HOME);
    let start = std::time::Instant::now();
    while start.elapsed() < eip::STATUS_BUDGET {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM integration_deliveries d
               JOIN integration_routes r ON r.id = d.route_id
              WHERE r.workspace_id = $1 AND d.status = 'pending'",
        )
        .bind(workspace)
        .fetch_one(&pool)
        .await
        .expect("read the waiting deliveries");
        if waiting > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!(
        "precondition: Sam's issue has a delivery waiting on the route within {:?}",
        eip::STATUS_BUDGET
    );
}

/// What a route or connection write leaves in the store for `workspace`:
/// its connections (the credential as a digest, never the bytes), routes,
/// route deliveries and audit records. Crate-level oracle: the store is
/// where "writes nothing" is decided; the pages show a projection of it.
async fn stored(world: &FoundryWorld, workspace: &str) -> Vec<String> {
    let pool = tm::pool(world);
    let id = tm::workspace_id(world, workspace);
    let mut lines = Vec::new();
    for sql in [
        "SELECT format('connection %s %s %s %s', id, kind, state,
                       md5(coalesce(sealed_credential, ''::bytea)))
           FROM integration_connections WHERE workspace_id = $1 ORDER BY id",
        "SELECT format('route %s %s %s %s paused=%s problem=%s receives_from=%s lanes=%s',
                       r.id, r.kind, r.events::text, r.content_level, r.paused_at IS NOT NULL,
                       coalesce(r.problem_reason, '-'), r.receives_from,
                       (SELECT count(*) FROM integration_route_lanes l WHERE l.route_id = r.id))
           FROM integration_routes r WHERE r.workspace_id = $1 ORDER BY r.id",
        "SELECT format('delivery %s route=%s %s', d.id, d.route_id, d.status)
           FROM integration_deliveries d JOIN integration_routes r ON r.id = d.route_id
          WHERE r.workspace_id = $1 ORDER BY d.id",
        "SELECT format('audit %s %s %s', action, kind, subject)
           FROM integration_audit_events WHERE workspace_id = $1 ORDER BY created_at, id",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sql)
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap_or_else(|err| panic!("read the stored integrations: {err}"));
        lines.extend(rows);
    }
    lines
}

/// `None` when `after` is `before`; else what was written and what went.
fn written(before: &[String], after: &[String]) -> Option<String> {
    if before == after {
        return None;
    }
    let gone: Vec<&String> = before.iter().filter(|l| !after.contains(l)).collect();
    let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
    Some(format!("written {new:?}, gone {gone:?}"))
}

/// The requests Foundry made to take the bot out of a room (Matrix only).
fn leaves(world: &FoundryWorld, kind: eip::Kind) -> usize {
    match kind {
        eip::Kind::Slack => 0,
        eip::Kind::Matrix => eip::fault_proxy(world).requests_to("/leave"),
    }
}

/// What Foundry sent towards the destination: Slack posts, or homeserver
/// sends.
fn sent(world: &FoundryWorld, kind: eip::Kind) -> usize {
    match kind {
        eip::Kind::Slack => eip::slack_double(world).all_posts().len(),
        eip::Kind::Matrix => eip::fault_proxy(world).requests_to(mx::SEND_PATH),
    }
}

/// The page text of `answer`, whitespace folded.
fn page(answer: &tm::Answer) -> String {
    tm::page_text(&answer.body)
}

/// Every part of a refused write that did not hold; empty when it was
/// refused with K-19 and nothing was written or asked of the homeserver.
async fn refusal_problems(
    world: &mut FoundryWorld,
    kind: eip::Kind,
    answer: &tm::Answer,
    before: &[String],
    leaves_before: usize,
) -> Vec<String> {
    let mut problems = Vec::new();
    let read = page(answer);
    if !read.contains(k19(kind)) {
        problems.push(format!(
            "the answer reads K-19 {:?}; {}",
            k19(kind),
            tm::describe(answer)
        ));
    }
    if let Some(what) = written(before, &stored(world, tm::HOME).await) {
        problems.push(format!("nothing is written; {what}"));
    }
    let left = leaves(world, kind) - leaves_before;
    if left != 0 {
        problems.push(format!(
            "Foundry asked the homeserver to take the bot out of a room {left} time(s)"
        ));
    }
    problems
}

fn assert_none(what: &str, problems: &[String]) {
    assert!(
        problems.is_empty(),
        "{what}:\n  - {}",
        problems.join("\n  - ")
    );
}

/// FU-11 (a), create. Canzan Labs' `kind` connection was sealed under a key
/// this Foundry no longer holds. Lena saves a New issue route from Identity
/// Platform to #auth-dev (or the room): the answer reads K-19 for that kind,
/// no route or audit record is saved, and the bot is never taken out of a
/// room. RED today: the save answers K-4 or "Foundry cannot post to that
/// channel", not K-19.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-02 / US-EIP-09 (a team lead
/// routes a project, or learns why it cannot be routed now).
async fn fu11_create(world: &mut FoundryWorld, kind: eip::Kind) {
    workspace(world).await;
    connected(world, kind).await;
    unreadable(world).await;
    let destination_ref = match kind {
        eip::Kind::Slack => eip::slack_double(world).channel_id(CHANNEL),
        eip::Kind::Matrix => world.eip.names.real(ROOM),
    };
    let fields = vec![
        ("kind".to_string(), kind.wire().to_string()),
        ("destination".to_string(), destination_ref),
        ("events".to_string(), "issue.created".to_string()),
        ("content_level".to_string(), "summary".to_string()),
    ];
    let before = stored(world, tm::HOME).await;
    let leaves_before = leaves(world, kind);
    let routes = format!(
        "{}/routes",
        eip::integrations_path(world, "Identity Platform")
    );

    let answer = eip::submit_form(world, "Lena", &routes, &fields).await;

    let problems = refusal_problems(world, kind, &answer, &before, leaves_before).await;
    assert_none(
        &format!("{kind:?} create on a connection Foundry cannot read"),
        &problems,
    );
}

/// FU-11 (a), edit / pause / resume / delete. Identity Platform's route to
/// #auth-dev (or the room) runs on Canzan Labs' `kind` connection, which was
/// sealed under a key this Foundry no longer holds. For edit, pause and
/// delete Sam's issue is waiting on the route. Lena makes the write: the
/// answer reads K-19 for that kind, and nothing is written: the route as it
/// was, its deliveries untouched, no audit record, and the bot never taken
/// out of the room. RED today: every write except a refusal-stopped resume
/// goes through (DD-2 says none is admitted), and none answers K-19.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-07 (a team lead manages a
/// route, and nothing changes behind a connection Foundry cannot read).
async fn fu11_route_write(world: &mut FoundryWorld, kind: eip::Kind, write: RouteWrite) {
    routed(world, kind).await;
    let dest = destination(kind);
    match write {
        RouteWrite::ResumePaused => eip::given_route_action(world, "Lena", "pause", dest).await,
        RouteWrite::ResumeStopped => stopped_by_refusal(world, kind).await,
        RouteWrite::Edit | RouteWrite::Pause | RouteWrite::Delete => {}
    }
    unreadable(world).await;
    if matches!(
        write,
        RouteWrite::Edit | RouteWrite::Pause | RouteWrite::Delete
    ) {
        delivery_waiting(world).await;
    }
    let before = stored(world, tm::HOME).await;
    let leaves_before = leaves(world, kind);

    let answer = match write {
        RouteWrite::Edit => {
            eip::edit_add_event(world, "Lena", eip::EventChoice::IssueMoved, dest).await
        }
        RouteWrite::Pause => eip::route_action(world, "Lena", "pause", dest, true).await,
        RouteWrite::ResumePaused | RouteWrite::ResumeStopped => {
            eip::route_action(world, "Lena", "resume", dest, true).await
        }
        RouteWrite::Delete => eip::route_action(world, "Lena", "delete", dest, true).await,
    };

    let problems = refusal_problems(world, kind, &answer, &before, leaves_before).await;
    assert_none(
        &format!("{kind:?} {write:?} on a connection Foundry cannot read"),
        &problems,
    );
}

/// FU-11 (b). Identity Platform routes to #auth-dev (or the room) on a
/// connection this Foundry cannot read. Lena chooses Send test: the answer
/// reads K-19 for that kind, nothing is sent, and nothing is written. RED
/// today: Slack answers "Foundry cannot post to that channel" (K-4 on the
/// form), Matrix the uniform not-found page. Whether the refused test uses
/// one of Lena's six a minute (R-5) is left open: neither DD-2 nor DDR-25
/// says.
///
/// CONTRACT_SHAPE: unbounded-preservation
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-02 (a team lead tests a
/// route before relying on it).
async fn fu11_send_test(world: &mut FoundryWorld, kind: eip::Kind) {
    routed(world, kind).await;
    unreadable(world).await;
    let before = stored(world, tm::HOME).await;
    let sent_before = sent(world, kind);

    let answer = match kind {
        eip::Kind::Slack => {
            let reference = eip::slack_double(world).channel_id(CHANNEL);
            eip::send_test_for_reference(world, "Lena", "Identity Platform", &reference).await
        }
        eip::Kind::Matrix => {
            eip::when_matrix_test(world, "Lena", ROOM).await;
            world.eip.when_answer.clone().expect("Send test's answer")
        }
    };

    let mut problems = refusal_problems(world, kind, &answer, &before, 0).await;
    let more = sent(world, kind) - sent_before;
    if more != 0 {
        problems.push(format!("nothing is sent; {more} send(s) were made"));
    }
    assert_none(
        &format!("{kind:?} Send test on a connection Foundry cannot read"),
        &problems,
    );
}

/// The state of the workspace's connection of `kind`, from the store.
async fn connection_state(world: &FoundryWorld, kind: eip::Kind) -> Option<String> {
    sqlx::query_scalar(
        "SELECT state FROM integration_connections WHERE workspace_id = $1 AND kind = $2",
    )
    .bind(tm::workspace_id(world, tm::HOME))
    .bind(kind.wire())
    .fetch_optional(&tm::pool(world))
    .await
    .expect("read the connection state")
}

/// FU-11 (c), Disconnect as a repair (user decision 2026-10-27). Canzan
/// Labs' `kind` connection, with a route, cannot be read by this Foundry.
/// Priya opens Disconnect and confirms: the connection is forgotten, and the
/// workspace page no longer reads K-19 or a connection. For Slack the
/// question first warns that Foundry cannot remove its access at Slack
/// (R-13, ratified), in place of the question's last sentence ("Foundry's
/// access to Canzan Labs' Slack is removed."): it cannot open the token
/// `apps.uninstall` needs. RED today (Slack only): no warning, and the
/// untrue sentence is shown. The Matrix case passes today and guards
/// the repair path against the fix.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-07 (a workspace admin
/// disconnects a chat connection and knows what that leaves behind).
async fn fu11_disconnect(world: &mut FoundryWorld, kind: eip::Kind) {
    routed(world, kind).await;
    unreadable(world).await;

    eip::when_disconnect_dialog(world, "Priya", kind).await;
    let question = world
        .eip
        .when_answer
        .clone()
        .expect("the disconnect question");
    let answer = eip::disconnect(world, "Priya", kind, true).await;

    let mut problems = Vec::new();
    if kind == eip::Kind::Slack && !page(&question).contains(R13_CANZAN_LABS) {
        problems.push(format!(
            "the disconnect question warns R-13 {R13_CANZAN_LABS:?}; {}",
            tm::describe(&question)
        ));
    }
    if kind == eip::Kind::Slack && page(&question).contains(ACCESS_REMOVED_CANZAN_LABS) {
        problems.push(format!(
            "R-13 replaces {ACCESS_REMOVED_CANZAN_LABS:?}, which is not true here; {}",
            tm::describe(&question)
        ));
    }
    let section = section_text(&answer, kind.wire());
    if section.contains(k19(kind)) {
        problems.push(format!(
            "after the disconnect the {kind:?} section no longer reads K-19; it reads {section:?}"
        ));
    }
    if section.contains("Connected to") || section.contains("Connected as") {
        problems.push(format!(
            "after the disconnect no {kind:?} connection is shown; it reads {section:?}"
        ));
    }
    let state = connection_state(world, kind).await;
    if state.as_deref() != Some("disconnected") {
        problems.push(format!(
            "the connection is forgotten (state disconnected); it is {state:?}"
        ));
    }
    assert_none(
        &format!("{kind:?} Disconnect of a connection Foundry cannot read"),
        &problems,
    );
}

/// The links in the workspace page's `integration` section.
fn section_links(answer: &tm::Answer, integration: &str) -> Vec<String> {
    let doc = scraper::Html::parse_document(&answer.body);
    let selector = scraper::Selector::parse(&format!(
        "section[data-integration=\"{integration}\"] a[href]"
    ))
    .expect("selector");
    doc.select(&selector)
        .filter_map(|a| a.value().attr("href").map(str::to_string))
        .collect()
}

/// Whether the Matrix section offers Replace token: the connect form, its
/// button reading "Replace token".
fn offers_replace_token(answer: &tm::Answer) -> bool {
    let doc = scraper::Html::parse_document(&answer.body);
    let selector = scraper::Selector::parse(&format!(
        "section[data-integration=\"matrix\"] form[action=\"{}\"] button",
        eip::MATRIX_CONNECT
    ))
    .expect("selector");
    doc.select(&selector)
        .any(|b| b.text().collect::<String>().trim() == "Replace token")
}

/// FU-11 (c), the guard for R-13's placement (user decision 2026-10-27).
/// Canzan Labs' Slack connection, with a route, opens: Priya's disconnect
/// question still ends "Foundry's access to Canzan Labs' Slack is removed."
/// and carries no R-13. Passes today.
///
/// CONTRACT_SHAPE: pure-function
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-07.
async fn fu11_readable_disconnect_question(world: &mut FoundryWorld) {
    routed(world, eip::Kind::Slack).await;

    eip::when_disconnect_dialog(world, "Priya", eip::Kind::Slack).await;

    let question = world
        .eip
        .when_answer
        .clone()
        .expect("the disconnect question");
    let read = page(&question);
    let mut problems = Vec::new();
    if !read.contains(ACCESS_REMOVED_CANZAN_LABS) {
        problems.push(format!(
            "the question still reads {ACCESS_REMOVED_CANZAN_LABS:?}; {}",
            tm::describe(&question)
        ));
    }
    if read.contains(R13_CANZAN_LABS) {
        problems.push(format!(
            "R-13 is only for a connection Foundry cannot read; {}",
            tm::describe(&question)
        ));
    }
    assert_none(
        "Slack Disconnect of a connection Foundry can read",
        &problems,
    );
}

/// FU-11 (c), the repair doors beside K-19 (user decision 2026-10-27).
/// Canzan Labs' `kind` connection, with a route, cannot be read by this
/// Foundry. Priya opens the workspace page: the `kind` section reads K-19
/// and offers Disconnect beside it; the Matrix section also keeps its
/// Replace token form. RED today: K-19 hides the Disconnect link (Slack and
/// Matrix); Replace token is still offered.
///
/// CONTRACT_SHAPE: pure-function
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-07 / US-EIP-08 (a
/// workspace admin can mend a connection Foundry cannot read).
async fn fu11_repair_doors(world: &mut FoundryWorld, kind: eip::Kind) {
    routed(world, kind).await;
    unreadable(world).await;

    eip::when_open(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;

    let answer = world.eip.when_answer.clone().expect("the page");
    let section = section_text(&answer, kind.wire());
    let mut problems = Vec::new();
    if !section.contains(k19(kind)) {
        problems.push(format!(
            "the {kind:?} section reads K-19; it reads {section:?}"
        ));
    }
    let links = section_links(&answer, kind.wire());
    if !links.contains(&kind.disconnect_path().to_string()) {
        problems.push(format!(
            "the {kind:?} section offers Disconnect beside K-19 (a link to {}); its links {links:?}, it reads {section:?}",
            kind.disconnect_path()
        ));
    }
    if kind == eip::Kind::Matrix && !offers_replace_token(&answer) {
        problems.push(format!(
            "the Matrix section keeps its Replace token form beside K-19; it reads {section:?}"
        ));
    }
    assert_none(&format!("{kind:?} repair doors beside K-19"), &problems);
}

/// FU-11 (c), Replace token as a repair (user decision 2026-10-27). The
/// Matrix connection, with a route, cannot be read by this Foundry. Priya
/// enters a fresh access token for the bot and chooses Replace token: the
/// token is saved under the key Foundry holds, and the Matrix section reads
/// the connection, not K-19. RED today: the section still reads K-19 (the
/// unavailable state is the boot probe's, instance-wide, and never cleared).
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-08 (a workspace admin
/// connects Foundry's Matrix bot, or mends its connection).
async fn fu11_replace_token(world: &mut FoundryWorld) {
    routed(world, eip::Kind::Matrix).await;
    unreadable(world).await;

    let answer = eip::connect_matrix(
        world,
        "Priya",
        mx::FOUNDRY_ADDRESS,
        "a new access token for the bot",
    )
    .await;

    let section = world
        .eip
        .names
        .as_scenario(&section_text(&answer, "matrix"));
    let mut problems = Vec::new();
    if section.contains(K19_MATRIX) {
        problems.push(format!(
            "the repaired connection no longer reads K-19; it reads {section:?}"
        ));
    }
    if !section.contains("Connected as @foundry:canzan.dev") {
        problems.push(format!(
            "the repaired connection reads \"Connected as @foundry:canzan.dev\"; it reads {section:?}"
        ));
    }
    assert_none(
        "Replace token on a Matrix connection Foundry cannot read",
        &problems,
    );
}

/// FU-11 (d) (`Integrations::matrix_offer` answers NoSecretsKey before it
/// looks at the saved connection). Matrix is connected, then Foundry
/// restarts with no secrets key at all: the Matrix section reads K-19, not
/// K-18 ("not set up yet"), for a connection that exists. RED today: K-18,
/// with the connection line and its Disconnect link.
///
/// CONTRACT_SHAPE: pure-function
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-08.
async fn fu11_key_removed(world: &mut FoundryWorld) {
    workspace(world).await;
    matrix_connected(world).await;
    eip::restart_in_process_with_key(world, None).await;

    eip::when_open(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;

    let answer = world.eip.when_answer.clone().expect("the page");
    let section = section_text(&answer, "matrix");
    let mut problems = Vec::new();
    if !section.contains(K19_MATRIX) {
        problems.push(format!(
            "the Matrix section reads K-19; it reads {section:?}"
        ));
    }
    if section.contains(K18) {
        problems.push(format!(
            "the Matrix section does not read K-18 for a saved connection; it reads {section:?}"
        ));
    }
    // The repair beside K-19 (user decision 2026-10-27): Disconnect only.
    // Replace token cannot work with no key at all (nothing can be sealed),
    // so it is not asserted here.
    if !section_links(&answer, "matrix").contains(&eip::MATRIX_DISCONNECT.to_string()) {
        problems.push(format!(
            "the Matrix section offers Disconnect Matrix beside K-19; {}",
            tm::describe(&answer)
        ));
    }
    assert_none("Matrix with its secrets key removed", &problems);
}

/// The id of the first route on `project`'s integrations page, as `who`
/// reads it.
async fn first_route_id(world: &mut FoundryWorld, who: &str, project: &str) -> String {
    let path = eip::integrations_path(world, project);
    eip::when_open(world, who, &path).await;
    let answer = world.eip.when_answer.clone().expect("the project page");
    let doc = scraper::Html::parse_document(&answer.body);
    let selector = scraper::Selector::parse("[data-route-id]").expect("selector");
    doc.select(&selector)
        .find_map(|e| e.value().attr("data-route-id").map(str::to_string))
        .unwrap_or_else(|| {
            panic!(
                "precondition: {project}'s page lists a route; {}",
                tm::describe(&answer)
            )
        })
}

/// Tomás saves a New issue route from Globex Platform to `destination` (a
/// Slack channel or a Matrix room, by its scenario name).
async fn globex_route(world: &mut FoundryWorld, kind: eip::Kind, destination: &str) -> tm::Answer {
    let destination_ref = match kind {
        eip::Kind::Slack => eip::slack_double(world).channel_id(destination),
        eip::Kind::Matrix => world.eip.names.real(destination),
    };
    let fields = vec![
        ("kind".to_string(), kind.wire().to_string()),
        ("destination".to_string(), destination_ref),
        ("events".to_string(), "issue.created".to_string()),
        ("content_level".to_string(), "summary".to_string()),
    ];
    let routes = format!("{}/routes", eip::integrations_path(world, GLOBEX_PROJECT));
    eip::submit_form(world, "Tomás", &routes, &fields).await
}

fn went_through(answer: &tm::Answer) -> bool {
    answer.status.is_success() || answer.status.is_redirection()
}

/// Globex's `kind` connection, made by its admin Tomás through the product
/// (Slack: the Canzan Personal Slack workspace; Matrix: the same bot), and
/// a route from Globex Platform to it.
async fn globex_connected_and_routed(world: &mut FoundryWorld, kind: eip::Kind) {
    let (answer, connected_line) = match kind {
        eip::Kind::Slack => (
            eip::connect_slack(
                world,
                "Tomás",
                Approval::Approve(SlackTeam::named("Canzan Personal")),
            )
            .await,
            "Connected to Canzan Personal",
        ),
        eip::Kind::Matrix => {
            eip::given_room(world, GLOBEX_ROOM, Some("Globex"), 2, false).await;
            eip::invite_bot(world, GLOBEX_ROOM).await;
            let answer = eip::connect_matrix(
                world,
                "Tomás",
                mx::FOUNDRY_ADDRESS,
                "the bot's access token",
            )
            .await;
            (answer, "Connected as @foundry:canzan.dev")
        }
    };
    let read = world.eip.names.as_scenario(&page(&answer));
    assert!(
        read.contains(connected_line),
        "precondition: Tomás reads {connected_line:?}; {}",
        tm::describe(&answer)
    );
    let first = match kind {
        eip::Kind::Slack => "#ops-alerts",
        eip::Kind::Matrix => GLOBEX_ROOM,
    };
    let saved = globex_route(world, kind, first).await;
    assert!(
        went_through(&saved),
        "precondition: Tomás saves Globex Platform's route to {first}; {}",
        tm::describe(&saved)
    );
}

/// FU-11 (e), per connection (user decision 2026-10-27). Canzan Labs' `kind`
/// connection was sealed under the first secrets key. The operator rotates:
/// a new key, the first kept as the previous one; Globex (Tomás) connects
/// `kind` under the new key and routes Globex Platform to it. Then the first
/// key is dropped. Canzan Labs' connection is unreadable; Globex's opens.
///
/// Globex: its page reads its connection and no K-19; for Slack, Add to
/// Slack still takes Tomás to Slack's approval screen (Reconnect); its route
/// writes go through (a new route saved, the first one paused). Canzan Labs:
/// its page reads K-19, and pausing its route answers K-19 and writes
/// nothing. RED today: the unavailable state is instance-wide, so Globex's
/// page reads K-19 and Slack's Add to Slack goes nowhere, while Canzan Labs'
/// pause goes through.
///
/// CONTRACT_SHAPE: bounded-change
/// Outcome anchor: DISCUSS Elevator Pitch US-EIP-01 / US-EIP-08 (a
/// workspace's chat connection works, whatever another workspace's does).
async fn fu11_per_connection(world: &mut FoundryWorld, kind: eip::Kind) {
    routed(world, kind).await;
    tm::seed_foreign_workspace(world, "Platform", "Tomás Rivera", "tomas.rivera@canzan.dev").await;
    eip::given_projects(
        world,
        &[(
            GLOBEX_PROJECT.to_string(),
            "GLX".to_string(),
            "Platform".to_string(),
        )],
    )
    .await;
    world.eip.spec.previous_secrets_key = Some(eip::SECRETS_KEY.to_string());
    eip::restart_in_process_with_key(world, Some(eip::NEW_SECRETS_KEY)).await;
    globex_connected_and_routed(world, kind).await;
    world.eip.spec.previous_secrets_key = None;
    eip::restart_in_process_with_key(world, Some(eip::NEW_SECRETS_KEY)).await;

    let mut problems = Vec::new();

    // Globex: its connection opens.
    eip::when_open(world, "Tomás", eip::WORKSPACE_INTEGRATIONS).await;
    let globex_page = world.eip.when_answer.clone().expect("Globex's page");
    let globex = world
        .eip
        .names
        .as_scenario(&section_text(&globex_page, kind.wire()));
    if globex.contains(k19(kind)) {
        problems.push(format!(
            "Globex's {kind:?} section does not read K-19 (its connection opens); it reads {globex:?}"
        ));
    }
    let connected_line = match kind {
        eip::Kind::Slack => "Connected to Canzan Personal",
        eip::Kind::Matrix => "Connected as @foundry:canzan.dev",
    };
    if !globex.contains(connected_line) {
        problems.push(format!(
            "Globex's {kind:?} section reads {connected_line:?}; it reads {globex:?}"
        ));
    }
    if kind == eip::Kind::Slack {
        eip::when_open(world, "Tomás", eip::SLACK_INSTALL).await;
        let install = world
            .eip
            .when_answer
            .clone()
            .expect("Add to Slack's answer");
        let to_slack = install.location.clone().unwrap_or_default();
        if !(install.status.is_redirection()
            && to_slack.starts_with(&eip::slack_double(world).authorize_url()))
        {
            problems.push(format!(
                "Add to Slack (Reconnect) takes Tomás to Slack's approval screen; {}",
                tm::describe(&install)
            ));
        }
    }
    let globex_first = first_route_id(world, "Tomás", GLOBEX_PROJECT).await;
    let second = match kind {
        eip::Kind::Slack => "#general",
        eip::Kind::Matrix => {
            eip::given_room(world, GLOBEX_OPS_ROOM, Some("Globex ops"), 2, false).await;
            eip::invite_bot(world, GLOBEX_OPS_ROOM).await;
            GLOBEX_OPS_ROOM
        }
    };
    let created = globex_route(world, kind, second).await;
    if !went_through(&created) || page(&created).contains(k19(kind)) {
        problems.push(format!(
            "Tomás saves Globex Platform's route to {second}; {}",
            tm::describe(&created)
        ));
    }
    let pause = format!(
        "{}/routes/{globex_first}/pause",
        eip::integrations_path(world, GLOBEX_PROJECT)
    );
    let paused = eip::submit_form(world, "Tomás", &pause, &[]).await;
    let paused_now: Option<bool> = sqlx::query_scalar(
        "SELECT paused_at IS NOT NULL FROM integration_routes WHERE id::text = $1",
    )
    .bind(&globex_first)
    .fetch_optional(&tm::pool(world))
    .await
    .expect("read Globex's route");
    if paused.status != StatusCode::SEE_OTHER || paused_now != Some(true) {
        problems.push(format!(
            "Tomás pauses Globex Platform's first route (paused: {paused_now:?}); {}",
            tm::describe(&paused)
        ));
    }

    // Canzan Labs: its connection is unreadable.
    eip::when_open(world, "Priya", eip::WORKSPACE_INTEGRATIONS).await;
    let canzan_page = world.eip.when_answer.clone().expect("Canzan Labs' page");
    let canzan = section_text(&canzan_page, kind.wire());
    if !canzan.contains(k19(kind)) {
        problems.push(format!(
            "Canzan Labs' {kind:?} section reads K-19; it reads {canzan:?}"
        ));
    }
    let before = stored(world, tm::HOME).await;
    let leaves_before = leaves(world, kind);
    let answer = eip::route_action(world, "Lena", "pause", destination(kind), true).await;
    problems.extend(
        refusal_problems(world, kind, &answer, &before, leaves_before)
            .await
            .into_iter()
            .map(|p| format!("Canzan Labs' pause: {p}")),
    );

    assert_none(
        &format!("{kind:?} unavailable judged per connection"),
        &problems,
    );
}

// ===========================================================================
// The runner
// ===========================================================================

struct Selection {
    ignored_only: bool,
    include_ignored: bool,
    list: bool,
    exact: bool,
    filters: Vec<String>,
}

impl Selection {
    fn from_args() -> Self {
        let mut selection = Selection {
            ignored_only: false,
            include_ignored: false,
            list: false,
            exact: false,
            filters: Vec::new(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--ignored" => selection.ignored_only = true,
                "--include-ignored" => selection.include_ignored = true,
                "--list" => selection.list = true,
                "--exact" => selection.exact = true,
                // libtest flags that take a value.
                "--test-threads" | "--format" | "--color" | "--skip" | "-Z" => {
                    args.next();
                }
                flag if flag.starts_with('-') => {}
                filter => selection.filters.push(filter.to_string()),
            }
        }
        selection
    }

    fn matches(&self, case: &Case) -> bool {
        self.filters.is_empty()
            || self.filters.iter().any(|f| {
                if self.exact {
                    case.name == f
                } else {
                    case.name.contains(f.as_str())
                }
            })
    }

    fn runs(&self, case: &Case) -> bool {
        self.include_ignored || case.ignore.is_some() == self.ignored_only
    }
}

#[tokio::main]
async fn main() {
    let selection = Selection::from_args();
    let chosen: Vec<&Case> = CASES.iter().filter(|c| selection.matches(c)).collect();
    if selection.list {
        for case in &chosen {
            println!("{}: test", case.name);
        }
        return;
    }
    println!("\nrunning {} tests", chosen.len());
    let (mut passed, mut failed, mut ignored) = (Vec::new(), Vec::new(), 0usize);
    for case in chosen {
        if !selection.runs(case) {
            println!(
                "test {} ... ignored, {}",
                case.name,
                case.ignore.unwrap_or("")
            );
            ignored += 1;
            continue;
        }
        let mut world = FoundryWorld::default();
        let outcome = tokio::time::timeout(
            CASE_BOUND,
            AssertUnwindSafe((case.body)(&mut world)).catch_unwind(),
        )
        .await;
        drop(world);
        match outcome {
            Ok(Ok(())) => {
                println!("test {} ... ok", case.name);
                passed.push(case.name);
            }
            Ok(Err(panic)) => {
                let why = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                println!("test {} ... FAILED\n    {why}", case.name);
                failed.push(case.name);
            }
            Err(_) => {
                println!(
                    "test {} ... FAILED\n    did not finish within {CASE_BOUND:?}",
                    case.name
                );
                failed.push(case.name);
            }
        }
    }
    foundry_acceptance::support::harness::shutdown_postgres().await;
    mx::shutdown_synapse().await;
    let verdict = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {verdict}. {} passed; {} failed; {ignored} ignored",
        passed.len(),
        failed.len()
    );
    if !failed.is_empty() {
        println!("failures:");
        for name in &failed {
            println!("    {name}");
        }
        std::process::exit(101);
    }
}
