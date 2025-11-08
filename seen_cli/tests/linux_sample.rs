use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
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

    Command::new(cargo_bin!("seen_cli"))
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

    let original_ndk_home = std::env::var("ANDROID_NDK_HOME").ok();

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

    let mut cmd = Command::new(cargo_bin!("seen_cli"));
    cmd.current_dir(&workspace);
    cmd.env_remove("ANDROID_NDK_HOME");

    cmd.args([
        "build",
        sample.to_string_lossy().as_ref(),
        "--backend",
        "llvm",
        "--target",
        "aarch64-linux-android",
        "--output",
        output.to_string_lossy().as_ref(),
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("ANDROID_NDK_HOME must be set"));

    match original_ndk_home {
        Some(value) => std::env::set_var("ANDROID_NDK_HOME", value),
        None => std::env::remove_var("ANDROID_NDK_HOME"),
    }
}

#[test]
fn linux_shared_library_builds() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping linux_shared_library_builds because LLVM feature is disabled");
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
    let output = temp.path().join("libhello_cli.so");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            sample.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--shared",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output.exists(),
        "shared library output should exist at {:?}",
        output
    );
}

#[test]
fn linux_static_library_builds() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping linux_static_library_builds because LLVM feature is disabled");
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
    let output = temp.path().join("libhello_cli.a");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            sample.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--static",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output.exists(),
        "static library output should exist at {:?}",
        output
    );
}
