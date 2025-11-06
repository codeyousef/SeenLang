use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn linux_sample_builds_to_ir() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let sample = workspace
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("main.seen");
    assert!(sample.exists(), "linux sample should exist at {:?}", sample);

    let temp = tempdir().expect("create temp dir");
    let output = temp.path().join("hello.ir");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .args([
            "build",
            sample.to_string_lossy().as_ref(),
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "IR output should exist at {:?}", output);
}
