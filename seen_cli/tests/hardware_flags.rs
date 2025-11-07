use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn sample_source() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("main.seen")
}

#[test]
fn cpu_feature_flag_emits_ll() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping cpu_feature_flag_emits_ll because the LLVM backend is not enabled in tests"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();
    assert!(
        source.exists(),
        "sample source should exist at {:?}",
        source
    );

    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("hardware.ll");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--emit-ll",
            "--cpu-feature",
            "avx10-512",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "expected LLVM IR output at {:?}", output);
}

#[test]
fn memory_topology_hint_succeeds() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping memory_topology_hint_succeeds because the LLVM backend is not enabled in tests"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("memory.ll");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--emit-ll",
            "--memory-topology",
            "cxl-far",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "expected LLVM IR output at {:?}", output);
}

#[test]
fn deterministic_profile_rejects_hardware_overrides() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping deterministic_profile_rejects_hardware_overrides because the LLVM backend is not enabled in tests"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .args([
            "--profile",
            "deterministic",
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--emit-ll",
            "--cpu-feature",
            "avx10-512",
            "--memory-topology",
            "cxl-near",
        ])
        .assert()
        .failure()
        .stderr(contains("--cpu-feature").and(contains("deterministic")));
}
