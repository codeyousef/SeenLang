use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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

fn read_report(path: &Path) -> Value {
    let data = fs::read_to_string(path).expect("report contents");
    serde_json::from_str(&data).expect("valid json report")
}

#[test]
fn simd_report_reflects_policy_modes() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping simd_report_reflects_policy_modes because the LLVM backend is not enabled"
        );
        return;
    }

    let workspace = workspace_root();
    let source = sample_source();
    assert!(source.exists(), "expected sample source at {:?}", source);

    let temp = tempdir().expect("temp dir");
    let off_report = temp.path().join("simd_off.json");
    run_with_simd_policy(&workspace, &source, "off", &off_report);
    assert!(
        off_report.exists(),
        "expected off report at {:?}",
        off_report
    );

    let off = read_report(&off_report);
    assert_eq!(
        off["requested_policy"].as_str(),
        Some("off"),
        "requested policy should reflect CLI flag"
    );
    let off_functions = off["functions"]
        .as_array()
        .expect("functions array should exist");
    assert!(
        !off_functions.is_empty(),
        "expected at least one function in report"
    );
    assert_eq!(off_functions[0]["mode"].as_str(), Some("scalar"));
    assert_eq!(off_functions[0]["reason"].as_str(), Some("policy_off"));

    let max_report = temp.path().join("simd_max.json");
    run_with_simd_policy(&workspace, &source, "max", &max_report);
    assert!(
        max_report.exists(),
        "expected max report at {:?}",
        max_report
    );
    let max = read_report(&max_report);
    assert_eq!(max["requested_policy"].as_str(), Some("max"));
    let max_functions = max["functions"]
        .as_array()
        .expect("functions array should exist");
    assert!(
        !max_functions.is_empty(),
        "expected at least one function in max report"
    );
    assert_eq!(max_functions[0]["mode"].as_str(), Some("vectorized"));
    assert_eq!(
        max_functions[0]["reason"].as_str(),
        Some("policy_forced_max")
    );
    let speedup = max_functions[0]["estimated_speedup"]
        .as_f64()
        .expect("speedup recorded");
    assert!(
        speedup >= 1.0,
        "expected non-zero speedup, found {}",
        speedup
    );
}

fn run_with_simd_policy(workspace: &Path, source: &Path, policy: &str, report_path: &Path) {
    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--simd",
            policy,
            "--simd-report",
            report_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();
}
