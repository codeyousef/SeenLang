use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn create_manifest_project() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("create temp dir");
    let main = temp.path().join("main.seen");
    fs::write(
        &main,
        r#"
fun main() -> Int {
    return 0
}
"#,
    )
    .expect("write main.seen");

    fs::write(
        temp.path().join("Seen.toml"),
        r#"[project]
modules = ["missing/module"]
"#,
    )
    .expect("write Seen.toml");

    (temp, main)
}

#[test]
fn manifest_modules_disabled_by_default() {
    let (project, main) = create_manifest_project();
    let output = project.path().join("out.ir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .args([
            "build",
            main.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "build without manifest env should succeed");
}

#[test]
fn manifest_modules_require_env_to_load() {
    let (project, main) = create_manifest_project();
    let output = project.path().join("out.ir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args([
            "build",
            main.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .failure()
        .stderr(contains("Failed to read module path"));
}
