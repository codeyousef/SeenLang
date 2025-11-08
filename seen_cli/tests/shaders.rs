use assert_cmd::Command;
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn sample_shader() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("shaders")
        .join("triangle.spv")
}

#[test]
fn shaders_command_emits_wgsl_and_msl() {
    let shader = sample_shader();
    assert!(shader.exists(), "sample shader missing: {:?}", shader);
    let temp = tempdir().expect("temp dir");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace_root())
        .args([
            "shaders",
            shader.to_string_lossy().as_ref(),
            "--output",
            temp.path().to_string_lossy().as_ref(),
            "--target",
            "wgsl",
            "--target",
            "msl",
        ])
        .assert()
        .success()
        .stdout(contains("validated"));

    assert!(temp.path().join("triangle.wgsl").exists());
    assert!(temp.path().join("triangle.metal").exists());
}

#[test]
fn validate_only_skips_outputs() {
    let shader = sample_shader();
    let temp = tempdir().expect("temp dir");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace_root())
        .args([
            "shaders",
            shader.to_string_lossy().as_ref(),
            "--output",
            temp.path().to_string_lossy().as_ref(),
            "--validate-only",
        ])
        .assert()
        .success()
        .stdout(contains("validated"));

    assert!(!temp.path().join("triangle.wgsl").exists());
    assert!(!temp.path().join("triangle.metal").exists());
}

#[test]
fn directory_scans_require_recursive_flag() {
    let dir = workspace_root().join("examples");
    let temp = tempdir().expect("temp dir");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace_root())
        .args([
            "shaders",
            dir.to_string_lossy().as_ref(),
            "--output",
            temp.path().to_string_lossy().as_ref(),
        ])
        .assert()
        .failure()
        .stderr(contains("No .spv files"));

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace_root())
        .args([
            "shaders",
            dir.to_string_lossy().as_ref(),
            "--output",
            temp.path().to_string_lossy().as_ref(),
            "--recursive",
        ])
        .assert()
        .success();

    assert!(temp.path().join("triangle.wgsl").exists());
}
