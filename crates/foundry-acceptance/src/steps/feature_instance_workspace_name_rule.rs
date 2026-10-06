//! instance-workspace-name-rule step definitions
//! (`tests/features/instance-workspace-name-rule.feature`, DISTILL 2026-10-05,
//! every scenario `@pending`).
//!
//! The seams these steps drive are the DESIGN-pinned driving ports, nothing
//! internal (feature delta, DESIGN [REF] Driving Ports / Handoff):
//!
//! - P4 `POST /admin/instance/workspaces/{id}/rename` — through the iawr steps
//!   (`priya_renames`, the `workspace-rename-error` fragment, the record oracle).
//! - P3 `POST /bootstrap?token=…` — form `email`, `password`, `display_name`,
//!   `workspace_name`; a refusal is a 422 claim page with one `.error` carrying the
//!   copy and the three retained inputs (OQ-D6); a dead link is today's uniform
//!   refusal page, byte for byte (D8).
//! - P1 `POST /admin/instance/workspaces` — form `name`, `email`; a refusal is a
//!   422 full dashboard with `[data-provision-form] [data-provision-error]`
//!   carrying the copy and the retained `name`/`email` values (DDD-8, OQ-D6); a
//!   success is the shipped `instance-provisioned` fragment.
//! - P2 `foundry doctor provision-workspace --name … --admin-email … --as …` — the
//!   real binary as a subprocess; a refusal is exit 2, stderr
//!   `foundry doctor provision-workspace: <copy>`, empty stdout (DDD-10).
//!
//! Every step is real: HTTP against the in-process router with real session +
//! CSRF layers, SQL reads of the per-scenario Postgres schema, a real headless
//! Chrome, and the real `foundry` binary. No placeholder is needed at this layer:
//! DELIVER's new API (`foundry_core::WorkspaceName`, the typed `ProvisionRequest`,
//! the three door changes) is reached only through those ports, so every scenario
//! compiles and runs today and reds on behaviour, never on a missing symbol.
//!
//! STATE DELTA (Mandate 8): every attempt captures the [`NamingUniverse`] — every
//! workspace `(id, name)`, the row counts of every table a creation or rename can
//! touch, and the number of live bootstrap links — just before it, and the
//! outcome steps assert it after: a refusal moves NOTHING (fail-closed: any
//! undeclared change is a violation, D7/KPI-3); a provision adds exactly one
//! workspace, one user, one membership and one invite.
//!
//! LAYER 3 (real adapter + real HTTP/subprocess, `@real-io`): example-based only
//! (Mandates 9 and 11). The D4 set and its boundaries are pinned as properties
//! and exact pairs at the unit layer (`crates/foundry-core/tests/workspace_name.rs`);
//! here a few representative characters per class prove each door is wired to
//! the one rule, and the KPI-2 outlines prove the doors agree byte for byte.
//!
//! INVISIBLE CHARACTERS: a quoted name in the feature goes through
//! [`decode_invisibles`] (the `[TAB]` / `[U+202E]` marks documented in the
//! feature header) before it is sent or compared.

use crate::steps::feature_instance_admin_workspace_rename::{
    assert_head_fragment, assert_universe_delta, before as rename_before, capture_universe,
    harness, head_css, http, pool, priya_renames, resolve, wait_for_text, ERROR_FRAGMENT_ATTR,
    MARCO_EMAIL, MARCO_PASSWORD, PRIYA_EMAIL, PRIYA_PASSWORD,
};
use crate::support::harness::{ensure_postgres, signed_in_get, signed_in_post};
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use fantoccini::Locator;
use foundry_app::Clock;
use reqwest::StatusCode;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Duration;

/// DESIGN-pinned seams. If DELIVER moves one, the template and this module move
/// in the same change.
const DASHBOARD_PATH: &str = "/admin/instance/workspaces";
const PROVISION_ERROR_CSS: &str = "[data-provision-form] [data-provision-error]";
const PROVISION_NAME_CSS: &str = r#"[data-provision-form] input[name="name"]"#;
const PROVISION_EMAIL_CSS: &str = r#"[data-provision-form] input[name="email"]"#;
const PROVISIONED_FRAGMENT_CSS: &str = r#"[data-hx-fragment="instance-provisioned"]"#;
const RENAME_ERROR_CSS: &str = r#"[data-hx-fragment="workspace-rename-error"]"#;
const CLI_PREFIX: &str = "foundry doctor provision-workspace: ";
/// The command's own answer when `--name` or `--admin-email` is left out.
const MISSING_FLAGS_USAGE: &str = "foundry doctor provision-workspace: missing required flags. \
    Usage: foundry doctor provision-workspace --name <name> --admin-email <addr> \
    --as <super-admin-email>";

/// The bootstrap claim as Priya fills it in. The password is distinctive so a
/// page that echoes it anywhere is caught (D9: never echoed).
const CLAIM_EMAIL: &str = "priya@raman.family";
const CLAIM_PASSWORD: &str = "claim-password-never-echoed-4711";
const CLAIM_DISPLAY_NAME: &str = "Priya Raman";
/// The name a dead link is also asked with, to read its baseline answer.
const ACCEPTABLE_CLAIM_NAME: &str = "Raman Household";

/// The fixed test secret the in-process harness signs with (the slice-06 CLI
/// steps pass the same literal), so the CLI's invite link is signable.
const CLI_SESSION_SECRET: &str = "test-only-secret-must-be-at-least-32-bytes-long-please-yes";
/// A database address nothing listens on: a CLI that tries to connect to it
/// fails with exit 3, so exit 2 proves the rule ran first (D8, DDD-10).
const UNREACHABLE_DATABASE_URL: &str = "postgres://foundry:foundry@127.0.0.1:9/unreachable";

/// First admins used by the parity matrix's two provisioning doors.
const PARITY_DASHBOARD_ADMIN: &str = "dana@canzan.net";
const PARITY_CLI_ADMIN: &str = "erin@canzan.net";
const PARITY_CLAIM_EMAIL: &str = "parity-claim@raman.family";

/// Every table a workspace creation, a bootstrap claim, or a rename can write.
/// (`session` is deliberately absent: the steps' own sign-ins write it.)
const COUNTED_TABLES: [&str; 9] = [
    "users",
    "workspace_memberships",
    "teams",
    "team_memberships",
    "projects",
    "lanes",
    "invites",
    "instance_admins",
    "workspace_rename_events",
];

// ===========================================================================
// Domain types — the vocabulary the steps share
// ===========================================================================

/// The observable universe of naming a workspace (port-exposed: rows a door can
/// create, read back through SQL; never an internal struct).
#[derive(Debug, Clone, PartialEq)]
pub struct NamingUniverse {
    pub workspaces: BTreeMap<uuid::Uuid, String>,
    pub row_counts: BTreeMap<&'static str, i64>,
    pub live_bootstrap_links: i64,
}

/// What Priya typed into the bootstrap claim form (all fields, as submitted).
#[derive(Debug, Clone)]
pub struct ClaimSubmission {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub workspace_name: String,
}

/// Where the provisioning command finds (or fails to find) its database.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CliDatabase {
    /// The scenario's own schema (the default).
    #[default]
    Scenario,
    /// `DATABASE_URL` absent from the command's environment.
    NoneConfigured,
    /// `DATABASE_URL` points at an address nothing listens on.
    Unreachable,
}

/// The provisioning command line Priya types: each flag present with its value,
/// or `None` to leave that flag out entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningCommand {
    pub name: Option<String>,
    pub first_admin: Option<String>,
    pub acting_as: Option<String>,
}

impl ProvisioningCommand {
    /// Every flag present, Priya saying who she is.
    pub fn by_priya(name: Option<String>, first_admin: &str) -> Self {
        Self {
            name,
            first_admin: Some(first_admin.to_string()),
            acting_as: Some(PRIYA_EMAIL.to_string()),
        }
    }

    /// The same command without `--as`.
    pub fn without_saying_who(self) -> Self {
        Self {
            acting_as: None,
            ..self
        }
    }

    /// The same command without `--admin-email`.
    pub fn without_first_admin(self) -> Self {
        Self {
            first_admin: None,
            ..self
        }
    }
}

/// The four doors, in the order the parity matrix visits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    Rename,
    BootstrapClaim,
    DashboardProvision,
    Cli,
}

/// One door's verdict on one name, reduced to what the operator sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Refused with this copy (the text of the door's refusal slot / stderr line).
    Refused(String),
    /// Accepted; the workspace is now stored under this name.
    Accepted(String),
    /// Anything else (a 500, a 404, an unexpected exit) — always a failure.
    Other(String),
}

/// Expand the feature's invisible-character marks (header of the feature file)
/// into the characters they stand for. Unknown bracketed text is kept literally,
/// so a name like `<b>[draft]</b>` survives untouched.
pub(crate) fn decode_invisibles(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find(']') else {
            out.push_str(after);
            return out;
        };
        let token = &after[1..close];
        let decoded = match token {
            "TAB" => Some('\t'),
            "NEWLINE" => Some('\n'),
            "NUL" => Some('\0'),
            "SPACE" => Some(' '),
            "NBSP" => Some('\u{00A0}'),
            "ZWJ" => Some('\u{200D}'),
            "ZWNJ" => Some('\u{200C}'),
            _ => token
                .strip_prefix("U+")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => out.push(c),
            None => out.push_str(&after[..=close]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

// ===========================================================================
// The universe (Mandate 8)
// ===========================================================================

pub(crate) async fn capture_naming_universe(world: &FoundryWorld) -> NamingUniverse {
    let pool = pool(world);
    let workspaces: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, name FROM workspaces")
        .fetch_all(&pool)
        .await
        .expect("read every workspace");
    let mut row_counts = BTreeMap::new();
    for table in COUNTED_TABLES {
        let (n,): (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("count {table}: {e}"));
        row_counts.insert(table, n);
    }
    let (live,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM bootstrap_tokens WHERE used_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("count live bootstrap links");
    NamingUniverse {
        workspaces: workspaces.into_iter().collect(),
        row_counts,
        live_bootstrap_links: live,
    }
}

fn naming_before(world: &FoundryWorld) -> NamingUniverse {
    world
        .iwnr_before
        .clone()
        .expect("the naming universe was captured before the attempt")
}

/// Fail-closed: a refusal changes no workspace and creates no row anywhere.
fn assert_nothing_moved(before: &NamingUniverse, after: &NamingUniverse) {
    assert_eq!(
        after.workspaces, before.workspaces,
        "a refusal must create, rename or remove no workspace (D7)"
    );
    assert_eq!(
        after.row_counts, before.row_counts,
        "a refusal must leave no user, membership, team, project, lane, invite, instance admin \
         or rename record behind (D7, KPI-3)"
    );
    assert_eq!(
        after.live_bootstrap_links, before.live_bootstrap_links,
        "a refusal must consume no bootstrap link (D7, KPI-3)"
    );
}

/// A provision adds exactly one workspace named `name`, one user, one membership
/// and one invite (the shipped `Store::provision_workspace` transaction); nothing
/// else moves, and every existing workspace keeps its name.
fn assert_provisioned_exactly(before: &NamingUniverse, after: &NamingUniverse, name: &str) {
    for (id, old) in &before.workspaces {
        assert_eq!(
            after.workspaces.get(id),
            Some(old),
            "an existing workspace must be untouched by provisioning"
        );
    }
    let added: Vec<&String> = after
        .workspaces
        .iter()
        .filter(|(id, _)| !before.workspaces.contains_key(id))
        .map(|(_, n)| n)
        .collect();
    assert_eq!(
        added,
        vec![name],
        "exactly one workspace named {name:?} must be added"
    );
    for (table, n) in &before.row_counts {
        let grew_by = after.row_counts[table] - n;
        let expected = match *table {
            "users" | "workspace_memberships" | "invites" => 1,
            _ => 0,
        };
        assert_eq!(
            grew_by, expected,
            "provisioning must add {expected} row(s) to {table}, added {grew_by}"
        );
    }
}

// ===========================================================================
// Driving the doors
// ===========================================================================

fn sha256(s: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().into()
}

/// The raw token value of a link the scenario named (minted by the us-05 Given);
/// a never-minted name is used verbatim (the "never issued" arm).
fn raw_link(world: &FoundryWorld, name: &str) -> String {
    world
        .minted_tokens
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

async fn post_claim(
    world: &FoundryWorld,
    raw_token: &str,
    claim: &ClaimSubmission,
) -> (StatusCode, String, Option<String>) {
    let url = format!(
        "{}/bootstrap?token={}",
        harness(world).base_url(),
        urlencoding::encode(raw_token)
    );
    let form = [
        ("email", claim.email.as_str()),
        ("password", claim.password.as_str()),
        ("display_name", claim.display_name.as_str()),
        ("workspace_name", claim.workspace_name.as_str()),
    ];
    let resp = http(world)
        .post(url)
        .form(&form)
        .send()
        .await
        .expect("submit the bootstrap claim");
    let status = resp.status();
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = resp.text().await.unwrap_or_default();
    (status, body, location)
}

fn priyas_claim(workspace_name: &str) -> ClaimSubmission {
    ClaimSubmission {
        email: CLAIM_EMAIL.to_string(),
        password: CLAIM_PASSWORD.to_string(),
        display_name: CLAIM_DISPLAY_NAME.to_string(),
        workspace_name: workspace_name.to_string(),
    }
}

async fn priya_claims(world: &mut FoundryWorld, link: &str, workspace_name: &str) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let claim = priyas_claim(&decode_invisibles(workspace_name));
    let raw = raw_link(world, link);
    let (status, body, location) = post_claim(world, &raw, &claim).await;
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(loc) = location {
        headers.insert(
            reqwest::header::LOCATION,
            reqwest::header::HeaderValue::from_str(&loc).expect("location header"),
        );
    }
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
    world.iwnr_claim = Some(claim);
}

async fn priya_provisions(world: &mut FoundryWorld, name: &str, first_admin: &str) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let name = decode_invisibles(name);
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
        &[("name", name.as_str()), ("email", first_admin)],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
    world.iwnr_provision = Some((name, first_admin.to_string()));
}

/// Run the REAL `foundry doctor provision-workspace` with exactly the flags of
/// `command`; a `None` flag is left out of the command line entirely.
async fn run_provisioning_command(
    world: &FoundryWorld,
    command: ProvisioningCommand,
) -> (i32, String, String) {
    let database_url = match world.iwnr_cli_database {
        CliDatabase::Scenario => {
            let base = ensure_postgres().await;
            let schema = harness(world).schema.clone();
            Some(format!("{base}?options=-csearch_path%3D{schema}"))
        }
        CliDatabase::NoneConfigured => None,
        CliDatabase::Unreachable => Some(UNREACHABLE_DATABASE_URL.to_string()),
    };
    let output = tokio::task::spawn_blocking(move || {
        let mut cmd = AssertCommand::cargo_bin("foundry").expect("cargo-bin foundry");
        match database_url {
            Some(url) => cmd.env("DATABASE_URL", url),
            None => cmd.env_remove("DATABASE_URL"),
        };
        cmd.env("SESSION_SECRET", CLI_SESSION_SECRET)
            .env("FOUNDRY_PUBLIC_URL", "http://localhost")
            .args(["doctor", "provision-workspace"]);
        for (flag, value) in [
            ("--name", &command.name),
            ("--admin-email", &command.first_admin),
            ("--as", &command.acting_as),
        ] {
            if let Some(value) = value {
                cmd.args([flag, value.as_str()]);
            }
        }
        cmd.timeout(Duration::from_secs(60))
            .output()
            .expect("invoke foundry doctor provision-workspace")
    })
    .await
    .expect("join the blocking command");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

async fn priya_runs_command(world: &mut FoundryWorld, command: ProvisioningCommand) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let out = run_provisioning_command(world, command).await;
    world.iwnr_cli = Some(out);
}

fn cli_result(world: &FoundryWorld) -> (i32, String, String) {
    world
        .iwnr_cli
        .clone()
        .expect("the provisioning command ran")
}

// ===========================================================================
// Reading an answer
// ===========================================================================

fn texts(html: &str, css: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css).expect("selector");
    doc.select(&sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect()
}

/// Like [`texts`], but the text exactly as served: NOT trimmed. For an echo
/// whose edges are the point (a trimmed name must not come back padded).
fn raw_texts(html: &str, css: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css).expect("selector");
    doc.select(&sel)
        .map(|el| el.text().collect::<String>())
        .collect()
}

fn input_value(html: &str, css: &str) -> Option<Option<String>> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css).expect("selector");
    let el = doc.select(&sel).next()?;
    Some(el.value().attr("value").map(str::to_string))
}

/// The stored name of the workspace whose admin is the user with this email.
async fn workspace_of_first_admin(world: &FoundryWorld, email: &str) -> Option<String> {
    sqlx::query_as::<_, (String,)>(
        "SELECT w.name FROM workspaces w
           JOIN workspace_memberships m ON m.workspace_id = w.id
           JOIN users u ON u.id = m.user_id
          WHERE u.email_lower = $1",
    )
    .bind(email.to_ascii_lowercase())
    .fetch_optional(&pool(world))
    .await
    .expect("read the first admin's workspace")
    .map(|(n,)| n)
}

fn snippet(body: &str) -> String {
    body.chars().take(300).collect()
}

/// Exactly one element matches `css`, and its trimmed text is `copy`.
fn assert_one_refusal(body: &str, css: &str, copy: &str, what: &str) {
    let found = texts(body, css);
    assert_eq!(
        found,
        vec![copy.to_string()],
        "{what} must carry exactly one {css} reading {copy:?}; body = {body:?}"
    );
}

// ===========================================================================
// Given
// ===========================================================================

/// Store fixture (DDD-6: the store does not validate): a name written the way a
/// pre-rule door or a restore would have left it.
#[given(regex = r#"^workspace "([^"]+)" was named before the rule existed$"#)]
async fn legacy_workspace(world: &mut FoundryWorld, name: String) {
    let name = decode_invisibles(&name);
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(uuid::Uuid::now_v7())
        .bind(&name)
        .execute(&pool(world))
        .await
        .expect("seed a legacy workspace name");
}

#[given(
    regex = r#"^Priya's claim through link "([^"]+)" was refused for the workspace name "([^"]*)"$"#
)]
async fn claim_was_refused(world: &mut FoundryWorld, link: String, name: String) {
    priya_claims(world, &link, &name).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the claim with {name:?} must be refused for its name; body = {:?}",
        world.last_body.as_deref().map(snippet)
    );
    assert_nothing_moved(&naming_before(world), &capture_naming_universe(world).await);
}

#[given(
    regex = r#"^Priya's dashboard provision of "([^"]*)" for first admin "([^"]+)" was refused for its name$"#
)]
async fn provision_was_refused(world: &mut FoundryWorld, name: String, first_admin: String) {
    priya_provisions(world, &name, &first_admin).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the provision of {name:?} must be refused for its name; body = {:?}",
        world.last_body.as_deref().map(snippet)
    );
    assert_nothing_moved(&naming_before(world), &capture_naming_universe(world).await);
}

#[given(regex = r"^the command reaches the instance's database$")]
async fn database_configured(world: &mut FoundryWorld) {
    world.iwnr_cli_database = CliDatabase::Scenario;
}

#[given(regex = r"^no database is configured for the command$")]
async fn no_database_configured(world: &mut FoundryWorld) {
    world.iwnr_cli_database = CliDatabase::NoneConfigured;
}

#[given(regex = r"^the command's database cannot be reached$")]
async fn database_unreachable(world: &mut FoundryWorld) {
    world.iwnr_cli_database = CliDatabase::Unreachable;
}

// ===========================================================================
// When — the rename door (P4)
// ===========================================================================

#[when(regex = r#"^Priya renames workspace "([^"]+)" to the pasted name "([^"]*)"$"#)]
async fn priya_renames_pasted(world: &mut FoundryWorld, label: String, pasted: String) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    priya_renames(
        world,
        &decode_invisibles(&label),
        &decode_invisibles(&pasted),
    )
    .await;
}

/// A browser cannot TYPE a tab into a text input (the key moves focus), so the
/// paste is simulated the way a clipboard paste lands: the value is set, then the
/// row's own submit button is pressed.
#[when(
    regex = r#"^she pastes "([^"]+)" over the "([^"]+)" workspace name in her browser and submits it$"#
)]
async fn pastes_in_browser(world: &mut FoundryWorld, pasted: String, label: String) {
    let target = resolve(world, &label).await;
    world.iawr_target = Some(target);
    let head = head_css(target);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(&format!("{head} input[name='name']")))
        .await
        .unwrap_or_else(|e| panic!("the {label:?} row must carry a rename input: {e}"));
    browser
        .execute(
            "document.querySelector(arguments[0]).value = arguments[1];",
            vec![
                serde_json::Value::String(format!("{head} input[name='name']")),
                serde_json::Value::String(decode_invisibles(&pasted)),
            ],
        )
        .await
        .expect("paste the name into the rename input");
    browser
        .find(Locator::Css(&format!("{head} button[type='submit']")))
        .await
        .expect("the workspace head must carry a submit button")
        .click()
        .await
        .expect("submit the rename");
}

// ===========================================================================
// When — the bootstrap claim (P3)
// ===========================================================================

#[when(
    regex = r#"^Priya claims the instance through link "([^"]+)" naming the workspace "([^"]*)"$"#
)]
async fn when_priya_claims(world: &mut FoundryWorld, link: String, name: String) {
    priya_claims(world, &link, &name).await;
}

#[when(regex = r#"^Priya opens the claim link "([^"]+)"$"#)]
async fn priya_opens_claim_link(world: &mut FoundryWorld, link: String) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let url = format!(
        "{}/bootstrap?token={}",
        harness(world).base_url(),
        urlencoding::encode(&raw_link(world, &link))
    );
    let resp = http(world)
        .get(url)
        .send()
        .await
        .expect("open the bootstrap claim link");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// For each dead link: its answer to an acceptable name is the baseline, and
/// every unfit name must get that exact answer back (status and body).
#[when(
    regex = r#"^a visitor posts claims with unfit workspace names through the links "([^"]+)", "([^"]+)" and "([^"]+)"$"#
)]
async fn dead_link_claims(world: &mut FoundryWorld, a: String, b: String, c: String) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let unfit = [
        "Raman Household Operations Center",
        "   ",
        "Raman\nHousehold",
        "Raman\0Household",
    ];
    let mut mismatches = Vec::new();
    let mut compared = 0usize;
    for link in [a, b, c] {
        let raw = raw_link(world, &link);
        let (base_status, base_body, _) =
            post_claim(world, &raw, &priyas_claim(ACCEPTABLE_CLAIM_NAME)).await;
        assert_eq!(
            base_status,
            StatusCode::OK,
            "dead link {link:?} must answer the acceptable name with the uniform refusal page; body = {}",
            snippet(&base_body)
        );
        for name in unfit {
            let (status, body, _) = post_claim(world, &raw, &priyas_claim(name)).await;
            compared += 1;
            if status != base_status || body != base_body {
                mismatches.push(format!(
                    "link {link:?} with {name:?}: {status} {:?} vs baseline {base_status}",
                    snippet(&body)
                ));
            }
        }
    }
    world.iwnr_dead_link_report = Some((compared, mismatches));
}

// ===========================================================================
// When — the dashboard Provision form (P1)
// ===========================================================================

#[when(
    regex = r#"^Priya provisions workspace "([^"]*)" for first admin "([^"]+)" from the dashboard$"#
)]
async fn when_priya_provisions(world: &mut FoundryWorld, name: String, first_admin: String) {
    priya_provisions(world, &name, &first_admin).await;
}

#[when(regex = r"^Marco posts a dashboard provision of a 40-character workspace name$")]
async fn marco_provisions(world: &mut FoundryWorld) {
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let name = "Canzan Labs Platform Engineering and SRE";
    assert_eq!(
        name.chars().count(),
        40,
        "the example must be 40 characters"
    );
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        MARCO_EMAIL,
        MARCO_PASSWORD,
        DASHBOARD_PATH,
        &[("name", name), ("email", "dana@canzan.net")],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

#[when(regex = r#"^she provisions workspace "([^"]+)" for first admin "([^"]+)" in her browser$"#)]
async fn provisions_in_browser(world: &mut FoundryWorld, name: String, first_admin: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let name_input = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(PROVISION_NAME_CSS))
        .await
        .unwrap_or_else(|e| panic!("the dashboard must carry the Provision form: {e}"));
    name_input.clear().await.expect("clear the name input");
    name_input
        .send_keys(&decode_invisibles(&name))
        .await
        .expect("type the workspace name");
    let email_input = browser
        .find(Locator::Css(PROVISION_EMAIL_CSS))
        .await
        .expect("the Provision form must carry a first-admin email input");
    email_input.clear().await.expect("clear the email input");
    email_input
        .send_keys(&first_admin)
        .await
        .expect("type the first-admin email");
    browser
        .find(Locator::Css("[data-provision-form] button[type='submit']"))
        .await
        .expect("the Provision form must carry a submit button")
        .click()
        .await
        .expect("submit the Provision form");
}

// ===========================================================================
// When — the operator CLI (P2)
// ===========================================================================

#[when(
    regex = r#"^Priya runs the provisioning command naming the workspace "([^"]*)" for first admin "([^"]+)"$"#
)]
async fn when_priya_runs_command(world: &mut FoundryWorld, name: String, first_admin: String) {
    priya_runs_command(
        world,
        ProvisioningCommand::by_priya(Some(decode_invisibles(&name)), &first_admin),
    )
    .await;
}

#[when(
    regex = r#"^Priya runs the provisioning command without a workspace name for first admin "([^"]+)"$"#
)]
async fn when_priya_runs_command_without_name(world: &mut FoundryWorld, first_admin: String) {
    priya_runs_command(world, ProvisioningCommand::by_priya(None, &first_admin)).await;
}

#[when(
    regex = r#"^Priya runs the provisioning command naming the workspace "([^"]*)" for first admin "([^"]+)" without saying who she is$"#
)]
async fn when_priya_runs_command_without_saying_who(
    world: &mut FoundryWorld,
    name: String,
    first_admin: String,
) {
    let command = ProvisioningCommand::by_priya(Some(decode_invisibles(&name)), &first_admin);
    priya_runs_command(world, command.without_saying_who()).await;
}

#[when(
    regex = r#"^Priya runs the provisioning command naming the workspace "([^"]*)" without naming a first admin$"#
)]
async fn when_priya_runs_command_without_first_admin(world: &mut FoundryWorld, name: String) {
    let command = ProvisioningCommand::by_priya(Some(decode_invisibles(&name)), "");
    priya_runs_command(world, command.without_first_admin()).await;
}

// ===========================================================================
// When — the parity matrix (KPI-2)
// ===========================================================================

async fn offer_at_rename(world: &mut FoundryWorld, name: &str) -> Verdict {
    priya_renames(world, "Household", name).await;
    let target = world.iawr_target.expect("the rename was aimed");
    let status = world.last_status.expect("status");
    let body = world.last_body.clone().unwrap_or_default();
    match status {
        StatusCode::UNPROCESSABLE_ENTITY if body.contains(ERROR_FRAGMENT_ATTR) => {
            Verdict::Refused(texts(&body, RENAME_ERROR_CSS).join(" | "))
        }
        StatusCode::OK => {
            let (stored,): (String,) = sqlx::query_as("SELECT name FROM workspaces WHERE id = $1")
                .bind(target)
                .fetch_one(&pool(world))
                .await
                .expect("read the renamed workspace");
            Verdict::Accepted(stored)
        }
        other => Verdict::Other(format!("{other}: {}", snippet(&body))),
    }
}

async fn mint_parity_link(world: &mut FoundryWorld) -> String {
    let raw = format!("parity-link-{}", uuid::Uuid::now_v7());
    let h = harness(world);
    let expires_at = h.fake_clock.now() + time::Duration::minutes(30);
    h.app
        .state
        .store
        .insert_bootstrap_token(uuid::Uuid::now_v7(), &sha256(&raw), expires_at)
        .await
        .expect("mint a live bootstrap link");
    raw
}

async fn offer_at_bootstrap(world: &mut FoundryWorld, raw: &str, name: &str) -> Verdict {
    let claim = ClaimSubmission {
        email: PARITY_CLAIM_EMAIL.to_string(),
        ..priyas_claim(name)
    };
    let (status, body, _) = post_claim(world, raw, &claim).await;
    match status {
        StatusCode::UNPROCESSABLE_ENTITY => Verdict::Refused(texts(&body, ".error").join(" | ")),
        StatusCode::SEE_OTHER => match workspace_of_first_admin(world, PARITY_CLAIM_EMAIL).await {
            Some(stored) => Verdict::Accepted(stored),
            None => Verdict::Other("claimed, but no workspace for the claimant".into()),
        },
        other => Verdict::Other(format!("{other}: {}", snippet(&body))),
    }
}

async fn offer_at_dashboard(world: &mut FoundryWorld, name: &str) -> Verdict {
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
        &[("name", name), ("email", PARITY_DASHBOARD_ADMIN)],
    )
    .await;
    match outcome.status {
        StatusCode::UNPROCESSABLE_ENTITY => {
            Verdict::Refused(texts(&outcome.body, PROVISION_ERROR_CSS).join(" | "))
        }
        StatusCode::OK => match workspace_of_first_admin(world, PARITY_DASHBOARD_ADMIN).await {
            Some(stored) => Verdict::Accepted(stored),
            None => Verdict::Other("200, but no workspace for the first admin".into()),
        },
        other => Verdict::Other(format!("{other}: {}", snippet(&outcome.body))),
    }
}

async fn offer_at_cli(world: &mut FoundryWorld, name: &str) -> Verdict {
    let (exit, stdout, stderr) = run_provisioning_command(
        world,
        ProvisioningCommand::by_priya(Some(name.to_string()), PARITY_CLI_ADMIN),
    )
    .await;
    match exit {
        2 => match stderr.lines().find_map(|l| l.strip_prefix(CLI_PREFIX)) {
            Some(copy) => Verdict::Refused(copy.to_string()),
            None => Verdict::Other(format!("exit 2 without a refusal line: {stderr:?}")),
        },
        0 => match workspace_of_first_admin(world, PARITY_CLI_ADMIN).await {
            Some(stored) => Verdict::Accepted(stored),
            None => Verdict::Other(format!("exit 0, but no workspace: {stdout:?}")),
        },
        other => Verdict::Other(format!("exit {other}: stdout={stdout:?} stderr={stderr:?}")),
    }
}

async fn offer_at_doors(world: &mut FoundryWorld, pasted: &str, doors: &[Door]) {
    let name = decode_invisibles(pasted);
    // The bootstrap door's live link is a PRECONDITION, so it is minted BEFORE the
    // "before" snapshot: a correct refusal leaves it live, and the universe must
    // then read unchanged (live links 1 -> 1), not 0 -> 1.
    let link = mint_parity_link(world).await;
    world.iwnr_before = Some(capture_naming_universe(world).await);
    let mut verdicts = Vec::new();
    for door in doors {
        let verdict = match door {
            Door::Rename => offer_at_rename(world, &name).await,
            Door::BootstrapClaim => offer_at_bootstrap(world, &link, &name).await,
            Door::DashboardProvision => offer_at_dashboard(world, &name).await,
            Door::Cli => offer_at_cli(world, &name).await,
        };
        verdicts.push((*door, verdict));
    }
    world.iwnr_verdicts = verdicts;
}

#[when(regex = r#"^Priya offers the workspace name "([^"]*)" at every door$"#)]
async fn offer_at_every_door(world: &mut FoundryWorld, pasted: String) {
    let doors = [
        Door::Rename,
        Door::BootstrapClaim,
        Door::DashboardProvision,
        Door::Cli,
    ];
    offer_at_doors(world, &pasted, &doors).await;
}

/// A process argument cannot carry a NUL (the OS refuses it before `foundry`
/// runs), so the NUL row visits the three web doors only.
#[when(regex = r#"^Priya offers the workspace name "([^"]*)" at every web door$"#)]
async fn offer_at_every_web_door(world: &mut FoundryWorld, pasted: String) {
    let doors = [Door::Rename, Door::BootstrapClaim, Door::DashboardProvision];
    offer_at_doors(world, &pasted, &doors).await;
}

// ===========================================================================
// Then — refusals change nothing
// ===========================================================================

#[then(regex = r"^nothing was created or changed$")]
async fn nothing_created_or_changed(world: &mut FoundryWorld) {
    let after = capture_naming_universe(world).await;
    assert_nothing_moved(&naming_before(world), &after);
}

// ===========================================================================
// Then — the rename door (P4)
// ===========================================================================

#[then(
    regex = r#"^the workspace row she gets back shows the pasted name "([^"]+)" and carries no error$"#
)]
async fn row_shows_pasted_no_error(world: &mut FoundryWorld, pasted: String) {
    let name = decode_invisibles(&pasted);
    assert_head_fragment(world, &name);
    let body = world.last_body.as_deref().expect("answer captured");
    assert!(
        !body.contains(ERROR_FRAGMENT_ATTR),
        "a quiet success carries no refusal fragment; got {body:?}"
    );
}

#[then(regex = r#"^the pasted name "([^"]+)" is stored exactly, with one rename on record$"#)]
async fn pasted_name_stored(world: &mut FoundryWorld, pasted: String) {
    let name = decode_invisibles(&pasted);
    assert_head_fragment(world, &name);
    let target = world.iawr_target.expect("a rename was aimed");
    let after = capture_universe(world).await;
    assert_universe_delta(
        &rename_before(world),
        &after,
        Some((target, &name)),
        world.iapr_priya_id,
        world.iawr_window,
    );
}

// ===========================================================================
// Then — the bootstrap claim (P3)
// ===========================================================================

#[then(regex = r#"^the claim page is shown again saying "([^"]+)"$"#)]
async fn claim_page_again(world: &mut FoundryWorld, copy: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "a live link with an unfit name gets the claim page again with status 422 (D9, DDD-9); body = {}",
        snippet(&body)
    );
    assert!(
        !texts(&body, r#"form[action^="/bootstrap?token="]"#).is_empty(),
        "the 422 answer must be the claim page, with its form to correct; body = {body:?}"
    );
    assert_one_refusal(&body, ".error", &copy, "the claim page");
}

#[then(regex = r#"^the claim page for link "([^"]+)" is shown with an empty form and no error$"#)]
async fn empty_claim_page(world: &mut FoundryWorld, link: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "a live link opens the claim page; body = {}",
        snippet(&body)
    );
    let expected_action = format!("/bootstrap?token={}", raw_link(world, &link));
    let doc = Html::parse_document(&body);
    let form = Selector::parse(r#"form[method="post"]"#).expect("selector");
    let actions: Vec<_> = doc
        .select(&form)
        .filter_map(|f| f.value().attr("action"))
        .collect();
    assert_eq!(
        actions,
        vec![expected_action.as_str()],
        "the claim page must carry one form posting back to the link it was opened with; \
         body = {body:?}"
    );
    for field in ["email", "password", "display_name", "workspace_name"] {
        let value = input_value(&body, &format!(r#"input[name="{field}"]"#))
            .unwrap_or_else(|| panic!("the claim form must ask for {field}; body = {body:?}"));
        assert!(
            value.as_deref().unwrap_or("").is_empty(),
            "a freshly opened claim form starts with {field} empty, got {value:?}"
        );
    }
    assert!(
        texts(&body, ".error").is_empty(),
        "a freshly opened claim form carries no error; body = {body:?}"
    );
}

#[then(
    regex = r"^her email, display name and workspace name are still filled in, and her password is not$"
)]
async fn claim_fields_kept(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let claim = world.iwnr_claim.clone().expect("a claim was submitted");
    for (field, expected) in [
        ("email", &claim.email),
        ("display_name", &claim.display_name),
        ("workspace_name", &claim.workspace_name),
    ] {
        assert_eq!(
            input_value(&body, &format!(r#"input[name="{field}"]"#)),
            Some(Some(expected.clone())),
            "the claim page must keep {field} as submitted (D9); body = {body:?}"
        );
    }
    let password = input_value(&body, r#"input[name="password"]"#)
        .expect("the claim page must still ask for the password");
    assert!(
        password.as_deref().unwrap_or("").is_empty(),
        "the password input must come back empty (D9), got value {password:?}"
    );
    assert!(
        !body.contains(&claim.password),
        "the submitted password must appear nowhere on the page (D9)"
    );
}

#[then(
    regex = r"^each link answers every unfit name byte-identically to its answer for an acceptable name$"
)]
async fn dead_links_byte_identical(world: &mut FoundryWorld) {
    let (compared, mismatches) = world
        .iwnr_dead_link_report
        .clone()
        .expect("the dead-link claims were posted");
    assert_eq!(
        compared, 12,
        "three dead links x four unfit names must have been compared"
    );
    assert!(
        mismatches.is_empty(),
        "a dead link must answer any name byte-identically (D8, no new oracle): {mismatches:#?}"
    );
}

// ===========================================================================
// Then — the dashboard Provision form (P1)
// ===========================================================================

#[then(
    regex = r#"^the dashboard confirms workspace "([^"]+)" was provisioned for first admin "([^"]+)"$"#
)]
async fn dashboard_confirms(world: &mut FoundryWorld, name: String, first_admin: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the provision must succeed; body = {}",
        snippet(&body)
    );
    // Untrimmed on purpose: a fragment echoing the raw `form.name` ("  Globex  ")
    // instead of the validated name must fail here (DDD-8, scenario 18).
    assert_eq!(
        raw_texts(&body, &format!("{PROVISIONED_FRAGMENT_CSS} strong")),
        vec![name.clone()],
        "the confirmation must show exactly the stored (trimmed) name, no padding; \
         body = {body:?}"
    );
    assert!(
        body.contains(&format!(r#"data-first-admin-email="{first_admin}""#)),
        "the confirmation must name the first admin; body = {body:?}"
    );
    let after = capture_naming_universe(world).await;
    assert_provisioned_exactly(&naming_before(world), &after, &name);
}

#[then(regex = r#"^the dashboard refuses the provision saying "([^"]+)"$"#)]
async fn dashboard_refuses(world: &mut FoundryWorld, copy: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "an unfit name gets 422 (D9, DDD-8); body = {}",
        snippet(&body)
    );
    assert!(
        !texts(&body, "[data-instance-dashboard]").is_empty(),
        "the 422 answer must be the whole dashboard, so the form is there to correct (DDD-8); \
         body = {body:?}"
    );
    assert_one_refusal(&body, PROVISION_ERROR_CSS, &copy, "the Provision form");
}

#[then(
    regex = r#"^the provision form still holds the name "([^"]+)" and the first admin "([^"]+)"$"#
)]
async fn provision_form_kept(world: &mut FoundryWorld, name: String, first_admin: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        input_value(&body, PROVISION_NAME_CSS),
        Some(Some(decode_invisibles(&name))),
        "the Provision form must keep the submitted name (DDD-8); body = {body:?}"
    );
    assert_eq!(
        input_value(&body, PROVISION_EMAIL_CSS),
        Some(Some(first_admin)),
        "the Provision form must keep the first-admin email (D9); body = {body:?}"
    );
}

#[then(regex = r#"^the instance dashboard lists workspace "([^"]+)"$"#)]
async fn dashboard_lists(world: &mut FoundryWorld, name: String) {
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    assert_eq!(outcome.status, StatusCode::OK, "the dashboard must render");
    assert!(
        texts(&outcome.body, "[data-workspace-list] [data-workspace-name]").contains(&name),
        "the dashboard must list workspace {name:?}"
    );
}

#[then(regex = r#"^"([^"]+)" appears in the Provision form in her browser$"#)]
async fn provision_error_in_browser(world: &mut FoundryWorld, copy: String) {
    wait_for_text(
        world,
        PROVISION_ERROR_CSS,
        &format!("{copy:?} must appear in the Provision form's error slot (D9, DDD-8)"),
        |text| text == copy,
    )
    .await;
}

#[then(
    regex = r#"^the Provision form in her browser still holds the name "([^"]+)" and the first admin "([^"]+)"$"#
)]
async fn provision_form_kept_in_browser(
    world: &mut FoundryWorld,
    name: String,
    first_admin: String,
) {
    let browser = world.browser.as_ref().expect("browser session");
    for (css, expected) in [
        (PROVISION_NAME_CSS, decode_invisibles(&name)),
        (PROVISION_EMAIL_CSS, first_admin),
    ] {
        let value = browser
            .find(Locator::Css(css))
            .await
            .unwrap_or_else(|e| panic!("the Provision form must still carry {css}: {e}"))
            .prop("value")
            .await
            .expect("read the input's value");
        assert_eq!(
            value.as_deref(),
            Some(expected.as_str()),
            "the Provision form must keep what Priya typed in {css}"
        );
    }
}

// ===========================================================================
// Then — the operator CLI (P2)
// ===========================================================================

#[then(regex = r#"^the command exits 2 saying "([^"]+)" with nothing on standard output$"#)]
async fn command_refuses(world: &mut FoundryWorld, copy: String) {
    let (exit, stdout, stderr) = cli_result(world);
    assert_eq!(
        exit, 2,
        "an unfit name is an argument error: exit 2 (D8, DDD-10); stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.is_empty(),
        "a refusal prints nothing on stdout; got {stdout:?}"
    );
    let line = format!("{CLI_PREFIX}{copy}");
    assert!(
        stderr.lines().any(|l| l == line),
        "stderr must carry the line {line:?}; got {stderr:?}"
    );
}

#[then(regex = r#"^neither output carries the forged line "([^"]+)"$"#)]
async fn no_forged_line(world: &mut FoundryWorld, forged: String) {
    let (_, stdout, stderr) = cli_result(world);
    for (stream, text) in [("stdout", &stdout), ("stderr", &stderr)] {
        assert!(
            !text.lines().any(|l| l.trim() == forged),
            "{stream} must not carry a forged {forged:?} line (DDD-11); got {text:?}"
        );
    }
}

#[then(regex = r#"^the command exits 0 and reports "([^"]+)"$"#)]
async fn command_succeeds(world: &mut FoundryWorld, line: String) {
    let (exit, stdout, stderr) = cli_result(world);
    assert_eq!(
        exit, 0,
        "the provision must succeed; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.lines().any(|l| l == line),
        "stdout must carry the line {line:?} (D10); got {stdout:?}"
    );
}

#[then(regex = r#"^the provisioned workspace is stored as "([^"]+)"$"#)]
async fn provisioned_stored_as(world: &mut FoundryWorld, name: String) {
    let (_, stdout, _) = cli_result(world);
    let id: uuid::Uuid = stdout
        .lines()
        .find_map(|l| l.strip_prefix("workspace-id: "))
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| panic!("stdout must report the new workspace id; got {stdout:?}"));
    let (stored,): (String,) = sqlx::query_as("SELECT name FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_one(&pool(world))
        .await
        .expect("read the provisioned workspace");
    assert_eq!(stored, name, "the CLI must store the trimmed name (D10)");
    let after = capture_naming_universe(world).await;
    assert_provisioned_exactly(&naming_before(world), &after, &name);
}

#[then(regex = r"^the command exits 2 with its usage line and no name-rule message$")]
async fn command_usage(world: &mut FoundryWorld) {
    let (exit, stdout, stderr) = cli_result(world);
    assert_eq!(
        exit, 2,
        "an absent --name, --admin-email or --as is a usage error: exit 2 (D8, DDD-10); \
         stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.is_empty(),
        "a usage error prints nothing on stdout; got {stdout:?}"
    );
    assert!(
        stderr.contains("Usage: foundry doctor provision-workspace"),
        "an absent required flag still gets the usage line (DDD-10); got {stderr:?}"
    );
    assert!(
        !stderr.contains("Workspace name must"),
        "an absent required flag is a usage error, checked before the name rule \
         (D8, DDD-10); got {stderr:?}"
    );
}

#[then(regex = r"^the usage line says required flags are missing$")]
async fn usage_says_flags_missing(world: &mut FoundryWorld) {
    let (_, _, stderr) = cli_result(world);
    assert!(
        stderr.lines().any(|l| l == MISSING_FLAGS_USAGE),
        "stderr must carry exactly the line {MISSING_FLAGS_USAGE:?}; got {stderr:?}"
    );
}

// ===========================================================================
// Then — the parity matrix (KPI-2)
// ===========================================================================

#[then(regex = r#"^every door refuses it saying "([^"]+)"$"#)]
async fn every_door_refuses(world: &mut FoundryWorld, copy: String) {
    let verdicts = world.iwnr_verdicts.clone();
    assert!(!verdicts.is_empty(), "the name was offered at the doors");
    let wrong: Vec<_> = verdicts
        .iter()
        .filter(|(_, v)| *v != Verdict::Refused(copy.clone()))
        .collect();
    assert!(
        wrong.is_empty(),
        "every door must refuse with exactly {copy:?} (KPI-2, D3); all verdicts = {verdicts:#?}"
    );
}

#[then(regex = r#"^every door accepts it and stores "([^"]+)"$"#)]
async fn every_door_accepts(world: &mut FoundryWorld, stored: String) {
    let stored = decode_invisibles(&stored);
    let verdicts = world.iwnr_verdicts.clone();
    assert_eq!(verdicts.len(), 4, "the name was offered at all four doors");
    let wrong: Vec<_> = verdicts
        .iter()
        .filter(|(_, v)| *v != Verdict::Accepted(stored.clone()))
        .collect();
    assert!(
        wrong.is_empty(),
        "every door must accept and store exactly {stored:?} (KPI-2, D1); all verdicts = {verdicts:#?}"
    );
}

#[cfg(test)]
mod decode_tests {
    use super::decode_invisibles;

    #[test]
    fn marks_become_the_characters_they_name() {
        assert_eq!(decode_invisibles("House[TAB]hold"), "House\thold");
        assert_eq!(decode_invisibles("[TAB]Kitchen[NEWLINE]"), "\tKitchen\n");
        assert_eq!(decode_invisibles("Bailey[NUL]Family"), "Bailey\0Family");
        assert_eq!(decode_invisibles("[SPACE][SPACE]"), "  ");
        assert_eq!(decode_invisibles("Ops[U+202E]x"), "Ops\u{202E}x");
        assert_eq!(
            decode_invisibles("a[ZWJ]b[ZWNJ]c[NBSP]d"),
            "a\u{200D}b\u{200C}c\u{A0}d"
        );
    }

    #[test]
    fn other_bracketed_text_is_literal() {
        assert_eq!(decode_invisibles("<b>[draft]</b>"), "<b>[draft]</b>");
        assert_eq!(decode_invisibles("a [U+ZZZZ] b ["), "a [U+ZZZZ] b [");
        assert_eq!(decode_invisibles("Canzan Labs"), "Canzan Labs");
    }
}
