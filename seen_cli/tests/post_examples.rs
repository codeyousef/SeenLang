use assert_cmd::Command;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn vulkan_sample_source() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("seen-vulkan-min")
        .join("src")
        .join("main.seen")
}

fn ecs_sample_source() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("seen-ecs-min")
        .join("src")
        .join("main.seen")
}

#[test]
fn seen_vulkan_min_runs_with_interpreter_backend() {
    let workspace = workspace_root();
    let sample = vulkan_sample_source();
    assert!(
        sample.exists(),
        "vulkan sample should exist at {:?}",
        sample
    );

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["run", sample.to_string_lossy().as_ref()])
        .assert()
        .success();
}

#[test]
fn seen_ecs_min_runs_with_interpreter_backend() {
    let workspace = workspace_root();
    let sample = ecs_sample_source();
    assert!(sample.exists(), "ecs sample should exist at {:?}", sample);

    Command::new(assert_cmd::cargo::cargo_bin!("seen_cli"))
        .current_dir(&workspace)
        .args(["run", sample.to_string_lossy().as_ref()])
        .assert()
        .success();
}
