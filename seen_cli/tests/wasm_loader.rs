use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
    cmd.arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("ir")
        .arg("--wasm-loader");

    cmd.assert()
        .failure()
        .stderr(contains("--wasm-loader requires the LLVM backend"));
}

#[test]
fn wasm_loader_reports_missing_wasm_linker() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping wasm_loader_reports_missing_wasm_linker test because LLVM feature is disabled");
        return;
    }

    let dir = tempdir().expect("temp dir");
    let program_path = write_program(&dir);
    let missing_linker = dir.path().join("missing-wasm-ld");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
    cmd.env(
        "SEEN_LLVM_LINKER",
        missing_linker.to_string_lossy().as_ref(),
    )
        .arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("llvm")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--wasm-loader");

    cmd.assert().failure().stderr(contains("SEEN_LLVM_LINKER"));
}

#[test]
fn bundle_requires_wasm_loader() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping bundle_requires_wasm_loader because LLVM feature is disabled");
        return;
    }

    let dir = tempdir().expect("temp dir");
    let program_path = write_program(&dir);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
    cmd.arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("llvm")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--bundle");

    cmd.assert()
        .failure()
        .stderr(contains("--bundle requires --wasm-loader"));
}

#[test]
fn bundle_requires_wasm_target() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping bundle_requires_wasm_target because LLVM feature is disabled");
        return;
    }

    let dir = tempdir().expect("temp dir");
    let program_path = write_program(&dir);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
    cmd.arg("build")
        .arg(program_path)
        .arg("--backend")
        .arg("llvm")
        .arg("--wasm-loader")
        .arg("--bundle");

    cmd.assert()
        .failure()
        .stderr(contains("--wasm-loader requires --target wasm32"));
}
