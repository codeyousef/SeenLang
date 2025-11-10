use assert_cmd::Command;
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::NamedTempFile;

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

#[test]
fn runtime_trace_capture_and_replay() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping runtime_trace_capture_and_replay because LLVM feature is disabled");
        return;
    }

    let workspace = workspace_root();
    let sample = workspace
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("main.seen");
    assert!(sample.exists(), "sample should exist at {:?}", sample);

    let trace_file = NamedTempFile::new().expect("temp trace file");
    let trace_path = trace_file.path().to_path_buf();

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "trace",
            sample.to_string_lossy().as_ref(),
            "--runtime",
            trace_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .stdout(contains("Captured runtime trace"));

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["trace", "--replay", trace_path.to_string_lossy().as_ref()])
        .assert()
        .success()
        .stdout(contains("[program] start"));
}
