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

    // LAYER 2 — cargo-deny crate-graph dependency-direction.
    if let Some(dep_violation) = check_dependency_direction(&root) {
        violations.push(dep_violation);
    }

    if violations.is_empty() {
        println!("check-arch: boundary guard PASSED (api≠HTML, api≠ad-hoc-authz, api≠mint, JWT alg pinned to [EdDSA], dependency direction)");
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
}
