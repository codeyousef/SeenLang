use assert_cmd::Command;
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
fn wasm_emit_ll_succeeds_without_linker() {
    if !cfg!(feature = "llvm") {
        eprintln!("skipping wasm_emit_ll_succeeds_without_linker because LLVM feature is disabled");
        return;
    }

    let workspace = workspace_root();
    let source = workspace
        .join("examples")
        .join("web")
        .join("hello_wasm")
        .join("main.seen");
    assert!(source.exists(), "web sample should exist at {:?}", source);

    let temp = tempdir().expect("create temp dir");
    let output = temp.path().join("hello.ll");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "wasm32-unknown-unknown",
            "--emit-ll",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output.exists(),
        "LLVM IR output should exist at {:?}",
        output
    );
}

#[test]
fn wasm_loader_generates_artifacts_with_custom_linker() {
    if !cfg!(feature = "llvm") {
        eprintln!(
            "skipping wasm_loader_generates_artifacts_with_custom_linker because LLVM feature is disabled"
        );
        return;
    }

    let workspace = workspace_root();
    let source = workspace
        .join("examples")
        .join("web")
        .join("hello_wasm")
        .join("main.seen");
    assert!(source.exists(), "web sample should exist at {:?}", source);

    let temp = tempdir().expect("temp dir");
    let linker_path = temp.path().join("fake-wasm-ld.sh");
    std::fs::write(
        &linker_path,
        r#"#!/bin/sh
set -eu
out=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then
        shift
        if [ "$#" -eq 0 ]; then
            echo "fake wasm-ld: missing output path" >&2
            exit 1
        fi
        out="$1"
    fi
    shift || true
done
if [ -z "$out" ]; then
    echo "fake wasm-ld: no output specified" >&2
    exit 1
fi
printf '\0asm' > "$out"
exit 0
"#,
    )
    .expect("write fake linker script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&linker_path)
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&linker_path, perms).expect("chmod");
    }

    let wasm_output = temp.path().join("hello.wasm");

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .env("SEEN_LLVM_LINKER", linker_path.to_string_lossy().as_ref())
        .args([
            "build",
            source.to_string_lossy().as_ref(),
            "--backend",
            "llvm",
            "--target",
            "wasm32-unknown-unknown",
            "--wasm-loader",
            "--bundle",
            "--output",
            wasm_output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let js_loader = wasm_output.with_extension("js");
    let html_bootstrap = wasm_output.with_extension("html");
    let bundle_zip = wasm_output.with_extension("zip");

    assert!(
        wasm_output.exists(),
        "expected wasm output at {:?}",
        wasm_output
    );
    assert!(js_loader.exists(), "expected js loader at {:?}", js_loader);
    assert!(
        html_bootstrap.exists(),
        "expected html bootstrap at {:?}",
        html_bootstrap
    );
    assert!(
        bundle_zip.exists(),
        "expected wasm bundle at {:?}",
        bundle_zip
    );

    let archive = File::open(&bundle_zip).expect("open bundle");
    let mut zip = ZipArchive::new(archive).expect("parse zip");
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i).expect("zip entry");
        entries.push(file.name().to_string());
    }
    assert!(entries.contains(&"hello.wasm".to_string()));
    assert!(entries.iter().any(|name| name.ends_with(".js")));
    assert!(entries.iter().any(|name| name.ends_with(".html")));
}
