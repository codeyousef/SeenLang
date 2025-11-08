use assert_cmd::{cargo::cargo_bin, cargo_bin, prelude::*};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn run_channel_select_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().expect("workspace root").to_path_buf();
    let sample = workspace_root.join("seen_cli/tests/fixtures/channel_select.seen");

    let assert = Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace_root)
        .args(["run", sample.to_string_lossy().as_ref()])
        .assert()
        .success();

    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout was not UTF-8");
    let mut lines: Vec<_> = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    assert!(
        lines.len() >= 3,
        "expected at least three output lines (two prints + result), got {:?}",
        lines
    );

    let result = lines.pop().expect("missing result line");
    assert_eq!(result, "3", "expected final result to be 3");

    let mut observed = HashSet::new();
    for line in lines {
        observed.insert(line.to_string());
    }

    assert!(observed.contains("1"), "expected to see output '1'");
    assert!(observed.contains("2"), "expected to see output '2'");
}

#[test]
fn build_channel_select_with_llvm_backend() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping LLVM channel select build because LLVM feature is disabled");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().expect("workspace root").to_path_buf();
    let sample = workspace_root.join("seen_cli/tests/fixtures/channel_select.seen");
    let temp = tempdir().expect("create temp dir");
    let artifact = temp.path().join("channel_select");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace_root)
        .args([
            "build",
            sample.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--output",
            artifact.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    Command::new(&artifact)
        .current_dir(temp.path())
        .assert()
        .success();
}

#[test]
fn run_channel_select_with_llvm_backend() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping LLVM channel select run because LLVM feature is disabled");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().expect("workspace root").to_path_buf();
    let sample = workspace_root.join("seen_cli/tests/fixtures/channel_select.seen");

    let assert = Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace_root)
        .args([
            "run",
            sample.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
        ])
        .assert()
        .success();

    let stdout =
        String::from_utf8(assert.get_output().stdout.clone()).expect("stdout was not UTF-8");
    assert!(
        stdout.lines().any(|line| line.trim() == "3"),
        "expected LLVM run to print final result 3, got {stdout}"
    );
}
