use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use serde_json::Value;
use std::fs;
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

    Command::new(cargo_bin!("seen_cli"))
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

    Command::new(cargo_bin!("seen_cli"))
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

    Command::new(cargo_bin!("seen_cli"))
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

#[test]
fn deterministic_profile_run_rejects_hardware_overrides() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping deterministic_profile_run_rejects_hardware_overrides because the LLVM backend is not enabled in tests"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "--profile",
            "deterministic",
            "run",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--cpu-feature",
            "avx10-256",
        ])
        .assert()
        .failure()
        .stderr(contains("--cpu-feature").and(contains("deterministic")));
}

#[test]
fn deterministic_profile_rejects_simd_override() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping deterministic_profile_rejects_simd_override because the LLVM backend is not enabled in tests"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "--profile",
            "deterministic",
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--simd",
            "max",
        ])
        .assert()
        .failure()
        .stderr(contains("--simd").and(contains("deterministic")));
}

#[test]
fn mlir_output_embeds_cpu_features() {
    let workspace = workspace_root();
    let source = sample_source();
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("hardware.mlir");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "mlir",
            "--cpu-feature",
            "avx10-512",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&output).expect("read mlir output");
    assert!(
        contents.contains("seen.cpu_features = [\"avx10-512\"]"),
        "expected MLIR output to include cpu feature annotation, got:\n{}",
        contents
    );
    assert!(
        contents.contains("seen.vector_width = 512"),
        "expected MLIR output to include vector width"
    );
    assert!(
        contents.contains("seen.register_budget ="),
        "expected MLIR output to include register budget hint"
    );
    assert!(
        contents.contains("seen.scheduler = \"vector\""),
        "expected MLIR output to include scheduler hint"
    );
}

#[test]
fn clif_output_includes_hardware_header() {
    let workspace = workspace_root();
    let source = sample_source();
    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("hardware.clif");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "clif",
            "--cpu-feature",
            "sve256",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&output).expect("read clif output");
    assert!(
        contents.contains("; cpu-features: sve256"),
        "expected CLIF output to list cpu feature header"
    );
    assert!(
        contents.contains("; max-vector-bits: 256"),
        "expected CLIF header to include vector width"
    );
    assert!(
        contents.contains("; seen.register_budget ="),
        "expected CLIF output to include per-function register budget comment"
    );
    assert!(
        contents.contains("; seen.scheduler ="),
        "expected CLIF output to include scheduler comment"
    );
}

#[test]
fn simd_report_writes_json() {
    let workspace = workspace_root();
    let source = sample_source();
    let temp = tempdir().expect("temp dir");
    let report_path = temp.path().join("simd_report.json");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--simd",
            "max",
            "--simd-report",
            report_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&report_path).expect("read simd report");
    let parsed: Value = serde_json::from_str(&contents).expect("valid json");
    assert_eq!(parsed["policy"], "max");
}
