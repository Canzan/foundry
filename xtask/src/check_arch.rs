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
            "check-arch: boundary guard PASSED (api≠HTML, api≠ad-hoc-authz, api≠mint, JWT alg pinned to [EdDSA] + OIDC to [RS256], tenant-scoping by resolved ActingWorkspace, single slugify in foundry-core, no static lane list in app/api, the lanes position constraint is still DEFERRABLE, no board-*.js registers a keydown listener, nothing outside migration 0017 UPDATEs users.provisioned_at (D9), both publish workflows stamp every image with its commit and the Dockerfile hands it to build.rs (AC-8), every /static reference resolves, every content-hashed filename is its own sha256 prefix, every VENDOR.md sha256 recomputes, no colour literal outside the three stylesheet token regions, the three stylesheet token regions declare the identical colour-token set, dependency direction)"
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
    let chars: Vec<char> = source.chars().collect();
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
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
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
            }
            '"' => {
                let opened_on = line;
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
                literals.push((opened_on, text));
            }
            'r' if i == 0 || !is_ident(chars[i - 1]) || chars[i - 1] == 'b' => {
                let hashes = chars[i + 1..].iter().take_while(|&&c| c == '#').count();
                if chars.get(i + 1 + hashes) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let opened_on = line;
                let closing: Vec<char> = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let mut text = String::new();
                i += hashes + 2;
                while i < chars.len() && !chars[i..].starts_with(&closing) {
                    line += usize::from(chars[i] == '\n');
                    text.push(chars[i]);
                    i += 1;
                }
                i += closing.len();
                literals.push((opened_on, text));
            }
            '\'' if next == Some('\\') => {
                i += 3;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            }
            '\'' if chars.get(i + 2) == Some(&'\'') => i += 3,
            _ => i += 1,
        }
    }
    literals
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

/// The offset of the `}` closing the `{` at `open`, or `None` if it never does.
fn matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
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
    const LAYER_1_RULE_MARKERS: [&str; 14] = [
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
                 let row = state.store.find_attachment_in_workspace(id, ws).await;\n",
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
        let clean = stage(&[
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
