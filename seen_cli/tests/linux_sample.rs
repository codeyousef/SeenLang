use assert_cmd::Command;
use predicates::str::contains;
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

#[test]
fn android_target_requires_ndk_home() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping android_target_requires_ndk_home test because LLVM feature is disabled"
        );
        return;
    }

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
    let output = temp.path().join("hello_android");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .env_remove("ANDROID_NDK_HOME")
        .args([
            "build",
            sample.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "aarch64-linux-android",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .failure()
        .stderr(contains("ANDROID_NDK_HOME must be set"));
}
