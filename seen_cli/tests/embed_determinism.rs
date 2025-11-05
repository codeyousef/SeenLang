use assert_cmd::cargo::cargo_bin;
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

    Command::from(cargo_bin("seen_cli"))
        .current_dir(workspace_root)
        .args(["determinism", source.to_string_lossy().as_ref(), "-O0"])
        .assert()
        .success();
}
