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
//     worse than none. See `emit_rerun_directives` and the pure path choice
//     it delegates to (`git_watch_paths`, `ref_watch_path`).
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
// `stamp`, `UNKNOWN`, `git_watch_paths` and `ref_watch_path`; that is why
// those four are `pub(crate)`.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
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
///   * the git paths `git_watch_paths` chooses: `HEAD`, the branch tip and
///     `packed-refs`.
///
/// Every git path comes from `git rev-parse --git-path` (correct in a linked
/// worktree) and is canonicalised by `rerun_if_present`. If git cannot answer,
/// only the first three directives are emitted.
fn emit_rerun_directives() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={STAMP_SHA_INPUT}");
    println!("cargo:rerun-if-env-changed={STAMP_DATE_INPUT}");

    let head = git_path("HEAD");
    // `symbolic-ref` fails on a detached HEAD, which is correct: `HEAD` holds
    // the commit itself there, and there is no branch tip to watch.
    let branch_ref = git(&["symbolic-ref", "--quiet", "HEAD"]).and_then(|branch| git_path(&branch));
    let packed_refs = git_path("packed-refs");

    for path in git_watch_paths(
        head.as_deref().map(Path::new),
        branch_ref.as_deref().map(Path::new),
        packed_refs.as_deref().map(Path::new),
        Path::exists,
    ) {
        rerun_if_present(&path);
    }
}

/// Where git keeps `name` for this checkout (`git rev-parse --git-path`).
fn git_path(name: &str) -> Option<String> {
    git(&["rev-parse", "--git-path", name])
}

/// The git paths cargo must watch, in order, each only if it EXISTS (a
/// `rerun-if-changed` on a missing path makes cargo rerun the script on every
/// build):
///
///   * `head` — moves on a branch switch, and IS the commit when detached.
///   * the branch tip, chosen by `ref_watch_path` from `branch_ref` (the loose
///     ref HEAD points at; `None` when detached) — what a COMMIT changes.
///   * `packed_refs` — where a packed branch tip lives.
///
/// Pure over `exists` so `src/build_stamp_tests.rs` can drive it.
pub(crate) fn git_watch_paths(
    head: Option<&Path>,
    branch_ref: Option<&Path>,
    packed_refs: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let present = |path: Option<&Path>| path.filter(|path| exists(path)).map(Path::to_path_buf);
    let branch_tip = branch_ref.and_then(|path| ref_watch_path(path, &exists));
    [present(head), branch_tip, present(packed_refs)]
        .into_iter()
        .flatten()
        .collect()
}

/// The path to watch for the branch tip at `ref_path`: the loose ref file when
/// it exists, else its nearest existing ancestor directory. The loose file is
/// ABSENT after `git pack-refs` or in a fresh clone, and the next commit
/// CREATES it; cargo scans a watched directory's contents, so the file
/// appearing there reruns the script. The climb stops at the `refs` directory:
/// watching the git dir itself would rerun the script on every index write.
/// `None` means there is nothing safe to watch; `HEAD` and `packed-refs` are
/// then all cargo has.
pub(crate) fn ref_watch_path(ref_path: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if exists(ref_path) {
        return Some(ref_path.to_path_buf());
    }
    let parents_up_to_refs = ref_path
        .ancestors()
        .skip(1)
        .position(|dir| dir.file_name() == Some(OsStr::new("refs")))?
        + 1;
    ref_path
        .ancestors()
        .skip(1)
        .take(parents_up_to_refs)
        .find(|dir| exists(dir))
        .map(Path::to_path_buf)
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
