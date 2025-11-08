use assert_cmd::Command;
use predicates::str::contains;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn trace_emits_function_body() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping trace_emits_function_body because LLVM feature is disabled");
        return;
    }

    let workspace = workspace_root();
    let sample = workspace
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("main.seen");
    assert!(sample.exists(), "sample should exist at {:?}", sample);

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["trace", sample.to_string_lossy().as_ref(), "-O1"])
        .assert()
        .success()
        .stdout(contains("fn main"));
}

#[test]
fn trace_reports_parse_errors() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping trace_reports_parse_errors because LLVM feature is disabled");
        return;
    }

    let workspace = workspace_root();
    let bad_file = workspace
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("missing.seen");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["trace", bad_file.to_string_lossy().as_ref()])
        .assert()
        .failure();
}
