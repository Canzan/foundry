// build.rs — the build stamp (release-version-footer US-RVF-02).
//
// Bakes the short commit id and the commit date into foundry-app as two
// compile-time environment variables, `FOUNDRY_BUILD_SHA` and
// `FOUNDRY_BUILD_DATE` (DDD-1). `src/views.rs` reads them with `env!` and the
// site footer on every full page renders them, so a running server can say
// which build it is: the release names the version, the date and `data-commit`
// name the commit. The version itself stays `CARGO_PKG_VERSION` (D6), so this
// script emits no version output.
//
// THREE PROPERTIES, in priority order (the canzan-lift `build.rs` precedent):
//
// (1) NO NEW CRATES. No `[build-dependencies]`: this is `std::process::Command`
//     shelling out to `git`.
//
// (2) THE BUILD NEVER FAILS BECAUSE OF THIS (D10, DDD-4). No `git` on PATH, not
//     a repository, a source tarball, a Docker context without `.git` — each one
//     degrades to the honest placeholder `unknown`. There is no `unwrap`, no
//     `expect` and no `?` on a git or env result anywhere below.
//
// (3) THE STAMP DOES NOT GO STALE (D11). A footer that names the WRONG commit is
//     worse than none. See `emit_rerun_directives`.
//
// WHERE EACH FIELD COMES FROM, in precedence order (D9, DDD-2):
//
//   1. `FOUNDRY_STAMP_SHA` / `FOUNDRY_STAMP_DATE`, when set and not blank. The
//      container build has no `.git` (`.dockerignore`), so the publish
//      workflows hand the commit in as build-args. Blank counts as absent
//      because a Dockerfile `ARG X=` without `--build-arg` arrives EMPTY. The
//      inputs are named STAMP, not BUILD, so an input never reads as "set the
//      answer".
//   2. `git` (DDD-3: `rev-parse --short=7 HEAD`, `log -1 --format=%cd
//      --date=short` — the commit's own date, never the build time).
//   3. `unknown`.
//
// `src/build_stamp_tests.rs` compiles this very file (`include!`) to drive
// `stamp` and `UNKNOWN`; that is why those two are `pub(crate)`.

use std::env;
use std::path::Path;
use std::process::Command;

/// The placeholder a field carries when nobody could answer. Pinned equal to
/// `views::BUILD_FIELD_UNKNOWN` by a unit test (DDD-5).
pub(crate) const UNKNOWN: &str = "unknown";

/// The explicit inputs. Distinct from the `FOUNDRY_BUILD_*` outputs on purpose.
const STAMP_SHA_INPUT: &str = "FOUNDRY_STAMP_SHA";
const STAMP_DATE_INPUT: &str = "FOUNDRY_STAMP_DATE";

fn main() {
    emit_rerun_directives();

    let sha = stamp(given(STAMP_SHA_INPUT).as_deref(), || {
        git(&["rev-parse", "--short=7", "HEAD"])
    });
    let date = stamp(given(STAMP_DATE_INPUT).as_deref(), || {
        git(&["log", "-1", "--format=%cd", "--date=short"])
    });
    println!("cargo:rustc-env=FOUNDRY_BUILD_SHA={sha}");
    println!("cargo:rustc-env=FOUNDRY_BUILD_DATE={date}");
}

/// One stamp field: the explicit input when it says something (trimmed), else
/// git's answer, else `UNKNOWN`. `from_git` runs only when needed — an explicit
/// stamp means git's answer would be thrown away. The blank-as-absent rule
/// lives here and nowhere else.
pub(crate) fn stamp(explicit: Option<&str>, from_git: impl FnOnce() -> Option<String>) -> String {
    match explicit.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.to_string(),
        None => from_git().unwrap_or_else(|| UNKNOWN.to_string()),
    }
}

/// The raw explicit input, or `None` when unset or not UTF-8 — never an error.
fn given(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// `git <args>`'s trimmed stdout, or `None` for every failure: no `git` binary,
/// a non-zero exit, non-UTF-8 or empty output. stderr is inherited so
/// `cargo build -vv` still shows why a stamp degraded.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

/// Tell cargo exactly which inputs invalidate the stamp (DDD-6, revised).
///
/// Emitting ANY `rerun-if-changed` replaces cargo's default heuristic with this
/// list alone, so the list must be complete:
///
///   * `build.rs` and both `FOUNDRY_STAMP_*` inputs — setting, blanking or
///     dropping an input must re-stamp (in the container these are the ONLY
///     triggers).
///   * `HEAD` — moves on a branch switch, and IS the commit when detached.
///   * the branch ref HEAD points at — what a COMMIT changes (`HEAD` still says
///     `ref: refs/heads/<branch>`). When its loose file is ABSENT (after
///     `git pack-refs`, or a fresh clone) the nearest existing parent directory
///     is watched instead: the next commit creates the loose file inside it,
///     and cargo scans a registered directory's contents. Watching nothing
///     there would leave the stamp stale.
///   * `packed-refs` — where a packed branch tip lives.
///
/// Every path comes from `git rev-parse --git-path` (correct in a linked
/// worktree) and is canonicalised. A path that does not exist is never
/// registered: cargo would rerun the script on every build. If git cannot
/// answer, only the first three directives are emitted.
fn emit_rerun_directives() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={STAMP_SHA_INPUT}");
    println!("cargo:rerun-if-env-changed={STAMP_DATE_INPUT}");

    if let Some(head) = git_path("HEAD") {
        rerun_if_present(Path::new(&head));
    }

    // `symbolic-ref` fails on a detached HEAD, which is correct: `HEAD` holds
    // the commit itself there, and there is no branch tip to watch.
    if let Some(branch_ref) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        if let Some(loose_ref) = git_path(&branch_ref) {
            rerun_on_nearest_existing(Path::new(&loose_ref));
        }
    }

    if let Some(packed) = git_path("packed-refs") {
        rerun_if_present(Path::new(&packed));
    }
}

/// Where git keeps `name` for this checkout (`git rev-parse --git-path`).
fn git_path(name: &str) -> Option<String> {
    git(&["rev-parse", "--git-path", name])
}

/// Register `path`, or — when it does not exist — its nearest existing ancestor
/// directory, so a file appearing there later still re-runs the script.
fn rerun_on_nearest_existing(path: &Path) {
    if let Some(existing) = path.ancestors().find(|candidate| candidate.exists()) {
        rerun_if_present(existing);
    }
}

/// Register `path` with cargo only if it exists, as a canonical absolute path
/// (`--git-path` may answer relative to the manifest dir, or with a path in a
/// worktree's common dir). If canonicalisation fails, emit nothing.
fn rerun_if_present(path: &Path) {
    let Ok(absolute) = path.canonicalize() else {
        return;
    };
    println!("cargo:rerun-if-changed={}", absolute.display());
}
