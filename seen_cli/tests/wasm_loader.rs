use assert_cmd::prelude::*;
use predicates::str::contains;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn write_program(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let program_path = dir.path().join("main.seen");
    fs::write(
        &program_path,
        r#"
fun main() -> Int {
    return 0
}
"#,
    )
        .expect("write source");
    program_path
}

#[test]
fn wasm_loader_requires_wasm_target() {
    let dir = tempdir().expect("temp dir");
    let program_path = write_program(&dir);

    let mut cmd = Command::cargo_bin("seen_cli").expect("binary exists");
    cmd.arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("llvm")
        .arg("--wasm-loader");

    cmd.assert()
        .failure()
        .stderr(contains("--wasm-loader requires --target wasm32"));
}

#[test]
fn wasm_loader_disallowed_on_ir_backend() {
    let dir = tempdir().expect("temp dir");
    let program_path = write_program(&dir);

    let mut cmd = Command::cargo_bin("seen_cli").expect("binary exists");
    cmd.arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("ir")
        .arg("--wasm-loader");

    cmd.assert()
        .failure()
        .stderr(contains("--wasm-loader requires the LLVM backend"));
}
