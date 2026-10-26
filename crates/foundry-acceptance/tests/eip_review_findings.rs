//! external-integration-plugins — DELIVER Phase 4 review findings (2026-10-25,
//! feature-delta DISTILL DIST-32): crate-level tests that reach the product
//! through the same doors and doubles as the acceptance lane (the in-process
//! app over the shared Postgres, the Slack double, the pinned homeserver
//! behind its fault proxy), written without Gherkin: no scenario was added.
//!
//! Each case is RED today for the defect it names and is ignored with
//! "RED until review fix <ID>"; the crafter lifts the ignore with the fix.
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
use foundry_acceptance::support::slack_double::Mood;
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
