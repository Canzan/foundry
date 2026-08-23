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

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Run the boundary guard. `args` is everything after `check-arch`.
pub fn run(args: Vec<String>) -> ExitCode {
    let root = match parse_root(&args) {
        Ok(root) => root,
        Err(message) => {
            eprintln!("check-arch: {message}");
            return ExitCode::from(2);
        }
    };

    let mut violations: Vec<String> = Vec::new();

    // LAYER 1 — AST / source walk.
    violations.extend(check_api_no_html(&root));
    violations.extend(check_api_no_adhoc_authz(&root));
    violations.extend(check_api_no_mint_route(&root));
    violations.extend(check_jwt_alg_pin(&root));
    violations.extend(check_oidc_alg_pin(&root));
    violations.extend(check_app_tenant_scoping(&root));
    violations.extend(check_app_no_slugify_definition(&root));
    violations.extend(check_no_static_lane_list(&root));

    // LAYER 2 — cargo-deny crate-graph dependency-direction.
    if let Some(dep_violation) = check_dependency_direction(&root) {
        violations.push(dep_violation);
    }

    if violations.is_empty() {
        println!("check-arch: boundary guard PASSED (api≠HTML, api≠ad-hoc-authz, api≠mint, JWT alg pinned to [EdDSA] + OIDC to [RS256], tenant-scoping by resolved ActingWorkspace, single slugify in foundry-core, no static lane list in app/api, dependency direction)");
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "check-arch: boundary guard FAILED — {} violation(s):",
        violations.len()
    );
    for violation in &violations {
        eprintln!("  - {violation}");
    }
    ExitCode::from(1)
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
fn workspace_root() -> PathBuf {
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

        idx = end + 1;
    }
    hits
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
             let att = state.store.find_attachment_in_workspace(id, user.workspace_id).await;\n",
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
             let row = state.store.find_attachment_in_workspace(id, ws).await;\n",
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
}
