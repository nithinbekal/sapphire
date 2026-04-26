use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sapphire-{prefix}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn write_file(path: &Path, contents: &str) {
    fs::write(path, contents).expect("failed to write test file");
}

#[test]
fn run_command_exits_on_type_error_before_execution() {
    let dir = unique_temp_dir("run");
    let file = dir.join("main.spr");
    write_file(
        &file,
        "def takes_int(x: Int) { x }\ntakes_int(\"oops\")\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sapphire"))
        .arg("run")
        .arg(&file)
        .output()
        .expect("failed to run sapphire binary");

    assert!(
        !output.status.success(),
        "expected failure, got status {:?}",
        output.status
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expected Int, got String"),
        "unexpected stderr: {stderr}"
    );

    fs::remove_dir_all(dir).expect("failed to remove temp dir");
}

#[test]
fn test_command_exits_on_type_error_before_running_tests() {
    let dir = unique_temp_dir("test");
    let file = dir.join("sample_test.spr");
    write_file(
        &file,
        "def takes_int(x: Int) { x }\ntakes_int(\"oops\")\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sapphire"))
        .arg("test")
        .arg(&dir)
        .output()
        .expect("failed to run sapphire binary");

    assert!(
        !output.status.success(),
        "expected failure, got status {:?}",
        output.status
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expected Int, got String"),
        "unexpected stderr: {stderr}"
    );

    fs::remove_dir_all(dir).expect("failed to remove temp dir");
}
