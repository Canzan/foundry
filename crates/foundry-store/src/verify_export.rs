//! per-workspace-backup (US-PWB-02, AC-02.2 / NFR-PWB-INT-01) — the OFFLINE
//! verifier for an exported workspace archive.
//!
//! This is the COMPANION to [`crate::Store::export_workspace`]: where the export
//! applies the §5 scope predicate as ten scoped SQL `WHERE` clauses against the
//! live DB, this verifier RE-APPLIES the SAME predicate offline, against the
//! whole-row JSONL the archive captured — using ONLY the archive's own bytes (the
//! declared workspace id from the manifest header + the per-table rows), with NO
//! out-of-band workspace argument and NO database access. That path-only property
//! is NFR-PWB-INT-01.
//!
//! Two checks (architecture.md §4 verify pipeline):
//!
//! - **Completeness** — all ten [`crate::TENANT_TABLES`] are present in the archive
//!   AND, per table, the JSONL line count equals the manifest `row_counts` entry.
//!   A missing table or a short/long count reds (the exit-4 truncation tripwire).
//! - **Isolation** — every archived row resolves to the declared workspace and no
//!   row resolves to a sibling. The predicate is re-applied per table exactly as
//!   §5 defines it:
//!     - `workspaces`: row `id` == declared W (exactly one row).
//!     - direct-`workspace_id` tables (`workspace_memberships`, `teams`,
//!       `projects`, `issues`, `invites`, `comments`, `machine_tokens`): row
//!       `workspace_id` == declared W.
//!     - `team_memberships` (transitive): row `team_id` resolves to a `teams` row
//!       present in the archive (which is itself scoped to W).
//!     - `users` (membership-bounded, ADR-001): row `id` appears as a `user_id` in
//!       the archived `workspace_memberships` — i.e. the user is a member of W. A
//!       user who is ALSO a member of a sibling is NOT a violation.
//!     - `comments` FK cross-check (DRIFT-2): row `issue_id` resolves to an
//!       `issues` row present in the archive (referential closure).
//!
//! The driving port is the pure function [`verify_workspace_export`]: it takes the
//! parsed archive (declared id + per-table parsed rows + declared row counts) and
//! returns a [`VerifyReport`]. The CLI adapter (`admin_cli::run_verify_export`)
//! owns the tar reading + JSON parsing and feeds this function; keeping the
//! predicate here means selection and verification cannot diverge — they sit in
//! the same crate as `export_workspace`.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::TENANT_TABLES;

/// One archived tenant table: its name plus the parsed whole-row JSON objects the
/// archive carried (one per JSONL line). Mirrors the export's
/// `(table_name, rows)` shape, parsed.
#[derive(Debug, Clone)]
pub struct ArchivedTable {
    /// Tenant table name (must be one of [`TENANT_TABLES`]).
    pub name: String,
    /// The manifest `row_counts` entry the export DECLARED for this table.
    /// Verify compares it against `rows.len()` to catch truncation cheaply
    /// without trusting it blindly (isolation still reads every row).
    pub declared_count: usize,
    /// The whole-row JSON objects parsed from the table's JSONL.
    pub rows: Vec<serde_json::Value>,
}

/// The input to [`verify_workspace_export`]: the archive's self-describing
/// contents, already read off disk and parsed. Path-only (NFR-PWB-INT-01): the
/// declared workspace id comes from the manifest header, never from a caller arg.
#[derive(Debug, Clone)]
pub struct ArchiveContents {
    /// The manifest `declared_workspace_id` — the workspace the archive claims to
    /// hold. Isolation is checked against THIS, read from the archive itself.
    pub declared_workspace_id: Uuid,
    /// The archived tenant tables, ideally in [`TENANT_TABLES`] order (order is not
    /// relied upon — completeness checks presence by name).
    pub tables: Vec<ArchivedTable>,
}

/// The result of verifying an archive. `is_ok()` is true exactly when the archive
/// is both complete and isolation-clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// Completeness violations: a missing tenant table, or a per-table line count
    /// that disagrees with the manifest's declared count. Empty == complete.
    pub completeness_violations: Vec<String>,
    /// Isolation violations: an archived row that resolves to a workspace other
    /// than the declared one, or a row whose FK does not resolve within the
    /// archive (dangling reference — referential closure broken). Each entry NAMES
    /// the offending row (table + the resolving workspace / dangling FK), so the
    /// CLI can print which row leaked. Empty == isolation-clean.
    pub isolation_violations: Vec<String>,
    /// Count of `team_memberships` rows that were resolved to their owning
    /// workspace THROUGH their team (the transitive `team_id -> teams.workspace_id`
    /// chain — §5 row 4). team_memberships has no direct `workspace_id`, so this
    /// counts the rows the transitive resolver actually walked. Exposed so the CLI
    /// can report that the FK-chain check ran (AC-02.3), not just that it passed.
    pub team_memberships_resolved: usize,
    /// Count of `comments` rows whose `issue_id` was cross-checked against an
    /// archived issue's owning workspace (the DRIFT-2 `comment.issue_id ->
    /// issues.workspace_id` corruption cross-check — §5 row 8). Exposed so the CLI
    /// can report that the comment cross-check ran (AC-02.3).
    pub comments_cross_checked: usize,
}

impl VerifyReport {
    /// True when the archive is both complete and isolation-clean.
    pub fn is_ok(&self) -> bool {
        self.completeness_violations.is_empty() && self.isolation_violations.is_empty()
    }

    /// True when every one of the ten tenant tables is present with a matching
    /// count (completeness holds).
    pub fn is_complete(&self) -> bool {
        self.completeness_violations.is_empty()
    }

    /// True when no archived row resolves to a sibling workspace and every FK
    /// resolves within the archive (isolation holds).
    pub fn is_isolation_clean(&self) -> bool {
        self.isolation_violations.is_empty()
    }
}

/// Tables whose scope predicate is a direct `workspace_id == W` column check
/// (architecture.md §5). `workspaces`, `team_memberships`, and `users` are handled
/// specially below.
const DIRECT_WORKSPACE_ID_TABLES: [&str; 7] = [
    "workspace_memberships",
    "teams",
    "projects",
    "issues",
    "invites",
    "comments",
    "machine_tokens",
];

/// Re-apply the §5 scope predicate offline to an archive's whole-row JSON, with no
/// DB and no out-of-band workspace argument (NFR-PWB-INT-01). The driving port for
/// the verifier; pure — same input yields same [`VerifyReport`].
pub fn verify_workspace_export(archive: &ArchiveContents) -> VerifyReport {
    let declared = archive.declared_workspace_id;

    let completeness_violations = check_completeness(archive);
    let isolation = check_isolation(archive, declared);

    VerifyReport {
        completeness_violations,
        isolation_violations: isolation.violations,
        team_memberships_resolved: isolation.team_memberships_resolved,
        comments_cross_checked: isolation.comments_cross_checked,
    }
}

/// The outcome of the isolation pass: the per-row violations plus the counts of
/// transitively-resolved rows (so the CLI can report the FK-chain check ran).
struct IsolationOutcome {
    violations: Vec<String>,
    team_memberships_resolved: usize,
    comments_cross_checked: usize,
}

/// Completeness: every tenant table present AND per-table JSONL line count ==
/// declared manifest count.
fn check_completeness(archive: &ArchiveContents) -> Vec<String> {
    let mut violations = Vec::new();
    for table in TENANT_TABLES {
        let Some(archived) = archive.tables.iter().find(|t| t.name == table) else {
            violations.push(format!(
                "tenant table {table:?} is missing from the archive"
            ));
            continue;
        };
        if archived.rows.len() != archived.declared_count {
            violations.push(format!(
                "tenant table {table:?} is truncated or incomplete: manifest declares \
                 {declared} rows but the archive holds {actual}",
                declared = archived.declared_count,
                actual = archived.rows.len(),
            ));
        }
    }
    violations
}

/// Isolation: re-apply the §5 predicate to every archived row.
fn check_isolation(archive: &ArchiveContents, declared: Uuid) -> IsolationOutcome {
    let mut violations = Vec::new();
    // Count the rows the transitive resolvers actually walk (team_memberships via
    // team_id, comments via issue_id) so the CLI can report the FK-chain check ran.
    let mut team_memberships_resolved = 0usize;
    let mut comments_cross_checked = 0usize;

    // Build the in-archive resolution sets the transitive / membership / FK checks
    // need: the team ids, issue ids, and member user ids the archive carries.
    let archived_team_ids = collect_uuids(archive, "teams", "id");
    let archived_issue_ids = collect_uuids(archive, "issues", "id");
    let archived_member_user_ids = collect_uuids(archive, "workspace_memberships", "user_id");

    for archived in &archive.tables {
        match archived.name.as_str() {
            "workspaces" => {
                for row in &archived.rows {
                    if uuid_field(row, "id") != Some(declared) {
                        violations.push(name_violation("workspaces", row, "id", declared));
                    }
                }
            }
            "users" => {
                // Membership-bounded (ADR-001): each archived user must be a member
                // of W (appear as a user_id in the archived memberships). Belonging
                // ALSO to a sibling is NOT a violation.
                for row in &archived.rows {
                    let id = uuid_field(row, "id");
                    let is_member = id.is_some_and(|u| archived_member_user_ids.contains(&u));
                    if !is_member {
                        violations.push(format!(
                            "users row {id:?} is not a member of the declared workspace \
                             {declared} (membership-bounded isolation, ADR-001)"
                        ));
                    }
                }
            }
            "team_memberships" => {
                // Transitive: team_id must resolve to a team present in the archive
                // (which is itself scoped to W).
                for row in &archived.rows {
                    let team_id = uuid_field(row, "team_id");
                    let resolves = team_id.is_some_and(|t| archived_team_ids.contains(&t));
                    if resolves {
                        team_memberships_resolved += 1;
                    } else {
                        violations.push(format!(
                            "team_memberships row references team_id {team_id:?} which does \
                             not resolve to any team in the archive (dangling transitive \
                             reference; isolation broken)"
                        ));
                    }
                }
            }
            name if DIRECT_WORKSPACE_ID_TABLES.contains(&name) => {
                for row in &archived.rows {
                    if uuid_field(row, "workspace_id") != Some(declared) {
                        violations.push(name_violation(name, row, "workspace_id", declared));
                    }
                    // comments FK cross-check (DRIFT-2): issue_id must resolve to an
                    // archived issue — referential closure for the transitive chain.
                    if name == "comments" {
                        let issue_id = uuid_field(row, "issue_id");
                        let resolves = issue_id.is_some_and(|i| archived_issue_ids.contains(&i));
                        if resolves {
                            comments_cross_checked += 1;
                        } else {
                            violations.push(format!(
                                "comments row references issue_id {issue_id:?} which does not \
                                 resolve to any issue in the archive (dangling FK; isolation \
                                 cross-check failed)"
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    IsolationOutcome {
        violations,
        team_memberships_resolved,
        comments_cross_checked,
    }
}

/// Collect the set of UUIDs at `field` across every row of `table` in the archive.
fn collect_uuids(archive: &ArchiveContents, table: &str, field: &str) -> BTreeSet<Uuid> {
    archive
        .tables
        .iter()
        .find(|t| t.name == table)
        .map(|t| t.rows.iter().filter_map(|r| uuid_field(r, field)).collect())
        .unwrap_or_default()
}

/// Parse a UUID-valued JSON field, returning `None` if absent or not a UUID.
fn uuid_field(row: &serde_json::Value, field: &str) -> Option<Uuid> {
    row.get(field)
        .and_then(serde_json::Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Build an isolation-violation message that NAMES the offending row and the
/// workspace it actually resolves to (vs the declared one).
fn name_violation(table: &str, row: &serde_json::Value, field: &str, declared: Uuid) -> String {
    let resolved = uuid_field(row, field);
    format!(
        "{table} row resolves to workspace {resolved:?} via {field}, which is not the \
         declared workspace {declared} (sibling-workspace row found)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a JSON row object from `(field, value)` string pairs.
    fn row(fields: &[(&str, &str)]) -> serde_json::Value {
        let map: serde_json::Map<String, serde_json::Value> = fields
            .iter()
            .map(|(k, v)| ((*k).to_string(), serde_json::Value::from(*v)))
            .collect();
        serde_json::Value::Object(map)
    }

    /// A complete, isolation-clean two-table-populated archive for workspace W:
    /// every tenant table present, counts honest, every row scoped to W, the
    /// transitive `team_memberships`/`users`/`comments` references closing inside
    /// the archive. The verifier must report it OK.
    fn clean_archive(w: Uuid) -> ArchiveContents {
        let team = Uuid::now_v7();
        let issue = Uuid::now_v7();
        let user = Uuid::now_v7();
        let mut tables: Vec<ArchivedTable> = Vec::new();
        for table in TENANT_TABLES {
            let rows = match table {
                "workspaces" => vec![row(&[("id", &w.to_string())])],
                "users" => vec![row(&[("id", &user.to_string())])],
                "workspace_memberships" => vec![row(&[
                    ("workspace_id", &w.to_string()),
                    ("user_id", &user.to_string()),
                ])],
                "teams" => vec![row(&[
                    ("id", &team.to_string()),
                    ("workspace_id", &w.to_string()),
                ])],
                "team_memberships" => vec![row(&[("team_id", &team.to_string())])],
                "issues" => vec![row(&[
                    ("id", &issue.to_string()),
                    ("workspace_id", &w.to_string()),
                ])],
                "comments" => vec![row(&[
                    ("workspace_id", &w.to_string()),
                    ("issue_id", &issue.to_string()),
                ])],
                _ => vec![row(&[("workspace_id", &w.to_string())])],
            };
            tables.push(ArchivedTable {
                name: table.to_string(),
                declared_count: rows.len(),
                rows,
            });
        }
        ArchiveContents {
            declared_workspace_id: w,
            tables,
        }
    }

    #[test]
    fn clean_archive_is_complete_and_isolation_clean() {
        let w = Uuid::now_v7();
        let report = verify_workspace_export(&clean_archive(w));
        assert!(
            report.is_ok(),
            "a clean archive must pass both checks; report={report:?}"
        );
        assert!(report.is_complete());
        assert!(report.is_isolation_clean());
    }

    #[test]
    fn planted_sibling_row_reds_isolation_and_names_it() {
        // The falsifiability crux (AC-02.4): plant one Acme (sibling) row into the
        // Globex archive's `issues` table. Isolation must red and name the row.
        let globex = Uuid::now_v7();
        let acme = Uuid::now_v7();
        let mut archive = clean_archive(globex);
        let issues = archive
            .tables
            .iter_mut()
            .find(|t| t.name == "issues")
            .expect("issues table");
        issues.rows.push(row(&[
            ("id", &Uuid::now_v7().to_string()),
            ("workspace_id", &acme.to_string()),
        ]));
        issues.declared_count = issues.rows.len();

        let report = verify_workspace_export(&archive);
        assert!(
            !report.is_isolation_clean(),
            "a planted sibling row must red isolation"
        );
        assert!(
            report
                .isolation_violations
                .iter()
                .any(|v| v.contains(&acme.to_string()) && v.contains("issues")),
            "the isolation violation must NAME the foreign issues row resolving to \
             the sibling workspace {acme}; got {:?}",
            report.isolation_violations
        );
    }

    #[test]
    fn truncated_table_reds_completeness() {
        // The exit-4 truncation tripwire (AC-03.5): the manifest declares more rows
        // than the JSONL actually carries. Completeness must red.
        let w = Uuid::now_v7();
        let mut archive = clean_archive(w);
        let comments = archive
            .tables
            .iter_mut()
            .find(|t| t.name == "comments")
            .expect("comments table");
        comments.declared_count = comments.rows.len() + 5; // claims 5 more than present

        let report = verify_workspace_export(&archive);
        assert!(
            !report.is_complete(),
            "a count mismatch must red completeness"
        );
        assert!(
            report
                .completeness_violations
                .iter()
                .any(|v| v.contains("comments") && v.contains("truncated")),
            "the completeness violation must name the short comments table; got {:?}",
            report.completeness_violations
        );
    }

    #[test]
    fn missing_table_reds_completeness() {
        let w = Uuid::now_v7();
        let mut archive = clean_archive(w);
        archive.tables.retain(|t| t.name != "machine_tokens");

        let report = verify_workspace_export(&archive);
        assert!(!report.is_complete());
        assert!(
            report
                .completeness_violations
                .iter()
                .any(|v| v.contains("machine_tokens") && v.contains("missing")),
            "removing a tenant table must red completeness naming it; got {:?}",
            report.completeness_violations
        );
    }

    #[test]
    fn multi_membership_user_is_not_flagged_as_a_leak() {
        // OD-PWB-1 / ADR-001: a user who belongs to BOTH Globex and Acme is included
        // in the Globex archive and must NOT be flagged. The verifier sees only that
        // the user is a member of the declared workspace (appears in the archived
        // memberships) and accepts it — it has no sibling-membership column to trip on.
        let globex = Uuid::now_v7();
        let report = verify_workspace_export(&clean_archive(globex));
        assert!(
            report.is_isolation_clean(),
            "a legitimately-included member user must not red isolation; report={report:?}"
        );
    }

    #[test]
    fn transitive_rows_are_resolved_through_the_fk_chain_and_counted() {
        // AC-02.3: the isolation pass walks the FK chains — team_memberships through
        // team_id -> teams (which is scoped to W), and comments through issue_id ->
        // issues (the DRIFT-2 cross-check) — and REPORTS how many transitively-scoped
        // rows it resolved, so the operator sees the chain check actually ran (not
        // merely that nothing failed). Two extra team_memberships + comments on the
        // same archived team/issue must all resolve and be counted.
        let w = Uuid::now_v7();
        let mut archive = clean_archive(w);
        let team_id = uuid_field(
            &archive
                .tables
                .iter()
                .find(|t| t.name == "teams")
                .unwrap()
                .rows[0],
            "id",
        )
        .expect("archived team id");
        let issue_id = uuid_field(
            &archive
                .tables
                .iter()
                .find(|t| t.name == "issues")
                .unwrap()
                .rows[0],
            "id",
        )
        .expect("archived issue id");
        for t in &mut archive.tables {
            if t.name == "team_memberships" {
                t.rows.push(row(&[("team_id", &team_id.to_string())]));
                t.declared_count = t.rows.len();
            }
            if t.name == "comments" {
                t.rows.push(row(&[
                    ("workspace_id", &w.to_string()),
                    ("issue_id", &issue_id.to_string()),
                ]));
                t.declared_count = t.rows.len();
            }
        }

        let report = verify_workspace_export(&archive);
        assert!(report.is_isolation_clean(), "report={report:?}");
        assert_eq!(
            report.team_memberships_resolved, 2,
            "both team_memberships rows must resolve to their owning workspace through \
             their team_id; report={report:?}"
        );
        assert_eq!(
            report.comments_cross_checked, 2,
            "both comments must be cross-checked against their issue's owning workspace; \
             report={report:?}"
        );
    }
}
