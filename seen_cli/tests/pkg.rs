use assert_cmd::Command;
use predicates::str::contains;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use zip::ZipArchive;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn pkg_creates_seenpkg_with_default_name() {
    let workspace = workspace_root();
    let input = workspace
        .join("seen_cli")
        .join("tests")
        .join("fixtures")
        .join("pkg_project");
    assert!(input.is_dir(), "expected fixture directory at {:?}", input);

    let temp = tempdir().expect("temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["pkg", input.to_string_lossy().as_ref()])
        .assert()
        .success();

    let expected = workspace.join("libpkg_fixture-1.2.3.seenpkg");
    assert!(expected.exists(), "expected package at {:?}", expected);

    let temp_archive = temp.path().join("libpkg_fixture-1.2.3.seenpkg");
    fs::copy(&expected, &temp_archive).expect("copy archive to temp");
    fs::remove_file(&expected).expect("cleanup workspace artifact");

    let file = File::open(&temp_archive).expect("open archive");
    let mut archive = ZipArchive::new(file).expect("zip archive");
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).expect("entry");
        entries.push(file.name().to_string());
    }
    assert!(
        entries.iter().any(|name| name.ends_with("src/lib.seen")),
        "expected lib.seen in archive, entries: {:?}",
        entries
    );
}

#[test]
fn pkg_requires_seen_lock() {
    let temp = tempdir().expect("temp dir");
    let project_dir = copy_pkg_fixture(temp.path());
    let lock_path = project_dir.join("Seen.lock");
    fs::remove_file(&lock_path).expect("remove lock file");

    let workspace = workspace_root();

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["pkg", project_dir.to_string_lossy().as_ref()])
        .assert()
        .failure()
        .stderr(contains("Seen.lock"));

    assert!(
        !lock_path.exists(),
        "lock file should remain deleted in temp copy"
    );
}

#[test]
fn pkg_detects_hash_mismatch() {
    let temp = tempdir().expect("temp dir");
    let project_dir = copy_pkg_fixture(temp.path());
    let src = project_dir.join("src").join("lib.seen");
    fs::write(&src, "fun tamper() -> Int { return 42 }\n").expect("rewrite source");

    let workspace = workspace_root();

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["pkg", project_dir.to_string_lossy().as_ref()])
        .assert()
        .failure()
        .stderr(contains("hash mismatch"));
}

fn copy_pkg_fixture(temp_root: &Path) -> PathBuf {
    let workspace = workspace_root();
    let fixture = workspace
        .join("seen_cli")
        .join("tests")
        .join("fixtures")
        .join("pkg_project");
    let destination = temp_root.join("pkg_fixture_copy");
    copy_dir_recursive(&fixture, &destination);
    destination
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create destination dir");
    for entry in fs::read_dir(src).expect("read src") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            fs::copy(&path, &target).expect("copy file");
        }
    }
}
