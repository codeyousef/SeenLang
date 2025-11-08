use assert_cmd::{cargo::cargo_bin, cargo_bin, Command};
use predicates::str::contains;
use std::fs::File;
use std::path::PathBuf;
use tempfile::tempdir;
use zip::ZipArchive;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn pkg_creates_zip_archive() {
    let workspace = workspace_root();
    let input = workspace.join("examples").join("linux").join("hello_cli");
    assert!(input.is_dir(), "expected directory at {:?}", input);

    let temp = tempdir().expect("temp dir");
    let output = temp.path().join("hello_pkg.zip");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "pkg",
            input.to_string_lossy().as_ref(),
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output.exists(), "expected archive at {:?}", output);

    let file = File::open(&output).expect("open archive");
    let mut archive = ZipArchive::new(file).expect("zip archive");
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).expect("entry");
        entries.push(file.name().to_string());
    }
    assert!(entries.iter().any(|name| name.ends_with("main.seen")));
}

#[test]
fn pkg_missing_directory_fails() {
    let workspace = workspace_root();
    let missing = workspace.join("examples").join("does_not_exist");

    Command::new(cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["pkg", missing.to_string_lossy().as_ref()])
        .assert()
        .failure()
        .stderr(contains("does not exist"));
}
