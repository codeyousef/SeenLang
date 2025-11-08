use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn format_check_flags_unformatted_code() {
    let workspace = workspace_root();
    let temp_dir = tempdir().expect("temp dir");
    let source = temp_dir.path().join("main.seen");
    fs::write(&source, "fun main()->Int {\nreturn 0\n}\n").expect("write source");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["format", source.to_string_lossy().as_ref(), "--check"])
        .assert()
        .failure()
        .stderr(contains("not formatted"));
}

#[test]
fn format_check_passes_after_fix() {
    let workspace = workspace_root();
    let temp_dir = tempdir().expect("temp dir");
    let source = temp_dir.path().join("main.seen");
    fs::write(&source, "fun main()->Int {\nreturn 0\n}\n").expect("write source");

    // First format in place
    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["format", source.to_string_lossy().as_ref(), "--in-place"])
        .assert()
        .success();

    // Then check succeeds
    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["format", source.to_string_lossy().as_ref(), "--check"])
        .assert()
        .success();
}
