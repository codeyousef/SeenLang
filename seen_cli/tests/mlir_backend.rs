use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn mlir_backend_produces_textual_module() {
    let dir = tempdir().expect("create temp dir");
    let source_path = dir.path().join("main.seen");
    fs::write(
        &source_path,
        r#"
fun Helper(value: Int) -> Int {
    value + 1
}

fun Main(input: Int) -> Int {
    if input > 0 {
        Helper(input)
    } else {
        0
    }
}
"#,
    )
    .expect("write source");

    let mlir_out = dir.path().join("out.mlir");

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
            "mlir",
            "--output",
            mlir_out.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&mlir_out).expect("read mlir output");
    assert!(
        contents.contains("module @"),
        "expected MLIR module header, got:\n{}",
        contents
    );
    assert!(
        contents.contains("func.func @"),
        "expected a function lowering in MLIR output"
    );
    assert!(
        contents.contains("transform.module @seen_pipeline"),
        "expected transform pipeline stub in MLIR output"
    );
}

#[test]
fn mlir_backend_determinism_succeeds() {
    let dir = tempdir().expect("create temp dir");
    let source_path = dir.path().join("main.seen");
    fs::write(
        &source_path,
        r#"
fun Increment(value: Int) -> Int {
    value + 1
}

fun Main(x: Int) -> Int {
    if x > 10 {
        Increment(x)
    } else {
        10
    }
}
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
            "-O0",
            "--backend",
            "mlir",
        ])
        .assert()
        .success();
}
