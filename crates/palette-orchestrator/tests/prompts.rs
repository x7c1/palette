//! Contract tests for the worker prompt files that the Orchestrator copies
//! into member containers. The prompts encode how the Orchestrator and the
//! Worker share responsibility for branch creation and plan access — if
//! those responsibilities drift, these tests catch the drift early.

use std::fs;
use std::path::PathBuf;

/// Path to the palette crate root (the `palette/` directory), derived from the
/// orchestrator crate's manifest dir.
fn palette_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for this crate is `palette/crates/palette-orchestrator`.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("palette root")
        .to_path_buf()
}

#[test]
fn crafter_prompt_does_not_ask_worker_to_create_a_branch() {
    let prompt = fs::read_to_string(palette_root().join("prompts/crafter.md"))
        .expect("prompts/crafter.md must exist");
    assert!(
        !prompt.contains("git checkout -b"),
        "crafter prompt must not instruct the Worker to create a branch; \
         the Orchestrator owns that responsibility"
    );
    assert!(
        !prompt.contains("{branch}"),
        "crafter prompt must not carry unfilled {{branch}} placeholders"
    );
}

#[test]
fn crafter_prompt_documents_push_is_handled_by_palette() {
    let prompt = fs::read_to_string(palette_root().join("prompts/crafter.md"))
        .expect("prompts/crafter.md must exist");
    assert!(
        prompt.to_lowercase().contains("do not"),
        "crafter prompt should continue to tell the Worker not to push"
    );
    assert!(
        prompt.contains("push"),
        "crafter prompt should still mention push handling"
    );
}

#[test]
fn craft_reviewer_prompt_reads_plan_path_verbatim() {
    let prompt = fs::read_to_string(palette_root().join("prompts/craft-reviewer.md"))
        .expect("prompts/craft-reviewer.md must exist");
    assert!(
        prompt.contains("verbatim") || prompt.contains("read the path verbatim"),
        "craft-reviewer prompt should instruct the Worker to read the Plan: line verbatim"
    );
}
