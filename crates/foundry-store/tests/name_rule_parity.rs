//! name-db-checks KPI-2 / DoD 2 (DESIGN DDD-1/2/3/16, ADR-NAME-DB-001): the
//! database's verdict on a name is the app's, arm for arm. For every string `s`
//! (U+0000 excluded: `text` cannot hold it) and each table's cap,
//! `foundry_name_rule_violation(s, cap)` is
//!
//! - NULL                when `try_new(s)` is `Ok(n)` and `n == s`,
//! - `trimmed`           when `try_new(s)` is `Ok(n)` and `n != s`,
//! - `not_empty`         when it is `Err(Empty)`,
//! - `no_control_chars`  when it is `Err(ControlCharacter)`,
//! - `max_<cap>_chars`   when it is `Err(TooLong)`,
//!
//! with `WorkspaceName` at 24 and `ProjectName` at 256 (each table's own type).
//! The database is therefore never stricter than the app (an app-accepted name
//! would 500) and never looser (the rule would have a hole), D3.
//!
//! The verdict function is IMMUTABLE and pure, so parity is checked in bulk
//! (`unnest … WITH ORDINALITY`) without INSERTs; `name_rule_in_database.rs` proves
//! the triggers wire it to every name write.
//!
//! Every database test starts with [`require_rule_installed`], the RED gate that
//! fails with a readable message if migration 0019 has not installed the verdict
//! function.
//! The workspace half (cap 24) was un-pended in slice 01, the project half
//! (cap 256, the cap migration 0020's triggers pass) in slice 02.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/name_rule_parity.rs
//!   CLOSEST-EXISTING: crates/foundry-core/tests/workspace_name.rs
//!   EXTENSION-COST: that suite is pure Rust; this one needs a real Postgres to
//!     evaluate the SQL copy of the rule.
//!   PARALLEL-RATIONALE: "the rule is right" (core) and "the SQL copy equals the
//!     rule" (here) are different properties with different substrates.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`, UTF8): `btrim`, the regex
//! engine and `char_length` cannot be faked. Layer 2 by cost (one container per
//! test, no I/O per name), so the generative half is PBT (Mandate 9); the exact
//! boundary pairs pin the traps a generator may under-sample.

use foundry_core::{
    ProjectName, ProjectNameError, WorkspaceName, WorkspaceNameError, PROJECT_NAME_MAX_CHARS,
    WORKSPACE_NAME_MAX_CHARS,
};
use foundry_store::run_migrations;
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// Generated names per table per run (KPI-2: at least 10,000).
const GENERATED_PER_TABLE: usize = 10_000;
/// Names sampled to check the generator reaches every arm (1% floor each).
const REACH_SAMPLE: usize = 2_000;
/// Names judged per round trip.
const BATCH: usize = 1_000;

/// Rust `str::trim`'s set: Unicode White_Space, all 25 code points (D2, DDD-3).
const WHITE_SPACE: [char; 25] = [
    '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}', '\u{00A0}',
    '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
    '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}',
    '\u{3000}',
];

/// Format and invisible characters the app allows (DDD-16): ZWSP, ZWNJ, ZWJ,
/// LRM, RLM, soft hyphen, Mongolian vowel separator, BOM.
const ALLOWED_FORMAT: [char; 8] = [
    '\u{200B}', '\u{200C}', '\u{200D}', '\u{200E}', '\u{200F}', '\u{00AD}', '\u{180E}', '\u{FEFF}',
];

/// Letters of every UTF-8 width, astral included (a code point counts once).
const LETTERS: [char; 8] = ['a', 'Z', '7', 'é', 'Ω', '日', '😀', '𝒳'];

/// The two tables, each with its cap and its own Rust rule.
#[derive(Debug, Clone, Copy)]
enum Table {
    Workspaces,
    Projects,
}

impl Table {
    fn cap(self) -> usize {
        match self {
            Self::Workspaces => WORKSPACE_NAME_MAX_CHARS,
            Self::Projects => PROJECT_NAME_MAX_CHARS,
        }
    }

    /// The app's verdict, mapped to the database's arm suffix (DDD-1/2).
    fn app_verdict(self, s: &str) -> Option<String> {
        let cap = format!("max_{}_chars", self.cap());
        let (accepted, arm) = match self {
            Self::Workspaces => match WorkspaceName::try_new(s) {
                Ok(n) => (Some(n.as_str().to_string()), None),
                Err(WorkspaceNameError::Empty) => (None, Some("not_empty".to_string())),
                Err(WorkspaceNameError::ControlCharacter) => {
                    (None, Some("no_control_chars".to_string()))
                }
                Err(WorkspaceNameError::TooLong) => (None, Some(cap)),
            },
            Self::Projects => match ProjectName::try_new(s) {
                Ok(n) => (Some(n.as_str().to_string()), None),
                Err(ProjectNameError::Empty) => (None, Some("not_empty".to_string())),
                Err(ProjectNameError::ControlCharacter) => {
                    (None, Some("no_control_chars".to_string()))
                }
                Err(ProjectNameError::TooLong) => (None, Some(cap)),
                Err(ProjectNameError::NotUnique) => {
                    unreachable!("try_new never answers NotUnique")
                }
            },
        };
        match (accepted, arm) {
            (Some(stored), _) if stored == s => None,
            (Some(_), _) => Some("trimmed".to_string()),
            (None, arm) => arm,
        }
    }
}

/// Every code point of a name, escaped, so a failure is readable.
fn escaped(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_graphic() {
                c.to_string()
            } else {
                format!("\\u{{{:04X}}}", c as u32)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

async fn migrated() -> (PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("apply every migration");
    (pool, container)
}

/// RED gate: the verdict function exists. Until migration 0019 lands this fails
/// with a readable RED-gate message, never with an undefined-function database error.
async fn require_rule_installed(pool: &PgPool) {
    let (present,): (bool,) = sqlx::query_as(
        "SELECT to_regprocedure('foundry_name_rule_violation(text, integer)') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .expect("look the verdict function up");
    assert!(
        present,
        "RED gate: migration 0019 has not installed foundry_name_rule_violation(text, integer) \
         yet (name-db-checks DDD-1/DDD-7)"
    );
}

/// The database's verdict for each name, in order.
async fn db_verdicts(pool: &PgPool, names: &[String], cap: usize) -> Vec<Option<String>> {
    let mut out = Vec::with_capacity(names.len());
    for chunk in names.chunks(BATCH) {
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT foundry_name_rule_violation(n, $2)
               FROM unnest($1::text[]) WITH ORDINALITY AS t(n, i)
              ORDER BY i",
        )
        .bind(chunk)
        .bind(i32::try_from(cap).expect("cap fits i32"))
        .fetch_all(pool)
        .await
        .expect("judge a batch of names");
        out.extend(rows.into_iter().map(|(v,)| v));
    }
    out
}

/// Every disagreement between the database and the app, readable.
async fn disagreements(pool: &PgPool, table: Table, names: &[String]) -> Vec<String> {
    let db = db_verdicts(pool, names, table.cap()).await;
    assert_eq!(db.len(), names.len(), "one verdict per name");
    names
        .iter()
        .zip(db)
        .filter_map(|(name, db)| {
            let app = table.app_verdict(name);
            (app != db).then(|| {
                format!(
                    "{table:?} {:?}: app {app:?}, database {db:?}",
                    escaped(name)
                )
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Generator (DDD-16 alphabet)
// ---------------------------------------------------------------------------

/// Every code point the app refuses (U+0000 excluded).
fn refused_char() -> impl Strategy<Value = char> {
    prop_oneof![
        (0x01u32..=0x1F).prop_map(|c| char::from_u32(c).expect("Cc")),
        (0x7Fu32..=0x9F).prop_map(|c| char::from_u32(c).expect("Cc")),
        (0x2028u32..=0x202E).prop_map(|c| char::from_u32(c).expect("separator or bidi")),
        (0x2066u32..=0x2069).prop_map(|c| char::from_u32(c).expect("bidi isolate")),
    ]
}

/// The neighbours of every refused range, all allowed.
fn boundary_neighbour() -> impl Strategy<Value = char> {
    prop::sample::select(vec![
        '\u{0020}', '\u{007E}', '\u{00A0}', '\u{2027}', '\u{202F}', '\u{2065}', '\u{206A}',
    ])
}

fn white_space() -> impl Strategy<Value = char> {
    prop::sample::select(WHITE_SPACE.to_vec())
}

fn letter() -> impl Strategy<Value = char> {
    prop::sample::select(LETTERS.to_vec())
}

fn allowed_format() -> impl Strategy<Value = char> {
    prop::sample::select(ALLOWED_FORMAT.to_vec())
}

/// Any character the generator offers.
fn any_char() -> impl Strategy<Value = char> {
    prop_oneof![
        12 => letter(),
        2 => white_space(),
        2 => allowed_format(),
        2 => boundary_neighbour(),
        1 => refused_char(),
    ]
}

/// A character the app keeps inside a name (no refused point).
fn clean_char() -> impl Strategy<Value = char> {
    prop_oneof![
        12 => letter(),
        2 => allowed_format(),
        2 => boundary_neighbour(),
        1 => white_space().prop_filter("interior White_Space the app keeps", |c| {
            !c.is_control() && !matches!(c, '\u{2028}' | '\u{2029}')
        }),
    ]
}

/// Edge padding (0-2 White_Space points, sometimes a refused or format point),
/// a core of 0-6 characters or of cap−2 ..= cap+2, edge padding again.
fn name_strategy(cap: usize) -> impl Strategy<Value = String> {
    let edge = prop::collection::vec(
        prop_oneof![6 => white_space(), 1 => allowed_format(), 1 => refused_char()],
        0..3,
    );
    let len = prop_oneof![0usize..=6, (cap - 2)..=(cap + 2)];
    let core = len.prop_flat_map(|n| {
        prop_oneof![
            prop::collection::vec(clean_char(), n),
            prop::collection::vec(any_char(), n),
        ]
    });
    (edge.clone(), core, edge)
        .prop_map(|(pre, core, post)| pre.into_iter().chain(core).chain(post).collect::<String>())
}

fn generated_names(cap: usize, count: usize) -> Vec<String> {
    let mut runner = TestRunner::default();
    let strategy = name_strategy(cap);
    (0..count)
        .map(|_| {
            strategy
                .new_tree(&mut runner)
                .expect("generate a name")
                .current()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bulk parity (KPI-2)
// ---------------------------------------------------------------------------

async fn bulk_parity(table: Table) {
    let (pool, _container) = migrated().await;
    require_rule_installed(&pool).await;
    let names = generated_names(table.cap(), GENERATED_PER_TABLE);
    assert!(
        names.len() >= 10_000,
        "KPI-2: at least 10,000 names per table"
    );
    let wrong = disagreements(&pool, table, &names).await;
    assert!(
        wrong.is_empty(),
        "{} of {} generated names judged differently (D3); first ones:\n{}",
        wrong.len(),
        names.len(),
        wrong
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[tokio::test]
async fn workspace_verdicts_match_workspace_name_arm_for_arm_over_10k_generated_names() {
    bulk_parity(Table::Workspaces).await;
}

#[tokio::test]
async fn project_verdicts_match_project_name_arm_for_arm_over_10k_generated_names() {
    bulk_parity(Table::Projects).await;
}

// ---------------------------------------------------------------------------
// Exact boundary pairs (DoD 2) — pinned literally AND against the app
// ---------------------------------------------------------------------------

/// Check each `(name, expected arm)` at `table`: the literal expectation, the
/// app's verdict and the database's verdict must all agree.
async fn assert_exact(pool: &PgPool, table: Table, cases: &[(String, Option<&str>)]) {
    let names: Vec<String> = cases.iter().map(|(n, _)| n.clone()).collect();
    let db = db_verdicts(pool, &names, table.cap()).await;
    let mut wrong = Vec::new();
    for ((name, expected), db) in cases.iter().zip(db) {
        let expected = expected.map(str::to_string);
        let app = table.app_verdict(name);
        if app != expected || db != expected {
            wrong.push(format!(
                "{table:?} {:?}: expected {expected:?}, app {app:?}, database {db:?}",
                escaped(name)
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "exact pairs disagree:\n{}",
        wrong.join("\n")
    );
}

fn rep(c: char, n: usize) -> String {
    std::iter::repeat_n(c, n).collect()
}

/// Every White_Space point at either edge is `trimmed`; in the interior it is
/// kept, unless the app also refuses it there (Cc, U+2028, U+2029).
fn white_space_cases() -> Vec<(String, Option<&'static str>)> {
    let mut cases = Vec::new();
    for c in WHITE_SPACE {
        let refused_inside = c.is_control() || matches!(c, '\u{2028}' | '\u{2029}');
        cases.push((format!("{c}Globex"), Some("trimmed")));
        cases.push((format!("Globex{c}"), Some("trimmed")));
        cases.push((
            format!("Glo{c}bex"),
            refused_inside.then_some("no_control_chars"),
        ));
        cases.push((rep(c, 3), Some("not_empty")));
    }
    cases
}

/// Each refused range at its bounds, inside a name, beside its allowed neighbour.
fn refused_boundary_cases() -> Vec<(String, Option<&'static str>)> {
    let pairs: [(char, bool); 14] = [
        ('\u{0001}', true),
        ('\u{001F}', true),
        ('\u{0020}', false),
        ('\u{007E}', false),
        ('\u{007F}', true),
        ('\u{009F}', true),
        ('\u{00A0}', false),
        ('\u{2027}', false),
        ('\u{2028}', true),
        ('\u{202E}', true),
        ('\u{202F}', false),
        ('\u{2065}', false),
        ('\u{2066}', true),
        ('\u{206A}', false),
    ];
    let mut cases: Vec<(String, Option<&'static str>)> = pairs
        .iter()
        .map(|(c, refused)| (format!("Ab{c}cd"), refused.then_some("no_control_chars")))
        .collect();
    cases.push(("Ab\u{2069}cd".to_string(), Some("no_control_chars")));
    cases.push(("Ab\u{202A}cd".to_string(), Some("no_control_chars")));
    cases.push(("Ab\u{2029}cd".to_string(), Some("no_control_chars")));
    cases
}

/// The app allows these anywhere, edges included (none is White_Space).
fn allowed_format_cases() -> Vec<(String, Option<&'static str>)> {
    ALLOWED_FORMAT
        .iter()
        .flat_map(|c| {
            [
                (format!("{c}Globex"), None),
                (format!("Glo{c}bex"), None),
                (format!("Globex{c}"), None),
            ]
        })
        .collect()
}

/// The app's order: trim, then empty, then refused, then length; `trimmed` last.
fn precedence_cases(cap: usize) -> Vec<(String, Option<&'static str>)> {
    let long = rep('x', cap + 1);
    vec![
        (String::new(), Some("not_empty")),
        (" \t ".to_string(), Some("not_empty")),
        (" Glo\tbex".to_string(), Some("no_control_chars")),
        (format!("{long}\t{long}"), Some("no_control_chars")),
        (format!(" {} ", rep('x', cap)), Some("trimmed")),
        (format!("\u{2028}{}", rep('x', cap)), Some("trimmed")),
    ]
}

/// Lengths cap−1, cap, cap+1 of one-, two-, three- and four-byte characters,
/// padded or not.
fn length_cases(cap: usize, arm: &'static str) -> Vec<(String, Option<&'static str>)> {
    let mut cases = Vec::new();
    for c in ['x', 'é', '日', '😀'] {
        cases.push((rep(c, cap - 1), None));
        cases.push((rep(c, cap), None));
        cases.push((rep(c, cap + 1), Some(arm)));
        cases.push((format!(" {} ", rep(c, cap + 1)), Some(arm)));
    }
    cases
}

/// The DoD-2 example table for one table, with its literal expected arms.
fn exact_cases(table: Table) -> Vec<(String, Option<&'static str>)> {
    let arm: &'static str = match table {
        Table::Workspaces => "max_24_chars",
        Table::Projects => "max_256_chars",
    };
    let mut cases = white_space_cases();
    cases.extend(refused_boundary_cases());
    cases.extend(allowed_format_cases());
    cases.extend(precedence_cases(table.cap()));
    cases.extend(length_cases(table.cap(), arm));
    cases.push(("👨\u{200D}👩\u{200D}👧 Bailey".to_string(), None));
    cases.push(("Ångström Øresund Société".to_string(), None));
    cases
}

async fn exact_pairs(table: Table) {
    let (pool, _container) = migrated().await;
    require_rule_installed(&pool).await;
    assert_exact(&pool, table, &exact_cases(table)).await;
}

#[tokio::test]
async fn workspace_boundary_code_points_and_lengths_match_exactly() {
    exact_pairs(Table::Workspaces).await;
}

#[tokio::test]
async fn project_boundary_code_points_and_lengths_match_exactly() {
    exact_pairs(Table::Projects).await;
}

/// DDD-1: a NULL name has no verdict (`NOT NULL` owns NULLs), and the function
/// is IMMUTABLE, so it is a pure function of its two arguments.
#[tokio::test]
async fn the_verdict_function_is_pure_and_leaves_null_to_not_null() {
    let (pool, _container) = migrated().await;
    require_rule_installed(&pool).await;
    let (null_verdict, volatility): (Option<String>, String) = sqlx::query_as(
        "SELECT foundry_name_rule_violation(NULL, 24),
                (SELECT provolatile::text FROM pg_proc
                  WHERE oid = 'foundry_name_rule_violation(text, integer)'::regprocedure)",
    )
    .fetch_one(&pool)
    .await
    .expect("judge NULL and read the volatility");
    assert_eq!(null_verdict, None, "a NULL name has no verdict (DDD-1)");
    assert_eq!(volatility, "i", "the verdict function is IMMUTABLE (DDD-1)");
}

// ---------------------------------------------------------------------------
// The oracle and the generator, checked against the app rule alone (no
// database, not ignored): a wrong literal or a generator that never reaches an
// arm would make the database tests above prove less than they claim.
// ---------------------------------------------------------------------------

#[test]
fn the_literal_expectations_are_the_app_rules_own_verdicts() {
    for table in [Table::Workspaces, Table::Projects] {
        let wrong: Vec<String> = exact_cases(table)
            .into_iter()
            .filter_map(|(name, expected)| {
                let app = table.app_verdict(&name);
                (app.as_deref() != expected).then(|| {
                    format!(
                        "{table:?} {:?}: literal {expected:?}, app {app:?}",
                        escaped(&name)
                    )
                })
            })
            .collect();
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}

#[test]
fn the_generator_reaches_every_arm_of_both_rules() {
    for table in [Table::Workspaces, Table::Projects] {
        let names = generated_names(table.cap(), REACH_SAMPLE);
        let mut seen: std::collections::BTreeMap<String, usize> = Default::default();
        for name in &names {
            assert!(!name.contains('\0'), "U+0000 is never generated");
            let verdict = table
                .app_verdict(name)
                .unwrap_or_else(|| "accepted".to_string());
            *seen.entry(verdict).or_default() += 1;
        }
        for arm in [
            "accepted".to_string(),
            "trimmed".to_string(),
            "not_empty".to_string(),
            "no_control_chars".to_string(),
            format!("max_{}_chars", table.cap()),
        ] {
            let n = seen.get(&arm).copied().unwrap_or(0);
            assert!(
                n >= REACH_SAMPLE / 100,
                "{table:?}: the generator reached {arm} only {n} times in {REACH_SAMPLE} ({seen:?})"
            );
        }
    }
}
