use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn compiler_frontend_smoke_test_passes() {
    let workspace = workspace_root();
    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace.join("compiler_seen"))
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args(["run", "tests/frontend_smoke.seen"])
        .assert()
        .success();
}

#[test]
fn compiler_compile_smoke_runs() {
    let workspace = workspace_root();
    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace.join("compiler_seen"))
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args(["run", "tests/compile_smoke.seen"])
        .assert()
        .success();
}

#[test]
fn compiler_refuses_seen_invocations() {
    let workspace = workspace_root();
    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace.join("compiler_seen"))
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args(["run", "tests/forbid_seen_shell.seen"])
        .assert()
        .success();
}

#[test]
fn compiler_complete_lexer_parses() {
    let workspace = workspace_root();
    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["parse", "compiler_seen/src/lexer/complete_lexer.seen"])
        .assert()
        .success();
}
