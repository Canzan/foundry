//! `cargo xtask check-arch` — the US-W06 web/api boundary guard.
//!
//! Two of the three orthogonal layers from `boundary-guard.md` live here; the
//! third (the injected-violation gold test) lives in the acceptance suite,
//! which drives THIS binary against a planted-violation tree copy:
//!
//!   LAYER 1 — AST / source walk (this module). Walks `crates/foundry-api/src`
//!     and `crates/foundry-auth/src` and asserts:
//!       * api≠HTML       — no `foundry-api` source constructs `Html(..)`,
//!                          returns `Html<..>`, or sets a `text/html`
//!                          content-type (api-contract.md allows an HTML string
//!                          INSIDE a JSON field, so only response-body / header
//!                          construction is flagged).
//!       * api≠ad-hoc-authz — no `is_team_member` / `is_workspace_admin` call
//!                          site appears in `foundry-api` (authz lives in
//!                          foundry-services, NFR-WEB-API-SEC-02).
//!       * api≠mint        — no `foundry-api` source names `mint_token` and no
//!                          `post(` is registered on the `.../tokens` collection
//!                          route. Minting stays confined to the /admin/tokens
//!                          human-session path (foundry-app); the bearer surface
//!                          exposes no programmatic mint (no-mint-boundary.md
//!                          DD-TMA-04). Doc-comment mentions of `mint_token` are
//!                          NOT flagged (strip_comment).
//!       * JWT alg pin    — the machine-token `Validation` pins
//!                          `algorithms = [EdDSA]` and never disables signature
//!                          validation (closes the alg-confusion / `alg:none`
//!                          footgun structurally).
//!       * api-openapi-operations — the /api/v1 routes in foundry-api, their
//!                          path parameters, request bodies and success answers
//!                          equal the operations `crates/foundry-api/openapi.json`
//!                          describes, failing closed on a route it cannot read;
//!                          every operation is described, none pending
//!                          (api-openapi-document ADR-API-OPENAPI-001, DDD-14).
//!       * api-openapi-statuses — each described operation's statuses, body
//!                          kinds and error codes, derived from the code by
//!                          lexical reachability (handler and extractor →
//!                          use-cases → `ServiceError` tokens → `status_for`'s
//!                          arms), equal the description's; anything it cannot
//!                          read fails closed (ADR-API-OPENAPI-002).
//!       * api-docs-page  — the API page stays at /docs/api, only its template
//!                          loads swagger-ui, its initializer pins every D10
//!                          option and reaches nowhere else, its markup has no
//!                          inline code, its script-src is 'self' alone and the
//!                          vendored stylesheet loads nothing
//!                          (ADR-API-OPENAPI-003/004).
//!     On a violation it NAMES the offending file + line and exits non-zero.
//!
//!   LAYER 2 — `cargo-deny` crate-graph dependency-direction (delegated). Runs
//!     `cargo deny check bans` against the target tree's `Cargo.toml`; the
//!     `[[bans.deny]]` entries in `deny.toml` forbid `foundry-api ->
//!     foundry-store` and the reversed `foundry-services -> foundry-api` edge.
//!     cargo-deny NAMES the forbidden crate on a violation.
//!
//! `check-arch [--root <DIR>]` analyses `<DIR>` (default: the workspace root
//! inferred from this crate's `CARGO_MANIFEST_DIR`). The acceptance gold test
//! passes `--root <copy>` pointing at a throwaway tree with a planted
//! violation, proving the guard bites (Principle 12c self-application).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The guard's decision, decoupled from the process-exit encoding of it.
///
/// SEAM (2026-08-30). `run` used to fold argument parsing, the eleven LAYER 1
/// rules, the LAYER 2 delegation, the reporting and the exit code into one
/// body — and nothing drove it: every gold test calls a rule function directly.
/// `cargo mutants` found the hole, reporting `run -> ExitCode` replaced by
/// `Default::default()` as a survivor. `ExitCode::default()` IS
/// `ExitCode::SUCCESS`, so that mutant makes `cargo xtask check-arch` pass
/// unconditionally while all eleven rules still collect their violations into a
/// vec nobody acts on, with the whole suite green.
///
/// So the decision moved out to where a test can assert on it: `ExitCode`
/// implements neither `PartialEq` nor any accessor, but `Verdict` is a plain
/// value. `run` is now a thin shell — [`verdict`], report, `ExitCode::from`.
/// The extraction is not itself the new blind spot: [`source_violations`],
/// [`verdict_with`] and [`Verdict::exit_code`] each carry tests, and `run`
/// keeps its own falsifiability assertions.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Passed,
    Violations(Vec<String>),
    UnparseableArguments(String),
}

impl Verdict {
    /// The process exit code this verdict encodes, and the contract
    /// `cargo xtask ci` reads: 0 clean, 1 violations, 2 unusable arguments.
    fn exit_code(&self) -> u8 {
        match self {
            Verdict::Passed => 0,
            Verdict::Violations(_) => 1,
            Verdict::UnparseableArguments(_) => 2,
        }
    }
}

/// LAYER 1 — every AST / source rule, aggregated in the order `check-arch`
/// reports them. Hermetic: it reads the tree and starts no subprocess, so a
/// test can drive the whole rule set against a staged fixture.
fn source_violations(root: &Path) -> Vec<String> {
    let mut violations: Vec<String> = Vec::new();
    violations.extend(check_api_no_html(root));
    violations.extend(check_api_no_adhoc_authz(root));
    violations.extend(check_api_no_mint_route(root));
    violations.extend(check_jwt_alg_pin(root));
    violations.extend(check_oidc_alg_pin(root));
    violations.extend(check_app_tenant_scoping(root));
    violations.extend(check_app_no_slugify_definition(root));
    violations.extend(check_no_static_lane_list(root));
    violations.extend(check_lane_position_deferrable(root));
    violations.extend(check_board_modules_have_no_keydown_listener(root));
    violations.extend(check_provisioned_marker_is_never_rewritten(root));
    violations.extend(check_publish_workflows_stamp_the_image(root));
    violations.extend(check_static_asset_integrity(root));
    violations.extend(check_stylesheet_colour_seam(root));
    violations.extend(check_stylesheet_dark_block_parity(root));
    violations.extend(check_workspace_name_one_source(root));
    violations.extend(check_project_name_one_source(root));
    violations.extend(check_name_rule_legacy_seam(root));
    violations.extend(check_team_membership_one_seam(root));
    violations.extend(check_team_name_one_source(root));
    violations.extend(check_team_lifecycle_one_seam(root));
    violations.extend(check_project_lifecycle_one_seam(root));
    violations.extend(check_project_lifecycle_write_guard(root));
    violations.extend(check_project_lifecycle_live_reads(root));
    violations.extend(check_store_natural_key_scoped(root));
    violations.extend(check_api_openapi_operations(root));
    violations.extend(check_api_openapi_statuses(root));
    violations.extend(check_api_docs_page(root));
    violations
}

/// The guard's decision over both layers, as shipped.
fn verdict(args: &[String]) -> Verdict {
    verdict_with(args, check_dependency_direction)
}

/// The guard's decision with LAYER 2 supplied. `dependency_direction` shells
/// out to `cargo deny`, so it is the one driven adapter in this module and the
/// one place a test double belongs: a staged fixture tree has no `Cargo.toml`,
/// which means the `Passed` branch is unreachable in a test unless LAYER 2 can
/// be stubbed at that boundary.
fn verdict_with(args: &[String], dependency_direction: fn(&Path) -> Option<String>) -> Verdict {
    let root = match parse_root(args) {
        Ok(root) => root,
        Err(message) => return Verdict::UnparseableArguments(message),
    };

    let mut violations = source_violations(&root);
    violations.extend(dependency_direction(&root));

    if violations.is_empty() {
        return Verdict::Passed;
    }
    Verdict::Violations(violations)
}

/// Run the boundary guard. `args` is everything after `check-arch`.
///
/// A thin shell over [`verdict`]: decide, report, encode. Reporting stays
/// inlined here on purpose — hoisting it into its own `fn report(&Verdict)`
/// would create exactly the untested unit this extraction exists to remove,
/// since no test asserts on stdout.
pub fn run(args: Vec<String>) -> ExitCode {
    let verdict = verdict(&args);
    match &verdict {
        Verdict::Passed => println!(
            "check-arch: boundary guard PASSED (api≠HTML, api≠ad-hoc-authz, api≠mint, JWT alg pinned to [EdDSA] + OIDC to [RS256], tenant-scoping by resolved ActingWorkspace, single slugify in foundry-core, no static lane list in app/api, the lanes position constraint is still DEFERRABLE, no board-*.js registers a keydown listener, nothing outside migration 0017 UPDATEs users.provisioned_at (D9), both publish workflows stamp every image with its commit and the Dockerfile hands it to build.rs (AC-8), every /static reference resolves, every content-hashed filename is its own sha256 prefix, every VENDOR.md sha256 recomputes, no colour literal outside the three stylesheet token regions, the three stylesheet token regions declare the identical colour-token set, the workspace-name refusal copy lives only in foundry-core and no production door calls create_initial_workspace, the project-name refusal copy lives only in foundry-core and no door calls insert_project, no production door calls seed_row_predating_name_rule and no crate but its seam file disables a trigger or sets session_replication_role, the team-name refusal copy lives only in foundry-core, teams and team_lifecycle_events are written only by foundry-store and no door calls create_team_with_lead, rename_team_with_audit or delete_team_if_empty, projects, live_projects and project_lifecycle_events are written only by foundry-store and no door calls rename_project_with_audit, soft_delete_project or restore_deleted_project (project-lifecycle-one-seam), every foundry-store writer of issues, comments, attachments, lanes or change events holds lock_live_project on its transaction before its first write (project-lifecycle-write-guard), no crate but foundry-store's project_lifecycle module names the projects base table in SQL or as an exact literal outside the four reviewed exempt items (project-lifecycle-live-reads), every foundry-store query filtering on key_prefix, a slug or an issue number also compares workspace_id, team_id or project_id to a bound parameter in the same disjunct (store-natural-key-scoped), the code's /api/v1 routes, path parameters, request bodies and success answers equal crates/foundry-api/openapi.json, with no operation undescribed (api-openapi-operations), every described operation's statuses, body kinds and error codes, derived from the code by fail-closed reachability through status_for, equal the description's (api-openapi-statuses), /docs/api stays put, only its template loads swagger-ui, its initializer reaches only this Foundry, pins url, validatorUrl \"none\", persistAuthorization, queryConfigEnabled and withCredentials and names no other description, storage or eval, its template carries no inline script, handler or style, its script-src is 'self' alone and the vendored swagger-ui.css loads nothing (api-docs-page); the drift check's type-derived layer is the foundry-api tests wire_shapes_match_document, document_is_valid_openapi_3_1 and info_version_is_the_release, dependency direction)"
        ),
        Verdict::UnparseableArguments(message) => eprintln!("check-arch: {message}"),
        Verdict::Violations(violations) => {
            eprintln!(
                "check-arch: boundary guard FAILED — {} violation(s):",
                violations.len()
            );
            for violation in violations {
                eprintln!("  - {violation}");
            }
        }
    }
    ExitCode::from(verdict.exit_code())
}

/// Parse `[--root <DIR>]`, defaulting to the workspace root.
fn parse_root(args: &[String]) -> Result<PathBuf, String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--root" {
            let dir = iter
                .next()
                .ok_or_else(|| "--root requires a directory argument".to_string())?;
            return Ok(PathBuf::from(dir));
        }
    }
    Ok(workspace_root())
}

/// The workspace root: `xtask`'s `CARGO_MANIFEST_DIR` parent.
pub(crate) fn workspace_root() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// LAYER 1a — api≠HTML. No `foundry-api` source may construct an HTML response
/// body or set a `text/html` content-type. A JSON string field whose contents
/// happen to be markup (e.g. `body_html`) is explicitly allowed — the rule
/// targets response-body / content-type CONSTRUCTION, not string contents.
fn check_api_no_html(root: &Path) -> Vec<String> {
    let api_src = root.join("crates").join("foundry-api").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&api_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (line_no, line) in contents.lines().enumerate() {
            let code = strip_comment(line);
            // Response-body / content-type construction patterns.
            let constructs_html = code.contains("Html(")
                || code.contains("Html::")
                || code.contains("response::Html")
                || code.contains("Html<")
                || code.contains("text/html");
            if constructs_html {
                violations.push(format!(
                    "api≠HTML: {} constructs an HTML response at {}:{} (`{}`) — the data-API tier must emit JSON only (boundary-guard.md NFR-WEB-BND-01)",
                    handler_label(&file),
                    rel(root, &file),
                    line_no + 1,
                    code.trim(),
                ));
            }
        }
    }
    violations
}

/// LAYER 1b — api≠ad-hoc-authz. Authorization (`is_team_member` /
/// `is_workspace_admin`) belongs in foundry-services, never the adapter.
fn check_api_no_adhoc_authz(root: &Path) -> Vec<String> {
    let api_src = root.join("crates").join("foundry-api").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&api_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (line_no, line) in contents.lines().enumerate() {
            let code = strip_comment(line);
            for needle in ["is_team_member", "is_workspace_admin"] {
                if code.contains(&format!("{needle}(")) {
                    violations.push(format!(
                        "api≠ad-hoc-authz: {} performs `{needle}` at {}:{} — authorization belongs in foundry-services (NFR-WEB-API-SEC-02)",
                        handler_label(&file),
                        rel(root, &file),
                        line_no + 1,
                    ));
                }
            }
        }
    }
    violations
}

/// LAYER 1d — api≠mint. The bearer surface (`foundry-api`) must NEVER mint a
/// token: minting is confined to the `/admin/tokens` human-session path in
/// `foundry-app`, which calls `Services::mint_token` (DD4). The no-mint boundary
/// (no-mint-boundary.md / DD-TMA-04) is enforced structurally — there is no POST
/// on the `.../tokens` collection route — and this rule LOCKS that invariant so
/// a future edit cannot wire a bearer mint path green.
///
/// Two orthogonal detectors, both NAMING the offending file + line:
///   * load-bearing — any `foundry-api` source line that names `mint_token`
///     (a `Services::mint_token` / `services.mint_token(` call). `strip_comment`
///     means a doc-comment mention of `mint_token` (design prose) is NOT flagged.
///   * belt-and-braces — a `post(` registration on the `.../tokens` COLLECTION
///     route. Detection is per `.route(..)` BLOCK (a route-literal + its method
///     handlers), so a `post(` and the `/tokens"` collection literal split across
///     SEPARATE source lines of the same axum route block (the multi-line form)
///     are caught — co-location on one line is NOT required. The existing
///     `get(list_tokens_handler)` + `delete(revoke_token_handler)` registrations,
///     and a `post(create_comment_handler)` on a DIFFERENT (issues/comments)
///     route block, are NOT flagged — only a `post(` inside the SAME route block
///     that carries the tokens-collection literal.
fn check_api_no_mint_route(root: &Path) -> Vec<String> {
    let api_src = root.join("crates").join("foundry-api").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&api_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };

        // Comment-stripped source, one entry per line (1-based line numbers).
        let stripped: Vec<String> = contents.lines().map(strip_comment).collect();

        // Load-bearing: a mint_token call site in the data-API tier.
        for (line_no, code) in stripped.iter().enumerate() {
            if code.contains("mint_token") {
                violations.push(format!(
                    "api≠mint: {} names `mint_token` at {}:{} (`{}`) — minting is confined to the /admin/tokens human-session path (foundry-app); the bearer data-API must never expose a mint surface (no-mint-boundary.md DD-TMA-04)",
                    handler_label(&file),
                    rel(root, &file),
                    line_no + 1,
                    code.trim(),
                ));
            }
        }

        // Belt-and-braces: per `.route(..)` BLOCK, flag the block if it contains
        // BOTH a `post(` AND a `.../tokens"` collection literal (regardless of
        // line co-location), naming the line carrying the `post(`.
        violations.extend(post_on_tokens_collection_blocks(&stripped).into_iter().map(
            |(post_line, post_code)| {
                format!(
                    "api≠mint: {} registers a POST on the `.../tokens` collection route at {}:{} (`{}`) — a mint route on the bearer surface is forbidden (no-mint-boundary.md DD-TMA-04)",
                    handler_label(&file),
                    rel(root, &file),
                    post_line + 1,
                    post_code.trim(),
                )
            },
        ));
    }
    violations
}

/// Scan `.route(..)` blocks in a comment-stripped source and report each block
/// that registers a `post(` against the `.../tokens` COLLECTION route. Returns
/// `(line_index, line_text)` of the offending `post(` line for each hit.
///
/// A route block opens at a line containing `.route(` and closes when the paren
/// depth (counted from the `.route(` onward) returns to zero. Within a block we
/// independently collect whether ANY line carries a `post(` and whether ANY line
/// carries a tokens-collection literal (`.../tokens"`, NOT `.../tokens/{...}"`).
/// If BOTH hold, the block is a mint surface — even when `post(` and the literal
/// sit on different lines (the multi-line evasion). This is intentionally
/// block-scoped, not file-scoped, so a `post(` on a SEPARATE (issues) route
/// block plus a GET-only tokens block do NOT false-positive.
fn post_on_tokens_collection_blocks(stripped: &[String]) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    for (block_start, end) in route_blocks(stripped) {
        // Two independent passes over the block's lines.
        let mut post_line: Option<(usize, String)> = None;
        let mut has_tokens_collection = false;
        for (line_no, line) in stripped[block_start..=end].iter().enumerate() {
            if post_line.is_none() && line.contains("post(") {
                post_line = Some((block_start + line_no, line.clone()));
            }
            if line_contains_tokens_collection_literal(line) {
                has_tokens_collection = true;
            }
        }
        if let (true, Some(hit)) = (has_tokens_collection, post_line) {
            hits.push(hit);
        }
    }
    hits
}

/// The `.route(..)` blocks of a comment-stripped source, as `(first, last)`
/// 0-based line indices. Shared by `api≠mint` and `api-openapi-operations`
/// (DESIGN OQ-D4: one block parser, not two).
fn route_blocks(stripped: &[String]) -> Vec<(usize, usize)> {
    let mut blocks = Vec::new();
    let mut idx = 0;
    while idx < stripped.len() {
        if !stripped[idx].contains(".route(") {
            idx += 1;
            continue;
        }
        // Walk the block from `.route(` to the matching close paren, tracking
        // paren depth across lines.
        let block_start = idx;
        let mut depth: i32 = 0;
        let mut started = false;
        let mut end = idx;
        'block: for (offset, line) in stripped[block_start..].iter().enumerate() {
            // Begin depth-counting at the `.route(` token on the first line.
            let scan = if offset == 0 {
                match line.find(".route(") {
                    Some(p) => &line[p..],
                    None => line.as_str(),
                }
            } else {
                line.as_str()
            };
            for ch in scan.chars() {
                if ch == '(' {
                    depth += 1;
                    started = true;
                } else if ch == ')' {
                    depth -= 1;
                }
                if started && depth == 0 {
                    end = block_start + offset;
                    break 'block;
                }
            }
            end = block_start + offset;
        }
        blocks.push((block_start, end));
        idx = end + 1;
    }
    blocks
}

/// True iff `code` carries a `.../tokens"` COLLECTION route literal (path segment
/// `tokens` immediately followed by the closing quote), NOT the `.../tokens/{jti}`
/// revoke route. The char before `tokens"` must be `/` (a path segment, not a
/// suffix like `mtokens"`).
fn line_contains_tokens_collection_literal(code: &str) -> bool {
    let mut search_from = 0;
    while let Some(rel_idx) = code[search_from..].find("tokens\"") {
        let idx = search_from + rel_idx;
        if code[..idx].ends_with('/') {
            return true;
        }
        search_from = idx + "tokens\"".len();
    }
    false
}

/// LAYER 1e — tenant-scoping (ADR-002, multi-workspace-tenancy / NFR-MWT-SEC-06).
/// A foundry-app handler must scope every tenant-scoped store call by the
/// RESOLVED acting workspace (`ActingWorkspace` / `user.workspace_id`), NEVER by
/// a workspace id parsed from request input (path/query/body). Trusting a
/// client-supplied workspace would let a member of A read/write B's data; the
/// shipped `find_*_in_workspace(id, acting_workspace_id)` idiom is only safe
/// because the second argument comes from the trusted resolution seam.
///
/// The detector flags a workspace-scoped store call (`*_in_workspace(`) whose
/// workspace argument is derived from `Uuid::parse*` of request input — either
/// INLINE in the call, or via a local bound from such a parse earlier in the
/// SAME file. It NAMES the offending file + the line of the scoped call.
///
/// Allow-list (ADR-002 escape hatch + ADR-004 provisioning): the resolution seam
/// and the super-admin / bootstrap provisioning paths legitimately handle a
/// literal/parsed workspace id, so those files are exempt — keeping the guard
/// precise on the genuinely instance-scoped paths and false-positive-free on the
/// shipped scoped queries (which pass the resolved id, not a parsed one).
fn check_app_tenant_scoping(root: &Path) -> Vec<String> {
    let app_src = root.join("crates").join("foundry-app").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&app_src) {
        if is_tenant_scoping_allowlisted(&file) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        let stripped: Vec<String> = contents.lines().map(strip_comment).collect();

        // Pass 1: collect locals bound to a `Uuid::parse*` of request input.
        // `let <name> = ... Uuid::parse_str(...) | .parse::<Uuid>() | .parse() ...`
        // The provenance is a request param (we are lenient: any parse of a
        // string into a Uuid is suspect as a workspace scope source — the
        // resolution seam, which is allow-listed, is the only legitimate parser).
        let mut tainted: Vec<String> = Vec::new();
        for line in &stripped {
            if let Some(name) = let_binding_name(line) {
                if line_parses_uuid(line) {
                    tainted.push(name);
                }
            }
        }

        // Pass 2: flag a `*_in_workspace(` call whose workspace argument is a
        // parse-derived value — INLINE, or a tainted local passed as the LAST
        // argument (the workspace-id slot by convention in `find_*_in_workspace`).
        for (line_no, line) in stripped.iter().enumerate() {
            let Some(args) = scoped_call_args(line) else {
                continue;
            };
            let inline_parse = line_parses_uuid(line);
            let arg_is_tainted = tainted
                .iter()
                .any(|t| args_mention_workspace_local(&args, t));
            if inline_parse || arg_is_tainted {
                violations.push(format!(
                    "tenant-scoping: {} scopes a tenant query by a request-parsed workspace id at {}:{} (`{}`) — a tenant-scoped store call must take the RESOLVED ActingWorkspace (user.workspace_id), never a Uuid parsed from path/query/body (ADR-002 LAYER-1e / NFR-MWT-SEC-06)",
                    handler_label(&file).replace("foundry-api", "foundry-app"),
                    rel(root, &file),
                    line_no + 1,
                    line.trim(),
                ));
            }
        }
    }
    violations
}

/// True iff `file` is on the tenant-scoping allow-list (ADR-002/004): the
/// resolution seam + provisioning paths that legitimately handle a literal or
/// parsed workspace id. Matched by file stem so a copy under a temp root (the
/// gold test) is exempt identically to the real tree.
fn is_tenant_scoping_allowlisted(file: &Path) -> bool {
    matches!(
        file.file_stem().and_then(|s| s.to_str()),
        // signin: the resolution seam (resolve_active_workspace, ADR-005).
        // bootstrap: initial-workspace provisioning / claim.
        // admin_cli: super-admin provisioning (ADR-004).
        // session: the ActingWorkspace newtype's home (no store calls).
        // instance_admin: super-admin WEB provisioning (web-provisioning-flow,
        //   ADR-004 / D6) — instance-scoped, creates a brand-new workspace id.
        Some("signin")
            | Some("bootstrap")
            | Some("admin_cli")
            | Some("session")
            | Some("instance_admin")
    )
}

/// If `line` is a `let <name> = ...;` binding, return `<name>` (the simple
/// identifier, ignoring `mut`). Returns `None` for non-bindings or pattern
/// destructures we don't track.
fn let_binding_name(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t.strip_prefix("let ")?;
    let rest = rest.strip_prefix("mut ").unwrap_or(rest);
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// True iff the line parses a `Uuid` from a string — the suspect provenance for
/// a workspace scope source. Covers `Uuid::parse_str(`, `Uuid::parse(`, and a
/// turbofish/explicit `.parse::<Uuid>(` / `.parse::<uuid::Uuid>(`.
fn line_parses_uuid(line: &str) -> bool {
    line.contains("Uuid::parse")
        || line.contains(".parse::<Uuid>")
        || line.contains(".parse::<uuid::Uuid>")
}

/// If `line` contains a workspace-scoped store call (`*_in_workspace(`), return
/// the argument substring between that call's opening paren and the line end (a
/// best-effort capture sufficient for the tainted-local membership test). The
/// `find_*_in_workspace` / `*_in_workspace` convention is the shipped
/// non-enumerable idiom (attachments.rs); the workspace id is its scope arg.
fn scoped_call_args(line: &str) -> Option<String> {
    let idx = line.find("_in_workspace(")?;
    let after = &line[idx + "_in_workspace(".len()..];
    Some(after.to_string())
}

/// True iff the captured argument list references the tainted local `name` as a
/// standalone identifier (not as a substring of a longer ident). The workspace
/// id is conventionally the trailing argument of `find_*_in_workspace`.
fn args_mention_workspace_local(args: &str, name: &str) -> bool {
    args.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|tok| tok == name)
}

/// LAYER 1c — JWT alg pin. The machine-token `Validation` (in foundry-auth, the
/// home of `MachineTokenVerifier`) MUST pin `algorithms = [EdDSA]` and never
/// disable signature validation. A `Validation` construction that loses the
/// pin (no EdDSA-only `algorithms` assignment) or sets
/// `insecure_disable_signature_validation` reopens the alg-confusion footgun.
fn check_jwt_alg_pin(root: &Path) -> Vec<String> {
    let auth_src = root.join("crates").join("foundry-auth").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&auth_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        let code: String = contents
            .lines()
            .map(strip_comment)
            .collect::<Vec<_>>()
            .join("\n");

        let constructs_validation =
            code.contains("Validation::new") || code.contains("Validation {");
        if !constructs_validation {
            continue;
        }

        // The footgun: explicitly disabling signature validation.
        if code.contains("insecure_disable_signature_validation") {
            violations.push(format!(
                "JWT alg pin: {} disables signature validation (`insecure_disable_signature_validation`) — the credential verifier no longer pins the single allowed algorithm [EdDSA] (NFR-WEB-API-SEC-02)",
                rel(root, &file),
            ));
            continue;
        }

        // The pin must be present: an `algorithms = vec![... EdDSA ...]`
        // assignment that lists ONLY EdDSA. Accept the canonical
        // `validation.algorithms = vec![..EdDSA..]` form. If a `Validation` is
        // built but no EdDSA-only `algorithms` pin appears, the verifier would
        // accept whatever default/extra alg is configured — a lost pin.
        let pins_eddsa_only = pins_algorithms_to_eddsa(&code);
        if !pins_eddsa_only {
            violations.push(format!(
                "JWT alg pin: {} builds a JWT `Validation` without pinning `algorithms = [EdDSA]` — the credential verifier no longer pins the single allowed algorithm, reopening the alg-confusion footgun (NFR-WEB-API-SEC-02)",
                rel(root, &file),
            ));
        }
    }
    violations
}

/// The SIBLING of [`check_jwt_alg_pin`], for the other credential class.
///
/// foundry verifies two kinds of JWT with two different algorithms: self-issued
/// machine tokens (EdDSA, in foundry-auth) and Keycloak ID tokens (RS256, in
/// foundry-oidc). One file-scoped rule cannot express "EdDSA here, RS256 there" —
/// `pins_algorithms_to_eddsa` reads only the FIRST `algorithms` list in a file —
/// so the crate boundary IS the security boundary (ADR-OIDC-001), and each side
/// gets its own scanner that fails independently.
///
/// Without this, moving ID-token validation into its own crate would have bought
/// separation at the cost of ALL algorithm pinning on the federated path: nothing
/// would stop a later edit accepting `none` or HS256, which is the alg-confusion
/// footgun that authenticates the wrong person and emits no signal.
fn check_oidc_alg_pin(root: &Path) -> Vec<String> {
    let oidc_src = root.join("crates").join("foundry-oidc").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&oidc_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        let code: String = contents
            .lines()
            .map(strip_comment)
            .collect::<Vec<_>>()
            .join("\n");

        if !(code.contains("Validation::new") || code.contains("Validation {")) {
            continue;
        }

        if code.contains("insecure_disable_signature_validation") {
            violations.push(format!(
                "OIDC alg pin: {} disables signature validation (`insecure_disable_signature_validation`) — the ID-token verifier no longer pins the single allowed algorithm [RS256] (ADR-OIDC-001)",
                rel(root, &file),
            ));
            continue;
        }

        if !pins_algorithms_to_rs256(&code) {
            violations.push(format!(
                "OIDC alg pin: {} builds a JWT `Validation` without pinning `algorithms = [RS256]` — the ID-token verifier would accept whatever default/extra alg is configured, reopening the alg-confusion footgun (ADR-OIDC-001)",
                rel(root, &file),
            ));
        }
    }
    violations
}

/// LAYER 1f — single production slugify (ADR-PROJECT-RENAME-001). `slugify`
/// lives ONCE, in `foundry-core`; any `fn slugify(` DEFINITION under
/// `crates/foundry-app/src` fails the build. Calling `foundry_core::slugify`
/// is fine — growing a new private name→slug derivation is the regression
/// class behind the D2 defect (render paths re-deriving a stored project's
/// URL identity from its display name, so a name-only rename 404s every card
/// action). Follows the file-scoped posture of [`check_jwt_alg_pin`]:
/// invariants live in build-time scanners, not conventions.
fn check_app_no_slugify_definition(root: &Path) -> Vec<String> {
    let app_src = root.join("crates").join("foundry-app").join("src");
    let mut violations = Vec::new();
    for file in rust_sources(&app_src) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        let defines_slugify = contents
            .lines()
            .map(strip_comment)
            .any(|line| line.contains("fn slugify("));
        if defines_slugify {
            violations.push(format!(
                "single slugify: {} defines `fn slugify(` — the ONLY production slug derivation is `foundry_core::slugify`, minted once at creation time; a private re-derivation is the regression class that 404s every board card after a display-name-only rename (ADR-PROJECT-RENAME-001)",
                rel(root, &file),
            ));
        }
    }
    violations
}

/// LAYER 1g — no static lane list (board-lane-management, architecture-
/// design.md §8 / ADR-BOARD-LANE-001). A board's lanes are the project's OWN
/// rows (`Store::list_project_lanes`); the render/validation adapters must
/// never re-acquire a static enumeration of lane slugs. The rule follows the
/// `slugify`-ban idiom: fail the build if a static array/match enumerating
/// lane slugs (`"backlog"`, `"todo"`, …) reappears under
/// `crates/foundry-app/src` or `crates/foundry-api/src` outside `#[cfg(test)]`.
///
/// Matcher shape: within any window of 6 consecutive comment-stripped code
/// lines, string literals naming ≥ 3 DISTINCT lanes of the closed set
/// (slug or label form: backlog/Backlog, todo/Todo, in_progress/in-progress/
/// In-Progress/In Progress, done/Done, cancelled/canceled/Cancelled) flag the
/// window — a genuine lane list enumerates several lanes close together,
/// while a single hardcoded slug (e.g. one OOB column selector) does not.
///
/// The TWO documented exemptions (design contract):
///   * the store creation-seed template (`CREATION_LANE_SEED`,
///     foundry-store/src/lanes.rs) — it WRITES lane rows at project creation
///     and never renders or validates; it lives OUTSIDE the scanned dirs.
///   * `humanize_state`'s historical fallback (foundry-app/src/comments.rs) —
///     the display fallback for DEAD slugs in old change events; its function
///     body is region-skipped below.
///
/// `#[cfg(test)]` blocks are region-skipped (test fixtures may enumerate).
fn check_no_static_lane_list(root: &Path) -> Vec<String> {
    /// `(lane identity, the quoted literal forms that name it)`.
    const LANE_TOKENS: &[(&str, &[&str])] = &[
        ("backlog", &["\"backlog\"", "\"Backlog\""]),
        ("todo", &["\"todo\"", "\"Todo\""]),
        (
            "in_progress",
            &[
                "\"in_progress\"",
                "\"in-progress\"",
                "\"In-Progress\"",
                "\"In Progress\"",
            ],
        ),
        ("done", &["\"done\"", "\"Done\""]),
        (
            "cancelled",
            &["\"cancelled\"", "\"canceled\"", "\"Cancelled\""],
        ),
    ];
    const WINDOW: usize = 6;
    const DISTINCT_LANES_THRESHOLD: usize = 3;

    let mut violations = Vec::new();
    for crate_dir in ["foundry-app", "foundry-api"] {
        let src = root.join("crates").join(crate_dir).join("src");
        for file in rust_sources(&src) {
            let Ok(contents) = std::fs::read_to_string(&file) else {
                continue;
            };
            let stripped: Vec<String> = contents.lines().map(strip_comment).collect();
            let scan = lane_scan_mask(&stripped);

            // Per-line distinct-lane sets, zeroed on skipped lines.
            let lanes_per_line: Vec<Vec<&str>> = stripped
                .iter()
                .enumerate()
                .map(|(idx, line)| {
                    if !scan[idx] {
                        return Vec::new();
                    }
                    LANE_TOKENS
                        .iter()
                        .filter(|(_, forms)| forms.iter().any(|form| line.contains(form)))
                        .map(|(lane, _)| *lane)
                        .collect()
                })
                .collect();

            for start in 0..stripped.len() {
                let end = (start + WINDOW).min(stripped.len());
                let mut distinct: Vec<&str> = lanes_per_line[start..end].concat();
                distinct.sort_unstable();
                distinct.dedup();
                if distinct.len() >= DISTINCT_LANES_THRESHOLD {
                    violations.push(format!(
                        "no-static-lane-list: {} enumerates {} lane slugs ({}) near {}:{} — a board's lanes are the project's OWN rows (Store::list_project_lanes); a static lane list in a render/validation adapter is the regression class D8 exists to end (architecture-design.md §8 / ADR-BOARD-LANE-001; exemptions: the store creation seed, humanize_state's historical fallback)",
                        handler_label(&file).replace("foundry-api", crate_dir),
                        distinct.len(),
                        distinct.join(", "),
                        rel(root, &file),
                        start + 1,
                    ));
                    break; // one violation per file names it well enough
                }
            }
        }
    }
    violations
}

/// The scan mask for [`check_no_static_lane_list`]: `false` on lines inside a
/// `#[cfg(test)]` item block or inside `fn humanize_state(`'s body (the
/// documented display-fallback exemption). Blocks are skipped by brace
/// counting from the marker line to the line where its depth returns to zero.
fn lane_scan_mask(stripped: &[String]) -> Vec<bool> {
    let mut scan = vec![true; stripped.len()];
    let mut idx = 0;
    while idx < stripped.len() {
        let line = &stripped[idx];
        if line.contains("#[cfg(test)]") || line.contains("fn humanize_state(") {
            let end = block_end(stripped, idx);
            for masked in scan.iter_mut().take(end + 1).skip(idx) {
                *masked = false;
            }
            idx = end + 1;
        } else {
            idx += 1;
        }
    }
    scan
}

/// The index of the line on which the item block starting at/after
/// `start` closes: brace depth counted from the FIRST `{` at or after
/// `start`, returning when it drops back to zero. If no brace opens within
/// the next few lines (a bare attribute on a non-block item), returns `start`.
fn block_end(stripped: &[String], start: usize) -> usize {
    let mut depth: i32 = 0;
    let mut started = false;
    for (offset, line) in stripped[start..].iter().enumerate() {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                started = true;
            } else if ch == '}' {
                depth -= 1;
            }
            if started && depth == 0 {
                return start + offset;
            }
        }
        // A `#[cfg(test)]` attribute whose item never opens a block within a
        // conservative lookahead: stop masking after that lookahead.
        if !started && offset > 3 {
            return start + offset;
        }
    }
    stripped.len().saturating_sub(1)
}

/// True iff the source pins the algorithm allow-list to EXACTLY `[RS256]`.
/// Mirrors [`pins_algorithms_to_eddsa`], with the accepted and rejected sets
/// swapped: `EdDSA` leaking into the OIDC list is as wrong as `RS256` leaking
/// into the machine-token one.
fn pins_algorithms_to_rs256(code: &str) -> bool {
    let Some(idx) = code.find("algorithms") else {
        return false;
    };
    let tail = &code[idx..];
    let Some(open) = tail.find('[') else {
        return false;
    };
    let Some(close_rel) = tail[open..].find(']') else {
        return false;
    };
    let inside = &tail[open + 1..open + close_rel];
    let mentions_rs256 = inside.contains("RS256");
    let other_alg = [
        "EdDSA", "RS384", "RS512", "HS256", "HS384", "HS512", "ES256", "ES384", "PS256", "PS384",
        "PS512", "none", "None",
    ]
    .iter()
    .any(|alg| inside.contains(alg));
    mentions_rs256 && !other_alg
}

/// True iff the source pins the JWT algorithm allow-list to EXACTLY `[EdDSA]`:
/// an `algorithms = vec![ ... EdDSA ... ]` assignment that mentions EdDSA and
/// no OTHER algorithm token. A bare `Validation::new(EdDSA)` is NOT sufficient
/// on its own here because the production verifier reassigns `algorithms`; we
/// require the explicit pinning assignment to be present and EdDSA-only.
fn pins_algorithms_to_eddsa(code: &str) -> bool {
    // Find an `algorithms = vec![...]` assignment.
    let Some(idx) = code.find("algorithms") else {
        return false;
    };
    let tail = &code[idx..];
    let Some(open) = tail.find('[') else {
        return false;
    };
    let Some(close_rel) = tail[open..].find(']') else {
        return false;
    };
    let inside = &tail[open + 1..open + close_rel];
    let mentions_eddsa = inside.contains("EdDSA");
    // Reject if any non-EdDSA algorithm token leaks into the allow-list.
    let other_alg = [
        "RS256", "RS384", "RS512", "HS256", "HS384", "HS512", "ES256", "ES384", "PS256", "PS384",
        "PS512", "none", "None",
    ]
    .iter()
    .any(|alg| inside.contains(alg));
    mentions_eddsa && !other_alg
}

/// LAYER 2 — delegate the crate-graph dependency-direction check to cargo-deny
/// against the target tree's manifest. Returns `Some(violation)` if cargo-deny
/// reports a banned edge (NAMING the forbidden crate), `None` if clean.
/// The `UNIQUE (project_id, position)` constraint on `lanes` must stay
/// `DEFERRABLE`.
///
/// This rule exists because the keyword is load-bearing and its removal is
/// SILENT. `DEFERRABLE INITIALLY IMMEDIATE` makes the constraint checked at
/// end-of-STATEMENT rather than per row, and that is the only reason two
/// shipped operations work:
///
///   * lane INSERT shifts later positions with a plain bulk `UPDATE`
///     (ADR-BOARD-LANE-003), and
///   * lane MOVE applies a whole permutation in one `CASE` statement
///     (ADR-BOARD-LANE-006).
///
/// Both were MEASURED to fail against a non-deferrable twin — insert and all
/// three candidate move shapes, the last with
/// `constraint "…" is not deferrable`. Nothing in the existing test suite
/// notices the keyword's absence until a runtime insert or move fails, because
/// no test asserts the constraint's definition. ADR-BOARD-LANE-003 recommended
/// this guard and left it unimplemented; board-lane-reorder made it a DoD item
/// once the keyword was carrying two operations by four routes.
fn check_lane_position_deferrable(root: &Path) -> Vec<String> {
    let migration = root
        .join("crates")
        .join("foundry-store")
        .join("migrations")
        .join("0015_project_lanes.sql");
    let Ok(contents) = std::fs::read_to_string(&migration) else {
        return vec![format!(
            "check-arch: cannot read {} — the lanes position constraint could not be verified. \
             That constraint's DEFERRABLE keyword is load-bearing for lane insert \
             (ADR-BOARD-LANE-003) and lane move (ADR-BOARD-LANE-006).",
            migration.display()
        )];
    };
    // SQL comments are `--`, not `//` — `strip_comment` is the Rust stripper and
    // would leave a commented-out constraint looking live. The gold test
    // `a_commented_out_deferrable_does_not_satisfy_the_rule` failed on exactly
    // that before this line existed.
    let stripped: String = contents
        .lines()
        .map(|line| match line.find("--") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join(" ");
    let flat = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    let declares_position_unique = flat.contains("UNIQUE (project_id, position)");
    if !declares_position_unique {
        return vec![format!(
            "check-arch: {} no longer declares `UNIQUE (project_id, position)` on `lanes`. \
             That constraint is the only DB-level guard against two lanes sharing a slot, and \
             lane insert + lane move both depend on its DEFERRABLE end-of-statement semantics \
             (ADR-BOARD-LANE-003 / -006).",
            migration.display()
        )];
    }
    let deferrable = flat.contains("UNIQUE (project_id, position) DEFERRABLE");
    if deferrable {
        return Vec::new();
    }
    vec![format!(
        "check-arch: `UNIQUE (project_id, position)` in {} is no longer DEFERRABLE. This breaks \
         lane INSERT (the bulk position shift, ADR-BOARD-LANE-003) and lane MOVE (the CASE \
         permutation, ADR-BOARD-LANE-006) at RUNTIME while every existing test stays green — \
         both were measured to fail with `duplicate key` against a non-deferrable constraint. \
         Restore the keyword, or change both store operations and their ADRs together.",
        migration.display()
    )]
}

/// DDD-22 / BR-4 — Escape has exactly one owner, `keyboard.js::closeTopLayer()`.
/// A drag module cancels through a closeTopLayer arm, never through a keydown
/// listener of its own. Every `board-*.js` in the served js directory is
/// scanned — listed, never hard-coded, so a new board module is covered the
/// day it lands. Only REGISTRATIONS are flagged (`addEventListener("keydown"`,
/// `.onkeydown =`, `on("keydown"`), after JS comments are stripped: the
/// DEFERRABLE rule's lesson is that a stripper for the wrong comment syntax
/// lets a commented-out case decide the verdict. A missing js directory fails
/// the rule rather than passing it vacuously.
fn check_board_modules_have_no_keydown_listener(root: &Path) -> Vec<String> {
    let js_dir = root
        .join("crates")
        .join("foundry-app")
        .join("static")
        .join("js");
    let Ok(entries) = std::fs::read_dir(&js_dir) else {
        return vec![format!(
            "no-board-keydown: cannot list {} — no board module could be checked for a \
             keydown listener. BR-4: Escape has one owner, `keyboard.js::closeTopLayer()`.",
            js_dir.display()
        )];
    };
    let mut modules: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("board-") && n.ends_with(".js"))
        })
        .collect();
    modules.sort();
    let mut violations = Vec::new();
    for module in modules {
        let Ok(source) = std::fs::read_to_string(&module) else {
            violations.push(format!(
                "no-board-keydown: cannot read {} — it could not be checked for a keydown \
                 listener.",
                rel(root, &module)
            ));
            continue;
        };
        for (idx, line) in strip_js_comments(&source).lines().enumerate() {
            if registers_keydown_listener(line) {
                violations.push(format!(
                    "no-board-keydown: {}:{} registers a keydown listener. BR-4: Escape has \
                     one owner, `keyboard.js::closeTopLayer()` — cancel through a \
                     closeTopLayer arm instead.",
                    rel(root, &module),
                    idx + 1
                ));
            }
        }
    }
    violations
}

/// Blank every JS `//` line comment and `/* … */` block comment, PRESERVING
/// newlines so line numbers still map onto the original file. String literals
/// (`'`, `"`, `` ` ``) are tracked and kept intact, so a `//` inside
/// `"https://…"` does not hide a registration later on the same line.
/// Known limitations: regex literals and `${…}` nesting inside template
/// literals are not modelled — neither occurs in a way that matters for the
/// board modules today.
fn strip_js_comments(source: &str) -> String {
    #[derive(PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Str(char),
    }
    let mut out = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match state {
            State::Code => match (ch, chars.peek()) {
                ('/', Some('/')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Line;
                }
                ('/', Some('*')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Block;
                }
                ('"' | '\'' | '`', _) => {
                    out.push(ch);
                    state = State::Str(ch);
                }
                _ => out.push(ch),
            },
            State::Line => {
                if ch == '\n' {
                    out.push('\n');
                    state = State::Code;
                } else {
                    out.push(' ');
                }
            }
            State::Block => {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    out.push_str("  ");
                    state = State::Code;
                } else if ch == '\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
            }
            State::Str(quote) => {
                out.push(ch);
                if ch == '\\' {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                } else if ch == quote || (ch == '\n' && quote != '`') {
                    state = State::Code;
                }
            }
        }
    }
    out
}

/// True when `code` (one comment-stripped line) registers a keydown listener:
/// `addEventListener(\s*["'`]keydown["'`]`, `\.onkeydown\s*=`, or
/// `on(\s*["']keydown["']`. The bare word in a string is not a registration.
fn registers_keydown_listener(code: &str) -> bool {
    let quoted_keydown_follows = |rest: &str, quotes: &[char]| -> bool {
        let rest = rest.trim_start();
        let mut it = rest.chars();
        let Some(open) = it.next() else {
            return false;
        };
        let tail = it.as_str();
        quotes.contains(&open)
            && tail
                .strip_prefix("keydown")
                .and_then(|after| after.chars().next())
                .is_some_and(|close| quotes.contains(&close))
    };
    let after_each = |needle: &str| -> Vec<&str> {
        code.match_indices(needle)
            .map(|(idx, _)| &code[idx + needle.len()..])
            .collect()
    };
    after_each("addEventListener(")
        .into_iter()
        .any(|rest| quoted_keydown_follows(rest, &['"', '\'', '`']))
        || after_each(".onkeydown")
            .into_iter()
            .any(|rest| rest.trim_start().starts_with('='))
        || after_each("on(")
            .into_iter()
            .any(|rest| quoted_keydown_follows(rest, &['"', '\'']))
}

/// The one migration allowed to UPDATE `users.provisioned_at`: its OD-14
/// backfill marks pre-existing password-less accounts at their `created_at`.
const PROVISIONED_MARKER_BACKFILL: &str =
    "crates/foundry-store/migrations/0017_users_provisioned_at.sql";

/// keycloak-sso D9 / DDD-27 / OQ-8 — the provisioned marker is permanent.
/// `users.provisioned_at` is written by exactly one INSERT
/// (`Store::provision_federated_member`) and by migration 0017's backfill;
/// nothing clears or rewrites it. Every `.rs` under `crates/` (except each
/// crate's `tests/`, where store tests seed rows directly) and every
/// `crates/*/migrations/*.sql` is globbed — never hand-listed — and any SQL
/// `UPDATE … SET … provisioned_at = …` is flagged, case-insensitively and
/// across line breaks within one Rust string literal or one SQL statement.
/// Rust and SQL comments are stripped first, so a commented-out statement
/// cannot decide the verdict. An unreadable directory fails the rule.
fn check_provisioned_marker_is_never_rewritten(root: &Path) -> Vec<String> {
    let crates_dir = root.join("crates");
    let Ok(entries) = std::fs::read_dir(&crates_dir) else {
        return vec![format!(
            "provisioned-marker: cannot list {} — no crate could be checked for an UPDATE of \
             users.provisioned_at. D9: the provisioned marker is permanent.",
            crates_dir.display()
        )];
    };
    let mut crate_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    crate_dirs.sort();

    let mut violations = Vec::new();
    for crate_dir in crate_dirs {
        let tests_dir = crate_dir.join("tests");
        let (sources, unreadable) = files_under(&crate_dir, "rs", &|dir| dir == tests_dir);
        let migrations_dir = crate_dir.join("migrations");
        let (migrations, unlisted) = if migrations_dir.is_dir() {
            // `*.sql` directly inside `migrations/` — no descent.
            files_under(&migrations_dir, "sql", &|_| true)
        } else {
            (Vec::new(), Vec::new())
        };
        for dir in unreadable.iter().chain(&unlisted) {
            violations.push(format!(
                "provisioned-marker: cannot list {} — it could not be checked for an UPDATE of \
                 users.provisioned_at (D9).",
                rel(root, dir)
            ));
        }
        for file in sources.iter().chain(&migrations) {
            if rel(root, file) == PROVISIONED_MARKER_BACKFILL {
                continue;
            }
            violations.extend(provisioned_marker_violations_in(root, file));
        }
    }
    violations
}

/// The D9 violations in one file, each naming `file:line`.
fn provisioned_marker_violations_in(root: &Path, file: &Path) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(file) else {
        return vec![format!(
            "provisioned-marker: cannot read {} — it could not be checked for an UPDATE of \
             users.provisioned_at (D9).",
            rel(root, file)
        )];
    };
    let sql_texts: Vec<(usize, String)> = if file.extension().is_some_and(|e| e == "sql") {
        vec![(1, source)]
    } else {
        rust_string_literals(&source)
    };
    sql_texts
        .iter()
        .flat_map(|(first_line, sql)| {
            provisioned_at_assignments(sql)
                .into_iter()
                .map(move |offset| first_line + offset)
        })
        .map(|line| {
            format!(
                "provisioned-marker: {}:{} UPDATEs users.provisioned_at. D9: the provisioned \
                 marker is permanent — only the provisioning INSERT \
                 (`Store::provision_federated_member`) writes it, and only migration 0017's \
                 backfill may UPDATE it (DDD-27).",
                rel(root, file),
                line
            )
        })
        .collect()
}

/// Recursively list `*.{extension}` files under `dir`, not descending into any
/// directory `skip` accepts. Also returns every directory that could not be
/// read, so the caller can fail closed instead of passing over it.
fn files_under(
    dir: &Path,
    extension: &str,
    skip: &dyn Fn(&Path) -> bool,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut files = Vec::new();
    let mut unreadable = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            unreadable.push(current);
            continue;
        };
        for path in entries.flatten().map(|entry| entry.path()) {
            if path.is_dir() {
                if !skip(&path) {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == extension) {
                files.push(path);
            }
        }
    }
    files.sort();
    unreadable.sort();
    (files, unreadable)
}

/// Every string literal in a Rust source (`"…"`, `r#"…"#`, `b"…"`) with the
/// 1-based line it opens on. `//` and nested `/* … */` comments are skipped
/// rather than read, so a commented-out query never yields a literal; char
/// literals are consumed so `'"'` does not open a string.
fn rust_string_literals(source: &str) -> Vec<(usize, String)> {
    RustSource::scan(source)
        .literals
        .into_iter()
        .map(|literal| (literal.line, literal.text))
        .collect()
}

/// One string literal of a [`RustSource`]: the 1-based line it opens on, its
/// text, and its `[start, end)` span in [`RustSource::code`].
struct RustLiteral {
    line: usize,
    start: usize,
    end: usize,
    text: String,
}

/// A Rust source split into its string literals and its CODE: the source with
/// every comment and every literal's contents blanked to spaces (newlines
/// kept, non-ASCII code blanked too), so one byte of `code` is one char of the
/// source and a brace, a call or a `;` found in `code` is never inside a
/// string or a comment.
struct RustSource {
    code: String,
    literals: Vec<RustLiteral>,
}

impl RustSource {
    fn scan(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let mut code: Vec<char> = chars
            .iter()
            .map(|&c| if c == '\n' || c.is_ascii() { c } else { ' ' })
            .collect();
        let blank = |code: &mut Vec<char>, from: usize, to: usize| {
            for c in code.iter_mut().take(to.min(chars.len())).skip(from) {
                if *c != '\n' {
                    *c = ' ';
                }
            }
        };
        let is_ident = |c: char| c.is_alphanumeric() || c == '_';
        let mut literals = Vec::new();
        let mut line = 1;
        let mut i = 0;
        while i < chars.len() {
            let next = chars.get(i + 1).copied();
            match chars[i] {
                '\n' => {
                    line += 1;
                    i += 1;
                }
                '/' if next == Some('/') => {
                    let from = i;
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    blank(&mut code, from, i);
                }
                '/' if next == Some('*') => {
                    let from = i;
                    let mut depth = 0usize;
                    while i < chars.len() {
                        match (chars[i], chars.get(i + 1).copied()) {
                            ('/', Some('*')) => {
                                depth += 1;
                                i += 2;
                            }
                            ('*', Some('/')) => {
                                depth -= 1;
                                i += 2;
                                if depth == 0 {
                                    break;
                                }
                            }
                            (c, _) => {
                                line += usize::from(c == '\n');
                                i += 1;
                            }
                        }
                    }
                    blank(&mut code, from, i);
                }
                '"' => {
                    let opened_on = line;
                    let start = i;
                    let mut text = String::new();
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        let take = if chars[i] == '\\' { 2 } else { 1 };
                        for &c in chars.iter().skip(i).take(take) {
                            line += usize::from(c == '\n');
                            text.push(c);
                        }
                        i += take;
                    }
                    i += 1;
                    blank(&mut code, start + 1, i.saturating_sub(1));
                    literals.push(RustLiteral {
                        line: opened_on,
                        start,
                        end: i.min(chars.len()),
                        text,
                    });
                }
                'r' if i == 0 || !is_ident(chars[i - 1]) || chars[i - 1] == 'b' => {
                    let hashes = chars[i + 1..].iter().take_while(|&&c| c == '#').count();
                    if chars.get(i + 1 + hashes) != Some(&'"') {
                        i += 1;
                        continue;
                    }
                    let opened_on = line;
                    let start = i;
                    let closing: Vec<char> = std::iter::once('"')
                        .chain(std::iter::repeat_n('#', hashes))
                        .collect();
                    let mut text = String::new();
                    i += hashes + 2;
                    let body = i;
                    while i < chars.len() && !chars[i..].starts_with(&closing) {
                        line += usize::from(chars[i] == '\n');
                        text.push(chars[i]);
                        i += 1;
                    }
                    blank(&mut code, body, i);
                    i += closing.len();
                    literals.push(RustLiteral {
                        line: opened_on,
                        start,
                        end: i.min(chars.len()),
                        text,
                    });
                }
                '\'' if next == Some('\\') => {
                    i += 3;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                }
                '\'' if chars.get(i + 2) == Some(&'\'') => {
                    blank(&mut code, i + 1, i + 2);
                    i += 3;
                }
                _ => i += 1,
            }
        }
        RustSource {
            code: code.into_iter().collect(),
            literals,
        }
    }
}

/// The 0-based line offsets, within `sql`, of every assignment to
/// `provisioned_at` in an `UPDATE … SET` clause — `provisioned_at = …` or a
/// `(…, provisioned_at, …) = (…)` row assignment. SQL comments are stripped
/// first and each `;`-separated statement is matched whole, so the clause may
/// span lines. The clause ends at WHERE / FROM / RETURNING, which keeps a
/// `WHERE provisioned_at = $1` filter and every SELECT out of scope.
fn provisioned_at_assignments(sql: &str) -> Vec<usize> {
    let code = strip_sql_comments(sql).to_ascii_lowercase();
    let mut offsets = Vec::new();
    let mut statement_start = 0;
    for statement in code.split(';') {
        let mut from = 0;
        while let Some(update) = find_sql_word(statement, "update", from) {
            from = update + "update".len();
            let Some(set) = find_sql_word(statement, "set", from) else {
                break;
            };
            let clause_start = set + "set".len();
            let clause_end = ["where", "from", "returning"]
                .iter()
                .filter_map(|word| find_sql_word(statement, word, clause_start))
                .min()
                .unwrap_or(statement.len());
            let clause = &statement[clause_start..clause_end];
            let mut at = 0;
            while let Some(column) = find_sql_word(clause, "provisioned_at", at) {
                at = column + "provisioned_at".len();
                if is_assignment_target(clause, column, at) {
                    let offset = statement_start + clause_start + column;
                    offsets.push(code[..offset].matches('\n').count());
                }
            }
        }
        statement_start += statement.len() + 1;
    }
    offsets.dedup();
    offsets
}

/// True when the column word spanning `start..end` in a SET clause is assigned:
/// followed by `=`, or inside a parenthesised column list followed by `=`.
fn is_assignment_target(clause: &str, start: usize, end: usize) -> bool {
    let assigns = |rest: &str| {
        let rest = rest.trim_start();
        rest.starts_with('=') && !rest.starts_with("==")
    };
    if assigns(&clause[end..]) {
        return true;
    }
    let opened = clause[..start].rfind('(');
    let closed_before = clause[..start].rfind(')');
    match (opened, clause[end..].find(')')) {
        (Some(open), Some(close)) if closed_before.is_none_or(|c| c < open) => {
            assigns(&clause[end + close + 1..])
        }
        _ => false,
    }
}

/// Byte offset of the first whole-word occurrence of `word` in `text` at or
/// after `from`. `text` and `word` are expected to share a case.
fn find_sql_word(text: &str, word: &str, from: usize) -> Option<usize> {
    let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    text.get(from..)?
        .match_indices(word)
        .map(|(idx, _)| from + idx)
        .find(|&idx| {
            let before = idx.checked_sub(1).map(|b| bytes[b]);
            let after = bytes.get(idx + word.len()).copied();
            !before.is_some_and(is_word_byte) && !after.is_some_and(is_word_byte)
        })
}

/// Blank SQL `--` line comments and `/* … */` block comments, preserving
/// newlines so line offsets survive. `'…'` string literals are kept intact.
fn strip_sql_comments(sql: &str) -> String {
    #[derive(PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Quoted,
    }
    let mut out = String::with_capacity(sql.len());
    let mut state = State::Code;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        let blank = if ch == '\n' { '\n' } else { ' ' };
        match state {
            State::Code => match (ch, chars.peek()) {
                ('-', Some('-')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Line;
                }
                ('/', Some('*')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Block;
                }
                ('\'', _) => {
                    out.push(ch);
                    state = State::Quoted;
                }
                _ => out.push(ch),
            },
            State::Line => {
                out.push(blank);
                if ch == '\n' {
                    state = State::Code;
                }
            }
            State::Block => {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    out.push_str("  ");
                    state = State::Code;
                } else {
                    out.push(blank);
                }
            }
            State::Quoted => {
                out.push(ch);
                if ch == '\'' {
                    state = State::Code;
                }
            }
        }
    }
    out
}

fn check_dependency_direction(root: &Path) -> Option<String> {
    let manifest = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .args(["deny", "--manifest-path"])
        .arg(&manifest)
        .args(["check", "bans"])
        .output();
    match output {
        Ok(out) if out.status.success() => None,
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // cargo-deny names the banned crate (e.g. "crate 'foundry-store …'
            // is explicitly banned"). Surface its naming verbatim so the guard
            // output names the forbidden dependency.
            let named = stderr
                .lines()
                .find(|l| l.contains("error[banned]") || l.contains("is explicitly banned"))
                .or_else(|| stderr.lines().find(|l| l.contains("banned")))
                .unwrap_or("a forbidden dependency edge")
                .trim();
            Some(format!(
                "dependency-direction: cargo-deny rejected the crate graph — {named} (an adapter must reach foundry-store ONLY through foundry-services; boundary-guard.md LAYER 2)"
            ))
        }
        Err(err) => Some(format!(
            "dependency-direction: could not run `cargo deny check bans` (is cargo-deny installed?): {err}"
        )),
    }
}

/// LAYER 1i — static-asset integrity (ADR-CANZAN-THEME-003). Three assertions
/// over `crates/foundry-app`, keeping `assets.md:115-126`'s eight-month-old
/// promise of an asset-resolution probe:
///
///   * **R1 — every reference resolves.** Every `/static/<path>` token in the
///     app's Rust sources, askama templates, stylesheets and
///     `manifest.webmanifest` names a file that exists under
///     `crates/foundry-app/static/`. Catches *renamed but not re-referenced*.
///   * **R2 — every content-hashed filename is honest.** Every file named
///     `<stem>.<8 lowercase hex>.<ext>` hashes to that 8-hex prefix. Catches
///     *edited but not renamed* — the failure R1 structurally cannot see,
///     which pins a stale byte at an `immutable` URL for a year.
///   * **R3 — every `VENDOR.md` row is true.** Every provenance table row's
///     recorded sha256 recomputes (Tier 1 of ADR-CANZAN-THEME-002).
///
/// The scan set is DERIVED, never enumerated: adding a blob and referencing it
/// enrols it automatically, so the guard cannot itself go stale.
///
/// Three scope exclusions, each forced by evidence in the tree (ADR-003
/// Decision, scoping rules 1-3):
///   1. `crates/foundry-acceptance/` is NOT scanned — it holds deliberate
///      non-resolving fixtures (`feature_b_web_tier.rs:486`
///      `/static/css/does-not-exist.css` expecting 404, and `:495`
///      `/static/../Cargo.toml` probing path traversal). Scanning it would
///      make the guard permanently red.
///   2. `#[cfg(test)]` blocks are NOT region-skipped — a deliberate departure
///      from `check_no_static_lane_list`'s posture, because the three stale-hash
///      literals this rule exists to protect live INSIDE
///      `#[cfg(test)] mod static_cache_policy_tests` (`lib.rs:312-373`). A rule
///      that skipped test blocks would miss its entire reason for existing.
///   3. An extracted path must carry a file extension — `projects.rs:1048`
///      holds the deliberate hash-agnostic PREFIX literal
///      `href="/static/css/foundry.`. Requiring a trailing `.<2-12 alphanumerics>`
///      skips prefixes and format templates without an allowlist. (ADR-003
///      writes `2-5`; widened to 12 here so `manifest.webmanifest` — a real
///      reference in `base.html:10` — is checked rather than skipped.)
///      Documented limit: a typo that also drops the extension escapes R1.
fn check_static_asset_integrity(root: &Path) -> Vec<String> {
    let app = root.join("crates").join("foundry-app");
    let static_dir = app.join("static");
    let mut violations = Vec::new();
    violations.extend(check_static_references_resolve(root, &app, &static_dir));
    violations.extend(check_hashed_filenames_are_honest(root, &static_dir));
    violations.extend(check_vendor_rows_recompute(root, &static_dir));
    violations
}

/// R1 — every `/static/<path>` reference in `foundry-app` resolves on disk.
fn check_static_references_resolve(root: &Path, app: &Path, static_dir: &Path) -> Vec<String> {
    let mut sources = files_with_extensions(&app.join("src"), &["rs"]);
    sources.extend(files_with_extensions(&app.join("templates"), &["html"]));
    sources.extend(files_with_extensions(static_dir, &["css", "webmanifest"]));
    sources.sort();

    let mut violations = Vec::new();
    for file in sources {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            for reference in static_references(line) {
                if static_dir.join(&reference).is_file() {
                    continue;
                }
                violations.push(format!(
                    "asset-reference: {}:{} references /static/{reference} — no such file under crates/foundry-app/static/ (a renamed or deleted blob left this reference dangling; assets.md:93-94 / ADR-CANZAN-THEME-003 R1)",
                    rel(root, &file),
                    index + 1
                ));
            }
        }
    }
    violations
}

/// Extract every `/static/<path>` token on `line` that carries a file
/// extension. A path runs over `[A-Za-z0-9._/-]` and stops at the first other
/// byte, so a quote, `)`, `#` or whitespace terminates it.
fn static_references(line: &str) -> Vec<String> {
    const MARKER: &str = "/static/";
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut cursor = 0;
    while let Some(found) = line[cursor..].find(MARKER) {
        let start = cursor + found + MARKER.len();
        let mut end = start;
        while end < bytes.len() && is_path_byte(bytes[end]) {
            end += 1;
        }
        cursor = start.max(cursor + found + 1);
        let candidate = &line[start..end];
        if has_file_extension(candidate) {
            out.push(candidate.to_string());
        }
    }
    out
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/')
}

/// True when the final path segment ends in `.<2-12 alphanumerics>`. Rejects
/// the deliberate prefix literal `css/foundry.` and any path with no dot.
fn has_file_extension(candidate: &str) -> bool {
    let Some(segment) = candidate.rsplit('/').next() else {
        return false;
    };
    let Some((stem, extension)) = segment.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && (2..=12).contains(&extension.len())
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
}

/// R2 — every `<stem>.<8 lowercase hex>.<ext>` file hashes to its own name.
fn check_hashed_filenames_are_honest(root: &Path, static_dir: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for file in files_with_extensions(static_dir, &[]) {
        let Some(name) = file.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(claimed) = content_hash_segment(name) else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let actual = sha256_hex(&bytes);
        if actual.starts_with(&claimed) {
            continue;
        }
        violations.push(format!(
            "asset-hash: {} claims content hash `{claimed}` but its bytes hash to `{}` — the blob was edited without being renamed, so every browser holding the old bytes at this `Cache-Control: immutable` URL is pinned stale (ADR-CANZAN-THEME-003 R2)",
            rel(root, &file),
            &actual[..8]
        ));
    }
    violations
}

/// The middle `<8 lowercase hex>` segment of `<stem>.<hash>.<ext>`, if any.
fn content_hash_segment(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let candidate = parts[parts.len() - 2];
    let is_hash = candidate.len() == 8
        && candidate
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    is_hash.then(|| candidate.to_string())
}

/// R3 — every `VENDOR.md` provenance row's recorded sha256 recomputes.
///
/// Parses only pipe-delimited TABLE rows whose first cell is a backticked path
/// with a file extension and whose last cell is a backticked 64-hex digest.
/// Header, separator and prose lines carry neither and are skipped; so are
/// ADR-CANZAN-THEME-002's per-blob derived-recipe blocks, which record input
/// and intermediate hashes for artefacts that are NOT committed files.
fn check_vendor_rows_recompute(root: &Path, static_dir: &Path) -> Vec<String> {
    let vendor_md = static_dir.join("VENDOR.md");
    let Ok(contents) = std::fs::read_to_string(&vendor_md) else {
        return Vec::new();
    };
    let label = rel(root, &vendor_md);
    let mut violations = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let Some((asset, recorded)) = vendor_row(line) else {
            continue;
        };
        let blob = static_dir.join(&asset);
        let Ok(bytes) = std::fs::read(&blob) else {
            violations.push(format!(
                "asset-provenance: {label}:{} records a sha256 for `{asset}`, but no such file exists under crates/foundry-app/static/ (ADR-CANZAN-THEME-003 R3)",
                index + 1
            ));
            continue;
        };
        let actual = sha256_hex(&bytes);
        if actual == recorded {
            continue;
        }
        violations.push(format!(
            "asset-provenance: {label}:{} records sha256 `{recorded}` for `{asset}`, but the committed bytes hash to `{actual}` — VENDOR.md's Tier 1 integrity claim is false for this row (ADR-CANZAN-THEME-002 / ADR-CANZAN-THEME-003 R3)",
            index + 1
        ));
    }
    violations
}

/// `(asset path relative to static/, recorded lowercase sha256)` for a
/// provenance table row; `None` for headers, separators and prose.
fn vendor_row(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    let cells: Vec<&str> = trimmed
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    if cells.len() < 2 {
        return None;
    }
    let asset = backticked(cells[0]).filter(|cell| has_file_extension(cell))?;
    let recorded = cells
        .iter()
        .rev()
        .find_map(|cell| backticked(cell).filter(|value| is_sha256_hex(value)))?;
    Some((asset, recorded))
}

/// The contents of a cell that is exactly one backtick-quoted token.
fn backticked(cell: &str) -> Option<String> {
    let inner = cell.strip_prefix('`')?.strip_suffix('`')?;
    (!inner.is_empty() && !inner.contains('`')).then(|| inner.to_string())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Lowercase hex sha256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Enumerate files under `dir` (recursively) whose extension is in
/// `extensions`; an EMPTY `extensions` means every file. `rust_sources` walks
/// `*.rs` only, so R1's `.html` / `.css` / `.webmanifest` scan and R2's
/// walk-everything need this generalised form.
fn files_with_extensions(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let matches = extensions.is_empty()
                || path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| extensions.contains(&e))
                    .unwrap_or(false);
            if matches {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Enumerate `*.rs` files under `dir` (recursively). Empty if `dir` is absent.
fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Strip a trailing `//` line comment so a doc-comment mention of `Html(` or
/// `is_team_member` (this file's own design prose, or foundry-api's doc
/// comments) is not flagged as a code construction. A `//` inside a string
/// literal is rare in this codebase's handlers; the guard errs toward NOT
/// flagging commentary, which the gold test compensates for by planting REAL
/// code violations.
fn strip_comment(line: &str) -> String {
    match line.find("//") {
        Some(idx) => line[..idx].to_string(),
        None => line.to_string(),
    }
}

/// A human label for the offending file — the file stem (e.g. `lib`, `issues`)
/// which names the handler module the maintainer must inspect.
fn handler_label(file: &Path) -> String {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("foundry-api handler");
    format!("foundry-api::{stem}")
}

/// Path relative to `root` for compact output.
fn rel(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string()
}

// ---- check_publish_workflows_stamp_the_image — US-RVF-02 AC-8 --------------
//
// The container build context has no `.git` (`.dockerignore`), so build.rs can
// only stamp a published image with the commit it was built from if the publish
// workflow hands that commit in. An image that silently reads `unknown` in
// production is the failure this rule exists to prevent (DDD-13), as is an ARG
// declared where build.rs never sees it (DDD-11).

/// The two workflows that publish an image. Hand-listed on purpose: each is a
/// publish path that must stamp, and a renamed or deleted one fails closed.
const STAMPED_PUBLISH_WORKFLOWS: [&str; 2] = [
    ".forgejo/workflows/build-and-publish.yml",
    ".github/workflows/release.yml",
];

/// The stamp inputs build.rs reads; each must be a builder-stage `ARG`.
const STAMP_INPUTS: [&str; 2] = ["FOUNDRY_STAMP_SHA", "FOUNDRY_STAMP_DATE"];

/// The DDD-13 refusal phrase; an `exit 1` must follow it inside its `if`.
const EMPTY_STAMP_REFUSAL: &str = "build stamp is empty";

/// AC-8. Both publish workflows compute the stamp from the commit (DDD-3),
/// refuse to publish on an empty value and pass both build-args; the Dockerfile
/// declares both inputs in the builder stage immediately before the cargo
/// build. A missing or unreadable file is a violation, never a pass.
fn check_publish_workflows_stamp_the_image(root: &Path) -> Vec<String> {
    let mut violations: Vec<String> = STAMPED_PUBLISH_WORKFLOWS
        .iter()
        .flat_map(|workflow| publish_workflow_stamp_violations(root, workflow))
        .collect();
    violations.extend(dockerfile_stamp_violations(root));
    violations
}

/// The DDD-13 violations in one publish workflow. YAML / shell comment lines
/// are ignored, so a commented-out command cannot satisfy the rule.
fn publish_workflow_stamp_violations(root: &Path, workflow: &str) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(root.join(workflow)) else {
        return vec![format!(
            "publish-stamp: cannot read {workflow} — it could not be checked for the build \
             stamp; an image it publishes may read `unknown` (DDD-13)."
        )];
    };
    let lines: Vec<(usize, &str)> = source
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, line)| !line.starts_with('#'))
        .collect();
    let has = |needle: &str| lines.iter().any(|(_, line)| line.contains(needle));

    let mut violations = Vec::new();
    for (needle, what) in [
        (
            "git rev-parse --short=7 HEAD",
            "never computes the stamp SHA with `git rev-parse --short=7 HEAD`",
        ),
        (
            "git log -1 --format=%cd --date=short",
            "never computes the stamp date from the commit with \
             `git log -1 --format=%cd --date=short`",
        ),
        ("build-args:", "passes no `build-args:` to the image build"),
        (
            "FOUNDRY_STAMP_SHA=",
            "passes no `FOUNDRY_STAMP_SHA=` build-arg",
        ),
        (
            "FOUNDRY_STAMP_DATE=",
            "passes no `FOUNDRY_STAMP_DATE=` build-arg",
        ),
    ] {
        if !has(needle) {
            violations.push(format!(
                "publish-stamp: {workflow} {what} — the published image would read `unknown` \
                 or the wrong commit (DDD-3/DDD-13)."
            ));
        }
    }
    if !refuses_an_empty_stamp(&lines) {
        violations.push(format!(
            "publish-stamp: {workflow} does not refuse an empty stamp (`{EMPTY_STAMP_REFUSAL}` \
             followed by `exit 1`) — it could publish an image whose footer reads `unknown` \
             (DDD-13)."
        ));
    }
    for (line_number, line) in &lines {
        if computes_the_build_time(line) {
            violations.push(format!(
                "publish-stamp: {workflow}:{line_number} computes a date from the build time \
                 (`date`) — the stamp date is the commit's own date \
                 (`git log -1 --format=%cd --date=short`, DDD-3)."
            ));
        }
    }
    violations
}

/// Whether the refusal phrase is followed by `exit 1` before its `if` closes.
fn refuses_an_empty_stamp(lines: &[(usize, &str)]) -> bool {
    lines
        .iter()
        .enumerate()
        .filter(|(_, (_, line))| line.contains(EMPTY_STAMP_REFUSAL))
        .any(|(index, _)| {
            lines[index + 1..]
                .iter()
                .map(|(_, line)| *line)
                .take_while(|line| *line != "fi")
                .any(|line| line == "exit 1")
        })
}

/// Whether a line runs the `date` command (`$(date …)`, `` `date …` ``, or
/// `date …` as a command), i.e. stamps the build time instead of the commit.
fn computes_the_build_time(line: &str) -> bool {
    line.contains("$(date") || line.contains("`date") || line.starts_with("date ")
}

/// The DDD-11 violations in the Dockerfile: each stamp input is an `ARG` in
/// the `builder` stage, positioned after every other instruction that precedes
/// the cargo build (so no earlier cached layer is invalidated by a new commit),
/// and declared in no other stage (where build.rs would never see it).
fn dockerfile_stamp_violations(root: &Path) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(root.join("Dockerfile")) else {
        return vec![
            "publish-stamp: cannot read Dockerfile — it could not be checked for the \
             FOUNDRY_STAMP_* build inputs (DDD-11)."
                .to_string(),
        ];
    };
    let instructions = dockerfile_instructions(&source);
    let Some(builder) = instructions
        .iter()
        .position(|i| i.keyword == "FROM" && i.text.to_lowercase().ends_with(" as builder"))
    else {
        return vec![
            "publish-stamp: Dockerfile has no `AS builder` stage — the stamp inputs cannot \
             reach build.rs (DDD-11)."
                .to_string(),
        ];
    };
    let stage_end = instructions[builder + 1..]
        .iter()
        .position(|i| i.keyword == "FROM")
        .map_or(instructions.len(), |offset| builder + 1 + offset);
    let Some(cargo_build) = (builder + 1..stage_end).find(|&index| {
        instructions[index].keyword == "RUN" && instructions[index].text.contains("cargo build")
    }) else {
        return vec![
            "publish-stamp: Dockerfile's builder stage has no `RUN … cargo build` — the stamp \
             inputs cannot reach build.rs (DDD-11)."
                .to_string(),
        ];
    };
    // The ARGs immediately before the cargo build: the run of ARG
    // instructions that ends right at it.
    let adjacent_start = (builder + 1..cargo_build)
        .rev()
        .take_while(|&index| instructions[index].keyword == "ARG")
        .last()
        .unwrap_or(cargo_build);

    let mut violations = Vec::new();
    for input in STAMP_INPUTS {
        let declares = |index: &usize| instructions[*index].declares_arg(input);
        if !(adjacent_start..cargo_build).any(|index| declares(&index)) {
            violations.push(format!(
                "publish-stamp: Dockerfile does not declare `ARG {input}=` in the builder stage \
                 immediately before the `cargo build` RUN (line {}) — build.rs would never see \
                 the stamp, or an earlier cached layer would be busted (DDD-11).",
                instructions[cargo_build].line
            ));
        }
        for index in (0..instructions.len()).filter(|index| !(builder..stage_end).contains(index)) {
            if declares(&index) {
                violations.push(format!(
                    "publish-stamp: Dockerfile:{} declares `ARG {input}` outside the builder \
                     stage — build.rs runs in the builder and never sees it (DDD-11).",
                    instructions[index].line
                ));
            }
        }
    }
    violations
}

/// One Dockerfile instruction: its first line, upper-cased keyword and the
/// full text with `\` continuations joined.
struct DockerInstruction {
    line: usize,
    keyword: String,
    text: String,
}

impl DockerInstruction {
    /// Whether this is `ARG <name>` or `ARG <name>=<default>`.
    fn declares_arg(&self, name: &str) -> bool {
        self.keyword == "ARG"
            && self
                .text
                .split_whitespace()
                .nth(1)
                .is_some_and(|declared| declared.split('=').next() == Some(name))
    }
}

/// The Dockerfile's instructions in order. Comment and blank lines are
/// skipped; a line ending in `\` continues onto the next.
fn dockerfile_instructions(source: &str) -> Vec<DockerInstruction> {
    let mut instructions: Vec<DockerInstruction> = Vec::new();
    let mut continuing = false;
    for (index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (body, continues) = match line.strip_suffix('\\') {
            Some(body) => (body.trim_end(), true),
            None => (line, false),
        };
        match instructions.last_mut() {
            Some(current) if continuing => {
                current.text.push(' ');
                current.text.push_str(body);
            }
            _ => instructions.push(DockerInstruction {
                line: index + 1,
                keyword: body
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_uppercase(),
                text: body.to_string(),
            }),
        }
        continuing = continues;
    }
    instructions
}

// ---- check_stylesheet_token_seam — S1/S2 (ADR-CANZAN-THEME-004) ------------
//
// ARMED IN TWO STEPS, DELIBERATELY. A rule armed against a subject that cannot
// yet satisfy it is a broken build, not a guard:
//
//   * S2 (`check_stylesheet_dark_block_parity`) IS ARMED as of step 02-01 —
//     the step that created the two dark regions. It treats a missing region as
//     a violation (it must, or it would pass vacuously on exactly the file it
//     exists to police), which is why it could not be armed before the regions
//     existed.
//   * S1 (`check_stylesheet_colour_seam`) IS ARMED as of step 03-01 — the step
//     that retired the last of the 46 colour literals that sat across 30 rules
//     below the seam (the dashboard's 21, the dialog's 2, the shortcut
//     overlay's 7, and the rail/board set 02-01 had already moved). Arming it
//     any earlier would have redded every commit in between.
//
// The gold tests are NOT deferred with the arming: they call these functions
// directly against staged planted-violation trees, so both rules are SHOWN to
// bite in the commit that introduced them rather than claimed to.

/// The three regions in which a colour value may appear — the `:root` token
/// block and the two dark blocks D-03 requires be written separately (a media
/// query and an attribute selector cannot express "either" in CSS).
///
/// Hardcoded per ADR-004 consequence (c): changing one of these selectors IS
/// changing the theming mechanism and should not be a quiet edit.
const SEAM_REGION_SELECTORS: [&str; 3] = [
    ":root",
    ":root:not([data-theme=\"light\"])",
    ":root[data-theme=\"dark\"]",
];

/// A `selector { … }` block located by brace matching, with byte offsets.
struct CssRegion {
    selector: String,
    /// Byte offset of the opening `{`.
    open: usize,
    /// Byte offset of the matching `}`.
    close: usize,
}

/// S1 — no colour beneath the seam (ADR-CANZAN-THEME-004).
///
/// After stripping `/* … */` block comments, reports every `#rgb` / `#rgba` /
/// `#rrggbb` / `#rrggbbaa` literal and every `rgb(` / `rgba(` / `hsl(` /
/// `hsla(` functional notation that lies outside the three token regions,
/// NAMING `file:line`.
///
/// Comment stripping is not optional: D-04 requires the six measured contrast
/// ratios recorded inline beside the tokens, so the file is dense with
/// comments and a scanner that ignored them would produce false positives on
/// the very discipline this feature introduces. Stripping preserves newlines,
/// so reported line numbers stay true to the original file.
///
/// Regions are located by their normalised selector text and delimited by
/// brace matching following the `block_end` IDIOM — deliberately WITHOUT
/// calling `block_end`, whose `!started && offset > 3` escape hatch exists for
/// a Rust `#[cfg(test)]` attribute on a non-block item and would silently
/// truncate a CSS region whose selector list runs more than three lines before
/// its `{`. `strip_comment` is likewise unusable: it strips `//` to
/// end-of-line, and CSS block comments span lines.
///
/// Four stated limits — this is a scanner, not a CSS parser (ADR-004
/// consequence):
///   (a) CSS NAMED colours (`white`, `rebeccapurple`) are not detected; a
///       blocklist of ~148 names is the staleness this design avoids
///       elsewhere, and the hole is covered downstream by the rendered sweep.
///       `transparent` / `currentColor` / `inherit` are intentionally allowed.
///   (b) Brace matching is textual and would be confused by a brace inside a
///       string or `content:` value; the file contains none, and unbalanced
///       braces are reported LOUDLY rather than swallowed.
///   (c) The three selectors are hardcoded (see [`SEAM_REGION_SELECTORS`]).
///   (d) An ID selector whose name is 3, 4, 6 or 8 hex characters (`#abc {`)
///       would be a false positive. The stylesheet uses none — it is styled
///       entirely by class and element — and a false positive fails loudly at
///       the author's next commit rather than hiding a colour.
fn check_stylesheet_colour_seam(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for path in served_stylesheets(root) {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = rel(root, &path);
        let stripped = strip_css_block_comments(&source);
        let (regions, balanced) = css_regions(&stripped);
        if !balanced {
            violations.push(format!(
                "token-seam S1: {file} — unbalanced braces; the seam regions cannot be \
                 delimited, so the colour scan is not trustworthy (ADR-CANZAN-THEME-004)"
            ));
            continue;
        }
        let seam: Vec<&CssRegion> = regions
            .iter()
            .filter(|region| SEAM_REGION_SELECTORS.contains(&region.selector.as_str()))
            .collect();
        let starts = line_starts(&stripped);
        for (offset, literal) in colour_literals(&stripped) {
            let inside_seam = seam
                .iter()
                .any(|region| offset > region.open && offset < region.close);
            if inside_seam {
                continue;
            }
            let line = line_of(&starts, offset);
            violations.push(format!(
                "token-seam S1: {file}:{line} — colour literal `{literal}` outside the \
                 token seam; name a --cz-* token instead (ADR-CANZAN-THEME-004 S1)"
            ));
        }
    }
    violations
}

/// S2 — the three token regions declare an identical colour-token set
/// (ADR-CANZAN-THEME-004).
///
/// D-03's duplication is mandatory and its failure mode is nasty: add a token
/// to `:root` and to `:root[data-theme="dark"]` but forget the media block, and
/// dark-by-toggle works perfectly while dark-by-device silently renders one
/// surface in the light value — invisible to the author, visible only to
/// system-dark operators, the persona the feature exists for. The blocks may
/// differ in VALUES and never in NAMES.
///
/// SCOPE (ADR-004 amendment of 2026-08-29): S2 compares the **colour-token
/// subset**, not every `--*` name. `component-boundaries.md` C1 gives `:root`
/// sole ownership of `--radius`, `--cz-gutter` and the three type tokens, which
/// the dark regions have no reason to redeclare; demanding `--radius: 6px`
/// verbatim in all three would be duplication with no failure mode behind it.
/// The reference set is therefore the names whose `:root` binding matches S1's
/// own colour detector; a dark region counts a name if it is in that reference
/// set OR is itself bound to a colour there (so a colour token introduced in a
/// dark block alone is still reported).
///
/// A stylesheet in which fewer than three regions are found is itself a
/// violation: without that, S2 would pass vacuously against a file that has no
/// dark blocks yet — which is exactly today's file.
fn check_stylesheet_dark_block_parity(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for path in served_stylesheets(root) {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = rel(root, &path);
        let stripped = strip_css_block_comments(&source);
        let (regions, balanced) = css_regions(&stripped);
        if !balanced {
            violations.push(format!(
                "token-seam S2: {file} — unbalanced braces; the three token regions \
                 cannot be delimited (ADR-CANZAN-THEME-004)"
            ));
            continue;
        }

        let mut located = Vec::new();
        for selector in SEAM_REGION_SELECTORS {
            match regions.iter().find(|region| region.selector == selector) {
                Some(region) => located.push(region),
                None => violations.push(format!(
                    "token-seam S2: {file} — token region `{selector}` not found; the \
                     colour palette must be bound in all three regions \
                     (ADR-CANZAN-THEME-004 S2)"
                )),
            }
        }
        if located.len() != SEAM_REGION_SELECTORS.len() {
            continue;
        }

        let reference = colour_token_names(&stripped, located[0]);
        for region in &located[1..] {
            let declared = declared_names(&stripped, region);
            let colours = colour_token_names(&stripped, region);
            let selector = &region.selector;
            for token in &reference {
                if declared.contains(token) {
                    continue;
                }
                violations.push(format!(
                    "token-seam S2: {file} — colour token `{token}` is bound in `:root` \
                     but MISSING from `{selector}`; that surface renders its light value \
                     on that path only (ADR-CANZAN-THEME-004 S2)"
                ));
            }
            for token in &colours {
                if reference.contains(token) {
                    continue;
                }
                violations.push(format!(
                    "token-seam S2: {file} — colour token `{token}` is bound in \
                     `{selector}` but NOT in `:root`; every colour token is declared at \
                     the seam first (ADR-CANZAN-THEME-004 S2)"
                ));
            }
        }
    }
    violations
}

/// The hand-authored stylesheets served from `foundry-app` — the subject of
/// S1/S2. There is no build step (VENDOR.md DB6), so these are the real files.
fn served_stylesheets(root: &Path) -> Vec<PathBuf> {
    let css_dir = root
        .join("crates")
        .join("foundry-app")
        .join("static")
        .join("css");
    files_with_extensions(&css_dir, &["css"])
}

/// Replace every `/* … */` block comment with equivalent whitespace, PRESERVING
/// newlines so byte offsets and line numbers still map onto the original file.
/// Unlike [`strip_comment`], which handles `//` to end-of-line only, this
/// spans lines — which is what CSS needs. An unterminated comment runs to EOF.
fn strip_css_block_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut idx = 0usize;
    let mut in_comment = false;
    while idx < bytes.len() {
        if !in_comment && bytes[idx] == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            in_comment = true;
            out.push_str("  ");
            idx += 2;
            continue;
        }
        if in_comment && bytes[idx] == b'*' && bytes.get(idx + 1) == Some(&b'/') {
            in_comment = false;
            out.push_str("  ");
            idx += 2;
            continue;
        }
        let ch = bytes[idx] as char;
        match (in_comment, ch) {
            (true, '\n') => out.push('\n'),
            (true, _) => out.push(' '),
            (false, _) => out.push(ch),
        }
        idx += 1;
    }
    out
}

/// Every `selector { … }` block in `stripped`, nested blocks included, with the
/// selector text normalised (whitespace collapsed, `'` folded to `"`). The
/// boolean is `false` when the braces do not balance — S1/S2 then report
/// loudly rather than scanning a tree they cannot delimit.
///
/// Brace matching follows the `block_end` idiom and NOT the function: its
/// `!started && offset > 3` lookahead escape is a Rust `#[cfg(test)]`
/// heuristic that would truncate a CSS region whose selector list runs more
/// than three lines before its `{`.
fn css_regions(stripped: &str) -> (Vec<CssRegion>, bool) {
    let bytes = stripped.as_bytes();
    let mut regions = Vec::new();
    let mut boundary = 0usize;
    let mut depth: i32 = 0;
    for idx in 0..bytes.len() {
        match bytes[idx] {
            b'{' => {
                depth += 1;
                if let Some(close) = matching_brace(bytes, idx) {
                    regions.push(CssRegion {
                        selector: normalize_selector(&stripped[boundary..idx]),
                        open: idx,
                        close,
                    });
                }
                boundary = idx + 1;
            }
            b'}' => {
                depth -= 1;
                boundary = idx + 1;
            }
            b';' => boundary = idx + 1,
            _ => {}
        }
        if depth < 0 {
            return (regions, false);
        }
    }
    (regions, depth == 0)
}

/// The offset of the bracket closing the `{`, `(` or `[` at `open`, or `None`
/// if it never does.
fn matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let (opener, closer) = match bytes.get(open) {
        Some(b'(') => (b'(', b')'),
        Some(b'[') => (b'[', b']'),
        _ => (b'{', b'}'),
    };
    let mut depth: i32 = 0;
    for (offset, &byte) in bytes[open..].iter().enumerate() {
        if byte == opener {
            depth += 1;
        } else if byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(open + offset);
            }
        }
    }
    None
}

/// Collapse whitespace runs to one space, fold `'` to `"`, trim — so a
/// selector split across lines still matches [`SEAM_REGION_SELECTORS`].
fn normalize_selector(raw: &str) -> String {
    let folded: String = raw
        .chars()
        .map(|ch| if ch == '\'' { '"' } else { ch })
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every colour literal in `css` as `(byte offset, text)`: `#` followed by
/// exactly 3, 4, 6 or 8 hex digits (and no further identifier character), or a
/// `rgb(` / `rgba(` / `hsl(` / `hsla(` opener not preceded by an identifier
/// character. Function names are matched case-insensitively; CSS is.
fn colour_literals(css: &str) -> Vec<(usize, String)> {
    const FUNCTIONS: [&str; 4] = ["rgba", "hsla", "rgb", "hsl"];
    let bytes = css.as_bytes();
    let lower = css.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let mut out = Vec::new();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'#' {
            let start = idx + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            let width = end - start;
            let trailing_identifier = bytes.get(end).is_some_and(|byte| is_identifier_byte(*byte));
            if matches!(width, 3 | 4 | 6 | 8) && !trailing_identifier {
                out.push((idx, css[idx..end].to_string()));
            }
            idx = end.max(idx + 1);
            continue;
        }
        let preceded_by_identifier = idx > 0 && is_identifier_byte(bytes[idx - 1]);
        if !preceded_by_identifier {
            let opener = FUNCTIONS.iter().find(|name| {
                let end = idx + name.len();
                lower_bytes.get(idx..end) == Some(name.as_bytes())
                    && lower_bytes.get(end) == Some(&b'(')
            });
            if let Some(name) = opener {
                out.push((idx, format!("{name}(")));
                idx += name.len();
                continue;
            }
        }
        idx += 1;
    }
    out
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// The `--*` custom-property names bound directly in `region`'s body.
///
/// Declarations are split on `;` and on the FIRST `:` — adequate because the
/// three seam regions contain value bindings only, never nested blocks.
fn declared_names(stripped: &str, region: &CssRegion) -> Vec<String> {
    stripped[region.open + 1..region.close]
        .split(';')
        .filter_map(|declaration| {
            let (name, _) = declaration.split_once(':')?;
            let name = name.trim();
            name.starts_with("--").then(|| name.to_string())
        })
        .collect()
}

/// The subset of [`declared_names`] whose VALUE is a colour literal — S2's
/// scope per the ADR-004 amendment.
fn colour_token_names(stripped: &str, region: &CssRegion) -> Vec<String> {
    stripped[region.open + 1..region.close]
        .split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            let name = name.trim();
            let is_colour = !colour_literals(value).is_empty();
            (name.starts_with("--") && is_colour).then(|| name.to_string())
        })
        .collect()
}

/// Byte offsets at which each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter(|(_, byte)| *byte == b'\n')
            .map(|(idx, _)| idx + 1),
    );
    starts
}

/// The 1-based line number containing byte `offset`.
fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|start| *start <= offset)
}

#[cfg(test)]
mod tests {
    //! Port-to-port unit tests for the AST detectors. Each detector's public
    //! behaviour is exercised through `run`-equivalent helpers operating on a
    //! staged fixture tree (the function signature IS the driving port).
    //!
    //! Behaviour budget: 3 AST detector behaviours (api≠HTML, api≠authz,
    //! alg-pin), each with a clean/violating pair = within 2× budget. Authored
    //! as 3 parametrized-style tests (clean+planted per detector) plus the
    //! alg-pin helper's equivalence classes.

    use super::*;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    /// The shipped 0015 shape — DEFERRABLE present. Must NOT be flagged.
    const LANES_MIGRATION_DEFERRABLE: &str = "CREATE TABLE lanes (\n\
    id UUID PRIMARY KEY,\n\
    position INTEGER NOT NULL CHECK (position >= 0),\n\
    UNIQUE (project_id, slug),\n\
    UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE\n);\n";

    fn stage_lanes_migration(sql: &str) -> tempfile::TempDir {
        stage(&[(
            "crates/foundry-store/migrations/0015_project_lanes.sql",
            sql,
        )])
    }

    #[test]
    fn deferrable_position_constraint_is_accepted() {
        let tree = stage_lanes_migration(LANES_MIGRATION_DEFERRABLE);
        assert!(
            check_lane_position_deferrable(tree.path()).is_empty(),
            "the shipped 0015 shape declares DEFERRABLE and must not be flagged"
        );
    }

    #[test]
    fn dropping_deferrable_is_flagged() {
        // The exact silent regression this rule exists for: a "tidy-up"
        // migration that drops one keyword. Lane insert (ADR-BOARD-LANE-003)
        // and lane move (ADR-BOARD-LANE-006) both break at RUNTIME while every
        // other test stays green.
        let tree = stage_lanes_migration(
            &LANES_MIGRATION_DEFERRABLE.replace(" DEFERRABLE INITIALLY IMMEDIATE", ""),
        );
        let violations = check_lane_position_deferrable(tree.path());
        assert_eq!(violations.len(), 1, "dropping DEFERRABLE must be flagged");
        assert!(
            violations[0].contains("no longer DEFERRABLE"),
            "the message must name what broke: {}",
            violations[0]
        );
    }

    #[test]
    fn dropping_the_position_constraint_entirely_is_flagged() {
        let tree = stage_lanes_migration(
            // NB: the const uses `\` line continuations, so Rust has already
            // stripped the leading indentation — match without it.
            &LANES_MIGRATION_DEFERRABLE.replace(
                "UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE\n",
                "",
            ),
        );
        let violations = check_lane_position_deferrable(tree.path());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].contains("no longer declares"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn a_commented_out_deferrable_does_not_satisfy_the_rule() {
        // Comments are stripped before matching, so a keyword surviving only
        // inside a comment must NOT count as present.
        let tree = stage_lanes_migration(
            "CREATE TABLE lanes (\n\
             -- UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE\n\
             UNIQUE (project_id, position)\n);\n",
        );
        let violations = check_lane_position_deferrable(tree.path());
        assert_eq!(
            violations.len(),
            1,
            "a commented-out keyword is not a live one"
        );
        assert!(
            violations[0].contains("no longer DEFERRABLE"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn a_missing_migration_is_flagged_rather_than_silently_passing() {
        let tree = stage(&[(
            "crates/foundry-store/migrations/0001_init.sql",
            "SELECT 1;\n",
        )]);
        assert_eq!(
            check_lane_position_deferrable(tree.path()).len(),
            1,
            "an unreadable 0015 must FAIL the guard, never pass it vacuously"
        );
    }

    // ---- DDD-22 / BR-4: no board-*.js registers a keydown listener --------

    const JS_DIR: &str = "crates/foundry-app/static/js";

    fn stage_js(files: &[(&str, &str)]) -> tempfile::TempDir {
        let staged: Vec<(String, &str)> = files
            .iter()
            .map(|(name, body)| (format!("{JS_DIR}/{name}"), *body))
            .collect();
        let borrowed: Vec<(&str, &str)> = staged.iter().map(|(p, b)| (p.as_str(), *b)).collect();
        stage(&borrowed)
    }

    fn shipped_js(name: &str) -> String {
        std::fs::read_to_string(workspace_root().join(JS_DIR).join(name))
            .unwrap_or_else(|e| panic!("read shipped {name}: {e}"))
    }

    #[test]
    fn shipped_board_modules_pass() {
        let dnd = shipped_js("board-dnd.js");
        let lane = shipped_js("board-lane-dnd.js");
        assert!(
            lane.contains("keydown"),
            "ANTI-VACUITY: board-lane-dnd.js is expected to mention `keydown` in a comment"
        );
        let tree = stage_js(&[("board-dnd.js", &dnd), ("board-lane-dnd.js", &lane)]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn a_keydown_listener_in_a_board_module_is_flagged() {
        let tree = stage_js(&[(
            "board-dnd.js",
            "const a = 1;\n\ndocument.addEventListener(\"keydown\", (e) => cancel(e));\n",
        )]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("board-dnd.js:3"),
            "{}",
            violations[0]
        );
        assert!(
            violations[0].contains("keyboard.js::closeTopLayer()"),
            "the message must name BR-4's single Escape owner: {}",
            violations[0]
        );
    }

    #[test]
    fn a_commented_out_keydown_listener_is_not_flagged() {
        let tree = stage_js(&[(
            "board-dnd.js",
            "// document.addEventListener('keydown', onKey);\nconst a = 1;\n",
        )]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn a_block_commented_keydown_listener_is_not_flagged() {
        let tree = stage_js(&[(
            "board-dnd.js",
            "/* legacy:\n   window.addEventListener(`keydown`, onKey); */\nconst a = 1;\n",
        )]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn an_onkeydown_assignment_is_flagged() {
        let tree = stage_js(&[("board-dnd.js", "card.onkeydown = handler;\n")]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
    }

    #[test]
    fn a_new_board_module_is_scanned() {
        let tree = stage_js(&[
            ("board-dnd.js", "const a = 1;\n"),
            ("board-foo.js", "board.on('keydown', cancel);\n"),
        ]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("board-foo.js:1"),
            "{}",
            violations[0]
        );
    }

    #[test]
    fn keyboard_js_is_out_of_scope() {
        let tree = stage_js(&[
            ("board-dnd.js", "const a = 1;\n"),
            (
                "keyboard.js",
                "document.addEventListener(\"keydown\", closeTopLayer);\n",
            ),
        ]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn the_word_keydown_in_a_string_is_not_a_listener() {
        let tree = stage_js(&[("board-dnd.js", "console.log(\"no keydown here\");\n")]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn a_missing_js_directory_is_flagged() {
        let tree = stage(&[("crates/foundry-app/src/main.rs", "fn main() {}\n")]);
        let violations = check_board_modules_have_no_keydown_listener(tree.path());
        assert_eq!(
            violations.len(),
            1,
            "a missing js directory must FAIL the guard, never pass it vacuously: {violations:?}"
        );
    }

    // ---- keycloak-sso D9 / OQ-8: nothing UPDATEs users.provisioned_at ------

    const MIGRATION_0017: &str = "crates/foundry-store/migrations/0017_users_provisioned_at.sql";

    fn assert_flagged_at(violations: &[String], location: &str) {
        assert!(
            violations
                .iter()
                .any(|v| v.contains(location) && v.contains("D9")),
            "expected a D9 violation naming {location}: {violations:?}"
        );
    }

    #[test]
    fn an_update_clearing_the_marker_in_a_rust_string_is_flagged() {
        let tree = stage(&[
            (
                "crates/foundry-store/src/lib.rs",
                "fn a() {}\n\nlet _ = sqlx::query(\"UPDATE users SET provisioned_at = NULL WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-services/src/deep/nested/reset.rs",
                "const Q: &str = \"update users set Provisioned_At = null where id = $1\";\n",
            ),
        ]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert_flagged_at(&violations, "crates/foundry-store/src/lib.rs:3");
        assert_flagged_at(
            &violations,
            "crates/foundry-services/src/deep/nested/reset.rs:1",
        );
    }

    #[test]
    fn a_multi_line_update_that_also_clears_the_marker_is_flagged() {
        let tree = stage(&[
            (
                "crates/foundry-store/src/lib.rs",
                "let _ = sqlx::query(\n    \"UPDATE users\n SET password_hash = $1, provisioned_at = NULL\n WHERE id = $2\",\n);\n",
            ),
            (
                "crates/foundry-store/src/raw.rs",
                "let _ = sqlx::query(r#\"UPDATE users\n   SET (display_name, provisioned_at) = ($1, NULL)\n WHERE id = $2\"#);\n",
            ),
        ]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert_flagged_at(&violations, "crates/foundry-store/src/lib.rs:3");
        assert_flagged_at(&violations, "crates/foundry-store/src/raw.rs:2");
    }

    #[test]
    fn the_same_update_inside_a_comment_is_not_flagged() {
        let tree = stage(&[(
            "crates/foundry-store/src/lib.rs",
            "// sqlx::query(\"UPDATE users\n// SET password_hash = $1, provisioned_at = NULL\")\n\
             /* legacy: sqlx::query(\"UPDATE users SET provisioned_at = NULL\") */\n\
             let _ = sqlx::query(\"UPDATE users SET password_hash = $1 -- , provisioned_at = NULL\n WHERE id = $2\");\n",
        )]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn a_new_migration_that_updates_the_marker_is_flagged_but_0017s_backfill_is_not() {
        let backfill = std::fs::read_to_string(workspace_root().join(MIGRATION_0017))
            .expect("read shipped migration 0017");
        assert!(
            backfill.contains("SET provisioned_at = created_at"),
            "ANTI-VACUITY: 0017 is expected to carry the sanctioned backfill UPDATE"
        );
        let tree = stage(&[
            (MIGRATION_0017, &backfill),
            (
                "crates/foundry-store/migrations/0018_reset_provenance.sql",
                "-- clear the marker on reset\n\
                 /* UPDATE users SET provisioned_at = NULL; */\n\
                 UPDATE users\n   SET provisioned_at = NULL\n WHERE password_hash IS NOT NULL;\n",
            ),
        ]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_flagged_at(
            &violations,
            "crates/foundry-store/migrations/0018_reset_provenance.sql:4",
        );
    }

    #[test]
    fn writing_the_marker_on_insert_and_reading_it_are_not_flagged() {
        let tree = stage(&[(
            "crates/foundry-store/src/lib.rs",
            "let _ = sqlx::query_as(\n\
             \"INSERT INTO users (id, email_lower, password_hash,\n\
                                 provisioned_at)\n\
                   VALUES ($1, $2, NULL, $3)\n\
              ON CONFLICT (email_lower) DO NOTHING\n\
                RETURNING id\",\n);\n\
             const READ: &str = \"SELECT (provisioned_at IS NOT NULL OR password_hash IS NULL) AS provisioned FROM users\";\n\
             const OTHER: &str = \"UPDATE users SET display_name = $1 WHERE provisioned_at = $2\";\n",
        )]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn integration_tests_may_seed_the_marker_directly() {
        let tree = stage(&[(
            "crates/foundry-store/tests/users_provisioned_at.rs",
            "sqlx::query(\"UPDATE users SET provisioned_at = $2 WHERE id = $1\");\n",
        )]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_is_flagged() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        assert_eq!(
            violations.len(),
            1,
            "a missing crates directory must FAIL the guard, never pass it vacuously: {violations:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_source_directory_is_flagged() {
        use std::os::unix::fs::PermissionsExt;
        let tree = stage(&[("crates/foundry-store/src/locked/lib.rs", "fn a() {}\n")]);
        let locked = tree.path().join("crates/foundry-store/src/locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
            .expect("lock dir");
        let violations = check_provisioned_marker_is_never_rewritten(tree.path());
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
            .expect("unlock dir");
        assert_eq!(
            violations.len(),
            1,
            "an unreadable directory must FAIL the guard: {violations:?}"
        );
    }

    #[test]
    fn the_shipped_tree_writes_the_marker_only_where_sanctioned() {
        let root = workspace_root();
        let store = std::fs::read_to_string(root.join("crates/foundry-store/src/lib.rs"))
            .expect("read shipped store");
        assert!(
            store.contains("provisioned_at"),
            "ANTI-VACUITY: the store is expected to name provisioned_at"
        );
        let violations = check_provisioned_marker_is_never_rewritten(&root);
        assert_eq!(violations.len(), 0, "{violations:?}");
    }

    #[test]
    fn api_html_construction_is_flagged_but_clean_json_is_not() {
        let clean = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "pub fn h() -> Json<Vec<u8>> { Json(vec![]) }\n// body_html is allowed inside JSON\n",
        )]);
        assert!(
            check_api_no_html(clean.path()).is_empty(),
            "a JSON handler (and a body_html doc comment) must NOT be flagged"
        );

        let planted = stage(&[(
            "crates/foundry-api/src/issues.rs",
            "pub fn h() -> Html<String> { Html(\"<p>nope</p>\".into()) }\n",
        )]);
        let found = check_api_no_html(planted.path());
        assert!(
            !found.is_empty() && found[0].contains("foundry-api::issues"),
            "an Html(..) return must be flagged and NAME the handler: {found:?}"
        );
    }

    #[test]
    fn api_adhoc_authz_is_flagged() {
        let planted = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "async fn h(s: &S) { let _ = s.is_team_member(t, u).await; }\n",
        )]);
        let found = check_api_no_adhoc_authz(planted.path());
        assert!(
            !found.is_empty() && found[0].contains("is_team_member"),
            "an is_team_member call site must be flagged: {found:?}"
        );
    }

    #[test]
    fn api_mint_surface_is_flagged_but_clean_read_delete_is_not() {
        // A clean foundry-api: a GET list + DELETE revoke on the tokens routes,
        // a doc-comment that NAMES mint_token (prose), and a POST on a DIFFERENT
        // (comments) route — none of which is a bearer mint surface.
        let clean = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "// the human /admin/tokens path calls Services::mint_token; the API never does\n\
             .route(\"/api/v1/teams/{t}/projects/{p}/tokens\", get(list_tokens_handler))\n\
             .route(\"/api/v1/teams/{t}/projects/{p}/tokens/{jti}\", delete(revoke_token_handler))\n\
             .route(\"/api/v1/teams/{t}/projects/{p}/issues/{n}/comments\", post(create_comment_handler))\n",
        )]);
        assert!(
            check_api_no_mint_route(clean.path()).is_empty(),
            "a GET/DELETE tokens surface, a mint_token DOC COMMENT, and a POST on a \
             non-tokens route must NOT be flagged: {:?}",
            check_api_no_mint_route(clean.path())
        );

        // The load-bearing violation: a foundry-api line CALLS Services::mint_token.
        let minting = stage(&[(
            "crates/foundry-api/src/tokens.rs",
            "async fn mint_handler(s: State<Services>) { let _ = s.mint_token(&signer, &p, input).await; }\n",
        )]);
        let found = check_api_no_mint_route(minting.path());
        assert!(
            !found.is_empty()
                && found[0].contains("foundry-api::tokens")
                && found[0].contains("tokens.rs:1")
                && found[0].contains("mint_token"),
            "a mint_token call site in foundry-api must be flagged and NAME file:line: {found:?}"
        );

        // Belt-and-braces: a POST registration on the .../tokens COLLECTION route.
        let posting = stage(&[(
            "crates/foundry-api/src/lib.rs",
            ".route(\"/api/v1/teams/{t}/projects/{p}/tokens\", get(list_tokens_handler).post(create_token_handler))\n",
        )]);
        let found = check_api_no_mint_route(posting.path());
        assert!(
            !found.is_empty() && found[0].contains("lib.rs:1"),
            "a post( registration on the .../tokens collection route must be flagged: {found:?}"
        );

        // Multi-line evasion: the same POST-on-the-tokens-collection split across
        // a multi-line axum `.route(..)` block — the `post(` and the `/tokens"`
        // collection literal land on DIFFERENT source lines. The detector must
        // bite the route BLOCK, not co-located lines, and NAME the offending line.
        let multiline = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "    Router::new()\n\
             \x20       .route(\n\
             \x20           \"/api/v1/teams/{t}/projects/{p}/tokens\",\n\
             \x20           get(list_tokens_handler).post(mint_handler),\n\
             \x20       )\n",
        )]);
        let found = check_api_no_mint_route(multiline.path());
        assert!(
            !found.is_empty() && found[0].contains("foundry-api::lib"),
            "a multi-line POST on the .../tokens collection route must be flagged: {found:?}"
        );

        // No false positive on the REAL router shape: a multi-line GET-only
        // tokens-collection route block plus a SEPARATE issues route block that
        // carries a `post(` (on a DIFFERENT, non-tokens literal) must NOT trip the
        // detector — the `post(` and the tokens literal live in distinct blocks.
        let real_shape = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "    Router::new()\n\
             \x20       .route(\n\
             \x20           \"/api/v1/teams/{t}/projects/{p}/issues\",\n\
             \x20           get(list_issues_handler).post(create_issue_handler),\n\
             \x20       )\n\
             \x20       .route(\n\
             \x20           \"/api/v1/teams/{t}/projects/{p}/tokens\",\n\
             \x20           get(list_tokens_handler),\n\
             \x20       )\n\
             \x20       .route(\n\
             \x20           \"/api/v1/teams/{t}/projects/{p}/tokens/{jti}\",\n\
             \x20           delete(revoke_token_handler),\n\
             \x20       )\n",
        )]);
        assert!(
            check_api_no_mint_route(real_shape.path()).is_empty(),
            "the real GET-tokens + DELETE-tokens/{{jti}} router (a post( only on the \
             issues block) must NOT be flagged: {:?}",
            check_api_no_mint_route(real_shape.path())
        );
    }

    #[test]
    fn app_tenant_scoping_flags_a_path_parsed_workspace_id_but_not_the_resolved_seam() {
        // CLEAN: the shipped idiom — a tenant-scoped store call fed the RESOLVED
        // acting workspace (`acting.workspace_id()` / `user.workspace_id`), never
        // a path-parsed id. Must NOT be flagged.
        let clean = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "let acting = user.acting_workspace();\n\
             let team = state.store.find_team_by_slug(acting.workspace_id(), &team_slug).await;\n\
             let att = state.store.list_users_in_workspace(user.workspace_id).await;\n",
        )]);
        assert!(
            check_app_tenant_scoping(clean.path()).is_empty(),
            "a tenant query scoped by the resolved acting workspace must NOT be flagged: {:?}",
            check_app_tenant_scoping(clean.path())
        );

        // PLANTED: a handler parses a workspace id straight from request input
        // and feeds it into a workspace-scoped store call — the "trust a
        // client-supplied workspace" footgun ADR-002 forbids. Must be flagged,
        // NAMING file:line.
        let planted = stage(&[(
            "crates/foundry-app/src/evil.rs",
            "let ws = uuid::Uuid::parse_str(&params.workspace_id).unwrap();\n\
             let row = state.store.list_users_in_workspace(ws).await;\n",
        )]);
        let found = check_app_tenant_scoping(planted.path());
        assert!(
            !found.is_empty() && found[0].contains("evil.rs") && found[0].contains(":2"),
            "a path-parsed workspace id fed to a tenant-scoped store call must be \
             flagged and NAME file:line: {found:?}"
        );

        // PLANTED (single-line evasion): the parse and the scoped call co-located.
        let inline = stage(&[(
            "crates/foundry-app/src/evil2.rs",
            "let row = store.find_team_in_workspace(t, uuid::Uuid::parse_str(&q.ws).unwrap()).await;\n",
        )]);
        let found = check_app_tenant_scoping(inline.path());
        assert!(
            !found.is_empty() && found[0].contains("evil2.rs:1"),
            "an inline parse-then-scope must be flagged and NAME file:line: {found:?}"
        );

        // ALLOW-LIST: the resolution seam itself + provisioning (ADR-004) may use
        // a literal/parsed id — those files are exempt so the guard does not
        // false-positive on the legitimately instance-scoped paths.
        let provisioning = stage(&[(
            "crates/foundry-app/src/signin.rs",
            "let ws = uuid::Uuid::parse_str(&claim.workspace_id).unwrap();\n\
             let m = state.store.find_membership_in_workspace(uid, ws).await;\n",
        )]);
        assert!(
            check_app_tenant_scoping(provisioning.path()).is_empty(),
            "the resolution/provisioning allow-list must exempt the seam: {:?}",
            check_app_tenant_scoping(provisioning.path())
        );
    }

    #[test]
    fn jwt_validation_must_pin_eddsa_only() {
        let pinned = stage(&[(
            "crates/foundry-auth/src/lib.rs",
            "let mut v = Validation::new(JwtAlgorithm::EdDSA);\nv.algorithms = vec![JwtAlgorithm::EdDSA];\n",
        )]);
        assert!(
            check_jwt_alg_pin(pinned.path()).is_empty(),
            "an EdDSA-only pin must pass"
        );

        let lost = stage(&[(
            "crates/foundry-auth/src/lib.rs",
            "let v = Validation::new(JwtAlgorithm::EdDSA);\n// no algorithms pin reassigned\n",
        )]);
        assert!(
            !check_jwt_alg_pin(lost.path()).is_empty(),
            "a Validation without an EdDSA-only algorithms pin must be flagged"
        );

        let widened = stage(&[(
            "crates/foundry-auth/src/lib.rs",
            "let mut v = Validation::new(JwtAlgorithm::EdDSA);\nv.algorithms = vec![JwtAlgorithm::EdDSA, JwtAlgorithm::HS256];\n",
        )]);
        assert!(
            !check_jwt_alg_pin(widened.path()).is_empty(),
            "an algorithms list that also admits HS256 must be flagged"
        );

        let disabled = stage(&[(
            "crates/foundry-auth/src/lib.rs",
            "let mut v = Validation::new(JwtAlgorithm::EdDSA);\nv.algorithms = vec![JwtAlgorithm::EdDSA];\nv.insecure_disable_signature_validation();\n",
        )]);
        assert!(
            !check_jwt_alg_pin(disabled.path()).is_empty(),
            "disabling signature validation must be flagged even with an EdDSA list"
        );
    }

    /// board-lane-management D8 (architecture-design.md §8): a static
    /// array/match enumerating lane slugs under foundry-app/foundry-api src
    /// is flagged; a lone hardcoded slug, the `humanize_state` fallback body,
    /// and `#[cfg(test)]` fixtures are NOT.
    #[test]
    fn static_lane_list_is_flagged_but_exemptions_are_not() {
        // CLEAN: a single column selector literal + lane-row-driven render.
        let clean = stage(&[(
            "crates/foundry-app/src/issues.rs",
            "let oob = format!(\"beforeend:[data-column='backlog']\");\n\
             for lane in board.lanes { render(lane.slug, lane.label); }\n",
        )]);
        assert!(
            check_no_static_lane_list(clean.path()).is_empty(),
            "lane-row-driven render with one hardcoded selector must NOT be flagged: {:?}",
            check_no_static_lane_list(clean.path())
        );

        // PLANTED: a reintroduced static lane array (the DEFAULT_COLUMNS shape).
        let array = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "const COLS: &[&str] = &[\"Backlog\", \"Todo\", \"In-Progress\", \"Done\"];\n",
        )]);
        let found = check_no_static_lane_list(array.path());
        assert!(
            !found.is_empty() && found[0].contains("projects.rs:1"),
            "a static lane array must be flagged and NAME file:line: {found:?}"
        );

        // PLANTED: a reintroduced label→slug match (the column_label_to_state
        // shape) in foundry-api, arms on separate lines.
        let arms = stage(&[(
            "crates/foundry-api/src/lib.rs",
            "fn to_state(label: &str) -> &str {\n\
             match label {\n\
             \"Backlog\" => \"backlog\",\n\
             \"Todo\" => \"todo\",\n\
             \"Done\" => \"done\",\n\
             _ => \"\",\n\
             }\n}\n",
        )]);
        assert!(
            !check_no_static_lane_list(arms.path()).is_empty(),
            "a multi-line lane match must be flagged: {:?}",
            check_no_static_lane_list(arms.path())
        );

        // EXEMPT: humanize_state's historical fallback body (comments.rs) and
        // a #[cfg(test)] fixture enumerating grandfather lanes.
        let exempt = stage(&[(
            "crates/foundry-app/src/comments.rs",
            "pub(crate) fn humanize_state(slug: &str) -> String {\n\
             match slug {\n\
             \"backlog\" => \"Backlog\".to_string(),\n\
             \"todo\" => \"Todo\".to_string(),\n\
             \"done\" => \"Done\".to_string(),\n\
             other => other.to_string(),\n\
             }\n}\n\
             #[cfg(test)]\n\
             mod tests {\n\
             fn lanes() -> Vec<(&'static str, &'static str)> {\n\
             vec![(\"backlog\", \"Backlog\"), (\"todo\", \"Todo\"), (\"done\", \"Done\")]\n\
             }\n}\n",
        )]);
        assert!(
            check_no_static_lane_list(exempt.path()).is_empty(),
            "the humanize_state fallback and #[cfg(test)] fixtures must NOT be flagged: {:?}",
            check_no_static_lane_list(exempt.path())
        );
    }

    /// The `#[cfg(test)]` region-skip must end EXACTLY at the block close
    /// (added at DELIVER Phase 5: mutation testing showed the brace-counting
    /// arithmetic in `block_end`/`lane_scan_mask` survived the coarse
    /// exemption fixture above). Four boundary fixtures:
    ///   1. a static lane list AFTER a long `#[cfg(test)]` mod IS flagged —
    ///      the mask may not overrun the closing brace;
    ///   2. a lane enumeration DEEP inside a `#[cfg(test)]` mod (beyond the
    ///      bare-attribute lookahead) and ON the block-closing line is NOT
    ///      flagged — the mask covers the whole block through its last line;
    ///   3. a single-line `#[cfg(test)]` item masks only itself and the scan
    ///      resumes on the very next line, where a planted list IS flagged;
    ///   4. a bare `#[cfg(test)]` attribute that never opens a block masks
    ///      the conservative 4-line lookahead — a lane list on the last
    ///      looked-ahead line is NOT flagged.
    #[test]
    fn cfg_test_masking_ends_exactly_at_the_block_close() {
        // 1. The mask may not overrun: production list after a LONG test mod.
        let after_long_mod = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "#[cfg(test)]\n\
             mod tests {\n\
             fn a() { let x = 1; }\n\
             fn b() { let y = 2; }\n\
             fn c() { let z = 3; }\n\
             fn d() { let w = 4; }\n\
             }\n\
             const COLS: &[&str] = &[\"Backlog\", \"Todo\", \"Done\"];\n",
        )]);
        let found = check_no_static_lane_list(after_long_mod.path());
        assert!(
            !found.is_empty(),
            "a lane list AFTER the #[cfg(test)] block must be flagged (the mask may not overrun to EOF): {found:?}"
        );

        // 2. The mask covers the whole block: lane fixture deep inside the
        //    mod, enumerated on the block-CLOSING line itself.
        let deep_fixture = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "#[cfg(test)]\n\
             mod tests {\n\
             fn setup() { let a = 1; }\n\
             fn more() { let b = 2; }\n\
             fn lanes() -> Vec<(&'static str, &'static str)> {\n\
             vec![(\"backlog\", \"Backlog\"), (\"todo\", \"Todo\"), (\"done\", \"Done\")] } }\n",
        )]);
        assert!(
            check_no_static_lane_list(deep_fixture.path()).is_empty(),
            "a lane fixture on the deep block-closing line must be masked: {:?}",
            check_no_static_lane_list(deep_fixture.path())
        );

        // 3. Single-line #[cfg(test)] item: the scan resumes on the NEXT line.
        let single_line = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "#[cfg(test)] mod t { fn x() {} }\n\
             const COLS: &[&str] = &[\"Backlog\", \"Todo\", \"Done\"];\n",
        )]);
        assert!(
            !check_no_static_lane_list(single_line.path()).is_empty(),
            "a lane list right after a SINGLE-LINE test item must be flagged"
        );

        // 4. Bare attribute, no block: the conservative lookahead masks
        //    exactly 4 following lines (the documented stop-after-lookahead).
        let bare_attribute = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "#[cfg(test)]\n\
             use a;\n\
             use b;\n\
             use c;\n\
             const COLS: &[&str] = &[\"Backlog\", \"Todo\", \"Done\"];\n",
        )]);
        assert!(
            check_no_static_lane_list(bare_attribute.path()).is_empty(),
            "the bare-attribute lookahead must mask its 4-line window: {:?}",
            check_no_static_lane_list(bare_attribute.path())
        );
    }

    /// ADR-PROJECT-RENAME-001: USING `foundry_core::slugify` (and mentioning
    /// `fn slugify(` in a comment) is clean; DEFINING `fn slugify(` anywhere
    /// under crates/foundry-app/src is flagged and NAMES the file.
    #[test]
    fn app_slugify_definition_is_flagged_but_core_use_is_not() {
        let clean = stage(&[(
            "crates/foundry-app/src/projects.rs",
            "// a doc mention of fn slugify( is fine\nlet slug = foundry_core::slugify(raw_name);\n",
        )]);
        assert!(
            check_app_no_slugify_definition(clean.path()).is_empty(),
            "calling foundry_core::slugify (or a comment mention) must NOT be flagged"
        );

        let planted = stage(&[(
            "crates/foundry-app/src/admin_tokens.rs",
            "fn slugify(input: &str) -> String { input.to_lowercase() }\n",
        )]);
        let found = check_app_no_slugify_definition(planted.path());
        assert!(
            !found.is_empty() && found[0].contains("admin_tokens.rs"),
            "a private fn slugify( definition must be flagged and NAME the file: {found:?}"
        );
    }

    // ---- static-asset integrity gold tests (R1/R2/R3, ADR-CANZAN-THEME-003) --
    //
    // Behaviour budget: 3 distinct behaviours (R1 dangling reference, R2
    // dishonest content-hash filename, R3 false VENDOR.md row) x 2 = 6 unit
    // tests permitted; 3 authored, each pairing a planted violation with the
    // clean-tree silence assertion.
    //
    // The two expected sha256 values below are INDEPENDENT ORACLES computed
    // with `shasum -a 256`, not with this module's hasher — so a wrong hash
    // algorithm or a wrong digest encoding fails these tests instead of
    // cancelling out (no circular verification).

    /// sha256 of `CLEAN_CSS` per `printf ':root{--canzan-x:1}\n' | shasum -a 256`.
    const CLEAN_CSS: &str = ":root{--canzan-x:1}\n";
    const CLEAN_CSS_SHA256: &str =
        "6fdd04abc6afc02c4f52d1d907d0df36b44d8071d7f3ee24b266123bdc9ab8a6";
    /// sha256 of `CLEAN_VENDOR_JS` per `printf 'vendored-bytes\n' | shasum -a 256`.
    const CLEAN_VENDOR_JS: &str = "vendored-bytes\n";
    const CLEAN_VENDOR_SHA256: &str =
        "2e36225b221d824141b90691558311a740f71b87df408be08d897d9ba27b8e8c";

    /// A tree that satisfies R1, R2 and R3. Deliberately includes a
    /// `#[cfg(test)]` region naming the hashed stylesheet (ADR-003 scope rule
    /// 2 — test blocks are NOT region-skipped) and a hash-agnostic prefix
    /// literal (scope rule 3 — no extension, therefore not checked).
    fn clean_asset_tree() -> tempfile::TempDir {
        stage(&[
            (
                "crates/foundry-app/templates/base.html",
                "<link rel=\"stylesheet\" href=\"/static/css/foundry.6fdd04ab.css\">\n\
                 <link rel=\"manifest\" href=\"/static/manifest.webmanifest\">\n\
                 <script src=\"/static/vendor/htmx.min.js\" defer></script>\n",
            ),
            ("crates/foundry-app/static/css/foundry.6fdd04ab.css", CLEAN_CSS),
            ("crates/foundry-app/static/vendor/htmx.min.js", CLEAN_VENDOR_JS),
            (
                "crates/foundry-app/static/manifest.webmanifest",
                "{ \"icons\": [{ \"src\": \"/static/icons/icon-192.png\" }] }\n",
            ),
            ("crates/foundry-app/static/icons/icon-192.png", "not-really-a-png\n"),
            (
                "crates/foundry-app/src/lib.rs",
                "#[cfg(test)]\nmod static_cache_policy_tests {\n    \
                 const CSS: &str = \"/static/css/foundry.6fdd04ab.css\";\n}\n",
            ),
            (
                "crates/foundry-app/src/projects.rs",
                "let css_link = r#\"href=\"/static/css/foundry.\"#;\n",
            ),
            (
                "crates/foundry-app/static/VENDOR.md",
                &format!(
                    "| File | Version | Upstream | Retrieved | sha256 |\n\
                     |------|---------|----------|-----------|--------|\n\
                     | `vendor/htmx.min.js` | 2.0.4 | https://example.invalid | 2026-06-04 | `{CLEAN_VENDOR_SHA256}` |\n\
                     | `css/foundry.6fdd04ab.css` | hand-authored | — | 2026-08-22 | `{CLEAN_CSS_SHA256}` |\n\
                     \n- Re-verify with `shasum -a 256 crates/foundry-app/static/vendor/<file>`.\n"
                ),
            ),
        ])
    }

    #[test]
    fn clean_asset_tree_is_silent() {
        let clean = clean_asset_tree();
        assert!(
            check_static_asset_integrity(clean.path()).is_empty(),
            "a tree whose references resolve, whose hashed name is honest and whose \
             VENDOR rows recompute must produce NO violations: {:?}",
            check_static_asset_integrity(clean.path())
        );
    }

    #[test]
    fn r1_flags_a_reference_left_dangling_by_a_rename() {
        let tree = clean_asset_tree();
        std::fs::rename(
            tree.path()
                .join("crates/foundry-app/static/css/foundry.6fdd04ab.css"),
            tree.path()
                .join("crates/foundry-app/static/css/foundry.deadbeef.css"),
        )
        .expect("rename the hashed stylesheet");

        let found = check_static_asset_integrity(tree.path());
        assert!(
            found.iter().any(|violation| {
                violation.contains("asset-reference")
                    && violation.contains("base.html")
                    && violation.contains("/static/css/foundry.6fdd04ab.css")
            }),
            "R1 must fire on the rename and NAME base.html plus the dangling path: {found:?}"
        );
    }

    #[test]
    fn r2_flags_a_hashed_blob_edited_without_being_renamed() {
        let tree = clean_asset_tree();
        let css = tree
            .path()
            .join("crates/foundry-app/static/css/foundry.6fdd04ab.css");
        std::fs::write(&css, format!("{CLEAN_CSS}!")).expect("append a byte");

        let found = check_static_asset_integrity(tree.path());
        assert!(
            found.iter().any(|violation| {
                violation.contains("asset-hash") && violation.contains("css/foundry.6fdd04ab.css")
            }),
            "R2 must fire when a content-hashed blob's bytes change without a rename \
             and NAME the file: {found:?}"
        );
    }

    #[test]
    fn r3_flags_a_vendor_row_whose_recorded_sha256_no_longer_recomputes() {
        let tree = clean_asset_tree();
        let vendor_md = tree.path().join("crates/foundry-app/static/VENDOR.md");
        let doctored = std::fs::read_to_string(&vendor_md)
            .expect("read VENDOR.md")
            .replace(
                CLEAN_VENDOR_SHA256,
                "0000000000000000000000000000000000000000000000000000000000000000",
            );
        std::fs::write(&vendor_md, doctored).expect("plant a wrong sha256");

        let found = check_static_asset_integrity(tree.path());
        assert!(
            found.iter().any(|violation| {
                violation.contains("asset-provenance")
                    && violation.contains("vendor/htmx.min.js")
                    && violation.contains("0000000000000000")
            }),
            "R3 must fire on a VENDOR.md row whose recorded sha256 no longer recomputes \
             and NAME the row: {found:?}"
        );
    }

    // ---- token-seam gold tests (S1/S2, ADR-CANZAN-THEME-004) ---------------
    //
    // Behaviour budget: 3 distinct behaviours — (1) S1 reports colour literals
    // outside the seam and only those (comments stripped first), (2) S2 reports
    // colour-token parity breaks across the three regions by name AND region,
    // (3) S2 refuses to pass vacuously on a stylesheet with fewer than three
    // regions — x2 = 6 unit tests permitted; 4 authored (2 injected-violation
    // gold tests + 2 exhaustively-generated properties).
    //
    // PROPERTY PARADIGM NOTE (`// bypass:` for the tool, not the paradigm):
    // the workspace vendors no `proptest`/`quickcheck` dependency and this step
    // may only touch `check_arch.rs`, so the two properties below are
    // EXHAUSTIVE over their generated space rather than randomly sampled —
    // 399 comment/literal interleavings and all 512 three-set combinations.
    // Exhaustion over a small space is a strictly stronger oracle than
    // sampling it, and needs no new crate.

    const THEME_CSS_REL: &str = "crates/foundry-app/static/css/foundry.abcd1234.css";
    const THEME_CSS_NAME: &str = "foundry.abcd1234.css";
    const MEDIA_REGION: &str = ":root:not([data-theme=\"light\"])";
    const ATTR_REGION: &str = ":root[data-theme=\"dark\"]";

    /// A stylesheet that satisfies S1 and S2. Deliberately dense with the
    /// comments D-04 mandates — an inline measured-contrast note BESIDE a token
    /// and, critically, a second one BELOW the seam plus a multi-line block
    /// comment naming a retired literal. A scanner that did not strip `/* … */`
    /// first would fire on all three; that is what makes comment-stripping
    /// exercised here rather than assumed.
    ///
    /// `--radius` lives in `:root` only (component-boundaries C1), and
    /// `--cz-accent` is bound with different NOTATION in the two dark regions:
    /// S2 compares names, never values.
    const CLEAN_THEME_CSS: &str = r#":root {
  --cz-surface: #fbfbf9;
  --cz-ink: #1a1a18;            /* body copy — 4.57:1 on #fbfbf9 */
  --cz-accent: rgb(36 82 201);
  --radius: 6px;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --cz-surface: #14161a;
    --cz-ink: #f2f2ef;
    --cz-accent: rgb(140 170 255);
  }
}

:root[data-theme="dark"] {
  --cz-surface: #14161a;
  --cz-ink: #f2f2ef;
  --cz-accent: #8caaff;
}

/*
 * The seam ends here. Every rule below names a token.
 * Historical note: this block once hardcoded #5b5bd6.
 */
.card {
  background: var(--cz-surface);
  color: var(--cz-ink);         /* 4.57:1 on #fbfbf9 */
  border-radius: var(--radius);
}
"#;

    fn stage_stylesheet(css: &str) -> tempfile::TempDir {
        stage(&[(THEME_CSS_REL, css)])
    }

    /// The 1-based line numbers S1 named, parsed back out of its messages, so
    /// the property can assert an EXACT line set rather than a count.
    fn reported_lines(violations: &[String]) -> Vec<usize> {
        let needle = format!("{THEME_CSS_NAME}:");
        let mut lines: Vec<usize> = violations
            .iter()
            .filter_map(|violation| {
                let idx = violation.find(&needle)? + needle.len();
                let digits: String = violation[idx..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                digits.parse().ok()
            })
            .collect();
        lines.sort_unstable();
        lines
    }

    /// GOLD (S1): a `color: #ff0000` planted in a rule BELOW the seam fires and
    /// names `file:line`; the clean tree — comments and all — stays silent.
    #[test]
    fn s1_flags_a_colour_literal_planted_below_the_seam_and_names_file_line() {
        let clean = stage_stylesheet(CLEAN_THEME_CSS);
        assert!(
            check_stylesheet_colour_seam(clean.path()).is_empty(),
            "a stylesheet whose only colour literals are inside the three token \
             regions (plus measured-contrast comments below the seam) must produce \
             NO violations: {:?}",
            check_stylesheet_colour_seam(clean.path())
        );

        // PLANTED: a literal in `.card`, below the seam.
        let planted = CLEAN_THEME_CSS.replace(
            "  border-radius: var(--radius);\n",
            "  border-radius: var(--radius);\n  color: #ff0000;\n",
        );
        assert!(planted != CLEAN_THEME_CSS, "the plant must actually apply");
        let expected_line = planted
            .lines()
            .position(|line| line.contains("#ff0000"))
            .expect("planted line")
            + 1;
        let tree = stage_stylesheet(&planted);

        let found = check_stylesheet_colour_seam(tree.path());
        assert!(
            found.iter().any(|violation| {
                violation.contains(&format!("{THEME_CSS_NAME}:{expected_line}"))
                    && violation.contains("#ff0000")
            }),
            "S1 must fire on a colour literal below the seam and NAME file:line \
             ({THEME_CSS_NAME}:{expected_line}): {found:?}"
        );
        assert_eq!(
            reported_lines(&found),
            vec![expected_line],
            "S1 must report the planted literal and NOTHING else — the four \
             measured-contrast / historical-note comments are not violations: {found:?}"
        );
    }

    /// GOLD (S2): deleting one colour token from the dark-BY-DEVICE region only
    /// — the defect an author testing with the toggle never sees — fires and
    /// names the token AND the region. Deleting the region outright fires too,
    /// so S2 cannot pass vacuously against a stylesheet with no dark blocks.
    #[test]
    fn s2_flags_a_token_deleted_from_the_dark_by_device_region_only() {
        let clean = stage_stylesheet(CLEAN_THEME_CSS);
        assert!(
            check_stylesheet_dark_block_parity(clean.path()).is_empty(),
            "three regions binding the same colour-token names (in different \
             notations, with `--radius` in `:root` alone per C1) must produce NO \
             violations: {:?}",
            check_stylesheet_dark_block_parity(clean.path())
        );

        // PLANTED: drop `--cz-accent` from the media block ONLY — the attribute
        // block keeps it, so dark-by-toggle looks perfect.
        let media_selector_line = CLEAN_THEME_CSS
            .lines()
            .position(|line| line.contains(MEDIA_REGION))
            .expect("media region");
        let deleted_line = CLEAN_THEME_CSS
            .lines()
            .enumerate()
            .position(|(index, line)| index > media_selector_line && line.contains("--cz-accent:"))
            .expect("the media block's accent binding");
        let doctored: String = CLEAN_THEME_CSS
            .lines()
            .enumerate()
            .filter(|(index, _)| *index != deleted_line)
            .map(|(_, line)| format!("{line}\n"))
            .collect();
        assert!(
            doctored.matches("--cz-accent").count() == 2,
            "exactly one of the three `--cz-accent` bindings must be deleted"
        );
        let tree = stage_stylesheet(&doctored);

        let found = check_stylesheet_dark_block_parity(tree.path());
        assert!(
            found.iter().any(|violation| {
                violation.contains("--cz-accent") && violation.contains(MEDIA_REGION)
            }),
            "S2 must fire on a token present in `:root` and in the attribute block \
             but missing from the media block, NAMING the token and the region: {found:?}"
        );
        assert!(
            !found
                .iter()
                .any(|violation| violation.contains(ATTR_REGION)),
            "S2 must not implicate the region that is still correct: {found:?}"
        );

        // A stylesheet with no dark-by-device region at all is itself a
        // violation — S2 may not pass vacuously on today's stylesheet.
        let no_dark_block = CLEAN_THEME_CSS
            .split("@media")
            .next()
            .expect("prefix")
            .to_string();
        let vacuous = stage_stylesheet(&no_dark_block);
        let found = check_stylesheet_dark_block_parity(vacuous.path());
        assert!(
            found
                .iter()
                .any(|violation| violation.contains(MEDIA_REGION)),
            "a stylesheet in which fewer than three regions are found is itself an \
             S2 violation, NAMING the missing region: {found:?}"
        );
    }

    /// PROPERTY (S1): for ANY interleaving of colour literals and block
    /// comments below the seam, strip-then-match reports EXACTLY the literals
    /// that lie outside comments — never one inside a comment, never one fewer.
    /// Exhaustive over all 1-, 2- and 3-piece sequences of seven segment kinds
    /// (7 + 49 + 343 = 399 documents).
    #[test]
    fn s1_reports_exactly_the_literals_outside_block_comments() {
        const SEAM: &str = ":root { --cz-ink: #1a1a18; }\n\
                            @media (prefers-color-scheme: dark) { :root:not([data-theme=\"light\"]) { --cz-ink: #f2f2ef; } }\n\
                            :root[data-theme=\"dark\"] { --cz-ink: #f2f2ef; }\n\
                            .probe {\n";
        /// (segment body, line offsets within the segment that MUST be reported)
        const PIECES: [(&str, &[usize]); 7] = [
            ("  color: #ff0000;\n", &[0]),
            ("  background: rgba(0, 0, 0, 0.5);\n", &[0]),
            ("  color: var(--cz-ink); /* was #ff0000 */\n", &[]),
            ("  /* rgb(1, 2, 3) lived here */\n", &[]),
            ("  /* multi\n   line #abc\n   comment */\n", &[]),
            ("  border: 1px solid var(--cz-ink);\n", &[]),
            ("  /* a */ color: #123456; /* b rgb( */\n", &[0]),
        ];

        let tree = tempfile::tempdir().expect("tempdir");
        let css_path = tree.path().join(THEME_CSS_REL);
        std::fs::create_dir_all(css_path.parent().unwrap()).expect("mkdir");

        let mut cases = 0usize;
        for length in 1..=3usize {
            let space = PIECES.len().pow(length as u32);
            for encoded in 0..space {
                // decode `encoded` as a base-PIECES.len() numeral of `length` digits
                let sequence: Vec<usize> = (0..length)
                    .map(|digit| encoded / PIECES.len().pow(digit as u32) % PIECES.len())
                    .collect();

                let mut document = String::from(SEAM);
                let mut line = SEAM.lines().count(); // lines already emitted
                let mut expected: Vec<usize> = Vec::new();
                for &choice in &sequence {
                    let (body, offsets) = PIECES[choice];
                    for offset in offsets {
                        expected.push(line + offset + 1);
                    }
                    line += body.lines().count();
                    document.push_str(body);
                }
                document.push_str("}\n");

                std::fs::write(&css_path, &document).expect("write");
                let found = check_stylesheet_colour_seam(tree.path());
                assert_eq!(
                    reported_lines(&found),
                    expected,
                    "S1 must report exactly the literals outside comments for \
                     sequence {sequence:?}\n--- document ---\n{document}--- got ---\n{found:?}"
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 7 + 49 + 343, "the generated space must be exhausted");
    }

    /// PROPERTY (S2): for ANY three colour-token name sets over a fixed
    /// universe, the rule is silent IFF the three sets are equal, and every
    /// element of a symmetric difference is reported by name AND by region.
    /// `--radius` is bound in `:root` and in the media block throughout and may
    /// NEVER be reported — S2 is scoped to the colour-token subset (ADR-004
    /// amendment, component-boundaries C1). Exhaustive over all 8x8x8 = 512
    /// combinations.
    #[test]
    fn s2_is_silent_iff_the_three_colour_token_sets_are_equal() {
        const TOKENS: [&str; 3] = ["--cz-surface", "--cz-ink", "--cz-accent"];
        const VALUES: [&str; 3] = ["#fbfbf9", "rgb(36 82 201)", "hsl(50 20% 96%)"];

        fn bindings(mask: usize, indent: &str) -> String {
            (0..TOKENS.len())
                .filter(|index| mask >> index & 1 == 1)
                .map(|index| format!("{indent}{}: {};\n", TOKENS[index], VALUES[index]))
                .collect()
        }

        let tree = tempfile::tempdir().expect("tempdir");
        let css_path = tree.path().join(THEME_CSS_REL);
        std::fs::create_dir_all(css_path.parent().unwrap()).expect("mkdir");

        for root_mask in 0..8usize {
            for media_mask in 0..8usize {
                for attr_mask in 0..8usize {
                    let document = format!(
                        ":root {{\n{}  --radius: 6px;\n}}\n\
                         @media (prefers-color-scheme: dark) {{\n  \
                         :root:not([data-theme=\"light\"]) {{\n{}    --radius: 6px;\n  }}\n}}\n\
                         :root[data-theme=\"dark\"] {{\n{}}}\n",
                        bindings(root_mask, "  "),
                        bindings(media_mask, "    "),
                        bindings(attr_mask, "  "),
                    );
                    std::fs::write(&css_path, &document).expect("write");
                    let found = check_stylesheet_dark_block_parity(tree.path());

                    let equal = media_mask == root_mask && attr_mask == root_mask;
                    assert_eq!(
                        found.is_empty(),
                        equal,
                        "S2 must be silent IFF the three colour-token sets are equal \
                         (root={root_mask:03b} media={media_mask:03b} attr={attr_mask:03b}): {found:?}"
                    );
                    assert!(
                        !found.iter().any(|violation| violation.contains("--radius")),
                        "`--radius` is `:root`-owned (C1) and outside S2's colour-token \
                         scope; it may never be reported: {found:?}"
                    );

                    for (mask, region) in [(media_mask, MEDIA_REGION), (attr_mask, ATTR_REGION)] {
                        for (index, token) in TOKENS.iter().enumerate() {
                            if root_mask >> index & 1 == mask >> index & 1 {
                                continue;
                            }
                            assert!(
                                found.iter().any(|violation| {
                                    violation.contains(token) && violation.contains(region)
                                }),
                                "every symmetric-difference element must be reported by \
                                 NAME ({token}) and by REGION ({region}) — \
                                 root={root_mask:03b} media={media_mask:03b} \
                                 attr={attr_mask:03b}: {found:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    // ---- the guard's own entry point ---------------------------------------
    //
    // WHY THESE EXIST. Until 2026-08-30 nothing in this suite called `run`.
    // `cargo mutants` reported `run -> ExitCode` replaced by
    // `Default::default()` as a SURVIVOR, and `ExitCode::default()` IS
    // `ExitCode::SUCCESS` — so the guard over eleven invariants could be
    // disarmed at its top level while every gold test stayed green, because
    // they all call the rule functions directly. Only a full `cargo xtask ci`
    // would have noticed.
    //
    // Behaviour budget: 3 distinct behaviours — (1) LAYER 1 aggregates EVERY
    // rule, (2) the verdict maps arguments and both layers onto a decision,
    // (3) `run` encodes that decision as the process exit code — x2 = 6 unit
    // tests permitted; 4 authored.

    /// `ExitCode` is opaque: it implements neither `PartialEq` nor any
    /// accessor, so the only way to observe what `run` actually returned is to
    /// compare its `Debug` rendering with that of a reference `ExitCode` built
    /// here. This compares rendering to rendering and never to a hardcoded
    /// string, so it survives any change to std's `Debug` format — provided
    /// distinct codes still render distinctly, which
    /// `exit_code_rendering_distinguishes_the_codes_the_guard_returns` asserts
    /// directly so this helper cannot go quietly vacuous.
    fn same_exit_code(actual: ExitCode, expected: ExitCode) -> bool {
        format!("{actual:?}") == format!("{expected:?}")
    }

    /// The helper above is only a real oracle while `Debug` separates the three
    /// codes `run` returns. Asserted, not assumed.
    #[test]
    fn exit_code_rendering_distinguishes_the_codes_the_guard_returns() {
        assert!(
            same_exit_code(ExitCode::default(), ExitCode::SUCCESS),
            "the survivor this file's `run` tests exist to kill is \
             `Default::default()`, which is only dangerous because it IS \
             SUCCESS — if that ever stops holding, those tests need rewriting"
        );
        for (left, right) in [(0u8, 1u8), (0, 2), (1, 2)] {
            assert!(
                !same_exit_code(ExitCode::from(left), ExitCode::from(right)),
                "exit codes {left} and {right} must render differently, or \
                 `same_exit_code` silently passes everything"
            );
        }
    }

    /// `run` is not a constant. Two argument shapes drive it to two DIFFERENT
    /// exit codes, neither of them SUCCESS, so no constant-returning body —
    /// `Default::default()` included — can satisfy both.
    ///
    /// The success branch is deliberately absent: `run` runs LAYER 2, which
    /// shells out to `cargo deny` against the target tree's `Cargo.toml`, and a
    /// staged fixture tree has none, so a "clean" tempdir cannot reach
    /// `Verdict::Passed`. That branch is covered hermetically at the `verdict`
    /// seam below, with LAYER 2 stubbed at its subprocess boundary. Stating the
    /// gap beats faking it.
    #[test]
    fn run_returns_a_different_exit_code_for_each_failure_it_reports() {
        assert!(
            same_exit_code(run(vec!["--root".to_string()]), ExitCode::from(2)),
            "`--root` with no directory argument is unusable input, which the \
             `cargo xtask ci` contract distinguishes from a violation: it must \
             exit 2"
        );

        let planted = every_layer_1_rule_violated_tree();
        assert!(
            same_exit_code(
                run(vec![
                    "--root".to_string(),
                    planted.path().display().to_string(),
                ]),
                ExitCode::from(1),
            ),
            "a tree carrying planted violations must exit 1 — a guard that \
             returns SUCCESS here is disarmed"
        );
    }

    /// One marker per LAYER 1 rule, in `run`'s call order. Every message a rule
    /// emits is prefixed with its own name, so a marker absent from the
    /// aggregate's output means that rule is no longer wired into the guard —
    /// the silent-disarm failure this whole block exists for.
    const LAYER_1_RULE_MARKERS: [&str; 15] = [
        "api≠HTML:",
        "api≠ad-hoc-authz:",
        "api≠mint:",
        "JWT alg pin:",
        "OIDC alg pin:",
        "tenant-scoping:",
        "single slugify:",
        "no-static-lane-list:",
        "no-board-keydown:",
        "provisioned-marker:",
        "publish-stamp:",
        "asset-reference:",
        "token-seam S1:",
        "token-seam S2:",
        "store-natural-key-scoped:",
    ];

    /// A tree that breaks every LAYER 1 rule at once. Each fixture body is the
    /// planted violation from that rule's own gold test above, so this tree
    /// tests AGGREGATION — that `run` still calls all twelve — and never
    /// re-tests detection.
    fn every_layer_1_rule_violated_tree() -> tempfile::TempDir {
        stage(&[
            (
                "crates/foundry-api/src/issues.rs",
                "pub fn h() -> Html<String> { Html(\"<p>nope</p>\".into()) }\n",
            ),
            (
                "crates/foundry-api/src/authz.rs",
                "async fn h(s: &S) { let _ = s.is_team_member(t, u).await; }\n",
            ),
            (
                "crates/foundry-api/src/tokens.rs",
                "async fn mint_handler(s: State<Services>) { let _ = s.mint_token(&signer, &p, input).await; }\n",
            ),
            (
                "crates/foundry-auth/src/lib.rs",
                "let v = Validation::new(JwtAlgorithm::EdDSA);\n",
            ),
            (
                "crates/foundry-oidc/src/lib.rs",
                "let v = Validation::new(JwtAlgorithm::RS256);\n",
            ),
            (
                "crates/foundry-app/src/evil.rs",
                "let ws = uuid::Uuid::parse_str(&params.workspace_id).unwrap();\n\
                 let row = state.store.list_users_in_workspace(ws).await;\n",
            ),
            (
                "crates/foundry-app/src/admin_tokens.rs",
                "fn slugify(input: &str) -> String { input.to_lowercase() }\n",
            ),
            (
                "crates/foundry-app/src/projects.rs",
                "const COLS: &[&str] = &[\"Backlog\", \"Todo\", \"In-Progress\", \"Done\"];\n",
            ),
            (
                "crates/foundry-app/templates/base.html",
                "<link rel=\"stylesheet\" href=\"/static/css/deleted-by-a-rename.css\">\n",
            ),
            (
                "crates/foundry-app/static/css/planted.css",
                ".card { color: #ff0000; }\n",
            ),
            (
                "crates/foundry-app/static/js/board-dnd.js",
                "document.addEventListener(\"keydown\", cancel);\n",
            ),
            (
                "crates/foundry-store/src/reset.rs",
                "let _ = sqlx::query(\"UPDATE users SET provisioned_at = NULL WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-store/src/natural_key.rs",
                "let _ = sqlx::query(\"SELECT id FROM projects WHERE key_prefix = $1\");\n",
            ),
        ])
    }

    /// LAYER 1 aggregates EVERY rule. A rule dropped from `source_violations`
    /// disarms exactly as much of the guard as the `Default::default()`
    /// survivor did, just more quietly — the eleven per-rule gold tests above
    /// would all still pass.
    #[test]
    fn layer_1_aggregates_every_rule() {
        let planted = every_layer_1_rule_violated_tree();
        let found = source_violations(planted.path());
        let unwired: Vec<&str> = LAYER_1_RULE_MARKERS
            .into_iter()
            .filter(|marker| !found.iter().any(|violation| violation.contains(marker)))
            .collect();
        assert!(
            unwired.is_empty(),
            "these LAYER 1 rules are no longer wired into the guard: {unwired:?} \
             (reported: {found:?})"
        );

        let root = workspace_root();
        assert!(
            root.join("crates").join("foundry-app").join("src").is_dir(),
            "ANTI-VACUITY. Every rule below scans a subdirectory of the root, so \
             a root resolving to nothing makes all eleven silently pass — the \
             same disarm as the `Default::default()` survivor, reached from the \
             other end. The clean assertion that follows only means something \
             once the root is known to hold the tree: {root:?}"
        );
        assert!(
            source_violations(&root).is_empty(),
            "the shipped tree must satisfy every LAYER 1 rule — the guard is \
             self-applied (Principle 12c): {:?}",
            source_violations(&root)
        );
    }

    /// The verdict maps arguments plus BOTH layers onto a decision, and each
    /// decision names its exit code. LAYER 2 is stubbed at its subprocess
    /// boundary (`cargo deny`) — the one driven adapter here — which is what
    /// makes the `Passed` branch reachable in a test at all.
    #[test]
    fn the_verdict_folds_both_layers_and_the_arguments_into_an_exit_code() {
        fn layer_2_silent(_: &Path) -> Option<String> {
            None
        }
        fn layer_2_rejects(_: &Path) -> Option<String> {
            Some("dependency-direction: cargo-deny rejected the crate graph".to_string())
        }

        // The "clean" tree must satisfy every rule, and one of them
        // (`check_lane_position_deferrable`) is deliberately FAIL-CLOSED on an
        // unreadable 0015: a guard that goes quiet when its subject is missing
        // is a guard that cannot be trusted, and
        // `a_missing_migration_is_flagged_rather_than_silently_passing` pins
        // that. So a clean tree carries a valid migration rather than none.
        // Likewise `check_board_modules_have_no_keydown_listener` fails closed
        // on a missing js directory, so the clean tree carries a clean one.
        // And `check_publish_workflows_stamp_the_image` fails closed on missing
        // publish workflows or Dockerfile, so it carries stamping ones.
        // `check_store_natural_key_scoped` fails closed on a missing
        // foundry-store src, so it carries a store with no query.
        let clean = stage(&[
            ("crates/foundry-store/src/lib.rs", "pub struct Store;\n"),
            (
                "crates/foundry-store/migrations/0015_project_lanes.sql",
                "CREATE TABLE lanes (\n  UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE\n);\n",
            ),
            (
                "crates/foundry-app/static/js/board-dnd.js",
                "export const noop = () => {};\n",
            ),
            (PUBLISH_WORKFLOWS[0], STAMPING_WORKFLOW),
            (PUBLISH_WORKFLOWS[1], STAMPING_WORKFLOW),
            ("Dockerfile", STAMPING_DOCKERFILE),
            // `check_api_openapi_operations` fails closed on a missing
            // description, so the clean tree carries one describing nothing.
            ("crates/foundry-api/openapi.json", "{\"paths\": {}}\n"),
            // `check_api_docs_page` fails closed on any missing page file, so
            // the clean tree carries a minimal page that keeps every clause.
            (
                "crates/foundry-app/src/api_docs.rs",
                "pub const API_DOCS_PATH: &str = \"/docs/api\";\n\
                 pub const API_DOCS_CSP: &str = \"script-src 'self'\";\n\
                 fn r() { Router::new().route(API_DOCS_PATH, get(show)) }\n",
            ),
            ("crates/foundry-app/templates/api_docs.html", "<div id=\"swagger-ui\"></div>\n"),
            (
                "crates/foundry-app/static/js/api-docs.js",
                "var D = \"/api/v1/openapi.json\";\n\
                 SwaggerUIBundle({ url: D, queryConfigEnabled: false, validatorUrl: \"none\", \
                 persistAuthorization: false, withCredentials: false });\n",
            ),
            ("crates/foundry-app/static/vendor/swagger-ui-0/swagger-ui.css", ".a{}\n"),
        ]);
        let clean_args = vec!["--root".to_string(), clean.path().display().to_string()];
        let planted = every_layer_1_rule_violated_tree();
        let planted_args = vec!["--root".to_string(), planted.path().display().to_string()];

        assert_eq!(
            verdict_with(&clean_args, layer_2_silent),
            Verdict::Passed,
            "a tree that breaks no rule in either layer passes"
        );
        assert_eq!(
            verdict_with(&clean_args, layer_2_silent).exit_code(),
            0,
            "a pass exits 0"
        );

        let layer_1 = verdict_with(&planted_args, layer_2_silent);
        assert!(
            matches!(&layer_1, Verdict::Violations(found) if !found.is_empty()),
            "LAYER 1 violations reach the verdict: {layer_1:?}"
        );
        assert_eq!(layer_1.exit_code(), 1, "a violation exits 1");

        let layer_2 = verdict_with(&clean_args, layer_2_rejects);
        assert!(
            matches!(&layer_2, Verdict::Violations(found)
                if found.iter().any(|v| v.contains("cargo-deny rejected"))),
            "LAYER 2's verdict must reach the exit code too — it is the half of \
             the guard that no gold test can stage: {layer_2:?}"
        );
        assert_eq!(layer_2.exit_code(), 1, "a LAYER 2 violation also exits 1");

        let unusable = verdict_with(&["--root".to_string()], layer_2_silent);
        assert!(
            matches!(&unusable, Verdict::UnparseableArguments(message)
                if message.contains("--root requires a directory argument")),
            "unusable arguments are their own verdict, not a violation: {unusable:?}"
        );
        assert_eq!(
            unusable.exit_code(),
            2,
            "unusable input exits 2, so `cargo xtask ci` can tell a misinvoked \
             guard from a failing one"
        );
    }

    // =======================================================================
    // release-version-footer US-RVF-02 AC-8 (DDD-11/DDD-13) — every published
    // image carries the commit it was built from.
    // =======================================================================
    //
    // Proposed by DISTILL 2026-10-04, accepted and implemented by DELIVER 02-02
    // as `check_publish_workflows_stamp_the_image`, wired into
    // `source_violations`.
    //
    // The rule, per file (each miss is one violation naming the file):
    //   * `.forgejo/workflows/build-and-publish.yml` and
    //     `.github/workflows/release.yml` each contain the two DDD-3 commands
    //     (`git rev-parse --short=7 HEAD`, `git log -1 --format=%cd --date=short`),
    //     the DDD-13 refusal (`build stamp is empty` followed by `exit 1`), and
    //     both build-args (`FOUNDRY_STAMP_SHA=`, `FOUNDRY_STAMP_DATE=`);
    //   * `Dockerfile` declares `ARG FOUNDRY_STAMP_SHA` and `ARG FOUNDRY_STAMP_DATE`
    //     after the builder `FROM` and before the `RUN` that runs `cargo build`
    //     (DDD-11: earlier busts the apt/COPY cache; later, or in another stage,
    //     never reaches build.rs).

    /// A stamping workflow shaped like DDD-13 (YAML indentation kept so the
    /// fixture reads like the real file; the rule matches lines, not YAML).
    const STAMPING_WORKFLOW: &str = concat!(
        "jobs:\n",
        "  build:\n",
        "    steps:\n",
        "      - name: Compute build stamp\n",
        "        id: stamp\n",
        "        run: |\n",
        "          set -euo pipefail\n",
        "          stamp_sha=\"$(git rev-parse --short=7 HEAD)\"\n",
        "          stamp_date=\"$(git log -1 --format=%cd --date=short)\"\n",
        "          if [ -z \"$stamp_sha\" ] || [ -z \"$stamp_date\" ]; then\n",
        "            echo \"::error::build stamp is empty (sha='$stamp_sha' date='$stamp_date')\"\n",
        "            exit 1\n",
        "          fi\n",
        "          echo \"stamp_sha=$stamp_sha\" >> \"$GITHUB_OUTPUT\"\n",
        "          echo \"stamp_date=$stamp_date\" >> \"$GITHUB_OUTPUT\"\n",
        "      - uses: docker/build-push-action@v6\n",
        "        with:\n",
        "          build-args: |\n",
        "            FOUNDRY_STAMP_SHA=${{ steps.stamp.outputs.stamp_sha }}\n",
        "            FOUNDRY_STAMP_DATE=${{ steps.stamp.outputs.stamp_date }}\n",
    );

    /// A Dockerfile shaped like DDD-11: both inputs in the builder stage,
    /// immediately before the cargo build.
    const STAMPING_DOCKERFILE: &str = concat!(
        "FROM --platform=$BUILDPLATFORM rust:1.85-slim AS builder\n",
        "RUN apt-get update\n",
        "COPY crates ./crates\n",
        "ARG FOUNDRY_STAMP_SHA=\n",
        "ARG FOUNDRY_STAMP_DATE=\n",
        "RUN --mount=type=cache,target=/work/target cargo build --locked --release -p foundry-app\n",
        "FROM gcr.io/distroless/cc-debian12 AS runtime\n",
    );

    const PUBLISH_WORKFLOWS: [&str; 2] = [
        ".forgejo/workflows/build-and-publish.yml",
        ".github/workflows/release.yml",
    ];

    fn stage_publish(forgejo: &str, github: &str, dockerfile: &str) -> tempfile::TempDir {
        stage(&[
            (PUBLISH_WORKFLOWS[0], forgejo),
            (PUBLISH_WORKFLOWS[1], github),
            ("Dockerfile", dockerfile),
        ])
    }

    #[test]
    fn the_shipped_publish_path_stamps_every_image() {
        let violations = check_publish_workflows_stamp_the_image(&workspace_root());
        assert!(
            violations.is_empty(),
            "both publish workflows must compute, refuse on empty and pass the stamp, and the \
             Dockerfile must receive it right before the cargo build: {violations:?}"
        );
    }

    /// A planner stage ahead of the builder (the cargo-chef shape): the
    /// builder is found by its `AS builder` name, not by being first.
    const PLANNER_STAGE: &str = concat!(
        "FROM --platform=$BUILDPLATFORM rust:1.85-slim AS planner\n",
        "RUN cargo chef prepare --recipe-path recipe.json\n",
    );

    #[test]
    fn a_publish_path_that_stamps_every_image_is_accepted() {
        for (shape, dockerfile) in [
            (
                "the builder is the only build stage",
                STAMPING_DOCKERFILE.to_string(),
            ),
            (
                "a planner stage precedes the builder",
                format!("{PLANNER_STAGE}{STAMPING_DOCKERFILE}"),
            ),
        ] {
            let tree = stage_publish(STAMPING_WORKFLOW, STAMPING_WORKFLOW, &dockerfile);
            assert_eq!(
                check_publish_workflows_stamp_the_image(tree.path()),
                Vec::<String>::new(),
                "{shape}"
            );
        }
    }

    #[test]
    fn a_publish_path_that_can_ship_an_unstamped_image_is_flagged() {
        let dockerfile_late_arg = STAMPING_DOCKERFILE
            .replace("ARG FOUNDRY_STAMP_SHA=\n", "")
            .replace(
                "FROM gcr.io/distroless/cc-debian12 AS runtime\n",
                "FROM gcr.io/distroless/cc-debian12 AS runtime\nARG FOUNDRY_STAMP_SHA=\n",
            );
        let refusal = "            echo \"::error::build stamp is empty (sha='$stamp_sha' \
                       date='$stamp_date')\"\n";
        let exit = "            exit 1\n";
        let exit_before_refusal =
            STAMPING_WORKFLOW.replace(&format!("{refusal}{exit}"), &format!("{exit}{refusal}"));
        assert_ne!(
            exit_before_refusal, STAMPING_WORKFLOW,
            "the swap must apply"
        );
        let planner_holds_the_args = format!(
            "{}{}",
            PLANNER_STAGE.replace(
                "RUN cargo chef prepare --recipe-path recipe.json\n",
                "ARG FOUNDRY_STAMP_SHA=\nARG FOUNDRY_STAMP_DATE=\n\
                 RUN cargo build --release -p foundry-app\n",
            ),
            STAMPING_DOCKERFILE
                .replace("ARG FOUNDRY_STAMP_SHA=\n", "")
                .replace("ARG FOUNDRY_STAMP_DATE=\n", ""),
        );
        for (fault, forgejo, github, dockerfile, names) in [
            (
                "a workflow passes no SHA build-arg",
                STAMPING_WORKFLOW.replace("FOUNDRY_STAMP_SHA=", "OTHER_ARG="),
                STAMPING_WORKFLOW.to_string(),
                STAMPING_DOCKERFILE.to_string(),
                PUBLISH_WORKFLOWS[0],
            ),
            (
                "a workflow publishes with an empty stamp",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.replace("build stamp is empty", "stamp missing, continuing"),
                STAMPING_DOCKERFILE.to_string(),
                PUBLISH_WORKFLOWS[1],
            ),
            (
                "a workflow stamps the build time, not the commit date",
                STAMPING_WORKFLOW
                    .replace("git log -1 --format=%cd --date=short", "date -u +%Y-%m-%d"),
                STAMPING_WORKFLOW.to_string(),
                STAMPING_DOCKERFILE.to_string(),
                PUBLISH_WORKFLOWS[0],
            ),
            (
                "the Dockerfile declares the SHA input outside the builder stage",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.to_string(),
                dockerfile_late_arg,
                "Dockerfile",
            ),
            (
                "a workflow keeps both commit commands but also stamps the build time",
                STAMPING_WORKFLOW.replace(
                    "          stamp_date=\"$(git log -1 --format=%cd --date=short)\"\n",
                    "          stamp_date=\"$(git log -1 --format=%cd --date=short)\"\n          \
                     built_on=\"$(date -u +%F)\"\n",
                ),
                STAMPING_WORKFLOW.to_string(),
                STAMPING_DOCKERFILE.to_string(),
                ".forgejo/workflows/build-and-publish.yml:10 computes a date from the build time",
            ),
            (
                "the Dockerfile stamps the builder correctly but also declares the SHA in runtime",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.to_string(),
                format!("{STAMPING_DOCKERFILE}ARG FOUNDRY_STAMP_SHA=\n"),
                "Dockerfile:8 declares `ARG FOUNDRY_STAMP_SHA` outside the builder stage",
            ),
            (
                "a workflow's `exit 1` comes before the refusal, not after it",
                exit_before_refusal,
                STAMPING_WORKFLOW.to_string(),
                STAMPING_DOCKERFILE.to_string(),
                ".forgejo/workflows/build-and-publish.yml does not refuse an empty stamp",
            ),
            (
                "a workflow stamps the build time with backtick `date`",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.replace(
                    "          stamp_date=\"$(git log -1 --format=%cd --date=short)\"\n",
                    "          stamp_date=\"$(git log -1 --format=%cd --date=short)\"\n          \
                     built_on=`date -u +%F`\n",
                ),
                STAMPING_DOCKERFILE.to_string(),
                ".github/workflows/release.yml:10 computes a date from the build time",
            ),
            (
                "the stamp inputs sit in a planner stage ahead of the builder, not in it",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.to_string(),
                planner_holds_the_args,
                "Dockerfile does not declare `ARG FOUNDRY_STAMP_SHA=` in the builder stage",
            ),
            (
                "the builder declares only the date input, the SHA input nowhere",
                STAMPING_WORKFLOW.to_string(),
                STAMPING_WORKFLOW.to_string(),
                STAMPING_DOCKERFILE.replace("ARG FOUNDRY_STAMP_SHA=\n", ""),
                "Dockerfile does not declare `ARG FOUNDRY_STAMP_SHA=` in the builder stage",
            ),
        ] {
            let tree = stage_publish(&forgejo, &github, &dockerfile);
            let violations = check_publish_workflows_stamp_the_image(tree.path());
            assert!(
                violations.iter().any(|v| v.contains(names)),
                "{fault}: expected a violation naming {names}, got {violations:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// instance-workspace-name-rule DDD-12 — `workspace-name-one-source`
// ---------------------------------------------------------------------------

/// The adapter, service and store crates that must never state a name-rule
/// refusal copy themselves (workspace DDD-12a, project DDD-12a).
const NAME_COPY_FORBIDDEN_IN: [&str; 4] = [
    "foundry-app",
    "foundry-services",
    "foundry-api",
    "foundry-store",
];

/// The production-door crates that must never call the raw-name test-seeding
/// seam `create_initial_workspace` (DDD-12b); foundry-store defines it.
const WORKSPACE_SEAM_FORBIDDEN_IN: [&str; 3] = ["foundry-app", "foundry-services", "foundry-api"];

/// A call (or any mention outside a definition) of the test-seeding seam.
const WORKSPACE_SEAM_CALL: &str = "create_initial_workspace(";

/// The opening of every D3 workspace-name refusal, quote included, so prose
/// that merely mentions the rule is not a second copy.
const WORKSPACE_NAME_COPY_PREFIX: &str = "\"Workspace name must";

/// `workspace-name-one-source` (instance-workspace-name-rule DDD-12).
///
/// (a) the literal prefix `"Workspace name must` appears in no `.rs` under
/// `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src` — its
/// only production home is `crates/foundry-core/src` (the D3 copy is
/// `WorkspaceNameError`'s `Display`, DDD-3). Test trees (`tests/`, the
/// acceptance crate) may hold the expected strings. Every violation names
/// `file:line`; a missing `crates/` or an unreadable directory fails the rule.
///
/// (b) `create_initial_workspace(` — the raw-name test-seeding seam (DDD-7) —
/// has no call site under `crates/{foundry-app,foundry-services,foundry-api}/src`:
/// every production door names a workspace through `WorkspaceName`. The seam's
/// definition in foundry-store and its test callers are not flagged.
fn check_workspace_name_one_source(root: &Path) -> Vec<String> {
    scan_one_source(root, &WORKSPACE_NAME_ONE_SOURCE)
}

/// The seam-call violations in one file (`root`, `file`, `contents`), each
/// naming `file:line`.
type SeamCallsIn = fn(&Path, &Path, &str) -> Vec<String>;

/// A "the refusal copy has one home" rule: no `.rs` under the
/// [`NAME_COPY_FORBIDDEN_IN`] crates' `src` may contain `copy_prefix`, and no
/// `.rs` under the `seam_forbidden_in` crates' `src` may call the rule's
/// bypassing seam.
struct OneSourceRule {
    /// Rule id, the prefix of every violation it reports.
    id: &'static str,
    /// The literal opening of the copy, quote included.
    copy_prefix: &'static str,
    /// Where the copy lives instead, for the violation message.
    home: &'static str,
    /// The design reference the messages cite.
    reference: &'static str,
    /// The crates whose `src` must not call the seam.
    seam_forbidden_in: &'static [&'static str],
    /// The seam-call violations in one file, each naming `file:line`; `None`
    /// when the rule guards the copy only.
    seam_calls_in: Option<SeamCallsIn>,
}

const WORKSPACE_NAME_ONE_SOURCE: OneSourceRule = OneSourceRule {
    id: "workspace-name-one-source",
    copy_prefix: WORKSPACE_NAME_COPY_PREFIX,
    home: "WorkspaceNameError's Display in foundry-core",
    reference: "DDD-3/DDD-12",
    seam_forbidden_in: &WORKSPACE_SEAM_FORBIDDEN_IN,
    seam_calls_in: Some(workspace_seam_call_violations_in),
};

/// The shared scan behind every one-source rule: walk each forbidden crate's
/// `src`, flag every line stating the copy (`file:line`), and every seam call
/// in the crates the rule closes to it. A missing `crates/`, an unreadable
/// directory or file fails the rule — it never passes vacuously.
fn scan_one_source(root: &Path, rule: &OneSourceRule) -> Vec<String> {
    let crates_dir = root.join("crates");
    if !crates_dir.is_dir() {
        return vec![format!(
            "{}: cannot list {} — no crate could be checked for a second copy of the \
             refusal ({}).",
            rule.id,
            crates_dir.display(),
            rule.reference
        )];
    }
    let mut violations = Vec::new();
    for crate_name in NAME_COPY_FORBIDDEN_IN {
        let src = crates_dir.join(crate_name).join("src");
        if !src.is_dir() {
            continue;
        }
        let (sources, unreadable) = files_under(&src, "rs", &|_| false);
        for dir in &unreadable {
            violations.push(format!(
                "{}: cannot list {} — it could not be checked for a second copy of the \
                 refusal ({}).",
                rule.id,
                rel(root, dir),
                rule.reference
            ));
        }
        for file in &sources {
            let Ok(contents) = std::fs::read_to_string(file) else {
                violations.push(format!(
                    "{}: cannot read {} ({}).",
                    rule.id,
                    rel(root, file),
                    rule.reference
                ));
                continue;
            };
            violations.extend(copy_violations_in(root, rule, file, &contents));
            if let Some(seam_calls_in) = rule.seam_calls_in {
                if rule.seam_forbidden_in.contains(&crate_name) {
                    violations.extend(seam_calls_in(root, file, &contents));
                }
            }
        }
    }
    violations
}

/// The copy violations in one file, each naming `file:line`.
fn copy_violations_in(
    root: &Path,
    rule: &OneSourceRule,
    file: &Path,
    contents: &str,
) -> Vec<String> {
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(rule.copy_prefix))
        .map(|(line_no, _)| {
            format!(
                "{}: {}:{} states the refusal copy — its one home is {}; render \
                 `err.to_string()` instead ({})",
                rule.id,
                rel(root, file),
                line_no + 1,
                rule.home,
                rule.reference,
            )
        })
        .collect()
}

/// The DDD-12b violations in one file: each line calling the test-seeding seam,
/// named `file:line`. Comment lines and the seam's own `fn` definition are not calls.
fn workspace_seam_call_violations_in(root: &Path, file: &Path, contents: &str) -> Vec<String> {
    let definition = format!("fn {WORKSPACE_SEAM_CALL}");
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            line.contains(WORKSPACE_SEAM_CALL)
                && !line.contains(&definition)
                && !line.trim_start().starts_with("//")
        })
        .map(|(line_no, _)| {
            format!(
                "workspace-name-one-source: {}:{} calls the test-seeding seam \
                 `create_initial_workspace` from a production door — it applies no \
                 workspace-name rule; name the workspace through `WorkspaceName` (DDD-7/DDD-12)",
                rel(root, file),
                line_no + 1,
            )
        })
        .collect()
}

#[cfg(test)]
mod workspace_name_one_source_tests {
    //! Injected-violation gold tests for `workspace-name-one-source` (DDD-12),
    //! the `check_provisioned_marker_is_never_rewritten` idiom: a staged tree, the
    //! rule run against it, the violations named by `file:line`.

    use super::check_workspace_name_one_source;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The copy's one production home, as DELIVER will write it.
    const CORE_COPY: &str =
        "#[derive(Debug, thiserror::Error)]\npub enum WorkspaceNameError {\n    \
        #[error(\"Workspace name must not be empty\")]\n    Empty,\n}\n";

    #[test]
    fn the_copy_in_foundry_core_is_its_one_home() {
        let tree = stage(&[("crates/foundry-core/src/lib.rs", CORE_COPY)]);
        let violations = check_workspace_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_second_copy_in_any_adapter_or_service_crate_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/lib.rs", CORE_COPY),
            (
                "crates/foundry-app/src/instance_admin.rs",
                "fn a() {}\nlet m = \"Workspace name must be at most 24 characters\";\n",
            ),
            (
                "crates/foundry-services/src/deep/workspaces.rs",
                "const M: &str = \"Workspace name must not contain control characters\";\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\n\n\nlet _ = format!(\"Workspace name must {}\", x);\n",
            ),
            (
                "crates/foundry-store/src/lib.rs",
                "let _ = \"Workspace name must not be empty\";\n",
            ),
        ]);
        let violations = check_workspace_name_one_source(tree.path());
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/instance_admin.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(
                &violations,
                "crates/foundry-services/src/deep/workspaces.rs:1"
            ),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:4"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-store/src/lib.rs:1"),
            "{violations:?}"
        );
    }

    #[test]
    fn expected_strings_in_the_acceptance_suite_and_crate_tests_are_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/lib.rs", CORE_COPY),
            (
                "crates/foundry-acceptance/src/steps/feature_x.rs",
                "let m = \"Workspace name must not be empty\";\n",
            ),
            (
                "crates/foundry-app/tests/rename.rs",
                "assert_eq!(m, \"Workspace name must be at most 24 characters\");\n",
            ),
        ]);
        let violations = check_workspace_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_production_call_to_the_test_seeding_seam_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/lib.rs", CORE_COPY),
            (
                "crates/foundry-app/src/bootstrap.rs",
                "async fn f() {\n    store.create_initial_workspace(id, name).await;\n}\n",
            ),
            (
                "crates/foundry-services/src/provisioning.rs",
                "let _ = s.create_initial_workspace(\n    a,\n);\n",
            ),
        ]);
        let violations = check_workspace_name_one_source(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/bootstrap.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/provisioning.rs:1"),
            "{violations:?}"
        );
    }

    #[test]
    fn the_seam_itself_and_its_test_callers_are_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/lib.rs", CORE_COPY),
            (
                "crates/foundry-store/src/lib.rs",
                "pub async fn create_initial_workspace(&self) {}\n",
            ),
            (
                "crates/foundry-store/tests/seed.rs",
                "store.create_initial_workspace(a).await;\n",
            ),
            (
                "crates/foundry-acceptance/src/steps/feature_mwt_slice_06.rs",
                "store.create_initial_workspace(a).await;\n",
            ),
        ]);
        let violations = check_workspace_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_workspace_name_one_source(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }
}

// ---------------------------------------------------------------------------
// project-name-rule DDD-12 — `project-name-one-source`
// ---------------------------------------------------------------------------
//
// Clause (a): the copy has one home (slice 01). Clause (b): no `insert_project(`
// call in the app/api doors (slice 02).

/// The opening of every D4 project-name refusal, quote included.
const PROJECT_NAME_COPY_PREFIX: &str = "\"Project name must";

const PROJECT_NAME_ONE_SOURCE: OneSourceRule = OneSourceRule {
    id: "project-name-one-source",
    copy_prefix: PROJECT_NAME_COPY_PREFIX,
    home: "ProjectNameError's Display in foundry-core",
    reference: "DDD-4/DDD-12",
    seam_forbidden_in: &PROJECT_INSERT_FORBIDDEN_IN,
    seam_calls_in: Some(project_insert_call_violations_in),
};

/// `project-name-one-source` (project-name-rule DDD-12).
///
/// (a) the literal prefix `"Project name must` appears in no `.rs` under
/// `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src`
/// (comments and in-file `#[cfg(test)]` included) — its one production home is
/// `ProjectNameError`'s `Display` in `crates/foundry-core/src` (DDD-4).
///
/// (b) `insert_project(` has no call site under `crates/{foundry-app,foundry-api}/src`:
/// the one mint point is `foundry_services::projects::create_project` (DDD-5/9).
/// A call split across lines (`insert_project` then `(` on the next) is still a
/// call. Comment lines, the store's definition, the services caller and test
/// callers are not flagged.
fn check_project_name_one_source(root: &Path) -> Vec<String> {
    scan_one_source(root, &PROJECT_NAME_ONE_SOURCE)
}

/// The doors that must create a project through the use-case, never the store.
const PROJECT_INSERT_FORBIDDEN_IN: [&str; 2] = ["foundry-app", "foundry-api"];

/// The store method only `create_project` may call (DDD-5/9).
const PROJECT_INSERT_SEAM: &str = "insert_project";

/// The DDD-12b violations in one file: each `insert_project` CALL, named
/// `file:line` (see [`call_sites`] for what counts as a call).
fn project_insert_call_violations_in(root: &Path, file: &Path, contents: &str) -> Vec<String> {
    call_sites(contents, PROJECT_INSERT_SEAM)
        .into_iter()
        .map(|line| {
            format!(
                "project-name-one-source: {}:{line} calls `insert_project` from a door — it \
                 applies no project-name rule and mints no address; create through \
                 `foundry_services::projects::create_project` (DDD-5/DDD-12)",
                rel(root, file),
            )
        })
        .collect()
}

/// The 1-based line of every CALL of `name` in a Rust source: the name, optional
/// whitespace (newlines included), then `(`. A longer identifier containing the
/// name, a `fn name` definition and any occurrence on a `//` comment line are
/// not calls. A call split across lines is reported at the line naming it.
fn call_sites(contents: &str, name: &str) -> Vec<usize> {
    call_offsets(contents, name)
        .into_iter()
        .map(|at| contents[..at].matches('\n').count() + 1)
        .collect()
}

/// The byte offset of every CALL of `name` in a Rust source, by the rules of
/// [`call_sites`].
fn call_offsets(contents: &str, name: &str) -> Vec<usize> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    contents
        .match_indices(name)
        .filter(|(at, _)| {
            let before = &contents[..*at];
            let after = &contents[at + name.len()..];
            let line_start = before.rfind('\n').map_or(0, |i| i + 1);
            let joined_to_ident = before.chars().next_back().is_some_and(is_ident)
                || after.chars().next().is_some_and(is_ident);
            !joined_to_ident
                && after.trim_start().starts_with('(')
                && !before.trim_end().ends_with("fn")
                && !contents[line_start..].trim_start().starts_with("//")
        })
        .map(|(at, _)| at)
        .collect()
}

#[cfg(test)]
mod project_name_one_source_tests {
    //! Injected-violation gold tests for `project-name-one-source` (DDD-12), the
    //! `workspace_name_one_source_tests` idiom: a staged tree, the rule run
    //! against it, the violations named by `file:line`.

    use super::check_project_name_one_source;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The copy's one production home, as DELIVER will write it.
    const CORE_COPY: &str = "#[derive(Debug, thiserror::Error)]\npub enum ProjectNameError {\n    \
        #[error(\"Project name must not be empty\")]\n    Empty,\n}\n";

    #[test]
    fn the_copy_in_foundry_core_is_its_one_home() {
        let tree = stage(&[("crates/foundry-core/src/project_name.rs", CORE_COPY)]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_second_copy_in_any_adapter_service_or_store_crate_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/projects.rs",
                "fn a() {}\nlet m = \"Project name must be unique within the team\";\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "/// Trimmed name empty -> 422 \"Project name must not be empty\".\nEmptyName,\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\n\nlet _ = format!(\"Project name must {}\", x);\n",
            ),
            (
                "crates/foundry-store/src/lib.rs",
                "#[cfg(test)]\nmod t { const M: &str = \"Project name must be at most 256 characters\"; }\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/projects.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/projects.rs:1"),
            "a doc comment quoting the copy is a second copy too: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-store/src/lib.rs:2"),
            "an in-file test quoting the copy is flagged; assert via to_string(): {violations:?}"
        );
    }

    #[test]
    fn expected_strings_in_the_acceptance_suite_and_crate_tests_are_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-acceptance/src/steps/feature_project_name_rule.rs",
                "let m = \"Project name must not be empty\";\n",
            ),
            (
                "crates/foundry-core/tests/project_name.rs",
                "assert_eq!(m, \"Project name must be at most 256 characters\");\n",
            ),
            (
                "crates/foundry-services/tests/create_project_use_case.rs",
                "let _ = \"Project name must be unique within the team\";\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn the_workspace_copy_and_other_project_copy_are_not_this_rules_business() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/projects.rs",
                "let a = \"Project key must not be empty\";\nlet b = \"Workspace name must not be empty\";\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_door_that_inserts_a_project_itself_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/projects.rs",
                "async fn f() {\n    state.store.insert_project(id, ws, team, name, slug, key).await;\n}\n",
            ),
            (
                "crates/foundry-api/src/projects.rs",
                "let _ = store\n    .insert_project(\n        a,\n    );\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/projects.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/projects.rs:2"),
            "{violations:?}"
        );
    }

    #[test]
    fn the_store_definition_the_use_case_and_test_callers_are_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-store/src/lib.rs",
                "pub async fn insert_project(\n    &self,\n) {}\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "store.insert_project(id, ws, team, name.as_str(), slug, key).await?;\n",
            ),
            (
                "crates/foundry-store/tests/projects.rs",
                "store.insert_project(a).await;\n",
            ),
            (
                "crates/foundry-app/src/projects.rs",
                "// the use-case calls insert_project( on our behalf\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_longer_name_that_ends_in_insert_project_is_not_the_store_call() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/projects.rs",
                "async fn f() {\n    state.store.bulk_insert_project(a).await;\n    \
                 let x = reinsert_project(b);\n}\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_comment_mentioning_the_store_call_past_the_first_line_is_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/project_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/projects.rs",
                "fn a() {}\n    // the use-case calls insert_project( for us\n",
            ),
        ]);
        let violations = check_project_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_project_name_one_source(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }
}

// ---------------------------------------------------------------------------
// name-db-checks DDD-12 — `name-rule-legacy-seam`
// ---------------------------------------------------------------------------

/// The test-only seam that seeds a row predating the name rule (DDD-11).
const LEGACY_SEAM_CALL: &str = "seed_row_predating_name_rule";

/// The seam's own file — the one `.rs` allowed to switch the name-rule trigger.
const LEGACY_SEAM_FILE: &str = "crates/foundry-store/src/name_rule_legacy_seam.rs";

/// The production doors that must never call the legacy seam (DDD-12a).
const LEGACY_SEAM_FORBIDDEN_IN: [&str; 3] = ["foundry-app", "foundry-services", "foundry-api"];

/// The trigger bypasses, lowercase with single spaces: SQL keywords are
/// case-insensitive, so lines are compared folded (DDD-12b, D8).
const NAME_RULE_BYPASSES: [&str; 2] = ["disable trigger", "session_replication_role"];

/// `name-rule-legacy-seam` (name-db-checks DDD-12).
///
/// (a) `seed_row_predating_name_rule` has no CALL under
/// `crates/{foundry-app,foundry-services,foundry-api}/src` (a call split across
/// lines included); its definition, test callers, comment lines and longer
/// identifiers are not flagged.
///
/// (b) `DISABLE TRIGGER` (any letter case) and `session_replication_role` appear
/// in no `.rs` under `crates/` except [`LEGACY_SEAM_FILE`] — comments and tests
/// included: one greppable bypass (D8, DDD-11).
///
/// A missing `crates/`, an unreadable directory or file fails the rule.
fn check_name_rule_legacy_seam(root: &Path) -> Vec<String> {
    let crates_dir = root.join("crates");
    if !crates_dir.is_dir() {
        return vec![format!(
            "name-rule-legacy-seam: cannot list {} — no crate could be checked for a \
             name-rule bypass (DDD-12).",
            crates_dir.display()
        )];
    }
    let (sources, unreadable) = files_under(&crates_dir, "rs", &|_| false);
    let mut violations: Vec<String> = unreadable
        .iter()
        .map(|dir| {
            format!(
                "name-rule-legacy-seam: cannot list {} — it could not be checked for a \
                 name-rule bypass (DDD-12).",
                rel(root, dir)
            )
        })
        .collect();
    for file in &sources {
        let Ok(contents) = std::fs::read_to_string(file) else {
            violations.push(format!(
                "name-rule-legacy-seam: cannot read {} (DDD-12).",
                rel(root, file)
            ));
            continue;
        };
        if is_production_door(&crates_dir, file) {
            violations.extend(legacy_seam_call_violations_in(root, file, &contents));
        }
        if rel(root, file) != LEGACY_SEAM_FILE {
            violations.extend(name_rule_bypass_violations_in(root, file, &contents));
        }
    }
    violations
}

/// Whether `file` sits under the `src` of a crate the legacy seam is closed to.
fn is_production_door(crates_dir: &Path, file: &Path) -> bool {
    LEGACY_SEAM_FORBIDDEN_IN
        .iter()
        .any(|crate_name| file.starts_with(crates_dir.join(crate_name).join("src")))
}

/// The DDD-12a violations in one file: each legacy-seam call, named `file:line`.
fn legacy_seam_call_violations_in(root: &Path, file: &Path, contents: &str) -> Vec<String> {
    call_sites(contents, LEGACY_SEAM_CALL)
        .into_iter()
        .map(|line| {
            format!(
                "name-rule-legacy-seam: {}:{line} calls the test-only seam \
                 `seed_row_predating_name_rule` from a production door — it switches the \
                 name-rule trigger off; production writes go through the rule (DDD-11/DDD-12)",
                rel(root, file),
            )
        })
        .collect()
}

/// The DDD-12b violations in one file: each line naming a trigger bypass,
/// compared lowercase with whitespace runs folded to one space.
fn name_rule_bypass_violations_in(root: &Path, file: &Path, contents: &str) -> Vec<String> {
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let folded = line
                .to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            NAME_RULE_BYPASSES
                .iter()
                .any(|bypass| folded.contains(bypass))
        })
        .map(|(line_no, _)| {
            format!(
                "name-rule-legacy-seam: {}:{} switches the name-rule trigger off (DISABLE \
                 TRIGGER / session_replication_role) — the one bypass lives in {} (D8, \
                 DDD-11/DDD-12)",
                rel(root, file),
                line_no + 1,
                LEGACY_SEAM_FILE,
            )
        })
        .collect()
}

#[cfg(test)]
mod name_rule_legacy_seam_tests {
    //! Injected-violation gold tests for `name-rule-legacy-seam` (DDD-12), the
    //! `project_name_one_source_tests` idiom: a staged tree, the rule run against
    //! it, the violations named by `file:line`.

    use super::check_name_rule_legacy_seam;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The seam's own file, as DELIVER will write it (working path and name).
    const SEAM_FILE: &str = "crates/foundry-store/src/name_rule_legacy_seam.rs";
    const SEAM: &str = "//! Never `session_replication_role` (superuser, skips FKs).\n\
        #[cfg(feature = \"test-support\")]\n\
        pub async fn seed_row_predating_name_rule(pool: &PgPool, table: NameRuleTable) {\n    \
        let off = format!(\"ALTER TABLE {t} DISABLE TRIGGER {t}_name_rule_on_insert\", t = table.name());\n}\n";

    #[test]
    fn the_seam_its_definition_and_its_test_callers_pass() {
        let tree = stage(&[
            (SEAM_FILE, SEAM),
            (
                "crates/foundry-store/tests/name_rule_in_database.rs",
                "seed_row_predating_name_rule(&pool, NameRuleTable::Workspaces).await;\n",
            ),
            (
                "crates/foundry-acceptance/src/steps/feature_name_db_checks.rs",
                "foundry_store::seed_row_predating_name_rule(&pool, table).await;\n",
            ),
        ]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_door_that_calls_the_seam_is_flagged() {
        let tree = stage(&[
            (SEAM_FILE, SEAM),
            (
                "crates/foundry-app/src/instance_admin.rs",
                "fn a() {}\nstore.seed_row_predating_name_rule(&pool, t).await;\n",
            ),
            (
                "crates/foundry-services/src/workspaces.rs",
                "let _ = foundry_store::seed_row_predating_name_rule\n    (&pool, t);\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\n\nseed_row_predating_name_rule(&p, t).await;\n",
            ),
        ]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/instance_admin.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/workspaces.rs:1"),
            "a call split across lines is still a call: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_comment_or_a_longer_name_in_a_door_is_not_a_seam_call() {
        let tree = stage(&[
            (SEAM_FILE, SEAM),
            (
                "crates/foundry-app/src/instance_admin.rs",
                "// tests seed legacy rows with seed_row_predating_name_rule( only\n\
                 let x = reseed_row_predating_name_rule_count(a);\n",
            ),
        ]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_trigger_switch_or_replica_mode_outside_the_seam_file_is_flagged() {
        let tree = stage(&[
            (SEAM_FILE, SEAM),
            (
                "crates/foundry-store/src/lib.rs",
                "fn a() {}\nlet s = \"ALTER TABLE projects DISABLE TRIGGER projects_name_rule_on_insert\";\n",
            ),
            (
                "crates/foundry-acceptance/src/steps/feature_x.rs",
                "sqlx::query(\"alter table workspaces disable trigger all\");\n",
            ),
            (
                "crates/foundry-app/tests/fixtures.rs",
                "\n\n\nsqlx::query(\"SET session_replication_role = replica\");\n",
            ),
            (
                "crates/foundry-store/tests/seed.rs",
                "// a fixture once did: DISABLE TRIGGER workspaces_name_rule_on_rename\n",
            ),
        ]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-store/src/lib.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(
                &violations,
                "crates/foundry-acceptance/src/steps/feature_x.rs:1"
            ),
            "letter case does not matter to SQL: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-app/tests/fixtures.rs:4"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-store/tests/seed.rs:1"),
            "comments count: one greppable bypass: {violations:?}"
        );
    }

    #[test]
    fn migrations_docs_and_files_outside_crates_are_not_this_rules_business() {
        let tree = stage(&[
            (SEAM_FILE, SEAM),
            (
                "crates/foundry-store/migrations/0019_workspace_name_rule.sql",
                "-- undo: DROP TRIGGER IF EXISTS …; never DISABLE TRIGGER in production\n",
            ),
            (
                "docs/feature/name-db-checks/feature-delta.md",
                "session_replication_role is rejected (D8 d)\n",
            ),
            (
                "xtask/src/check_arch.rs",
                "const SWITCH: &str = \"DISABLE TRIGGER\";\n",
            ),
        ]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_name_rule_legacy_seam(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }
}

// ---------------------------------------------------------------------------
// team-membership DDD-19 — `team-membership-one-seam`
// ---------------------------------------------------------------------------

/// The store's team-membership writes: no door may call them directly — every
/// write goes through a `foundry-services` use-case and its policy (ADR-003).
/// 02-01 adds `set_team_role`, 03-01 adds `remove_team_member`.
const TEAM_WRITE_SEAMS: [&str; 3] = ["add_team_member", "set_team_role", "remove_team_member"];

/// The doors whose `src` must not call a [`TEAM_WRITE_SEAMS`] method.
const TEAM_WRITE_SEAMS_FORBIDDEN_IN: [&str; 2] = ["foundry-app", "foundry-api"];

/// The crates whose `src` may hold SQL writing `team_memberships`: the store
/// (its one home) and the acceptance harness (fixtures seed rows directly).
const TEAM_MEMBERSHIP_SQL_ALLOWED_IN: [&str; 2] = ["foundry-store", "foundry-acceptance"];

const TEAM_MEMBERSHIP_ONE_SEAM: OneSeamRule = OneSeamRule {
    id: "team-membership-one-seam",
    tables: &["team_memberships"],
    sql_allowed_in: &TEAM_MEMBERSHIP_SQL_ALLOWED_IN,
    sql_why: "every membership write goes through the store, its lock and the lead trigger \
              (DDD-19, ADR-TEAM-MEMBERSHIP-001)",
    seams: &TEAM_WRITE_SEAMS,
    seams_forbidden_in: &TEAM_WRITE_SEAMS_FORBIDDEN_IN,
    seam_why: "team writes go through the foundry-services use-case and its policy (DDD-19, \
               ADR-003)",
    reference: "DDD-19",
};

/// `team-membership-one-seam` (team-membership DDD-19).
///
/// (a) no SQL `INSERT INTO` / `UPDATE` / `DELETE FROM team_memberships` in any
/// `.rs` under `crates/*/src` except [`TEAM_MEMBERSHIP_SQL_ALLOWED_IN`] — case
/// and line breaks inside one string literal do not matter; comments do not
/// count; each crate's `tests/` is outside `src` and so out of scope.
///
/// (b) no [`TEAM_WRITE_SEAMS`] method has a call site under
/// `crates/{foundry-app,foundry-api}/src`.
///
/// A missing `crates/`, an unreadable directory or file fails the rule.
fn check_team_membership_one_seam(root: &Path) -> Vec<String> {
    scan_one_seam(root, &TEAM_MEMBERSHIP_ONE_SEAM)
}

/// A "these tables have one writer" rule: no `.rs` under a `crates/*/src` outside
/// `sql_allowed_in` may hold SQL writing one of `tables`, and no `.rs` under the
/// `seams_forbidden_in` crates' `src` may call one of the store's `seams`.
struct OneSeamRule {
    /// Rule id, the prefix of every violation it reports.
    id: &'static str,
    /// The tables only the store may write.
    tables: &'static [&'static str],
    /// The crates whose `src` may hold SQL writing `tables`.
    sql_allowed_in: &'static [&'static str],
    /// Why the SQL write belongs in the store, for the violation message.
    sql_why: &'static str,
    /// The store's write methods no door may call.
    seams: &'static [&'static str],
    /// The doors whose `src` must not call a seam.
    seams_forbidden_in: &'static [&'static str],
    /// Why the seam call belongs in the use-case, for the violation message.
    seam_why: &'static str,
    /// The design reference the fail-closed messages cite.
    reference: &'static str,
}

/// The shared scan behind every one-seam rule: walk each crate's `src`, flag
/// every SQL write of a guarded table outside the allowed crates and every seam
/// call in the closed doors, each named `file:line`. A missing `crates/`, an
/// unreadable directory or file fails the rule — it never passes vacuously.
fn scan_one_seam(root: &Path, rule: &OneSeamRule) -> Vec<String> {
    let crates_dir = root.join("crates");
    let tables = rule.tables.join(" or ");
    let Ok(entries) = std::fs::read_dir(&crates_dir) else {
        return vec![format!(
            "{}: cannot list {} — no crate could be checked for a {tables} write outside the \
             store ({}).",
            rule.id,
            crates_dir.display(),
            rule.reference
        )];
    };
    let mut crate_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("src").is_dir())
        .collect();
    crate_dirs.sort();

    let mut violations = Vec::new();
    for crate_dir in crate_dirs {
        let crate_name = crate_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let scan_sql = !rule.sql_allowed_in.contains(&crate_name);
        let scan_calls = rule.seams_forbidden_in.contains(&crate_name);
        if !(scan_sql || scan_calls) {
            continue;
        }
        let (sources, unreadable) = files_under(&crate_dir.join("src"), "rs", &|_| false);
        for dir in &unreadable {
            violations.push(format!(
                "{}: cannot list {} — it could not be checked for a {tables} write ({}).",
                rule.id,
                rel(root, dir),
                rule.reference
            ));
        }
        for file in &sources {
            let Ok(contents) = std::fs::read_to_string(file) else {
                violations.push(format!(
                    "{}: cannot read {} ({}).",
                    rule.id,
                    rel(root, file),
                    rule.reference
                ));
                continue;
            };
            if scan_sql {
                violations.extend(sql_write_violations_in(root, rule, file, &contents));
            }
            if scan_calls {
                violations.extend(seam_call_violations_in(root, rule, file, &contents));
            }
        }
    }
    violations
}

/// The SQL-clause violations in one file: each SQL write of a guarded table,
/// named `file:line`.
fn sql_write_violations_in(
    root: &Path,
    rule: &OneSeamRule,
    file: &Path,
    contents: &str,
) -> Vec<String> {
    let literals = rust_string_literals(contents);
    rule.tables
        .iter()
        .flat_map(|table| {
            literals.iter().flat_map(move |(first_line, sql)| {
                sql_writes_of(sql, table)
                    .into_iter()
                    .map(move |offset| (table, first_line + offset))
            })
        })
        .map(|(table, line)| {
            format!(
                "{}: {}:{line} writes {table} in SQL outside foundry-store — {}",
                rule.id,
                rel(root, file),
                rule.sql_why,
            )
        })
        .collect()
}

/// The 0-based line offsets, within `sql`, of every `INSERT INTO` / `UPDATE` /
/// `DELETE FROM` naming `table` as its target. SQL comments are stripped first;
/// the verb may sit on an earlier line than the table.
fn sql_writes_of(sql: &str, table: &str) -> Vec<usize> {
    let code = strip_sql_comments(sql).to_ascii_lowercase();
    let mut offsets = Vec::new();
    let mut from = 0;
    while let Some(at) = find_sql_word(&code, table, from) {
        from = at + table.len();
        if let Some(write) = sql_write_targeting(&code, at) {
            offsets.push(code[..write.statement_at].matches('\n').count());
        }
    }
    offsets
}

/// An SQL write statement, as seen from the table word it targets.
struct SqlWrite {
    verb: &'static str,
    statement_at: usize,
}

/// The `INSERT INTO` / `UPDATE` / `DELETE FROM` whose target is the table word
/// starting at `at` in `code` (lowercased, comments stripped), if any.
fn sql_write_targeting(code: &str, at: usize) -> Option<SqlWrite> {
    // The word ending just before `end`, and where it starts.
    let word_before = |end: usize| {
        let text = code[..end].trim_end();
        let start = text.rfind(char::is_whitespace).map_or(0, |i| i + 1);
        (&text[start..], start)
    };
    let (last, last_at) = word_before(at);
    let (verb, verb_at) = word_before(last_at);
    match (last, verb) {
        ("update", _) => Some(SqlWrite {
            verb: "update",
            statement_at: last_at,
        }),
        ("into", "insert") => Some(SqlWrite {
            verb: "insert",
            statement_at: verb_at,
        }),
        ("from", "delete") => Some(SqlWrite {
            verb: "delete",
            statement_at: verb_at,
        }),
        _ => None,
    }
}

/// The seam-clause violations in one file: each call of a guarded store write,
/// named `file:line` (see [`call_sites`] for what counts as a call).
fn seam_call_violations_in(
    root: &Path,
    rule: &OneSeamRule,
    file: &Path,
    contents: &str,
) -> Vec<String> {
    rule.seams
        .iter()
        .flat_map(|seam| {
            call_sites(contents, seam)
                .into_iter()
                .map(move |line| (seam, line))
        })
        .map(|(seam, line)| {
            format!(
                "{}: {}:{line} calls `{seam}` from a door — {}",
                rule.id,
                rel(root, file),
                rule.seam_why,
            )
        })
        .collect()
}

#[cfg(test)]
mod team_membership_one_seam_tests {
    //! Injected-violation gold tests for `team-membership-one-seam` (DDD-19), the
    //! `name_rule_legacy_seam_tests` idiom: a staged tree, the rule run against
    //! it, the violations named by `file:line`.

    use super::check_team_membership_one_seam;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The store's own writes, as shipped: the one place SQL may write the table.
    const STORE_TEAMS: &str = "pub async fn add_team_member(&self) {\n    \
        sqlx::query(\n        \"INSERT INTO team_memberships (team_id, user_id, role)\n         \
        SELECT $1, $2, 'member' ON CONFLICT DO NOTHING\",\n    );\n}\n";

    // ---- clause (a): SQL writing team_memberships lives only in foundry-store/src

    #[test]
    fn a_team_memberships_write_in_store_src_acceptance_or_tests_passes() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-store/src/bootstrap.rs",
                "sqlx::query(\"DELETE FROM team_memberships WHERE user_id = $1\");\n",
            ),
            (
                "crates/foundry-acceptance/src/steps/feature_x.rs",
                "sqlx::query(\"INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')\");\n",
            ),
            (
                "crates/foundry-acceptance/src/support/team_membership.rs",
                "sqlx::query(\"UPDATE team_memberships SET role = $3\");\n",
            ),
            (
                "crates/foundry-store/tests/team_lead_guard.rs",
                "sqlx::query(\"UPDATE team_memberships SET role = 'member' WHERE team_id = $1\");\n",
            ),
            (
                "crates/foundry-services/tests/write_use_cases.rs",
                "sqlx::query(\"INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')\");\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_team_memberships_write_in_any_other_crates_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nsqlx::query(\"INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')\");\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "\n\nlet _ = sqlx::query(r#\"\n    update\n    Team_Memberships SET role = 'lead'\"#);\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "sqlx::query(\"DELETE   FROM team_memberships WHERE user_id = $1\");\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/teams.rs:4"),
            "letter case and line breaks do not matter to SQL; the verb's line is named: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:1"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_read_a_comment_or_another_table_outside_the_store_is_not_a_write() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "// sqlx::query(\"DELETE FROM team_memberships\")\n\
                 const READ: &str = \"SELECT user_id FROM team_memberships WHERE team_id = $1\";\n\
                 const OTHER: &str = \"DELETE FROM team_memberships_archive WHERE id = $1\";\n\
                 const JOIN: &str = \"UPDATE teams SET name = $1 FROM team_memberships tm WHERE tm.team_id = teams.id\";\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    // ---- clause (b): the store's team writes have no call site in app or api src

    #[test]
    fn a_team_write_called_from_app_or_api_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nlet _ = state.store.add_team_member(ws, team, user).await;\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\nlet _ = store\n    .add_team_member\n    (ws, team, user);\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "a call split across lines is still a call: {violations:?}"
        );
    }

    #[test]
    fn a_role_change_called_from_app_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\n\nlet _ = state.store.set_team_role(team, user, TeamRole::Lead).await;\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "let changed = self.store.set_team_role(facts.team_id, user_id, role).await?;\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:3")
                && violations[0].contains("`set_team_role`"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_team_removal_called_from_api_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-api/src/teams.rs",
                "fn a() {}\nfn b() {}\n\nlet _ = state.store.remove_team_member(team, user).await;\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "let removed = self.store.remove_team_member(facts.team_id, user_id).await?;\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/teams.rs:4")
                && violations[0].contains("`remove_team_member`"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_team_write_called_from_services_store_or_tests_passes() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-services/src/teams.rs",
                "let added = self.store.add_team_member(ws, facts.team_id, user_id).await?;\n",
            ),
            (
                "crates/foundry-store/tests/team_lead_guard.rs",
                "s1.add_team_member(ws, team, tomas).await;\n",
            ),
            (
                "crates/foundry-app/tests/teams.rs",
                "store.add_team_member(ws, team, tomas).await;\n",
            ),
            (
                "crates/foundry-app/src/teams.rs",
                "// the use-case calls add_team_member( behind the policy\n\
                 let r = services.add_member_to_team(principal, slug, user_id).await;\n",
            ),
        ]);
        let violations = check_team_membership_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_team_membership_one_seam(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn the_shipped_tree_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf();
        let store = std::fs::read_to_string(root.join("crates/foundry-store/src/teams.rs"))
            .expect("store teams");
        assert!(
            store.contains("INSERT INTO team_memberships"),
            "ANTI-VACUITY: the store is expected to write team_memberships"
        );
        let violations = check_team_membership_one_seam(&root);
        assert!(violations.is_empty(), "{violations:?}");
    }
}

// ---------------------------------------------------------------------------
// team-lifecycle DDD-19 — `team-name-one-source` and `team-lifecycle-one-seam`
// ---------------------------------------------------------------------------

/// The opening of every team-name refusal, quote included.
const TEAM_NAME_COPY_PREFIX: &str = "\"Team name must";

const TEAM_NAME_ONE_SOURCE: OneSourceRule = OneSourceRule {
    id: "team-name-one-source",
    copy_prefix: TEAM_NAME_COPY_PREFIX,
    home: "TeamNameError's Display in foundry-core",
    reference: "team-lifecycle DDD-19",
    seam_forbidden_in: &[],
    seam_calls_in: None,
};

/// `team-name-one-source` (team-lifecycle DDD-19): the literal prefix
/// `"Team name must` appears in no `.rs` under
/// `crates/{foundry-app,foundry-services,foundry-api,foundry-store}/src`
/// (comments and in-file `#[cfg(test)]` included) — its one production home is
/// `TeamNameError`'s `Display` in `crates/foundry-core/src`. Test trees
/// (`tests/`, the acceptance crate) may hold the expected strings.
fn check_team_name_one_source(root: &Path) -> Vec<String> {
    scan_one_source(root, &TEAM_NAME_ONE_SOURCE)
}

/// The store's team-lifecycle writes, which no door may call: every create,
/// rename and delete goes through its `foundry-services` use-case. 02-01
/// added `delete_team_if_empty`; 04-01 added `rename_team_with_audit`.
const TEAM_LIFECYCLE_SEAMS: [&str; 3] = [
    "create_team_with_lead",
    "delete_team_if_empty",
    "rename_team_with_audit",
];

const TEAM_LIFECYCLE_ONE_SEAM: OneSeamRule = OneSeamRule {
    id: "team-lifecycle-one-seam",
    tables: &["teams", "team_lifecycle_events"],
    sql_allowed_in: &TEAM_MEMBERSHIP_SQL_ALLOWED_IN,
    sql_why: "every team create, rename and delete goes through the store, its lock and the \
              transaction that writes its audit record (team-lifecycle DDD-19)",
    seams: &TEAM_LIFECYCLE_SEAMS,
    seams_forbidden_in: &TEAM_WRITE_SEAMS_FORBIDDEN_IN,
    seam_why: "team lifecycle writes go through the foundry-services use-case, its policy and \
               its one slug mint (team-lifecycle DDD-1, DDD-19)",
    reference: "team-lifecycle DDD-19",
};

/// `team-lifecycle-one-seam` (team-lifecycle DDD-19).
///
/// (i) no SQL `INSERT INTO` / `UPDATE` / `DELETE FROM` on `teams` or
/// `team_lifecycle_events` in any `.rs` under `crates/*/src` except
/// `foundry-store` (and the acceptance harness, whose fixtures seed rows) — a
/// `teams` UPDATE joined to `team_memberships` is a `teams` write (OQ-D7).
///
/// (ii) no [`TEAM_LIFECYCLE_SEAMS`] method has a call site under
/// `crates/{foundry-app,foundry-api}/src`.
///
/// A missing `crates/`, an unreadable directory or file fails the rule.
fn check_team_lifecycle_one_seam(root: &Path) -> Vec<String> {
    scan_one_seam(root, &TEAM_LIFECYCLE_ONE_SEAM)
}

#[cfg(test)]
mod team_name_one_source_tests {
    //! Injected-violation gold tests for `team-name-one-source` (team-lifecycle
    //! DDD-19), the `project_name_one_source_tests` idiom: a staged tree, the
    //! rule run against it, the violations named by `file:line`.

    use super::check_team_name_one_source;

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The copy's one production home, as shipped.
    const CORE_COPY: &str = "#[derive(Debug, thiserror::Error)]\npub enum TeamNameError {\n    \
        #[error(\"Team name must not be empty\")]\n    Empty,\n}\n";

    #[test]
    fn the_copy_in_foundry_core_is_its_one_home() {
        let tree = stage(&[("crates/foundry-core/src/team_name.rs", CORE_COPY)]);
        let violations = check_team_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_second_copy_in_any_adapter_service_or_store_crate_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/team_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nlet m = \"Team name must be unique within the workspace\";\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "/// Trimmed name empty -> 422 \"Team name must not be empty\".\nEmpty,\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\n\nlet _ = format!(\"Team name must {}\", x);\n",
            ),
            (
                "crates/foundry-store/src/teams.rs",
                "#[cfg(test)]\nmod t { const M: &str = \"Team name must be at most 64 characters\"; }\n",
            ),
        ]);
        let violations = check_team_name_one_source(tree.path());
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/teams.rs:1"),
            "a doc comment quoting the copy is a second copy too: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-store/src/teams.rs:2"),
            "an in-file test quoting the copy is flagged; assert via to_string(): {violations:?}"
        );
        assert!(
            violations
                .iter()
                .all(|v| v.starts_with("team-name-one-source: ")),
            "{violations:?}"
        );
    }

    #[test]
    fn expected_strings_in_the_acceptance_suite_and_crate_tests_are_not_flagged() {
        let tree = stage(&[
            ("crates/foundry-core/src/team_name.rs", CORE_COPY),
            (
                "crates/foundry-acceptance/src/steps/feature_team_lifecycle.rs",
                "let m = \"Team name must not be empty\";\n",
            ),
            (
                "crates/foundry-core/tests/team_name.rs",
                "assert_eq!(m, \"Team name must be at most 64 characters\");\n",
            ),
            (
                "crates/foundry-app/tests/teams.rs",
                "let _ = \"Team name must be unique within the workspace\";\n",
            ),
        ]);
        let violations = check_team_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn the_project_and_workspace_copy_are_not_this_rules_business() {
        let tree = stage(&[
            ("crates/foundry-core/src/team_name.rs", CORE_COPY),
            (
                "crates/foundry-app/src/teams.rs",
                "let a = \"Project name must not be empty\";\nlet b = \"Workspace name must not be empty\";\nlet c = \"Team names are listed here\";\n",
            ),
        ]);
        let violations = check_team_name_one_source(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_team_name_one_source(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn the_shipped_tree_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf();
        let core = std::fs::read_to_string(root.join("crates/foundry-core/src/team_name.rs"))
            .expect("core team_name");
        assert!(
            core.contains("\"Team name must"),
            "ANTI-VACUITY: foundry-core is expected to state the copy"
        );
        let violations = check_team_name_one_source(&root);
        assert!(violations.is_empty(), "{violations:?}");
    }
}

#[cfg(test)]
mod team_lifecycle_one_seam_tests {
    //! Injected-violation gold tests for `team-lifecycle-one-seam` (team-lifecycle
    //! DDD-19), the `team_membership_one_seam_tests` idiom: a staged tree, the
    //! rule run against it, the violations named by `file:line`.

    use super::{check_team_lifecycle_one_seam, check_team_membership_one_seam};

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    /// The store's own writes, as shipped: the one place SQL may write the tables.
    const STORE_TEAMS: &str = "pub async fn create_team_with_lead(&self) {\n    \
        sqlx::query(\"INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)\");\n    \
        sqlx::query(\"INSERT INTO team_lifecycle_events (id, kind) VALUES ($1, 'created')\");\n}\n";

    // ---- clause (i): SQL writing teams / team_lifecycle_events lives only in foundry-store/src

    #[test]
    fn a_team_write_in_store_src_acceptance_or_tests_passes() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-store/src/lib.rs",
                "sqlx::query(\"UPDATE teams SET name = $1 WHERE id = $2\");\n",
            ),
            (
                "crates/foundry-acceptance/src/steps/feature_x.rs",
                "sqlx::query(\"INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')\");\n",
            ),
            (
                "crates/foundry-acceptance/src/support/team_lifecycle.rs",
                "sqlx::query(\"DELETE FROM team_lifecycle_events\");\n",
            ),
            (
                "crates/foundry-store/tests/team_delete.rs",
                "sqlx::query(\"DELETE FROM teams WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-services/tests/team_lifecycle_use_cases.rs",
                "sqlx::query(\"DELETE FROM teams WHERE workspace_id = $1\");\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_team_write_in_any_other_crates_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nsqlx::query(\"INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)\");\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "\n\nlet _ = sqlx::query(r#\"\n    update\n    Teams SET name = 'x'\"#);\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "sqlx::query(\"DELETE   FROM teams WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-realtime/src/lib.rs",
                "fn a() {}\nfn b() {}\nsqlx::query(\"INSERT INTO team_lifecycle_events (id) VALUES ($1)\");\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/teams.rs:4"),
            "letter case and line breaks do not matter to SQL; the verb's line is named: {violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:1"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-realtime/src/lib.rs:3")
                && violations
                    .iter()
                    .any(|v| v.contains("team_lifecycle_events")),
            "the audit table is guarded too: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .all(|v| v.starts_with("team-lifecycle-one-seam: ")),
            "{violations:?}"
        );
    }

    /// OQ-D7: team-membership-one-seam's own counter-example — a `teams` UPDATE
    /// joined to `team_memberships` — is not a membership write, but it IS a
    /// `teams` write, so in app src it is this rule's positive case.
    #[test]
    fn the_team_membership_join_example_is_a_teams_write_outside_the_store() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\n\
                 const JOIN: &str = \"UPDATE teams SET name = $1 FROM team_memberships tm WHERE tm.team_id = teams.id\";\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2")
                && violations[0].contains("writes teams"),
            "{violations:?}"
        );
        let membership = check_team_membership_one_seam(tree.path());
        assert!(
            membership.is_empty(),
            "the JOIN stays a non-violation of team-membership-one-seam: {membership:?}"
        );
    }

    #[test]
    fn a_read_a_comment_or_another_table_outside_the_store_is_not_a_team_write() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "// sqlx::query(\"DELETE FROM teams\")\n\
                 const READ: &str = \"SELECT id FROM teams WHERE slug = $1\";\n\
                 const OTHER: &str = \"DELETE FROM teams_archive WHERE id = $1\";\n\
                 const EVENTS: &str = \"SELECT kind FROM team_lifecycle_events\";\n\
                 const PROJECTS: &str = \"UPDATE projects SET team_id = teams.id FROM teams\";\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    // ---- clause (ii): the store's team-lifecycle writes have no call site in app or api src

    #[test]
    fn a_team_lifecycle_write_called_from_app_or_api_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nlet _ = state.store.create_team_with_lead(ws, name, slug, actor).await;\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "\nlet _ = store\n    .create_team_with_lead\n    (ws, name, slug, actor);\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2")
                && violations[0].contains("`create_team_with_lead`"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "a call split across lines is still a call: {violations:?}"
        );
    }

    /// 02-01: the guarded delete is a seam too — a door calling it directly
    /// skips the `Administer` policy (ADR-TEAM-LIFECYCLE-002 decision 7).
    #[test]
    fn a_team_delete_called_from_app_or_api_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nlet _ = state.store.delete_team_if_empty(ws, team, actor).await;\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "let outcome = store.delete_team_if_empty(ws, team, actor).await?;\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2")
                && violations[0].contains("`delete_team_if_empty`"),
            "{violations:?}"
        );
    }

    /// 04-01: the audited rename is a seam too — a door calling it directly
    /// skips the `Manage` policy and the name rule (ADR-TEAM-LIFECYCLE-003).
    #[test]
    fn a_team_rename_called_from_app_or_api_src_is_flagged() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nlet _ = state.store.rename_team_with_audit(ws, team, actor, name).await;\n",
            ),
            (
                "crates/foundry-services/src/teams.rs",
                "let outcome = store.rename_team_with_audit(ws, team, actor, name).await?;\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2")
                && violations[0].contains("`rename_team_with_audit`"),
            "{violations:?}"
        );
    }

    #[test]
    fn a_team_lifecycle_write_called_from_services_store_or_tests_passes() {
        let tree = stage(&[
            ("crates/foundry-store/src/teams.rs", STORE_TEAMS),
            (
                "crates/foundry-services/src/teams.rs",
                "let created = self.store.create_team_with_lead(ws, &name, &slug, actor).await?;\n",
            ),
            (
                "crates/foundry-store/tests/team_create.rs",
                "store.create_team_with_lead(ws, &name, &slug, actor).await;\n",
            ),
            (
                "crates/foundry-app/tests/teams.rs",
                "store.create_team_with_lead(ws, &name, &slug, actor).await;\n",
            ),
            (
                "crates/foundry-app/src/teams.rs",
                "// the use-case calls create_team_with_lead( behind the policy\n\
                 let r = services.create_team(&principal, &typed).await;\n",
            ),
        ]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_team_lifecycle_one_seam(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn the_shipped_tree_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf();
        let store = std::fs::read_to_string(root.join("crates/foundry-store/src/teams.rs"))
            .expect("store teams");
        assert!(
            store.contains("INSERT INTO teams"),
            "ANTI-VACUITY: the store is expected to write teams"
        );
        let services = std::fs::read_to_string(root.join("crates/foundry-services/src/teams.rs"))
            .expect("services teams");
        assert!(
            services.contains("create_team_with_lead("),
            "ANTI-VACUITY: the services use-case is expected to call the store's create"
        );
        let violations = check_team_lifecycle_one_seam(&root);
        assert!(violations.is_empty(), "{violations:?}");
    }
}

// ---------------------------------------------------------------------------
// store-natural-key-scoped (fix-issue-attachment-scoping RC-2)
// ---------------------------------------------------------------------------

/// The natural keys that are unique only within an owner (`UNIQUE (workspace_id,
/// key_prefix)`, `UNIQUE (team_id, slug)`, `UNIQUE (project_id, slug)`,
/// `UNIQUE (project_id, number)` on issues). `number` is included because every
/// shipped `number` filter already compares `project_id` (or `team_id`, with the
/// project slug) to a bound parameter; an issue number alone names an issue in
/// every project at once.
const NATURAL_KEYS: [&str; 3] = ["key_prefix", "slug", "number"];

/// The owner ids that narrow a natural key to one tenant.
const SCOPE_COLUMNS: [&str; 3] = ["workspace_id", "team_id", "project_id"];

/// The one crate whose SQL the rule judges: every production query lives there.
const NATURAL_KEY_STORE_SRC: &str = "crates/foundry-store/src";

/// `store-natural-key-scoped` (RCA RC-2): every `foundry-store` SQL statement
/// that filters on a natural key (`key_prefix`, `slug`, `number`) also compares
/// a scope column (`workspace_id`, `team_id`, `project_id`) with a bind
/// parameter, in the same `OR`-disjunct and not only inside a subquery. A key
/// wrapped in a call or parentheses (`lower(slug)`, `(slug)`), cast
/// (`slug::text`) or collated (`key_prefix COLLATE "C"`) is still filtered on.
///
/// Each Rust string literal is judged on its own — multi-line raw strings
/// included, `;`-separated statements one at a time. A `format!` or `concat!`
/// fragment is therefore judged as written: a scope the other fragments might
/// add is invisible, so the fragment fails closed. Joining on `project_id`
/// (`ON p.id = i.project_id`) is not a scope: nothing the caller resolved
/// narrows the rows. A missing `crates/` or store `src`, an unreadable
/// directory or file fails the rule.
fn check_store_natural_key_scoped(root: &Path) -> Vec<String> {
    const WHY: &str = "natural keys are unique only within a workspace, team or project, RCA RC-2";
    let crates_dir = root.join("crates");
    if !crates_dir.is_dir() {
        return vec![format!(
            "store-natural-key-scoped: cannot list {} — no store query could be checked \
             for a natural-key filter without a scope column ({WHY}).",
            crates_dir.display()
        )];
    }
    let (sources, unreadable) = files_under(&root.join(NATURAL_KEY_STORE_SRC), "rs", &|_| false);
    let mut violations: Vec<String> = unreadable
        .iter()
        .map(|dir| {
            format!(
                "store-natural-key-scoped: cannot list {} — it could not be checked for a \
                 natural-key filter without a scope column ({WHY}).",
                rel(root, dir)
            )
        })
        .collect();
    for file in &sources {
        let Ok(contents) = std::fs::read_to_string(file) else {
            violations.push(format!(
                "store-natural-key-scoped: cannot read {} ({WHY}).",
                rel(root, file)
            ));
            continue;
        };
        for (first_line, sql) in rust_string_literals(&contents) {
            for (offset, key) in unscoped_natural_key_filters(&sql) {
                violations.push(format!(
                    "store-natural-key-scoped: {}:{} filters on {key} without comparing \
                     workspace_id, team_id or project_id to a bound parameter — {WHY}.",
                    rel(root, file),
                    first_line + offset
                ));
            }
        }
    }
    violations
}

/// The 0-based line offsets, within `sql`, of every natural-key filter that no
/// scope comparison narrows, each with the key it filters on. SQL comments are
/// stripped first; each `;`-separated statement is judged on its own.
///
/// A filter is scoped only when a scope column is compared with a bind
/// parameter in the same `OR`-disjunct of the same parenthesised group as the
/// filter, or of an enclosing group that is not a subquery. So `A AND B OR C`
/// does not let a scope in `C` narrow `A`, and a scope compared only inside an
/// `EXISTS (SELECT …)` narrows the subquery's rows, not the outer ones.
fn unscoped_natural_key_filters(sql: &str) -> Vec<(usize, &'static str)> {
    let code = strip_sql_comments(sql).to_ascii_lowercase();
    let mut found = Vec::new();
    let mut statement_start = 0;
    for statement in code.split(';') {
        let groups = paren_groups(statement);
        let scopes: Vec<usize> = SCOPE_COLUMNS
            .iter()
            .flat_map(|column| parameter_comparisons(statement, column))
            .collect();
        for key in NATURAL_KEYS {
            let mut from = 0;
            while let Some(at) = find_sql_word(statement, key, from) {
                from = at + key.len();
                let unscoped = compared_operand(statement, &groups, at, from)
                    .is_some_and(|operand| !is_scoped(statement, &groups, &scopes, operand));
                if unscoped {
                    let offset = code[..statement_start + at].matches('\n').count();
                    found.push((offset, key));
                }
            }
        }
        statement_start += statement.len() + 1;
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// A parenthesised group of a statement as the byte offsets of its `(` and
/// `)`; `None` is the statement itself.
type SqlGroup = Option<(usize, usize)>;

/// Every matched `(`…`)` pair in `statement`, skipping parentheses inside
/// `'…'` literals and `"…"` identifiers. An unclosed `(` runs to the end.
fn paren_groups(statement: &str) -> Vec<(usize, usize)> {
    let mut open = Vec::new();
    let mut pairs = Vec::new();
    let mut quote: Option<u8> = None;
    for (at, byte) in statement.bytes().enumerate() {
        match (quote, byte) {
            (Some(q), b) if b == q => quote = None,
            (Some(_), _) => {}
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'(') => open.push(at),
            (None, b')') => {
                if let Some(opened) = open.pop() {
                    pairs.push((opened, at));
                }
            }
            _ => {}
        }
    }
    pairs.extend(open.into_iter().map(|opened| (opened, statement.len())));
    pairs
}

/// The innermost group strictly enclosing byte `at`.
fn enclosing_group(groups: &[(usize, usize)], at: usize) -> SqlGroup {
    groups
        .iter()
        .copied()
        .filter(|&(open, close)| open < at && at < close)
        .max_by_key(|&(open, _)| open)
}

/// The byte range of the top-level `OR`-disjunct of `group` that holds `at`.
fn disjunct_around(
    statement: &str,
    groups: &[(usize, usize)],
    group: SqlGroup,
    at: usize,
) -> (usize, usize) {
    let (mut low, mut high) = match group {
        Some((open, close)) => (open + 1, close),
        None => (0, statement.len()),
    };
    let body = &statement[..high];
    let mut from = low;
    while let Some(or) = find_sql_word(body, "or", from) {
        from = or + "or".len();
        if enclosing_group(groups, or) != group {
            continue;
        }
        if or < at {
            low = from;
        } else {
            high = or;
            break;
        }
    }
    (low, high)
}

/// True when a scope comparison at one of `scopes` sits in the disjunct holding
/// the filter whose operand starts at `operand`, in its group or an enclosing
/// group that is not a subquery.
fn is_scoped(statement: &str, groups: &[(usize, usize)], scopes: &[usize], operand: usize) -> bool {
    let mut group = enclosing_group(groups, operand);
    let mut at = operand;
    loop {
        let (low, high) = disjunct_around(statement, groups, group, at);
        let scoped = scopes
            .iter()
            .any(|&scope| low <= scope && scope < high && enclosing_group(groups, scope) == group);
        if scoped {
            return true;
        }
        let Some((open, _)) = group else {
            return false;
        };
        if find_sql_word(statement[open + 1..].trim_start(), "select", 0) == Some(0) {
            return false;
        }
        at = open;
        group = enclosing_group(groups, open);
    }
}

/// Where the natural key spanning `start..end` is compared: the start of the
/// outermost operand holding it — the key itself, `(key)`, or a call such as
/// `lower(key)` — or `None` when the key is only selected, assigned or listed.
fn compared_operand(
    statement: &str,
    groups: &[(usize, usize)],
    start: usize,
    end: usize,
) -> Option<usize> {
    let (mut start, mut end) = (start, end);
    loop {
        if is_compared(statement, start, end) {
            return Some(start);
        }
        let (open, close) = enclosing_group(groups, start)?;
        start = call_start(statement, open);
        end = (close + 1).min(statement.len());
    }
}

/// The start of the call whose argument list opens at `open` (`lower(` →
/// `lower`), or `open` itself for a bare parenthesis.
fn call_start(statement: &str, open: usize) -> usize {
    let name = statement[..open]
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    name.len()
}

/// `text` with any leading `::type` casts and `COLLATE "x"` clauses skipped.
fn skip_operand_decorations(text: &str) -> &str {
    let is_type_char =
        |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']';
    let mut text = text.trim_start();
    loop {
        if let Some(cast) = text.strip_prefix("::") {
            text = cast
                .trim_start()
                .trim_start_matches(is_type_char)
                .trim_start();
        } else if find_sql_word(text, "collate", 0) == Some(0) {
            // A literal judged unescaped still carries `\"` around the name.
            let collation = text["collate".len()..].trim_start();
            let collation = collation.strip_prefix('\\').unwrap_or(collation);
            text = match collation.strip_prefix('"') {
                Some(quoted) => quoted.split_once('"').map_or("", |(_, rest)| rest),
                None => collation.trim_start_matches(is_type_char),
            }
            .trim_start();
        } else {
            return text;
        }
    }
}

/// The statement text before the column word starting at `start`, with any
/// `alias.` qualifier and trailing whitespace removed.
fn before_column(statement: &str, start: usize) -> &str {
    let text = &statement[..start];
    let text = match text.strip_suffix('.') {
        Some(qualified) => {
            qualified.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
        }
        None => text,
    };
    text.trim_end()
}

/// True when `text` ends with the whole SQL word `word` (both lower-case).
fn ends_with_sql_word(text: &str, word: &str) -> bool {
    text.len()
        .checked_sub(word.len())
        .is_some_and(|at| find_sql_word(text, word, at).is_some())
}

/// True when `text` ends with a bind parameter (`$1`, `$12`).
fn ends_with_parameter(text: &str) -> bool {
    let unnumbered = text.trim_end_matches(|c: char| c.is_ascii_digit());
    unnumbered.len() < text.len() && unnumbered.ends_with('$')
}

/// True when `text` starts with a bind parameter (`$1`, `$12`).
fn starts_with_parameter(text: &str) -> bool {
    text.strip_prefix('$')
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// True when the operand spanning `start..end` is compared (`=`, `<>`, `!=`,
/// `<`, `>`, `<=`, `>=`, `~` on either side; `IN`, `NOT …`, `LIKE`, `ILIKE`,
/// `SIMILAR TO`, `BETWEEN`), looking past any `::type` cast or `COLLATE`
/// clause, and is not the target of a `SET` assignment.
fn is_compared(statement: &str, start: usize, end: usize) -> bool {
    let after = skip_operand_decorations(&statement[end..]);
    let before = before_column(statement, start);
    let assigned =
        after.starts_with('=') && (before.ends_with(',') || ends_with_sql_word(before, "set"));
    if assigned {
        return false;
    }
    let operator_after = after.starts_with(['=', '<', '>', '!', '~'])
        || ["in", "not", "like", "ilike", "similar", "between"]
            .iter()
            .any(|word| find_sql_word(after, word, 0) == Some(0));
    let operator_before = before.ends_with(['=', '<', '>', '~']);
    operator_after || operator_before
}

/// Where `column` is compared with a bind parameter in the statement:
/// `column = $n`, `$n = column`, `column = ANY($n)` or `column IN ($n, …)`. A
/// `SET column = $n` assignment does not count.
fn parameter_comparisons(statement: &str, column: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = find_sql_word(statement, column, from) {
        from = at + column.len();
        if !is_compared(statement, at, from) {
            continue;
        }
        let after = statement[from..].trim_start();
        let right = match after.strip_prefix('=') {
            Some(rest) => {
                let rest = rest.trim_start();
                rest.strip_prefix("any(").unwrap_or(rest).trim_start()
            }
            None if find_sql_word(after, "in", 0) == Some(0) => after["in".len()..]
                .trim_start()
                .trim_start_matches('(')
                .trim_start(),
            None => "",
        };
        let left = before_column(statement, at)
            .strip_suffix('=')
            .is_some_and(|rest| ends_with_parameter(rest.trim_end()));
        if starts_with_parameter(right) || left {
            found.push(at);
        }
    }
    found
}

#[cfg(test)]
mod store_natural_key_scoped_tests {
    //! Injected-violation gold tests for `store-natural-key-scoped` (RCA RC-2):
    //! a staged tree, the rule run against it, the violations named by
    //! `file:line`.

    use super::check_store_natural_key_scoped;

    const STORE_LIB: &str = "crates/foundry-store/src/lib.rs";

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    /// A store method whose single query is `sql`, opening on line 3.
    fn store_query(sql: &str) -> String {
        format!("pub async fn lookup(&self) {{\n    sqlx::query(\n        \"{sql}\",\n    );\n}}\n")
    }

    fn violations_for(sql: &str) -> Vec<String> {
        let tree = stage(&[(STORE_LIB, &store_query(sql))]);
        check_store_natural_key_scoped(tree.path())
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    // ---- violations ---------------------------------------------------------

    /// The pre-01-01 issue lookup: the join compares `project_id`, but no
    /// owner id bound by the caller narrows the prefix to one workspace.
    #[test]
    fn a_lookup_by_key_prefix_and_number_alone_is_flagged_at_file_and_line() {
        let violations = violations_for(
            "SELECT i.id FROM issues i JOIN projects p ON p.id = i.project_id \
             WHERE p.key_prefix = $1 AND i.number = $2",
        );
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .all(|v| v.starts_with("store-natural-key-scoped:")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("key_prefix")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_lookup_by_slug_alone_is_flagged_at_file_and_line() {
        let violations = violations_for("SELECT id, name FROM teams WHERE slug = $1");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("slug")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_reversed_comparison_on_a_slug_is_still_a_filter() {
        let violations = violations_for("SELECT id FROM projects WHERE $1 = p.slug");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// A scope column ASSIGNED is not a scope column filtered on.
    #[test]
    fn assigning_a_scope_column_does_not_scope_the_slug_filter() {
        let violations = violations_for("UPDATE projects SET workspace_id = $1 WHERE slug = $2");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_case_when_slug_in_an_unscoped_statement_is_flagged() {
        let violations = violations_for(
            "UPDATE lanes SET position = CASE WHEN slug = $2 THEN $4 ELSE position END",
        );
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// The literal opens on line 3; the predicate sits on line 6, and that is
    /// the line reported.
    #[test]
    fn a_violation_inside_a_multi_line_raw_string_is_flagged_on_its_own_line() {
        let source = "pub async fn lookup(&self) {\n    sqlx::query(\n        r#\"\n            \
                      SELECT id, name\n              FROM projects\n             WHERE slug = $1\n        \
                      \"#,\n    );\n}\n";
        let tree = stage(&[(STORE_LIB, source)]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:6")),
            "{violations:?}"
        );
    }

    /// The scope a `format!` argument might splice in is invisible to a source
    /// scan, so the literal is judged as written: it fails closed.
    #[test]
    fn a_format_fragment_is_judged_on_its_own_literal_and_fails_closed() {
        let source = "pub async fn lookup(&self, scope: &str) {\n    let sql = format!(\n        \
                      \"SELECT id FROM projects WHERE slug = $1 {scope}\"\n    );\n}\n";
        let tree = stage(&[(STORE_LIB, source)]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// `concat!` joins its literals at compile time, but each literal is judged
    /// on its own: the scope in the second literal does not rescue the first.
    #[test]
    fn a_concat_fragment_is_judged_on_its_own_literal_and_fails_closed() {
        let source =
            "const SQL: &str = concat!(\n    \"SELECT id FROM projects WHERE slug = $2\",\n    \
                      \" AND team_id = $1\",\n);\n";
        let tree = stage(&[(STORE_LIB, source)]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:2")),
            "{violations:?}"
        );
    }

    /// An `OR` beside the natural key defeats the scope: `A AND B OR C` is
    /// `(A AND B) OR C`, so the scope on the far side of the `OR` narrows
    /// nothing on this side.
    #[test]
    fn an_or_beside_the_natural_key_defeats_the_scope_and_is_flagged() {
        let violations = violations_for(
            "SELECT i.id FROM issues i JOIN projects p ON p.id = i.project_id \
             WHERE p.key_prefix = $1 AND i.number = $2 OR i.workspace_id = $3",
        );
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
        let violations = violations_for(
            "SELECT id FROM teams WHERE (workspace_id = $1 AND slug = $2) OR slug = $3",
        );
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// A scope compared only inside a subquery narrows the subquery's rows,
    /// not the rows the natural key selects.
    #[test]
    fn a_scope_only_inside_a_subquery_does_not_scope_the_outer_filter() {
        let violations = violations_for(
            "SELECT id FROM teams WHERE slug = $1 \
             AND EXISTS (SELECT 1 FROM teams t WHERE t.workspace_id = $2)",
        );
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// A natural key wrapped in a call or parentheses, cast, or given a
    /// collation is still the operand being compared.
    #[test]
    fn a_wrapped_cast_or_collated_natural_key_is_still_a_filter() {
        for sql in [
            "SELECT id FROM teams WHERE lower(slug) = $1",
            "SELECT id FROM teams WHERE (slug) = $1",
            "SELECT id FROM teams WHERE slug::text = $1",
            "SELECT id FROM projects WHERE key_prefix COLLATE \\\"C\\\" = $1",
            "SELECT id FROM teams WHERE $1 = lower(t.slug)",
            "SELECT id FROM teams WHERE coalesce(slug, '') = $1",
        ] {
            let violations = violations_for(sql);
            assert!(
                flagged_at(&violations, &format!("{STORE_LIB}:3")),
                "{sql}: {violations:?}"
            );
        }
    }

    /// An issue number is unique only within its project.
    #[test]
    fn a_lookup_by_issue_number_alone_is_flagged() {
        let violations = violations_for("SELECT id FROM issues WHERE number = $1");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("number")),
            "{violations:?}"
        );
        let violations =
            violations_for("SELECT id FROM issues WHERE project_id = $1 AND number = $2");
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    // ---- counter-examples ---------------------------------------------------

    #[test]
    fn a_slug_scoped_by_workspace_id_passes() {
        let violations =
            violations_for("SELECT id, name FROM teams WHERE t.workspace_id = $1 AND t.slug = $2");
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_slug_scoped_by_team_id_passes() {
        let violations = violations_for(
            "SELECT id, name, key_prefix FROM projects WHERE team_id = $1 AND slug = $2",
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_slug_scoped_by_project_id_passes() {
        let violations =
            violations_for("SELECT id FROM lanes WHERE project_id = $1 AND slug = $2 FOR UPDATE");
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_key_prefix_scoped_by_workspace_id_passes() {
        let violations =
            violations_for("SELECT 1 FROM projects WHERE workspace_id = $1 AND key_prefix = $2");
        assert!(violations.is_empty(), "{violations:?}");
    }

    /// The shipped lane move (lanes.rs): the slug is compared inside a CASE,
    /// in a statement scoped by `project_id`.
    #[test]
    fn a_case_when_slug_in_a_project_id_scoped_statement_passes() {
        let violations = violations_for(
            "UPDATE lanes SET position = CASE WHEN slug = $2 THEN $4 ELSE position END \
             WHERE project_id = $1",
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_wrapped_natural_key_scoped_in_its_own_conjunct_passes() {
        for sql in [
            "SELECT id FROM teams WHERE workspace_id = $1 AND lower(slug) = $2",
            "SELECT id FROM teams WHERE workspace_id = $1 AND (slug = $2 OR slug = $3)",
            "SELECT id FROM teams WHERE (workspace_id = $1 AND slug = $2) OR (team_id = $3 AND slug = $4)",
            "SELECT id FROM issues WHERE project_id = (SELECT id FROM projects WHERE workspace_id = $1 AND key_prefix = $2)",
        ] {
            let violations = violations_for(sql);
            assert!(violations.is_empty(), "{sql}: {violations:?}");
        }
    }

    #[test]
    fn listing_a_natural_key_as_a_column_is_not_filtering_on_it() {
        for sql in [
            "INSERT INTO teams (workspace_id, slug, name) VALUES ($1, $2, $3)",
            "INSERT INTO teams (workspace_id, slug) VALUES ($1, $2) ON CONFLICT (workspace_id, slug) DO NOTHING",
            "SELECT json_build_object('slug', p.slug) FROM projects p WHERE p.id = $1",
        ] {
            let violations = violations_for(sql);
            assert!(violations.is_empty(), "{sql}: {violations:?}");
        }
    }

    #[test]
    fn selecting_or_assigning_a_natural_key_is_not_filtering_on_it() {
        let violations = violations_for(
            "SELECT e.id, p.key_prefix, p.slug FROM issue_change_events e \
             JOIN projects p ON p.id = e.project_id ORDER BY e.id",
        );
        assert!(violations.is_empty(), "{violations:?}");
        let violations = violations_for("UPDATE projects SET slug = $2 WHERE id = $1");
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn sql_outside_foundry_store_src_is_not_this_rules_business() {
        let unscoped = store_query("SELECT id FROM projects WHERE key_prefix = $1");
        let tree = stage(&[
            ("crates/foundry-store/src/lib.rs", "pub struct Store;\n"),
            ("crates/foundry-app/src/admin_cli.rs", unscoped.as_str()),
            ("crates/foundry-store/tests/seed.rs", unscoped.as_str()),
            (
                "crates/foundry-acceptance/src/steps/seed.rs",
                unscoped.as_str(),
            ),
        ]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_commented_out_unscoped_query_is_not_flagged() {
        let source = "// sqlx::query(\"SELECT id FROM projects WHERE slug = $1\");\n\
                      pub fn q() -> &'static str {\n    \"SELECT id FROM projects -- WHERE slug = $1\n     \
                      WHERE id = $1\"\n}\n";
        let tree = stage(&[(STORE_LIB, source)]);
        let violations = check_store_natural_key_scoped(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn the_shipped_tree_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf();
        let lanes = std::fs::read_to_string(root.join("crates/foundry-store/src/lanes.rs"))
            .expect("store lanes");
        assert!(
            lanes.contains("WHEN slug = $2"),
            "ANTI-VACUITY: the store's lane move is expected to compare a slug in a CASE"
        );
        let store = std::fs::read_to_string(root.join("crates/foundry-store/src/lib.rs"))
            .expect("store lib");
        assert!(
            store.contains("i.project_id = $1 AND i.number = $2"),
            "ANTI-VACUITY: the store is expected to look issues up by a project-scoped number"
        );
        let violations = check_store_natural_key_scoped(&root);
        assert!(violations.is_empty(), "{violations:?}");
    }
}

#[cfg(test)]
mod store_natural_key_scoped_boundary_tests {
    //! Boundary gold tests for the `store-natural-key-scoped` helpers: the
    //! rule's verdicts on the SQL shapes each helper exists to read, and the
    //! helpers' own byte-offset contracts where a verdict cannot see them.

    use super::{
        check_store_natural_key_scoped, disjunct_around, enclosing_group, ends_with_parameter,
        is_scoped, paren_groups, unscoped_natural_key_filters,
    };

    const STORE_LIB: &str = "crates/foundry-store/src/lib.rs";

    /// The rule's violations for a store whose single query is `sql`, opening
    /// on line 3.
    fn violations_for(sql: &str) -> Vec<String> {
        let tree = tempfile::tempdir().expect("tempdir");
        let path = tree.path().join(STORE_LIB);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let source = format!(
            "pub async fn lookup(&self) {{\n    sqlx::query(\n        \"{sql}\",\n    );\n}}\n"
        );
        std::fs::write(&path, source).expect("write fixture");
        check_store_natural_key_scoped(tree.path())
    }

    fn assert_flagged(sql: &str) {
        let violations = violations_for(sql);
        assert!(
            violations
                .iter()
                .any(|v| v.contains(&format!("{STORE_LIB}:3"))),
            "{sql}: {violations:?}"
        );
    }

    fn assert_passes(sql: &str) {
        let violations = violations_for(sql);
        assert!(violations.is_empty(), "{sql}: {violations:?}");
    }

    // ---- verdicts -----------------------------------------------------------

    /// A key in a later statement is reported on its own line: the offset
    /// counts every earlier statement and its `;`.
    #[test]
    fn a_filter_in_a_later_statement_is_reported_on_its_own_line() {
        let sql = "update teams set name = $1 where id = $2 and workspace_id = $3\n\
                   ;select id from teams where\nslug = $4";
        assert_eq!(unscoped_natural_key_filters(sql), vec![(2, "slug")]);
    }

    /// The scope column on the left of `=`, compared with a parameter on the
    /// right, scopes the filter; a bare `$` or a literal number does not.
    #[test]
    fn a_parameter_on_the_left_of_the_scope_comparison_scopes_the_filter() {
        assert_passes("SELECT id FROM teams WHERE $1 = workspace_id AND slug = $2");
        assert_passes("SELECT id FROM teams WHERE $12 = t.team_id AND slug = $2");
        assert_flagged("SELECT id FROM teams WHERE 5 = workspace_id AND slug = $1");
        assert_flagged("SELECT id FROM teams WHERE $ = workspace_id AND slug = $1");
    }

    /// `IN ($n, …)` scopes; an inequality with a parameter does not.
    #[test]
    fn an_in_list_of_parameters_scopes_and_an_inequality_does_not() {
        assert_passes("SELECT id FROM teams WHERE workspace_id IN ($1, $2) AND slug = $3");
        assert_passes("SELECT id FROM teams WHERE workspace_id in($1) AND slug = $2");
        assert_flagged("SELECT id FROM teams WHERE workspace_id <> $1 AND slug = $2");
        assert_flagged("SELECT id FROM teams WHERE workspace_id IN (1, 2) AND slug = $1");
    }

    /// A key filtered inside a subquery is judged by the subquery's own scope:
    /// the outer query's scope narrows the outer rows only.
    #[test]
    fn an_outer_scope_does_not_scope_a_filter_inside_a_subquery() {
        assert_flagged(
            "SELECT id FROM issues WHERE workspace_id = $1 \
             AND project_id IN (SELECT id FROM projects WHERE slug = $2)",
        );
        assert_passes(
            "SELECT id FROM issues WHERE workspace_id = $1 \
             AND (project_id = $3 OR slug = $2)",
        );
    }

    /// A call named with `_` is still the call wrapping the key, and a cast to
    /// a schema-qualified, underscored or array type is still a cast.
    #[test]
    fn underscored_calls_and_qualified_or_array_casts_still_wrap_the_key() {
        for sql in [
            "SELECT id FROM teams WHERE $1 = my_lower(slug)",
            "SELECT id FROM teams WHERE slug::pg_catalog.text = $1",
            "SELECT id FROM teams WHERE slug::my_text = $1",
            "SELECT id FROM teams WHERE slug::text[] = $1",
        ] {
            assert_flagged(sql);
        }
    }

    // ---- helper contracts ---------------------------------------------------

    #[test]
    fn parentheses_inside_literals_and_quoted_identifiers_open_no_group() {
        assert_eq!(paren_groups("f('(x') y"), vec![(1, 6)]);
        assert_eq!(paren_groups("f(\"(x\") y"), vec![(1, 6)]);
        assert_eq!(paren_groups("f('x\"(') y"), vec![(1, 7)]);
        assert_eq!(paren_groups("a (b (c) d"), vec![(5, 7), (2, 10)]);
    }

    #[test]
    fn a_group_encloses_only_the_bytes_strictly_between_its_parentheses() {
        let groups = [(0, 5), (1, 3)];
        assert_eq!(enclosing_group(&groups, 0), None);
        assert_eq!(enclosing_group(&groups, 2), Some((1, 3)));
        assert_eq!(enclosing_group(&groups, 3), Some((0, 5)));
        assert_eq!(enclosing_group(&groups, 5), None);
    }

    #[test]
    fn a_disjunct_runs_from_after_the_previous_or_to_the_next_one() {
        let grouped = "(a or b)";
        let groups = paren_groups(grouped);
        assert_eq!(disjunct_around(grouped, &groups, Some((0, 7)), 1), (1, 3));
        assert_eq!(disjunct_around("aa or b", &[], None, 6), (5, 7));
        assert_eq!(disjunct_around("a or b", &[], None, 2), (0, 2));
        assert_eq!(disjunct_around("a or b or c", &[], None, 10), (9, 11));
        assert_eq!(disjunct_around("a or b or c", &[], None, 5), (4, 7));
    }

    #[test]
    fn a_scope_at_the_end_of_the_disjunct_is_outside_it() {
        assert!(!is_scoped("a or b", &[], &[2], 0));
        assert!(is_scoped("a or b", &[], &[1], 0));
    }

    #[test]
    fn a_parameter_is_a_dollar_followed_by_digits_at_the_end() {
        for (text, expected) in [
            ("$1", true),
            ("x = $12", true),
            ("$", false),
            ("5", false),
            ("", false),
            ("$1a", false),
        ] {
            assert_eq!(ends_with_parameter(text), expected, "{text:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// project-lifecycle DDD-22 — `project-lifecycle-live-reads` (rule 1),
// `project-lifecycle-write-guard` (rule 2), `project-lifecycle-one-seam`
// (rule 3). All three are registered in `source_violations()`; the gold tests
// below pin each against a staged tree and against the shipped tree.
// ---------------------------------------------------------------------------

/// `project-lifecycle-live-reads` (ADR-PROJECT-LIFECYCLE-001 decision 7).
///
/// Scans every `.rs` under `crates/*/src` except `crates/foundry-acceptance`
/// and `crates/foundry-store/src/project_lifecycle.rs`. In every Rust string
/// literal (normal or raw, multi-line) that holds an SQL statement, the table
/// name `projects` is a violation in any form — bare, quoted, `public.`-qualified,
/// after `FROM` / `JOIN` / `UPDATE`, in a comma join, after a line break, in a
/// sub-select — except `INSERT INTO projects`. A literal holding no statement
/// verb is still SQL where `projects` stands right after `FROM` or `JOIN` (a
/// fragment such as `" FROM projects p"`), while English copy ("a team with
/// projects") stays clear. The literal pieces of every `concat!` / `format!`
/// call are also read joined as one template (rule 2's [`macro_template`]), a
/// violation found only in the join named at the macro's line.
/// `live_projects` and `project_lifecycle_events` never match; comments are not
/// scanned. An exact
/// `"projects"` literal outside the module is a violation too, except in the
/// four reviewed items named by file and enclosing item: `foundry-store`
/// `lib.rs` const `TENANT_TABLES`, `verify_export.rs` const
/// `DIRECT_WORKSPACE_ID_TABLES`, `name_rule_legacy_seam.rs` fn
/// `NameRuleTable::table`, `foundry-app` `admin_cli.rs` const
/// `KNOWN_FOUNDRY_TABLES`.
///
/// Contract the gold tests pin: every violation starts
/// `project-lifecycle-live-reads: ` and names `file:line`, the line on which the
/// offending `projects` (or the exact literal) stands. A missing `crates/` fails
/// the rule.
fn check_project_lifecycle_live_reads(root: &Path) -> Vec<String> {
    let crates_dir = root.join("crates");
    let Ok(entries) = std::fs::read_dir(&crates_dir) else {
        return vec![format!(
            "{LIVE_READS_ID}: cannot list {} — no crate could be checked for a read of the \
             projects base table ({LIVE_READS_REFERENCE}).",
            crates_dir.display()
        )];
    };
    let mut crate_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("src").is_dir())
        .filter(|path| !path.ends_with(LIVE_READS_SKIPPED_CRATE))
        .collect();
    crate_dirs.sort();

    let mut violations = Vec::new();
    for crate_dir in crate_dirs {
        let (sources, unreadable) = files_under(&crate_dir.join("src"), "rs", &|_| false);
        violations.extend(unreadable.iter().map(|dir| {
            format!(
                "{LIVE_READS_ID}: cannot list {} ({LIVE_READS_REFERENCE}).",
                rel(root, dir)
            )
        }));
        for file in sources {
            let file_rel = rel(root, &file);
            if file_rel == LIVE_READS_MODULE {
                continue;
            }
            match std::fs::read_to_string(&file) {
                Ok(contents) => violations.extend(live_reads_violations_in(&file_rel, &contents)),
                Err(_) => violations.push(format!(
                    "{LIVE_READS_ID}: cannot read {file_rel} ({LIVE_READS_REFERENCE})."
                )),
            }
        }
    }
    violations
}

const LIVE_READS_ID: &str = "project-lifecycle-live-reads";
const LIVE_READS_MODULE: &str = "crates/foundry-store/src/project_lifecycle.rs";
const LIVE_READS_SKIPPED_CRATE: &str = "foundry-acceptance";
const LIVE_READS_TABLE: &str = "projects";
const LIVE_READS_REFERENCE: &str = "ADR-PROJECT-LIFECYCLE-001: outside \
     crates/foundry-store/src/project_lifecycle.rs, read projects through the live_projects view";

/// The four reviewed items that may hold an exact `"projects"` literal, each
/// named by file and by its enclosing item path (never by line).
const LIVE_READS_EXEMPT_ITEMS: [(&str, &[(&str, &str)]); 4] = [
    (
        "crates/foundry-store/src/lib.rs",
        &[("const", "TENANT_TABLES")],
    ),
    (
        "crates/foundry-store/src/verify_export.rs",
        &[("const", "DIRECT_WORKSPACE_ID_TABLES")],
    ),
    (
        "crates/foundry-store/src/name_rule_legacy_seam.rs",
        &[("impl", "NameRuleTable"), ("fn", "table")],
    ),
    (
        "crates/foundry-app/src/admin_cli.rs",
        &[("const", "KNOWN_FOUNDRY_TABLES")],
    ),
];

/// The rule-1 violations in one file (`file_rel` is root-relative): every SQL
/// literal naming the projects table other than as an `INSERT INTO` target,
/// and every exact `"projects"` literal outside the file's exempt items.
fn live_reads_violations_in(file_rel: &str, contents: &str) -> Vec<String> {
    let source = RustSource::scan(contents);
    let exempt_spans: Vec<(usize, usize)> = LIVE_READS_EXEMPT_ITEMS
        .iter()
        .filter(|(file, _)| *file == file_rel)
        .flat_map(|(_, path)| item_spans(&source.code, path))
        .collect();
    let mut violations = Vec::new();
    for literal in &source.literals {
        if literal.text == LIVE_READS_TABLE {
            if !exempt_spans
                .iter()
                .any(|&(start, end)| start <= literal.start && literal.end <= end)
            {
                violations.push(format!(
                    "{LIVE_READS_ID}: {file_rel}:{} holds the exact literal \"projects\" outside \
                     the four reviewed exempt items ({LIVE_READS_REFERENCE}).",
                    literal.line
                ));
            }
            continue;
        }
        violations.extend(
            projects_reads_in(&literal.text)
                .into_iter()
                .map(|offset| live_reads_violation(file_rel, literal.line + offset)),
        );
    }
    for call in macro_calls(&source) {
        let flagged_piece = call
            .parts
            .iter()
            .any(|part| part.text == LIVE_READS_TABLE || !projects_reads_in(&part.text).is_empty());
        if !flagged_piece {
            violations.extend(
                projects_reads_in(&call.joined)
                    .into_iter()
                    .map(|_| live_reads_violation(file_rel, call.line)),
            );
        }
    }
    violations
}

fn live_reads_violation(file_rel: &str, line: usize) -> String {
    format!(
        "{LIVE_READS_ID}: {file_rel}:{line} names the projects base table in SQL \
         ({LIVE_READS_REFERENCE})."
    )
}

/// The 0-based line offsets, within one literal, of every mention of the
/// `projects` table in SQL, in any lexical form, that is not an
/// `INSERT INTO projects` target. In a literal holding no SQL statement only a
/// `projects` standing right after `FROM` / `JOIN` is SQL.
fn projects_reads_in(text: &str) -> Vec<usize> {
    let code = strip_sql_comments(&unqualified_sql(text));
    let statement = holds_sql_statement(&code);
    let mut offsets = Vec::new();
    let mut from = 0;
    while let Some(at) = find_sql_word(&code, LIVE_READS_TABLE, from) {
        from = at + LIVE_READS_TABLE.len();
        let in_sql = statement || follows_from_or_join(&code, at);
        if in_sql && sql_write_targeting(&code, at).is_none_or(|write| write.verb != "insert") {
            offsets.push(code[..at].matches('\n').count());
        }
    }
    offsets
}

/// The word just before `at` in lowercased `code` is `from` or `join`.
fn follows_from_or_join(code: &str, at: usize) -> bool {
    matches!(
        code[..at].split_whitespace().next_back(),
        Some("from" | "join")
    )
}

/// True when lowercased `code` holds the shape of an SQL statement: `SELECT`,
/// `INSERT INTO`, `UPDATE … SET`, `DELETE FROM` or `TRUNCATE`. Stricter than
/// [`is_sql`] on purpose: rule 1 reads every crate, whose user-facing copy says
/// "a team with projects" or "soft delete:", and a bare `WITH` or `DELETE` in
/// prose is not a query (a CTE always holds one of the shapes above).
fn holds_sql_statement(code: &str) -> bool {
    let followed_by = |verb: &str, next: &str| {
        let mut from = 0;
        while let Some(at) = find_sql_word(code, verb, from) {
            from = at + verb.len();
            if code[from..].split_whitespace().next() == Some(next) {
                return true;
            }
        }
        false
    };
    find_sql_word(code, "select", 0).is_some()
        || find_sql_word(code, "truncate", 0).is_some()
        || followed_by("insert", "into")
        || followed_by("delete", "from")
        || find_sql_word(code, "update", 0)
            .is_some_and(|at| find_sql_word(code, "set", at).is_some())
}

/// The `[start, end)` spans in `code` (a [`RustSource::code`]) of the item at
/// `path`, each step a `(keyword, name)` nested inside the previous step's
/// span: a `const` / `static` runs to the `;` ending its value, a `fn` /
/// `impl` to its closing brace.
fn item_spans(code: &str, path: &[(&str, &str)]) -> Vec<(usize, usize)> {
    path.iter()
        .fold(vec![(0, code.len())], |spans, &(keyword, name)| {
            spans
                .into_iter()
                .flat_map(|(start, end)| items_within(code, start, end, keyword, name))
                .collect()
        })
}

/// The spans of every `keyword name` item declared within `code[start..end]`.
fn items_within(
    code: &str,
    start: usize,
    end: usize,
    keyword: &str,
    name: &str,
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = start;
    while let Some(at) = find_sql_word(code, name, from).filter(|&at| at < end) {
        from = at + name.len();
        if code[..at].split_whitespace().next_back() != Some(keyword) {
            continue;
        }
        let item_end = if matches!(keyword, "const" | "static") {
            code[from..end].find('=').and_then(|eq| {
                code[from + eq..end]
                    .find(';')
                    .map(|semi| from + eq + semi + 1)
            })
        } else {
            code[from..end]
                .find('{')
                .and_then(|open| matching_brace(code.as_bytes(), from + open))
                .map(|close| close + 1)
        };
        if let Some(item_end) = item_end.filter(|&e| e <= end) {
            spans.push((at, item_end));
        }
    }
    spans
}

/// `project-lifecycle-write-guard` (ADR-PROJECT-LIFECYCLE-002 decision 7).
///
/// In `crates/foundry-store/src`, every function body that writes `issues`,
/// `comments`, `issue_attachments`, `lanes` or `issue_change_events` must call
/// `lock_live_project(` on the transaction that then performs the writes, before
/// the first write (DDD-7: "the guard is the first statement of the writer's
/// transaction"; DISTILL DIST-10 tightened DESIGN's "with or without
/// `.begin()`", since a guard on a pooled connection or an autocommit write
/// releases the row lock before the write lands). "Writes" is an SQL
/// `INSERT INTO` / `UPDATE` / `DELETE FROM` on those tables (rule 1's lexical
/// rules), or a call to a transaction-taking writing helper (a function with a
/// transaction or connection parameter whose body writes, directly or through
/// another such helper; computed in a first pass). The helpers themselves are
/// exempt; a call to a self-transacting function is not a write in the caller.
/// `insert_issue_attempt` is exempt only while its first SQL literal is
/// `UPDATE live_projects`. A `format!` / `concat!` whose literal pieces, joined,
/// write a guarded table is a write at the macro. A DML template with an
/// interpolation in the table position is a violation (DDL and column-list
/// interpolation are out of scope). Allow-list: [`WRITE_GUARD_ALLOW_LIST`],
/// by file and item path.
///
/// The guard's `bool` gates the write: the call is the condition of an
/// `if !…` / `match` whose false path returns early (or of a plain `if` whose
/// block holds the write), or it is bound by `let` and that binding is tested
/// the same way before the first write. A bare `lock_live_project(…).await?;`
/// or an untested binding guards nothing. The gate stands in the block of the
/// first write or in a block enclosing it, never in a branch that does not
/// dominate it. A `const` / `static` whose literal writes a guarded table is a
/// write in every `fn` naming it; such a literal outside any `fn` and any named
/// `const` / `static` is a violation at its `file:line`.
///
/// Contract the gold tests pin: every violation starts
/// `project-lifecycle-write-guard: `, names the file, and names the function as
/// `` `fn name` ``; a `format!`/`concat!` violation names `file:line` of the
/// template. A missing `crates/` or store `src` fails the rule.
fn check_project_lifecycle_write_guard(root: &Path) -> Vec<String> {
    let crates_dir = root.join("crates");
    if std::fs::read_dir(&crates_dir).is_err() {
        return vec![format!(
            "{WRITE_GUARD_ID}: cannot list {} — no store writer could be checked for the \
             live-project guard ({WRITE_GUARD_REFERENCE}).",
            crates_dir.display()
        )];
    }
    let store_src = root.join(WRITE_GUARD_STORE_SRC);
    if !store_src.is_dir() {
        return vec![format!(
            "{WRITE_GUARD_ID}: cannot list {WRITE_GUARD_STORE_SRC} — no store writer could be \
             checked for the live-project guard ({WRITE_GUARD_REFERENCE})."
        )];
    }
    let (sources, unreadable) = files_under(&store_src, "rs", &|_| false);
    let mut violations: Vec<String> = unreadable
        .iter()
        .map(|dir| {
            format!(
                "{WRITE_GUARD_ID}: cannot list {} ({WRITE_GUARD_REFERENCE}).",
                rel(root, dir)
            )
        })
        .collect();
    let mut scanned = Vec::new();
    for file in &sources {
        let Ok(contents) = std::fs::read_to_string(file) else {
            violations.push(format!(
                "{WRITE_GUARD_ID}: cannot read {} ({WRITE_GUARD_REFERENCE}).",
                rel(root, file)
            ));
            continue;
        };
        let source = RustSource::scan(&contents);
        violations.extend(interpolated_dml_tables(&source).into_iter().map(|line| {
            format!(
                "{WRITE_GUARD_ID}: {}:{line} builds an INSERT INTO / UPDATE / DELETE FROM with \
                 an interpolated table — the guard rule cannot see which table it writes; name \
                 the table in the literal ({WRITE_GUARD_REFERENCE}).",
                rel(root, file)
            )
        }));
        scanned.push((rel(root, file), source));
    }
    let mut writing_items = BTreeSet::new();
    for (file, source) in &scanned {
        let (named, loose) = item_writes(source);
        writing_items.extend(named);
        violations.extend(loose.into_iter().map(|line| {
            format!(
                "{WRITE_GUARD_ID}: {file}:{line} writes a project's issues, comments, \
                 attachments, lanes or change events from a literal outside any fn and any \
                 named const or static — the guard rule cannot see which writer runs it \
                 ({WRITE_GUARD_REFERENCE})."
            )
        }));
    }
    let functions: Vec<StoreFn> = scanned
        .iter()
        .flat_map(|(file, source)| StoreFn::all_in(file, source, &writing_items))
        .collect();
    let helpers = writing_helpers(&functions);
    violations.extend(
        functions
            .iter()
            .filter(|function| function.must_hold_the_guard(&helpers))
            .filter(|function| !function.holds_the_guard_across_its_writes(&helpers))
            .map(|function| {
                format!(
                    "{WRITE_GUARD_ID}: {}:{} fn `{}` writes a project's issues, comments, \
                     attachments, lanes or change events without lock_live_project( on the \
                     transaction that performs the write, before its first write — a write \
                     queued behind a project delete would land under the deleted project \
                     ({WRITE_GUARD_REFERENCE}).",
                    function.file, function.line, function.name
                )
            }),
    );
    violations
}

/// The tables a project's writers write under it; each such write holds the
/// live-project guard (DDD-7).
const GUARDED_CHILD_TABLES: [&str; 5] = [
    "issues",
    "comments",
    "issue_attachments",
    "lanes",
    "issue_change_events",
];

/// Writers allow-listed (DDD-22), each named by file and enclosing item path
/// (never by bare name): the tombstone GC (D12) and the writers that create the
/// project they write under, in the same transaction, so no delete can be
/// queued on it — `insert_project`, and the two bootstrap doors that seed the
/// first workspace's project through `seed_initial_workspace` (a helper
/// DESIGN's list did not name).
const WRITE_GUARD_ALLOW_LIST: [(&str, &[(&str, &str)]); 4] = [
    (
        "crates/foundry-store/src/lib.rs",
        &[("impl", "Store"), ("fn", "gc_tombstoned_comments")],
    ),
    (
        "crates/foundry-store/src/lib.rs",
        &[("impl", "Store"), ("fn", "insert_project")],
    ),
    (
        "crates/foundry-store/src/lib.rs",
        &[("impl", "Store"), ("fn", "create_initial_workspace")],
    ),
    (
        "crates/foundry-store/src/lib.rs",
        &[
            ("impl", "Store"),
            ("fn", "claim_bootstrap_and_create_workspace"),
        ],
    ),
];

/// Issue create's attempt, whose counter `UPDATE live_projects` is its guard.
const ISSUE_CREATE_ATTEMPT: &str = "insert_issue_attempt";

const WRITE_GUARD_CALL: &str = "lock_live_project";
const WRITE_GUARD_ID: &str = "project-lifecycle-write-guard";
const WRITE_GUARD_STORE_SRC: &str = "crates/foundry-store/src";
const WRITE_GUARD_REFERENCE: &str =
    "project-lifecycle DDD-7, DDD-22, ADR-PROJECT-LIFECYCLE-002 decision 2, DIST-10";

/// One `fn` of a store source, as rule 2 reads it: where it stands, whether it
/// takes a transaction or connection, and the offsets (in its file's
/// [`RustSource::code`]) of its direct writes, its valid guards and its body.
struct StoreFn {
    file: String,
    name: String,
    line: usize,
    takes_connection: bool,
    body: String,
    body_start: usize,
    direct_writes: Vec<usize>,
    autocommit_write: bool,
    guards: Vec<Guard>,
    first_sql_is_counter: bool,
    allow_listed: bool,
}

impl StoreFn {
    /// Every `fn` with a body in `source`; a literal belongs to the innermost.
    /// A `concat!` / `format!` whose joined pieces write a guarded table (no
    /// one piece doing so) is a direct write at the macro.
    /// A word naming one of `writing_items` (a `const` / `static` whose literal
    /// writes a guarded table) is a direct write where it stands.
    fn all_in(file: &str, source: &RustSource, writing_items: &BTreeSet<String>) -> Vec<StoreFn> {
        let spans = fn_spans(&source.code);
        let templates = macro_calls(source);
        let allowed: Vec<(usize, usize)> = WRITE_GUARD_ALLOW_LIST
            .iter()
            .filter(|(allowed_file, _)| *allowed_file == file)
            .flat_map(|(_, path)| item_spans(&source.code, path))
            .collect();
        spans
            .iter()
            .map(|span| {
                let mut function =
                    StoreFn::read(file, source, &spans, span, &templates, writing_items);
                function.allow_listed = allowed
                    .iter()
                    .any(|&(start, end)| start <= span.body.0 && span.body.1 < end);
                function
            })
            .collect()
    }

    fn read(
        file: &str,
        source: &RustSource,
        spans: &[FnSpan],
        span: &FnSpan,
        templates: &[MacroCall],
        writing_items: &BTreeSet<String>,
    ) -> StoreFn {
        let own = source.literals.iter().filter(|literal| {
            innermost(spans, literal.start).is_some_and(|inner| inner.at == span.at)
        });
        let mut direct_writes = Vec::new();
        let mut autocommit_write = false;
        let mut first_sql_is_counter = None;
        for literal in own {
            let sql = unqualified_sql(&literal.text);
            if first_sql_is_counter.is_none() && is_sql(&sql) {
                first_sql_is_counter = Some(is_counter_update(&sql));
            }
            if writes_a_guarded_table(&literal.text) {
                direct_writes.push(literal.start);
                autocommit_write |= runs_on_the_pool(&source.code, literal.end);
            }
        }
        for call in templates {
            let own_call = innermost(spans, call.at).is_some_and(|inner| inner.at == span.at);
            let piece_writes = call
                .parts
                .iter()
                .any(|part| writes_a_guarded_table(&part.text));
            if own_call && !piece_writes && writes_a_guarded_table(&call.joined) {
                direct_writes.push(call.at);
                autocommit_write |= runs_on_the_pool(&source.code, call.close);
            }
        }
        let params = &source.code[span.params.0..=span.params.1];
        let body = &source.code[span.body.0..=span.body.1];
        for item in writing_items {
            let mut from = 0;
            while let Some(at) = find_sql_word(body, item, from) {
                from = at + item.len();
                if innermost(spans, span.body.0 + at).is_some_and(|inner| inner.at == span.at) {
                    direct_writes.push(span.body.0 + at);
                    autocommit_write |= runs_on_the_pool(&source.code, span.body.0 + from);
                }
            }
        }
        StoreFn {
            file: file.to_string(),
            name: span.name.clone(),
            line: span.line,
            takes_connection: params.contains("Transaction") || params.contains("Connection"),
            body: body.to_string(),
            body_start: span.body.0,
            direct_writes,
            autocommit_write,
            guards: call_offsets(body, WRITE_GUARD_CALL)
                .into_iter()
                .filter(|&at| holds_the_transaction(body, at))
                .filter_map(|at| Guard::gating(body, at))
                .map(|guard| guard.shifted(span.body.0))
                .collect(),
            first_sql_is_counter: first_sql_is_counter.unwrap_or(false),
            allow_listed: false,
        }
    }

    fn is_helper(&self, helpers: &BTreeSet<String>) -> bool {
        self.takes_connection && helpers.contains(&self.name)
    }

    /// Every writer that is not exempt.
    fn must_hold_the_guard(&self, helpers: &BTreeSet<String>) -> bool {
        !self.is_exempt(helpers) && !self.writes(helpers).is_empty()
    }

    /// A transaction-taking helper (its callers are in scope), the allow-list,
    /// and issue create's attempt while its first SQL is the counter
    /// `UPDATE live_projects`.
    fn is_exempt(&self, helpers: &BTreeSet<String>) -> bool {
        let counter_guarded = self.name == ISSUE_CREATE_ATTEMPT && self.first_sql_is_counter;
        self.is_helper(helpers) || self.allow_listed || counter_guarded
    }

    /// The offsets of every write: a direct write, or a call of a writing helper.
    fn writes(&self, helpers: &BTreeSet<String>) -> Vec<usize> {
        let mut writes = self.direct_writes.clone();
        for helper in helpers {
            writes.extend(
                call_offsets(&self.body, helper)
                    .into_iter()
                    .map(|at| self.body_start + at),
            );
        }
        writes
    }

    /// A guard on the transaction gates the first write — it precedes it, in
    /// its block or one enclosing it — and no write runs in autocommit on the
    /// pool.
    fn holds_the_guard_across_its_writes(&self, helpers: &BTreeSet<String>) -> bool {
        let first_write = self.writes(helpers).into_iter().min();
        !self.autocommit_write
            && first_write.is_some_and(|write| self.guards.iter().any(|guard| guard.gates(write)))
    }
}

/// One guard whose `bool` gates what follows: `at`, the offset of the test of
/// the result (the `if` / `match` taking the call or its binding), and `scope`,
/// the `[open, close]` braces of the block the guard protects — the block the
/// test stands in when its false path returns early, else the block run only
/// on `true`.
struct Guard {
    at: usize,
    scope: (usize, usize),
}

impl Guard {
    /// The gate the guard call at `at` in `body` feeds: the call is the
    /// condition of an `if` / `match`, or bound by `let` and the binding is then
    /// tested. A bare call statement, or an untested binding, gates nothing.
    fn gating(body: &str, at: usize) -> Option<Guard> {
        let open = body[at..].find('(').map(|i| at + i)?;
        let close = matching_brace(body.as_bytes(), open)?;
        let (head, head_at) = statement_head(body, at);
        if let Some(test) = GuardTest::read(&head, head_at) {
            return test.gate(body, close);
        }
        let name = let_bound_name(&head)?;
        let statement_end = body[close..].find(';').map(|i| close + i)?;
        let mut from = statement_end;
        while let Some(use_at) = find_sql_word(body, &name, from) {
            from = use_at + name.len();
            let (head, head_at) = statement_head(body, use_at);
            if let Some(test) = GuardTest::read(&head, head_at) {
                return test.gate(body, from);
            }
        }
        None
    }

    fn shifted(self, by: usize) -> Guard {
        Guard {
            at: self.at + by,
            scope: (self.scope.0 + by, self.scope.1 + by),
        }
    }

    /// The write at `write` runs after this guard, inside the block it protects.
    fn gates(&self, write: usize) -> bool {
        self.at < write && self.scope.0 < write && write < self.scope.1
    }
}

/// The `if` / `if !` / `match` that takes a guard's result as its condition.
struct GuardTest {
    at: usize,
    keyword: &'static str,
    negated: bool,
}

impl GuardTest {
    /// The test whose condition opens with the text after `head` (the
    /// statement text before the call or binding, starting at `head_at`).
    fn read(head: &str, head_at: usize) -> Option<GuardTest> {
        let words: String = head.split_whitespace().collect::<Vec<_>>().join(" ");
        let (keyword, negated) = match words.as_str() {
            "if" => ("if", false),
            "if !" | "if!" => ("if", true),
            "match" => ("match", false),
            _ => return None,
        };
        let at = head_at + head.find(keyword)?;
        Some(GuardTest {
            at,
            keyword,
            negated,
        })
    }

    /// The guard this test makes, its condition ending at `condition_end`: the
    /// test's own block when the false path returns early, else the block run
    /// only on `true` (a plain `if`).
    fn gate(&self, body: &str, condition_end: usize) -> Option<Guard> {
        let bytes = body.as_bytes();
        let then_open = body[condition_end..].find('{').map(|i| condition_end + i)?;
        let then_close = matching_brace(bytes, then_open)?;
        let returns = |(open, close): (usize, usize)| {
            find_sql_word(&body[open..=close], "return", 0).is_some()
        };
        let enclosing = enclosing_block(body, self.at)?;
        if self.keyword == "match" {
            return returns((then_open, then_close)).then_some(Guard {
                at: self.at,
                scope: enclosing,
            });
        }
        let after = &body[then_close + 1..];
        let else_block = if after.trim_start().starts_with("else") {
            after
                .find('{')
                .map(|i| then_close + 1 + i)
                .and_then(|open| matching_brace(bytes, open).map(|close| (open, close)))
        } else {
            None
        };
        let then_block = (then_open, then_close);
        let (on_true, on_false) = if self.negated {
            (else_block, Some(then_block))
        } else {
            (Some(then_block), else_block)
        };
        if on_false.is_some_and(returns) {
            return Some(Guard {
                at: self.at,
                scope: enclosing,
            });
        }
        on_true.map(|scope| Guard { at: self.at, scope })
    }
}

/// The statement text before `at` (from the last `;`, `{` or `}`), with the
/// path qualifying a call at `at` dropped, and the offset it starts at.
fn statement_head(body: &str, at: usize) -> (String, usize) {
    let start = body[..at].rfind([';', '{', '}']).map_or(0, |i| i + 1);
    let mut head = body[start..at].trim_end();
    while let Some(path) = head.strip_suffix("::") {
        let ident = path.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
        head = ident.trim_end();
    }
    (head.to_string(), start)
}

/// The name a `let [mut] name [: Type] =` head binds; `_` binds nothing.
fn let_bound_name(head: &str) -> Option<String> {
    let rest = head.trim().strip_prefix("let ")?.trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (head.trim_end().ends_with('=') && !name.is_empty() && name != "_").then_some(name)
}

/// The `[open, close]` braces of the innermost block of `body` holding `at`.
fn enclosing_block(body: &str, at: usize) -> Option<(usize, usize)> {
    let mut opens = Vec::new();
    for (offset, byte) in body.bytes().enumerate().take(at) {
        match byte {
            b'{' => opens.push(offset),
            b'}' => {
                opens.pop();
            }
            _ => {}
        }
    }
    let open = *opens.last()?;
    matching_brace(body.as_bytes(), open).map(|close| (open, close))
}

/// The literal writes (`INSERT INTO` / `UPDATE` / `DELETE FROM`) one of
/// [`GUARDED_CHILD_TABLES`].
fn writes_a_guarded_table(text: &str) -> bool {
    let sql = unqualified_sql(text);
    GUARDED_CHILD_TABLES
        .iter()
        .any(|table| !sql_writes_of(&sql, table).is_empty())
}

/// The literals of `source` outside every `fn` body that write a guarded
/// table: the names of the `const` / `static` items holding them, and the
/// 1-based lines of those held by no named item.
fn item_writes(source: &RustSource) -> (Vec<String>, Vec<usize>) {
    let code = source.code.as_str();
    let spans = fn_spans(code);
    let items = named_items(code);
    let mut names = Vec::new();
    let mut loose = Vec::new();
    for literal in &source.literals {
        if innermost(&spans, literal.start).is_some() || !writes_a_guarded_table(&literal.text) {
            continue;
        }
        match items
            .iter()
            .find(|(_, start, end)| *start <= literal.start && literal.end <= *end)
        {
            Some((name, _, _)) => names.push(name.clone()),
            None => loose.push(literal.line),
        }
    }
    (names, loose)
}

/// Every `const` / `static` item of `code`: its name and its `[start, end)`
/// span, by [`items_within`].
fn named_items(code: &str) -> Vec<(String, usize, usize)> {
    let mut items = Vec::new();
    for keyword in ["const", "static"] {
        let mut from = 0;
        while let Some(at) = find_sql_word(code, keyword, from) {
            from = at + keyword.len();
            let rest = code[from..].trim_start();
            let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() || name == "fn" {
                continue;
            }
            items.extend(
                items_within(code, at, code.len(), keyword, &name)
                    .into_iter()
                    .take(1)
                    .map(|(start, end)| (name.clone(), start, end)),
            );
        }
    }
    items
}

/// The transaction-taking writing helpers, by name: a `fn` with a transaction
/// or connection parameter whose body writes, directly or through another
/// such helper (a fixpoint over every store `fn`).
fn writing_helpers(functions: &[StoreFn]) -> BTreeSet<String> {
    let mut helpers = BTreeSet::new();
    loop {
        let found: BTreeSet<String> = functions
            .iter()
            .filter(|function| function.takes_connection)
            .filter(|function| !function.writes(&helpers).is_empty())
            .map(|function| function.name.clone())
            .collect();
        if found == helpers {
            return helpers;
        }
        helpers = found;
    }
}

/// One `fn` item of a [`RustSource::code`]: its name, the line and offset of
/// `fn`, and the `[open, close]` offsets of its parameter list and body.
struct FnSpan {
    name: String,
    at: usize,
    line: usize,
    params: (usize, usize),
    body: (usize, usize),
}

/// Every `fn` with a body; a bodiless trait method or a `fn(…)` pointer type
/// is skipped.
fn fn_spans(code: &str) -> Vec<FnSpan> {
    let bytes = code.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(at) = find_sql_word(code, "fn", from) {
        from = at + 2;
        let name_at = from + (code[from..].len() - code[from..].trim_start().len());
        let name_len = bytes[name_at..]
            .iter()
            .take_while(|&&b| is_ident(b))
            .count();
        if name_len == 0 {
            continue;
        }
        let mut cursor = name_at + name_len;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'<') {
            let mut depth = 0i32;
            while let Some(&b) = bytes.get(cursor) {
                cursor += 1;
                depth += i32::from(b == b'<') - i32::from(b == b'>');
                if depth == 0 {
                    break;
                }
            }
        }
        let Some(open) = code[cursor..].find('(').map(|i| cursor + i) else {
            continue;
        };
        let Some(close) = matching_brace(bytes, open) else {
            continue;
        };
        let Some(body_open) = code[close..]
            .find(['{', ';'])
            .map(|i| close + i)
            .filter(|&i| bytes[i] == b'{')
        else {
            continue;
        };
        let Some(body_close) = matching_brace(bytes, body_open) else {
            continue;
        };
        spans.push(FnSpan {
            name: code[name_at..name_at + name_len].to_string(),
            at,
            line: code[..at].matches('\n').count() + 1,
            params: (open, close),
            body: (body_open, body_close),
        });
    }
    spans
}

/// The innermost `fn` whose body holds `offset`.
fn innermost(spans: &[FnSpan], offset: usize) -> Option<&FnSpan> {
    spans
        .iter()
        .filter(|span| span.body.0 < offset && offset < span.body.1)
        .min_by_key(|span| span.body.1 - span.body.0)
}

/// An SQL literal with its quoting and schema dropped, so `"lanes"`,
/// `public.lanes` and `\"lanes\"` read as `lanes` (rule 1's lexical forms).
fn unqualified_sql(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace(['"', '\\'], " ")
        .replace("public.", "")
}

fn is_sql(sql: &str) -> bool {
    ["select", "insert", "update", "delete", "with"]
        .iter()
        .any(|verb| find_sql_word(sql, verb, 0).is_some())
}

/// Issue create's guard: the statement opens `UPDATE live_projects`.
fn is_counter_update(sql: &str) -> bool {
    let code = strip_sql_comments(sql);
    let words: Vec<&str> = code.split_whitespace().take(2).collect();
    words == ["update", "live_projects"]
}

/// The statement a literal ends in runs on the pool (autocommit), not on the
/// writer's transaction.
fn runs_on_the_pool(code: &str, literal_end: usize) -> bool {
    let rest = &code[literal_end.min(code.len())..];
    rest[..rest.find(';').unwrap_or(rest.len())].contains("pool")
}

/// The guard call at `at` takes the writer's transaction, never a connection or
/// transaction of its own from the pool.
fn holds_the_transaction(body: &str, at: usize) -> bool {
    let Some(open) = body[at..].find('(').map(|i| at + i) else {
        return false;
    };
    matching_brace(body.as_bytes(), open).is_some_and(|close| !body[open..close].contains("pool"))
}

/// The 1-based lines of every `format!` / `concat!` template that builds an
/// `INSERT INTO`, `UPDATE` or `DELETE FROM` with an interpolation where the
/// table stands. DDL and an interpolated column list are out of scope.
fn interpolated_dml_tables(source: &RustSource) -> Vec<usize> {
    let mut lines: Vec<usize> = macro_calls(source)
        .into_iter()
        .filter(|call| {
            let template = if call.joins_arguments {
                call.joined.clone()
            } else {
                call.parts
                    .first()
                    .map(|l| l.text.clone())
                    .unwrap_or_default()
            };
            dml_table_is_interpolated(&template)
        })
        .map(|call| call.line)
        .collect();
    lines.sort_unstable();
    lines
}

/// One `format!` / `concat!` call: its offset and closing paren in
/// [`RustSource::code`], the 1-based line naming it, its literal pieces in
/// order, and those pieces joined by [`macro_template`].
struct MacroCall<'a> {
    at: usize,
    close: usize,
    line: usize,
    joins_arguments: bool,
    parts: Vec<&'a RustLiteral>,
    joined: String,
}

/// Every `format!` then every `concat!` call in `source`.
fn macro_calls(source: &RustSource) -> Vec<MacroCall<'_>> {
    let code = source.code.as_str();
    let mut calls = Vec::new();
    for (mac, joins_arguments) in [("format!", false), ("concat!", true)] {
        for at in call_offsets(code, mac) {
            let Some(open) = code[at..].find('(').map(|i| at + i) else {
                continue;
            };
            let Some(close) = matching_brace(code.as_bytes(), open) else {
                continue;
            };
            let parts: Vec<&RustLiteral> = source
                .literals
                .iter()
                .filter(|literal| open < literal.start && literal.end <= close)
                .collect();
            calls.push(MacroCall {
                at,
                close,
                line: code[..at].matches('\n').count() + 1,
                joins_arguments,
                joined: macro_template(code, open, close, &parts),
                parts,
            });
        }
    }
    calls
}

/// A `concat!` / `format!` call as one template: its literals in order, `{}` for every
/// argument between them that is not a literal.
fn macro_template(code: &str, open: usize, close: usize, parts: &[&RustLiteral]) -> String {
    let is_argument = |gap: &str| gap.chars().any(|c| !c.is_whitespace() && c != ',');
    let mut template = String::new();
    let mut cursor = open + 1;
    for part in parts {
        if is_argument(&code[cursor..part.start]) {
            template.push_str("{}");
        }
        template.push_str(&part.text);
        cursor = part.end;
    }
    if is_argument(&code[cursor..close]) {
        template.push_str("{}");
    }
    template
}

fn dml_table_is_interpolated(template: &str) -> bool {
    let sql = unqualified_sql(template);
    let words: Vec<&str> = sql.split_whitespace().collect();
    words.iter().enumerate().any(|(i, word)| {
        let target = match *word {
            "update" => words.get(i + 1),
            "insert" if words.get(i + 1) == Some(&"into") => words.get(i + 2),
            "delete" if words.get(i + 1) == Some(&"from") => words.get(i + 2),
            _ => None,
        };
        target.is_some_and(|table| table.starts_with('{') && !table.starts_with("{{"))
    })
}

/// The store's project-lifecycle writes, which no door may call: each goes
/// through its `foundry-services` use-case: the audited rename, the soft
/// delete and the operator's restore.
const PROJECT_LIFECYCLE_SEAMS: [&str; 3] = [
    "rename_project_with_audit",
    "soft_delete_project",
    "restore_deleted_project",
];

const PROJECT_LIFECYCLE_ONE_SEAM: OneSeamRule = OneSeamRule {
    id: "project-lifecycle-one-seam",
    tables: &["projects", "live_projects", "project_lifecycle_events"],
    sql_allowed_in: &TEAM_MEMBERSHIP_SQL_ALLOWED_IN,
    sql_why: "every project write goes through the store, its lock and the transaction that \
              writes its lifecycle record (project-lifecycle DDD-22)",
    seams: &PROJECT_LIFECYCLE_SEAMS,
    seams_forbidden_in: &TEAM_WRITE_SEAMS_FORBIDDEN_IN,
    seam_why: "project lifecycle writes go through the foundry-services use-case and its \
               policy (project-lifecycle DDD-10, DDD-22)",
    reference: "project-lifecycle DDD-22",
};

/// `project-lifecycle-one-seam` (project-lifecycle DDD-22 rule 3).
///
/// (i) no SQL `INSERT INTO` / `UPDATE` / `DELETE FROM` on `projects`,
/// `live_projects` or `project_lifecycle_events` in any `.rs` under
/// `crates/*/src` except `foundry-store` (and the acceptance harness, whose
/// fixtures seed rows).
///
/// (ii) no [`PROJECT_LIFECYCLE_SEAMS`] method has a call site under
/// `crates/{foundry-app,foundry-api}/src`.
///
/// A missing `crates/`, an unreadable directory or file fails the rule.
fn check_project_lifecycle_one_seam(root: &Path) -> Vec<String> {
    scan_one_seam(root, &PROJECT_LIFECYCLE_ONE_SEAM)
}

#[cfg(test)]
mod project_lifecycle_check_arch_tests {
    //! Injected-violation gold tests for the three project-lifecycle rules
    //! (DESIGN [REF] check-arch Rules, DDD-22), the `team_lifecycle_one_seam_tests`
    //! idiom: a staged tree, the rule run against it, the violations named.

    use super::{
        check_project_lifecycle_live_reads, check_project_lifecycle_one_seam,
        check_project_lifecycle_write_guard, rust_string_literals, RustSource,
    };

    const STORE_LIB: &str = "crates/foundry-store/src/lib.rs";
    const MODULE: &str = "crates/foundry-store/src/project_lifecycle.rs";
    const VERIFY_EXPORT: &str = "crates/foundry-store/src/verify_export.rs";
    const LEGACY_SEAM: &str = "crates/foundry-store/src/name_rule_legacy_seam.rs";
    const ADMIN_CLI: &str = "crates/foundry-app/src/admin_cli.rs";

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    fn flagged_at(violations: &[String], location: &str) -> bool {
        violations.iter().any(|v| v.contains(location))
    }

    fn shipped_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf()
    }

    // =======================================================================
    // Rule 1: project-lifecycle-live-reads
    // =======================================================================

    /// The module: the one place that may name `projects` (deleted rows too).
    const MODULE_BODY: &str = r##"pub(crate) async fn lock_live_project(tx: &mut Tx, project_id: Uuid) -> bool {
    sqlx::query("SELECT 1 FROM projects WHERE id = $1 AND deleted_at IS NULL FOR KEY SHARE");
    true
}
pub async fn soft_delete_project(&self) {
    sqlx::query("UPDATE projects SET deleted_at = clock_timestamp(), deleted_by = $3 WHERE id = $1 AND team_id = $2");
}
pub const EXPORT_PROJECTS: (&str, &str) = (
    "projects",
    "SELECT to_jsonb(t.*) FROM projects t WHERE t.workspace_id = $1",
);
"##;

    /// Three of the four reviewed exact-literal exemptions, each in its own file
    /// and item (`TENANT_TABLES` sits in [`CLEAN_STORE_LIB`]).
    const VERIFY_EXPORT_BODY: &str = r##"const DIRECT_WORKSPACE_ID_TABLES: &[&str] = &["teams", "projects"];
"##;
    const LEGACY_SEAM_BODY: &str = r##"impl NameRuleTable {
    pub fn table(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Projects => "projects",
        }
    }
}
"##;
    const ADMIN_CLI_BODY: &str = r##"const KNOWN_FOUNDRY_TABLES: &[&str] = &[
    "workspaces",
    "projects",
];
"##;

    /// A clean store lib: live reads, records, the one allowed insert, and
    /// comments that quote old SQL.
    const CLEAN_STORE_LIB: &str = r##"// sqlx::query("SELECT id FROM projects WHERE team_id = $1")
/// Reads FROM projects once did; now the view.
pub async fn list_projects_for_workspace(&self) {
    sqlx::query("SELECT p.id FROM live_projects p WHERE p.workspace_id = $1");
    sqlx::query("SELECT kind FROM project_lifecycle_events WHERE workspace_id = $1");
    sqlx::query("INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix) VALUES ($1, $2, $3, $4, $5, $6)");
    sqlx::query("UPDATE live_projects SET next_issue_number = next_issue_number + 1 WHERE id = $1");
    sqlx::query("SELECT id FROM projects_archive WHERE workspace_id = $1");
    /* sqlx::query("UPDATE projects SET name = $1") */
}
pub const TENANT_TABLES: [&str; 3] = [
    "workspaces",
    "projects",
    "issues",
];
const CONSTRAINT: &str = "projects_team_id_fkey";
"##;

    fn clean_tree_files() -> Vec<(&'static str, &'static str)> {
        vec![
            (STORE_LIB, CLEAN_STORE_LIB),
            (MODULE, MODULE_BODY),
            (VERIFY_EXPORT, VERIFY_EXPORT_BODY),
            (LEGACY_SEAM, LEGACY_SEAM_BODY),
            (ADMIN_CLI, ADMIN_CLI_BODY),
            (
                "crates/foundry-app/src/lib.rs",
                r##"fn routes() {
    let _ = Router::new().route("/team/{team_slug}/projects/new", get(new_project));
}
const ROUTE: &str = "/team/{t}/projects";
const LABEL: &str = "Your projects";
"##,
            ),
            (
                "crates/foundry-acceptance/src/support/project_lifecycle.rs",
                r##"sqlx::query("SELECT count(*) FROM projects WHERE deleted_at IS NOT NULL");
const T: &str = "projects";
"##,
            ),
            (
                "crates/foundry-store/tests/project_lifecycle.rs",
                r##"sqlx::query("UPDATE projects SET deleted_at = now() WHERE id = $1");
"##,
            ),
        ]
    }

    /// A store method whose single query is `sql`, opening on line 3.
    fn store_query(sql: &str) -> String {
        format!("pub async fn lookup(&self) {{\n    sqlx::query(\n        \"{sql}\",\n    );\n}}\n")
    }

    #[test]
    fn live_reads_a_clean_tree_with_the_four_exemptions_passes() {
        let tree = stage(&clean_tree_files());
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn live_reads_every_form_of_the_projects_table_in_sql_is_flagged() {
        for (form, sql) in [
            ("after FROM", "SELECT id FROM projects WHERE team_id = $1"),
            (
                "after JOIN",
                "SELECT i.id FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.project_id = $1",
            ),
            ("after UPDATE", "UPDATE projects SET name = $1 WHERE id = $2"),
            ("a delete", "DELETE FROM projects WHERE id = $1"),
            (
                "a comma join",
                "SELECT i.id FROM issues i, projects p WHERE p.id = i.project_id AND i.project_id = $1",
            ),
            ("public.-qualified", "SELECT id FROM public.projects WHERE team_id = $1"),
            ("quoted", r#"SELECT id FROM \"projects\" WHERE team_id = $1"#),
            (
                "quoted and public.-qualified",
                r#"SELECT id FROM public.\"projects\" WHERE team_id = $1"#,
            ),
            ("any letter case", "select id From PROJECTS where team_id = $1"),
            (
                "a sub-select",
                "SELECT id FROM issues WHERE project_id IN (SELECT id FROM projects WHERE team_id = $1)",
            ),
        ] {
            let tree = stage(&[(STORE_LIB, &store_query(sql)), (MODULE, MODULE_BODY)]);
            let violations = check_project_lifecycle_live_reads(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(
                flagged_at(&violations, &format!("{STORE_LIB}:3")),
                "{form}: {violations:?}"
            );
            assert!(
                violations[0].starts_with("project-lifecycle-live-reads: "),
                "{form}: {violations:?}"
            );
        }
    }

    #[test]
    fn live_reads_a_line_break_before_the_table_name_is_flagged_on_the_names_line() {
        let tree = stage(&[
            (
                STORE_LIB,
                "pub async fn lookup(&self) {\n    sqlx::query(\n        r\"SELECT p.id\n            FROM\n            projects p\n           WHERE p.team_id = $1\",\n    );\n}\n",
            ),
            (MODULE, MODULE_BODY),
        ]);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:5")),
            "{violations:?}"
        );
    }

    #[test]
    fn live_reads_the_rule_scans_every_crates_src_not_only_the_store() {
        let mut files = clean_tree_files();
        files.push((
            "crates/foundry-app/src/backup.rs",
            "fn a() {}\nconst COUNT: &str = \"SELECT count(*) FROM projects\";\n",
        ));
        files.push((
            "crates/foundry-api/src/lib.rs",
            "fn a() {}\nfn b() {}\nlet _ = sqlx::query(\"SELECT id FROM projects WHERE team_id = $1\");\n",
        ));
        let tree = stage(&files);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/backup.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "{violations:?}"
        );
    }

    #[test]
    fn live_reads_the_view_the_record_table_and_comments_never_match() {
        let tree = stage(&[
            (
                STORE_LIB,
                r##"pub async fn a(&self) {
    sqlx::query("SELECT id FROM live_projects WHERE team_id = $1 AND slug = $2");
    sqlx::query("UPDATE live_projects SET next_issue_number = next_issue_number + 1 WHERE id = $1");
    sqlx::query("INSERT INTO project_lifecycle_events (id, kind) VALUES ($1, 'renamed')");
    sqlx::query("SELECT i.id FROM issues i JOIN public.live_projects p ON p.id = i.project_id WHERE i.project_id = $1");
    // sqlx::query("DELETE FROM projects WHERE id = $1")
}
"##,
            ),
            (MODULE, MODULE_BODY),
        ]);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn live_reads_an_exact_projects_literal_outside_the_four_exempt_items_is_flagged() {
        let tree = stage(&[
            (
                STORE_LIB,
                r##"pub const TENANT_TABLES: [&str; 3] = [
    "workspaces",
    "projects",
    "issues",
];
const OTHER_TABLES: &[&str] = &["projects"];
pub async fn export_workspace(&self) {
    let tables = [
        (
            "projects",
            "SELECT to_jsonb(t.*) FROM projects t WHERE t.workspace_id = $1",
        ),
    ];
}
"##,
            ),
            (MODULE, MODULE_BODY),
            (
                VERIFY_EXPORT,
                "const DIRECT_WORKSPACE_ID_TABLES: &[&str] = &[\"teams\", \"projects\"];\nconst MORE: &[&str] = &[\"projects\"];\n",
            ),
            (
                LEGACY_SEAM,
                r##"impl NameRuleTable {
    pub fn table(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Projects => "projects",
        }
    }
    pub fn label(self) -> &'static str {
        "projects"
    }
}
"##,
            ),
            (
                ADMIN_CLI,
                r##"const KNOWN_FOUNDRY_TABLES: &[&str] = &[
    "workspaces",
    "projects",
];
fn count_rows() {
    let table = "projects";
}
"##,
            ),
            (
                "crates/foundry-services/src/lib.rs",
                "pub const TENANT_TABLES: &[&str] = &[\"projects\"];\n",
            ),
        ]);
        let violations = check_project_lifecycle_live_reads(tree.path());
        for location in [
            format!("{STORE_LIB}:6"),
            format!("{STORE_LIB}:10"),
            format!("{STORE_LIB}:11"),
            format!("{VERIFY_EXPORT}:2"),
            format!("{LEGACY_SEAM}:9"),
            format!("{ADMIN_CLI}:6"),
            "crates/foundry-services/src/lib.rs:1".to_string(),
        ] {
            assert!(
                flagged_at(&violations, &location),
                "expected {location} flagged: {violations:?}"
            );
        }
        assert_eq!(
            violations.len(),
            7,
            "the four exemptions (lib.rs:3, verify_export.rs:1, NameRuleTable::table, \
             KNOWN_FOUNDRY_TABLES) are not flagged: {violations:?}"
        );
    }

    /// Review F-07: a verbless fragment is still SQL where `projects` stands
    /// right after `FROM` / `JOIN` — the shape a query split across literals
    /// leaves in its tail piece.
    #[test]
    fn live_reads_a_verbless_from_or_join_fragment_is_flagged() {
        for (form, sql) in [
            ("FROM, no verb", " FROM projects p"),
            ("JOIN, no verb", " JOIN projects p ON p.id = i.project_id"),
            ("public.-qualified FROM, no verb", " from public.projects"),
        ] {
            let tree = stage(&[(STORE_LIB, &store_query(sql)), (MODULE, MODULE_BODY)]);
            let violations = check_project_lifecycle_live_reads(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(
                flagged_at(&violations, &format!("{STORE_LIB}:3")),
                "{form}: {violations:?}"
            );
        }
    }

    /// Review F-07: the literal pieces of a `concat!` / `format!` are read
    /// joined, as rule 2 reads a DML template; a read found only in the join is
    /// named at the macro's line.
    #[test]
    fn live_reads_concat_and_format_pieces_are_read_joined() {
        for (form, call) in [
            (
                "concat!, the table in a FROM piece",
                r#"concat!("SELECT name ", "FROM projects")"#,
            ),
            (
                "concat!, the verb and the table in different pieces",
                r#"concat!("UPDATE ", "projects SET name = $1 WHERE id = $2")"#,
            ),
            (
                "format!, the SET in an argument piece",
                r#"format!("UPDATE projects{}", " SET name = $1 WHERE id = $2")"#,
            ),
        ] {
            let body = format!("fn a() {{}}\nfn lookup() {{ let q = {call}; }}\n");
            let tree = stage(&[(STORE_LIB, &body), (MODULE, MODULE_BODY)]);
            let violations = check_project_lifecycle_live_reads(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(
                flagged_at(&violations, &format!("{STORE_LIB}:2")),
                "{form}: {violations:?}"
            );
        }
    }

    /// Review F-07: the shipped user-facing copy naming projects in English
    /// (teams.rs, upgrade_warnings.rs) stays clear under the fragment and
    /// macro-join clauses.
    #[test]
    fn live_reads_english_copy_naming_projects_stays_clear() {
        let mut files = clean_tree_files();
        files.push((
            "crates/foundry-app/src/teams.rs",
            r##"fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
        Self::NoProjectsYet { team } => write!(f, "{team} has no projects yet."),
        Self::HasProjects { team, projects } => write!(
            f,
            "{team} still has {} project(s): {}. A team with projects can't be deleted.",
            projects.len(),
            project_list(projects)
        ),
    }
}
fn notice(team: &str, n: usize) -> String {
    format!("{team} still has {n} project(s): {}. A team with projects can't be deleted.", "x")
}
const EMPTY: &str = "Bakcend has 2 member(s) and no projects. They lose access to Bakcend. This can't be undone.";
"##,
        ));
        files.push((
            "crates/foundry-store/src/upgrade_warnings.rs",
            r##"pub const UPGRADE_WARNING_0026: &str = "Migration 0026 added the live_projects view over \
projects and projects.deleted_by, which references users. A database dump taken before 0026 can \
no longer be restored over this database with pg_restore --clean: restore it into a fresh \
database, or drop live_projects and projects.deleted_by first.";
"##,
        ));
        let tree = stage(&files);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn live_reads_a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert!(
            !violations.is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn live_reads_the_shipped_tree_passes() {
        let root = shipped_root();
        let violations = check_project_lifecycle_live_reads(&root);
        let module = std::fs::read_to_string(root.join(MODULE)).expect("the module exists");
        assert!(
            module.contains("projects"),
            "ANTI-VACUITY: the module is expected to name projects"
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    // =======================================================================
    // Rule 2: project-lifecycle-write-guard
    // =======================================================================

    /// The guard itself, in the module (reads `projects`, writes nothing guarded).
    const GUARD_MODULE: &str = r##"pub(crate) async fn lock_live_project(tx: &mut Transaction<'_, Postgres>, project_id: Uuid) -> Result<bool, StoreError> {
    let row = sqlx::query("SELECT 1 FROM projects WHERE id = $1 AND deleted_at IS NULL FOR KEY SHARE").fetch_optional(&mut **tx).await?;
    Ok(row.is_some())
}
"##;

    /// The shipped shapes, guarded: a direct writer, the transaction-taking
    /// helpers (exempt themselves), their guarded callers, the self-transacting
    /// issue create and its retrying caller, the allow-list, a read-only helper —
    /// inside `impl Store`, where the allow-list's item paths name them.
    const CLEAN_STORE: &str = r##"impl Store {
pub async fn insert_comment_with_outbox(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query("INSERT INTO comments (id, issue_id, body) VALUES ($1, $2, $3)").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_attachment(&self, issue_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    let (project_id,): (Uuid,) = sqlx::query_as("SELECT project_id FROM issues WHERE id = $1").fetch_one(&mut *tx).await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query("INSERT INTO issue_attachments (id, issue_id) VALUES ($1, $2)").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn renumber_column(tx: &mut Transaction<'_, Postgres>, project_id: Uuid) -> Result<(), StoreError> {
    sqlx::query("UPDATE issues SET position = position + 1 WHERE project_id = $1").execute(&mut **tx).await?;
    Ok(())
}

async fn move_cards_to_destination(tx: &mut Transaction<'_, Postgres>, project_id: Uuid) -> Result<(), StoreError> {
    renumber_column(tx, project_id).await
}

async fn record_issue_change(conn: &mut PgConnection, issue_id: Uuid) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO issue_change_events (id, issue_id) VALUES ($1, $2)").execute(&mut *conn).await?;
    Ok(())
}

async fn read_positions(tx: &mut Transaction<'_, Postgres>, project_id: Uuid) -> Result<(), StoreError> {
    sqlx::query("SELECT position FROM issues WHERE project_id = $1").fetch_all(&mut **tx).await?;
    Ok(())
}

pub async fn reposition_issue_with_outbox(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    renumber_column(&mut tx, project_id).await?;
    record_issue_change(&mut tx, project_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_lane(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    move_cards_to_destination(&mut tx, project_id).await?;
    sqlx::query("DELETE FROM lanes WHERE id = $1").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn board_positions(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    read_positions(&mut tx, project_id).await?;
    Ok(())
}

async fn insert_issue_attempt(&self, project_id: Uuid) -> Result<Option<Uuid>, StoreError> {
    let mut tx = self.pool.begin().await?;
    let row = sqlx::query("UPDATE live_projects SET next_issue_number = next_issue_number + 1 WHERE id = $1 RETURNING next_issue_number - 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    sqlx::query("INSERT INTO issues (id, project_id, number) VALUES ($1, $2, $3)").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(row))
}

pub async fn insert_issue_with_outbox(&self, project_id: Uuid) -> Result<Uuid, StoreError> {
    match self.insert_issue_attempt(project_id).await? {
        Some(issue) => Ok(issue),
        None => self.insert_issue_attempt(project_id).await?.ok_or(StoreError::NotFound),
    }
}

pub async fn gc_tombstoned_comments(&self) -> Result<u64, StoreError> {
    sqlx::query("DELETE FROM comments WHERE deleted_at < now() - interval '90 days'").execute(&self.pool).await?;
    Ok(0)
}

pub async fn insert_project(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    sqlx::query("INSERT INTO projects (id) VALUES ($1)").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO lanes (id, project_id) VALUES ($1, $2)").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_issue_field(&self, project_id: Uuid, column: &str) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    let sql = format!("UPDATE issues SET {column} = $1 WHERE id = $2");
    sqlx::query(&sql).execute(&mut *tx).await?;
    Ok(())
}

pub async fn find_user(&self) -> Result<(), StoreError> {
    let sql = format!("SELECT {USER_ROW_COLUMNS} FROM users WHERE id = $1");
    sqlx::query(&sql).fetch_one(&self.pool).await?;
    Ok(())
}

pub async fn the_comment_says(&self) -> Result<(), StoreError> {
    // sqlx::query("DELETE FROM comments WHERE id = $1")
    sqlx::query("SELECT body FROM comments WHERE id = $1").fetch_one(&self.pool).await?;
    Ok(())
}
}
"##;

    const LEGACY_SEAM_DDL: &str = r##"pub async fn seed_row_predating_name_rule(conn: &mut PgConnection, table: NameRuleTable) {
    let t = table.table();
    let sql = format!("ALTER TABLE {t} DISABLE TRIGGER USER");
    sqlx::query(&sql).execute(&mut *conn).await;
}
"##;

    fn guard_tree(extra: &[(&'static str, &'static str)]) -> tempfile::TempDir {
        let mut files = vec![
            (STORE_LIB, CLEAN_STORE),
            (MODULE, GUARD_MODULE),
            (LEGACY_SEAM, LEGACY_SEAM_DDL),
        ];
        files.extend_from_slice(extra);
        stage(&files)
    }

    fn names_writer(violations: &[String], file: &str, function: &str) -> bool {
        violations
            .iter()
            .any(|v| v.contains(file) && v.contains(&format!("`{function}`")))
    }

    #[test]
    fn write_guard_a_clean_store_passes() {
        let tree = guard_tree(&[
            (
                "crates/foundry-store/tests/issue_delete.rs",
                "sqlx::query(\"DELETE FROM issues WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-app/src/admin_cli.rs",
                "fn a() { sqlx::query(\"UPDATE comments SET deleted_at = NULL\"); }\n",
            ),
        ]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn write_guard_an_unguarded_direct_writer_of_each_table_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/attachments.rs";
        for (form, body) in [
            (
                "an autocommit INSERT INTO issue_attachments",
                "pub async fn writer(&self) {\n    sqlx::query(\"INSERT INTO issue_attachments (id, issue_id) VALUES ($1, $2)\").execute(&self.pool).await;\n}\n",
            ),
            (
                "UPDATE issues",
                "pub async fn writer(&self) {\n    let mut tx = self.pool.begin().await?;\n    sqlx::query(\"UPDATE issues SET title = $1 WHERE id = $2\").execute(&mut *tx).await;\n}\n",
            ),
            (
                "DELETE FROM comments",
                "pub async fn writer(&self) {\n    sqlx::query(\"DELETE FROM comments WHERE id = $1\").execute(&self.pool).await;\n}\n",
            ),
            (
                "public.lanes, lower case",
                "pub async fn writer(&self) {\n    sqlx::query(\"insert into public.lanes (id) values ($1)\").execute(&self.pool).await;\n}\n",
            ),
            (
                "quoted issue_change_events",
                "pub async fn writer(&self) {\n    sqlx::query(\"UPDATE \\\"issue_change_events\\\" SET kind = $1\").execute(&self.pool).await;\n}\n",
            ),
            (
                "a line break inside DELETE FROM lanes",
                "pub async fn writer(&self) {\n    sqlx::query(r\"DELETE\n        FROM\n        lanes WHERE id = $1\").execute(&self.pool).await;\n}\n",
            ),
        ] {
            let tree = guard_tree(&[(FILE, body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(names_writer(&violations, FILE, "writer"), "{form}: {violations:?}");
            assert!(
                violations[0].starts_with("project-lifecycle-write-guard: "),
                "{form}: {violations:?}"
            );
        }
    }

    /// DIST-10: the guard must hold across the write (DDD-7, INV-PL-3). A lock
    /// taken on a pooled connection with the write in autocommit, or a write
    /// before the lock in the same transaction, is flagged.
    #[test]
    fn write_guard_a_guard_that_does_not_hold_across_the_write_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        for (form, body) in [
            (
                "a guard on a pooled connection, the write in autocommit",
                r##"pub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {
    if !lock_live_project(&mut self.pool.acquire().await?, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query("UPDATE lanes SET name = $1 WHERE id = $2").execute(&self.pool).await?;
    Ok(())
}
"##,
            ),
            (
                "a write before the guard in the same transaction",
                r##"pub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    sqlx::query("UPDATE lanes SET name = $1 WHERE id = $2").execute(&mut *tx).await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    tx.commit().await?;
    Ok(())
}
"##,
            ),
        ] {
            let tree = guard_tree(&[(FILE, body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert!(
                names_writer(&violations, FILE, "writer"),
                "{form}: {violations:?}"
            );
        }
    }

    #[test]
    fn write_guard_a_caller_of_a_writing_helper_without_the_guard_is_flagged_transitively() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn reorder(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    renumber_column(&mut tx, project_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn collapse_lane(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    move_cards_to_destination(&mut tx, project_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn log_change(&self, issue_id: Uuid) -> Result<(), StoreError> {
    let mut conn = self.pool.acquire().await?;
    record_issue_change(&mut conn, issue_id).await
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(names_writer(&violations, FILE, "reorder"), "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "collapse_lane"),
            "a helper that writes only through another helper is in the set: {violations:?}"
        );
        assert!(
            names_writer(&violations, FILE, "log_change"),
            "a connection parameter makes a helper too: {violations:?}"
        );
    }

    #[test]
    fn write_guard_insert_issue_attempt_with_a_write_before_its_counter_update_is_flagged() {
        let bad_first_insert = CLEAN_STORE.replace(
            "    let row = sqlx::query(\"UPDATE live_projects",
            "    sqlx::query(\"INSERT INTO issue_change_events (id) VALUES ($1)\").execute(&mut *tx).await?;\n    let row = sqlx::query(\"UPDATE live_projects",
        );
        let base_projects = CLEAN_STORE.replace(
            "sqlx::query(\"UPDATE live_projects SET next_issue_number",
            "sqlx::query(\"UPDATE projects SET next_issue_number",
        );
        for (form, store) in [
            ("an INSERT before UPDATE live_projects", bad_first_insert),
            ("the counter on the base table", base_projects),
        ] {
            assert_ne!(store, CLEAN_STORE, "{form}: the fixture edit applied");
            let tree = stage(&[
                (STORE_LIB, store.as_str()),
                (MODULE, GUARD_MODULE),
                (LEGACY_SEAM, LEGACY_SEAM_DDL),
            ]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert!(
                names_writer(&violations, STORE_LIB, "insert_issue_attempt"),
                "{form}: {violations:?}"
            );
            assert!(
                !names_writer(&violations, STORE_LIB, "insert_issue_with_outbox"),
                "{form}: the caller holds no SQL and calls a self-transacting fn: {violations:?}"
            );
        }
    }

    #[test]
    fn write_guard_the_retrying_caller_with_an_inline_issue_insert_is_flagged() {
        let store = CLEAN_STORE.replace(
            "    match self.insert_issue_attempt(project_id).await? {",
            "    sqlx::query(\"INSERT INTO issues (id, project_id, number) VALUES ($1, $2, 0)\").execute(&self.pool).await?;\n    match self.insert_issue_attempt(project_id).await? {",
        );
        assert_ne!(store, CLEAN_STORE, "the fixture edit applied");
        let tree = stage(&[
            (STORE_LIB, store.as_str()),
            (MODULE, GUARD_MODULE),
            (LEGACY_SEAM, LEGACY_SEAM_DDL),
        ]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            names_writer(&violations, STORE_LIB, "insert_issue_with_outbox"),
            "{violations:?}"
        );
    }

    #[test]
    fn write_guard_the_allow_list_is_by_name_only() {
        const FILE: &str = "crates/foundry-store/src/gc.rs";
        let tree = guard_tree(&[(
            FILE,
            "pub async fn purge_old_comments(&self) {\n    sqlx::query(\"DELETE FROM comments WHERE deleted_at < now() - interval '90 days'\").execute(&self.pool).await;\n}\n",
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(
            violations.len(),
            1,
            "gc_tombstoned_comments and insert_project pass; the same body under another name does not: {violations:?}"
        );
        assert!(
            names_writer(&violations, FILE, "purge_old_comments"),
            "{violations:?}"
        );
    }

    #[test]
    fn write_guard_a_dml_template_with_an_interpolated_table_is_flagged_even_when_guarded() {
        const FILE: &str = "crates/foundry-store/src/generic.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn touch(&self, project_id: Uuid, t: &str) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    let a = format!("UPDATE {t} SET position = $1 WHERE id = $2");
    let b = format!("INSERT INTO {} (id) VALUES ($1)", t);
    let c = concat!("DELETE FROM ", TABLE, " WHERE id = $1");
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        for line in [6, 7, 8] {
            assert!(
                flagged_at(&violations, &format!("{FILE}:{line}")),
                "line {line}: {violations:?}"
            );
        }
    }

    /// Review F-04: the guard's `bool` must gate the write. A bare call
    /// statement, a binding never tested (or tested only after the write), and
    /// an `if !` that does not return each guard nothing.
    #[test]
    fn write_guard_a_guard_whose_bool_is_ignored_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        for (form, gate) in [
            (
                "a bare call statement",
                "    crate::project_lifecycle::lock_live_project(&mut tx, project_id).await?;\n    WRITE\n",
            ),
            (
                "bound to _",
                "    let _ = lock_live_project(&mut tx, project_id).await?;\n    WRITE\n",
            ),
            (
                "a binding never tested",
                "    let live = lock_live_project(&mut tx, project_id).await?;\n    WRITE\n",
            ),
            (
                "a binding tested only after the write",
                "    let live = lock_live_project(&mut tx, project_id).await?;\n    WRITE\n    if !live {\n        return Err(StoreError::NotFound);\n    }\n",
            ),
            (
                "an if ! that does not return",
                "    if !lock_live_project(&mut tx, project_id).await? {\n        tracing::warn!(\"gone\");\n    }\n    WRITE\n",
            ),
        ] {
            let body = format!(
                "pub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {{\n    let mut tx = self.pool.begin().await?;\n{}    tx.commit().await?;\n    Ok(())\n}}\n",
                gate.replace(
                    "WRITE",
                    "sqlx::query(\"UPDATE lanes SET name = $1 WHERE id = $2\").execute(&mut *tx).await?;"
                )
            );
            let tree = stage(&[(STORE_LIB, CLEAN_STORE), (MODULE, GUARD_MODULE), (FILE, &body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(names_writer(&violations, FILE, "writer"), "{form}: {violations:?}");
        }
    }

    /// The gating forms the rule accepts: the call (qualified or not) as an
    /// `if !` / `match` condition whose false path returns, a binding tested so
    /// before the write, a plain `if` whose block holds the write, and a plain
    /// `if` whose `else` returns.
    #[test]
    fn write_guard_every_gating_form_of_the_guard_passes() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn bound(&self, project_id: Uuid) -> Result<bool, StoreError> {
    let mut tx = self.pool.begin().await?;
    let live = crate::project_lifecycle::lock_live_project(&mut tx, project_id).await?;
    if !live {
        return Ok(false);
    }
    sqlx::query("UPDATE lanes SET name = $1 WHERE id = $2").execute(&mut *tx).await?;
    Ok(true)
}

pub async fn matched(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    match project_lifecycle::lock_live_project(&mut tx, project_id).await? {
        true => {}
        false => return Err(StoreError::NotFound),
    }
    sqlx::query("DELETE FROM lanes WHERE id = $1").execute(&mut *tx).await?;
    Ok(())
}

pub async fn inside(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if lock_live_project(&mut tx, project_id).await? {
        sqlx::query("INSERT INTO lanes (id) VALUES ($1)").execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(())
}

pub async fn otherwise(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if lock_live_project(&mut tx, project_id).await? {
        tracing::debug!("live");
    } else {
        return Err(StoreError::NotFound);
    }
    sqlx::query("UPDATE issues SET title = $1 WHERE id = $2").execute(&mut *tx).await?;
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    /// Review F-06: a guard inside a branch that does not dominate the first
    /// write protects only that branch.
    #[test]
    fn write_guard_a_guard_in_a_branch_that_does_not_dominate_the_write_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        for (form, gate) in [
            (
                "an if ! guard inside an if",
                "    if strict {\n        if !lock_live_project(&mut tx, project_id).await? {\n            return Err(StoreError::NotFound);\n        }\n    }\n",
            ),
            (
                "an if ! guard inside a match arm",
                "    match mode {\n        Mode::Strict => {\n            if !lock_live_project(&mut tx, project_id).await? {\n                return Err(StoreError::NotFound);\n            }\n        }\n        _ => {}\n    }\n",
            ),
            (
                "a plain if guard whose block ends before the write",
                "    if lock_live_project(&mut tx, project_id).await? {\n        tracing::debug!(\"live\");\n    }\n",
            ),
        ] {
            let body = format!(
                "pub async fn writer(&self, project_id: Uuid, strict: bool) -> Result<(), StoreError> {{\n    let mut tx = self.pool.begin().await?;\n{gate}    sqlx::query(\"UPDATE lanes SET name = $1 WHERE id = $2\").execute(&mut *tx).await?;\n    tx.commit().await?;\n    Ok(())\n}}\n"
            );
            let tree = stage(&[(STORE_LIB, CLEAN_STORE), (MODULE, GUARD_MODULE), (FILE, &body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(names_writer(&violations, FILE, "writer"), "{form}: {violations:?}");
        }
    }

    /// Review F-05: a `const` / `static` literal writing a guarded table is a
    /// write in every `fn` naming it; such a literal held by no fn and no named
    /// item is flagged where it stands.
    #[test]
    fn write_guard_a_const_dml_on_a_guarded_table_is_a_write_in_every_fn_naming_it() {
        const FILE: &str = "crates/foundry-store/src/positions.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"const BUMP_POSITIONS: &str = "UPDATE issues SET position = position + 1 WHERE project_id = $1";
static PURGE_LANE: &str = "DELETE FROM lanes WHERE id = $1";
const READ_POSITIONS: &str = "SELECT position FROM issues WHERE project_id = $1";

pub async fn bump_unguarded(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    sqlx::query(BUMP_POSITIONS).bind(project_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn bump_guarded(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query(BUMP_POSITIONS).bind(project_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn purge(&self, lane_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    sqlx::query(PURGE_LANE).bind(lane_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn read(&self, project_id: Uuid) -> Result<(), StoreError> {
    sqlx::query(READ_POSITIONS).bind(project_id).fetch_all(&self.pool).await?;
    Ok(())
}

macro_rules! renumber_sql {
    () => {
        "UPDATE issues SET number = number + 1 WHERE project_id = $1"
    };
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "bump_unguarded"),
            "{violations:?}"
        );
        assert!(names_writer(&violations, FILE, "purge"), "{violations:?}");
        assert!(
            flagged_at(&violations, &format!("{FILE}:36")),
            "a guarded-table write held by no fn and no named item: {violations:?}"
        );
    }

    /// Review F-21: the allow-list names each writer by file and item path; the
    /// same name in another file, or in another impl, is not exempt.
    #[test]
    fn write_guard_the_allow_list_is_by_file_and_item_path() {
        const FILE: &str = "crates/foundry-store/src/gc.rs";
        let store = format!(
            "{CLEAN_STORE}impl Seeder {{\npub async fn insert_project(&self) {{\n    sqlx::query(\"INSERT INTO lanes (id) VALUES ($1)\").execute(&self.pool).await;\n}}\n}}\n"
        );
        let tree = stage(&[
            (STORE_LIB, store.as_str()),
            (MODULE, GUARD_MODULE),
            (
                FILE,
                "impl Store {\npub async fn gc_tombstoned_comments(&self) {\n    sqlx::query(\"DELETE FROM comments WHERE deleted_at < now() - interval '90 days'\").execute(&self.pool).await;\n}\n}\n",
            ),
        ]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "gc_tombstoned_comments"),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains(STORE_LIB) && v.contains("`insert_project`")),
            "{violations:?}"
        );
    }

    #[test]
    fn write_guard_a_missing_crates_or_store_src_fails_the_rule() {
        let none = stage(&[("README.md", "nothing here\n")]);
        assert!(
            !check_project_lifecycle_write_guard(none.path()).is_empty(),
            "a missing crates directory must FAIL the guard"
        );
        let no_store = stage(&[("crates/foundry-app/src/lib.rs", "fn a() {}\n")]);
        assert!(
            !check_project_lifecycle_write_guard(no_store.path()).is_empty(),
            "a missing foundry-store/src must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn write_guard_the_shipped_tree_passes() {
        let root = shipped_root();
        let violations = check_project_lifecycle_write_guard(&root);
        let module = std::fs::read_to_string(root.join(MODULE)).expect("the module exists");
        assert!(
            module.contains("fn lock_live_project("),
            "ANTI-VACUITY: the module is expected to define the guard"
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    // =======================================================================
    // Rule 3: project-lifecycle-one-seam
    // =======================================================================

    const STORE_LIFECYCLE: &str = r##"pub async fn rename_project_with_audit(&self) {
    sqlx::query("UPDATE live_projects SET name = $1 WHERE id = $2");
    sqlx::query("INSERT INTO project_lifecycle_events (id, kind) VALUES ($1, 'renamed')");
}
pub async fn soft_delete_project(&self) {
    sqlx::query("UPDATE projects SET deleted_at = clock_timestamp() WHERE id = $1");
}
"##;

    #[test]
    fn one_seam_a_project_write_outside_the_store_is_flagged() {
        let tree = stage(&[
            (MODULE, STORE_LIFECYCLE),
            (
                "crates/foundry-acceptance/src/support/project_lifecycle.rs",
                "sqlx::query(\"UPDATE projects SET deleted_at = now() WHERE id = $1\");\n",
            ),
            (
                "crates/foundry-app/src/teams.rs",
                "fn a() {}\nsqlx::query(\"UPDATE projects SET name = $1 WHERE id = $2\");\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "sqlx::query(\"INSERT INTO project_lifecycle_events (id) VALUES ($1)\");\n",
            ),
            (
                "crates/foundry-api/src/lib.rs",
                "fn a() {}\nfn b() {}\nsqlx::query(\"update live_projects set name = $1\");\n",
            ),
            (
                "crates/foundry-app/src/lib.rs",
                "const READ: &str = \"SELECT id FROM live_projects WHERE team_id = $1\";\n",
            ),
        ]);
        let violations = check_project_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/teams.rs:2"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-services/src/projects.rs:1"),
            "{violations:?}"
        );
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3"),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .all(|v| v.starts_with("project-lifecycle-one-seam: ")),
            "{violations:?}"
        );
    }

    #[test]
    fn one_seam_the_audited_rename_called_from_a_door_is_flagged() {
        let tree = stage(&[
            (MODULE, STORE_LIFECYCLE),
            (
                "crates/foundry-app/src/admin.rs",
                "fn a() {}\nlet _ = state.store.rename_project_with_audit(id, actor, name).await;\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "let w = store.rename_project_with_audit(id, actor, name).await?;\n",
            ),
        ]);
        let violations = check_project_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-app/src/admin.rs:2")
                && violations[0].contains("`rename_project_with_audit`"),
            "{violations:?}"
        );
    }

    #[test]
    fn one_seam_the_soft_delete_called_from_a_door_is_flagged() {
        let tree = stage(&[
            (MODULE, STORE_LIFECYCLE),
            (
                "crates/foundry-api/src/lib.rs",
                "\nlet _ = store\n    .soft_delete_project\n    (ws, team, id, actor);\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "let o = store.soft_delete_project(ws, team, id, actor).await?;\n",
            ),
        ]);
        let violations = check_project_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, "crates/foundry-api/src/lib.rs:3")
                && violations[0].contains("`soft_delete_project`"),
            "{violations:?}"
        );
    }

    #[test]
    fn one_seam_the_restore_called_from_the_operator_cli_is_flagged() {
        let tree = stage(&[
            (MODULE, STORE_LIFECYCLE),
            (
                ADMIN_CLI,
                "fn run_restore_project() {\n    let _ = store.restore_deleted_project(id).await;\n}\n",
            ),
            (
                "crates/foundry-services/src/projects.rs",
                "let o = store.restore_deleted_project(id).await?;\n",
            ),
        ]);
        let violations = check_project_lifecycle_one_seam(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, &format!("{ADMIN_CLI}:2"))
                && violations[0].contains("`restore_deleted_project`"),
            "the CLI goes through services (DDD-10): {violations:?}"
        );
    }

    #[test]
    fn one_seam_a_missing_crates_directory_fails_the_rule() {
        let tree = stage(&[("README.md", "nothing here\n")]);
        assert!(
            !check_project_lifecycle_one_seam(tree.path()).is_empty(),
            "a missing crates directory must FAIL the guard, never pass it vacuously"
        );
    }

    #[test]
    fn one_seam_the_shipped_tree_passes() {
        let root = shipped_root();
        let violations = check_project_lifecycle_one_seam(&root);
        let module = std::fs::read_to_string(root.join(MODULE)).expect("the module exists");
        assert!(
            module.contains("fn rename_project_with_audit("),
            "ANTI-VACUITY: the module is expected to hold the audited rename"
        );
        assert!(violations.is_empty(), "{violations:?}");
    }

    // =======================================================================
    // Phase 5 mutation gaps (mutation-report.md X-1..X-20): the Rust scanner
    // the three rules share, and the write-guard edges the gold tests above
    // leave open. The `*_shipped_tree_passes` tests only show the real tree
    // is not flagged; these pin what the scanner reads and what the rules
    // flag on inputs built to tell a correct reading from a near miss.
    // =======================================================================

    fn literals(source: &str) -> Vec<(usize, String)> {
        rust_string_literals(source)
    }

    fn at(line: usize, text: &str) -> (usize, String) {
        (line, text.to_string())
    }

    /// X-1: a nested, multi-line block comment yields no literal (not even a
    /// quoted word after its inner `*/`), the code after it is still scanned on
    /// the right line, and a division opens no comment.
    #[test]
    fn scanner_a_nested_block_comment_hides_its_text_and_a_division_opens_none() {
        let source = "let a = 1 / 2; let b = \"A\";\n\
                      /* SELECT x\n \
                      /* nested */ FROM \"projects\" */ let c = \"B\";\n\
                      // \"C\"\n\
                      let d = \"D\";\n";
        assert_eq!(literals(source), vec![at(1, "A"), at(3, "B"), at(5, "D")]);
    }

    /// X-2: a source ending in a `//` comment with no trailing newline.
    #[test]
    fn scanner_a_line_comment_at_the_end_of_the_source_ends_the_scan() {
        assert_eq!(
            literals("let q = \"SELECT 1\"; // no newline after this comment"),
            vec![at(1, "SELECT 1")]
        );
    }

    /// X-3: a raw string after a space, after `(`, with a `b` prefix, or at the
    /// very start of the source is one literal with its exact text (a `"` and
    /// a `\` inside included), and its body is blanked in the code.
    #[test]
    fn scanner_raw_strings_are_one_literal_each_with_the_exact_text() {
        let source = r##"let a = r#"SELECT "projects".id FROM t WHERE a ~ '\d'"#;
let b = f(r#"x"y"#);
let c = br#"q"\"#;
"##;
        assert_eq!(
            literals(source),
            vec![
                at(1, r#"SELECT "projects".id FROM t WHERE a ~ '\d'"#),
                at(2, r#"x"y"#),
                at(3, r#"q"\"#),
            ]
        );
        let code = RustSource::scan(source).code;
        assert_eq!(code.len(), source.len(), "one code byte per source char");
        for hidden in ["SELECT", "projects", "x\"y", "q\""] {
            assert!(!code.contains(hidden), "{hidden} is blanked: {code:?}");
        }
        assert!(code.starts_with("let a = r#\""), "{code:?}");
        assert_eq!(literals("r\"SELECT 1\""), vec![at(1, "SELECT 1")]);
    }

    /// X-4: the lines a multi-line raw string spans are counted, so the next
    /// literal is reported on its own line.
    #[test]
    fn scanner_a_literal_after_a_multi_line_raw_string_is_on_its_own_line() {
        assert_eq!(
            literals("let a = r\"one\ntwo\nthree\";\nlet b = \"B\";\n"),
            vec![at(1, "one\ntwo\nthree"), at(4, "B")]
        );
    }

    /// X-5: char literals — a quote, an escaped quote, a `\u{…}` escape
    /// followed by another char literal, and a brace — open no string; a brace
    /// char is blanked in the code and nothing around it is.
    #[test]
    fn scanner_char_literals_open_no_string_and_a_brace_char_is_blanked() {
        let source = r##"let q = '"'; let e = '\"'; let u = ['\u{7b}','"']; let b = '{'; let s = "SELECT 1";"##;
        assert_eq!(literals(source), vec![at(1, "SELECT 1")]);
        assert_eq!(RustSource::scan("f('{')").code, "f(' ')");
    }

    /// X-6: the code is the source with every comment, every literal's
    /// contents and every non-ASCII char blanked, one byte per source char.
    #[test]
    fn scanner_the_code_blanks_comments_literal_contents_and_non_ascii() {
        assert_eq!(
            RustSource::scan("let naïve = \"é\"; // café").code,
            format!("let na ve = \" \";{}", " ".repeat(8))
        );
    }

    /// X-5, X-6 through rule 2: a `'{'` inside a writer and a `}` or `fn`
    /// inside its SQL do not break its span — its guard and its write are both
    /// found, and the next fn is read on its own.
    #[test]
    fn write_guard_a_brace_char_or_brace_in_sql_keeps_the_writers_span() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn opener(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    let brace = '{';
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query("UPDATE lanes SET name = 'fn }' WHERE id = $2").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn unguarded(&self) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM lanes WHERE id = $1").execute(&self.pool).await?;
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "unguarded"),
            "{violations:?}"
        );
    }

    /// X-7: `projects` after `USING` is SQL because the literal is a DELETE
    /// statement, not because it follows FROM or JOIN.
    #[test]
    fn live_reads_a_delete_using_projects_is_flagged() {
        let sql =
            "DELETE FROM issues USING projects p WHERE p.id = issues.project_id AND p.team_id = $1";
        let tree = stage(&[(STORE_LIB, &store_query(sql)), (MODULE, MODULE_BODY)]);
        let violations = check_project_lifecycle_live_reads(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, &format!("{STORE_LIB}:3")),
            "{violations:?}"
        );
    }

    /// X-8: a guarded writer whose write runs on the pool (autocommit) after
    /// the guard is flagged — an inline literal, a writing `const`, a DML split
    /// across `concat!` pieces, and two of each (a toggled flag would clear
    /// itself on the second).
    #[test]
    fn write_guard_a_pooled_write_after_the_guard_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        for (form, writes) in [
            (
                "an inline literal",
                "    sqlx::query(\"UPDATE lanes SET name = $1 WHERE id = $2\").execute(&self.pool).await?;\n",
            ),
            (
                "two inline literals",
                "    sqlx::query(\"UPDATE lanes SET name = $1 WHERE id = $2\").execute(&self.pool).await?;\n    sqlx::query(\"DELETE FROM lanes WHERE id = $1\").execute(&self.pool).await?;\n",
            ),
            (
                "a writing const",
                "    sqlx::query(RENAME_LANE).execute(&self.pool).await?;\n",
            ),
            (
                "two writing consts",
                "    sqlx::query(RENAME_LANE).execute(&self.pool).await?;\n    sqlx::query(DROP_LANE).execute(&self.pool).await?;\n",
            ),
            (
                "a DML split across concat! pieces",
                "    sqlx::query(concat!(\"UPDATE \", \"lanes \", \"SET name = $1\")).execute(&self.pool).await?;\n",
            ),
            (
                "two DMLs split across concat! pieces",
                "    sqlx::query(concat!(\"UPDATE \", \"lanes \", \"SET name = $1\")).execute(&self.pool).await?;\n    sqlx::query(concat!(\"DELETE FROM \", \"lanes WHERE id = $1\")).execute(&self.pool).await?;\n",
            ),
        ] {
            let body = format!(
                "const RENAME_LANE: &str = \"UPDATE lanes SET name = $1 WHERE id = $2\";\nconst DROP_LANE: &str = \"DELETE FROM lanes WHERE id = $1\";\n\npub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {{\n    let mut tx = self.pool.begin().await?;\n    if !lock_live_project(&mut tx, project_id).await? {{\n        return Err(StoreError::NotFound);\n    }}\n{writes}    tx.commit().await?;\n    Ok(())\n}}\n"
            );
            let tree = stage(&[(STORE_LIB, CLEAN_STORE), (MODULE, GUARD_MODULE), (FILE, &body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(names_writer(&violations, FILE, "writer"), "{form}: {violations:?}");
        }
    }

    /// X-9: a writer on the pool that shares its name with a transaction-taking
    /// helper elsewhere (another impl) is not that helper: it holds the guard.
    #[test]
    fn write_guard_a_pool_writer_named_like_a_helper_must_hold_the_guard() {
        const FILE: &str = "crates/foundry-store/src/positions.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"impl Board {
pub async fn renumber_column(&self, project_id: Uuid) -> Result<(), StoreError> {
    sqlx::query("UPDATE issues SET position = position + 1 WHERE project_id = $1").execute(&self.pool).await?;
    Ok(())
}
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "renumber_column"),
            "{violations:?}"
        );
    }

    /// Run rule 2 on `tree` on a worker thread: a scan that loops forever is
    /// a failure, not a hang.
    fn write_guard_within_ten_seconds(tree: &tempfile::TempDir) -> Vec<String> {
        let root = tree.path().to_path_buf();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(check_project_lifecycle_write_guard(&root));
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the write-guard rule finishes within 10 s")
    }

    /// X-10: a guard binding used before its test (a log line) still gates the
    /// write once tested; X-11: a bound guard tested by a plain `if` whose
    /// `else` returns gates a write after the if/else.
    #[test]
    fn write_guard_a_binding_used_before_its_test_or_tested_by_if_else_gates_the_write() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn logged(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    let live = lock_live_project(&mut tx, project_id).await?;
    tracing::debug!(live);
    if !live {
        return Err(StoreError::NotFound);
    }
    sqlx::query("UPDATE lanes SET name = $1 WHERE id = $2").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn branched(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    let live = lock_live_project(&mut tx, project_id).await?;
    if live {
        tracing::debug!("live");
    } else {
        return Err(StoreError::NotFound);
    }
    sqlx::query("DELETE FROM lanes WHERE id = $1").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
"##,
        )]);
        let violations = write_guard_within_ten_seconds(&tree);
        assert!(violations.is_empty(), "{violations:?}");
    }

    /// X-11 (control): the same plain `if` whose `else` does not return gates
    /// nothing after the if/else.
    #[test]
    fn write_guard_a_bound_guard_whose_else_does_not_return_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn branched(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    let live = lock_live_project(&mut tx, project_id).await?;
    if live {
        tracing::debug!("live");
    } else {
        tracing::warn!("gone");
    }
    sqlx::query("DELETE FROM lanes WHERE id = $1").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            names_writer(&violations, FILE, "branched"),
            "{violations:?}"
        );
    }

    /// X-12: a guard nested in another call's arguments is not a binding of the
    /// guard, so testing the outer result gates nothing; X-13: a guard taking a
    /// connection of its own from the pool is not a guard on the writer's
    /// transaction (DELIVER pin 02-02).
    #[test]
    fn write_guard_a_wrapped_guard_or_a_guard_on_a_pooled_transaction_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        for (form, gate) in [
            (
                "a guard wrapped in another call",
                "    let ok = wrap(lock_live_project(&mut tx, project_id).await?);\n    if !ok {\n        return Err(StoreError::NotFound);\n    }\n",
            ),
            (
                "a guard on a transaction begun from the pool",
                "    if !lock_live_project(&mut self.pool.begin().await?, project_id).await? {\n        return Err(StoreError::NotFound);\n    }\n",
            ),
        ] {
            let body = format!(
                "pub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {{\n    let mut tx = self.pool.begin().await?;\n{gate}    sqlx::query(\"UPDATE lanes SET name = $1 WHERE id = $2\").execute(&mut *tx).await?;\n    tx.commit().await?;\n    Ok(())\n}}\n"
            );
            let tree = stage(&[(STORE_LIB, CLEAN_STORE), (MODULE, GUARD_MODULE), (FILE, &body)]);
            let violations = check_project_lifecycle_write_guard(tree.path());
            assert_eq!(violations.len(), 1, "{form}: {violations:?}");
            assert!(names_writer(&violations, FILE, "writer"), "{form}: {violations:?}");
        }
    }

    /// X-14: generic fns are read past their generics: a generic writer must
    /// hold the guard, and a generic helper whose generics hold a `Fn(…)` is
    /// still read by its real parameter list (it takes the transaction, so it
    /// is an exempt helper and its guarded caller passes).
    #[test]
    fn write_guard_generic_fns_are_read_by_their_real_parameters() {
        const FILE: &str = "crates/foundry-store/src/positions.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn write<'a, T: Into<Uuid>>(&self, id: T) -> Result<(), StoreError> {
    sqlx::query("UPDATE lanes SET name = $1 WHERE id = $2").execute(&self.pool).await?;
    Ok(())
}

async fn bump_positions<F: FnOnce(Uuid)>(tx: &mut Transaction<'_, Postgres>, project_id: Uuid, then: F) -> Result<(), StoreError> {
    sqlx::query("UPDATE issues SET position = position + 1 WHERE project_id = $1").execute(&mut **tx).await?;
    then(project_id);
    Ok(())
}

pub async fn bump(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    bump_positions(&mut tx, project_id, |_| {}).await?;
    tx.commit().await?;
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(names_writer(&violations, FILE, "write"), "{violations:?}");
    }

    /// X-15: the violation names the writer's own 1-based line.
    #[test]
    fn write_guard_the_violation_names_the_writers_line() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            "// Lanes.\n\npub async fn writer(&self) {\n    sqlx::query(\"DELETE FROM lanes WHERE id = $1\").execute(&self.pool).await;\n}\n",
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains(&format!("{FILE}:3 fn `writer`")),
            "{violations:?}"
        );
    }

    /// X-16: a write in a fn nested near the end of a long guarded fn belongs
    /// to the nested fn, which holds no guard.
    #[test]
    fn write_guard_a_write_in_a_nested_fn_belongs_to_the_nested_fn() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let body = format!(
            "pub async fn outer(&self, project_id: Uuid) -> Result<(), StoreError> {{\n    let mut tx = self.pool.begin().await?;\n    if !lock_live_project(&mut tx, project_id).await? {{\n        return Err(StoreError::NotFound);\n    }}\n{}    async fn nested(pool: &PgPool) {{\n        sqlx::query(\"DELETE FROM lanes WHERE id = $1\").execute(pool).await;\n    }}\n    tx.commit().await?;\n    Ok(())\n}}\n",
            "    let _step = 1;\n".repeat(60)
        );
        let tree = stage(&[
            (STORE_LIB, CLEAN_STORE),
            (MODULE, GUARD_MODULE),
            (FILE, &body),
        ]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(names_writer(&violations, FILE, "nested"), "{violations:?}");
    }

    /// X-17: issue create's attempt stays exempt when a non-SQL literal (a
    /// constraint name) stands before its counter `UPDATE live_projects`.
    #[test]
    fn write_guard_insert_issue_attempt_with_a_constraint_name_before_its_counter_passes() {
        let store = CLEAN_STORE.replace(
            "    let row = sqlx::query(\"UPDATE live_projects",
            "    let _retry_on = \"issues_project_id_number_key\";\n    let row = sqlx::query(\"UPDATE live_projects",
        );
        assert_ne!(store, CLEAN_STORE, "the fixture edit applied");
        let tree = stage(&[
            (STORE_LIB, store.as_str()),
            (MODULE, GUARD_MODULE),
            (LEGACY_SEAM, LEGACY_SEAM_DDL),
        ]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }

    /// X-18: an inline-capture `format!` (no arguments) with the table
    /// interpolated is flagged; X-19: a `concat!` of literals only names its
    /// table and is not; X-20: DDL with `ON DELETE CASCADE, {…}` and an
    /// `INSERT` not followed by `INTO` are not DML targets.
    #[test]
    fn write_guard_only_an_interpolated_dml_table_is_a_template_violation() {
        const FILE: &str = "crates/foundry-store/src/generic.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn templates(&self, project_id: Uuid, table: &str, rest: &str) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    let named = concat!("UPDATE ", "lanes ", "SET name = $1 WHERE id = $2");
    let fk = format!("ALTER TABLE lanes ADD CONSTRAINT lanes_project_fk FOREIGN KEY (project_id) REFERENCES live_projects ON DELETE CASCADE, {rest}");
    let rule = format!("CREATE RULE keep AS ON INSERT TO {table} DO INSTEAD NOTHING");
    let interpolated = format!("UPDATE {table} SET name = $1");
    Ok(())
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            flagged_at(&violations, &format!("{FILE}:9")),
            "{violations:?}"
        );
    }

    /// A `concat!` that splits its DML across pieces writes the table its
    /// joined pieces name: an unguarded one is flagged, whatever the split.
    #[test]
    fn write_guard_an_unguarded_dml_split_across_concat_pieces_is_flagged() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn writer(&self) {
    sqlx::query(concat!("UPDATE ", "lanes ", "SET name = $1")).execute(&self.pool).await;
}
pub async fn writer2(&self) {
    sqlx::query(concat!("DELETE FROM ", "lanes WHERE id = $1")).execute(&self.pool).await;
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(names_writer(&violations, FILE, "writer"), "{violations:?}");
        assert!(names_writer(&violations, FILE, "writer2"), "{violations:?}");
    }

    /// The same split DML behind the guard on its transaction passes, and a
    /// `concat!` whose joined pieces are not DML is no write.
    #[test]
    fn write_guard_a_guarded_or_non_dml_concat_passes() {
        const FILE: &str = "crates/foundry-store/src/lanes.rs";
        let tree = guard_tree(&[(
            FILE,
            r##"pub async fn writer(&self, project_id: Uuid) -> Result<(), StoreError> {
    let mut tx = self.pool.begin().await?;
    if !lock_live_project(&mut tx, project_id).await? {
        return Err(StoreError::NotFound);
    }
    sqlx::query(concat!("UPDATE ", "lanes ", "SET name = $1")).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn reader(&self) {
    sqlx::query(concat!("SELECT name ", "FROM ", "lanes WHERE id = $1")).fetch_one(&self.pool).await;
}
"##,
        )]);
        let violations = check_project_lifecycle_write_guard(tree.path());
        assert!(violations.is_empty(), "{violations:?}");
    }
}

// ---------------------------------------------------------------------------
// api-openapi-operations (api-openapi-document slice 01, ADR-API-OPENAPI-001)
// ---------------------------------------------------------------------------

/// The uniform-404 catch-all: not an operation (D4), so it is never compared.
const API_CATCH_ALL: &str = "/api/v1/{*rest}";
/// The description's own route, the one route allowed to chain its `get` on an
/// `any(api_not_found_handler)` (DDD-4, DELIVER UI-1).
const API_DESCRIPTION_ROUTE: &str = "/api/v1/openapi.json";
const API_DESCRIPTION_FILE: &str = "crates/foundry-api/openapi.json";
const API_SRC: &str = "crates/foundry-api/src";
const API_METHODS: [&str; 5] = ["get", "post", "patch", "put", "delete"];
const OPENAPI_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Rule `api-openapi-operations` (DESIGN [REF] check-arch Rules, rule 1).
/// Every operation is described (D13): the shipped rule tolerates none pending.
fn check_api_openapi_operations(root: &Path) -> Vec<String> {
    api_openapi_operations(root, &[])
}

/// The rule with a pending list as an INPUT, so a gold test can prove that
/// only a listed operation is tolerated. The shipped rule passes an empty list:
/// slice 02 described every operation and deleted the list (D13, release gate R4).
fn api_openapi_operations(root: &Path, pending: &[(&str, &str)]) -> Vec<String> {
    let mut violations = api_routes_outside_foundry_api(root);
    let code = ApiCode::read(root);
    violations.extend(code.violations.iter().cloned());
    match ApiDescription::read(root) {
        Ok(description) => violations.extend(code.compare(&description, pending)),
        Err(unreadable) => violations.push(operations_violation(&unreadable)),
    }
    violations
}

fn operations_violation(text: &str) -> String {
    format!("api-openapi-operations: {text}")
}

/// One `.route(..)` call: the line it opens on, its path when that is a string
/// literal, and the code of its method router.
struct RouteCall {
    line: usize,
    path: Option<String>,
    methods: String,
}

/// Every `.route(..)` call in a Rust source. The blocks come from
/// [`route_blocks`], the parser `api≠mint` uses; each one is then read from
/// [`RustSource`], so a `.route(` inside a string literal is not a call.
fn route_calls(contents: &str) -> Vec<RouteCall> {
    let source = RustSource::scan(contents);
    let code = source.code.as_bytes();
    let stripped: Vec<String> = contents.lines().map(strip_comment).collect();
    let starts = line_starts(&source.code);
    let mut calls = Vec::new();
    for (first, _) in route_blocks(&stripped) {
        let line = &stripped[first];
        let column = line[..line.find(".route(").unwrap_or(0)].chars().count();
        let at = starts[first] + column;
        if !source.code[at..].starts_with(".route(") {
            continue;
        }
        let open = at + ".route".len();
        let close = matching_brace(code, open).unwrap_or(code.len());
        let first_arg = skip_whitespace(code, open + 1);
        let literal = source.literals.iter().find(|l| l.start == first_arg);
        let (path, methods) = match literal {
            Some(literal) => {
                let comma = skip_whitespace(code, literal.end);
                let methods = match code.get(comma) {
                    Some(b',') => source.code[comma + 1..close]
                        .trim()
                        .trim_end_matches(',')
                        .trim_end()
                        .to_string(),
                    _ => String::new(),
                };
                (Some(literal.text.clone()), methods)
            }
            None => (None, String::new()),
        };
        calls.push(RouteCall {
            line: first + 1,
            path,
            methods,
        });
    }
    calls
}

fn skip_whitespace(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

/// One `name(handler)` link of a method-router chain.
struct MethodLink {
    name: String,
    handler: String,
}

/// `get(a).post(b)` as links, or `None` when the expression is anything but a
/// chain of single-handler calls (a variable, a closure, a builder) — which
/// the rule cannot read, so it fails closed.
fn method_chain(expression: &str) -> Option<Vec<MethodLink>> {
    let bytes = expression.as_bytes();
    let is_path_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b':';
    let last_segment = |path: &str| path.rsplit("::").next().unwrap_or(path).to_string();
    let mut links = Vec::new();
    let mut at = 0;
    loop {
        let start = at;
        while bytes.get(at).is_some_and(|&b| is_path_byte(b)) {
            at += 1;
        }
        let name = last_segment(&expression[start..at]);
        if bytes.get(at) != Some(&b'(') {
            return None;
        }
        let close = matching_brace(bytes, at)?;
        let handler = expression[at + 1..close].trim();
        if handler.is_empty() || !handler.bytes().all(is_path_byte) {
            return None;
        }
        links.push(MethodLink {
            name,
            handler: last_segment(handler),
        });
        at = skip_whitespace(bytes, close + 1);
        if at == bytes.len() {
            return Some(links);
        }
        if bytes[at] != b'.' {
            return None;
        }
        at = skip_whitespace(bytes, at + 1);
    }
}

/// Clause (vi): no route literal outside `crates/foundry-api/src` starts with
/// `/api`, so an `/api/v1` route cannot be mounted where the rule does not look.
fn api_routes_outside_foundry_api(root: &Path) -> Vec<String> {
    let crates = root.join("crates");
    let api_src = root.join(API_SRC);
    let mut violations = Vec::new();
    for file in rust_sources(&crates) {
        let in_a_crate_src = file.strip_prefix(&crates).is_ok_and(|inside| {
            inside
                .components()
                .nth(1)
                .is_some_and(|c| c.as_os_str() == "src")
        });
        if file.starts_with(&api_src) || !in_a_crate_src {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        for call in route_calls(&contents) {
            if let Some(path) = call.path.filter(|path| path.starts_with("/api")) {
                violations.push(operations_violation(&format!(
                    "{}:{} mounts the route `{path}` outside crates/foundry-api/src; every /api route belongs in foundry_api::routes(), where it is held to the description",
                    rel(root, &file),
                    call.line,
                )));
            }
        }
    }
    violations
}

/// One `/api/v1` operation the code offers.
struct CodeOperation {
    method: String,
    template: String,
    handler: String,
    location: String,
    /// Chained on the reviewed `any(api_not_found_handler)` (the description's
    /// own route): every other method answers the catch-all 404, never a 405.
    beside_any: bool,
}

impl CodeOperation {
    fn label(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.template)
    }

    /// DDD-14: on the pending list, so neither description rule compares it.
    fn is_pending(&self, pending: &[(&str, &str)]) -> bool {
        pending.iter().any(|(method, path)| {
            method.eq_ignore_ascii_case(&self.method) && *path == self.template
        })
    }
}

/// A handler `fn` of foundry-api: its parameter list, return type and body,
/// as comment- and literal-blanked code.
struct Handler {
    params: String,
    returns: String,
    body: String,
}

/// What the code says about foundry-api's operations.
struct ApiCode {
    operations: Vec<CodeOperation>,
    handlers: Vec<(String, Handler)>,
    violations: Vec<String>,
}

impl ApiCode {
    fn read(root: &Path) -> Self {
        let api_src = root.join(API_SRC);
        let mut code = ApiCode {
            operations: Vec::new(),
            handlers: Vec::new(),
            violations: Vec::new(),
        };
        for file in rust_sources(&api_src) {
            let Ok(contents) = std::fs::read_to_string(&file) else {
                continue;
            };
            let source = RustSource::scan(&contents);
            for span in fn_spans(&source.code) {
                let signature = &source.code[span.params.0..span.body.0];
                let (params, returns) = signature.split_once("->").unwrap_or((signature, ""));
                code.handlers.push((
                    span.name.clone(),
                    Handler {
                        params: params.to_string(),
                        returns: returns.trim().to_string(),
                        body: source.code[span.body.0..span.body.1].to_string(),
                    },
                ));
            }
            let file_rel = rel(root, &file);
            for call in route_calls(&contents) {
                code.add_route(&format!("{file_rel}:{}", call.line), call);
            }
        }
        code
    }

    /// Clauses (v) and (vii): a readable `/api/v1/` literal path and a chain of
    /// `get`/`post`/`patch`/`put`/`delete`, with `any(` only on the catch-all
    /// and under the description's own `get`, and no `.fallback(` anywhere.
    fn add_route(&mut self, location: &str, call: RouteCall) {
        let Some(path) = call.path else {
            self.violations.push(operations_violation(&format!(
                "the `.route(` at {location} has a path that is not a string literal, so it cannot be compared with the description"
            )));
            return;
        };
        if !path.starts_with("/api/v1/") {
            self.violations.push(operations_violation(&format!(
                "the route `{path}` at {location} does not start with /api/v1/"
            )));
        }
        let Some(chain) = method_chain(&call.methods) else {
            self.violations.push(operations_violation(&format!(
                "the route `{path}` at {location} has a method router that is not a chain of get/post/patch/put/delete calls, so it cannot be compared with the description"
            )));
            return;
        };
        let names: Vec<&str> = chain.iter().map(|link| link.name.as_str()).collect();
        let reviewed_any = match (path.as_str(), names.as_slice()) {
            (API_CATCH_ALL, ["any"]) | (API_DESCRIPTION_ROUTE, ["any", "get"]) => {
                chain[0].handler == "api_not_found_handler"
            }
            _ => false,
        };
        if reviewed_any {
            for link in &chain[1..] {
                self.add_operation(location, &path, link, true);
            }
            return;
        }
        for link in &chain {
            if API_METHODS.contains(&link.name.as_str()) {
                self.add_operation(location, &path, link, false);
            } else {
                self.violations.push(operations_violation(&format!(
                    "the route `{path}` at {location} chains `{}({})`; only get/post/patch/put/delete are compared with the description (an `any(` belongs only to {API_CATCH_ALL} and, under its `get`, to {API_DESCRIPTION_ROUTE}; a `.fallback(` to none)",
                    link.name, link.handler
                )));
            }
        }
    }

    fn add_operation(&mut self, location: &str, path: &str, link: &MethodLink, beside_any: bool) {
        self.operations.push(CodeOperation {
            method: link.name.clone(),
            template: path.to_string(),
            handler: link.handler.clone(),
            location: location.to_string(),
            beside_any,
        });
    }

    fn handler(&self, name: &str) -> Result<&Handler, String> {
        let mut found = self.handlers.iter().filter(|(n, _)| n == name);
        match (found.next(), found.next()) {
            (Some((_, handler)), None) => Ok(handler),
            (None, _) => Err(format!(
                "its handler `{name}` is not found in crates/foundry-api/src"
            )),
            (Some(_), Some(_)) => Err(format!(
                "its handler `{name}` is defined more than once in crates/foundry-api/src"
            )),
        }
    }

    /// Clause (i), then (ii)-(iv) for each operation both sides have.
    fn compare(&self, description: &ApiDescription, pending: &[(&str, &str)]) -> Vec<String> {
        let mut violations = Vec::new();
        for op in &self.operations {
            let described = description.operation(&op.method, &op.template);
            let difference = match (op.is_pending(pending), described) {
                (true, None) => continue,
                (true, Some(_)) => vec![
                    "is described but still on the pending list; take it off the list".to_string(),
                ],
                (false, None) => vec![format!(
                    "is offered by the code but undocumented in {API_DESCRIPTION_FILE}"
                )],
                (false, Some(described)) => self.differences(op, described),
            };
            violations.extend(difference.into_iter().map(|difference| {
                operations_violation(&format!("{} ({}) {difference}", op.label(), op.location))
            }));
        }
        for (method, template) in description.operations() {
            if !self.operations.iter().any(|op| {
                op.method == method && template_shape(&op.template) == template_shape(template)
            }) {
                violations.push(operations_violation(&format!(
                    "{} {template} is described in {API_DESCRIPTION_FILE} but not offered by the code",
                    method.to_ascii_uppercase()
                )));
            }
        }
        violations
    }

    fn differences(&self, op: &CodeOperation, described: Described<'_>) -> Vec<String> {
        let handler = match self.handler(&op.handler) {
            Ok(handler) => handler,
            Err(missing) => return vec![missing],
        };
        let mut differences = Vec::new();
        if op.template != described.template {
            differences.push(format!(
                "has the path parameters {:?} in the code but {:?} in the description ({})",
                template_parameters(&op.template),
                template_parameters(described.template),
                described.template
            ));
        }
        match handler.path_parameters(&op.template) {
            Ok(parameters) => differences.extend(described.path_parameter_differences(&parameters)),
            Err(unreadable) => differences.push(unreadable),
        }
        differences.extend(described.request_body_difference(handler.request_body()));
        let status = handler.success_status();
        let documented = described.success_statuses();
        if documented != [status.to_string()] {
            differences.push(format!(
                "answers {status} on success in the code but {documented:?} in the description"
            ));
        }
        let body = if op.template == API_DESCRIPTION_ROUTE && op.method == "get" {
            Ok(SuccessBody::Description)
        } else {
            handler.success_body(status)
        };
        match body {
            Ok(body) => differences.extend(described.success_body_difference(status, &body)),
            Err(unreadable) => differences.push(unreadable),
        }
        differences
    }
}

/// The schema type a Rust path-parameter type is documented with (clause ii).
fn path_parameter_schema(rust: &str) -> Option<(&'static str, Option<&'static str>)> {
    match rust {
        "String" => Some(("string", None)),
        "i32" => Some(("integer", Some("int32"))),
        "Uuid" | "uuid::Uuid" => Some(("string", Some("uuid"))),
        _ => None,
    }
}

/// A path parameter as the code types it: its template name and Rust type.
struct PathParameter {
    name: String,
    rust: String,
}

/// The success answer's body as the code types it (clause iv).
enum SuccessBody {
    None,
    One(String),
    ArrayOf(String),
    /// The description's own route answers the description itself.
    Description,
}

impl Handler {
    fn path_parameters(&self, template: &str) -> Result<Vec<PathParameter>, String> {
        let names = template_parameters(template);
        let types: Vec<String> = match generic_argument(&self.params, "Path") {
            None => Vec::new(),
            Some(inner) if inner.starts_with('(') => split_top_level(&inner[1..inner.len() - 1]),
            Some(inner) => vec![inner],
        };
        if types.len() != names.len() {
            return Err(format!(
                "has {} path parameter(s) in its template but the handler's Path<…> extracts {}",
                names.len(),
                types.len()
            ));
        }
        Ok(names
            .into_iter()
            .zip(types)
            .map(|(name, rust)| PathParameter { name, rust })
            .collect())
    }

    fn request_body(&self) -> Option<String> {
        generic_argument(&self.params, "Json")
    }

    fn success_status(&self) -> u16 {
        if self.body.contains("StatusCode::CREATED") {
            201
        } else if self.body.contains("StatusCode::NO_CONTENT") {
            204
        } else {
            200
        }
    }

    fn success_body(&self, status: u16) -> Result<SuccessBody, String> {
        if status == 204 {
            return Ok(SuccessBody::None);
        }
        let returns = match generic_argument(&self.returns, "Result") {
            Some(ok_and_err) => split_top_level(&ok_and_err).remove(0),
            None => self.returns.split_whitespace().collect(),
        };
        if let Some(json) = generic_argument(&returns, "Json") {
            return Ok(match generic_argument(&json, "Vec") {
                Some(item) => SuccessBody::ArrayOf(item),
                None => SuccessBody::One(json),
            });
        }
        match self.json_wrapped_type() {
            Some(single) if returns == "Response" => Ok(SuccessBody::One(single)),
            _ => Err(format!(
                "returns `{returns}` and no single success body type can be read from it (clause iv: name the type, or wrap one struct literal in Json(…))"
            )),
        }
    }

    /// The one struct type the body's `Json(…)` calls wrap: a literal
    /// `Json(T { … })`, or a `Json(x)` whose `let x = T {` is in the body.
    /// `None` when a call wraps anything else, or the calls disagree.
    fn json_wrapped_type(&self) -> Option<String> {
        let bytes = self.body.as_bytes();
        let mut types = BTreeSet::new();
        let mut from = 0;
        while let Some(at) = find_sql_word(&self.body, "Json", from) {
            from = at + "Json".len();
            if bytes.get(from) != Some(&b'(') {
                continue;
            }
            let close = matching_brace(bytes, from).unwrap_or(bytes.len());
            let argument = self.body[from + 1..close].trim();
            let binding = format!("let {argument} = ");
            let literal = match self.body.find(&binding) {
                Some(at) => self.body[at + binding.len()..].trim_start(),
                None => argument,
            };
            let name: String = literal
                .chars()
                .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
                .collect();
            if name.is_empty() || !literal[name.len()..].trim_start().starts_with('{') {
                return None;
            }
            types.insert(name);
        }
        let mut types = types.into_iter();
        match (types.next(), types.next()) {
            (Some(single), None) => Some(single),
            _ => None,
        }
    }
}

/// The whitespace-free generic argument of the first `name<…>` in `text`.
fn generic_argument(text: &str, name: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(at) = find_sql_word(text, name, from) {
        from = at + name.len();
        if bytes.get(from) != Some(&b'<') {
            continue;
        }
        let mut depth = 0usize;
        for (offset, &b) in bytes[from..].iter().enumerate() {
            match b {
                b'<' => depth += 1,
                b'>' if depth == 1 => {
                    return Some(text[from + 1..from + offset].split_whitespace().collect());
                }
                b'>' => depth -= 1,
                _ => {}
            }
        }
    }
    None
}

/// Split on the commas outside any `<…>` or `(…)`, dropping a trailing empty.
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(part) = parts.last_mut() {
            part.push(c);
        }
    }
    parts.retain(|part| !part.trim().is_empty());
    parts.iter().map(|part| part.trim().to_string()).collect()
}

/// The `{names}` of a path template, in order.
fn template_parameters(template: &str) -> Vec<String> {
    template
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}'))
        .map(|(name, _)| name.to_string())
        .collect()
}

/// A template with every parameter name dropped: `/a/{x}` → `/a/{}`, so a
/// renamed parameter is a parameter difference, not a missing operation.
fn template_shape(template: &str) -> String {
    let mut shape = String::new();
    let mut inside = false;
    for c in template.chars() {
        match c {
            '{' => inside = true,
            '}' => inside = false,
            _ if inside => continue,
            _ => {}
        }
        shape.push(c);
    }
    shape
}

/// DDD-6: the component a Rust wire type is described as.
fn component_ref(rust: &str) -> String {
    let name = rust.rsplit("::").next().unwrap_or(rust);
    let component = match name {
        "ErrorBody" => "Error",
        "IssueHistoryJson" => "IssueHistoryEntry",
        other => other.strip_suffix("Json").unwrap_or(other),
    };
    format!("#/components/schemas/{component}")
}

/// The committed description, parsed. Fails closed when it cannot be read.
struct ApiDescription {
    document: serde_json::Value,
}

/// One described operation: the template it sits under and its object.
#[derive(Clone, Copy)]
struct Described<'a> {
    template: &'a str,
    operation: &'a serde_json::Value,
}

impl ApiDescription {
    fn read(root: &Path) -> Result<Self, String> {
        let path = root.join(API_DESCRIPTION_FILE);
        let bytes = std::fs::read(&path).map_err(|e| {
            format!("{API_DESCRIPTION_FILE} cannot be read ({e}); the rule fails closed")
        })?;
        let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            format!("{API_DESCRIPTION_FILE} is not JSON ({e}); the rule fails closed")
        })?;
        if !document["paths"].is_object() {
            return Err(format!(
                "{API_DESCRIPTION_FILE} has no `paths` object; the rule fails closed"
            ));
        }
        Ok(ApiDescription { document })
    }

    fn operations(&self) -> Vec<(&str, &str)> {
        let mut operations = Vec::new();
        for (template, item) in self.document["paths"].as_object().into_iter().flatten() {
            for method in item.as_object().into_iter().flatten().map(|(k, _)| k) {
                if OPENAPI_METHODS.contains(&method.as_str()) {
                    operations.push((method.as_str(), template.as_str()));
                }
            }
        }
        operations
    }

    fn operation(&self, method: &str, template: &str) -> Option<Described<'_>> {
        let shape = template_shape(template);
        self.document["paths"]
            .as_object()
            .into_iter()
            .flatten()
            .find(|(described, _)| template_shape(described) == shape)
            .and_then(|(described, item)| {
                item.get(method).map(|operation| Described {
                    template: described.as_str(),
                    operation,
                })
            })
    }
}

impl Described<'_> {
    /// Clause (ii): every template parameter is a required `in: path`
    /// parameter typed from the handler, and no other `in: path` is described.
    fn path_parameter_differences(&self, code: &[PathParameter]) -> Vec<String> {
        let described: Vec<&serde_json::Value> = self.operation["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["in"] == "path")
            .collect();
        let mut differences = Vec::new();
        for parameter in code {
            let name = &parameter.name;
            let Some(schema) = path_parameter_schema(&parameter.rust) else {
                differences.push(format!(
                    "types the path parameter `{name}` as `{}`, which has no documented schema (String, i32 or uuid::Uuid)",
                    parameter.rust
                ));
                continue;
            };
            let Some(doc) = described.iter().find(|p| p["name"] == name.as_str()) else {
                differences.push(format!(
                    "has the path parameter `{name}`, which the description does not describe"
                ));
                continue;
            };
            if doc["required"] != true {
                differences.push(format!(
                    "describes the path parameter `{name}` without `required: true`"
                ));
            }
            let found = (
                doc["schema"]["type"].as_str(),
                doc["schema"]["format"].as_str(),
            );
            if found != (Some(schema.0), schema.1) {
                differences.push(format!(
                    "types the path parameter `{name}` as `{}` (type {}, format {}) in the code but as type {}, format {} in the description",
                    parameter.rust,
                    schema.0,
                    schema.1.unwrap_or("none"),
                    found.0.unwrap_or("none"),
                    found.1.unwrap_or("none"),
                ));
            }
        }
        for doc in described {
            let name = doc["name"].as_str().unwrap_or_default();
            if !code.iter().any(|p| p.name == name) {
                differences.push(format!(
                    "describes the path parameter `{name}`, which the path does not have"
                ));
            }
        }
        differences
    }

    /// Clause (iii): a `Json<T>` argument is a required application/json body
    /// referring to T's component; no `Json<…>` argument, no request body.
    fn request_body_difference(&self, code: Option<String>) -> Option<String> {
        let body = &self.operation["requestBody"];
        match code {
            None if body.is_null() => None,
            None => Some(
                "has a request body in the description but the handler takes no Json<…>"
                    .to_string(),
            ),
            Some(rust) => {
                let expected = component_ref(&rust);
                let schema = &body["content"]["application/json"]["schema"];
                (body["required"] != true || schema["$ref"] != expected.as_str()).then(|| {
                    format!(
                        "takes a Json<{rust}> request body in the code, so the description needs a required application/json request body referring to {expected}"
                    )
                })
            }
        }
    }

    fn success_statuses(&self) -> Vec<String> {
        self.operation["responses"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(status, _)| status.clone())
            .filter(|status| status.starts_with('2'))
            .collect()
    }

    /// Clause (iv): the success answer's body.
    fn success_body_difference(&self, status: u16, code: &SuccessBody) -> Option<String> {
        let answer = &self.operation["responses"][status.to_string()];
        let schema = &answer["content"]["application/json"]["schema"];
        let exact = |expected: serde_json::Value| (*schema == expected, expected.to_string());
        let (matches, expected) = match code {
            SuccessBody::None => (answer["content"].is_null(), "no body".to_string()),
            SuccessBody::One(rust) => exact(serde_json::json!({"$ref": component_ref(rust)})),
            SuccessBody::ArrayOf(rust) => exact(serde_json::json!({
                "type": "array",
                "items": {"$ref": component_ref(rust)},
            })),
            SuccessBody::Description => (
                schema["type"] == "object",
                "an object schema (the description itself)".to_string(),
            ),
        };
        (!matches).then(|| {
            format!(
                "answers {status} with the body {expected} in the code, but the description's {status} answer has the content {}",
                answer["content"]
            )
        })
    }
}

// ---------------------------------------------------------------------------
// api-openapi-statuses (api-openapi-document slice 01, ADR-API-OPENAPI-002)
// ---------------------------------------------------------------------------
//
// For each described operation, the answers the code can give (status, body
// kind, error codes) equal the ones the description lists. The transport set
// is read from the handler's signature and route; the domain set by lexical,
// conservative reachability: from the handler body and, for a bearer
// operation, the `MachinePrincipal` extractor, every call to a crate `fn` of
// foundry-api or foundry-services is followed (same-named definitions
// unioned), every `ServiceError::<Variant>` token and literal Validation
// `code` collected, and each variant mapped through `status_for`'s parsed
// arms. The walk only grows the set. Its limit: an error produced with no
// lexical trace (inside a macro expansion, through a trait object) is not
// seen; every form it can recognise but not read fails closed instead
// (the soundness clause), and the acceptance requests are the backstop.

/// Reviewed exemptions, `(operation, status, reason)`, for a status the walk
/// over-approximates. Empty at design time; adding a row is a reviewed change.
const STATUSES_EXEMPTIONS: &[(&str, &str, &str)] = &[];

const SERVICES_SRC: &str = "crates/foundry-services/src";
/// Bodies the walk never enters: `status_for` is the map variants go through,
/// not a place they arise, and the catch-all is not an operation (D15).
const UNWALKED_FNS: [&str; 2] = ["status_for", "api_not_found_handler"];
const BEARER_EXTRACTOR: &str = "MachinePrincipal";
const EXTRACTOR_START: &str = "from_request_parts";
const RATE_GUARD: &str = "RevokeRateGuard";
const RATE_LIMITED_FN: &str = "rate_limited_response";
const ERROR_SCHEMA: &str = "#/components/schemas/Error";
const SHARED_RESPONSES: &str = "#/components/responses/";

/// Rule `api-openapi-statuses` (DESIGN [REF] check-arch Rules, rule 2).
fn check_api_openapi_statuses(root: &Path) -> Vec<String> {
    api_openapi_statuses(root, &[])
}

/// The rule with its pending list as an input (DDD-14), as rule 1 takes it.
fn api_openapi_statuses(root: &Path, pending: &[(&str, &str)]) -> Vec<String> {
    let code = ApiCode::read(root);
    let flow = ErrorFlow::read(root);
    let map = StatusMap::read(&flow);
    let mut violations: BTreeSet<String> = flow.soundness_violations().into_iter().collect();
    violations.extend(map.violations.iter().cloned());
    let description = match ApiDescription::read(root) {
        Ok(description) => description,
        Err(unreadable) => {
            violations.insert(statuses_violation(&unreadable));
            return violations.into_iter().collect();
        }
    };
    let describes_any = !description.operations().is_empty();
    if describes_any && !description.document["components"]["responses"]["NotFound"].is_object() {
        violations.insert(statuses_violation(&format!(
            "{API_DESCRIPTION_FILE} has no shared components.responses.NotFound, the answer of the {API_CATCH_ALL} catch-all"
        )));
    }
    for op in code.operations.iter().filter(|op| !op.is_pending(pending)) {
        let (Some(described), Ok(handler)) = (
            description.operation(&op.method, &op.template),
            code.handler(&op.handler),
        ) else {
            continue; // undescribed or unresolvable: rule 1 names it
        };
        violations.extend(map.missing.iter().cloned());
        match flow.answers(&code, op, handler, &map) {
            Ok(answers) => violations.extend(answers.compare(op, &description, described)),
            Err(unreadable) => violations.extend(unreadable),
        }
    }
    violations.into_iter().collect()
}

fn statuses_violation(text: &str) -> String {
    format!("api-openapi-statuses: {text}")
}

fn soundness_violation(location: &str, text: &str) -> String {
    statuses_violation(&format!(
        "{location} {text}; the rule cannot see what it produces, so it fails closed (soundness clause, ADR-API-OPENAPI-002)"
    ))
}

/// How an answer's body is described (ADR-API-OPENAPI-002 "equals").
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BodyKind {
    Envelope,
    PlainText,
    Empty,
}

impl BodyKind {
    fn label(self) -> &'static str {
        match self {
            BodyKind::Envelope => "the application/json Error envelope",
            BodyKind::PlainText => "text/plain",
            BodyKind::Empty => "no body",
        }
    }

    fn labels(kinds: &BTreeSet<BodyKind>) -> String {
        let labels: Vec<&str> = kinds.iter().map(|kind| kind.label()).collect();
        labels.join(" and ")
    }
}

/// One status of one operation: its body kinds, its envelope codes, and (on
/// the code side) why the code can give it.
#[derive(Default)]
struct Answer {
    kinds: BTreeSet<BodyKind>,
    codes: BTreeSet<String>,
    because: BTreeSet<String>,
}

/// An operation's non-success answers by status.
#[derive(Default)]
struct Answers(BTreeMap<u16, Answer>);

impl Answers {
    fn add(&mut self, status: u16, kind: BodyKind, codes: &BTreeSet<String>, because: String) {
        let answer = self.0.entry(status).or_default();
        answer.kinds.insert(kind);
        answer.codes.extend(codes.iter().cloned());
        answer.because.insert(because);
    }

    /// Both directions: status keys, then each shared status's body kinds and
    /// envelope codes. 2xx answers are rule 1's.
    fn compare(
        &self,
        op: &CodeOperation,
        description: &ApiDescription,
        described: Described<'_>,
    ) -> Vec<String> {
        let label = format!("{} ({})", op.label(), op.location);
        let documented = match description.answers(described) {
            Ok(documented) => documented,
            Err(unreadable) => return vec![statuses_violation(&format!("{label} {unreadable}"))],
        };
        let exempt = |status: &u16| {
            STATUSES_EXEMPTIONS
                .iter()
                .any(|(operation, code, _)| *operation == op.label() && *code == status.to_string())
        };
        let mut differences = Vec::new();
        for (status, answer) in self.0.iter().filter(|(status, _)| !exempt(status)) {
            let kinds = BodyKind::labels(&answer.kinds);
            let because: Vec<&str> = answer.because.iter().map(String::as_str).collect();
            let because = because.join("; ");
            let Some(listed) = documented.0.get(status) else {
                differences.push(format!(
                    "can answer {status} with {kinds} in the code ({because}), but the description does not list {status}"
                ));
                continue;
            };
            if listed.kinds != answer.kinds {
                differences.push(format!(
                    "answers {status} with {kinds} in the code ({because}) but with {} in the description",
                    BodyKind::labels(&listed.kinds)
                ));
            } else if listed.codes != answer.codes {
                differences.push(format!(
                    "answers {status} with the error code(s) {:?} in the code ({because}) but the description's examples list {:?}",
                    answer.codes, listed.codes
                ));
            }
        }
        for status in documented
            .0
            .keys()
            .filter(|s| !self.0.contains_key(s) && !exempt(s))
        {
            differences.push(format!(
                "lists {status} in the description, but the code cannot answer it"
            ));
        }
        differences
            .into_iter()
            .map(|difference| statuses_violation(&format!("{label} {difference}")))
            .collect()
    }
}

impl ApiDescription {
    /// The described non-success answers, `$ref`s resolved.
    fn answers(&self, described: Described<'_>) -> Result<Answers, String> {
        let mut answers = Answers::default();
        for (key, response) in described.operation["responses"]
            .as_object()
            .into_iter()
            .flatten()
        {
            if key.starts_with('2') {
                continue;
            }
            let status: u16 = key.parse().map_err(|_| {
                format!(
                    "lists the response key `{key}`, which is not a status the rule can compare"
                )
            })?;
            let response = self.resolve_response(response)?;
            let answer = answers.0.entry(status).or_default();
            match response["content"].as_object().filter(|c| !c.is_empty()) {
                None => {
                    answer.kinds.insert(BodyKind::Empty);
                }
                Some(content) => {
                    for (media, entry) in content {
                        match media.as_str() {
                            "text/plain" => answer.kinds.insert(BodyKind::PlainText),
                            "application/json" if entry["schema"]["$ref"] == ERROR_SCHEMA => {
                                let codes = entry["examples"].as_object().into_iter().flatten();
                                answer.codes.extend(codes.map(|(code, _)| code.clone()));
                                answer.kinds.insert(BodyKind::Envelope)
                            }
                            _ => {
                                return Err(format!(
                                    "describes {status} with the content `{media}` {}, which is neither text/plain nor the Error envelope",
                                    entry["schema"]
                                ))
                            }
                        };
                    }
                }
            }
        }
        Ok(answers)
    }

    fn resolve_response<'a>(
        &'a self,
        response: &'a serde_json::Value,
    ) -> Result<&'a serde_json::Value, String> {
        let Some(reference) = response["$ref"].as_str() else {
            return Ok(response);
        };
        reference
            .strip_prefix(SHARED_RESPONSES)
            .map(|name| &self.document["components"]["responses"][name])
            .filter(|shared| shared.is_object())
            .ok_or_else(|| {
                format!(
                    "refers to the response `{reference}`, which the description does not define"
                )
            })
    }
}

/// One Rust source of foundry-api or foundry-services, comments, literals and
/// `#[cfg(test)]` items blanked (a test cannot raise a production status).
struct CrateFile {
    rel: String,
    in_api: bool,
    code: String,
    literals: Vec<RustLiteral>,
    starts: Vec<usize>,
}

impl CrateFile {
    fn read(root: &Path, file: &Path, in_api: bool) -> Option<Self> {
        let contents = std::fs::read_to_string(file).ok()?;
        let source = RustSource::scan(&contents);
        let code = blank_test_items(source.code);
        Some(CrateFile {
            rel: rel(root, file),
            in_api,
            starts: line_starts(&code),
            code,
            literals: source.literals,
        })
    }

    fn location(&self, at: usize) -> String {
        format!("{}:{}", self.rel, line_of(&self.starts, at))
    }

    fn literal_at(&self, at: usize) -> Option<&RustLiteral> {
        self.literals.iter().find(|literal| literal.start == at)
    }

    /// Every word occurrence of `word` in `[from, to)`.
    fn words(&self, word: &str, from: usize, to: usize) -> Vec<usize> {
        let mut found = Vec::new();
        let mut at = from;
        while let Some(hit) = find_sql_word(&self.code, word, at).filter(|&hit| hit < to) {
            found.push(hit);
            at = hit + word.len();
        }
        found
    }

    /// `ServiceError::<Variant>` at `at` (the word's offset): the variant and
    /// where it ends.
    fn variant_at(&self, at: usize) -> Option<(String, usize)> {
        let bytes = self.code.as_bytes();
        let colons = skip_whitespace(bytes, at + "ServiceError".len());
        if !self.code[colons..].starts_with("::") {
            return None;
        }
        let start = skip_whitespace(bytes, colons + 2);
        let end = ident_end(bytes, start);
        let variant = &self.code[start..end];
        variant
            .starts_with(|c: char| c.is_ascii_uppercase())
            .then(|| (variant.to_string(), end))
    }

    /// The `code:` literal of the `ServiceError::Validation { … }` whose
    /// variant ends at `end`, or why it cannot be read.
    fn validation_code(&self, end: usize) -> Result<String, String> {
        let bytes = self.code.as_bytes();
        let open = skip_whitespace(bytes, end);
        let close = (bytes.get(open) == Some(&b'{'))
            .then(|| matching_brace(bytes, open))
            .flatten();
        let field = close.and_then(|close| self.words("code", open, close).first().copied());
        let value = field.and_then(|field| {
            let colon = skip_whitespace(bytes, field + "code".len());
            (bytes.get(colon) == Some(&b':') && bytes.get(colon + 1) != Some(&b':'))
                .then(|| skip_whitespace(bytes, colon + 1))
        });
        match value.and_then(|value| self.literal_at(value)) {
            Some(literal) => Ok(literal.text.clone()),
            None => Err(soundness_violation(
                &self.location(end),
                "builds a ServiceError::Validation whose `code` is not a string literal",
            )),
        }
    }

    /// Every call in `[from, to)`: `name(`, `path::name(`, `.name(`, or a
    /// `name::<…>` turbofish. A `fn name(` definition reads as a call too:
    /// following it only re-enters same-named functions, so the set can grow
    /// but never shrink.
    fn calls(&self, from: usize, to: usize) -> Vec<Call> {
        let bytes = self.code.as_bytes();
        let mut calls = Vec::new();
        let mut at = from;
        while at < to {
            let start = at;
            at = ident_end(bytes, at);
            if at == start {
                at += 1;
                continue;
            }
            let next = skip_whitespace(bytes, at);
            if bytes.get(next) == Some(&b'(') || self.code[next..].starts_with("::<") {
                calls.push(Call {
                    name: self.code[start..at].to_string(),
                    at: start,
                    receiver: Receiver::of(receiver_name(&self.code, start)),
                });
            }
        }
        calls
    }
}

fn is_rust_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn ident_end(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(|&b| is_rust_ident_byte(b)) {
        at += 1;
    }
    at
}

fn skip_whitespace_back(bytes: &[u8], mut at: usize) -> usize {
    while at > 0 && bytes[at - 1].is_ascii_whitespace() {
        at -= 1;
    }
    at
}

/// The identifier right before the `.` of a method call whose name starts at
/// `at` (`store` in `store\n    .find(`), or `""` for any other call.
fn receiver_name(code: &str, at: usize) -> &str {
    let bytes = code.as_bytes();
    let dot = skip_whitespace_back(bytes, at);
    if dot == 0 || bytes[dot - 1] != b'.' {
        return "";
    }
    let end = skip_whitespace_back(bytes, dot - 1);
    let mut start = end;
    while start > 0 && is_rust_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    &code[start..end]
}

/// What a call is made on, for the two receivers the walk treats specially.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Receiver {
    /// A free or path call, or a method on anything else.
    Path,
    /// A method on a `Store` value (`store.…(`, `self.store.…(`): not
    /// followed, because a store error reaches `ServiceError` only through a
    /// token the caller holds. A later link of the chain is followed like any
    /// call, which can only grow the set.
    Store,
    /// `services.<method>(`: must be a method of `impl Services`.
    Services,
}

impl Receiver {
    fn of(receiver: &str) -> Self {
        match receiver {
            "store" => Receiver::Store,
            "services" => Receiver::Services,
            _ => Receiver::Path,
        }
    }
}

struct Call {
    name: String,
    at: usize,
    receiver: Receiver,
}

/// Blank every `#[cfg(test)]` item: through its block, or its `;`.
fn blank_test_items(code: String) -> String {
    const ATTRIBUTE: &str = "#[cfg(test)]";
    let mut bytes = code.into_bytes();
    let mut from = 0;
    while let Some(at) = bytes[from..]
        .windows(ATTRIBUTE.len())
        .position(|window| window == ATTRIBUTE.as_bytes())
        .map(|offset| from + offset)
    {
        let opens = bytes[at..].iter().position(|&b| b == b'{' || b == b';');
        let end = match opens.map(|offset| at + offset) {
            Some(open) if bytes[open] == b'{' => {
                matching_brace(&bytes, open).unwrap_or(bytes.len())
            }
            Some(semicolon) => semicolon,
            None => bytes.len(),
        };
        for byte in bytes.iter_mut().take(end).skip(at) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        from = end;
    }
    String::from_utf8(bytes).unwrap_or_default()
}

/// A crate `fn` the walk can enter.
struct CrateFn {
    file: usize,
    name: String,
    body: (usize, usize),
}

/// What a walk reached: the variants, the Validation codes, and every form it
/// could not read.
#[derive(Default)]
struct Reach {
    variants: BTreeSet<String>,
    codes: BTreeSet<String>,
    unreadable: Vec<String>,
}

/// The `ServiceError` flow through foundry-api and foundry-services.
struct ErrorFlow {
    files: Vec<CrateFile>,
    fns: Vec<CrateFn>,
    facade: BTreeSet<String>,
}

impl ErrorFlow {
    fn read(root: &Path) -> Self {
        let mut flow = ErrorFlow {
            files: Vec::new(),
            fns: Vec::new(),
            facade: BTreeSet::new(),
        };
        for (dir, in_api) in [(SERVICES_SRC, false), (API_SRC, true)] {
            for file in rust_sources(&root.join(dir)) {
                if let Some(crate_file) = CrateFile::read(root, &file, in_api) {
                    flow.add(crate_file);
                }
            }
        }
        flow
    }

    fn add(&mut self, file: CrateFile) {
        let index = self.files.len();
        let spans = fn_spans(&file.code);
        if !file.in_api {
            let facade = impl_blocks(&file.code)
                .into_iter()
                .filter(|(header, _, _)| {
                    impl_target(header) == "Services" && !header.contains(" for ")
                });
            for (_, open, close) in facade {
                let methods = spans.iter().filter(|span| (open..close).contains(&span.at));
                self.facade.extend(methods.map(|span| span.name.clone()));
            }
        }
        self.fns.extend(spans.into_iter().map(|span| CrateFn {
            file: index,
            name: span.name,
            body: span.body,
        }));
        self.files.push(file);
    }

    fn named(&self, name: &str, api_only: bool) -> Vec<usize> {
        (0..self.fns.len())
            .filter(|&i| {
                self.fns[i].name == name && (!api_only || self.files[self.fns[i].file].in_api)
            })
            .collect()
    }

    /// The forms anywhere in both crates that would let a variant arise, or
    /// be named, with no `ServiceError::<Variant>` token for the walk to see.
    fn soundness_violations(&self) -> Vec<String> {
        let mut violations = Vec::new();
        for file in &self.files {
            for at in file.words("ServiceError", 0, file.code.len()) {
                violations.extend(reference_violation(file, at));
            }
            for (header, open, close) in impl_blocks(&file.code) {
                if impl_target(&header) != "ServiceError" {
                    continue;
                }
                let header = header.split_whitespace().collect::<Vec<_>>().join(" ");
                if header.contains("From<") {
                    violations.push(soundness_violation(
                        &file.location(open),
                        &format!("converts into ServiceError (`{header}`), so a variant can arise from `?` with no token"),
                    ));
                } else if let Some(offset) = file.code[open..close].find("Self::") {
                    violations.push(soundness_violation(
                        &file.location(open + offset),
                        "names a variant as `Self::` inside an impl of ServiceError",
                    ));
                }
            }
            let bytes = file.code.as_bytes();
            for at in file.words("enum", 0, file.code.len()) {
                let name = skip_whitespace(bytes, at + "enum".len());
                let open = skip_whitespace(bytes, name + "ServiceError".len());
                if !file.code[name..].starts_with("ServiceError") || bytes.get(open) != Some(&b'{')
                {
                    continue;
                }
                let close = matching_brace(bytes, open).unwrap_or(file.code.len());
                if file.code[open..close].contains("#[from]") {
                    violations.push(soundness_violation(
                        &file.location(open),
                        "declares a ServiceError variant `#[from]`, a conversion",
                    ));
                }
            }
        }
        violations
    }

    /// Walk from `starts`, following crate calls to a fixed point.
    fn reach(&self, starts: Vec<usize>) -> Reach {
        let mut reach = Reach::default();
        let mut seen = BTreeSet::new();
        let mut queue = starts;
        while let Some(index) = queue.pop() {
            if !seen.insert(index) {
                continue;
            }
            let function = &self.fns[index];
            let file = &self.files[function.file];
            let (from, to) = function.body;
            for at in file.words("ServiceError", from, to) {
                let Some((variant, end)) = file.variant_at(at) else {
                    continue; // named by the soundness scan
                };
                if variant == "Validation" {
                    match file.validation_code(end) {
                        Ok(code) => {
                            reach.codes.insert(code);
                        }
                        Err(unreadable) => reach.unreadable.push(unreadable),
                    }
                }
                reach.variants.insert(variant);
            }
            for call in file.calls(from, to) {
                if call.receiver == Receiver::Store || UNWALKED_FNS.contains(&call.name.as_str()) {
                    continue;
                }
                if call.receiver == Receiver::Services && !self.facade.contains(&call.name) {
                    reach.unreadable.push(soundness_violation(
                        &file.location(call.at),
                        &format!("calls `services.{}(`, which no `impl Services` method in {SERVICES_SRC} defines", call.name),
                    ));
                    continue;
                }
                queue.extend(self.named(&call.name, false));
            }
        }
        reach
    }

    /// The answers `op` can give: transport from its handler and route, then
    /// the domain set its walk reaches, mapped through `status_for`.
    fn answers(
        &self,
        code: &ApiCode,
        op: &CodeOperation,
        handler: &Handler,
        map: &StatusMap,
    ) -> Result<Answers, Vec<String>> {
        let mut answers = Answers::default();
        let mut variants: Vec<(String, String)> = Vec::new();
        let mut starts = self.named(&op.handler, true);
        let none = BTreeSet::new();
        if generic_argument(&handler.returns, "Result").is_some_and(|r| r.contains("ApiError")) {
            variants.push(("Internal".into(), "its ApiError error type".into()));
        }
        if find_sql_word(&handler.params, BEARER_EXTRACTOR, 0).is_some() {
            let extractor = self.named(EXTRACTOR_START, true);
            if extractor.is_empty() {
                return Err(vec![statuses_violation(&format!(
                    "{} ({}) takes {BEARER_EXTRACTOR}, but no `fn {EXTRACTOR_START}` in {API_SRC} can be walked; the rule fails closed",
                    op.label(), op.location
                ))]);
            }
            starts.extend(extractor);
            variants.push((
                "Unauthorized".into(),
                format!("its {BEARER_EXTRACTOR} extractor"),
            ));
        }
        if let Ok(parameters) = handler.path_parameters(&op.template) {
            if parameters.iter().any(|p| p.rust != "String") {
                answers.add(
                    400,
                    BodyKind::PlainText,
                    &none,
                    "its Path<…> holds a segment that may not parse".into(),
                );
            }
        }
        if let Some(body) = handler.request_body() {
            for status in [400, 413, 415, 422] {
                answers.add(
                    status,
                    BodyKind::PlainText,
                    &none,
                    format!("its Json<{body}> body: 400, 413, 415 and 422 in text/plain"),
                );
            }
        }
        if find_sql_word(&handler.params, RATE_GUARD, 0).is_some() {
            let codes = BTreeSet::from([map.rate_limited.clone().map_err(|e| vec![e])?]);
            answers.add(
                429,
                BodyKind::Envelope,
                &codes,
                format!("its {RATE_GUARD} answers {RATE_LIMITED_FN}()"),
            );
        }
        let offered = code
            .operations
            .iter()
            .filter(|other| other.template == op.template)
            .count();
        if op.beside_any {
            variants.push((
                "NotFound".into(),
                "its route's any(api_not_found_handler)".into(),
            ));
        } else if offered < API_METHODS.len() {
            answers.add(
                405,
                BodyKind::Empty,
                &none,
                format!(
                    "its path offers {offered} of the {} methods",
                    API_METHODS.len()
                ),
            );
        }
        let reach = self.reach(starts);
        if !reach.unreadable.is_empty() {
            return Err(reach.unreadable);
        }
        variants.extend(reach.variants.iter().map(|variant| {
            (
                variant.clone(),
                format!("`ServiceError::{variant}` is reachable from its handler or extractor"),
            )
        }));
        let mut unmapped = Vec::new();
        for (variant, because) in variants {
            match map.arms.get(&variant).map(|arm| (arm.status, arm.code.clone())) {
                Some((status, Some(code))) => {
                    answers.add(status, BodyKind::Envelope, &BTreeSet::from([code]), because)
                }
                Some((status, None)) => {
                    answers.add(status, BodyKind::Envelope, &reach.codes, format!("{because}, with the code(s) {:?}", reach.codes))
                }
                None => unmapped.push(statuses_violation(&format!(
                    "{} ({}) can raise ServiceError::{variant} ({because}), but status_for has no readable arm for it",
                    op.label(), op.location
                ))),
            }
        }
        if unmapped.is_empty() {
            Ok(answers)
        } else {
            Err(unmapped)
        }
    }
}

/// `impl … {` blocks: the header text and the block's brace offsets.
fn impl_blocks(code: &str) -> Vec<(String, usize, usize)> {
    let bytes = code.as_bytes();
    let mut blocks = Vec::new();
    let mut from = 0;
    while let Some(at) = find_sql_word(code, "impl", from) {
        from = at + "impl".len();
        let Some(open) = code[from..].find(['{', ';']).map(|i| from + i) else {
            break;
        };
        if bytes[open] == b'{' {
            let close = matching_brace(bytes, open).unwrap_or(code.len());
            blocks.push((code[at..open].to_string(), open, close));
        }
    }
    blocks
}

/// The type an `impl` header implements on: `impl From<X> for a::T` → `T`.
fn impl_target(header: &str) -> &str {
    let target = header.split_whitespace().last().unwrap_or_default();
    target.rsplit("::").next().unwrap_or(target)
}

/// A `ServiceError` reference that is not `ServiceError::<Variant>` or a type:
/// a glob or brace import, a lowercase path, or an alias.
fn reference_violation(file: &CrateFile, at: usize) -> Option<String> {
    let bytes = file.code.as_bytes();
    let after = skip_whitespace(bytes, at + "ServiceError".len());
    let rest = &file.code[after..];
    let form = if let Some(path) = rest.strip_prefix("::") {
        let path = path.trim_start();
        (!path.starts_with(|c: char| c.is_ascii_uppercase())).then(|| {
            let shown: String = path.chars().take_while(|&c| !c.is_whitespace() && c != ';').collect();
            format!("refers to `ServiceError::{shown}`; only ServiceError::<Variant> is read, and a glob or brace import hides variants from the walk")
        })
    } else if find_sql_word(rest, "as", 0) == Some(0) {
        Some("imports ServiceError under an alias (`ServiceError as …`), which hides its variants from the walk".to_string())
    } else {
        let statement = file.code[..at]
            .rsplit([';', '{', '}'])
            .next()
            .unwrap_or_default();
        let words: Vec<&str> = statement.split_whitespace().collect();
        (words.last() == Some(&"=") && words.contains(&"type")).then(|| {
            "declares a type alias of ServiceError, which hides its variants from the walk"
                .to_string()
        })
    };
    form.map(|form| soundness_violation(&file.location(at), &form))
}

/// One readable `status_for` arm: the status, and its literal code (`None`
/// for the Validation arm, whose code is the variant's own `code`).
struct StatusArm {
    status: u16,
    code: Option<String>,
}

/// `status_for`'s parsed match arms, and `rate_limited_response`'s code.
struct StatusMap {
    arms: BTreeMap<String, StatusArm>,
    rate_limited: Result<String, String>,
    /// No single `status_for`: a violation once an operation needs the map.
    missing: Option<String>,
    violations: Vec<String>,
}

impl StatusMap {
    fn read(flow: &ErrorFlow) -> Self {
        let mut map = StatusMap {
            arms: BTreeMap::new(),
            rate_limited: flow.single_literal_code(RATE_LIMITED_FN),
            missing: None,
            violations: Vec::new(),
        };
        match flow.single_fn("status_for") {
            Ok((file, body)) => map.read_arms(file, body),
            Err(missing) => map.missing = Some(missing),
        }
        map
    }

    fn read_arms(&mut self, file: &CrateFile, (from, to): (usize, usize)) {
        let bytes = file.code.as_bytes();
        let open = file
            .words("match", from, to)
            .first()
            .and_then(|&at| file.code[at..to].find('{').map(|i| at + i));
        let Some((open, close)) = open.and_then(|open| Some((open, matching_brace(bytes, open)?)))
        else {
            self.violations.push(statuses_violation(&format!(
                "{} status_for has no `match` the rule can read; it maps variants only through its arms",
                file.location(from)
            )));
            return;
        };
        for (start, end) in top_level_pieces(bytes, open + 1, close) {
            if let Err(unreadable) = self.read_arm(file, start, end) {
                self.violations
                    .push(soundness_violation(&file.location(start), &unreadable));
            }
        }
    }

    fn read_arm(&mut self, file: &CrateFile, start: usize, end: usize) -> Result<(), String> {
        let text = &file.code[start..end];
        let arrow = text
            .find("=>")
            .ok_or("status_for has an arm the rule cannot read")?;
        let pattern = text[..arrow].trim();
        if find_sql_word(pattern, "if", 0).is_some() {
            return Err(format!("status_for has a guarded arm (`{pattern}`); a guard makes the mapping depend on a value"));
        }
        let Some(variant) = pattern.strip_prefix("ServiceError::") else {
            return Err(format!("status_for has a wildcard arm (`{pattern}`) or one that is not ServiceError::<Variant>"));
        };
        let variant: String = variant
            .chars()
            .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
            .collect();
        let bytes = file.code.as_bytes();
        let tuple = skip_whitespace(bytes, start + arrow + 2);
        let close = (bytes.get(tuple) == Some(&b'('))
            .then(|| matching_brace(bytes, tuple))
            .flatten()
            .filter(|&close| close < end)
            .ok_or(format!("status_for maps ServiceError::{variant} to something other than a (status, code, …) tuple"))?;
        let elements = top_level_pieces(bytes, tuple + 1, close);
        let status = elements
            .first()
            .and_then(|&(s, e)| file.code[s..e].trim().parse::<u16>().ok())
            .ok_or(format!("status_for maps ServiceError::{variant} to a status that is not an integer literal"))?;
        let code_at = elements.get(1).map(|&(s, _)| skip_whitespace(bytes, s));
        let code = match code_at {
            Some(at) if file.literal_at(at).is_some() => file.literal_at(at).map(|l| l.text.clone()),
            Some(at) if find_sql_word(&file.code, "code", at) == Some(at) && find_sql_word(pattern, "code", 0).is_some() => None,
            _ => return Err(format!("status_for maps ServiceError::{variant} to a code that is neither a string literal nor the Validation `code`")),
        };
        self.arms.insert(variant, StatusArm { status, code });
        Ok(())
    }
}

impl ErrorFlow {
    /// The one foundry-api `fn name`, or why there is not exactly one.
    fn single_fn(&self, name: &str) -> Result<(&CrateFile, (usize, usize)), String> {
        match self.named(name, true).as_slice() {
            [one] => Ok((&self.files[self.fns[*one].file], self.fns[*one].body)),
            found => Err(statuses_violation(&format!(
                "{} `fn {name}` found in {API_SRC} where the rule needs exactly one; it fails closed",
                found.len()
            ))),
        }
    }

    /// The `code: "<literal>"` of the one foundry-api `fn name`.
    fn single_literal_code(&self, name: &str) -> Result<String, String> {
        let (file, (from, to)) = self.single_fn(name)?;
        let bytes = file.code.as_bytes();
        file.words("code", from, to)
            .into_iter()
            .find_map(|at| {
                let colon = skip_whitespace(bytes, at + "code".len());
                (bytes.get(colon) == Some(&b':'))
                    .then(|| file.literal_at(skip_whitespace(bytes, colon + 1)))
                    .flatten()
                    .map(|literal| literal.text.clone())
            })
            .ok_or_else(|| {
                statuses_violation(&format!(
                    "{} `fn {name}` builds no `code: \"<literal>\"`; the rule fails closed",
                    file.location(from)
                ))
            })
    }
}

/// `[from, to)` split at the commas outside any `(…)`, `[…]` or `{…}`,
/// blank pieces dropped.
fn top_level_pieces(bytes: &[u8], from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut pieces = Vec::new();
    let mut depth = 0i32;
    let mut start = from;
    for (at, &byte) in bytes.iter().enumerate().take(to).skip(from) {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                pieces.push((start, at));
                start = at + 1;
            }
            _ => {}
        }
    }
    pieces.push((start, to));
    pieces
        .into_iter()
        .map(|(s, e)| (skip_whitespace(bytes, s), e))
        .filter(|&(s, e)| s < e)
        .collect()
}

/// api-docs-page (slice 03, ADR-API-OPENAPI-003/004, DESIGN check-arch rule 3).
///
/// The API page stays at `/docs/api` (i); only its template loads swagger-ui
/// (ii, D12); its initializer reaches nowhere else, pins every D10 option and
/// names no other description, no storage and no eval (iii, DDD-18); its markup
/// has no inline script, handler or style (iv, DDD-16); its `script-src` is
/// `'self'` alone (v, DDD-19); and the vendored stylesheet loads nothing (vi).
/// A file it cannot read fails the rule rather than passing it.
fn check_api_docs_page(root: &Path) -> Vec<String> {
    let app = root.join("crates").join("foundry-app");
    let templates = app.join("templates");
    let mut violations = docs_page_route_and_policy(root, &app.join("src").join("api_docs.rs"));
    violations.extend(docs_page_swagger_ui_loaders(root, &templates));
    violations.extend(docs_page_initializer(
        root,
        &app.join("static").join("js").join("api-docs.js"),
    ));
    violations.extend(docs_page_template(root, &templates.join("api_docs.html")));
    violations.extend(docs_page_vendored_css(
        root,
        &app.join("static").join("vendor"),
    ));
    violations
}

const DOCS_PAGE_PATH: &str = "/docs/api";
const DOCS_PAGE_DOCUMENT: &str = "\"/api/v1/openapi.json\"";

/// Each option the initializer must set, and its value in whitespace-free
/// spelling. The name is named exactly once in the initializer, as that
/// option: JavaScript keeps the last of two equal keys, so a second
/// `persistAuthorization` could quietly undo the first (review D4).
const DOCS_PAGE_PINNED: [(&str, &str); 4] = [
    ("validatorUrl", "\"none\""),
    ("persistAuthorization", "false"),
    ("queryConfigEnabled", "false"),
    ("withCredentials", "false"),
];

/// Names that choose another description, touch the browser's storage or
/// evaluate text, matched where an identifier starts.
const DOCS_PAGE_FORBIDDEN: [&str; 8] = [
    "urls:",
    "configUrl",
    "spec:",
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "document.cookie",
    "eval(",
];

/// Storage names short or common enough to sit inside another name, matched
/// only as a whole identifier: the Cache API's `caches` (not `cachesize`) and
/// `window.name` (not `window.namespace`) (review D5).
const DOCS_PAGE_FORBIDDEN_WORDS: [&str; 2] = ["caches", "window.name"];

fn docs_page_read(root: &Path, file: &Path) -> Result<String, String> {
    std::fs::read_to_string(file).map_err(|e| {
        format!(
            "api-docs-page: cannot read {} ({e}) — the API page could not be checked.",
            rel(root, file)
        )
    })
}

/// Clauses (i) and (v), both read from `api_docs.rs`.
fn docs_page_route_and_policy(root: &Path, file: &Path) -> Vec<String> {
    let source = match docs_page_read(root, file) {
        Ok(source) => source,
        Err(violation) => return vec![violation],
    };
    let label = rel(root, file);
    let literals = rust_string_literals(&source);
    let constant = |name: &str| -> Option<(usize, String)> {
        let line = source
            .lines()
            .position(|line| line.contains(&format!("const {name}:")))?
            + 1;
        literals.iter().find(|(at, _)| *at == line).cloned()
    };
    let mut violations = Vec::new();
    match constant("API_DOCS_PATH") {
        Some((_, path)) if path == DOCS_PAGE_PATH => {}
        Some((line, path)) => violations.push(format!(
            "api-docs-page: {label}:{line} sets API_DOCS_PATH to {path:?}; the API page stays at \
             /docs/api (DDD-15)."
        )),
        None => violations.push(format!(
            "api-docs-page: {label} has no `const API_DOCS_PATH` string; the API page stays at \
             /docs/api (DDD-15)."
        )),
    }
    let stripped: Vec<String> = source.lines().map(strip_comment).collect();
    let blocks = route_blocks(&stripped);
    if blocks.is_empty() {
        violations.push(format!(
            "api-docs-page: {label} mounts no `.route(`; the API page stays at /docs/api (DDD-15)."
        ));
    }
    for (first, last) in blocks {
        let block = stripped[first..=last].join(" ");
        let after = block.split_once(".route(").map_or("", |(_, rest)| rest);
        let target = after.split(',').next().unwrap_or_default().trim();
        if target != "API_DOCS_PATH" && target != format!("\"{DOCS_PAGE_PATH}\"") {
            violations.push(format!(
                "api-docs-page: {label}:{} routes the API page at {target}; it stays at /docs/api, \
                 through API_DOCS_PATH (DDD-15).",
                first + 1
            ));
        }
    }
    let Some((line, policy)) = constant("API_DOCS_CSP") else {
        violations.push(format!(
            "api-docs-page: {label} has no `const API_DOCS_CSP` string; the page's script-src \
             could not be checked (DDD-19)."
        ));
        return violations;
    };
    violations.extend(docs_page_script_src(&format!("{label}:{line}"), &policy));
    violations
}

/// Clause (v): the policy's `script-src` is `'self'` alone.
fn docs_page_script_src(location: &str, policy: &str) -> Option<String> {
    let script_src = policy
        .split(';')
        .map(|directive| directive.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|directive| directive.split(' ').next() == Some("script-src"));
    match script_src {
        Some(directive) if directive == "script-src 'self'" => None,
        Some(directive) => Some(format!(
            "api-docs-page: {location} API_DOCS_CSP has `{directive}`; the page's script-src \
             is 'self' alone: no 'unsafe-inline', '*', 'unsafe-eval' or host (DDD-19, OQ-U4)."
        )),
        None => Some(format!(
            "api-docs-page: {location} API_DOCS_CSP has no script-src; it is 'self' alone \
             (DDD-19)."
        )),
    }
}

/// Clause (ii): no template but the page's own references swagger-ui.
fn docs_page_swagger_ui_loaders(root: &Path, templates: &Path) -> Vec<String> {
    let own = templates.join("api_docs.html");
    let (files, unreadable) = files_under(templates, "html", &|_| false);
    let mut violations: Vec<String> = unreadable
        .iter()
        .map(|dir| {
            format!(
                "api-docs-page: cannot list {} — no template could be checked for swagger-ui.",
                rel(root, dir)
            )
        })
        .collect();
    for file in files.iter().filter(|file| **file != own) {
        let source = match docs_page_read(root, file) {
            Ok(source) => source,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        for (idx, line) in source.lines().enumerate() {
            if line.contains("/static/vendor/swagger-ui-") {
                violations.push(format!(
                    "api-docs-page: {}:{} loads swagger-ui; only templates/api_docs.html does \
                     (D12).",
                    rel(root, file),
                    idx + 1
                ));
            }
        }
    }
    violations
}

/// `text` with every whitespace character removed and `'` folded to `"`.
fn docs_page_compact(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == '\'' { '"' } else { c })
        .collect()
}

/// A character that continues a JavaScript identifier.
fn is_js_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Where `name` occurs in `text` with no identifier character before it.
fn docs_page_starts(text: &str, name: &str) -> Vec<usize> {
    text.match_indices(name)
        .map(|(at, _)| at)
        .filter(|at| {
            text[..*at]
                .chars()
                .next_back()
                .is_none_or(|c| !is_js_identifier_char(c))
        })
        .collect()
}

/// `name` occurs in `text` where no identifier character precedes it.
fn docs_page_names(text: &str, name: &str) -> bool {
    !docs_page_starts(text, name).is_empty()
}

/// Where `word` occurs in `text` as a whole identifier: no identifier
/// character on either side.
fn docs_page_words(text: &str, word: &str) -> Vec<usize> {
    docs_page_starts(text, word)
        .into_iter()
        .filter(|at| {
            text[at + word.len()..]
                .chars()
                .next()
                .is_none_or(|c| !is_js_identifier_char(c))
        })
        .collect()
}

/// Why the whitespace-free initializer `compact` does not pin `name` to
/// `value`: the name must occur exactly once, as a key (bare or quoted)
/// whose value is `value`.
fn docs_page_pin(compact: &str, name: &str, value: &str) -> Option<String> {
    let at = match docs_page_words(compact, name).as_slice() {
        [at] => *at,
        [] => return Some(format!("does not set `{name}:{value}`")),
        many => {
            return Some(format!(
                "names `{name}` {} times; JavaScript keeps the last, so it is set once, as \
                 `{name}:{value}`",
                many.len()
            ));
        }
    };
    let rest = &compact[at + name.len()..];
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let set = rest.strip_prefix(':').is_some_and(|rest| {
        rest.strip_prefix(value)
            .is_some_and(|after| after.starts_with([',', '}']))
    });
    (!set).then(|| format!("does not set `{name}:{value}`"))
}

/// The initializer's single `url:` option is the description's path, as a
/// literal or through a variable set to it.
fn docs_page_url_is_the_document(compact: &str) -> bool {
    const OPTION: &str = "url:";
    let values: Vec<&str> = compact
        .match_indices(OPTION)
        .filter(|(at, _)| compact[..*at].ends_with(['{', ',']))
        .map(|(at, _)| {
            let rest = &compact[at + OPTION.len()..];
            &rest[..rest.find([',', '}']).unwrap_or(rest.len())]
        })
        .collect();
    let [value] = values.as_slice() else {
        return false;
    };
    let identifier = !value.is_empty() && value.chars().all(is_js_identifier_char);
    *value == DOCS_PAGE_DOCUMENT
        || (identifier && compact.contains(&format!("{value}={DOCS_PAGE_DOCUMENT}")))
}

/// Clause (iii): the initializer, comments stripped.
fn docs_page_initializer(root: &Path, file: &Path) -> Vec<String> {
    let source = match docs_page_read(root, file) {
        Ok(source) => source,
        Err(violation) => return vec![violation],
    };
    let label = rel(root, file);
    let stripped = strip_js_comments(&source);
    let mut violations = Vec::new();
    for (idx, line) in stripped.lines().enumerate() {
        if ["http:", "https:", "//"]
            .iter()
            .any(|remote| line.contains(remote))
        {
            violations.push(format!(
                "api-docs-page: {label}:{} names a remote address; the initializer reaches only \
                 this Foundry (D10).",
                idx + 1
            ));
        }
        let compact = docs_page_compact(line);
        let forbidden = DOCS_PAGE_FORBIDDEN
            .iter()
            .copied()
            .filter(|name| docs_page_names(&compact, name))
            .chain(
                DOCS_PAGE_FORBIDDEN_WORDS
                    .iter()
                    .copied()
                    .filter(|word| !docs_page_words(&compact, word).is_empty()),
            )
            .chain(compact.contains("Function(").then_some("Function("));
        for name in forbidden {
            violations.push(format!(
                "api-docs-page: {label}:{} uses `{name}`; the initializer picks no other \
                 description, touches no storage and evaluates no text (D10, DDD-18).",
                idx + 1
            ));
        }
    }
    let compact = docs_page_compact(&stripped);
    let count = compact.matches(DOCS_PAGE_DOCUMENT).count();
    if count != 1 {
        violations.push(format!(
            "api-docs-page: {label} names \"/api/v1/openapi.json\" {count} times; exactly once, \
             as the url option (D10)."
        ));
    }
    for (name, value) in DOCS_PAGE_PINNED {
        if let Some(why) = docs_page_pin(&compact, name, value) {
            violations.push(format!(
                "api-docs-page: {label} {why}; every D10 option is pinned, once (DDD-18)."
            ));
        }
    }
    if !docs_page_url_is_the_document(&compact) {
        violations.push(format!(
            "api-docs-page: {label} does not set its one url option to \"/api/v1/openapi.json\" \
             (DDD-18)."
        ));
    }
    violations
}

/// What inline code the tag `tag` (lower-cased, `<` to `>`) carries, given
/// the markup `after` it.
fn docs_page_inline_code(tag: &str, after: &str) -> Option<String> {
    if tag.starts_with("<style") {
        return Some("<style> element".to_string());
    }
    if tag.starts_with("<script") {
        let body = after.find("</script").map_or(after, |end| &after[..end]);
        if !body.trim().is_empty() {
            return Some("<script> body".to_string());
        }
    }
    let attributes = tag
        .trim_end_matches('>')
        .split('=')
        .map(|piece| piece.trim_matches(' '))
        .collect::<Vec<_>>()
        .join("=");
    attributes
        .split_whitespace()
        .skip(1)
        .filter_map(|attribute| attribute.split_once('=').map(|(name, _)| name))
        .find_map(|name| {
            let handler = name.strip_prefix("on").is_some_and(|event| {
                !event.is_empty() && event.chars().all(|c| c.is_ascii_alphabetic())
            });
            match name {
                "style" => Some("style= attribute".to_string()),
                _ if handler => Some(format!("{name}= handler")),
                _ => None,
            }
        })
}

/// Clause (iv): Foundry's markup on the page carries no inline code.
fn docs_page_template(root: &Path, file: &Path) -> Vec<String> {
    let source = match docs_page_read(root, file) {
        Ok(source) => source,
        Err(violation) => return vec![violation],
    };
    let label = rel(root, file);
    let lower = source.to_ascii_lowercase();
    let line_of = |at: usize| lower[..at].matches('\n').count() + 1;
    let mut violations = Vec::new();
    let mut close = 0;
    for chunk in lower.split_inclusive('>') {
        let start = close;
        close += chunk.len();
        let Some(offset) = chunk.find('<') else {
            continue;
        };
        if let Some(what) = docs_page_inline_code(&chunk[offset..], &lower[close..]) {
            violations.push(format!(
                "api-docs-page: {label}:{} has an inline {what}; the API page's markup carries \
                 no inline script, handler or style (DDD-16, ADR-API-OPENAPI-004).",
                line_of(start + offset)
            ));
        }
    }
    for (at, _) in lower.match_indices("javascript:") {
        violations.push(format!(
            "api-docs-page: {label}:{} has a `javascript:` address; the API page's markup carries \
             no inline script (DDD-16).",
            line_of(at)
        ));
    }
    violations
}

/// Clause (vi): every vendored `swagger-ui.css` loads nothing — no `@import`,
/// no `@font-face`, and no `url(` but a `data:` URI (review H3).
fn docs_page_vendored_css(root: &Path, vendor: &Path) -> Vec<String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(vendor)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_dir()
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("swagger-ui-"))
                })
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    if dirs.is_empty() {
        return vec![format!(
            "api-docs-page: no vendored swagger-ui-*/swagger-ui.css under {} — the stylesheet \
             could not be checked (D11).",
            rel(root, vendor)
        )];
    }
    let mut violations = Vec::new();
    for dir in dirs {
        let file = dir.join("swagger-ui.css");
        let source = match docs_page_read(root, &file) {
            Ok(source) => source,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        let label = rel(root, &file);
        for (idx, line) in strip_css_block_comments(&source).lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            let at_rules = ["@import", "@font-face"]
                .into_iter()
                .filter(|at_rule| lower.contains(at_rule))
                .map(|at_rule| format!("`{at_rule}`"));
            let urls = css_url_arguments(&lower)
                .into_iter()
                .filter(|argument| !argument.starts_with("data:"))
                .map(|argument| {
                    let shown: String = argument.chars().take(40).collect();
                    format!("a `url(` to {shown:?}")
                });
            for what in at_rules.chain(urls) {
                violations.push(format!(
                    "api-docs-page: {label}:{} has {what}; the vendored stylesheet loads no \
                     remote stylesheet, font or file (review H3).",
                    idx + 1
                ));
            }
        }
    }
    violations
}

/// What each `url(` in a CSS line points at: the argument from just inside its
/// quote (or its first non-blank character) to the end of the line. A `url(`
/// inside an argument is part of that argument, not another reference.
fn css_url_arguments(line: &str) -> Vec<&str> {
    const OPEN: &str = "url(";
    let mut arguments = Vec::new();
    let mut rest = line;
    while let Some(offset) = rest.find(OPEN) {
        let argument = rest[offset + OPEN.len()..].trim_start();
        let quote = argument.chars().next().filter(|c| *c == '"' || *c == '\'');
        let inner = quote.map_or(argument, |_| &argument[1..]);
        arguments.push(inner);
        let end = quote
            .map_or_else(|| inner.find(')'), |q| inner.find(q))
            .unwrap_or(inner.len());
        rest = &inner[end..];
    }
    arguments
}

#[cfg(test)]
mod api_openapi_check_arch_tests {
    //! Injected-violation gold tests for the three api-openapi-document rules
    //! (DESIGN [REF] check-arch Rules, DDD-21; ADR-API-OPENAPI-001/002/003).
    //!
    //! Each test stages a tree (the `project_lifecycle_check_arch_tests` `stage`
    //! helper), plants one violation, runs the rule against it and asserts the
    //! offender is named, so an "always pass" rule fails it.

    // Rule 1: api-openapi-operations (slice 01, step 01-02).
    //
    // Each test stages a small foundry-api (routes + handlers shaped like the
    // shipped ones) and a description of it, plants one drift, and asserts the
    // rule names it. The pending list is passed in, so these tests plant their
    // own (the shipped rule's list was emptied and deleted in slice 02).

    use super::api_openapi_operations;
    use serde_json::{json, Value};

    const ISSUES: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues";
    const HISTORY: &str =
        "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/history";
    const REVOKE: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}";

    const API_LIB: &str = r#"
pub fn routes<S>() -> Router<S> {
    Router::new()
        .route("/api/v1/{*rest}", any(api_not_found_handler))
        .route(
            "/api/v1/openapi.json",
            any(api_not_found_handler).get(openapi::openapi_document),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
            get(list_issues_handler).post(create_issue_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/history",
            get(list_issue_history_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}",
            delete(revoke_token_handler),
        )
}

async fn api_not_found_handler() -> ApiError {
    ApiError(ServiceError::NotFound)
}

async fn list_issues_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<IssueJson>>, ApiError> {
    let rows = services.list_board_issues(&principal, &team_slug, &project_slug).await?;
    Ok(Json(rows.into_iter().map(IssueJson::from).collect()))
}

async fn create_issue_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Response, ApiError> {
    let created = services.create_issue(&principal, &team_slug, &project_slug, &request.title).await?;
    let body = IssueJson {
        key: created.key,
        number: created.number,
    };
    let location = format!("/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{}", body.number);
    let mut response = (StatusCode::CREATED, Json(body)).into_response();
    response.headers_mut().insert(header::LOCATION, location.parse().unwrap());
    Ok(response)
}

async fn list_issue_history_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
) -> Result<Json<Vec<IssueHistoryJson>>, ApiError> {
    let entries = services.list_issue_change_history(&principal, &team_slug, &project_slug, number).await?;
    Ok(Json(entries.into_iter().map(IssueHistoryJson::from).collect()))
}

async fn revoke_token_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, jti)): Path<(String, String, uuid::Uuid)>,
) -> Result<Response, ApiError> {
    services.revoke_project_token(&principal, &team_slug, &project_slug, jti).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
"#;

    const API_OPENAPI_RS: &str = "pub(crate) async fn openapi_document() -> Response {\n    ([(header::CONTENT_TYPE, \"application/json\")], DOCUMENT).into_response()\n}\n";

    fn path_parameter(name: &str, schema: Value) -> Value {
        json!({"name": name, "in": "path", "required": true, "schema": schema})
    }

    fn slugs() -> Vec<Value> {
        vec![
            path_parameter("team_slug", json!({"type": "string"})),
            path_parameter("project_slug", json!({"type": "string"})),
        ]
    }

    fn with(mut parameters: Vec<Value>, more: Value) -> Value {
        parameters.push(more);
        Value::Array(parameters)
    }

    fn answer(schema: Value) -> Value {
        json!({"description": "ok", "content": {"application/json": {"schema": schema}}})
    }

    /// The description of [`API_LIB`], with an error answer on each operation
    /// (non-2xx answers are rule 2's, so they never count as a success).
    fn description() -> Value {
        let issue = json!({"$ref": "#/components/schemas/Issue"});
        let not_found = json!({"$ref": "#/components/responses/NotFound"});
        json!({
            "openapi": "3.1.0",
            "paths": {
                "/api/v1/openapi.json": {
                    "get": {"security": [], "responses": {
                        "200": answer(json!({"type": "object"})),
                        "404": not_found,
                    }}
                },
                ISSUES: {
                    "get": {"parameters": slugs(), "responses": {
                        "200": answer(json!({"type": "array", "items": issue})),
                        "404": not_found,
                    }},
                    "post": {
                        "parameters": slugs(),
                        "requestBody": {"required": true, "content": {"application/json": {
                            "schema": {"$ref": "#/components/schemas/CreateIssueRequest"}
                        }}},
                        "responses": {"201": answer(issue), "404": not_found}
                    }
                },
                HISTORY: {
                    "get": {
                        "parameters": with(slugs(), path_parameter("number", json!({"type": "integer", "format": "int32"}))),
                        "responses": {
                            "200": answer(json!({"type": "array", "items": {"$ref": "#/components/schemas/IssueHistoryEntry"}})),
                            "404": not_found,
                        }
                    }
                },
                REVOKE: {
                    "delete": {
                        "parameters": with(slugs(), path_parameter("jti", json!({"type": "string", "format": "uuid"}))),
                        "responses": {"204": {"description": "revoked"}, "404": not_found}
                    }
                }
            }
        })
    }

    struct ApiTree {
        lib: String,
        description: Value,
        elsewhere: Vec<(&'static str, String)>,
    }

    impl ApiTree {
        fn clean() -> Self {
            ApiTree {
                lib: API_LIB.to_string(),
                description: description(),
                elsewhere: Vec::new(),
            }
        }

        /// Plant one drift in the code: `from` must occur in it.
        fn code(mut self, from: &str, to: &str) -> Self {
            assert!(self.lib.contains(from), "the fixture has no {from:?}");
            self.lib = self.lib.replacen(from, to, 1);
            self
        }

        fn describe(mut self, edit: impl FnOnce(&mut Value)) -> Self {
            edit(&mut self.description);
            self
        }

        fn violations(&self, pending: &[(&str, &str)]) -> Vec<String> {
            let description = serde_json::to_string_pretty(&self.description).expect("encode");
            let mut files: Vec<(&str, &str)> = vec![
                ("crates/foundry-api/src/lib.rs", self.lib.as_str()),
                ("crates/foundry-api/src/openapi.rs", API_OPENAPI_RS),
                ("crates/foundry-api/openapi.json", description.as_str()),
            ];
            files.extend(self.elsewhere.iter().map(|(p, b)| (*p, b.as_str())));
            let tree = stage(&files);
            api_openapi_operations(tree.path(), pending)
        }
    }

    fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel_path, body) in files {
            let path = dir.path().join(rel_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, body).expect("write fixture");
        }
        dir
    }

    /// One violation names every needle.
    fn assert_named(violations: &[String], needles: &[&str]) {
        assert!(
            violations
                .iter()
                .any(|v| needles.iter().all(|needle| v.contains(needle))),
            "no violation names all of {needles:?}: {violations:#?}"
        );
    }

    /// Kills the "always fail" mutant: the staged tree is the shipped shape.
    #[test]
    fn operations_clean_tree_passes() {
        let violations = ApiTree::clean().violations(&[]);
        assert!(violations.is_empty(), "{violations:#?}");
    }

    /// Clause (i), code → description. Kills the "always pass" mutant.
    #[test]
    fn operations_names_an_undocumented_route() {
        let violations = ApiTree::clean()
            .code(
                "    Router::new()\n",
                "    Router::new()\n        .route(\"/api/v1/teams/{team_slug}/projects\", get(list_projects_handler))\n",
            )
            .code(
                "async fn api_not_found_handler",
                "async fn list_projects_handler(Path(team_slug): Path<String>) -> Json<Vec<ProjectJson>> {\n    Json(Vec::new())\n}\n\nasync fn api_not_found_handler",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &["GET /api/v1/teams/{team_slug}/projects ", "undocumented"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Clause (i), description → code.
    #[test]
    fn operations_names_a_documented_operation_the_code_lacks() {
        let violations = ApiTree::clean()
            .code(
                "get(list_issues_handler).post(create_issue_handler)",
                "get(list_issues_handler)",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &[&format!("POST {ISSUES} "), "not offered by the code"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Clause (ii): `{number}` is an `i32`, so `type: string` is a lie.
    #[test]
    fn operations_names_a_mistyped_path_parameter() {
        let violations = ApiTree::clean()
            .describe(|d| {
                d["paths"][HISTORY]["get"]["parameters"][2]["schema"] = json!({"type": "string"})
            })
            .violations(&[]);
        assert_named(
            &violations,
            &[
                &format!("GET {HISTORY} "),
                "`number`",
                "integer",
                "int32",
                "type string",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Clause (ii): a renamed template parameter is named on both sides.
    #[test]
    fn operations_names_a_renamed_path_parameter() {
        let violations = ApiTree::clean()
            .code("issues/{number}/history", "issues/{issue_number}/history")
            .violations(&[]);
        assert_named(
            &violations,
            &["GET ", "/history", "\"issue_number\"", "\"number\""],
        );
        assert_named(&violations, &["`issue_number`", "does not describe"]);
        assert_named(&violations, &["`number`", "the path does not have"]);
    }

    /// Clause (iii): a handler that starts taking a body must describe one.
    #[test]
    fn operations_names_a_new_request_body() {
        let violations = ApiTree::clean()
            .code(
                "Path((team_slug, project_slug, number)): Path<(String, String, i32)>,\n) -> Result<Json<Vec<IssueHistoryJson>>",
                "Path((team_slug, project_slug, number)): Path<(String, String, i32)>,\n    Json(request): Json<CommentBodyRequest>,\n) -> Result<Json<Vec<IssueHistoryJson>>",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &[
                &format!("GET {HISTORY} "),
                "Json<CommentBodyRequest>",
                "#/components/schemas/CommentBodyRequest",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Clause (iv): the success status is read from the handler body.
    #[test]
    fn operations_names_a_changed_success_status() {
        let violations = ApiTree::clean()
            .code("StatusCode::NO_CONTENT", "StatusCode::OK")
            .violations(&[]);
        assert_named(
            &violations,
            &[
                &format!("DELETE {REVOKE} "),
                "answers 200 on success",
                "[\"204\"]",
            ],
        );
    }

    /// DDD-14: a pending operation may be undescribed; nothing else may.
    #[test]
    fn operations_tolerates_only_pending_operations() {
        let undescribed = ApiTree::clean().describe(|d| {
            d["paths"][ISSUES]
                .as_object_mut()
                .expect("issues")
                .remove("post");
        });
        let tolerated = undescribed.violations(&[("POST", ISSUES)]);
        assert!(tolerated.is_empty(), "{tolerated:#?}");

        let other = undescribed.violations(&[("PATCH", HISTORY)]);
        assert_named(&other, &[&format!("POST {ISSUES} "), "undocumented"]);
    }

    /// Clause (vi): an `/api` route mounted outside foundry-api is named.
    #[test]
    fn operations_names_an_api_route_outside_foundry_api() {
        let mut tree = ApiTree::clean();
        tree.elsewhere.push((
            "crates/foundry-app/src/lib.rs",
            "pub fn app() -> Router {\n    Router::new()\n        .route(\"/api/v1/teams\", get(list_teams))\n        .route(\"/teams\", get(teams_page))\n}\n".to_string(),
        ));
        let violations = tree.violations(&[]);
        assert_named(
            &violations,
            &[
                "crates/foundry-app/src/lib.rs:3",
                "`/api/v1/teams`",
                "outside crates/foundry-api/src",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Clause (vii): a path the rule cannot read is named, never skipped.
    #[test]
    fn operations_fails_closed_on_a_non_literal_route() {
        let violations = ApiTree::clean()
            .code(
                "        .route(\n            \"/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}\",",
                "        .route(\n            REVOKE_PATH,",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &["crates/foundry-api/src/lib.rs:", "not a string literal"],
        );
    }

    /// Clause (vii) as DELIVER UI-1 reads it: `any(api_not_found_handler)
    /// .get(..)` belongs to the description's route only, and a chained
    /// `.fallback(` to no route at all.
    #[test]
    fn operations_fails_closed_on_a_stray_fallback() {
        let stray_any = ApiTree::clean()
            .code(
                "get(list_issue_history_handler)",
                "any(api_not_found_handler).get(list_issue_history_handler)",
            )
            .violations(&[]);
        assert_named(
            &stray_any,
            &["`/api/v1/", "/history`", "any(api_not_found_handler)"],
        );
        assert_eq!(stray_any.len(), 1, "{stray_any:#?}");

        let stray_fallback = ApiTree::clean()
            .code(
                "get(list_issue_history_handler)",
                "get(list_issue_history_handler).fallback(api_not_found_handler)",
            )
            .violations(&[]);
        assert_named(
            &stray_fallback,
            &["/history`", "fallback(api_not_found_handler)"],
        );
        assert_eq!(stray_fallback.len(), 1, "{stray_fallback:#?}");

        let description_fallback = ApiTree::clean()
            .code(
                "any(api_not_found_handler).get(openapi::openapi_document)",
                "get(openapi::openapi_document).fallback(api_not_found_handler)",
            )
            .violations(&[]);
        assert_named(
            &description_fallback,
            &["`/api/v1/openapi.json`", "fallback(api_not_found_handler)"],
        );
    }

    /// Clause (iv): an untyped `Response` whose body type cannot be read fails.
    #[test]
    fn operations_fails_closed_on_an_untyped_success_body() {
        let violations = ApiTree::clean()
            .code(
                "Json(body)).into_response()",
                "Json(body.into_view())).into_response()",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &[
                &format!("POST {ISSUES} "),
                "returns `Response`",
                "no single success body type",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    // Rule 2: api-openapi-statuses (slice 01, step 01-03; ADR-API-OPENAPI-002 gold list).
    //
    // Each test stages a small foundry-api (routes, handlers, `status_for`, the
    // `MachinePrincipal` extractor and `token_auth`) and foundry-services (the
    // `Services` facade and the use-cases it delegates to), shaped like the
    // shipped ones, with a description listing every answer they can give;
    // plants one change; and asserts the rule names the operation, the status
    // and the difference.

    use super::api_openapi_statuses;

    const COMMENT: &str =
        "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id}";

    const STATUS_API_LIB: &str = r#"
use foundry_services::{Principal, ServiceError, Services};

pub fn status_for(err: &ServiceError) -> (u16, ErrorBody) {
    let (status, code, message): (u16, &str, String) = match err {
        ServiceError::Unauthorized => (401, "unauthorized", "unauthorized".into()),
        ServiceError::Forbidden => (403, "forbidden", "forbidden".into()),
        ServiceError::NotFound => (404, "not_found", "not found".into()),
        ServiceError::Gone => (410, "gone", "gone".into()),
        ServiceError::Conflict => (409, "conflict", "conflict".into()),
        ServiceError::Validation { code, message } => (422, code.as_str(), message.clone()),
        ServiceError::Internal => (500, "internal", "internal error".into()),
    };
    (status, ErrorBody { error: ErrorDetail { code: code.to_string(), message } })
}

pub struct ApiError(pub ServiceError);

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = status_for(&self.0);
        let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (code, Json(body)).into_response()
    }
}

fn rate_limited_response() -> Response {
    let body = ErrorBody {
        error: ErrorDetail {
            code: "rate_limited".to_string(),
            message: "too many token revocations; slow down".to_string(),
        },
    };
    (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response()
}

pub fn routes<S>() -> Router<S> {
    Router::new()
        .route("/api/v1/{*rest}", any(api_not_found_handler))
        .route(
            "/api/v1/openapi.json",
            any(api_not_found_handler).get(openapi::openapi_document),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
            get(list_issues_handler).post(create_issue_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/history",
            get(list_issue_history_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id}",
            patch(edit_comment_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}",
            delete(revoke_token_handler),
        )
}

async fn api_not_found_handler() -> ApiError {
    ApiError(ServiceError::NotFound)
}

async fn list_issues_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<IssueJson>>, ApiError> {
    let rows = services
        .list_board_issues(&principal, &team_slug, &project_slug)
        .await?;
    Ok(Json(rows.into_iter().map(IssueJson::from).collect()))
}

async fn create_issue_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Response, ApiError> {
    let created = services
        .create_issue(&principal, &team_slug, &project_slug, &request.title)
        .await?;
    let body = IssueJson {
        key: created.key,
        number: created.number,
    };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

async fn list_issue_history_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
) -> Result<Json<Vec<IssueHistoryJson>>, ApiError> {
    let entries = services
        .list_issue_change_history(&principal, &team_slug, &project_slug, number)
        .await?;
    Ok(Json(entries.into_iter().map(IssueHistoryJson::from).collect()))
}

async fn edit_comment_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number, comment_id)): Path<(String, String, i32, uuid::Uuid)>,
    Json(request): Json<CommentBodyRequest>,
) -> Result<Json<CommentJson>, ApiError> {
    let view = services
        .edit_comment(&principal, &team_slug, &project_slug, number, comment_id, &request.body)
        .await?;
    Ok(Json(view.into()))
}

async fn revoke_token_handler(
    State(services): State<Services>,
    State(rate_guard): State<Arc<dyn RevokeRateGuard>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, jti)): Path<(String, String, uuid::Uuid)>,
) -> Result<Response, ApiError> {
    if !rate_guard.check_revoke(principal.user_id()) {
        return Ok(rate_limited_response());
    }
    services
        .revoke_project_token(&principal, &team_slug, &project_slug, jti)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub struct MachinePrincipal(pub Principal);

impl<S> FromRequestParts<S> for MachinePrincipal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let verifier = Arc::<MachineTokenVerifier>::from_ref(state);
        let services = Services::from_ref(state);
        let header = parts.headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok());
        let principal = token_auth::authenticate(header, &verifier, &services).await?;
        Ok(MachinePrincipal(principal))
    }
}

pub mod token_auth {
    use super::*;

    pub fn verify_bearer(
        header: Option<&str>,
        verifier: &MachineTokenVerifier,
    ) -> Result<MachineTokenClaims, ServiceError> {
        let jwt = header.ok_or(ServiceError::Unauthorized)?;
        verifier.verify(jwt).map_err(|_| ServiceError::Unauthorized)
    }

    pub async fn authenticate(
        header: Option<&str>,
        verifier: &MachineTokenVerifier,
        services: &Services,
    ) -> Result<Principal, ServiceError> {
        let claims = verify_bearer(header, verifier)?;
        let active = services.resolve_active_token(claims.jti).await?;
        Ok(Principal::Machine { user_id: active.user_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id() -> uuid::Uuid {
        let _ = ServiceError::Conflict;
        uuid::Uuid::nil()
    }
}
"#;

    const STATUS_SERVICES_LIB: &str = r#"
pub struct Services {
    store: Arc<Store>,
}

impl Services {
    pub async fn list_board_issues(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Vec<BoardIssue>, ServiceError> {
        board::list_board_issues(&self.store, principal, team_slug, project_slug).await
    }

    pub async fn create_issue(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        title: &str,
    ) -> Result<CreatedIssue, ServiceError> {
        issues::create_issue(&self.store, principal, team_slug, project_slug, title).await
    }

    pub async fn list_issue_change_history(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        number: i32,
    ) -> Result<Vec<issues::IssueChangeHistoryEntry>, ServiceError> {
        issues::list_issue_change_history(&self.store, principal, team_slug, project_slug, number)
            .await
    }

    pub async fn edit_comment(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        issue_number: i32,
        comment_id: uuid::Uuid,
        new_body: &str,
    ) -> Result<comments::CommentView, ServiceError> {
        comments::edit_comment(&self.store, principal, team_slug, project_slug, issue_number, comment_id, new_body)
            .await
    }

    pub async fn resolve_active_token(
        &self,
        jti: uuid::Uuid,
    ) -> Result<auth::ActiveMachineToken, ServiceError> {
        auth::resolve_active_token(&self.store, jti).await
    }

    pub async fn revoke_project_token(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        jti: uuid::Uuid,
    ) -> Result<(), ServiceError> {
        tokens::revoke_project_token(&self.store, principal, team_slug, project_slug, jti).await
    }
}

impl Principal {
    pub fn user_id(&self) -> uuid::Uuid {
        self.user_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("unauthorized")]
    Unauthorized,
    #[error("validation: {code}: {message}")]
    Validation { code: String, message: String },
    #[error("gone")]
    Gone,
    #[error("conflict")]
    Conflict,
    #[error("internal error")]
    Internal,
}

pub mod auth {
    use super::*;

    pub async fn resolve_active_token(
        store: &Store,
        jti: uuid::Uuid,
    ) -> Result<ActiveMachineToken, ServiceError> {
        let row = store
            .find_machine_token_by_jti(jti)
            .await
            .map_err(|_| ServiceError::Unauthorized)?
            .ok_or(ServiceError::Unauthorized)?;
        if row.revoked_at.is_some() {
            return Err(ServiceError::Unauthorized);
        }
        Ok(ActiveMachineToken { user_id: row.user_id })
    }
}

#[cfg(test)]
use std::sync::Mutex;

pub mod board {
    use super::*;

    pub async fn list_board_issues(
        store: &Store,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Vec<BoardIssue>, ServiceError> {
        let project = resolve_board_project(store, principal, team_slug, project_slug).await?;
        let rows = store
            .list_issues_by_project(project.id)
            .await
            .map_err(|_| ServiceError::Internal)?;
        Ok(rows.into_iter().map(BoardIssue::from).collect())
    }

    async fn resolve_board_project(
        store: &Store,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<ProjectRow, ServiceError> {
        let team = store
            .find_team_by_slug(team_slug)
            .await
            .map_err(|_| ServiceError::Internal)?
            .ok_or(ServiceError::NotFound)?;
        store
            .find_project_by_slug(team.id, project_slug)
            .await
            .map_err(|_| ServiceError::Internal)?
            .ok_or(ServiceError::NotFound)
    }
}

pub mod comments;
pub mod issues;
pub mod teams;
pub mod tokens;
"#;

    const STATUS_SERVICES_ISSUES: &str = r#"
use crate::{Principal, ServiceError};

pub async fn create_issue(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    title: &str,
) -> Result<CreatedIssue, ServiceError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 256 {
        return Err(ServiceError::Validation {
            code: "title_required".to_string(),
            message: "Title is required".to_string(),
        });
    }
    if title.len() > 262_144 {
        return Err(too_long());
    }
    let project = resolve_member_project(store, principal, team_slug, project_slug).await?;
    store
        .insert_issue(project.id, title)
        .await
        .map_err(|_| ServiceError::Internal)
}

fn too_long() -> ServiceError {
    ServiceError::Validation {
        code: "description_too_long".to_string(),
        message: "Description is too long".to_string(),
    }
}

pub async fn list_issue_change_history(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
) -> Result<Vec<IssueChangeHistoryEntry>, ServiceError> {
    let project = resolve_member_project(store, principal, team_slug, project_slug).await?;
    store
        .list_issue_changes(project.id, number)
        .await
        .map_err(|_| ServiceError::Internal)
}

pub async fn resolve_member_project(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<ProjectRow, ServiceError> {
    store
        .find_member_project(principal.user_id(), team_slug, project_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_description_is_refused() {
        match too_long() {
            ServiceError::Validation { code, .. } => assert_eq!(code, "description_too_long"),
            other => panic!("{other:?}"),
        }
    }
}
"#;

    const STATUS_SERVICES_COMMENTS: &str = r#"
use crate::{Principal, ServiceError};

pub async fn edit_comment(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    issue_number: i32,
    comment_id: uuid::Uuid,
    new_body: &str,
) -> Result<CommentView, ServiceError> {
    let body = validate_body(new_body)?;
    let project =
        crate::issues::resolve_member_project(store, principal, team_slug, project_slug).await?;
    let comment = store
        .find_comment(project.id, issue_number, comment_id)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;
    if comment.deleted {
        return Err(ServiceError::Gone);
    }
    if comment.author_id != principal.user_id() {
        return Err(ServiceError::Forbidden);
    }
    store
        .update_comment(comment_id, &body)
        .await
        .map_err(|_| ServiceError::Internal)
}

fn validate_body(body: &str) -> Result<String, ServiceError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(ServiceError::Validation {
            code: "comment_empty".to_string(),
            message: "Comment cannot be empty".to_string(),
        });
    }
    if body.chars().count() > 65_536 {
        return Err(ServiceError::Validation {
            code: "comment_too_long".to_string(),
            message: "Comment is too long".to_string(),
        });
    }
    Ok(body.to_string())
}
"#;

    /// A web-tier lookup sharing its name with the store method the board gate
    /// calls: a store call is never followed, so its 409 reaches no operation.
    const STATUS_SERVICES_TEAMS: &str = r#"
use crate::{Principal, ServiceError};

pub async fn find_team_by_slug(store: &Store, team_slug: &str) -> Result<TeamRow, ServiceError> {
    Err(ServiceError::Conflict)
}
"#;

    const STATUS_SERVICES_TOKENS: &str = r#"
use crate::{Principal, ServiceError};

pub async fn revoke_project_token(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    jti: uuid::Uuid,
) -> Result<(), ServiceError> {
    require_workspace_admin(store, principal).await?;
    resolve_token_address(store, team_slug, project_slug).await?;
    store
        .revoke_machine_token(jti)
        .await
        .map_err(|_| ServiceError::Internal)
}

async fn require_workspace_admin(store: &Store, principal: &Principal) -> Result<(), ServiceError> {
    let admin = store
        .is_workspace_admin(principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !admin {
        return Err(ServiceError::Forbidden);
    }
    Ok(())
}

async fn resolve_token_address(store: &Store, team_slug: &str, project_slug: &str) -> Result<(), ServiceError> {
    store
        .find_project_by_slugs(team_slug, project_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;
    Ok(())
}
"#;

    fn shared(name: &str) -> Value {
        json!({"$ref": format!("#/components/responses/{name}")})
    }

    fn envelope(codes: &[&str]) -> Value {
        json!({"description": "Refused.", "content": {"application/json": envelope_content(codes)}})
    }

    fn envelope_content(codes: &[&str]) -> Value {
        let examples: serde_json::Map<String, Value> = codes
            .iter()
            .map(|code| {
                let value = json!({"value": {"error": {"code": code, "message": "m"}}});
                (code.to_string(), value)
            })
            .collect();
        json!({"schema": {"$ref": "#/components/schemas/Error"}, "examples": examples})
    }

    fn plain_text() -> Value {
        json!({"description": "From the framework.", "content": {"text/plain": {"schema": {"type": "string"}}}})
    }

    /// The 422 of a body-taking operation: the extractor's text/plain refusal
    /// and the envelope with its Validation codes, under one status key.
    fn unprocessable(codes: &[&str]) -> Value {
        json!({"description": "Unprocessable.", "content": {
            "text/plain": {"schema": {"type": "string"}},
            "application/json": envelope_content(codes),
        }})
    }

    /// The description of the statuses fixture: every answer it can give.
    fn status_description() -> Value {
        json!({
            "openapi": "3.1.0",
            "paths": {
                "/api/v1/openapi.json": {"get": {"responses": {
                    "200": answer(json!({"type": "object"})),
                    "404": shared("NotFound"),
                }}},
                ISSUES: {
                    "get": {"description": "The project's issues.", "responses": {
                        "200": answer(json!({"type": "array", "items": {"$ref": "#/components/schemas/Issue"}})),
                        "401": shared("Unauthorized"),
                        "404": shared("NotFound"),
                        "405": shared("MethodNotAllowed"),
                        "500": shared("Internal"),
                    }},
                    "post": {"responses": {
                        "201": answer(json!({"$ref": "#/components/schemas/Issue"})),
                        "400": shared("BadBody"),
                        "401": shared("Unauthorized"),
                        "404": shared("NotFound"),
                        "405": shared("MethodNotAllowed"),
                        "413": shared("PayloadTooLarge"),
                        "415": plain_text(),
                        "422": unprocessable(&["title_required", "description_too_long"]),
                        "500": shared("Internal"),
                    }}
                },
                HISTORY: {"get": {"responses": {
                    "200": answer(json!({"type": "array"})),
                    "400": shared("BadPath"),
                    "401": shared("Unauthorized"),
                    "404": shared("NotFound"),
                    "405": shared("MethodNotAllowed"),
                    "500": shared("Internal"),
                }}},
                COMMENT: {"patch": {"responses": {
                    "200": answer(json!({"$ref": "#/components/schemas/Comment"})),
                    "400": shared("BadPath"),
                    "401": shared("Unauthorized"),
                    "403": envelope(&["forbidden"]),
                    "404": shared("NotFound"),
                    "405": shared("MethodNotAllowed"),
                    "410": envelope(&["gone"]),
                    "413": shared("PayloadTooLarge"),
                    "415": plain_text(),
                    "422": unprocessable(&["comment_empty", "comment_too_long"]),
                    "500": shared("Internal"),
                }}},
                REVOKE: {"delete": {"responses": {
                    "204": {"description": "Revoked."},
                    "400": shared("BadPath"),
                    "401": shared("Unauthorized"),
                    "403": envelope(&["forbidden"]),
                    "404": shared("NotFound"),
                    "405": shared("MethodNotAllowed"),
                    "429": envelope(&["rate_limited"]),
                    "500": shared("Internal"),
                }}}
            },
            "components": {"responses": {
                "Unauthorized": envelope(&["unauthorized"]),
                "NotFound": envelope(&["not_found"]),
                "Internal": envelope(&["internal"]),
                "MethodNotAllowed": {"description": "No such method.", "headers": {"Allow": {"schema": {"type": "string"}}}},
                "BadPath": plain_text(),
                "BadBody": plain_text(),
                "PayloadTooLarge": plain_text(),
            }}
        })
    }

    const SERVICES_LIB: &str = "crates/foundry-services/src/lib.rs";
    const SERVICES_ISSUES: &str = "crates/foundry-services/src/issues.rs";
    const SERVICES_COMMENTS: &str = "crates/foundry-services/src/comments.rs";
    const SERVICES_TOKENS: &str = "crates/foundry-services/src/tokens.rs";
    const API_LIB_RS: &str = "crates/foundry-api/src/lib.rs";

    struct StatusTree {
        files: Vec<(&'static str, String)>,
        description: Value,
    }

    impl StatusTree {
        fn clean() -> Self {
            StatusTree {
                files: vec![
                    (API_LIB_RS, STATUS_API_LIB.to_string()),
                    (
                        "crates/foundry-api/src/openapi.rs",
                        API_OPENAPI_RS.to_string(),
                    ),
                    (SERVICES_LIB, STATUS_SERVICES_LIB.to_string()),
                    (SERVICES_ISSUES, STATUS_SERVICES_ISSUES.to_string()),
                    (SERVICES_COMMENTS, STATUS_SERVICES_COMMENTS.to_string()),
                    (
                        "crates/foundry-services/src/teams.rs",
                        STATUS_SERVICES_TEAMS.to_string(),
                    ),
                    (SERVICES_TOKENS, STATUS_SERVICES_TOKENS.to_string()),
                ],
                description: status_description(),
            }
        }

        /// Plant one change in `file`: `from` must occur in it.
        fn code(mut self, file: &str, from: &str, to: &str) -> Self {
            let (_, text) = self
                .files
                .iter_mut()
                .find(|(path, _)| *path == file)
                .expect("a staged file");
            assert!(text.contains(from), "{file} has no {from:?}");
            *text = text.replacen(from, to, 1);
            self
        }

        fn add(mut self, file: &'static str, text: &str) -> Self {
            self.files.push((file, text.to_string()));
            self
        }

        fn describe(mut self, edit: impl FnOnce(&mut Value)) -> Self {
            edit(&mut self.description);
            self
        }

        fn violations(&self) -> Vec<String> {
            let description = serde_json::to_string_pretty(&self.description).expect("encode");
            let mut files: Vec<(&str, &str)> = self
                .files
                .iter()
                .map(|(path, text)| (*path, text.as_str()))
                .collect();
            files.push(("crates/foundry-api/openapi.json", description.as_str()));
            let tree = stage(&files);
            api_openapi_statuses(tree.path(), &[])
        }
    }

    /// Kills the "always fail" mutant; a wording edit changes no status.
    #[test]
    fn statuses_clean_tree_and_wording_edit_pass() {
        let clean = StatusTree::clean().violations();
        assert!(clean.is_empty(), "{clean:#?}");

        let reworded = StatusTree::clean()
            .describe(|d| {
                d["paths"][ISSUES]["get"]["description"] = json!("Every issue, newest last.");
                d["components"]["responses"]["NotFound"]["description"] =
                    json!("Nothing is there, or your token cannot reach it.");
            })
            .violations();
        assert!(reworded.is_empty(), "{reworded:#?}");
    }

    /// The domain set: a `Conflict` in a use-case helper reaches list issues.
    #[test]
    fn statuses_names_a_conflict_in_a_use_case() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "        let team = store\n            .find_team_by_slug(",
                "        if team_slug.is_empty() {\n            return Err(ServiceError::Conflict);\n        }\n        let team = store\n            .find_team_by_slug(",
            )
            .violations();
        assert_named(
            &violations,
            &[&format!("GET {ISSUES} "), "409", "does not list"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// foundry-api is in the collector scope: a handler's own body counts.
    #[test]
    fn statuses_names_a_conflict_in_a_handler_body() {
        let violations = StatusTree::clean()
            .code(
                API_LIB_RS,
                "    let rows = services\n",
                "    if team_slug.is_empty() {\n        return Err(ApiError(ServiceError::Conflict));\n    }\n    let rows = services\n",
            )
            .violations();
        assert_named(
            &violations,
            &[&format!("GET {ISSUES} "), "409", "does not list"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// The other direction: a status the code gives that the document dropped.
    #[test]
    fn statuses_names_a_status_removed_from_the_document() {
        let violations = StatusTree::clean()
            .describe(|d| {
                d["paths"][HISTORY]["get"]["responses"]
                    .as_object_mut()
                    .expect("responses")
                    .remove("401");
            })
            .violations();
        assert_named(
            &violations,
            &[&format!("GET {HISTORY} "), "401", "does not list"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// A 422 envelope's codes are the reachable Validation literals.
    #[test]
    fn statuses_names_a_new_validation_code() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_ISSUES,
                "    if title.len() > 262_144 {",
                "    if title.len() > 200 {\n        return Err(ServiceError::Validation {\n            code: \"title_too_long\".to_string(),\n            message: \"Title is too long\".to_string(),\n        });\n    }\n    if title.len() > 262_144 {",
            )
            .violations();
        assert_named(
            &violations,
            &[&format!("POST {ISSUES} "), "422", "title_too_long"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Variants map through `status_for`'s parsed arms, never a fixed table.
    #[test]
    fn statuses_names_a_remapped_variant() {
        let violations = StatusTree::clean()
            .code(
                API_LIB_RS,
                "ServiceError::Gone => (410, \"gone\"",
                "ServiceError::Gone => (404, \"gone\"",
            )
            .violations();
        assert_named(
            &violations,
            &[&format!("PATCH {COMMENT} "), "410", "cannot answer"],
        );
        assert_named(&violations, &[&format!("PATCH {COMMENT} "), "404", "gone"]);
    }

    /// Soundness: a conversion lets a variant arise with no token to see.
    #[test]
    fn statuses_soundness_names_a_from_conversion() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "pub mod auth {",
                "impl From<StoreError> for ServiceError {\n    fn from(_: StoreError) -> Self {\n        ServiceError::Internal\n    }\n}\n\npub mod auth {",
            )
            .violations();
        assert_named(
            &violations,
            &[
                "crates/foundry-services/src/lib.rs:",
                "From<StoreError>",
                "soundness clause",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");

        let derived = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "#[error(\"internal error\")]\n    Internal,",
                "#[error(\"internal error\")]\n    Internal,\n    #[error(\"store\")]\n    Store(#[from] StoreError),",
            )
            .violations();
        assert_named(
            &derived,
            &[
                "crates/foundry-services/src/lib.rs:",
                "#[from]",
                "soundness clause",
            ],
        );
    }

    /// The transport set: a `Json<…>` body brings the extractor's refusals.
    #[test]
    fn statuses_names_new_text_plain_answers() {
        let violations = StatusTree::clean()
            .code(
                API_LIB_RS,
                "Path((team_slug, project_slug, number)): Path<(String, String, i32)>,\n) -> Result<Json<Vec<IssueHistoryJson>>",
                "Path((team_slug, project_slug, number)): Path<(String, String, i32)>,\n    Json(request): Json<CommentBodyRequest>,\n) -> Result<Json<Vec<IssueHistoryJson>>",
            )
            .violations();
        for status in ["413", "415", "422"] {
            assert_named(
                &violations,
                &[
                    &format!("GET {HISTORY} "),
                    &format!("answer {status} "),
                    "Json<CommentBodyRequest>",
                    "400, 413, 415 and 422",
                    "text/plain",
                ],
            );
        }
        assert_eq!(violations.len(), 3, "{violations:#?}");
    }

    #[test]
    fn statuses_fails_closed_on_a_non_literal_code() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_ISSUES,
                "code: \"title_required\".to_string(),",
                "code: TITLE_REQUIRED.to_string(),",
            )
            .violations();
        assert_named(
            &violations,
            &[
                "crates/foundry-services/src/issues.rs:",
                "Validation",
                "not a string literal",
            ],
        );

        let shorthand = StatusTree::clean()
            .code(
                SERVICES_COMMENTS,
                "code: \"comment_empty\".to_string(),",
                "code,",
            )
            .violations();
        assert_named(
            &shorthand,
            &[
                "crates/foundry-services/src/comments.rs:",
                "not a string literal",
            ],
        );
    }

    #[test]
    fn statuses_fails_closed_on_a_glob_import() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_TOKENS,
                "use crate::{Principal, ServiceError};",
                "use crate::{Principal, ServiceError};\nuse crate::ServiceError::*;",
            )
            .violations();
        assert_named(
            &violations,
            &[
                "crates/foundry-services/src/tokens.rs:3",
                "ServiceError::*",
                "soundness clause",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");

        let alias = StatusTree::clean()
            .code(
                SERVICES_TOKENS,
                "use crate::{Principal, ServiceError};",
                "use crate::{Principal, ServiceError as Refusal};",
            )
            .violations();
        assert_named(&alias, &["tokens.rs:2", "alias", "soundness clause"]);

        let type_alias = StatusTree::clean()
            .code(
                SERVICES_TOKENS,
                "use crate::{Principal, ServiceError};",
                "use crate::{Principal, ServiceError};\npub type Refusal = ServiceError;",
            )
            .violations();
        assert_named(
            &type_alias,
            &["tokens.rs:3", "type alias", "soundness clause"],
        );
    }

    #[test]
    fn statuses_fails_closed_on_a_guarded_arm() {
        let violations = StatusTree::clean()
            .code(
                API_LIB_RS,
                "ServiceError::Gone => (410",
                "ServiceError::Gone if cfg!(debug_assertions) => (410",
            )
            .violations();
        assert_named(
            &violations,
            &["crates/foundry-api/src/lib.rs:", "status_for", "guard"],
        );

        let wildcard = StatusTree::clean()
            .code(
                API_LIB_RS,
                "        ServiceError::Internal => (500, \"internal\", \"internal error\".into()),\n",
                "        _ => (500, \"internal\", \"internal error\".into()),\n",
            )
            .violations();
        assert_named(&wildcard, &["status_for", "wildcard"]);
    }

    #[test]
    fn statuses_fails_closed_on_an_unknown_services_method() {
        let violations = StatusTree::clean()
            .code(
                API_LIB_RS,
                ".list_board_issues(&principal",
                ".list_board_issues_for(&principal",
            )
            .violations();
        assert_named(
            &violations,
            &[
                "crates/foundry-api/src/lib.rs:",
                "list_board_issues_for",
                "impl Services",
            ],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// Same-named definitions are unioned: the walk cannot tell them apart.
    #[test]
    fn statuses_unions_same_named_definitions() {
        let violations = StatusTree::clean()
            .add(
                "crates/foundry-services/src/web_board.rs",
                "use crate::ServiceError;\n\nasync fn resolve_board_project(store: &Store) -> Result<ProjectRow, ServiceError> {\n    Err(ServiceError::Conflict)\n}\n",
            )
            .violations();
        assert_named(
            &violations,
            &[&format!("GET {ISSUES} "), "409", "does not list"],
        );
        assert_eq!(violations.len(), 1, "{violations:#?}");
    }

    /// The extractor is a start: what `token_auth` and `resolve_active_token`
    /// reach is added to every operation that takes `MachinePrincipal`.
    #[test]
    fn statuses_walks_the_extractor_start_set() {
        let bearer_without_403 = [
            format!("GET {ISSUES} "),
            format!("POST {ISSUES} "),
            format!("GET {HISTORY} "),
        ];
        let plants = [
            (
                API_LIB_RS,
                "        let claims = verify_bearer(header, verifier)?;\n",
                "        let claims = verify_bearer(header, verifier)?;\n        if claims.scoped {\n            return Err(ServiceError::Forbidden);\n        }\n",
            ),
            (
                SERVICES_LIB,
                "        if row.revoked_at.is_some() {",
                "        if row.suspended {\n            return Err(ServiceError::Forbidden);\n        }\n        if row.revoked_at.is_some() {",
            ),
        ];
        for (file, from, to) in plants {
            let violations = StatusTree::clean().code(file, from, to).violations();
            for operation in &bearer_without_403 {
                assert_named(&violations, &[operation, "403", "does not list"]);
            }
            assert_eq!(violations.len(), 3, "{file}: {violations:#?}");
        }
    }

    /// US-AOD-02 AC 8: the drift check is a step of both pre-release gates.
    #[test]
    fn ci_and_smoke_run_the_drift_check() {
        let source = super::RustSource::scan(include_str!("main.rs"));
        for gate in ["run_ci", "run_smoke"] {
            let span = super::fn_spans(&source.code)
                .into_iter()
                .find(|span| span.name == gate)
                .unwrap_or_else(|| panic!("xtask has no fn {gate}"));
            let literals: Vec<&str> = source
                .literals
                .iter()
                .filter(|l| span.body.0 < l.start && l.end <= span.body.1)
                .map(|l| l.text.as_str())
                .collect();
            for step in ["check-arch", "test", "--workspace"] {
                assert!(literals.contains(&step), "{gate} has no {step:?} step");
            }
            assert!(
                !literals.contains(&"foundry-api"),
                "{gate} excludes or narrows foundry-api, whose openapi tests are the type-derived layer"
            );
        }
    }

    // `cargo xtask openapi` (DDD-13) and its read-only `--check` (DEVOPS R2), step 01-05.
    //
    // Each test stages a crate manifest and a description laid out like the
    // committed one, runs the command through its argument list (`--root` at the
    // staged tree, as the maintainer and the Dockerfile run it) and reads the
    // file back. The description repeats the version literal outside `info`, so
    // a rewrite that touches the first or every match is caught.

    const OPENAPI_JSON: &str = "crates/foundry-api/openapi.json";
    const API_MANIFEST: &str = "crates/foundry-api/Cargo.toml";

    fn manifest_at(release: &str) -> String {
        format!(
            "[package]\nname = \"foundry-api\"\nversion = \"{release}\"\nedition.workspace = true\n\n\
             [dependencies]\nserde = {{ version = \"1\" }}\n"
        )
    }

    fn description_at(version: &str) -> String {
        format!(
            "{{\n  \"openapi\": \"3.1.0\",\n  \"x-pinned\": {{ \"version\": \"{version}\" }},\n  \
             \"info\": {{\n    \"title\": \"Foundry API v1\",\n    \"version\": \"{version}\",\n    \
             \"description\": \"Since {version}.\"\n  }},\n  \"paths\": {{}}\n}}\n"
        )
    }

    fn openapi_tree(release: &str, documented: &str) -> tempfile::TempDir {
        stage(&[
            (API_MANIFEST, &manifest_at(release)),
            (OPENAPI_JSON, &description_at(documented)),
        ])
    }

    fn xtask_openapi(tree: &tempfile::TempDir, check: bool) -> crate::openapi::Outcome {
        let mut args: Vec<String> = Vec::new();
        if check {
            args.push("--check".to_string());
        }
        args.push("--root".to_string());
        args.push(tree.path().display().to_string());
        crate::openapi::execute(&args)
    }

    fn committed(tree: &tempfile::TempDir) -> String {
        std::fs::read_to_string(tree.path().join(OPENAPI_JSON)).expect("read the description")
    }

    #[test]
    fn openapi_refresh_changes_only_the_version() {
        let tree = openapi_tree("0.16.1", "0.16.0");
        let before = committed(&tree);
        let outcome = xtask_openapi(&tree, false);
        assert!(
            outcome.succeeded(),
            "the refresh failed: {}",
            outcome.message
        );
        let info_version = "    \"version\": \"0.16.0\",\n    \"description\"";
        assert_eq!(before.matches(info_version).count(), 1, "fixture layout");
        assert_eq!(
            committed(&tree),
            before.replacen(
                info_version,
                "    \"version\": \"0.16.1\",\n    \"description\"",
                1
            ),
            "the refresh changed more than info.version"
        );
    }

    #[test]
    fn openapi_refresh_is_idempotent() {
        let tree = openapi_tree("0.16.1", "0.16.0");
        let first = xtask_openapi(&tree, false);
        assert!(first.succeeded(), "the refresh failed: {}", first.message);
        let refreshed = committed(&tree);
        let second = xtask_openapi(&tree, false);
        assert!(
            second.succeeded(),
            "the second refresh failed: {}",
            second.message
        );
        assert_eq!(
            committed(&tree),
            refreshed,
            "a second refresh changed the file"
        );
    }

    #[test]
    fn openapi_check_refuses_a_stale_description() {
        let tree = openapi_tree("0.16.1", "0.16.0");
        let outcome = xtask_openapi(&tree, true);
        assert!(!outcome.succeeded(), "--check accepted a stale description");
        for needle in ["0.16.0", "0.16.1", "cargo xtask openapi", OPENAPI_JSON] {
            assert!(
                outcome.message.contains(needle),
                "the refusal does not name {needle:?}: {}",
                outcome.message
            );
        }
    }

    #[test]
    fn openapi_check_leaves_the_file_unchanged() {
        let tree = openapi_tree("0.16.1", "0.16.0");
        let before = committed(&tree);
        let outcome = xtask_openapi(&tree, true);
        assert!(!outcome.succeeded(), "--check accepted a stale description");
        assert_eq!(
            committed(&tree),
            before,
            "a failing --check changed the file"
        );
    }

    #[test]
    fn openapi_check_accepts_a_current_description() {
        let tree = openapi_tree("0.16.1", "0.16.1");
        let before = committed(&tree);
        let outcome = xtask_openapi(&tree, true);
        assert!(
            outcome.succeeded(),
            "--check refused a current description: {}",
            outcome.message
        );
        assert_eq!(committed(&tree), before, "--check changed the file");
    }

    /// DESIGN Handoff, round-2 review L2 (DEVOPS R2): the Dockerfile builder
    /// stage runs `xtask openapi --check` in the same `RUN` as, and before, the
    /// `cargo build … --bin foundry`, so no image builds from a stale
    /// description.
    fn dockerfile_checks_before_building(dockerfile: &str) -> Result<(), String> {
        let joined = dockerfile.replace("\\\n", " ");
        let build = joined
            .lines()
            .find(|line| {
                line.trim_start().starts_with("RUN ")
                    && line.contains("cargo build")
                    && line.contains("--bin foundry")
            })
            .ok_or("Dockerfile: no RUN builds `--bin foundry`")?;
        let check = build.find("xtask -- openapi --check").ok_or(
            "Dockerfile: the RUN that builds foundry does not run `xtask -- openapi --check`",
        )?;
        if check > build.find("cargo build").unwrap_or(0) {
            return Err(
                "Dockerfile: `xtask -- openapi --check` runs after `cargo build … --bin foundry`"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn dockerfile_checks_the_description_before_building_foundry() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let dockerfile = std::fs::read_to_string(root.join("Dockerfile")).expect("read Dockerfile");
        if let Err(problem) = dockerfile_checks_before_building(&dockerfile) {
            panic!("{problem}");
        }
        let check = "    && cargo run --locked --release -q -p xtask -- openapi --check \\\n";
        let build = "    && cargo build --locked --release --target \"$triple\" -p foundry-app --bin foundry \\\n";
        let head = "RUN --mount=type=cache,target=/work/target \\\n    triple=x \\\n";
        let tail = "    && cp a b\n";
        for (planted, what) in [
            (format!("{head}{build}{tail}"), "missing"),
            (format!("{head}{build}{check}{tail}"), "misordered"),
            (
                format!("RUN true \\\n{check}    && true\n{head}{build}{tail}"),
                "in another RUN",
            ),
        ] {
            let verdict = dockerfile_checks_before_building(&planted);
            assert!(
                verdict.as_ref().is_err_and(|p| p.contains("Dockerfile")),
                "a {what} check is not named with the Dockerfile: {verdict:?}"
            );
        }
    }

    // Rules 1 and 2: plants and unit checks for the phase-5 mutation survivors.

    /// The shipped entry points fail closed on a tree with no description.
    #[test]
    fn shipped_rules_fail_closed_on_an_empty_tree() {
        let tree = stage(&[]);
        let operations = super::check_api_openapi_operations(tree.path());
        assert_named(&operations, &["api-openapi-operations"]);
        let statuses = super::check_api_openapi_statuses(tree.path());
        assert_named(&statuses, &["api-openapi-statuses"]);
    }

    /// Clause (iii): a described body the handler does not take, and a body
    /// the description does not require, are both named.
    #[test]
    fn operations_names_a_request_body_the_handler_does_not_take() {
        let unread = ApiTree::clean()
            .describe(|d| {
                d["paths"][HISTORY]["get"]["requestBody"] = json!({"required": true, "content": {
                    "application/json": {"schema": {"$ref": "#/components/schemas/CommentBodyRequest"}}
                }});
            })
            .violations(&[]);
        assert_named(
            &unread,
            &[
                &format!("GET {HISTORY} "),
                "has a request body in the description",
            ],
        );
        let optional = ApiTree::clean()
            .describe(|d| {
                d["paths"][ISSUES]["post"]["requestBody"]["required"] = json!(false);
            })
            .violations(&[]);
        assert_named(
            &optional,
            &[
                &format!("POST {ISSUES} "),
                "Json<CreateIssueRequest>",
                "required",
            ],
        );
    }

    /// Clause (iv): a changed success body is named.
    #[test]
    fn operations_names_a_changed_success_body() {
        let violations = ApiTree::clean()
            .describe(|d| {
                d["paths"][ISSUES]["post"]["responses"]["201"] =
                    answer(json!({"$ref": "#/components/schemas/Comment"}));
            })
            .violations(&[]);
        assert_named(&violations, &[&format!("POST {ISSUES} "), "201"]);
    }

    /// Clause (iv): only a handler returning `Response` has its body read from
    /// a `Json(T { … })` literal; any other unread return type fails closed.
    #[test]
    fn operations_fails_closed_on_an_opaque_return_type() {
        let violations = ApiTree::clean()
            .code(
                "    Json(request): Json<CreateIssueRequest>,\n) -> Result<Response, ApiError> {",
                "    Json(request): Json<CreateIssueRequest>,\n) -> Result<impl IntoResponse, ApiError> {",
            )
            .violations(&[]);
        assert_named(
            &violations,
            &[&format!("POST {ISSUES} "), "no single success body type"],
        );
    }

    /// A single `Path<T>` is one parameter of type T, not a tuple's inside.
    #[test]
    fn a_single_path_parameter_keeps_its_type() {
        let handler = super::Handler {
            params: "Path(jti): Path<Uuid>".to_string(),
            returns: String::new(),
            body: String::new(),
        };
        let parameters = handler.path_parameters("/x/{jti}").expect("one parameter");
        let read: Vec<(&str, &str)> = parameters
            .iter()
            .map(|p| (p.name.as_str(), p.rust.as_str()))
            .collect();
        assert_eq!(read, [("jti", "Uuid")]);
    }

    #[test]
    fn split_top_level_splits_only_outside_brackets() {
        assert_eq!(
            super::split_top_level("String, Vec<(A, B)>, (C, D), E,"),
            ["String", "Vec<(A, B)>", "(C, D)", "E"]
        );
        assert_eq!(super::split_top_level("A<B>, C"), ["A<B>", "C"]);
    }

    #[test]
    fn method_chain_refuses_a_handler_that_is_not_a_path() {
        assert!(super::method_chain("get(a.b)").is_none());
        assert!(super::method_chain("get()").is_none());
        let chain = super::method_chain("get(a::b).post(c)").expect("a chain");
        let links: Vec<(&str, &str)> = chain
            .iter()
            .map(|l| (l.name.as_str(), l.handler.as_str()))
            .collect();
        assert_eq!(links, [("get", "b"), ("post", "c")]);
    }

    #[test]
    fn receiver_name_reads_the_identifier_before_the_dot() {
        assert_eq!(super::skip_whitespace_back(b"  x", 2), 0);
        assert_eq!(super::skip_whitespace_back(b"a  x", 3), 1);
        let code = "store\n    .find(";
        assert_eq!(
            super::receiver_name(code, code.find("find").unwrap()),
            "store"
        );
        assert_eq!(super::receiver_name("find(", 0), "");
        assert_eq!(super::receiver_name(" find(", 1), "");
        assert_eq!(super::receiver_name("x(store).find(", 9), "");
    }

    fn crate_file(code: &str) -> super::CrateFile {
        super::CrateFile {
            rel: "f.rs".to_string(),
            in_api: true,
            code: code.to_string(),
            literals: Vec::new(),
            starts: super::line_starts(code),
        }
    }

    /// `words` and `calls` read `[from, to)`: nothing that starts at `to`.
    #[test]
    fn crate_file_ranges_exclude_their_end() {
        let file = crate_file("a(b(code code");
        let names: Vec<String> = file.calls(0, 2).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["a"]);
        let all: Vec<String> = file.calls(0, 4).into_iter().map(|c| c.name).collect();
        assert_eq!(all, ["a", "b"]);
        assert_eq!(file.words("code", 0, 9), [4]);
        assert_eq!(file.words("code", 0, 13), [4, 9]);
    }

    #[test]
    fn body_kinds_are_named_in_order() {
        use super::BodyKind;
        let all: std::collections::BTreeSet<BodyKind> =
            [BodyKind::Empty, BodyKind::PlainText, BodyKind::Envelope].into();
        assert_eq!(
            BodyKind::labels(&all),
            "the application/json Error envelope and text/plain and no body"
        );
    }

    /// An application/json answer that is not the Error envelope is no
    /// envelope, even with codes in its examples.
    #[test]
    fn statuses_fails_closed_on_a_json_answer_that_is_not_the_envelope() {
        let violations = StatusTree::clean()
            .describe(|d| {
                d["paths"][COMMENT]["patch"]["responses"]["403"] = json!({
                    "description": "x",
                    "content": {"application/json": {
                        "schema": {"$ref": "#/components/schemas/Comment"},
                        "examples": {"forbidden": {"value": {}}}
                    }}
                });
            })
            .violations();
        assert_named(
            &violations,
            &["403", "neither text/plain nor the Error envelope"],
        );
    }

    /// The api≠mint block pass names the line carrying the `post(`.
    #[test]
    fn post_on_the_tokens_collection_names_its_line() {
        let lines: Vec<String> = [
            "fn routes() -> Router {",
            "    let r = Router::new();",
            "    r",
            "        .route(",
            "            \"/api/v1/x/tokens\",",
            "            get(a).post(b),",
            "        )",
            "}",
        ]
        .iter()
        .map(|line| line.to_string())
        .collect();
        let hits = super::post_on_tokens_collection_blocks(&lines);
        let at: Vec<usize> = hits.iter().map(|(line, _)| *line).collect();
        assert_eq!(at, [5]);
    }

    /// A `Self::` variant inside an impl of ServiceError is named at its line.
    #[test]
    fn statuses_names_a_self_variant_at_its_line() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "pub mod auth {",
                "impl ServiceError {\n    pub fn internal() -> Self {\n\n        Self::Internal\n    }\n}\n\npub mod auth {",
            )
            .violations();
        let line = STATUS_SERVICES_LIB
            .lines()
            .position(|l| l.contains("pub mod auth {"))
            .expect("the fixture has `pub mod auth {`")
            + 4;
        assert_named(
            &violations,
            &[
                &format!("crates/foundry-services/src/lib.rs:{line} "),
                "Self::",
            ],
        );
    }

    /// Only ServiceError's own `#[from]` is a conversion; another enum's
    /// (even one whose name is as long) is not, and neither is a Result alias.
    #[test]
    fn statuses_reads_only_service_error_as_service_error() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "pub mod auth {",
                "pub enum StoreFailure {\n    Db(#[from] DbError),\n}\n\npub type ServiceResult<T> = Result<T, ServiceError>;\n\npub mod auth {",
            )
            .violations();
        assert!(violations.is_empty(), "{violations:#?}");
    }

    /// The glob is shown as written, up to its `;`.
    #[test]
    fn statuses_shows_a_glob_import_as_written() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_TOKENS,
                "use crate::{Principal, ServiceError};",
                "use crate::{Principal, ServiceError};\nuse crate::ServiceError::*;",
            )
            .violations();
        assert_named(&violations, &["refers to `ServiceError::*`;"]);
    }

    /// A status_for code that is neither a literal nor the Validation arm's
    /// own `code` fails closed.
    #[test]
    fn statuses_fails_closed_on_a_non_literal_status_for_code() {
        for code in ["FORBIDDEN", "code"] {
            let violations = StatusTree::clean()
                .code(
                    API_LIB_RS,
                    "ServiceError::Forbidden => (403, \"forbidden\",",
                    &format!("ServiceError::Forbidden => (403, {code},"),
                )
                .violations();
            assert_named(
                &violations,
                &[
                    "ServiceError::Forbidden",
                    "neither a string literal nor the Validation `code`",
                ],
            );
        }
    }

    /// Only `impl Services` is the facade: a method of another inherent impl
    /// is still unknown to it.
    #[test]
    fn statuses_facade_is_only_impl_services() {
        let violations = StatusTree::clean()
            .code(
                SERVICES_LIB,
                "pub mod auth {",
                "pub struct Other;\n\nimpl Other {\n    pub async fn list_board_issues_for(&self) {}\n}\n\npub mod auth {",
            )
            .code(
                API_LIB_RS,
                ".list_board_issues(&principal",
                ".list_board_issues_for(&principal",
            )
            .violations();
        assert_named(&violations, &["list_board_issues_for", "impl Services"]);
    }

    // Rule 3: api-docs-page (slice 03, step 03-03).
    //
    // Each test stages the page's five files (route module, its template, one
    // other template, the initializer and the vendored stylesheet) in their
    // shipped shape, plants one fault, and asserts the rule names the file and
    // what is wrong.

    use super::check_api_docs_page;

    const DOCS_RS: &str = "crates/foundry-app/src/api_docs.rs";
    const DOCS_HTML: &str = "crates/foundry-app/templates/api_docs.html";
    const BOARD_HTML: &str = "crates/foundry-app/templates/board.html";
    const DOCS_JS: &str = "crates/foundry-app/static/js/api-docs.js";
    const DOCS_CSS: &str = "crates/foundry-app/static/vendor/swagger-ui-5.33.3/swagger-ui.css";

    const DOCS_RS_BODY: &str = r#"
/// The page's address (DDD-15).
pub const API_DOCS_PATH: &str = "/docs/api";

pub const API_DOCS_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'";

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route(
        API_DOCS_PATH,
        get(show).layer(middleware::from_fn(api_docs_headers)),
    )
}
"#;

    const DOCS_HTML_BODY: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Foundry API</title>
<script defer src="/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js"></script>
<link rel="stylesheet" href="/static/vendor/swagger-ui-5.33.3/swagger-ui.css">
<script defer src="/static/js/api-docs.js"></script>
</head>
<body>
<h1>Foundry API</h1>
<p>Foundry API v{{ version }}. The OpenAPI document is at <a href="/api/v1/openapi.json">/api/v1/openapi.json</a>.</p>
<noscript><p>This page needs JavaScript.</p></noscript>
<div id="swagger-ui"></div>
</body>
</html>
"#;

    const BOARD_HTML_BODY: &str = r#"{% extends "base.html" %}
{% block head %}<script defer src="/static/js/board.js"></script>{% endblock %}
"#;

    const DOCS_JS_BODY: &str = r##"// The initializer. A comment may name https://validator.swagger.io,
// urls:, spec: or localStorage: comments are not code.
(function () {
  "use strict";
  /* configUrl in a block comment is not code either */
  var DOCUMENT_URL = "/api/v1/openapi.json";
  window.SwaggerUIBundle({
    url: DOCUMENT_URL,
    dom_id: "#swagger-ui",
    queryConfigEnabled: false,
    validatorUrl: "none",
    persistAuthorization: false,
    withCredentials: false,
  });
})();
"##;

    const DOCS_CSS_BODY: &str = "/*! swagger-ui; @import url(https://fonts.example/x.css) in a comment */\n.swagger-ui .a{background:url(\"data:image/svg+xml;charset=utf-8,%3Csvg%3E\")}.swagger-ui .b{background:url( data:image/png;base64,AAAA )}\n";

    /// The page's files in their shipped shape; `plant` changes one of them.
    struct DocsTree {
        files: Vec<(&'static str, String)>,
    }

    impl DocsTree {
        fn clean() -> Self {
            Self {
                files: vec![
                    (DOCS_RS, DOCS_RS_BODY.to_string()),
                    (DOCS_HTML, DOCS_HTML_BODY.to_string()),
                    (BOARD_HTML, BOARD_HTML_BODY.to_string()),
                    (DOCS_JS, DOCS_JS_BODY.to_string()),
                    (DOCS_CSS, DOCS_CSS_BODY.to_string()),
                ],
            }
        }

        /// Replace `from` with `to` in `file`; a plant that misses is a test bug.
        fn plant(mut self, file: &str, from: &str, to: &str) -> Self {
            let body = &mut self
                .files
                .iter_mut()
                .find(|(path, _)| *path == file)
                .expect("a staged file")
                .1;
            assert!(body.contains(from), "the fixture {file} has no {from:?}");
            *body = body.replacen(from, to, 1);
            self
        }

        fn violations(&self) -> Vec<String> {
            let files: Vec<(&str, &str)> = self
                .files
                .iter()
                .map(|(path, body)| (*path, body.as_str()))
                .collect();
            let tree = stage(&files);
            check_api_docs_page(tree.path())
        }
    }

    /// Kills the "always fail" mutant, on the fixture and on the shipped tree.
    #[test]
    fn docs_page_clean_tree_passes() {
        let violations = DocsTree::clean().violations();
        assert!(violations.is_empty(), "{violations:#?}");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let shipped = check_api_docs_page(&root);
        assert!(shipped.is_empty(), "{shipped:#?}");
    }

    /// Clause (i): the page stays at /docs/api, by the constant or by the route.
    #[test]
    fn docs_page_names_a_moved_route() {
        let moved_constant = DocsTree::clean()
            .plant(DOCS_RS, "\"/docs/api\"", "\"/docs/openapi\"")
            .violations();
        assert_named(
            &moved_constant,
            &["api-docs-page", "api_docs.rs", "/docs/openapi"],
        );
        let moved_route = DocsTree::clean()
            .plant(DOCS_RS, "        API_DOCS_PATH,", "        \"/swagger\",")
            .violations();
        assert_named(&moved_route, &["api-docs-page", "api_docs.rs", "/swagger"]);
        let no_route = DocsTree::clean()
            .plant(DOCS_RS, "Router::new().route(", "Router::new().nest(")
            .violations();
        assert_named(&no_route, &["api-docs-page", "api_docs.rs", "/docs/api"]);
    }

    /// Clause (ii): no other template loads swagger-ui (D12).
    #[test]
    fn docs_page_names_another_template_loading_swagger_ui() {
        let violations = DocsTree::clean()
            .plant(
                BOARD_HTML,
                "/static/js/board.js",
                "/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js",
            )
            .violations();
        assert_named(&violations, &["api-docs-page", "board.html", "swagger-ui"]);
    }

    /// Clause (iii): no remote address in the initializer, comments aside.
    #[test]
    fn docs_page_names_a_remote_url_in_the_initializer() {
        for remote in [
            "http://foundry.example/api/v1/openapi.json",
            "https://foundry.example/api/v1/openapi.json",
            "//foundry.example/api/v1/openapi.json",
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_JS,
                    "dom_id: \"#swagger-ui\",",
                    &format!("dom_id: \"#swagger-ui\", oauth2RedirectUrl: \"{remote}\","),
                )
                .violations();
            assert_named(&violations, &["api-docs-page", "api-docs.js:9", "remote"]);
        }
        let twice = DocsTree::clean()
            .plant(
                DOCS_JS,
                "url: DOCUMENT_URL,",
                "url: \"/api/v1/openapi.json\",",
            )
            .violations();
        assert_named(
            &twice,
            &[
                "api-docs-page",
                "api-docs.js",
                "/api/v1/openapi.json",
                "exactly once",
            ],
        );
        let elsewhere = DocsTree::clean()
            .plant(
                DOCS_JS,
                "url: DOCUMENT_URL,",
                "url: location.hash.slice(1),",
            )
            .violations();
        assert_named(&elsewhere, &["api-docs-page", "api-docs.js", "url"]);
    }

    /// Clause (iii): every D10 option is pinned explicitly (DDD-18).
    #[test]
    fn docs_page_names_a_missing_pinned_option() {
        for (from, to, option) in [
            ("    validatorUrl: \"none\",\n", "", "validatorUrl"),
            (
                "validatorUrl: \"none\"",
                "validatorUrl: null",
                "validatorUrl",
            ),
            (
                "persistAuthorization: false",
                "persistAuthorization: true",
                "persistAuthorization",
            ),
            ("    queryConfigEnabled: false,\n", "", "queryConfigEnabled"),
            (
                "withCredentials: false",
                "withCredentials: true",
                "withCredentials",
            ),
            ("    url: DOCUMENT_URL,\n", "", "url"),
        ] {
            let violations = DocsTree::clean().plant(DOCS_JS, from, to).violations();
            assert_named(&violations, &["api-docs-page", "api-docs.js", option]);
        }
    }

    /// Clause (iii): a pinned option set twice is named, because JavaScript
    /// keeps the last of two equal keys (review D4).
    #[test]
    fn docs_page_names_a_pinned_option_set_twice() {
        for (planted, option) in [
            ("persistAuthorization: true,", "persistAuthorization"),
            ("\"persistAuthorization\": true,", "persistAuthorization"),
            ("withCredentials: false,", "withCredentials"),
            ("'validatorUrl': \"none\",", "validatorUrl"),
            ("queryConfigEnabled: true,", "queryConfigEnabled"),
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_JS,
                    "    withCredentials: false,\n",
                    &format!("    withCredentials: false,\n    {planted}\n"),
                )
                .violations();
            assert_named(
                &violations,
                &["api-docs-page", "api-docs.js", option, "2 times"],
            );
        }
        for (from, to, option) in [
            (
                "persistAuthorization: false",
                "persistAuthorization: falsey",
                "persistAuthorization",
            ),
            (
                "persistAuthorization: false",
                "persistAuthorization: false || x",
                "persistAuthorization",
            ),
            (
                "withCredentials: false",
                "withCredentials = false",
                "withCredentials",
            ),
        ] {
            let violations = DocsTree::clean().plant(DOCS_JS, from, to).violations();
            assert_named(
                &violations,
                &["api-docs-page", "api-docs.js", option, "does not set"],
            );
        }
        let quoted = DocsTree::clean()
            .plant(
                DOCS_JS,
                "persistAuthorization: false",
                "\"persistAuthorization\": false",
            )
            .violations();
        assert!(
            quoted.is_empty(),
            "a quoted pinned key is still that key: {quoted:#?}"
        );
        let last = DocsTree::clean()
            .plant(
                DOCS_JS,
                "    withCredentials: false,\n",
                "    withCredentials: false\n",
            )
            .violations();
        assert!(last.is_empty(), "the last option needs no comma: {last:#?}");
    }

    /// Clause (iii): the Cache API, IndexedDB and `window.name` are storage too
    /// (review D5); a name that only contains a forbidden one is not.
    #[test]
    fn docs_page_names_the_other_browser_stores() {
        for (planted, name) in [
            ("x: indexedDB.open(\"d\"),", "indexedDB"),
            ("x: caches.open(\"c\"),", "caches"),
            ("x: self.caches,", "caches"),
            ("x: window.name,", "window.name"),
            ("x: localStorage,", "localStorage"),
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_JS,
                    "dom_id: \"#swagger-ui\",",
                    &format!("dom_id: \"#swagger-ui\", {planted}"),
                )
                .violations();
            assert_named(&violations, &["api-docs-page", "api-docs.js:9", name]);
        }
        for innocent in [
            "x: my_localStorage,",
            "x: $localStorage,",
            "x: mylocalStorage,",
            "x: cachesize,",
            "x: mycaches,",
            "x: my_caches,",
            "x: $caches,",
            "x: caches_,",
            "x: caches$,",
            "x: window.namespace,",
            "x: window.name_,",
            "x: window.name$,",
            "x: \"url:other\",",
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_JS,
                    "dom_id: \"#swagger-ui\",",
                    &format!("dom_id: \"#swagger-ui\", {innocent}"),
                )
                .violations();
            assert!(
                violations.is_empty(),
                "{innocent:?} was named: {violations:#?}"
            );
        }
    }

    /// Clause (iii): the url option may come through any identifier set to the
    /// document, `_` and `$` included, but not through an expression.
    #[test]
    fn docs_page_url_through_a_variable() {
        for name in ["DOC_URL", "$doc", "doc1"] {
            let violations = DocsTree::clean()
                .plant(DOCS_JS, "var DOCUMENT_URL =", &format!("var {name} ="))
                .plant(DOCS_JS, "url: DOCUMENT_URL,", &format!("url: {name},"))
                .violations();
            assert!(violations.is_empty(), "url through {name}: {violations:#?}");
        }
        for (from, to) in [
            ("url: DOCUMENT_URL,", "url: DOCUMENT_URL + x,"),
            ("url: DOCUMENT_URL,", "url: OTHER,"),
            ("url: DOCUMENT_URL,", "url: ,"),
            ("var DOCUMENT_URL = ", "var DOCUMENT_URL = x || "),
        ] {
            let violations = DocsTree::clean().plant(DOCS_JS, from, to).violations();
            assert_named(&violations, &["api-docs-page", "api-docs.js", "url option"]);
        }
        let first = DocsTree::clean()
            .plant(DOCS_JS, "    url: DOCUMENT_URL,\n", "")
            .plant(
                DOCS_JS,
                "window.SwaggerUIBundle({",
                "window.SwaggerUIBundle({url: DOCUMENT_URL,",
            )
            .violations();
        assert!(first.is_empty(), "url first in the object: {first:#?}");
        let last = DocsTree::clean()
            .plant(DOCS_JS, "    url: DOCUMENT_URL,\n", "")
            .plant(
                DOCS_JS,
                "withCredentials: false,",
                "withCredentials: false, url: DOCUMENT_URL",
            )
            .violations();
        assert!(last.is_empty(), "url last in the object: {last:#?}");
    }

    /// Clause (iii): nothing that picks another description or touches storage
    /// or evaluates text.
    #[test]
    fn docs_page_names_a_forbidden_initializer_name() {
        for (planted, name) in [
            ("urls: [],", "urls:"),
            ("configUrl: DOCUMENT_URL,", "configUrl"),
            ("spec: {},", "spec:"),
            ("x: localStorage.length,", "localStorage"),
            ("x: sessionStorage.length,", "sessionStorage"),
            ("x: document.cookie,", "document.cookie"),
            ("x: eval(\"1\"),", "eval("),
            ("x: new Function(\"1\"),", "Function("),
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_JS,
                    "dom_id: \"#swagger-ui\",",
                    &format!("dom_id: \"#swagger-ui\", {planted}"),
                )
                .violations();
            assert_named(&violations, &["api-docs-page", "api-docs.js:9", name]);
        }
    }

    /// Clause (iv): Foundry's markup carries no inline code (DDD-16).
    #[test]
    fn docs_page_names_inline_code_in_the_template() {
        for (from, to, what) in [
            (
                "<div id=\"swagger-ui\"></div>",
                "<script>window.x = 1;</script>",
                "script",
            ),
            (
                "<div id=\"swagger-ui\"></div>",
                "<div id=\"swagger-ui\" onclick=\"go()\"></div>",
                "onclick",
            ),
            (
                "<div id=\"swagger-ui\"></div>",
                "<div id=\"swagger-ui\" ONLOAD = \"go()\"></div>",
                "onload",
            ),
            (
                "<div id=\"swagger-ui\"></div>",
                "<div id=\"swagger-ui\" style=\"color: red\"></div>",
                "style",
            ),
            (
                "<div id=\"swagger-ui\"></div>",
                "<style>p { color: red }</style>",
                "style",
            ),
            (
                "<div id=\"swagger-ui\"></div>",
                "<a href=\"javascript:go()\">go</a>",
                "javascript:",
            ),
        ] {
            let violations = DocsTree::clean().plant(DOCS_HTML, from, to).violations();
            assert_named(&violations, &["api-docs-page", "api_docs.html:14", what]);
        }
    }

    /// Clause (iv): an attribute only named `on`, or `on` and no letter, is no
    /// handler; `on` and an event name is one.
    #[test]
    fn docs_page_tells_a_handler_from_an_on_attribute() {
        for innocent in [
            "<div id=\"swagger-ui\" on=\"x\"></div>",
            "<div id=\"swagger-ui\" on-x=\"y\"></div>",
            "<div id=\"swagger-ui\" data-onclick=\"go()\"></div>",
        ] {
            let violations = DocsTree::clean()
                .plant(DOCS_HTML, "<div id=\"swagger-ui\"></div>", innocent)
                .violations();
            let handlers: Vec<&String> = violations
                .iter()
                .filter(|v| v.contains("handler"))
                .collect();
            assert!(handlers.is_empty(), "{innocent:?}: {violations:#?}");
        }
        let spaced = DocsTree::clean()
            .plant(
                DOCS_HTML,
                "<div id=\"swagger-ui\"></div>",
                "<div id=\"swagger-ui\" onclick  =  \"go()\"></div>",
            )
            .violations();
        assert_named(&spaced, &["api_docs.html:14", "onclick= handler"]);
        let spaced_style = DocsTree::clean()
            .plant(
                DOCS_HTML,
                "<div id=\"swagger-ui\"></div>",
                "<div id=\"swagger-ui\" style =\"color: red\"></div>",
            )
            .violations();
        assert_named(&spaced_style, &["api_docs.html:14", "style= attribute"]);
        let empty_script = DocsTree::clean()
            .plant(
                DOCS_HTML,
                "<div id=\"swagger-ui\"></div>",
                "<script src=\"/static/js/x.js\">\n </script>",
            )
            .violations();
        assert!(
            empty_script.is_empty(),
            "a blank script body is no inline code: {empty_script:#?}"
        );
    }

    /// Clause (vi): only a `swagger-ui-*` directory is a vendored release; a
    /// file so named, or another vendored file, is not read as one.
    #[test]
    fn docs_page_reads_only_swagger_ui_directories() {
        let mut tree = DocsTree::clean();
        tree.files.push((
            "crates/foundry-app/static/vendor/swagger-ui-x",
            "@import \"theme.css\";".to_string(),
        ));
        tree.files.push((
            "crates/foundry-app/static/vendor/htmx.min.js",
            "x".to_string(),
        ));
        tree.files.push((
            "crates/foundry-app/static/vendor/htmx-2/swagger-ui.css",
            "@import \"theme.css\";".to_string(),
        ));
        let violations = tree.violations();
        assert!(violations.is_empty(), "{violations:#?}");
        let mut second = DocsTree::clean();
        second.files.push((
            "crates/foundry-app/static/vendor/swagger-ui-6.0.0/swagger-ui.css",
            ".a{}\n.b{background:url(/x.png)}".to_string(),
        ));
        assert_named(
            &second.violations(),
            &["api-docs-page", "swagger-ui-6.0.0/swagger-ui.css:2", "url("],
        );
    }

    /// Clause (v): the page's script-src stays `'self'` alone (DDD-19).
    #[test]
    fn docs_page_names_a_loose_script_policy() {
        for loosened in [
            "script-src 'self' 'unsafe-inline'",
            "script-src 'self' *",
            "script-src *",
            "script-src 'self' 'unsafe-eval'",
            "script-src 'self' https://cdn.example",
        ] {
            let violations = DocsTree::clean()
                .plant(DOCS_RS, "script-src 'self'", loosened)
                .violations();
            assert_named(
                &violations,
                &["api-docs-page", "api_docs.rs", "API_DOCS_CSP", loosened],
            );
        }
        let dropped = DocsTree::clean()
            .plant(DOCS_RS, " script-src 'self';", "")
            .violations();
        assert_named(
            &dropped,
            &["api-docs-page", "api_docs.rs", "API_DOCS_CSP", "script-src"],
        );
    }

    /// Clause (vi): the vendored stylesheet loads nothing (review H3).
    #[test]
    fn docs_page_names_a_remote_reference_in_the_css() {
        for (planted, what) in [
            ("@import \"theme.css\";", "@import"),
            ("@font-face{font-family:x;src:local(x)}", "@font-face"),
            (".c{background:url(https://cdn.example/x.png)}", "url("),
            (".c{background:url('/static/x.png')}", "url("),
        ] {
            let violations = DocsTree::clean()
                .plant(
                    DOCS_CSS,
                    ".swagger-ui .a{",
                    &format!("{planted}.swagger-ui .a{{"),
                )
                .violations();
            assert_named(&violations, &["api-docs-page", "swagger-ui.css:2", what]);
        }
        let mut unvendored = DocsTree::clean();
        unvendored.files.retain(|(path, _)| *path != DOCS_CSS);
        assert_named(
            &unvendored.violations(),
            &["api-docs-page", "swagger-ui.css"],
        );
    }
}
