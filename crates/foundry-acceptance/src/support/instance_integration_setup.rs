//! instance-integration-setup driver (`tests/features/instance-integration-setup.feature`,
//! DISTILL 2026-11-06, Deliverable A: US-IIS-01, 02, 03, 06, 08).
//!
//! Extends, not duplicates, the external-integration-plugins driver: the
//! Background, people, sessions, the in-process settings seam (`rewire`), the
//! operator's process, the replicas and the Slack double all come from
//! `support::external_integration_plugins`. This module owns:
//!
//! - **The vocabulary** (Mandate 12, criterion 1): [`KeySetting`],
//!   [`Restart`], [`InstanceDoor`], [`Button`], [`Offer`], [`Signer`],
//!   [`BadStart`], [`Refusal`], [`IisState`].
//! - **The doors** DESIGN pinned for Deliverable A (brief.md "For Acceptance
//!   Designer"): `GET /admin/instance/integrations`, `POST …/secrets-key`,
//!   `POST …/secrets-key/move`, `POST …/slack`; observed through the shipped
//!   `/workspace/integrations`, the route form, `/integrations/slack/events`,
//!   `/workspace/integrations/slack/install`, the instance dashboard, `/readyz`
//!   and the operator's `/metrics`.
//! - **How the operator configures Foundry**: the instance key
//!   (`FOUNDRY_INSTANCE_KEY`, legacy `FOUNDRY_SECRETS_KEY`), a temp
//!   `FOUNDRY_DATA_DIR` per scenario (never the real `/var/lib/foundry`), a
//!   short `FOUNDRY_INTEGRATIONS_RELOAD_MS`, `FOUNDRY_PUBLIC_URL` set or not,
//!   and a Slack app given as settings. They reach the in-process app through
//!   [`boot`] (`eip::rewire`, which with the restart goes through the one
//!   shared boot function `Integrations::boot`, ADR-IIS-001 decision 9) and
//!   the operator's process through its environment.
//!
//! Every oracle reads what a person or an operator reads: pages, the activity,
//! the key file in the scenario's own directory, `/readyz`, `/metrics`,
//! process output. Two exceptions, both named by DISCUSS and DESIGN: the
//! database-dump attack reads every table (US-IIS-02 AC 3, the dump is the
//! attack surface) and the race counts stored secrets keys (brief.md: "assert
//! one row"). The generated secrets key is read only through the
//! `integrations-test-seam` reader (`secrets_key_seam`, a RED scaffold today).
//!
//! Page seams DISTILL pins (DELIVER may change them together with this file):
//! a button is a `<button>` (or submit input) whose text is the copy, inside
//! the form that posts it; the Slack app form holds inputs `client_id`,
//! `client_secret`, `signing_secret`; the addresses `[data-slack-redirect-url]`
//! and `[data-slack-events-url]`; the manifest `pre[data-slack-manifest]` (as on
//! the shipped setup page); the activity `[data-instance-integration-activity] li`.
//!
//! Scenario names: the scenario's temp data directory reads as
//! `/var/lib/foundry`, and an in-process app's own fallback address as
//! `http://localhost:8080`, through `eip.names` (as Matrix names are mapped).

use crate::steps::handler_instrumentation::FoundrySubprocess;
use crate::support::external_integration_plugins::{
    self as eip, link_hrefs, texts_of, MATRIX_CONNECT, SLACK_EVENTS, SLACK_INSTALL, SLACK_SETUP,
    WORKSPACE_INTEGRATIONS,
};
use crate::support::html_assertions::{parse, select_all};
use crate::support::multi_replica_harness::{MultiReplicaHarness, ReplicaTime};
use crate::support::slack_double::{self as sd, Approval, SlackTeam};
use crate::support::team_membership::{self as tm, Answer};
use crate::world::FoundryWorld;
use base64::Engine as _;
use reqwest::header::COOKIE;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ===========================================================================
// Doors and planted values
// ===========================================================================

pub const INSTANCE_INTEGRATIONS: &str = "/admin/instance/integrations";
pub const SECRETS_KEY_DOOR: &str = "/admin/instance/integrations/secrets-key";
pub const MOVE_DOOR: &str = "/admin/instance/integrations/secrets-key/move";
pub const SLACK_APP_DOOR: &str = "/admin/instance/integrations/slack";
pub const INSTANCE_DASHBOARD: &str = "/admin/instance/workspaces";

/// The operator's instance key (32 bytes, base64url; a planted value).
pub const OPERATOR_KEY: &str = "aWlzLXNlbnRpbmVsLW9wZXJhdG9yLWluc3RhbmNlLTE";
/// A different operator key: another server's, or the one that replaces a file.
pub const OTHER_OPERATOR_KEY: &str = "aWlzLXNlbnRpbmVsLW9wZXJhdG9yLWluc3RhbmNlLTI";
/// The legacy `FOUNDRY_SECRETS_KEY` an instance configured before this feature holds.
pub const LEGACY_KEY: &str = eip::SECRETS_KEY;
/// A 20-character value: decodes to 15 bytes, not 32 (US-IIS-08 example 5).
pub const SHORT_KEY: &str = "c2hvcnQtaWlzLWtleS14";

/// The Slack app of the stories (planted test values, DISCUSS "Scenario cast").
pub const CLIENT_ID: &str = "6123456789012.7234567890123";
pub const CLIENT_SECRET: &str = "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8";
pub const SIGNING_SECRET: &str = "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05";
/// Another app (US-IIS-05's switch target; here saved while the environment's wins).
pub const OTHER_CLIENT_ID: &str = "6123456789012.8888888888888";
pub const OTHER_CLIENT_SECRET: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f9";
pub const OTHER_SIGNING_SECRET: &str = "9f8e7d6c5b4a39281706f5e4d3c2b1a0";
/// A signing secret Foundry never held (events signed with it are refused).
pub const STRANGER_SIGNING_SECRET: &str = "11111111111111111111111111111111";

pub const PUBLIC_URL: &str = "https://foundry.example.org";
/// How the scenario's temp data directory reads in the Gherkin.
pub const SCENARIO_DATA_DIR: &str = "/var/lib/foundry";
/// How an in-process app's own fallback address reads in the Gherkin.
pub const SCENARIO_FALLBACK_URL: &str = "http://localhost:8080";
/// `FOUNDRY_INTEGRATIONS_RELOAD_MS` in acceptance (brief.md: short, e.g. 200).
pub const RELOAD_MS: u64 = 200;

/// The bound the stories state ("within 60 seconds", D10).
const LIVE_BOUND: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(200);
/// Shared-clock time that passes per poll on the replicas (time is advanced,
/// not slept, brief.md "Time").
const CLOCK_STEP: Duration = Duration::from_secs(1);
const KEY_FILE: &str = "instance.key";
const ACTIVITY: &str = "[data-instance-integration-activity] li";

// ===========================================================================
// Vocabulary
// ===========================================================================

/// How the operator gives Foundry its instance key (the Given "this Foundry
/// starts with …").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySetting {
    /// `FOUNDRY_INSTANCE_KEY`.
    Operator,
    /// `FOUNDRY_SECRETS_KEY` only: an instance configured before this feature.
    Legacy,
    /// No key setting; a writable data directory (Foundry makes the file).
    FileWritable,
    /// No key setting; the data directory cannot hold a file (I-27).
    FileUnwritable,
    /// No key setting; a key file that cannot be parsed (I-31).
    FileUnreadable,
    /// Both names set to empty values: unset (DEVOPS Constraints).
    BothEmpty,
    /// Both names set to the same key: the legacy name (ADR-IIS-001 r4).
    BothSame,
}

impl KeySetting {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "its instance key set by the operator" => Self::Operator,
            "the legacy FOUNDRY_SECRETS_KEY setting" => Self::Legacy,
            "no instance key setting and a writable data directory" => Self::FileWritable,
            "no instance key setting and a data directory it cannot write" => Self::FileUnwritable,
            "no instance key setting and an instance key file it cannot read" => {
                Self::FileUnreadable
            }
            "FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY both set to empty values" => {
                Self::BothEmpty
            }
            "FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to the same key" => Self::BothSame,
            other => panic!("unknown key setting {other:?}"),
        }
    }
}

/// What changed when this Foundry restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    Same,
    OtherOperatorKey,
    SameKeyNewName,
    WithoutEnvironmentApp,
    OtherKeyFile,
}

impl Restart {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "with the same settings" => Self::Same,
            "with a different instance key set by the operator" => Self::OtherOperatorKey,
            "with the same key under FOUNDRY_INSTANCE_KEY" => Self::SameKeyNewName,
            "without the Slack app in its environment" => Self::WithoutEnvironmentApp,
            "with a different instance key file" => Self::OtherKeyFile,
            other => panic!("unknown restart {other:?}"),
        }
    }
}

/// A write door of the instance page, posted directly (not from the page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceDoor {
    Generate,
    Move,
    SlackSave,
}

impl InstanceDoor {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "generate" => Self::Generate,
            "move" => Self::Move,
            "Slack app save" => Self::SlackSave,
            other => panic!("unknown instance door {other:?}"),
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Generate => SECRETS_KEY_DOOR,
            Self::Move => MOVE_DOOR,
            Self::SlackSave => SLACK_APP_DOOR,
        }
    }

    fn fields(self) -> Vec<(String, String)> {
        match self {
            Self::Generate | Self::Move => Vec::new(),
            Self::SlackSave => slack_fields(CLIENT_ID, CLIENT_SECRET, SIGNING_SECRET),
        }
    }
}

/// A button the instance page offers (or withholds), by its copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Generate,
    Move,
    Replace,
    StartOver,
}

impl Button {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "Generate secrets key" => Self::Generate,
            "Move to a secrets key" => Self::Move,
            "Replace secrets key" => Self::Replace,
            "Start over" => Self::StartOver,
            other => panic!("unknown button {other:?}"),
        }
    }

    pub fn copy(self) -> &'static str {
        match self {
            Self::Generate => "Generate secrets key",
            Self::Move => "Move to a secrets key",
            Self::Replace => "Replace secrets key",
            Self::StartOver => "Start over",
        }
    }
}

/// "offered" | "not offered".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer(pub bool);

impl Offer {
    pub fn parse(word: &str) -> Self {
        match word.trim() {
            "offered" => Self(true),
            "not offered" => Self(false),
            other => panic!("unknown offer {other:?}"),
        }
    }
}

/// What a workspace page offers on every replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceOffer {
    ConnectMatrix,
    AddToSlack,
}

impl WorkspaceOffer {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "to connect Matrix" => Self::ConnectMatrix,
            "Add to Slack" => Self::AddToSlack,
            other => panic!("unknown workspace offer {other:?}"),
        }
    }

    fn holds(self, answer: &Answer) -> bool {
        let seen = eip::text(answer);
        match self {
            Self::ConnectMatrix => {
                tm::forms(&answer.body)
                    .iter()
                    .any(|f| f.action.ends_with(MATRIX_CONNECT))
                    && seen.contains("Connect Matrix")
            }
            Self::AddToSlack => {
                hrefs(&answer.body)
                    .iter()
                    .any(|h| h.ends_with(SLACK_INSTALL))
                    && seen.contains("Add to Slack")
            }
        }
    }
}

/// Which secret signs Slack's URL verification challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signer {
    Saved,
    Stranger,
}

impl Signer {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "the saved signing secret" => Self::Saved,
            "another signing secret" => Self::Stranger,
            other => panic!("unknown signer {other:?}"),
        }
    }

    fn secret(self) -> &'static str {
        match self {
            Self::Saved => SIGNING_SECRET,
            Self::Stranger => STRANGER_SIGNING_SECRET,
        }
    }
}

/// A key setting Foundry must refuse to start with (US-IIS-08 AC 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadStart {
    ShortInstanceKey,
    ConflictingNames,
}

impl BadStart {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "an instance key of 20 characters" => Self::ShortInstanceKey,
            "FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to different keys" => {
                Self::ConflictingNames
            }
            other => panic!("unknown start setting {other:?}"),
        }
    }
}

/// The settings a refusal names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    InstanceKey,
    BothNames,
}

impl Refusal {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "FOUNDRY_INSTANCE_KEY" => Self::InstanceKey,
            "FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY" => Self::BothNames,
            other => panic!("unknown refusal {other:?}"),
        }
    }

    fn names(self) -> &'static [&'static str] {
        match self {
            Self::InstanceKey => &["FOUNDRY_INSTANCE_KEY"],
            Self::BothNames => &["FOUNDRY_INSTANCE_KEY", "FOUNDRY_SECRETS_KEY"],
        }
    }
}

/// The scenario's operator settings and what the steps read back.
#[derive(Debug, Default)]
pub struct IisState {
    /// `FOUNDRY_INSTANCE_KEY` (`Some("")`: set to an empty value).
    pub instance_key: Option<String>,
    /// `FOUNDRY_SECRETS_KEY` (the legacy name).
    pub legacy_key: Option<String>,
    /// The scenario's temp directory (dropped with the scenario).
    pub temp: Option<tempfile::TempDir>,
    /// `FOUNDRY_DATA_DIR`.
    pub data_dir: Option<PathBuf>,
    /// The Slack app is given as settings (`SLACK_CLIENT_ID` and the rest).
    pub environment_app: bool,
    /// The operator never set `FOUNDRY_PUBLIC_URL`.
    pub public_url_unset: bool,
    /// The second server of a two-server scenario (the first is `eip.process`).
    pub second: Option<FoundrySubprocess>,
    /// The two answers of the generate race, each followed on its replica.
    pub race: Vec<Answer>,
    /// The whole database as text (the dump attack, the sweep).
    pub dump: Option<String>,
    /// The key file's bytes before the When.
    pub file_before: Option<Vec<u8>>,
    /// Pages swept for planted values.
    pub swept: Vec<Answer>,
}

// ===========================================================================
// Settings and boot
// ===========================================================================

/// The settings this feature adds, by name.
fn iis_env(world: &FoundryWorld) -> Vec<(&'static str, String)> {
    let s = &world.iis;
    let mut env = vec![("FOUNDRY_INTEGRATIONS_RELOAD_MS", RELOAD_MS.to_string())];
    if let Some(dir) = &s.data_dir {
        env.push(("FOUNDRY_DATA_DIR", dir.display().to_string()));
    }
    if let Some(key) = &s.instance_key {
        env.push(("FOUNDRY_INSTANCE_KEY", key.clone()));
    }
    if let Some(key) = &s.legacy_key {
        env.push(("FOUNDRY_SECRETS_KEY", key.clone()));
    }
    if s.environment_app {
        env.push(("SLACK_CLIENT_ID", CLIENT_ID.to_string()));
        env.push(("SLACK_CLIENT_SECRET", CLIENT_SECRET.to_string()));
        env.push(("SLACK_SIGNING_SECRET", SIGNING_SECRET.to_string()));
    }
    // The Slack test seams (D13) apply to whichever app is in effect.
    if let Some(double) = &world.eip.slack {
        env.push(("SLACK_API_BASE_URL", double.api_base_url()));
        env.push(("SLACK_AUTHORIZE_URL", double.authorize_url()));
    }
    env
}

/// Hand the feature's settings to the eip spec: the eip spec spells no key
/// and no Slack app of its own here, so no name is given twice.
fn apply(world: &mut FoundryWorld) {
    world.eip.spec.secrets_key = None;
    world.eip.spec.previous_secrets_key = None;
    world.eip.spec.slack_app = eip::SlackAppSetup::None;
    world.eip.spec.public_url = if world.iis.public_url_unset {
        None
    } else {
        Some(PUBLIC_URL.to_string())
    };
    world.eip.spec.extra_env = iis_env(world);
    if let Some(dir) = world.iis.data_dir.clone() {
        name(world, SCENARIO_DATA_DIR, &dir.display().to_string());
    }
}

/// Map a real value to the words the scenario uses for it.
fn name(world: &mut FoundryWorld, scenario: &str, real: &str) {
    let map = &mut world.eip.names.map;
    map.retain(|(s, _)| s != scenario);
    map.push((scenario.to_string(), real.to_string()));
}

/// Start (or restart) the in-process Foundry, and every replica, with the
/// scenario's settings, through `eip::rewire` and so the one shared boot
/// function `main.rs` starts with (brief.md "Boot and restart").
pub async fn boot(world: &mut FoundryWorld) {
    apply(world);
    if world.iis.public_url_unset {
        let fallback = match &world.eip.replicas {
            Some(r) => r.base_url(),
            None => tm::harness(world).base_url(),
        };
        name(world, SCENARIO_FALLBACK_URL, &fallback);
    }
    eip::rewire(world).await;
}

async fn ensure_double(world: &mut FoundryWorld) {
    let double = eip::ensure_slack(world).await;
    double.expect_client_secret(CLIENT_SECRET);
}

/// Set the key settings and the scenario's data directory, without starting.
fn configure(world: &mut FoundryWorld, setting: KeySetting) {
    let temp = tempfile::tempdir().expect("the scenario's temp directory");
    let dir = temp.path().to_path_buf();
    world.iis.instance_key = None;
    world.iis.legacy_key = None;
    world.iis.data_dir = Some(dir.clone());
    match setting {
        KeySetting::Operator => world.iis.instance_key = Some(OPERATOR_KEY.into()),
        KeySetting::Legacy => world.iis.legacy_key = Some(LEGACY_KEY.into()),
        KeySetting::FileWritable => {}
        KeySetting::FileUnwritable => {
            // A path under a regular file can never be a directory, whoever
            // runs the test (root included).
            let blocker = dir.join("not-a-directory");
            std::fs::write(&blocker, b"a file, not a directory\n").expect("plant the blocker");
            world.iis.data_dir = Some(blocker);
        }
        KeySetting::FileUnreadable => {
            let file = dir.join(KEY_FILE);
            write_private(&file, b"this is not an instance key\n");
            world.iis.file_before = Some(std::fs::read(&file).expect("the planted file"));
        }
        KeySetting::BothEmpty => {
            world.iis.instance_key = Some(String::new());
            world.iis.legacy_key = Some(String::new());
        }
        KeySetting::BothSame => {
            world.iis.instance_key = Some(OPERATOR_KEY.into());
            world.iis.legacy_key = Some(OPERATOR_KEY.into());
        }
    }
    world.iis.temp = Some(temp);
}

fn write_private(path: &Path, bytes: &[u8]) {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    f.write_all(bytes).expect("write the file");
}

/// "Priya Raman is this Foundry's instance admin" (an `instance_admins` row,
/// seeded as people are).
pub async fn given_instance_admin(world: &mut FoundryWorld) {
    let priya = tm::resolve_person(world, "Priya Raman").id;
    sqlx::query("INSERT INTO instance_admins (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(priya)
        .execute(&tm::pool(world))
        .await
        .expect("seed Priya's instance_admins row");
}

/// "this Foundry starts with <setting>".
pub async fn given_starts(world: &mut FoundryWorld, setting: KeySetting) {
    ensure_double(world).await;
    configure(world, setting);
    boot(world).await;
}

/// "FOUNDRY_PUBLIC_URL is not set for this Foundry".
pub async fn given_public_url_unset(world: &mut FoundryWorld) {
    world.iis.public_url_unset = true;
    boot(world).await;
}

/// "Foundry's environment sets the Slack app <client id>".
pub async fn given_environment_app(world: &mut FoundryWorld) {
    ensure_double(world).await;
    world.iis.environment_app = true;
    boot(world).await;
}

/// "this Foundry runs as two replicas with <setting>".
pub async fn given_replicas(world: &mut FoundryWorld, setting: KeySetting) {
    ensure_double(world).await;
    configure(world, setting);
    world.eip.spec.in_process_worker = false;
    boot(world).await;
    let now = {
        use foundry_app::Clock as _;
        tm::harness(world).fake_clock.now()
    };
    let replicas = MultiReplicaHarness::spawn_sharing_schema_with_time(
        2,
        tm::harness(world),
        now,
        ReplicaTime::Frozen,
    )
    .await;
    world.eip.replicas = Some(replicas);
    // Each replica starts with the operator's settings.
    boot(world).await;
}

// ---- the operator's process (subprocess lane) ------------------------------

/// "Foundry runs as the operator's standalone process with <setting>": the
/// prebuilt binary on the scenario's schema. The in-process app is never
/// given these settings, so it makes no key file of its own.
pub async fn given_standalone(world: &mut FoundryWorld, setting: KeySetting) {
    ensure_double(world).await;
    configure(world, setting);
    world.eip.spec.in_process_worker = false;
    apply(world);
    eip::start_process(world).await;
}

/// "two Foundry servers share one data directory and neither has an instance
/// key setting": configured, not started.
pub async fn given_two_servers_sharing(world: &mut FoundryWorld) {
    ensure_double(world).await;
    configure(world, KeySetting::FileWritable);
    world.eip.spec.in_process_worker = false;
    apply(world);
}

async fn spawn_server(world: &FoundryWorld, label: &str) -> FoundrySubprocess {
    let (url, schema) = eip::database_url(world).await;
    let mut env = eip::instance_env(world);
    env.push(("FOUNDRY_DB_SCHEMA", schema.clone()));
    let label = if label.is_empty() {
        schema
    } else {
        format!("{schema}_{label}")
    };
    FoundrySubprocess::spawn_with_env_overrides(&url, label, 1, &env)
        .await
        .unwrap_or_else(|e| panic!("start a Foundry server: {e}"))
}

/// "both servers start at the same moment".
pub async fn when_both_start(world: &mut FoundryWorld) {
    let (a, b) = tokio::join!(spawn_server(world, ""), spawn_server(world, "b"));
    world.eip.process = Some(a);
    world.iis.second = Some(b);
}

/// "Foundry runs as two servers on one database, the second with a different
/// instance key set by the operator".
pub async fn given_two_servers_two_keys(world: &mut FoundryWorld) {
    ensure_double(world).await;
    configure(world, KeySetting::Operator);
    world.eip.spec.in_process_worker = false;
    apply(world);
    let first = spawn_server(world, "").await;
    world.iis.instance_key = Some(OTHER_OPERATOR_KEY.into());
    apply(world);
    let second = spawn_server(world, "b").await;
    world.iis.instance_key = Some(OPERATOR_KEY.into());
    apply(world);
    world.eip.process = Some(first);
    world.iis.second = Some(second);
}

fn second_base(world: &FoundryWorld) -> String {
    let p = world
        .iis
        .second
        .as_ref()
        .expect("a second Foundry server runs");
    format!("http://{}", p.main_addr)
}

// ===========================================================================
// Restarts
// ===========================================================================

fn key_file(world: &FoundryWorld) -> PathBuf {
    world
        .iis
        .data_dir
        .clone()
        .expect("the scenario gave Foundry a data directory")
        .join(KEY_FILE)
}

/// "this Foundry has since restarted <how>" / "this Foundry restarts <how>".
pub async fn restart(world: &mut FoundryWorld, how: Restart) {
    world.iis.file_before = std::fs::read(key_file(world)).ok();
    match how {
        Restart::Same => {}
        Restart::OtherOperatorKey => {
            world.iis.instance_key = Some(OTHER_OPERATOR_KEY.into());
            world.iis.legacy_key = None;
        }
        Restart::SameKeyNewName => {
            world.iis.instance_key = world.iis.legacy_key.take();
            assert!(
                world.iis.instance_key.is_some(),
                "precondition: the legacy name held the key"
            );
        }
        Restart::WithoutEnvironmentApp => world.iis.environment_app = false,
        Restart::OtherKeyFile => {
            // Another server's file: a valid key, but not the one that
            // sealed the stored secrets key.
            let file = key_file(world);
            assert!(
                file.exists(),
                "precondition: Foundry made {}",
                file.display()
            );
            std::fs::remove_file(&file).expect("remove the key file");
            write_private(&file, format!("{OTHER_OPERATOR_KEY}\n").as_bytes());
        }
    }
    boot(world).await;
}

/// "the operator's standalone process restarts".
pub async fn when_process_restarts(world: &mut FoundryWorld) {
    eip::capture_before(world).await;
    eip::restart_process(world).await;
}

// ===========================================================================
// Pages and forms
// ===========================================================================

fn hrefs(body: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, "a[href]")
        .into_iter()
        .filter_map(|a| a.value().attr("href").map(str::to_string))
        .collect()
}

/// The text of every link and button on `body`.
fn clickable_texts(body: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, "a, button, input[type=submit]")
        .into_iter()
        .map(|e| {
            let text = e.text().collect::<String>().trim().to_string();
            if text.is_empty() {
                e.value().attr("value").unwrap_or("").to_string()
            } else {
                text
            }
        })
        .collect()
}

/// A form on `body` with a button (or submit input) reading `label`: its
/// action and its hidden fields.
fn form_with_button(body: &str, label: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = parse(body);
    let buttons = scraper::Selector::parse("button, input[type=submit]").expect("selector");
    let hidden = scraper::Selector::parse("input[type=hidden][name]").expect("selector");
    select_all(&doc, "form").into_iter().find_map(|form| {
        let has = form.select(&buttons).any(|b| {
            let text = b.text().collect::<String>();
            text.trim() == label || b.value().attr("value") == Some(label)
        });
        has.then(|| {
            let fields = form
                .select(&hidden)
                .map(|i| {
                    (
                        i.value().attr("name").unwrap_or("").to_string(),
                        i.value().attr("value").unwrap_or("").to_string(),
                    )
                })
                .collect();
            (
                form.value().attr("action").unwrap_or("").to_string(),
                fields,
            )
        })
    })
}

/// The Slack app form (the one holding a `client_secret` input): its action
/// and every named input with its value.
fn slack_form(body: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = parse(body);
    let inputs = scraper::Selector::parse("input[name]").expect("selector");
    select_all(&doc, "form").into_iter().find_map(|form| {
        let fields: Vec<(String, String)> = form
            .select(&inputs)
            .map(|i| {
                (
                    i.value().attr("name").unwrap_or("").to_string(),
                    i.value().attr("value").unwrap_or("").to_string(),
                )
            })
            .collect();
        fields.iter().any(|(n, _)| n == "client_secret").then(|| {
            (
                form.value().attr("action").unwrap_or("").to_string(),
                fields,
            )
        })
    })
}

fn slack_fields(
    client_id: &str,
    client_secret: &str,
    signing_secret: &str,
) -> Vec<(String, String)> {
    vec![
        ("client_id".into(), client_id.into()),
        ("client_secret".into(), client_secret.into()),
        ("signing_secret".into(), signing_secret.into()),
    ]
}

fn answer(world: &FoundryWorld) -> Answer {
    world
        .eip
        .when_answer
        .clone()
        .unwrap_or_else(|| tm::last(world).clone())
}

fn answered(answer: &Answer, what: &str) {
    assert!(
        answer.status.is_success(),
        "{what}: the page answered; {}",
        tm::describe(answer)
    );
}

fn seen(world: &FoundryWorld, answer: &Answer) -> String {
    world.eip.names.as_scenario(&eip::text(answer))
}

async fn instance_page(world: &mut FoundryWorld, who: &str) -> Answer {
    eip::get(world, who, INSTANCE_INTEGRATIONS).await
}

/// "<who> opens the instance integrations page".
pub async fn when_open(world: &mut FoundryWorld, who: &str) {
    eip::when_open(world, who, INSTANCE_INTEGRATIONS).await;
}

/// "Priya follows "Integrations" on the instance admin dashboard".
pub async fn when_follow_dashboard(world: &mut FoundryWorld, who: &str) {
    let dashboard = eip::get(world, who, INSTANCE_DASHBOARD).await;
    answered(&dashboard, "the instance admin dashboard");
    let links = link_hrefs(&dashboard.body, "Integrations");
    assert_eq!(
        links.len(),
        1,
        "{who} finds one \"Integrations\" link on the instance admin dashboard (I-1); {}",
        tm::describe(&dashboard)
    );
    eip::when_open(world, who, &links[0]).await;
}

/// Submit the instance page's form whose button reads `button`, as a click
/// would, and follow the answer.
async fn choose(world: &mut FoundryWorld, who: &str, button: Button) -> Answer {
    let page = instance_page(world, who).await;
    answered(&page, "the instance integrations page");
    let (action, fields) = form_with_button(&page.body, button.copy()).unwrap_or_else(|| {
        panic!(
            "{who} finds {:?} on the instance integrations page; {}",
            button.copy(),
            tm::describe(&page)
        )
    });
    let posted = eip::post(world, who, &action, &fields, true).await;
    eip::follow(world, who, posted).await
}

/// "Priya chooses <button> on the instance integrations page".
pub async fn when_choose(world: &mut FoundryWorld, who: &str, button: Button) {
    eip::capture_before(world).await;
    let answer = choose(world, who, button).await;
    world.eip.when_answer = Some(answer);
}

async fn save_slack_app(
    world: &mut FoundryWorld,
    who: &str,
    client_id: &str,
    client_secret: &str,
    signing_secret: &str,
) -> Answer {
    let page = instance_page(world, who).await;
    answered(&page, "the instance integrations page");
    let (action, mut fields) = slack_form(&page.body).unwrap_or_else(|| {
        panic!(
            "{who} finds the Slack app form on the instance integrations page; {}",
            tm::describe(&page)
        )
    });
    fields.retain(|(n, _)| !["client_id", "client_secret", "signing_secret"].contains(&n.as_str()));
    fields.extend(slack_fields(client_id, client_secret, signing_secret));
    let posted = eip::post(world, who, &action, &fields, true).await;
    eip::follow(world, who, posted).await
}

/// "Priya saves the Slack app with client ID …, client secret … and signing secret …".
pub async fn when_save_slack_app(
    world: &mut FoundryWorld,
    who: &str,
    client_id: &str,
    client_secret: &str,
    signing_secret: &str,
) {
    eip::capture_before(world).await;
    let answer = save_slack_app(world, who, client_id, client_secret, signing_secret).await;
    world.eip.when_answer = Some(answer);
}

/// "<who> posts a <door> request [anyway|again]": the door posted directly, as a
/// stale tab or a forged form would, with a valid form token.
pub async fn when_post_door(world: &mut FoundryWorld, who: &str, door: InstanceDoor) {
    eip::capture_before(world).await;
    let posted = eip::post(world, who, door.path(), &door.fields(), true).await;
    let answer = if posted.status.is_redirection() {
        eip::follow(world, who, posted).await
    } else {
        posted
    };
    world.eip.when_answer = Some(answer);
}

// ---- product-made preconditions --------------------------------------------

/// "Priya has generated the secrets key" (through the page, as she would).
pub async fn given_generated(world: &mut FoundryWorld) {
    let answer = choose(world, "Priya", Button::Generate).await;
    assert!(
        eip::text(&answer).contains("Secrets key generated."),
        "precondition: Priya generated the secrets key on the instance integrations page; {}",
        tm::describe(&answer)
    );
}

/// "Priya has saved the Slack app <client id>" (the story's planted secrets).
pub async fn given_saved_app(world: &mut FoundryWorld) {
    let answer = save_slack_app(world, "Priya", CLIENT_ID, CLIENT_SECRET, SIGNING_SECRET).await;
    assert!(
        eip::text(&answer).contains("Saved."),
        "precondition: Priya saved the Slack app {CLIENT_ID}; {}",
        tm::describe(&answer)
    );
}

/// "Priya has connected Canzan Labs to Slack with this Foundry's app": Add to
/// Slack through the product, the code exchanged with the app's client secret.
pub async fn given_connected(world: &mut FoundryWorld) {
    ensure_double(world).await;
    let answer = eip::connect_slack(
        world,
        "Priya",
        Approval::Approve(SlackTeam::named("Canzan Labs")),
    )
    .await;
    eip::went_through(&answer, "Priya's Add to Slack");
    assert!(
        eip::text(&answer).contains("Connected to Canzan Labs"),
        "precondition: Priya reads that Canzan Labs is connected; {}",
        tm::describe(&answer)
    );
}

// ===========================================================================
// Then: the instance page
// ===========================================================================

/// "Priya is offered | not offered <button>".
pub fn then_button(world: &FoundryWorld, who: &str, button: Button, offer: Offer) {
    let answer = answer(world);
    answered(&answer, who);
    // "Not offered" also means no link or other control carries the copy, so
    // a link styled as a button cannot make it pass.
    let anywhere = clickable_texts(&answer.body)
        .iter()
        .any(|t| t == button.copy());
    assert_eq!(
        form_with_button(&answer.body, button.copy()).is_some() && anywhere,
        offer.0,
        "{who} is {}offered {:?}; {}",
        if offer.0 { "" } else { "not " },
        button.copy(),
        tm::describe(&answer)
    );
}

/// "Priya is offered no way to enter the Slack app".
pub fn then_no_slack_form(world: &FoundryWorld, who: &str) {
    let answer = answer(world);
    answered(&answer, who);
    assert!(
        slack_form(&answer.body).is_none(),
        "{who} is offered no Slack app form; {}",
        tm::describe(&answer)
    );
}

/// "Priya sees the redirect URL <r> and the events URL <e>".
pub fn then_addresses(world: &FoundryWorld, redirect: &str, events: &str) {
    let answer = answer(world);
    answered(&answer, "Priya");
    for (selector, want) in [
        ("[data-slack-redirect-url]", redirect),
        ("[data-slack-events-url]", events),
    ] {
        let shown: Vec<String> = texts_of(&answer.body, selector)
            .into_iter()
            .map(|t| world.eip.names.as_scenario(&t))
            .collect();
        assert_eq!(
            shown,
            vec![want.to_string()],
            "the page shows {want} at {selector}; {}",
            tm::describe(&answer)
        );
    }
}

/// "the manifest shown equals the one on the Slack setup page".
pub async fn then_manifest_matches(world: &mut FoundryWorld) {
    let answer = answer(world);
    answered(&answer, "Priya");
    let here = texts_of(&answer.body, "pre[data-slack-manifest]");
    let setup = eip::get(world, "Priya", SLACK_SETUP).await;
    let there = texts_of(&setup.body, "pre[data-slack-manifest]");
    assert!(
        here.len() == 1 && !here[0].is_empty(),
        "the instance page shows one manifest; {}",
        tm::describe(&answer)
    );
    assert_eq!(
        here, there,
        "the instance page's manifest equals the setup page's"
    );
}

/// "the FOUNDRY_PUBLIC_URL warning comes before the Slack addresses".
pub fn then_warning_first(world: &FoundryWorld) {
    let answer = answer(world);
    answered(&answer, "Priya");
    let text = seen(world, &answer);
    let warning = text.find("FOUNDRY_PUBLIC_URL is not set");
    let address = text.find("/workspace/integrations/slack/return");
    assert!(
        matches!((warning, address), (Some(w), Some(a)) if w < a),
        "I-29 comes before the Slack addresses; {}",
        tm::describe(&answer)
    );
}

/// "Priya is on the instance integrations page".
pub fn then_on_instance_page(world: &FoundryWorld, who: &str) {
    let answer = answer(world);
    assert!(
        answer.status.is_success()
            && answer.path.split('?').next() == Some(INSTANCE_INTEGRATIONS)
            && eip::text(&answer).contains("Integrations for this Foundry"),
        "{who} is on the instance integrations page; {}",
        tm::describe(&answer)
    );
}

/// "the instance integrations page reads <words>" (Priya's view, now).
pub async fn then_page_reads(world: &mut FoundryWorld, words: &str) {
    let page = instance_page(world, "Priya").await;
    answered(&page, "Priya's instance integrations page");
    let text = seen(world, &page);
    assert!(
        text.contains(words),
        "the instance integrations page reads {words:?}; {}",
        tm::describe(&page)
    );
}

/// "this Foundry still has no secrets key": Priya's page states no stored key
/// and the activity records no generation.
pub async fn then_still_no_secrets_key(world: &mut FoundryWorld) {
    let page = instance_page(world, "Priya").await;
    answered(&page, "Priya's instance integrations page");
    let text = eip::text(&page);
    let lines = texts_of(&page.body, ACTIVITY);
    assert!(
        !text.contains("Secrets key: set") && !text.contains("Secrets key generated"),
        "this Foundry has no secrets key; {}",
        tm::describe(&page)
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("generated the secrets key")),
        "the activity records no generation; {lines:?}"
    );
}

/// "the instance integrations activity reads <line> [exactly once]".
pub async fn then_activity(world: &mut FoundryWorld, line: &str, once: bool) {
    let page = instance_page(world, "Priya").await;
    answered(&page, "Priya's instance integrations page");
    let lines = texts_of(&page.body, ACTIVITY);
    let n = lines.iter().filter(|l| l.contains(line)).count();
    assert!(
        if once { n == 1 } else { n >= 1 },
        "the instance activity reads {line:?}{}; it lists {lines:?}",
        if once { " exactly once" } else { "" }
    );
}

/// "the form keeps the client ID <id> and leaves both secrets empty".
pub fn then_form_kept(world: &FoundryWorld, client_id: &str) {
    let answer = answer(world);
    let (_, fields) = slack_form(&answer.body).unwrap_or_else(|| {
        panic!(
            "Priya is shown the Slack app form again; {}",
            tm::describe(&answer)
        )
    });
    let value = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    assert_eq!(value("client_id"), client_id, "the typed client ID is kept");
    assert_eq!(
        value("client_secret"),
        "",
        "the client secret field is empty"
    );
    assert_eq!(
        value("signing_secret"),
        "",
        "the signing secret field is empty"
    );
}

/// "Slack is still not set up on this Foundry": no app saved here, and no
/// workspace is offered Add to Slack.
pub async fn then_slack_not_set_up(world: &mut FoundryWorld) {
    let page = instance_page(world, "Priya").await;
    answered(&page, "Priya's instance integrations page");
    assert!(
        !eip::text(&page).contains("Slack: set up here"),
        "no Slack app is saved; {}",
        tm::describe(&page)
    );
    let workspace = eip::get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    answered(&workspace, "Canzan Labs' workspace integrations page");
    assert!(
        !WorkspaceOffer::AddToSlack.holds(&workspace),
        "Canzan Labs is not offered Add to Slack; {}",
        tm::describe(&workspace)
    );
}

// ===========================================================================
// Then: workspace pages, Slack, the board
// ===========================================================================

/// "Canzan Labs' workspace integrations page offers to connect Matrix".
pub async fn then_workspace_offers(world: &mut FoundryWorld, offer: WorkspaceOffer) {
    let page = eip::get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    answered(&page, "Canzan Labs' workspace integrations page");
    assert!(
        offer.holds(&page),
        "Canzan Labs' workspace integrations page offers {offer:?}; {}",
        tm::describe(&page)
    );
}

/// "Canzan Labs' workspace integrations page offers neither Add to Slack nor
/// to connect Matrix".
pub async fn then_workspace_offers_nothing(world: &mut FoundryWorld) {
    let page = eip::get(world, "Priya", WORKSPACE_INTEGRATIONS).await;
    answered(&page, "Canzan Labs' workspace integrations page");
    for offer in [WorkspaceOffer::AddToSlack, WorkspaceOffer::ConnectMatrix] {
        assert!(
            !offer.holds(&page),
            "Canzan Labs is not offered {offer:?}; {}",
            tm::describe(&page)
        );
    }
}

/// Ask every replica, at its own address, for `path` (as `who`) until each
/// answer holds `holds`, within the 60 s bound. The shared clock advances
/// 1 s per round (freshness and rate limits read it, ADR-IIS-004 r8); a
/// short real pause stays, because the reload period
/// (`FOUNDRY_INTEGRATIONS_RELOAD_MS`) is a timer, not a clock read.
async fn on_every_replica(
    world: &mut FoundryWorld,
    who: &str,
    path: &str,
    what: &str,
    holds: impl Fn(&Answer) -> bool,
) {
    let addrs = world
        .eip
        .replicas
        .as_ref()
        .expect("Foundry runs as replicas")
        .replica_addrs();
    let mut pending: Vec<String> = addrs.iter().map(|a| format!("http://{a}")).collect();
    let mut waited = Duration::ZERO;
    let wall = Instant::now();
    let mut last = String::new();
    while !pending.is_empty() {
        let mut still = Vec::new();
        for base in pending {
            let page = eip::get(world, who, &format!("{base}{path}")).await;
            if !(page.status.is_success() && holds(&page)) {
                last = tm::describe(&page);
                still.push(base);
            }
        }
        pending = still;
        if pending.is_empty() {
            break;
        }
        assert!(
            waited < LIVE_BOUND && wall.elapsed() < LIVE_BOUND,
            "within 60 seconds every replica: {what}; still not on {pending:?}; last: {last}"
        );
        if let Some(r) = &world.eip.replicas {
            r.fake_clock.advance(CLOCK_STEP);
        }
        waited += CLOCK_STEP;
        tokio::time::sleep(POLL).await;
    }
}

/// "within 60 seconds, without a restart, Canzan Labs' workspace integrations
/// page offers <offer> on every replica".
pub async fn then_every_replica_offers(world: &mut FoundryWorld, offer: WorkspaceOffer) {
    on_every_replica(
        world,
        "Priya",
        WORKSPACE_INTEGRATIONS,
        &format!("offers {offer:?}"),
        |page| offer.holds(page),
    )
    .await;
}

/// "within 60 seconds every replica's history of the route to <dest> lists
/// <key> as delivered" (US-IIS-06 AC 3).
pub async fn then_every_replica_history(world: &mut FoundryWorld, destination: &str, key: &str) {
    let history = eip::open_history(world, "Lena", destination).await;
    answered(&history, "Lena's route history");
    let path = history.path.clone();
    let key = key.to_string();
    on_every_replica(
        world,
        "Lena",
        &path,
        &format!("the history lists {key} delivered"),
        |page| {
            texts_of(&page.body, "[data-delivery]")
                .iter()
                .any(|e| e.contains(&key) && e.contains("Delivered"))
        },
    )
    .await;
}

/// "within 60 seconds Canzan Labs' Slack connection opens on every replica".
pub async fn then_connection_opens_on_every_replica(world: &mut FoundryWorld) {
    let path = format!(
        "{}/routes/new?kind=slack",
        eip::integrations_path(world, "Identity Platform")
    );
    on_every_replica(
        world,
        "Priya",
        &path,
        "the channel picker lists #auth-dev",
        |page| {
            texts_of(&page.body, "select[name=destination] option")
                .iter()
                .any(|o| o.contains("auth-dev"))
        },
    )
    .await;
}

/// "a generate request reaches each replica at the same moment".
pub async fn when_race(world: &mut FoundryWorld) {
    let addrs = world
        .eip
        .replicas
        .as_ref()
        .expect("Foundry runs as replicas")
        .replica_addrs();
    let email = eip::email_of(world, "Priya");
    let cookie = eip::cookie(world, email.as_deref()).await;
    let csrf = world.eip.csrf.clone();
    let http = eip::http(world);
    let post = |addr: std::net::SocketAddr| {
        let (http, cookie, csrf) = (http.clone(), cookie.clone(), csrf.clone());
        async move {
            let base = format!("http://{addr}");
            let resp = http
                .post(format!("{base}{SECRETS_KEY_DOOR}"))
                .header(COOKIE, cookie.clone())
                .form(&[("_csrf", csrf.as_str())])
                .send()
                .await
                .expect("POST the generate door");
            let posted = tm::into_answer(SECRETS_KEY_DOOR, resp).await;
            match (posted.status.is_redirection(), posted.location.clone()) {
                (true, Some(loc)) => {
                    let path = loc.strip_prefix(&base).unwrap_or(&loc).to_string();
                    let resp = http
                        .get(format!("{base}{path}"))
                        .header(COOKIE, cookie)
                        .send()
                        .await
                        .expect("follow the generate answer");
                    tm::into_answer(&path, resp).await
                }
                _ => posted,
            }
        }
    };
    let (a, b) = tokio::join!(post(addrs[0]), post(addrs[1]));
    world.iis.race = vec![a, b];
}

/// "exactly one secrets key is stored, and both requests were answered with it set".
pub async fn then_one_key(world: &mut FoundryWorld) {
    for answer in &world.iis.race {
        let text = eip::text(answer);
        assert!(
            answer.status.is_success()
                && (text.contains("Secrets key generated.") || text.contains("Secrets key: set.")),
            "each generate request is answered with the page's resulting state; {}",
            tm::describe(answer)
        );
    }
    let pool = &world
        .eip
        .replicas
        .as_ref()
        .expect("Foundry runs as replicas")
        .shared_pool;
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_secrets_key")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count the stored secrets keys: {e}"));
    assert_eq!(stored, 1, "exactly one secrets key is stored");
}

/// The new Slack route form on `base` lists Canzan Labs' channels: the
/// connection's token opened there (it was used to ask Slack).
async fn connection_opens_at(world: &mut FoundryWorld, base: &str) {
    let path = format!(
        "{base}{}/routes/new?kind=slack",
        eip::integrations_path(world, "Identity Platform")
    );
    let form = eip::get(world, "Priya", &path).await;
    answered(&form, "the new Slack route form");
    let options = texts_of(&form.body, "select[name=destination] option");
    assert!(
        options.iter().any(|o| o.contains("auth-dev")),
        "Canzan Labs' Slack connection opens at {base}: the channel picker lists #auth-dev; {}",
        tm::describe(&form)
    );
}

/// "Canzan Labs' saved Slack connection still opens".
pub async fn then_connection_opens(world: &mut FoundryWorld) {
    let base = eip::base(world);
    connection_opens_at(world, &base).await;
}

/// "Priya connects Canzan Labs to Slack with this Foundry's app": Add to Slack
/// through the product; Slack (the double) exchanges the code only for the
/// client secret of the app in effect (US-IIS-03 AC 4).
pub async fn when_connect(world: &mut FoundryWorld) {
    ensure_double(world).await;
    eip::capture_before(world).await;
    let answer = eip::connect_slack(
        world,
        "Priya",
        Approval::Approve(SlackTeam::named("Canzan Labs")),
    )
    .await;
    world.eip.when_answer = Some(answer);
}

/// "a new Matrix route for Identity Platform no longer reads <words>"
/// (US-IIS-02 AC 2).
pub async fn then_matrix_form_not(world: &mut FoundryWorld, words: &str) {
    let path = format!(
        "{}/routes/new?kind=matrix",
        eip::integrations_path(world, "Identity Platform")
    );
    let form = eip::get(world, "Priya", &path).await;
    answered(&form, "the new Matrix route form");
    assert!(
        !eip::text(&form).contains(words),
        "the new Matrix route form no longer reads {words:?}; {}",
        tm::describe(&form)
    );
}

/// "Add to Slack on Canzan Labs' workspace integrations page sends Priya to
/// Slack's approval for client ID <id>".
pub async fn then_approval_for(world: &mut FoundryWorld, client_id: &str) {
    let double = eip::slack_double(world);
    let start = eip::get(world, "Priya", SLACK_INSTALL).await;
    let to = start.location.clone().unwrap_or_default();
    assert!(
        start.status.is_redirection()
            && to.starts_with(&double.authorize_url())
            && to.contains(&format!("client_id={client_id}")),
        "Add to Slack asks Slack's approval for client ID {client_id}; {}",
        tm::describe(&start)
    );
}

/// "Slack sends its URL verification challenge signed with <signer>".
pub async fn when_challenge(world: &mut FoundryWorld, signer: Signer) {
    let double = eip::slack_double(world);
    let base = eip::base(world);
    let challenge = format!("iis-challenge-{}", uuid::Uuid::new_v4().simple());
    let (status, body) = double
        .post_event(
            &base,
            SLACK_EVENTS,
            &sd::url_verification_envelope(&challenge),
            Some(signer.secret()),
            Duration::ZERO,
        )
        .await;
    world.eip.comment_text = Some(challenge);
    world.eip.when_answer = Some(Answer {
        path: SLACK_EVENTS.into(),
        status,
        content_type: None,
        location: None,
        session: None,
        body,
    });
}

/// "this Foundry still answers that it is ready".
pub async fn then_ready(world: &mut FoundryWorld) {
    let ready = eip::get(world, "a signed-out visitor", "/readyz").await;
    assert!(
        ready.status.is_success(),
        "/readyz answers 200; {}",
        tm::describe(&ready)
    );
}

// ===========================================================================
// Then: the key file
// ===========================================================================

/// "the instance key file is unchanged".
pub fn then_file_unchanged(world: &FoundryWorld) {
    let before = world
        .iis
        .file_before
        .clone()
        .expect("precondition: the key file existed before");
    let now = std::fs::read(key_file(world)).expect("the key file is still there");
    assert_eq!(now, before, "Foundry never rewrites the instance key file");
}

/// The uid this test runs as (the owner of a file it creates).
fn own_uid(dir: &Path) -> u32 {
    use std::os::unix::fs::MetadataExt as _;
    let probe = dir.join(".iis-owner-probe");
    std::fs::write(&probe, b"").expect("write the owner probe");
    let uid = std::fs::metadata(&probe).expect("the probe").uid();
    let _ = std::fs::remove_file(&probe);
    uid
}

/// "the instance key file is readable only by Foundry's user".
pub fn then_file_private(world: &FoundryWorld) {
    use std::os::unix::fs::MetadataExt as _;
    let file = key_file(world);
    let meta =
        std::fs::metadata(&file).unwrap_or_else(|e| panic!("Foundry made {}: {e}", file.display()));
    let dir = world.iis.data_dir.clone().expect("the data directory");
    assert!(meta.is_file(), "{} is a regular file", file.display());
    assert_eq!(meta.mode() & 0o777, 0o600, "the key file's mode is 0600");
    assert_eq!(
        meta.uid(),
        own_uid(&dir),
        "the key file belongs to Foundry's user"
    );
}

/// "exactly one instance key file exists in the shared data directory".
pub fn then_one_key_file(world: &FoundryWorld) {
    let dir = world.iis.data_dir.clone().expect("the data directory");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("read the data directory")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec![KEY_FILE.to_string()],
        "exactly one instance key file, and no leftover, in the shared directory"
    );
    let key = std::fs::read_to_string(dir.join(KEY_FILE)).expect("read the key file");
    assert_eq!(
        base64url_decoded_len(key.trim()),
        Some(32),
        "the file holds one 32-byte key"
    );
}

fn base64url_decoded_len(text: &str) -> Option<usize> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.trim_end_matches('='))
        .ok()
        .map(|b| b.len())
}

// ===========================================================================
// Two servers
// ===========================================================================

/// "Priya generates the secrets key on the first server".
pub async fn when_generate_on_first(world: &mut FoundryWorld) {
    eip::capture_before(world).await;
    let answer = choose(world, "Priya", Button::Generate).await;
    world.eip.when_answer = Some(answer);
}

/// "within 60 seconds the second server's instance integrations page reads <words>".
pub async fn then_second_reads(world: &mut FoundryWorld, words: &str) {
    let url = format!("{}{INSTANCE_INTEGRATIONS}", second_base(world));
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < LIVE_BOUND {
        let page = eip::get(world, "Priya", &url).await;
        if page.status.is_success() && seen(world, &page).contains(words) {
            return;
        }
        last = tm::describe(&page);
        tokio::time::sleep(POLL).await;
    }
    panic!("within 60 seconds the second server's page reads {words:?}; last: {last}");
}

/// "the second server's health signals report the secrets key as degraded"
/// (DEVOPS D5, task 8: the reload path updates both alert inputs).
pub async fn then_second_degraded(world: &mut FoundryWorld) {
    let addr = world
        .iis
        .second
        .as_ref()
        .expect("a second Foundry server runs")
        .metrics_addr;
    let s = crate::support::metrics_scrape::scrape_metrics(addr).await;
    assert_eq!(
        s.sum_for("foundry_integration_secrets_degraded"),
        1.0,
        "the second server's degraded gauge reads 1"
    );
    let failures: f64 = s
        .samples_for("probe_failures_total")
        .iter()
        .filter(|m| m.labels.get("probe_name").map(String::as_str) == Some("secrets"))
        .map(|m| m.value)
        .sum();
    assert!(
        failures >= 1.0,
        "the second server counts a secrets probe failure on reload"
    );
}

// ===========================================================================
// Refused starts
// ===========================================================================

/// "Foundry is started with <setting>".
pub async fn when_started_with(world: &mut FoundryWorld, setting: BadStart) {
    ensure_double(world).await;
    configure(world, KeySetting::Operator);
    match setting {
        BadStart::ShortInstanceKey => world.iis.instance_key = Some(SHORT_KEY.into()),
        BadStart::ConflictingNames => world.iis.legacy_key = Some(OTHER_OPERATOR_KEY.into()),
    }
    apply(world);
    eip::start_expecting_refusal(world).await;
}

/// "Foundry refuses to start with a "health.startup.refused" line naming
/// <names>, never the value".
pub fn then_refused(world: &FoundryWorld, refusal: Refusal) {
    let (code, out) = world
        .eip
        .startup
        .clone()
        .expect("the operator started Foundry");
    assert!(
        code.is_some() && code != Some(0),
        "Foundry refuses to start; exit {code:?}"
    );
    assert!(
        out.contains("health.startup.refused"),
        "the refusal is a health.startup.refused line; {out}"
    );
    for name in refusal.names() {
        assert!(out.contains(name), "the refusal names {name}; {out}");
    }
    for value in [SHORT_KEY, OPERATOR_KEY, OTHER_OPERATOR_KEY] {
        assert!(!out.contains(value), "the refusal never prints a key value");
    }
}

// ===========================================================================
// KPI-IIS-G1: nothing shows, logs or stores a key or secret
// ===========================================================================

/// The instance key the scenario's Foundry holds: the operator's, or the
/// Foundry-made file's (read from the scenario's own directory).
fn held_instance_key(world: &FoundryWorld) -> Option<String> {
    if let Some(k) = world.iis.instance_key.clone().filter(|k| !k.is_empty()) {
        return Some(k);
    }
    if let Some(k) = world.iis.legacy_key.clone().filter(|k| !k.is_empty()) {
        return Some(k);
    }
    let file = world.iis.data_dir.as_ref()?.join(KEY_FILE);
    std::fs::read_to_string(file)
        .ok()
        .map(|k| k.trim().to_string())
}

/// Every planted value: the instance keys, the generated secrets key (through
/// the test seam, when one is stored), the Slack secrets and the bot tokens.
async fn planted(world: &FoundryWorld, with_generated: bool) -> Vec<String> {
    let mut values: Vec<String> = [
        OPERATOR_KEY,
        OTHER_OPERATOR_KEY,
        LEGACY_KEY,
        CLIENT_SECRET,
        SIGNING_SECRET,
        OTHER_CLIENT_SECRET,
        OTHER_SIGNING_SECRET,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(key) = held_instance_key(world) {
        values.push(key);
    }
    if with_generated {
        let instance_key = held_instance_key(world).expect("the instance key Foundry holds");
        let store = tm::pool(world);
        let generated = foundry_app::integrations::secrets_key_seam::generated_secrets_key(
            &store,
            &instance_key,
        )
        .await
        .expect("precondition: a secrets key is stored and opens with the instance key");
        values.push(generated);
    }
    if let Some(double) = &world.eip.slack {
        values.extend(double.issued_tokens());
    }
    values.sort();
    values.dedup();
    values
}

/// `value` as it could sit in a dump or a page: plain, base64 (standard and
/// url-safe, with and without padding) and hex.
fn encodings(value: &str) -> Vec<String> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    let bytes = value.as_bytes();
    vec![
        value.to_string(),
        STANDARD.encode(bytes),
        STANDARD_NO_PAD.encode(bytes),
        URL_SAFE.encode(bytes),
        URL_SAFE_NO_PAD.encode(bytes),
        hex::encode(bytes),
    ]
}

/// "the page shows no key and no Slack secret".
pub async fn then_page_shows_nothing_secret(world: &mut FoundryWorld) {
    let answer = answer(world);
    answered(&answer, "Priya");
    for value in planted(world, false).await {
        for form in encodings(&value) {
            assert!(
                !answer.body.contains(&form)
                    && !answer.location.clone().unwrap_or_default().contains(&form),
                "{} shows no key or secret, in any encoding",
                answer.path
            );
        }
    }
}

/// The whole database of the scenario's schema as text, one JSON row per line.
async fn dump(world: &FoundryWorld) -> String {
    let pool = tm::pool(world);
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'",
    )
    .fetch_all(&pool)
    .await
    .expect("list the tables");
    let mut out = String::new();
    for table in tables {
        let rows: String = sqlx::query_scalar(&format!(
            "SELECT coalesce(string_agg(row_to_json(t)::text, E'\\n'), '') FROM \"{table}\" t"
        ))
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("read {table}: {e}"));
        out.push_str(&rows);
        out.push('\n');
    }
    out
}

/// "the database's contents are read without the instance key".
pub async fn when_dump(world: &mut FoundryWorld) {
    world.iis.dump = Some(dump(world).await);
}

/// "neither the secrets key, the instance key, the Slack app's secrets nor
/// Canzan Labs' Slack token can be recovered from them": none appears in any
/// encoding, and no key-shaped value in the dump unwraps the stored key.
pub async fn then_dump_opens_nothing(world: &mut FoundryWorld) {
    let dump = world.iis.dump.clone().expect("the When read the database");
    for value in planted(world, true).await {
        for form in encodings(&value) {
            assert!(
                !dump.contains(&form),
                "a database dump holds no key, secret or token, in any encoding"
            );
        }
    }
    // An attacker's best try: every key-shaped value in the dump, offered to
    // the stored wrapped key through the test seam (opening stays inside the
    // integrations module, check-arch integrations-secret-seam).
    let candidates: Vec<String> = dump
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .filter(|t| base64url_decoded_len(t) == Some(32))
        .map(str::to_string)
        .collect();
    let store = tm::pool(world);
    for candidate in candidates {
        let opened =
            foundry_app::integrations::secrets_key_seam::generated_secrets_key(&store, &candidate)
                .await;
        assert!(
            opened.is_none(),
            "no value in the dump unwraps the stored secrets key"
        );
    }
}

/// "everything Foundry shows, logs and stores is searched for the keys and
/// the Slack secrets": every page Priya and Tomás open on the way, as the
/// operator's process serves them.
pub async fn when_sweep(world: &mut FoundryWorld) {
    let project = eip::integrations_path(world, "Identity Platform");
    let pages = [
        INSTANCE_INTEGRATIONS.to_string(),
        INSTANCE_DASHBOARD.to_string(),
        WORKSPACE_INTEGRATIONS.to_string(),
        SLACK_SETUP.to_string(),
        project.clone(),
        format!("{project}/routes/new?kind=slack"),
        format!("{project}/routes/new?kind=matrix"),
    ];
    let mut swept = Vec::new();
    for who in ["Priya", "Tomás"] {
        for path in &pages {
            swept.push(eip::get(world, who, path).await);
        }
    }
    world.iis.swept = swept;
    world.iis.dump = Some(dump(world).await);
}

/// "none of them appears in any page, page source, log line or activity
/// line, and none is stored in plaintext".
pub async fn then_sweep_clean(world: &mut FoundryWorld) {
    let instance = world
        .iis
        .swept
        .first()
        .cloned()
        .expect("the When swept the pages");
    answered(&instance, "Priya's instance integrations page");
    let output = eip::process_output_present(world);
    let dump = world.iis.dump.clone().expect("the When read the database");
    for value in planted(world, true).await {
        for form in encodings(&value) {
            for page in &world.iis.swept {
                assert!(
                    !page.body.contains(&form)
                        && !page.location.clone().unwrap_or_default().contains(&form),
                    "{} shows no key or secret in any encoding (activity lines included)",
                    page.path
                );
            }
            assert!(
                !output.contains(&form),
                "Foundry's log output carries no key or secret, in any encoding"
            );
        }
        assert!(
            !dump.contains(&value),
            "the database holds no key or secret in plaintext"
        );
    }
}
