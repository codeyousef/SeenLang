use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn run_command_propagates_abort_errors() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let program_path = dir.path().join("abort_test.seen");

    fs::write(&program_path, r#"__Abort("integration abort marker")"#)?;

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"));
    cmd.arg("run").arg(&program_path);

    cmd.assert()
        .failure()
        .stderr(contains("Abort").and(contains("integration abort marker")));

    Ok(())
}
