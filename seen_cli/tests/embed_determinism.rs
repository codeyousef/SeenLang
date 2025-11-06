use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn embed_determinism_succeeds() {
    let dir = tempdir().expect("create temp dir");
    let assets_dir = dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("create assets dir");

    let blob_path = assets_dir.join("blob.bin");
    fs::write(&blob_path, &[0xAA, 0xBB, 0xCC, 0xDD]).expect("write blob");

    let source = dir.path().join("main.seen");
    fs::write(
        &source,
        r#"
#[embed(path="assets/blob.bin")]
const DATA = 0

fun Main() -> Int {
    42
}
"#,
    )
    .expect("write source");

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(workspace_root)
        .args(["determinism", source.to_string_lossy().as_ref(), "-O0"])
        .assert()
        .success();
}

#[test]
fn embed_llvm_artifact_is_deterministic() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping embed_llvm_artifact_is_deterministic because LLVM feature is disabled");
        return;
    }

    let dir = tempdir().expect("create temp dir");
    let assets_dir = dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("create assets dir");

    let blob_path = assets_dir.join("payload.bin");
    fs::write(&blob_path, b"Embed deterministic payload").expect("write blob");

    let source = dir.path().join("main.seen");
    fs::write(
        &source,
        r#"
#[embed(path="assets/payload.bin")]
const DATA = 0

fun Main() -> Int {
    1
}
"#,
    )
        .expect("write source");

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();

    let out1 = dir.path().join("artifact_one.ll");
    let out2 = dir.path().join("artifact_two.ll");

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace_root)
        .args([
            "--profile",
            "deterministic",
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--output",
            out1.to_string_lossy().as_ref(),
            "--emit-ll",
        ])
        .assert()
        .success();

    Command::cargo_bin("seen_cli")
        .expect("binary exists")
        .current_dir(&workspace_root)
        .args([
            "--profile",
            "deterministic",
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--output",
            out2.to_string_lossy().as_ref(),
            "--emit-ll",
        ])
        .assert()
        .success();

    let bytes_one = fs::read(&out1).expect("read first artifact");
    let bytes_two = fs::read(&out2).expect("read second artifact");
    assert_eq!(
        bytes_one, bytes_two,
        "deterministic profile should emit identical binaries"
    );
}
