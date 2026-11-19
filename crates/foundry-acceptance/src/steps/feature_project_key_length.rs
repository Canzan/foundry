//! The one-home law for the sentence that refuses a project key (L7 / T1).
//!
//! The law is observed from outside, through the gate's own command: a
//! throwaway copy of the tree carries a planted second copy of the sentence and
//! the gate must refuse that copy and name the file holding it, while the
//! untouched copy must be accepted. Staging a copy and running the gate binary
//! follows the form the boundary-guard steps already use.

use crate::world::FoundryWorld;
use cucumber::{given, then, when};

/// The gate copies the tree, builds a binary and runs a nested dependency
/// check; one scenario at a time keeps the lane deterministic.
static GATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The sentence in the wordings a second copy could carry. The gate must bite
/// whichever of them the refusal ends up reading.
const PLANTED_SENTENCE_FILE: &str = "planted_project_key_sentence.rs";

fn planted_source() -> String {
    [
        "// A second copy of the project key refusal, planted to see the gate bite.",
        "pub const A: &str = \"project key must be 2 to 10 capital letters A to Z\";",
        "pub const B: &str = \"Project key must be 2 to 10 capital letters A to Z\";",
        "pub const C: &str = \"project key must be 2 to 10 capital letters A-Z\";",
        "pub const D: &str = \"Project key must be 2 to 10 capital letters A-Z\";",
    ]
    .join("\n")
        + "\n"
}

fn crate_dir_for(area: &str) -> &'static str {
    match area {
        "web" => "crates/foundry-app/src",
        "services" => "crates/foundry-services/src",
        "store" => "crates/foundry-store/src",
        other => panic!("no crate registered for the {other:?} code"),
    }
}

fn workspace_root() -> std::path::PathBuf {
    // crates/foundry-acceptance -> the workspace root is two levels up.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn copy_tree_excluding(
    from: &std::path::Path,
    to: &std::path::Path,
    skip: &[&str],
) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip.iter().any(|s| std::ffi::OsStr::new(s) == name) {
            continue;
        }
        let kind = entry.file_type()?;
        let source = entry.path();
        let target = to.join(&name);
        if kind.is_dir() {
            copy_tree_excluding(&source, &target, skip)?;
        } else if kind.is_file() {
            std::fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

#[given(
    regex = r"^a throwaway copy of the codebase carries a second copy of the project key refusal sentence in the (\w+) code$"
)]
async fn copy_carries_a_second_sentence(world: &mut FoundryWorld, area: String) {
    world.fa_guard_violation = Some(area);
}

#[given(regex = r"^a throwaway copy of the codebase with no second sentence planted in it$")]
async fn copy_carries_nothing(world: &mut FoundryWorld) {
    world.fa_guard_violation = None;
}

#[when(regex = r"^a contributor runs the architecture gate over that copy$")]
async fn runs_the_architecture_gate(world: &mut FoundryWorld) {
    let _serialised = GATE_LOCK.lock().await;
    let root = workspace_root();
    let planted = world.fa_guard_violation.clone();

    let staged = tokio::task::spawn_blocking({
        let root = root.clone();
        let planted = planted.clone();
        move || -> std::io::Result<tempfile::TempDir> {
            let copy = tempfile::Builder::new()
                .prefix("foundry-project-key-one-source-")
                .tempdir()?;
            copy_tree_excluding(&root, copy.path(), &["target", ".git"])?;
            if let Some(area) = &planted {
                let dir = copy.path().join(crate_dir_for(area));
                std::fs::create_dir_all(&dir)?;
                std::fs::write(dir.join(PLANTED_SENTENCE_FILE), planted_source())?;
            }
            Ok(copy)
        }
    })
    .await
    .expect("join the staging task")
    .expect("stage a throwaway copy of the codebase");

    let gate = ensure_gate_built(&root).await;
    let over = staged.path().to_path_buf();
    let cwd = root.clone();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&gate)
            .current_dir(&cwd)
            .args(["check-arch", "--root"])
            .arg(&over)
            .output()
    })
    .await
    .expect("join the gate run")
    .expect("run the architecture gate");

    world.fa_guard_exit_code = output.status.code();
    let mut said = String::new();
    said.push_str(&String::from_utf8_lossy(&output.stdout));
    said.push_str(&String::from_utf8_lossy(&output.stderr));
    world.fa_guard_stderr = Some(said);
    drop(staged);
}

async fn ensure_gate_built(root: &std::path::Path) -> std::path::PathBuf {
    let cwd = root.to_path_buf();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(env!("CARGO"))
            .current_dir(&cwd)
            .args(["build", "-q", "-p", "xtask"])
            .status()
    })
    .await
    .expect("join the gate build")
    .expect("build the gate");
    assert!(status.success(), "the gate binary could not be built");
    root.join("target/debug/xtask")
}

#[then(regex = r"^the gate refuses the copy and names the file that holds the second sentence$")]
async fn gate_refuses_and_names_it(world: &mut FoundryWorld) {
    let said = world.fa_guard_stderr.clone().unwrap_or_default();
    let code = world.fa_guard_exit_code;
    assert!(
        code.is_some_and(|c| c != 0),
        "WHAT: the gate accepted a tree carrying a second copy of the project key refusal \
         sentence (it ended with {code:?}). WHY: two copies of the sentence drift apart, and \
         the operator then reads a rule the product no longer enforces. HOW: fail the gate on \
         any copy outside the refusal that owns it. The gate said:\n{said}"
    );
    assert!(
        said.contains(PLANTED_SENTENCE_FILE),
        "WHAT: the gate refused the tree without naming the file that holds the second \
         sentence. WHY: a contributor has to be told which file to delete the copy from. \
         HOW: name the file and line of every copy. The gate said:\n{said}"
    );
}

#[then(regex = r"^the gate accepts the copy$")]
async fn gate_accepts(world: &mut FoundryWorld) {
    let said = world.fa_guard_stderr.clone().unwrap_or_default();
    assert_eq!(
        world.fa_guard_exit_code,
        Some(0),
        "WHAT: the gate refused the codebase as it stands. WHY: a gate that refuses an \
         untouched tree says nothing about a planted copy. HOW: keep the gate quiet on a tree \
         holding one sentence. The gate said:\n{said}"
    );
}
