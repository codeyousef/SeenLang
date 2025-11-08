use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn clif_backend_produces_textual_module() {
    let dir = tempdir().expect("create temp dir");
    let source_path = dir.path().join("main.seen");
    fs::write(
        &source_path,
        r#"
fun Add(lhs: Int, rhs: Int) -> Int {
    lhs + rhs
}
"#,
    )
        .expect("write source");

    let clif_out = dir.path().join("out.clif");
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(workspace_root)
        .args([
            "build",
            source_path.to_string_lossy().as_ref(),
            "--backend",
            "clif",
            "--output",
            clif_out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&clif_out).expect("read clif output");
    assert!(
        contents.contains("function %Add"),
        "expected Cranelift function header, got:\n{}",
        contents
    );
    assert!(
        contents.contains("iadd.i64"),
        "expected Cranelift binary op lowering"
    );
}

#[test]
fn clif_backend_determinism_succeeds() {
    let dir = tempdir().expect("create temp dir");
    let source_path = dir.path().join("main.seen");
    fs::write(
        &source_path,
        r#"
fun Id(x: Int) -> Int { x }
"#,
    )
        .expect("write source");

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(workspace_root)
        .args([
            "determinism",
            source_path.to_string_lossy().as_ref(),
            "--backend",
            "clif",
        ])
        .assert()
        .success();
}
