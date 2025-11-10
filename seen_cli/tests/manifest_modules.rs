use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn create_manifest_project() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("create temp dir");
    let main = temp.path().join("main.seen");
    fs::write(
        &main,
        r#"
fun main() -> Int {
    return 0
}
"#,
    )
    .expect("write main.seen");

    fs::write(
        temp.path().join("Seen.toml"),
        r#"[project]
modules = ["missing/module"]
"#,
    )
    .expect("write Seen.toml");

    (temp, main)
}

#[test]
fn manifest_modules_disabled_by_default() {
    let (project, main) = create_manifest_project();
    let output = project.path().join("out.ir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .args([
            "build",
            main.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "build without manifest env should succeed");
}

#[test]
fn manifest_modules_require_env_to_load() {
    let (project, main) = create_manifest_project();
    let output = project.path().join("out.ir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args([
            "build",
            main.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .failure()
        .stderr(contains("Failed to read module path"));
}

#[test]
fn manifest_modules_include_dependencies() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_dir = temp.path().join("app");
    let app_src = app_dir.join("src");
    fs::create_dir_all(&app_src).expect("app src dir");

    let main_path = app_src.join("main.seen");
    fs::write(
        &main_path,
        r#"
fun entry() -> Int {
    return getValue()
}

entry()
"#,
    )
        .expect("write main");

    fs::write(
        app_dir.join("Seen.toml"),
        r#"[project]
modules = ["src"]
language = "en"

[dependencies]
helper = "../deps/helper"

[build]
targets = ["native"]
"#,
    )
        .expect("write app Seen.toml");

    let dep_dir = temp.path().join("deps").join("helper");
    let dep_src = dep_dir.join("src");
    fs::create_dir_all(&dep_src).expect("dep src dir");
    fs::write(
        dep_src.join("lib.seen"),
        r#"
fun getValue() -> Int {
    return 42
}
"#,
    )
        .expect("write dep module");

    fs::write(
        dep_dir.join("Seen.toml"),
        r#"[project]
modules = ["src/lib.seen"]
language = "en"

[build]
targets = ["native"]
"#,
    )
        .expect("write dep Seen.toml");

    let output = app_dir.join("out.ir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args([
            "build",
            main_path.to_string_lossy().as_ref(),
            "--backend",
            "ir",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "expected build artifact at {:?}", output);
}

#[test]
fn manifest_modules_run_with_dependency() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_dir = temp.path().join("app");
    let app_src = app_dir.join("src");
    fs::create_dir_all(&app_src).expect("app src dir");

    let main_path = app_src.join("main.seen");
    fs::write(
        &main_path,
        r#"
fun entry() -> Int {
    return getValue()
}

entry()
"#,
    )
        .expect("write main");

    fs::write(
        app_dir.join("Seen.toml"),
        r#"[project]
modules = ["src"]
language = "en"

[dependencies]
helper = "../deps/helper"
"#,
    )
        .expect("write app Seen.toml");

    let dep_dir = temp.path().join("deps").join("helper");
    let dep_src = dep_dir.join("src");
    fs::create_dir_all(&dep_src).expect("dep src dir");
    fs::write(
        dep_src.join("lib.seen"),
        r#"
fun getValue() -> Int {
    return 42
}
"#,
    )
        .expect("write dep module");

    fs::write(
        dep_dir.join("Seen.toml"),
        r#"[project]
modules = ["src/lib.seen"]
language = "en"
"#,
    )
        .expect("write dep Seen.toml");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args(["run", main_path.to_string_lossy().as_ref()])
        .assert()
        .success()
        .stdout(contains("42"));
}

#[test]
fn manifest_std_vec_smoke_test() {
    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(workspace_root())
        .env("SEEN_ENABLE_MANIFEST_MODULES", "1")
        .args(["run", "seen_std/tests/vec_basic.seen"])
        .assert()
        .success();
}
