use assert_cmd::{cargo::cargo_bin, Command};
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn android_cross_build_produces_shared_object() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping android_cross_build_produces_shared_object because LLVM feature is disabled"
        );
        return;
    }

    let ndk_home = std::env::var("ANDROID_NDK_HOME").ok();
    if ndk_home.is_none() {
        eprintln!("skipping android_cross_build_produces_shared_object because ANDROID_NDK_HOME is not set");
        return;
    }

    let workspace = workspace_root();
    let source = workspace
        .join("examples")
        .join("android")
        .join("hello_ndk")
        .join("main.seen");
    assert!(
        source.exists(),
        "android sample should exist at {:?}",
        source
    );

    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("libhello_android.so");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "aarch64-linux-android",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output.exists(),
        "expected Android shared object at {:?}",
        output
    );
}

#[test]
fn android_bundle_requires_shared_output() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping android_bundle_requires_shared_output because LLVM feature is disabled"
        );
        return;
    }

    let workspace = workspace_root();
    let source = workspace
        .join("examples")
        .join("linux")
        .join("hello_cli")
        .join("main.seen");

    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("android_bundle_test");

    let mut cmd = Command::cargo_bin("seen_cli").expect("binary exists");
    cmd.current_dir(&workspace)
        .env("ANDROID_NDK_HOME", temp.path().to_string_lossy().as_ref())
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "aarch64-linux-android",
            "--bundle",
            "--output",
            output.to_string_lossy().as_ref(),
        ]);

    cmd.assert()
        .failure()
        .stderr(contains("Android bundling requires --output <name>.aab"));
}

#[test]
fn android_bundle_script_requires_ndk() {
    let workspace = workspace_root();
    let script = workspace.join("scripts").join("bundle_android.sh");
    assert!(
        script.exists(),
        "bundle script should exist at {:?}",
        script
    );

    let source = workspace
        .join("examples")
        .join("android")
        .join("hello_ndk")
        .join("main.seen");
    assert!(
        source.exists(),
        "android sample should exist at {:?}",
        source
    );

    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("bundle.aab");

    let seen_bin = cargo_bin("seen_cli");

    let mut cmd = Command::new("bash");
    cmd.arg(script)
        .arg(&seen_bin)
        .arg(&source)
        .arg(&output)
        .env_remove("ANDROID_NDK_HOME");

    cmd.assert()
        .failure()
        .stderr(contains("ANDROID_NDK_HOME must be set"));
}
