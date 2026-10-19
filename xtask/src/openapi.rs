//! `cargo xtask openapi [--check] [--root <dir>]` (api-openapi-document DDD-13,
//! DEVOPS R2).
//!
//! The committed API description (`crates/foundry-api/openapi.json`, served
//! verbatim at `/api/v1/openapi.json`) carries the release in `info.version`.
//! The release is the `[package] version` of `crates/foundry-api/Cargo.toml`
//! (every crate is bumped together, RELEASING.md step 3, so it is also the
//! footer's version).
//!
//! * Without a flag the command sets `info.version` to the release and touches
//!   no other byte: it swaps the one JSON string literal in place rather than
//!   re-serialising the document, and writes nothing when it is already
//!   current, so a rerun is a no-op.
//! * `--check` is read-only (the Dockerfile builder stage runs it): it exits 0
//!   when the two versions agree and 1, naming both and the fix, when not.
//! * `--root` points the command at another tree (the check-arch convention);
//!   it defaults to the workspace root.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

const DESCRIPTION: &str = "crates/foundry-api/openapi.json";
const MANIFEST: &str = "crates/foundry-api/Cargo.toml";

/// The command's exit codes: done, refused (stale or unreadable), misused.
const OK: u8 = 0;
const REFUSED: u8 = 1;
const USAGE: u8 = 2;

/// What one run of the command did: its exit code and what it says.
#[derive(Debug)]
pub struct Outcome {
    pub code: u8,
    pub message: String,
}

impl Outcome {
    fn new(code: u8, message: String) -> Self {
        Self { code, message }
    }

    pub fn succeeded(&self) -> bool {
        self.code == OK
    }
}

pub fn run(args: Vec<String>) -> ExitCode {
    let outcome = execute(&args);
    if outcome.succeeded() {
        println!("{}", outcome.message);
    } else {
        eprintln!("{}", outcome.message);
    }
    ExitCode::from(outcome.code)
}

/// The driving port: the command's arguments in, its exit code and message out.
pub fn execute(args: &[String]) -> Outcome {
    match Invocation::parse(args) {
        Ok(invocation) => invocation.perform(),
        Err(problem) => Outcome::new(
            USAGE,
            format!(
                "xtask openapi :: {problem}\nusage: cargo xtask openapi [--check] [--root <dir>]"
            ),
        ),
    }
}

struct Invocation {
    check: bool,
    root: PathBuf,
}

impl Invocation {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut check = false;
        let mut root = None;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--check" => check = true,
                "--root" => {
                    let dir = iter.next().ok_or("--root requires a directory argument")?;
                    root = Some(PathBuf::from(dir));
                }
                other => return Err(format!("unexpected argument {other:?}")),
            }
        }
        Ok(Self {
            check,
            root: root.unwrap_or_else(crate::check_arch::workspace_root),
        })
    }

    fn perform(&self) -> Outcome {
        let tree = match Tree::read(&self.root) {
            Ok(tree) => tree,
            Err(problem) => return Outcome::new(REFUSED, format!("xtask openapi :: {problem}")),
        };
        if self.check {
            tree.check()
        } else {
            tree.refresh(&self.root)
        }
    }
}

/// The two values the command compares, read from one tree.
struct Tree {
    release: String,
    documented: String,
    description: String,
}

impl Tree {
    fn read(root: &Path) -> Result<Self, String> {
        let manifest = read(root, MANIFEST)?;
        let release = package_version(&manifest)
            .ok_or_else(|| format!("{MANIFEST} declares no literal `version = \"…\"`"))?;
        let description = read(root, DESCRIPTION)?;
        let documented = documented_version(&description)?;
        Ok(Self {
            release,
            documented,
            description,
        })
    }

    fn is_current(&self) -> bool {
        self.documented == self.release
    }

    fn check(&self) -> Outcome {
        if self.is_current() {
            return Outcome::new(
                OK,
                format!(
                    "xtask openapi --check :: {DESCRIPTION} info.version {} is the release ({MANIFEST})",
                    self.release
                ),
            );
        }
        Outcome::new(
            REFUSED,
            format!(
                "xtask openapi --check :: {DESCRIPTION} is stale: info.version is {} but the release ({MANIFEST}) is {}. Run `cargo xtask openapi` and commit the file.",
                self.documented, self.release
            ),
        )
    }

    fn refresh(&self, root: &Path) -> Outcome {
        if self.is_current() {
            return Outcome::new(
                OK,
                format!(
                    "xtask openapi :: {DESCRIPTION} info.version is already {}; nothing changed",
                    self.release
                ),
            );
        }
        let rewritten = match with_version(&self.description, &self.release) {
            Ok(text) => text,
            Err(problem) => return Outcome::new(REFUSED, format!("xtask openapi :: {problem}")),
        };
        if let Err(err) = std::fs::write(root.join(DESCRIPTION), rewritten) {
            return Outcome::new(
                REFUSED,
                format!("xtask openapi :: write {DESCRIPTION}: {err}"),
            );
        }
        Outcome::new(
            OK,
            format!(
                "xtask openapi :: {DESCRIPTION} info.version {} -> {}",
                self.documented, self.release
            ),
        )
    }
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(rel)).map_err(|err| format!("read {rel}: {err}"))
}

/// The first `version = "…"` line of a crate manifest: its `[package]` version.
fn package_version(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let value = line
            .trim()
            .strip_prefix("version")?
            .trim_start()
            .strip_prefix('=')?;
        let quoted = value.trim().strip_prefix('"')?;
        quoted
            .split_once('"')
            .map(|(version, _)| version.to_string())
    })
}

fn parse(description: &str) -> Result<Value, String> {
    serde_json::from_str(description).map_err(|err| format!("{DESCRIPTION} is not JSON: {err}"))
}

fn documented_version(description: &str) -> Result<String, String> {
    parse(description)?["info"]["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{DESCRIPTION} has no string info.version"))
}

/// `description` with `info.version` set to `release` and every other byte
/// kept. Each occurrence of the current version's JSON literal is tried in
/// turn; the one whose swap yields exactly the document with only
/// `info.version` changed is `info.version`'s own.
fn with_version(description: &str, release: &str) -> Result<String, String> {
    let mut wanted = parse(description)?;
    let current = wanted["info"]["version"].clone();
    wanted["info"]["version"] = Value::String(release.to_string());
    let literal = current.to_string();
    let replacement = Value::String(release.to_string()).to_string();
    description
        .match_indices(&literal)
        .map(|(at, _)| {
            format!(
                "{}{replacement}{}",
                &description[..at],
                &description[at + literal.len()..]
            )
        })
        .find(|candidate| parse(candidate).is_ok_and(|doc| doc == wanted))
        .ok_or_else(|| {
            format!("{DESCRIPTION}: info.version {literal} is not written as a plain JSON string")
        })
}
